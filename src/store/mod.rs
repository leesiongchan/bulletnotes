//! A bounded async handle to one blocking SQLite owner.
mod delete_watches;
mod draft_admission;
mod editor_rebase;
pub use draft_admission::DraftAdmissionRequest;
mod outbox;
mod pages;
mod probes;
mod reconciliation;
pub mod recovery;
mod recovery_uploads;
mod remote;
mod restoration;
mod sync_queue;
pub use sync_queue::{CacheProgress, PageSyncFacts, PendingHead};
pub mod search;
pub mod settings;
mod worker;
use crate::{
    domain::editor::EditorPosition,
    domain::{connection::ConnectionId, error::StoreError, page::*},
};
use search::{SearchQuery, SearchResults};
use settings::AppSettings;
use std::path::Path;
use tokio::sync::{mpsc, oneshot, watch};
use worker::{Command, Worker};
pub use worker::{Durability, owner_process_id};

#[derive(Clone)]
pub struct StoreHandle {
    sender: mpsc::Sender<Command>,
    local_changes: watch::Sender<()>,
    merge_executor: crate::sync::merge_executor::MergeExecutor,
    draft_admission: draft_admission::SharedAdmission,
}
impl StoreHandle {
    pub async fn open(path: &Path) -> Result<Self, StoreError> {
        let path = path.to_owned();
        let (sender, receiver) = mpsc::channel(64);
        let (ready_tx, ready_rx) = oneshot::channel();
        std::thread::Builder::new()
            .name("bulletnotes-store".into())
            .spawn(move || match Worker::open(&path) {
                Ok(worker) => {
                    if ready_tx.send(Ok(())).is_ok() {
                        worker.run(receiver);
                    }
                }
                Err(error) => {
                    let _ = ready_tx.send(Err(error));
                }
            })?;
        ready_rx.await.map_err(|_| StoreError::Closed)??;
        let (local_changes, _) = watch::channel(());
        Ok(Self {
            sender,
            local_changes,
            merge_executor: Default::default(),
            draft_admission: Default::default(),
        })
    }
    async fn call<R: Send + 'static>(
        &self,
        operation: impl FnOnce(&mut Worker) -> Result<R, StoreError> + Send + 'static,
    ) -> Result<R, StoreError> {
        self.call_timed(crate::diagnostics::Operation::Storage, operation)
            .await
    }
    async fn call_timed<R: Send + 'static>(
        &self,
        category: crate::diagnostics::Operation,
        operation: impl FnOnce(&mut Worker) -> Result<R, StoreError> + Send + 'static,
    ) -> Result<R, StoreError> {
        let timer = crate::diagnostics::OperationTimer::start(category);
        let result = async {
            let (reply_tx, reply_rx) = oneshot::channel();
            self.sender
                .send(Command::Run(Box::new(move |worker| {
                    let _ = reply_tx.send(operation(worker));
                })))
                .await
                .map_err(|_| StoreError::Closed)?;
            reply_rx.await.map_err(|_| StoreError::Closed)?
        }
        .await;
        timer.store(result)
    }
    pub async fn load_page(&self, key: &PageKey) -> Result<Option<PageSnapshot>, StoreError> {
        let key = key.clone();
        self.call(move |w| pages::load(&w.db, &key)).await
    }
    pub async fn load_stored_page(
        &self,
        key: &PageKey,
    ) -> Result<Option<crate::domain::reconciliation::StoredPage>, StoreError> {
        let key = key.clone();
        self.call(move |w| pages::load_stored(&w.db, &key)).await
    }
    pub async fn load_stored_merge_draft(
        &self,
        key: &PageKey,
    ) -> Result<Option<crate::domain::reconciliation::StoredMergeDraft>, StoreError> {
        let key = key.clone();
        self.call(move |w| restoration::stored_merge_draft(&w.db, &key))
            .await
    }
    pub async fn load_active_page(
        &self,
        key: &PageKey,
    ) -> Result<Option<crate::domain::reconciliation::StoredPage>, StoreError> {
        let key = key.clone();
        self.call_timed(crate::diagnostics::Operation::CachedOpen, move |w| {
            pages::load_active(&w.db, &key)
        })
        .await
    }
    pub async fn load_editor_page(
        &self,
        key: &PageKey,
    ) -> Result<Option<crate::domain::reconciliation::EditorRestore>, StoreError> {
        let key = key.clone();
        self.call(move |w| editor_rebase::load_editor(&w.db, &key))
            .await
    }
    pub async fn create_draft(&self, key: PageKey) -> Result<SavedRevision, StoreError> {
        let saved = self.call(move |w| pages::create(&mut w.db, key)).await?;
        self.local_changes.send_replace(());
        Ok(saved)
    }
    pub async fn create_draft_outcome(
        &self,
        key: PageKey,
    ) -> Result<crate::domain::reconciliation::LocalSaveOutcome, StoreError> {
        let outcome = self
            .call(move |w| {
                let attempt = pages::create(&mut w.db, key)?;
                let stored =
                    pages::load_stored(&w.db, &attempt.key)?.ok_or(StoreError::PageMissing)?;
                Ok(crate::domain::reconciliation::LocalSaveOutcome { attempt, stored })
            })
            .await?;
        self.local_changes.send_replace(());
        Ok(outcome)
    }
    pub async fn save_local(
        &self,
        key: PageKey,
        body: String,
        revision: LocalRevision,
    ) -> Result<SavedRevision, StoreError> {
        let saved = self
            .call_timed(crate::diagnostics::Operation::LocalSave, move |w| {
                pages::save(&mut w.db, key, body, revision)
            })
            .await?;
        self.local_changes.send_replace(());
        Ok(saved)
    }
    pub async fn save_position(
        &self,
        key: &PageKey,
        position: EditorPosition,
        visited_ms: u64,
    ) -> Result<(), StoreError> {
        let key = key.clone();
        self.call(move |w| pages::save_position(&mut w.db, &key, position, visited_ms))
            .await
    }
    /// Most recently visited cached page, scoped to the selected space.
    pub async fn last_cached_page(
        &self,
        connection: &ConnectionId,
    ) -> Result<Option<PageKey>, StoreError> {
        let connection = connection.clone();
        self.call(move |w| pages::last_cached(&w.db, &connection))
            .await
    }
    /// Record recency without pretending to have observed the editor's selection.
    pub async fn record_visit(&self, key: &PageKey, visited_ms: u64) -> Result<(), StoreError> {
        let key = key.clone();
        self.call(move |w| pages::record_visit(&w.db, &key, visited_ms))
            .await
    }
    pub async fn load_settings(&self) -> Result<AppSettings, StoreError> {
        self.call(|w| settings::load(&w.db)).await
    }
    pub async fn save_settings(&self, settings: AppSettings) -> Result<(), StoreError> {
        self.call(move |w| settings::save(&mut w.db, settings))
            .await
    }
    pub async fn search(&self, query: SearchQuery) -> Result<SearchResults, StoreError> {
        self.call_timed(crate::diagnostics::Operation::CachedSearch, move |w| {
            search::search(&w.db, query)
        })
        .await
    }
    pub async fn claim_pending(
        &self,
        connection: &ConnectionId,
        limit: usize,
    ) -> Result<Vec<PendingMutation>, StoreError> {
        let connection = connection.clone();
        self.call(move |w| outbox::claim(&mut w.db, &connection, limit))
            .await
    }
    pub async fn unknown_mutations(
        &self,
        connection: &ConnectionId,
    ) -> Result<Vec<PendingMutation>, StoreError> {
        let connection = connection.clone();
        self.call(move |w| outbox::unknown(&w.db, &connection))
            .await
    }
    pub async fn acknowledge(
        &self,
        id: OperationId,
        ack: MutationAck,
    ) -> Result<Option<PageSnapshot>, StoreError> {
        self.call(move |w| outbox::acknowledge(&mut w.db, id, ack))
            .await
    }
    pub async fn load_sync_page(
        &self,
        key: &PageKey,
    ) -> Result<Option<(crate::domain::reconciliation::StoredPage, PageSyncFacts)>, StoreError>
    {
        let key = key.clone();
        self.call(move |w| sync_queue::page_facts(&w.db, &key))
            .await
    }
    pub fn subscribe_local_changes(&self) -> watch::Receiver<()> {
        self.local_changes.subscribe()
    }
    pub async fn pending_heads(
        &self,
        connection: &ConnectionId,
    ) -> Result<Vec<PendingHead>, StoreError> {
        let id = connection.clone();
        self.call(move |w| sync_queue::heads(&w.db, &id)).await
    }
    pub async fn claim_operation(
        &self,
        id: OperationId,
    ) -> Result<Option<PendingMutation>, StoreError> {
        self.call(move |w| sync_queue::claim(&mut w.db, id)).await
    }
    pub async fn operation_is_leased(&self, id: OperationId) -> Result<bool, StoreError> {
        self.call(move |w| sync_queue::leased(&w.db, id)).await
    }
    pub async fn mark_unknown(&self, id: OperationId) -> Result<(), StoreError> {
        self.call(move |w| sync_queue::mark_unknown(&w.db, id))
            .await
    }
    pub async fn operation_is_unknown(&self, id: OperationId) -> Result<bool, StoreError> {
        self.call(move |w| sync_queue::is_unknown(&w.db, id)).await
    }
    pub async fn unknown_batch(
        &self,
        connection: &ConnectionId,
        limit: usize,
    ) -> Result<Vec<PendingMutation>, StoreError> {
        let id = connection.clone();
        self.call(move |w| sync_queue::unknown_batch(&w.db, &id, limit))
            .await
    }
    pub async fn retry_after_readback(&self, id: OperationId) -> Result<(), StoreError> {
        self.call(move |w| sync_queue::retry(&mut w.db, id)).await
    }
    pub async fn block_page(&self, key: &PageKey) -> Result<(), StoreError> {
        let key = key.clone();
        self.call(move |w| sync_queue::block(&w.db, &key)).await
    }
    pub async fn release_sync_leases(&self, connection: &ConnectionId) -> Result<(), StoreError> {
        let id = connection.clone();
        self.call(move |w| sync_queue::release_connection(&w.db, &id))
            .await
    }
    pub async fn pending_reconciliation_keys(
        &self,
        connection: &ConnectionId,
    ) -> Result<Vec<PageKey>, StoreError> {
        let connection = connection.clone();
        self.call(move |w| sync_queue::reconciliation_keys(&w.db, &connection))
            .await
    }
    pub async fn known_remote_keys(
        &self,
        connection: &ConnectionId,
    ) -> Result<Vec<PageKey>, StoreError> {
        let id = connection.clone();
        self.call(move |w| sync_queue::keys(&w.db, &id)).await
    }
    pub async fn cache_progress(
        &self,
        connection: &ConnectionId,
    ) -> Result<CacheProgress, StoreError> {
        let id = connection.clone();
        self.call(move |w| sync_queue::progress(&w.db, &id)).await
    }
    pub async fn merge_draft(&self, key: &PageKey) -> Result<Option<String>, StoreError> {
        let key = key.clone();
        self.call(move |w| restoration::merge_draft(&w.db, &key))
            .await
    }
    pub async fn save_merge_drafts(
        &self,
        drafts: Vec<(PageKey, String)>,
    ) -> Result<(), StoreError> {
        self.call(move |w| restoration::save_merge_drafts(&mut w.db, drafts))
            .await
    }
    pub async fn recovery_index(
        &self,
        connection: &ConnectionId,
    ) -> Result<Vec<RecoverySummary>, StoreError> {
        let id = connection.clone();
        self.call(move |w| restoration::index(&w.db, &id)).await
    }
    pub async fn recovery_item(
        &self,
        connection: &ConnectionId,
        kind: RecoveryKind,
        id: uuid::Uuid,
    ) -> Result<RecoveryItem, StoreError> {
        let connection = connection.clone();
        self.call(move |w| restoration::item(&w.db, &connection, kind, id))
            .await
    }
    pub async fn resolve_conflict(
        &self,
        request: ConflictResolution,
    ) -> Result<Option<SavedRevision>, StoreError> {
        let result = self
            .call(move |w| restoration::resolve(&mut w.db, request))
            .await?;
        self.local_changes.send_replace(());
        Ok(result)
    }
    pub async fn move_to_trash(
        &self,
        key: PageKey,
        revision: LocalRevision,
    ) -> Result<TrashId, StoreError> {
        let id = self
            .call(move |w| restoration::move_to_trash(&mut w.db, key, revision))
            .await?;
        self.local_changes.send_replace(());
        Ok(id)
    }
    pub async fn trash(&self, connection: &ConnectionId) -> Result<Vec<TrashRecord>, StoreError> {
        let id = connection.clone();
        self.call(move |w| restoration::trash(&w.db, &id)).await
    }
    pub async fn recovery_copies(
        &self,
        connection: &ConnectionId,
    ) -> Result<Vec<RecoveryCopy>, StoreError> {
        let id = connection.clone();
        self.call(move |w| restoration::copies(&w.db, &id)).await
    }
    pub async fn restore_trash(
        &self,
        id: TrashId,
        destination: Option<PagePath>,
    ) -> Result<PageKey, StoreError> {
        let key = self
            .call(move |w| restoration::restore_trash(&mut w.db, id, destination))
            .await?;
        self.local_changes.send_replace(());
        Ok(key)
    }
    pub async fn restore_recovery(
        &self,
        id: uuid::Uuid,
        destination: Option<PagePath>,
    ) -> Result<PageKey, StoreError> {
        let key = self
            .call(move |w| restoration::restore_copy(&mut w.db, id, destination))
            .await?;
        self.local_changes.send_replace(());
        Ok(key)
    }
    pub async fn empty_trash(&self, ids: Vec<TrashId>) -> Result<(), StoreError> {
        self.call(move |w| restoration::empty_trash(&mut w.db, ids))
            .await
    }
    pub async fn save_conflict(&self, record: ConflictRecord) -> Result<(), StoreError> {
        self.call(move |w| remote::save_conflict(&mut w.db, record))
            .await
    }
    pub async fn apply_remote(
        &self,
        observation: RemoteRead,
    ) -> Result<ReconcileDecision, StoreError> {
        let key = match &observation {
            RemoteRead::Present(file) => &file.key,
            RemoteRead::Missing(key) => key,
        };
        match self.replay_editor_handoff(key).await {
            Ok(_) | Err(StoreError::PageMissing) => {}
            Err(error) => return Err(error),
        }
        let prepared = self
            .prepare_observation(key, observation.clone(), None)
            .await;
        let prepared = match prepared {
            Ok(prepared) => prepared,
            Err(StoreError::PageMissing) => {
                return self
                    .call(move |w| remote::apply_uncached(&mut w.db, observation))
                    .await;
            }
            Err(error) => return Err(error),
        };
        let disposition = prepared.0.disposition;
        let stored = self.commit_observation(prepared).await?;
        Ok(match disposition {
            crate::domain::reconciliation::ResolutionDisposition::RetainDeleted => {
                ReconcileDecision::RetainDeletedSnapshot
            }
            crate::domain::reconciliation::ResolutionDisposition::KeepDeletion => {
                ReconcileDecision::KeepPending
            }
            crate::domain::reconciliation::ResolutionDisposition::Install => match observation {
                RemoteRead::Present(file) if file.body == stored.page.body => {
                    ReconcileDecision::InstallRemote(file)
                }
                _ => ReconcileDecision::KeepPending,
            },
        })
    }
    pub async fn record_catalogue(
        &self,
        connection: &ConnectionId,
        entries: Vec<RemoteMetadata>,
        complete: bool,
    ) -> Result<(), StoreError> {
        let connection = connection.clone();
        self.call(move |w| remote::catalogue(&mut w.db, &connection, entries, complete))
            .await
    }
    pub async fn conflicts(
        &self,
        connection: &ConnectionId,
    ) -> Result<Vec<ConflictRecord>, StoreError> {
        let connection = connection.clone();
        self.call(move |w| remote::conflicts(&w.db, &connection))
            .await
    }
    pub async fn save_local_from_base(
        &self,
        key: PageKey,
        body: String,
        revision: LocalRevision,
        expected: EditorBaseline,
    ) -> Result<SavedRevision, StoreError> {
        Ok(self
            .save_local_from_base_outcome(key, body, revision, expected)
            .await?
            .attempt)
    }
    /// Acknowledges the authored attempt immediately after its durable transaction.
    pub async fn save_local_from_base_outcome(
        &self,
        key: PageKey,
        body: String,
        revision: LocalRevision,
        expected: EditorBaseline,
    ) -> Result<crate::domain::reconciliation::LocalSaveOutcome, StoreError> {
        let saved = self
            .call_timed(crate::diagnostics::Operation::LocalSave, move |w| {
                remote::save_from_base(&mut w.db, key, body, revision, expected)
            })
            .await?;
        self.local_changes.send_replace(());
        Ok(saved)
    }
    pub async fn register_probe(&self, key: &PageKey) -> Result<(), StoreError> {
        let key = key.clone();
        self.call(move |w| probes::register(&w.db, &key)).await
    }
    pub async fn pending_probes(&self, id: &ConnectionId) -> Result<Vec<PageKey>, StoreError> {
        let id = id.clone();
        self.call(move |w| probes::pending(&w.db, &id)).await
    }
    pub async fn forget_probe(&self, key: &PageKey) -> Result<(), StoreError> {
        let key = key.clone();
        self.call(move |w| probes::forget(&w.db, &key)).await
    }
    pub async fn durability(&self) -> Result<Durability, StoreError> {
        self.call(|w| Ok(w.durability.clone())).await
    }
    /// Drain earlier commands, close the database, and release single-writer ownership.
    pub async fn shutdown(&self) -> Result<(), StoreError> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.sender
            .send(Command::Shutdown(reply_tx))
            .await
            .map_err(|_| StoreError::Closed)?;
        reply_rx.await.map_err(|_| StoreError::Closed)
    }
}
