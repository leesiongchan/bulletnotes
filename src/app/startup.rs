//! Durable startup state. Opening the app never contacts a remote server.
use crate::{
    domain::{connection::ConnectionId, error::StoreError, page::PageKey},
    store::{StoreHandle, settings::AppSettings},
};
use std::path::Path;

pub struct Startup {
    pub store: StoreHandle,
    pub settings: AppSettings,
    pub connection: ConnectionId,
    pub last_page: Option<PageKey>,
}
pub async fn open(path: &Path, fallback: ConnectionId) -> Result<Startup, StoreError> {
    let store = StoreHandle::open(path).await?;
    let result = async {
        let mut settings = store.load_settings().await?;
        let connection = settings.active_connection.clone().unwrap_or(fallback);
        settings.active_connection = Some(connection.clone());
        let last_page = store.last_cached_page(&connection).await?;
        Ok(Startup {
            store: store.clone(),
            settings,
            connection,
            last_page,
        })
    }
    .await;
    if result.is_err() {
        let _ = store.shutdown().await;
    }
    result
}
