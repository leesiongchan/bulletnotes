//! Four bounded network lanes with shared admission serializing each page.
use super::{
    SilverBulletClient,
    probe::{self, CompatibilityEvidence},
};
use crate::{
    domain::{connection::ConnectionId, error::SyncError, page::*},
    store::{CacheProgress, PendingHead, StoreHandle},
};
use std::{
    collections::{BTreeMap, HashSet},
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use tokio::{
    sync::{Notify, mpsc, watch},
    task::JoinHandle,
    time::Instant,
};
const LANES: usize = 4;
use super::{
    catalogue::DownloadQueue,
    schedule::{Retry, WriteDebounce},
};
#[derive(Clone)]
pub struct SyncSnapshot {
    pub connection: ConnectionId,
    pub writable: bool,
    pub indexed: usize,
    pub catalogue_total: usize,
    pub complete: bool,
    pub pending: usize,
    pub conflicts: usize,
    pub error: Option<SyncError>,
    pub page_errors: BTreeMap<PageKey, SyncError>,
    pub generation: u64,
}
#[derive(Clone)]
struct Control {
    active: bool,
    refresh: u64,
    stop: bool,
    priority: Option<PageKey>,
}
struct Inner {
    control: watch::Sender<Control>,
    snapshot: watch::Receiver<SyncSnapshot>,
    task: tokio::sync::Mutex<Option<JoinHandle<Result<(), SyncError>>>>,
}
#[derive(Clone)]
pub struct SyncHandle(Arc<Inner>);
impl SyncHandle {
    pub fn start(
        store: StoreHandle,
        client: SilverBulletClient,
        evidence: Option<CompatibilityEvidence>,
    ) -> Result<Self, SyncError> {
        let runtime = tokio::runtime::Handle::try_current().map_err(|_| SyncError::Incompatible)?;
        let writable = evidence.is_some_and(|evidence| evidence.permits(&client));
        let connection = client.connection_id().clone();
        let initial = SyncSnapshot {
            connection,
            writable,
            indexed: 0,
            catalogue_total: 0,
            complete: false,
            pending: 0,
            conflicts: 0,
            error: None,
            page_errors: BTreeMap::new(),
            generation: 0,
        };
        let (snapshot, receiver) = watch::channel(initial);
        let shared = Arc::new(Shared {
            snapshot,
            notify: Notify::new(),
            active: Mutex::new(HashSet::new()),
            retry: Mutex::new(Retry {
                attempt: 0,
                until: Instant::now(),
                random: uuid::Uuid::new_v4().as_u128() as u64,
            }),
        });
        let (control, controls) = watch::channel(Control {
            active: true,
            refresh: 0,
            stop: false,
            priority: None,
        });
        let task = runtime.spawn(supervise(store, client, shared, controls));
        Ok(Self(Arc::new(Inner {
            control,
            snapshot: receiver,
            task: tokio::sync::Mutex::new(Some(task)),
        })))
    }
    pub fn set_active(&self, active: bool) {
        self.0.control.send_modify(|control| {
            if active && !control.active {
                control.refresh = control.refresh.wrapping_add(1);
            }
            control.active = active;
        });
    }
    pub fn refresh(&self) {
        self.0
            .control
            .send_modify(|control| control.refresh = control.refresh.wrapping_add(1));
    }
    pub fn request_page(&self, key: PageKey) {
        if key.connection != self.0.snapshot.borrow().connection {
            return;
        }
        self.0.control.send_modify(|control| {
            control.priority = Some(key);
            control.refresh = control.refresh.wrapping_add(1);
        });
    }
    pub fn snapshot(&self) -> SyncSnapshot {
        self.0.snapshot.borrow().clone()
    }
    pub fn subscribe(&self) -> watch::Receiver<SyncSnapshot> {
        self.0.snapshot.clone()
    }
    pub async fn shutdown(&self) -> Result<(), SyncError> {
        self.0.control.send_modify(|control| control.stop = true);
        if let Some(task) = self.0.task.lock().await.take() {
            task.await.map_err(|_| SyncError::LocalStorage)??;
        }
        Ok(())
    }
}
struct Shared {
    snapshot: watch::Sender<SyncSnapshot>,
    notify: Notify,
    retry: Mutex<Retry>,
    active: Mutex<HashSet<PageKey>>,
}
impl Shared {
    fn reserve(&self, key: &PageKey) -> bool {
        self.active
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(key.clone())
    }
    fn release(&self, key: &PageKey) {
        self.active
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(key);
        self.notify.notify_one();
    }
    fn busy(&self, key: &PageKey) -> bool {
        self.active
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .contains(key)
    }
    fn blocked(&self) -> bool {
        self.snapshot.borrow().error.is_some_and(|error| {
            !matches!(
                error,
                SyncError::Network | SyncError::UnknownMutationOutcome
            )
        })
    }
    fn page_blocked(&self, key: &PageKey) -> bool {
        self.snapshot.borrow().page_errors.contains_key(key)
    }
    fn ready(&self) -> bool {
        !self.blocked()
            && Instant::now() >= self.retry.lock().unwrap_or_else(|e| e.into_inner()).until
    }
    fn fail(&self, error: SyncError, key: Option<&PageKey>) {
        if matches!(
            error,
            SyncError::Network | SyncError::UnknownMutationOutcome
        ) {
            let mut retry = self.retry.lock().unwrap_or_else(|e| e.into_inner());
            let delay = retry.delay();
            retry.until = Instant::now() + delay;
        }
        self.snapshot.send_modify(|snapshot| {
            if let Some(key) = key
                && !matches!(
                    error,
                    SyncError::Authentication
                        | SyncError::Network
                        | SyncError::UnknownMutationOutcome
                        | SyncError::LocalStorage
                )
            {
                snapshot.page_errors.insert(key.clone(), error);
            } else {
                snapshot.error = Some(error);
            }
            snapshot.generation = snapshot.generation.wrapping_add(1);
        });
        self.notify.notify_one();
    }
    fn online(&self) {
        if matches!(
            self.snapshot.borrow().error,
            Some(SyncError::Network | SyncError::UnknownMutationOutcome)
        ) {
            self.snapshot.send_modify(|snapshot| snapshot.error = None);
            let mut retry = self.retry.lock().unwrap_or_else(|e| e.into_inner());
            retry.attempt = 0;
            retry.until = Instant::now();
        }
    }
    async fn publish(&self, store: &StoreHandle) -> Result<(), SyncError> {
        let connection = self.snapshot.borrow().connection.clone();
        let CacheProgress {
            indexed,
            catalogue_total,
            complete,
            pending,
            conflicts,
        } = store
            .cache_progress(&connection)
            .await
            .map_err(|_| SyncError::LocalStorage)?;
        let previous = self.snapshot.borrow();
        let changed = previous.generation == 0
            || previous.pending != pending
            || previous.conflicts != conflicts;
        drop(previous);
        if changed {
            crate::diagnostics::record(crate::diagnostics::DiagnosticEvent::Queue {
                id: uuid::Uuid::new_v4(),
                pending: pending as u64,
                conflicts: conflicts as u64,
            });
        }
        self.snapshot.send_modify(|snapshot| {
            snapshot.indexed = indexed;
            snapshot.catalogue_total = catalogue_total;
            snapshot.complete = complete;
            snapshot.pending = pending;
            snapshot.conflicts = conflicts;
            snapshot.generation = snapshot.generation.wrapping_add(1);
        });
        self.notify.notify_one();
        Ok(())
    }
}
enum Work {
    Read(PageKey),
    Cleanup(PageKey),
    Write(PendingMutation),
    Resolve(PendingMutation),
}
struct Lane {
    reads: mpsc::Sender<Work>,
    writes: mpsc::Sender<Work>,
    load: Arc<AtomicUsize>,
}
fn work_key(work: &Work) -> &PageKey {
    match work {
        Work::Read(key) | Work::Cleanup(key) => key,
        Work::Write(operation) | Work::Resolve(operation) => &operation.key,
    }
}
fn available_lane(lanes: &[Lane], writing: bool) -> Option<usize> {
    lanes
        .iter()
        .enumerate()
        .filter(|(_, lane)| {
            if writing {
                lane.writes.capacity() > 0
            } else {
                lane.reads.capacity() > 0
            }
        })
        .min_by_key(|(_, lane)| lane.load.load(Ordering::Relaxed))
        .map(|(index, _)| index)
}
fn enqueue(lane: &Lane, work: Work, writing: bool, shared: &Shared) -> Result<(), SyncError> {
    let key = work_key(&work).clone();
    lane.load.fetch_add(1, Ordering::Relaxed);
    let result = if writing {
        lane.writes.try_send(work)
    } else {
        lane.reads.try_send(work)
    };
    if result.is_err() {
        lane.load.fetch_sub(1, Ordering::Relaxed);
        shared.release(&key);
        return Err(SyncError::LocalStorage);
    }
    Ok(())
}
struct Admission {
    key: PageKey,
    shared: Arc<Shared>,
    load: Arc<AtomicUsize>,
}
impl Drop for Admission {
    fn drop(&mut self) {
        self.load.fetch_sub(1, Ordering::Relaxed);
        self.shared.release(&self.key);
    }
}
fn storage<T>(result: Result<T, crate::domain::error::StoreError>) -> Result<T, SyncError> {
    result.map_err(|_| SyncError::LocalStorage)
}
async fn observe(
    store: &StoreHandle,
    client: &SilverBulletClient,
    shared: &Shared,
    key: &PageKey,
) -> Result<(), SyncError> {
    let known = storage(store.load_page(key).await)?.and_then(|page| page.remote_base);
    let observation = match known {
        Some(known) => client.read_if_changed(&known).await?,
        None => client.read(key).await?,
    };
    let decision = storage(store.apply_remote(observation).await)?;
    if matches!(decision, ReconcileDecision::Conflict(_)) {
        shared.fail(SyncError::Conflict, Some(key));
    } else {
        shared.snapshot.send_modify(|snapshot| {
            snapshot.page_errors.remove(key);
        });
    }
    shared.online();
    Ok(())
}
async fn resolve(
    store: &StoreHandle,
    client: &SilverBulletClient,
    shared: &Shared,
    operation: &PendingMutation,
) -> Result<(), SyncError> {
    if !storage(store.operation_is_unknown(operation.id).await)? {
        return Ok(());
    }
    let remote = client.read(&operation.key).await?;
    let applied = match (&operation.kind, &remote) {
        (MutationKind::Put(body), RemoteRead::Present(file))
            if body == &file.body && file.revision.is_some() =>
        {
            Some(MutationAck {
                remote_revision: file.revision.clone(),
            })
        }
        (MutationKind::Delete, RemoteRead::Missing(_)) => Some(MutationAck {
            remote_revision: None,
        }),
        _ => None,
    };
    if let Some(ack) = applied {
        storage(store.acknowledge(operation.id, ack).await)?;
        if matches!(remote, RemoteRead::Present(_)) {
            storage(store.apply_remote(remote).await)?;
        }
        shared.online();
        return Ok(());
    }
    // A content validator cannot prove that this is the same incarnation.
    // A lost DELETE acknowledgement followed by identical-content recreation
    // must be reviewed, even if its validator equals the original precondition.
    let unchanged = operation.kind != MutationKind::Delete
        && match (&operation.condition, &remote) {
            (WriteCondition::CreateOnly, RemoteRead::Missing(_)) => true,
            (WriteCondition::Match(expected), RemoteRead::Present(file)) => {
                file.revision.as_ref() == Some(expected)
            }
            _ => false,
        };
    if unchanged {
        if let RemoteRead::Present(file) = &remote
            && file.permission == Permission::ReadOnly
        {
            storage(store.block_page(&operation.key).await)?;
            shared.fail(SyncError::Permission, Some(&operation.key));
            return Ok(());
        }
        storage(store.retry_after_readback(operation.id).await)?;
    } else {
        let local =
            storage(store.load_page(&operation.key).await)?.ok_or(SyncError::LocalStorage)?;
        storage(
            store
                .save_conflict(ConflictRecord {
                    id: uuid::Uuid::new_v4(),
                    key: operation.key.clone(),
                    base: local.remote_base.clone(),
                    local,
                    remote,
                })
                .await,
        )?;
        shared.fail(SyncError::Conflict, Some(&operation.key));
    }
    Ok(())
}
async fn perform(
    work: Work,
    store: &StoreHandle,
    client: &SilverBulletClient,
    shared: &Shared,
) -> Result<(), SyncError> {
    let key = match &work {
        Work::Read(key) | Work::Cleanup(key) => key.clone(),
        Work::Write(operation) | Work::Resolve(operation) => operation.key.clone(),
    };
    let result = match work {
        Work::Read(key) => observe(store, client, shared, &key).await,
        Work::Cleanup(key) => {
            // A historical probe may reappear after an uncertain create. Only
            // its exact generated body is eligible for guarded deletion.
            // Foreign replacements never block unrelated note synchronization.
            match probe::cleanup_probe(client, &key).await {
                Err(
                    error @ (SyncError::Authentication
                    | SyncError::Network
                    | SyncError::UnknownMutationOutcome),
                ) => Err(error),
                _ => Ok(()),
            }
        }
        Work::Resolve(operation) => resolve(store, client, shared, &operation).await,
        Work::Write(operation) => {
            if shared.blocked()
                || shared.page_blocked(&key)
                || !storage(store.operation_is_leased(operation.id).await)?
            {
                storage(store.mark_unknown(operation.id).await)?;
                return Ok(());
            }
            let result = match operation.kind {
                MutationKind::Put(_) => client.put(&operation).await,
                MutationKind::Delete => client.delete(&operation).await,
            };
            match result {
                Ok(ack) => {
                    storage(store.acknowledge(operation.id, ack).await)?;
                    shared.online();
                    Ok(())
                }
                Err(error) => {
                    storage(store.mark_unknown(operation.id).await)?;
                    if matches!(
                        error,
                        SyncError::Conflict
                            | SyncError::Missing
                            | SyncError::UnknownMutationOutcome
                    ) {
                        if error == SyncError::UnknownMutationOutcome {
                            shared.fail(error, None);
                        }
                        resolve(store, client, shared, &operation).await
                    } else {
                        if error == SyncError::Permission {
                            storage(store.block_page(&key).await)?;
                        }
                        Err(error)
                    }
                }
            }
        }
    };
    if let Err(error) = result {
        shared.fail(error, Some(&key));
    }
    shared.publish(store).await?;
    Ok(())
}
async fn run_lane(
    store: StoreHandle,
    client: SilverBulletClient,
    shared: Arc<Shared>,
    mut control: watch::Receiver<Control>,
    mut reads: mpsc::Receiver<Work>,
    mut writes: mpsc::Receiver<Work>,
    load: Arc<AtomicUsize>,
) {
    loop {
        let work = tokio::select! {biased;
            result=control.changed()=>{if result.is_err()||control.borrow().stop{break;}continue;},
            work=writes.recv()=>work,
            work=reads.recv()=>work,
        };
        let Some(work) = work else {
            break;
        };
        let _admission = Admission {
            key: work_key(&work).clone(),
            shared: shared.clone(),
            load: load.clone(),
        };
        // Cancellation retains every claimed payload as unknown during shutdown.
        let result = tokio::select! {biased;
            _=async{loop{if control.changed().await.is_err()||control.borrow().stop{break;}}}=>{break;},
            result=perform(work,&store,&client,&shared)=>result,
        };
        if let Err(error) = result {
            shared.fail(error, None);
        }
        shared.notify.notify_one();
    }
}
async fn supervise(
    store: StoreHandle,
    client: SilverBulletClient,
    shared: Arc<Shared>,
    mut controls: watch::Receiver<Control>,
) -> Result<(), SyncError> {
    let connection = client.connection_id().clone();
    let writable = shared.snapshot.borrow().writable;
    let mut lanes = Vec::new();
    let mut actors = Vec::new();
    for _ in 0..LANES {
        let (reads, rx) = mpsc::channel(1);
        let (writes, wx) = mpsc::channel(1);
        let load = Arc::new(AtomicUsize::new(0));
        actors.push(tokio::spawn(run_lane(
            store.clone(),
            client.clone(),
            shared.clone(),
            controls.clone(),
            rx,
            wx,
            load.clone(),
        )));
        lanes.push(Lane {
            reads,
            writes,
            load,
        });
    }
    let result=async {
        let mut changes=store.subscribe_local_changes();
        let mut reads=DownloadQueue::default();
        let mut cleanup=DownloadQueue::default();
        let started=Instant::now();let mut deadlines=WriteDebounce::default();
        let mut poll=Instant::now();let mut last_refresh=0;
        let mut listing:Option<JoinHandle<Result<Vec<RemoteMetadata>,SyncError>>>=None;
        let mut heads=Vec::<PendingHead>::new();let mut rescan=true;
        shared.publish(&store).await?;
        loop {
            let control=controls.borrow_and_update().clone();
            if control.stop {break;}
            if control.refresh!=last_refresh {
                poll=Instant::now();last_refresh=control.refresh;
                shared.retry.lock().unwrap_or_else(|e|e.into_inner()).until=Instant::now();
                if let Some(key)=&control.priority {
                    reads.prioritize(key.clone());
                }
            }
            if rescan {
                heads=storage(store.pending_heads(&connection).await)?;
                let pending:HashSet<_>=heads.iter().map(|head|head.key.clone()).collect();
                deadlines.retain(&pending);
                for head in &heads {deadlines.observe(&head.key,head.revision,started.elapsed());}
                rescan=false;
            }
            if shared.ready() {
                if Instant::now()>=poll && listing.is_none() {
                    let client=client.clone();listing=Some(tokio::spawn(async move{client.list_files().await}));
                }
                if writable {
                    for operation in storage(store.unknown_batch(&connection,LANES*2).await)? {
                        if !shared.page_blocked(&operation.key)
                            && let Some(index)=available_lane(&lanes,true)
                            && shared.reserve(&operation.key) {
                            enqueue(&lanes[index],Work::Resolve(operation),true,&shared)?;
                        }
                    }
                    for head in &heads {
                        if !shared.page_blocked(&head.key)
                            && deadlines.due(&head.key,started.elapsed())
                            && let Some(index)=available_lane(&lanes,true)
                            && shared.reserve(&head.key) {
                            match storage(store.claim_operation(head.id).await)? {
                                Some(operation)=>enqueue(&lanes[index],Work::Write(operation),true,&shared)?,
                                None=>{shared.release(&head.key);rescan=true;},
                            }
                        }
                    }
                }
                if writable {
                    let count=cleanup.len();
                    for _ in 0..count {
                        let Some(key)=cleanup.pop() else {break;};
                        if let Some(index)=available_lane(&lanes,false) && shared.reserve(&key) {
                            enqueue(&lanes[index],Work::Cleanup(key),false,&shared)?;
                        } else {cleanup.defer(key);}
                    }
                }
                let count=reads.len();
                for _ in 0..count {
                    let Some(key)=reads.pop() else {break;};
                    if let Some(index)=available_lane(&lanes,false) && shared.reserve(&key) {
                        enqueue(&lanes[index],Work::Read(key),false,&shared)?;
                    } else {reads.defer(key);}
                }
            }
            // Wake for real work, deadlines, or polls; do not query SQLite on a
            // repeating idle timer. Completed requests release admission and notify.
            let mut wake=None;
            if !shared.blocked() {
                let retry=shared.retry.lock().unwrap_or_else(|e|e.into_inner()).until;
                if retry>Instant::now() {wake=Some(retry);}
                else {
                    if listing.is_none() {wake=Some(poll);}
                    if writable && available_lane(&lanes,true).is_some() {
                        for head in &heads {
                            if !shared.busy(&head.key)&&!shared.page_blocked(&head.key)
                                && let Some(delay)=deadlines.deadline(&head.key) {
                                let deadline=started+delay;
                                wake=Some(wake.map_or(deadline,|current:Instant|current.min(deadline)));
                            }
                        }
                    }
                }
            }
            if rescan {wake=Some(Instant::now());}
            tokio::select! {
                biased;
                result=controls.changed()=>{if result.is_err(){break;}},
                result=changes.changed()=>{
                    if result.is_err(){return Err(SyncError::LocalStorage);}
                    rescan=true;
                    reads.extend(shared.snapshot.borrow().page_errors.keys().cloned());
                    shared.publish(&store).await?;
                },
                result=async{match listing.as_mut(){Some(task)=>Some(task.await),None=>std::future::pending().await}}=>{
                    listing=None;poll=Instant::now()+Duration::from_secs(if control.active{30}else{60});
                    match result {
                        Some(Ok(Ok(entries)))=>{
                            storage(store.record_catalogue(&connection,entries,true).await)?;
                            if writable {
                                cleanup.extend(storage(store.pending_probes(&connection).await)?);
                            }
                            reads.extend(storage(store.known_remote_keys(&connection).await)?);
                            shared.online();shared.publish(&store).await?;
                        },
                        Some(Ok(Err(error)))=>{
                            shared.fail(error,None);poll=shared.retry.lock().unwrap_or_else(|e|e.into_inner()).until;
                        },
                        _=>return Err(SyncError::Network),
                    }
                },
                _=shared.notify.notified()=>{rescan=true;},
                _=async{match wake{Some(deadline)=>tokio::time::sleep_until(deadline).await,None=>std::future::pending().await}}=>{},
            }
        }
        if let Some(task)=listing {task.abort();}
        Ok(())
    }.await;
    let stopping = controls.borrow().stop || controls.has_changed().is_err();
    for actor in &actors {
        actor.abort();
    }
    for actor in actors {
        let _ = actor.await;
    }
    storage(store.release_sync_leases(&connection).await)?;
    if stopping {
        return Ok(());
    }
    if let Err(error) = result {
        shared.fail(error, None);
    }
    result
}
