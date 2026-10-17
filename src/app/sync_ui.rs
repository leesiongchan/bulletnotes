//! Native orchestration of verified connections and the background sync worker.
use super::{
    connection::ConnectionOutcome,
    connection_ui::ConnectionUi,
    local_ui::{LocalUi, LocalUiError},
};
use crate::{AppWindow, domain::error::SyncError, store::StoreHandle, sync::worker::SyncHandle};
use slint::ComponentHandle;
use std::{cell::Cell, rc::Rc};
use tokio::runtime::Handle;
#[derive(Debug, thiserror::Error)]
pub enum SyncUiError {
    #[error("the current note could not be saved; it is still available")]
    Local(#[from] LocalUiError),
    #[error(transparent)]
    Sync(#[from] SyncError),
    #[error("connection setup stopped")]
    Closed,
    #[error(transparent)]
    EventLoop(#[from] slint::EventLoopError),
}
struct State {
    window: slint::Weak<AppWindow>,
    local: LocalUi,
    store: StoreHandle,
    runtime: Handle,
    epoch: Cell<u64>,
    serial: tokio::sync::Mutex<()>,
}
#[derive(Clone)]
pub struct SyncUi(Rc<State>);
impl SyncUi {
    pub async fn activate(&self, outcome: ConnectionOutcome) -> Result<(), SyncUiError> {
        let _serial = self.0.serial.lock().await;
        let warning = outcome.credential_warning;
        let cleanup_pending = outcome.cleanup_pending;
        let epoch = self
            .0
            .epoch
            .get()
            .checked_add(1)
            .ok_or(SyncUiError::Closed)?;
        self.0.epoch.set(epoch);
        let old = self.0.local.set_sync(None);
        if let Some(old) = old {
            old.shutdown().await?;
        }
        self.0
            .local
            .change_connection(outcome.client.connection_id().clone())
            .await?;
        let client = outcome.client;
        let evidence = outcome.write_capability.ok();
        let store = self.0.store.clone();
        let sync = self
            .0
            .runtime
            .spawn(async move { SyncHandle::start(store, client, evidence) })
            .await
            .map_err(|_| SyncUiError::Closed)??;
        if let Some(window) = self.0.window.upgrade() {
            sync.set_active(window.window().is_visible());
        }
        let mut snapshots = sync.subscribe();
        self.0.local.set_sync(Some(sync));
        self.0
            .local
            .receive_sync(snapshots.borrow_and_update().clone());
        let weak = Rc::downgrade(&self.0);
        slint::spawn_local(async move {
            while snapshots.changed().await.is_ok() {
                let Some(state) = weak.upgrade() else {
                    break;
                };
                if state.epoch.get() != epoch {
                    break;
                }
                let snapshot = snapshots.borrow_and_update().clone();
                state.local.receive_sync(snapshot.clone());
                if let Some(window) = state.window.upgrade() {
                    let mut message = if let Some(error) = snapshot.error {
                        format!(
                            "Automatic sync needs attention: {error}. Your local notes are available."
                        )
                    } else if !snapshot.writable {
                        "Pages are available for browsing. Write sync is paused until safe updates can be verified.".to_owned()
                    } else if !snapshot.complete {
                        format!(
                            "Automatic sync is on · {} of {} pages cached.",
                            snapshot.indexed, snapshot.catalogue_total
                        )
                    } else {
                        "Automatic sync is on.".to_owned()
                    };
                    if let Some(warning) = warning {
                        message.push_str(&format!(" {warning}."));
                    }
                    if cleanup_pending {
                        message.push_str(" Temporary-page cleanup is pending; reconnect to retry its guarded cleanup.");
                    }
                    window.set_connection_message(message.into());
                }
            }
        })?;
        Ok(())
    }
}
pub fn bind(
    window: &AppWindow,
    local: LocalUi,
    store: StoreHandle,
    runtime: Handle,
    connection: &ConnectionUi,
) -> Result<SyncUi, slint::EventLoopError> {
    window.set_sync_available(true);
    window.set_connection_message(
        "Connect to browse your SilverBullet pages and keep notes automatically in sync.".into(),
    );
    let state = Rc::new(State {
        window: window.as_weak(),
        local,
        store,
        runtime,
        epoch: Cell::new(0),
        serial: tokio::sync::Mutex::new(()),
    });
    let mut outcomes = connection.subscribe();
    let weak = Rc::downgrade(&state);
    slint::spawn_local(async move {
        while outcomes.changed().await.is_ok() {
            let Some(state) = weak.upgrade() else {
                break;
            };
            let outcome = outcomes.borrow_and_update().clone();
            let Some(outcome) = outcome else {
                continue;
            };
            let ui = SyncUi(state.clone());
            if let Err(error) = ui.activate(outcome).await
                && let Some(window) = state.window.upgrade()
            {
                window.set_connection_message(error.to_string().into());
            }
        }
    })?;
    Ok(SyncUi(state))
}
