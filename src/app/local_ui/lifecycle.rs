//! Durable connection replacement, close admission, barriers and shutdown.
use super::{LocalUi, LocalUiError, State};
use crate::{
    app::{coordinator::AppAction, lifecycle::CloseIntent},
    domain::{connection::ConnectionId, error::StoreError, page::*, status::LocalSaveState},
};
use std::rc::Rc;

impl LocalUi {
    pub async fn change_connection(&self, connection: ConnectionId) -> Result<(), LocalUiError> {
        if self
            .0
            .window
            .upgrade()
            .is_some_and(|window| window.get_recovery_busy())
            || !self.0.lifecycle.begin_connection()
        {
            return Err(LocalUiError::SaveFailed);
        }
        self.0.render();
        let local = self.clone();
        let (sender, receiver) = tokio::sync::oneshot::channel();
        // An admitted replacement owns its completion even if its caller goes away.
        if let Err(error) = slint::spawn_local(async move {
            let result = local.finish_connection_change(connection).await;
            let _ = sender.send(result);
        }) {
            self.0.lifecycle.finish_connection();
            self.0.render();
            return Err(error.into());
        }
        receiver.await.map_err(|_| LocalUiError::Closed)?
    }
    async fn finish_connection_change(&self, connection: ConnectionId) -> Result<(), LocalUiError> {
        let mut transition = ConnectionChange {
            state: &self.0,
            active: true,
        };
        let result = self.replace_connection(connection).await;
        transition.release();
        let result = result.map(|action| {
            if let Some(action) = action {
                self.0.dispatch(action);
            }
        });
        self.0.render();
        result
    }
    async fn replace_connection(
        &self,
        connection: ConnectionId,
    ) -> Result<Option<AppAction>, LocalUiError> {
        if connection == self.connection_id() {
            self.0
                .appearance
                .save_connection(connection)
                .await
                .map_err(|_| LocalUiError::SaveFailed)?;
            return Ok(None);
        }
        self.flush().await?;
        let last = self
            .0
            .store
            .last_cached_page(&connection)
            .await
            .map_err(|_| LocalUiError::LoadFailed)?;
        self.0
            .appearance
            .save_connection(connection.clone())
            .await
            .map_err(|_| LocalUiError::SaveFailed)?;
        let draft_connection = connection.clone();
        if !self
            .0
            .coordinator
            .borrow_mut()
            .change_connection(connection.clone(), move || PageKey {
                connection: draft_connection.clone(),
                path: new_note_path(&jiff::Zoned::now(), uuid::Uuid::new_v4()),
            })
        {
            return Err(LocalUiError::SaveFailed);
        }
        self.0.presentation.reset();
        self.0.sync.facts.borrow_mut().take();
        Ok(Some(match last {
            Some(key) => AppAction::OpenPage(key),
            None => AppAction::NewNote,
        }))
    }
    pub(crate) fn prepare_close(&self, intent: CloseIntent) -> bool {
        let admitted = !self
            .0
            .window
            .upgrade()
            .is_some_and(|window| window.get_recovery_busy())
            && self.0.lifecycle.prepare_close(intent);
        if admitted {
            self.0.render();
        }
        admitted
    }
    pub(crate) fn cancel_close(&self) {
        self.0.finish_lifecycle();
        self.0.render();
    }
    pub async fn wait_until_saved(&self) -> Result<(), LocalUiError> {
        let mut updates = self.0.updates.subscribe();
        loop {
            {
                let app = self.0.coordinator.borrow();
                match app.editor().map(|editor| editor.save_state()) {
                    None | Some(LocalSaveState::Clean(_)) => return Ok(()),
                    Some(LocalSaveState::Failed(_)) => return Err(LocalUiError::SaveFailed),
                    _ => {}
                }
            }
            updates.changed().await.map_err(|_| LocalUiError::Closed)?;
        }
    }
    /// Await body acknowledgement and all previously queued local metadata writes.
    pub async fn flush(&self) -> Result<(), LocalUiError> {
        let mut updates = self.0.updates.subscribe();
        self.0.dispatch(AppAction::Flush);
        loop {
            let state = self
                .0
                .coordinator
                .borrow()
                .editor()
                .map(|editor| (editor.dirty(), editor.writing(), editor.save_state()));
            if state.is_some_and(|(_, _, state)| matches!(state, LocalSaveState::Failed(_))) {
                return Err(LocalUiError::SaveFailed);
            }
            if state.is_none_or(|(dirty, writing, _)| !dirty && !writing) {
                self.0
                    .effects
                    .barrier()
                    .await
                    .map_err(|_| LocalUiError::MetadataFailed)?;
                if self
                    .0
                    .coordinator
                    .borrow()
                    .editor()
                    .is_none_or(|editor| !editor.dirty() && !editor.writing())
                {
                    return Ok(());
                }
            }
            updates.changed().await.map_err(|_| LocalUiError::Closed)?;
        }
    }
}

impl State {
    pub(super) fn finish_lifecycle(&self) {
        self.lifecycle.finish_close();
        if let Some(window) = self.window.upgrade() {
            window.set_lifecycle_pending(self.lifecycle.blocks_authoring());
        }
    }
    pub(super) fn quit(self: &Rc<Self>) {
        if !self.lifecycle.start_quit() {
            return;
        }
        let state = self.clone();
        if slint::spawn_local(async move {
            let result = async {
                state.appearance.flush().await?;
                state.effects.barrier().await?;
                let sync = state.sync.handle.borrow_mut().take();
                if let Some(sync) = sync {
                    sync.shutdown().await.map_err(|_| StoreError::Closed)?;
                }
                state.store.shutdown().await
            }
            .await;
            if result.is_ok() {
                let _ = slint::quit_event_loop();
            } else {
                state.finish_lifecycle();
                state.render();
                if let Some(window) = state.window.upgrade() {
                    window.set_status_text(
                        "Quit could not finish saving. Your note is still here.".into(),
                    );
                }
            }
        })
        .is_err()
        {
            self.finish_lifecycle();
        }
    }
}

// A dropped future must release foreground admission just like an explicit error.
struct ConnectionChange<'a> {
    state: &'a State,
    active: bool,
}
impl ConnectionChange<'_> {
    fn release(&mut self) {
        if self.active {
            self.state.lifecycle.finish_connection();
            self.active = false;
        }
    }
}
impl Drop for ConnectionChange<'_> {
    fn drop(&mut self) {
        if self.active {
            self.release();
            self.state.render();
        }
    }
}
