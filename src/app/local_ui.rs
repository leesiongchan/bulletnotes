//! Local application controller: dispatch, event delivery and component composition.
mod bindings;
mod effects;
mod lifecycle;
mod metadata;
mod presentation;
mod scratchpads;
mod search;
mod synchronization;

use super::{
    appearance_ui::AppearanceUi,
    coordinator::{AppAction, AppEffect, AppEvent, Coordinator},
    lifecycle::{CloseIntent, Lifecycle},
};
use crate::{
    AppWindow,
    domain::{connection::ConnectionId, page::*},
    store::StoreHandle,
};
use effects::Effects;
use presentation::Presentation;
use search::SearchState;
use slint::ComponentHandle;
use std::{cell::RefCell, rc::Rc, time::Instant};
use synchronization::Synchronization;
use tokio::sync::watch;

#[derive(Debug, thiserror::Error)]
pub enum LocalUiError {
    #[error(transparent)]
    EventLoop(#[from] slint::EventLoopError),
    #[error("local save failed; the editing buffer is still available")]
    SaveFailed,
    #[error("local worker is unavailable")]
    Closed,
    #[error("page position or history could not be saved; retry before closing")]
    MetadataFailed,
    #[error("page is unavailable locally")]
    LoadFailed,
    #[error("search could not be completed")]
    SearchFailed,
    #[error("note stack could not be loaded")]
    StackFailed,
}
struct State {
    coordinator: RefCell<Coordinator>,
    window: slint::Weak<AppWindow>,
    effects: Effects,
    updates: watch::Sender<()>,
    search: SearchState,
    presentation: Presentation,
    started: Instant,
    appearance: AppearanceUi,
    store: StoreHandle,
    lifecycle: Lifecycle,
    sync: Synchronization,
    scratchpads: scratchpads::StackRefresh,
}
#[derive(Clone)]
pub struct LocalUi(Rc<State>);
impl LocalUi {
    pub fn dispatch(&self, action: AppAction) {
        self.0.dispatch(action);
    }
    pub fn connection_id(&self) -> ConnectionId {
        self.0.coordinator.borrow().connection().clone()
    }
    pub fn current_revision(&self) -> Option<LocalRevision> {
        self.0
            .coordinator
            .borrow()
            .editor()
            .map(|editor| editor.revision())
    }
    pub fn current_key(&self) -> Option<PageKey> {
        self.0
            .coordinator
            .borrow()
            .editor()
            .map(|editor| editor.key().clone())
    }
    /// Observe durability without requesting a manual save.
    pub async fn wait_until_loaded(&self, key: &PageKey) -> Result<(), LocalUiError> {
        let mut updates = self.0.updates.subscribe();
        loop {
            {
                let app = self.0.coordinator.borrow();
                if !app.loading() {
                    return if app.editor().is_some_and(|editor| editor.key() == key) {
                        Ok(())
                    } else {
                        Err(LocalUiError::LoadFailed)
                    };
                }
            }
            updates.changed().await.map_err(|_| LocalUiError::Closed)?;
        }
    }
}
impl State {
    fn capture_position(&self, app: &mut Coordinator) -> Vec<AppEffect> {
        use crate::{app::model::View, domain::editor::EditorPosition};
        let Some(window) = self.window.upgrade() else {
            return Vec::new();
        };
        let Some(editor) = app.editor() else {
            return Vec::new();
        };
        // Layout/source replacement must not attribute an old widget's offsets to a new page.
        if app.view() != View::Note
            || self.presentation.key.borrow().as_ref() != Some(editor.key())
            || window.get_note_text().as_str() != editor.body()
        {
            return Vec::new();
        }
        let (Ok(anchor), Ok(caret)) = (
            usize::try_from(window.get_editor_anchor()),
            usize::try_from(window.get_editor_caret()),
        ) else {
            return Vec::new();
        };
        let position = EditorPosition {
            anchor,
            caret,
            scroll_y: window.get_editor_scroll_y(),
        };
        if position.validate(editor.body()).is_err() || editor.observed_position() == Some(position)
        {
            return Vec::new();
        }
        tracing::debug!(target: "bulletnotes::editor", "captured editor position");
        app.dispatch(AppAction::SavePosition(position))
    }
    fn dispatch(self: &Rc<Self>, action: AppAction) {
        let intent = match &action {
            AppAction::RequestHide => Some(CloseIntent::Hide),
            AppAction::RequestQuit => Some(CloseIntent::Quit),
            _ => None,
        };
        if let Some(intent) = intent {
            if !self.lifecycle.commit_close(intent) {
                return;
            }
        } else if self.lifecycle.quitting()
            || (self.lifecycle.blocks_authoring()
                && !matches!(
                    &action,
                    AppAction::Tick(_) | AppAction::Flush | AppAction::RefreshScratchpads
                ))
        {
            return;
        }
        if matches!(&action, AppAction::EditText(_)) {
            self.sync.facts.borrow_mut().take();
        }
        if matches!(&action, AppAction::RequestHide | AppAction::RequestQuit)
            && let Some(window) = self.window.upgrade()
        {
            window.set_lifecycle_pending(self.lifecycle.blocks_authoring());
        }
        let tick = matches!(&action, AppAction::Tick(_));
        let explicit_position = matches!(&action, AppAction::SavePosition(_));
        let edit = matches!(&action, AppAction::EditText(_));
        let mut app = self.coordinator.borrow_mut();
        let mut effects = if explicit_position {
            Vec::new()
        } else {
            self.capture_position(&mut app)
        };
        if !tick {
            effects.extend(app.dispatch(AppAction::Tick(self.started.elapsed())));
        }
        effects.extend(app.dispatch(action));
        if edit {
            effects.extend(self.capture_position(&mut app));
        }
        drop(app);
        self.update_sync_selection();
        self.execute(effects);
        self.render();
        self.arm_save();
    }
    fn receive(self: &Rc<Self>, event: AppEvent) {
        let save_failed = matches!(&event, AppEvent::LocalSaveFailed(..));
        let recovery = match &event {
            AppEvent::RecoveryFinished {
                connection, result, ..
            } if connection == self.coordinator.borrow().connection() => Some((
                result.is_ok(),
                match result {
                    Ok(_) => "Recovery action saved locally.".to_owned(),
                    Err(error) => {
                        format!("{error}. Review the latest version or choose another destination.")
                    }
                },
            )),
            _ => None,
        };
        let uncached = match &event {
            AppEvent::PageLoaded {
                key,
                result: Ok(None),
                ..
            } => Some(key.clone()),
            _ => None,
        };
        let refresh = matches!(
            &event,
            AppEvent::LocalSaved { .. } | AppEvent::PageLoaded { .. }
        );
        if refresh {
            self.sync.facts.borrow_mut().take();
        }
        if self.search.invalid_folder.get() && matches!(&event, AppEvent::SearchReady(..)) {
            return;
        }
        let mut app = self.coordinator.borrow_mut();
        let mut effects = self.capture_position(&mut app);
        effects.extend(app.dispatch(AppAction::Tick(self.started.elapsed())));
        effects.extend(app.receive(event));
        drop(app);
        self.update_sync_selection();
        if save_failed && self.lifecycle.flushing() {
            self.finish_lifecycle();
        }
        self.update_selected_status();
        self.execute(effects);
        self.render();
        self.arm_save();
        if let Some((success, message)) = recovery
            && let Some(window) = self.window.upgrade()
        {
            self.sync.facts.borrow_mut().take();
            window.invoke_recovery_completed(success, message.into());
            if let Some(sync) = self.sync.handle.borrow().as_ref() {
                sync.refresh();
            }
            self.refresh_remote();
        }
        if let Some(key) = uncached
            && self.coordinator.borrow().selected_uncached() == Some(&key)
            && let Some(sync) = self.sync.handle.borrow().as_ref()
        {
            sync.request_page(key);
        }
        if refresh {
            self.refresh_remote();
        }
    }
}

pub fn bind(
    window: &AppWindow,
    store: StoreHandle,
    connection: ConnectionId,
    appearance: AppearanceUi,
) -> Result<LocalUi, LocalUiError> {
    let draft_connection = connection.clone();
    let coordinator = Coordinator::new(connection, move || PageKey {
        connection: draft_connection.clone(),
        path: new_note_path(&jiff::Zoned::now(), uuid::Uuid::new_v4()),
    });
    let (effects, workers) = Effects::channels();
    let (updates, _) = watch::channel(());
    let state = Rc::new(State {
        coordinator: RefCell::new(coordinator),
        window: window.as_weak(),
        effects,
        updates,
        search: SearchState::default(),
        presentation: Presentation::default(),
        started: Instant::now(),
        appearance,
        store: store.clone(),
        lifecycle: Lifecycle::default(),
        sync: Synchronization::default(),
        scratchpads: Default::default(),
    });
    workers.start(&state, store)?;
    bindings::bind(window, &state);
    state.dispatch(AppAction::RefreshScratchpads);
    state.render();
    Ok(LocalUi(state))
}
