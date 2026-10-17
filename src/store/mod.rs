//! A bounded async handle to one blocking SQLite owner.
mod outbox;
mod pages;
pub mod recovery;
pub mod search;
pub mod settings;
mod worker;
use crate::{
    app::model::EditorPosition,
    domain::{connection::ConnectionId, error::StoreError, page::*},
};
use search::{SearchQuery, SearchResults};
use settings::AppSettings;
use std::path::Path;
use tokio::sync::{mpsc, oneshot};
pub use worker::Durability;
use worker::{Command, Worker};

#[derive(Clone)]
pub struct StoreHandle {
    sender: mpsc::Sender<Command>,
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
        Ok(Self { sender })
    }
    async fn call<R: Send + 'static>(
        &self,
        operation: impl FnOnce(&mut Worker) -> Result<R, StoreError> + Send + 'static,
    ) -> Result<R, StoreError> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.sender
            .send(Command::Run(Box::new(move |worker| {
                let _ = reply_tx.send(operation(worker));
            })))
            .await
            .map_err(|_| StoreError::Closed)?;
        reply_rx.await.map_err(|_| StoreError::Closed)?
    }
    pub async fn load_page(&self, key: &PageKey) -> Result<Option<PageSnapshot>, StoreError> {
        let key = key.clone();
        self.call(move |w| pages::load(&w.db, &key)).await
    }
    pub async fn create_draft(&self, key: PageKey) -> Result<SavedRevision, StoreError> {
        self.call(move |w| pages::create(&mut w.db, key)).await
    }
    pub async fn save_local(
        &self,
        key: PageKey,
        body: String,
        revision: LocalRevision,
    ) -> Result<SavedRevision, StoreError> {
        self.call(move |w| pages::save(&mut w.db, key, body, revision))
            .await
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
    pub async fn load_settings(&self) -> Result<AppSettings, StoreError> {
        self.call(|w| settings::load(&w.db)).await
    }
    pub async fn save_settings(&self, settings: AppSettings) -> Result<(), StoreError> {
        self.call(move |w| settings::save(&mut w.db, settings))
            .await
    }
    pub async fn search(&self, query: SearchQuery) -> Result<SearchResults, StoreError> {
        self.call(move |w| search::search(&w.db, query)).await
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
