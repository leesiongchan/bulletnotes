//! Four bounded network lanes with shared admission serializing each page.
use super::{
    SilverBulletClient,
    probe::{self, CompatibilityEvidence},
};
use crate::{
    domain::{
        connection::ConnectionId,
        error::{StoreError, SyncError},
        page::*,
        reconciliation::{RecoveryUpload, RecoveryUploadHead, RecoveryUploadState},
    },
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
            deferred: Mutex::new(std::collections::HashMap::new()),
            recovery_errors: Mutex::new(HashSet::new()),
            passive: AtomicUsize::new(0),
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
    deferred: Mutex<std::collections::HashMap<PageKey, Retry>>,
    recovery_errors: Mutex<HashSet<PageKey>>,
    passive: AtomicUsize,
}
impl Shared {
    fn deferred(&self, key: &PageKey) -> bool {
        self.deferred
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(key)
            .is_some_and(|retry| retry.until > Instant::now())
    }
    fn defer(&self, key: &PageKey) {
        let mut deferred = self.deferred.lock().unwrap_or_else(|e| e.into_inner());
        let retry = deferred.entry(key.clone()).or_insert_with(|| Retry {
            attempt: 0,
            until: Instant::now(),
            random: uuid::Uuid::new_v4().as_u128() as u64,
        });
        retry.until = Instant::now() + retry.delay();
    }
    fn reserve(&self, key: &PageKey) -> bool {
        if self.deferred(key) {
            return false;
        }

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
        self.deferred(key) || self.snapshot.borrow().page_errors.contains_key(key)
    }
    fn recovery_blocked(&self, key: &PageKey) -> bool {
        self.deferred(key)
            || self
                .snapshot
                .borrow()
                .page_errors
                .get(key)
                .is_some_and(|error| *error != SyncError::Permission)
    }
    fn settle_recovery_error(&self, origin: &PageKey, key: &PageKey) {
        let removed: Vec<_> = {
            let mut errors = self
                .recovery_errors
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            [origin, key]
                .into_iter()
                .filter(|key| errors.remove(*key))
                .cloned()
                .collect()
        };
        self.snapshot.send_modify(|snapshot| {
            for key in removed {
                if snapshot.page_errors.get(&key) == Some(&SyncError::Permission) {
                    snapshot.page_errors.remove(&key);
                }
            }
        });
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
        let blocked = store
            .blocked_recovery_pages(&connection)
            .await
            .map_err(|_| SyncError::LocalStorage)?;
        let recovery_errors = {
            let mut errors = self
                .recovery_errors
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            errors.extend(blocked);
            errors.iter().cloned().collect::<Vec<_>>()
        };
        self.snapshot.send_modify(|snapshot| {
            for key in recovery_errors {
                snapshot.page_errors.insert(key, SyncError::Permission);
            }
        });
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
    Watch(PageKey),
    Cleanup(PageKey),
    Write(PendingMutation),
    Resolve(PendingMutation),
    Recovery(RecoveryUploadHead, Option<RecoveryUpload>),
}
struct Lane {
    reads: mpsc::Sender<Work>,
    writes: mpsc::Sender<Work>,
    load: Arc<AtomicUsize>,
}
fn work_key(work: &Work) -> &PageKey {
    match work {
        Work::Read(key) | Work::Watch(key) | Work::Cleanup(key) => key,
        Work::Write(operation) | Work::Resolve(operation) => &operation.key,
        Work::Recovery(head, _) => &head.key,
    }
}
fn passive_work(work: &Work) -> bool {
    matches!(work, Work::Watch(_)) || matches!(work,Work::Recovery(head,_) if !head.current)
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
    let passive = passive_work(&work);
    if passive {
        shared.passive.fetch_add(1, Ordering::Relaxed);
    }
    lane.load.fetch_add(1, Ordering::Relaxed);
    let result = if writing {
        lane.writes.try_send(work)
    } else {
        lane.reads.try_send(work)
    };
    if result.is_err() {
        if passive {
            shared.passive.fetch_sub(1, Ordering::Relaxed);
        }
        lane.load.fetch_sub(1, Ordering::Relaxed);
        shared.release(&key);
        return Err(SyncError::LocalStorage);
    }
    Ok(())
}
struct Admission {
    passive: bool,
    key: PageKey,
    shared: Arc<Shared>,
    load: Arc<AtomicUsize>,
}
impl Drop for Admission {
    fn drop(&mut self) {
        if self.passive {
            self.shared.passive.fetch_sub(1, Ordering::Relaxed);
        }
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
    let mut observation = observation;
    let mut decision = None;
    for attempt in 0..3 {
        match store.apply_remote(observation).await {
            Ok(result) => {
                decision = Some(result);
                break;
            }
            Err(StoreError::UnknownOperation) => {
                shared
                    .deferred
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .remove(key);
                return Ok(());
            }
            Err(StoreError::StaleRevision) if attempt < 2 => {
                observation = client.read(key).await?;
            }
            Err(StoreError::StaleRevision) => {
                shared.defer(key);
                return Ok(());
            }
            Err(_) => return Err(SyncError::LocalStorage),
        }
    }
    shared
        .deferred
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(key);
    let decision = decision.ok_or(SyncError::LocalStorage)?;
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
            observe(store, client, shared, &operation.key).await?;
        }
        shared.online();
        return Ok(());
    }
    if let (MutationKind::Delete, RemoteRead::Present(file)) = (&operation.kind, &remote) {
        for attempt in 0..3 {
            let file = if attempt == 0 {
                file.clone()
            } else {
                match client.read(&operation.key).await? {
                    RemoteRead::Present(file) => file,
                    RemoteRead::Missing(_) => {
                        storage(
                            store
                                .acknowledge(
                                    operation.id,
                                    MutationAck {
                                        remote_revision: None,
                                    },
                                )
                                .await,
                        )?;
                        return Ok(());
                    }
                }
            };
            match store.retire_unknown_delete(operation.id, file).await {
                Ok(()) => return Ok(()),
                Err(StoreError::StaleRevision) if attempt < 2 => {}
                Err(StoreError::StaleRevision) => {
                    shared.defer(&operation.key);
                    return Ok(());
                }
                Err(_) => return Err(SyncError::LocalStorage),
            }
        }
    }
    let unchanged = match (&operation.condition, &remote) {
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
        } else {
            storage(store.retry_after_readback(operation.id).await)?;
            observe(store, client, shared, &operation.key).await?;
        }
    } else {
        let mut remote = remote;
        for attempt in 0..3 {
            match store.reconcile_unknown_put(operation.id, remote).await {
                Ok(()) => break,
                Err(StoreError::StaleRevision) if attempt < 2 => {
                    remote = client.read(&operation.key).await?;
                }
                Err(StoreError::StaleRevision) => {
                    shared.defer(&operation.key);
                    return Ok(());
                }
                Err(_) => return Err(SyncError::LocalStorage),
            }
        }
    }
    Ok(())
}
async fn recover(
    store: &StoreHandle,
    client: &SilverBulletClient,
    shared: &Shared,
    head: &RecoveryUploadHead,
    claimed: Option<RecoveryUpload>,
) -> Result<bool, SyncError> {
    let upload = match &claimed {
        Some(upload) => upload.clone(),
        None => storage(
            store
                .read_recovery_upload(head.event_id, head.ordinal, head.operation_id)
                .await,
        )?,
    };
    let operation = &upload.operation;
    let read_only = storage(store.load_page(&head.key).await)?
        .is_some_and(|page| page.permission == Permission::ReadOnly);
    let mut denied = false;
    if claimed.is_some() {
        if !storage(
            store
                .recovery_upload_is_leased(head.event_id, head.ordinal, operation.id)
                .await,
        )? {
            return Ok(false);
        }
        if shared.blocked() || shared.recovery_blocked(&head.key) {
            storage(
                store
                    .mark_recovery_unknown(head.event_id, head.ordinal, operation.id)
                    .await,
            )?;
            return Ok(false);
        }
        let result = if read_only {
            Err(SyncError::Conflict)
        } else {
            client.put(operation).await
        };
        match result {
            Ok(ack) => {
                storage(
                    store
                        .acknowledge_recovery_upload(head.event_id, head.ordinal, operation.id, ack)
                        .await,
                )?;
                shared.online();
                return Ok(true);
            }
            Err(error) => {
                storage(
                    store
                        .mark_recovery_unknown(head.event_id, head.ordinal, operation.id)
                        .await,
                )?;
                if error == SyncError::Permission {
                    storage(
                        store
                            .block_recovery_upload(head.event_id, head.ordinal, operation.id)
                            .await,
                    )?;
                    denied = true;
                }
                if !matches!(
                    error,
                    SyncError::Conflict
                        | SyncError::Missing
                        | SyncError::UnknownMutationOutcome
                        | SyncError::Permission
                ) {
                    return Err(error);
                }
            }
        }
    }
    match client.read(&head.key).await? {
        RemoteRead::Present(file) => {
            if operation.kind == MutationKind::Put(file.body.clone()) {
                if file.revision.is_none() {
                    return Err(SyncError::Incompatible);
                }
                storage(
                    store
                        .acknowledge_recovery_upload(
                            head.event_id,
                            head.ordinal,
                            operation.id,
                            MutationAck {
                                remote_revision: file.revision,
                            },
                        )
                        .await,
                )?;
            } else if head.current {
                storage(
                    store
                        .relocate_recovery_upload(head.event_id, head.ordinal, operation.id, file)
                        .await,
                )?;
            } else {
                return Ok(false);
            }
            shared.online();
            return Ok(head.current);
        }
        RemoteRead::Missing(_) => {
            if head.current && (read_only || denied) {
                storage(
                    store
                        .block_recovery_upload(head.event_id, head.ordinal, operation.id)
                        .await,
                )?;
                return Err(SyncError::Permission);
            }
            if head.current {
                storage(
                    store
                        .retry_recovery_upload(head.event_id, head.ordinal, operation.id)
                        .await,
                )?;
            }
        }
    }
    shared.online();
    Ok(false)
}
async fn perform(
    work: Work,
    store: &StoreHandle,
    client: &SilverBulletClient,
    shared: &Shared,
) -> Result<(), SyncError> {
    let key = work_key(&work).clone();
    let recovery_origin = if let Work::Recovery(head, _) = &work {
        Some(
            storage(store.automatic_recovery_event(head.event_id).await)?
                .input
                .local
                .key,
        )
    } else {
        None
    };
    let result = match work {
        Work::Recovery(head, claimed) => match recover(store, client, shared, &head, claimed).await
        {
            Ok(settled) => {
                if settled && let Some(origin) = &recovery_origin {
                    shared.settle_recovery_error(origin, &key);
                }
                Ok(())
            }
            Err(error) => Err(error),
        },
        Work::Read(key) | Work::Watch(key) => observe(store, client, shared, &key).await,
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
                    if operation.kind == MutationKind::Delete && error == SyncError::Conflict {
                        storage(store.record_delete_rejection(operation.id).await)?;
                    } else {
                        storage(store.mark_unknown(operation.id).await)?;
                    }
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
        if let Some(origin) = &recovery_origin {
            if error == SyncError::Permission {
                shared
                    .recovery_errors
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .extend([origin.clone(), key.clone()]);
            }
            shared.fail(error, Some(origin));
        }
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
            passive: passive_work(&work),
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
        let mut watches=DownloadQueue::default();
        let started=Instant::now();let mut deadlines=WriteDebounce::default();
        let mut poll=Instant::now();let mut last_refresh=0;
        let mut listing:Option<JoinHandle<Result<Vec<RemoteMetadata>,SyncError>>>=None;
        let mut heads=Vec::<PendingHead>::new();let mut rescan=true;
        let mut recovery_due=std::collections::HashMap::<OperationId,(RecoveryUploadState,Instant)>::new();
        let mut prefer_watch=false;
        shared.publish(&store).await?;
        loop {
            let control=controls.borrow_and_update().clone();
            if control.stop {break;}
            if control.refresh!=last_refresh {
                poll=Instant::now();last_refresh=control.refresh;
                recovery_due.clear();
                shared.retry.lock().unwrap_or_else(|e|e.into_inner()).until=Instant::now();
                if let Some(key)=&control.priority {
                    reads.prioritize(key.clone());
                }
            }
            if rescan {
                heads=storage(store.pending_heads(&connection).await)?;
                reads.extend(storage(store.pending_reconciliation_keys(&connection).await)?);
                let pending:HashSet<_>=heads.iter().map(|head|head.key.clone()).collect();
                deadlines.retain(&pending);
                for head in &heads {deadlines.observe(&head.key,head.revision,started.elapsed());}
                rescan=false;
            }
            if shared.ready() {
                reads.extend(shared.deferred.lock().unwrap_or_else(|e|e.into_inner()).iter().filter(|(_,retry)|retry.until<=Instant::now()).map(|(key,_)|key.clone()));
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
                    let mut uploads=storage(store.recovery_uploads(&connection,i64::MAX as usize).await)?;
                    uploads.sort_by_key(|head|(!head.current,recovery_due.get(&head.operation_id).map(|(_,due)|*due)));
                    let mut passive=false;
                    for head in uploads {
                        if !head.current && (passive || shared.passive.load(Ordering::Relaxed)>0 || (prefer_watch && watches.len()>0)) {continue;}
                        if let Some((prior,due))=recovery_due.get_mut(&head.operation_id) {
                            if *prior!=RecoveryUploadState::Blocked && head.state==RecoveryUploadState::Blocked {
                                *prior=head.state;*due=Instant::now()+Duration::from_secs(30);
                            }
                            if !(*prior==RecoveryUploadState::Blocked && head.state==RecoveryUploadState::Pending) && *due>Instant::now() {continue;}
                        }
                        let writing=head.current;
                        if !shared.recovery_blocked(&head.key)
                            && let Some(index)=available_lane(&lanes,writing)
                            && shared.reserve(&head.key) {
                            let claimed=if head.state==RecoveryUploadState::Pending {
                                storage(store.claim_recovery_upload_attempt(head.event_id,head.ordinal,head.operation_id).await)?
                            } else {None};
                            if head.state==RecoveryUploadState::Pending && claimed.is_none() {shared.release(&head.key);continue;}
                            recovery_due.insert(head.operation_id,(head.state,Instant::now()+Duration::from_secs(if head.current && head.state!=RecoveryUploadState::Blocked {1}else{30})));
                            passive|=!head.current;
                            if !head.current {prefer_watch=true;}
                            enqueue(&lanes[index],Work::Recovery(head,claimed),writing,&shared)?;
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
            if shared.ready() && shared.passive.load(Ordering::Relaxed)==0
                && let Some(key)=watches.pop() {
                if let Some(index)=available_lane(&lanes,false) && shared.reserve(&key) {
                    prefer_watch=false;
                    enqueue(&lanes[index],Work::Watch(key),false,&shared)?;
                } else {watches.defer(key);}
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
            if !shared.blocked() {
                for retry in shared.deferred.lock().unwrap_or_else(|e|e.into_inner()).values().filter(|retry|retry.until>Instant::now()) {
                    wake=Some(wake.map_or(retry.until,|current:Instant|current.min(retry.until)));
                }
                for (_,deadline) in recovery_due.values().filter(|(_,deadline)|*deadline>Instant::now()) {
                    wake=Some(wake.map_or(*deadline,|current:Instant|current.min(*deadline)));
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
                            let watched:HashSet<_>=storage(store.retired_delete_keys(&connection).await)?.into_iter().collect();
                            reads.extend(storage(store.known_remote_keys(&connection).await)?.into_iter().filter(|key|!watched.contains(key)));
                            watches.extend(watched);
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
