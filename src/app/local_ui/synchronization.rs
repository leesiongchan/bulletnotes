//! Revision-checked remote refresh and synchronization presentation.
use super::{LocalUi, State};
use crate::{
    app::{
        coordinator::{AppAction, AppEvent},
        editor::EditorSession,
        model::View,
    },
    domain::{page::*, status::SyncState},
    store::PageSyncFacts,
    sync::worker::{SyncHandle, SyncSnapshot},
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Duration,
};

#[derive(Default)]
pub(super) struct Synchronization {
    pub(super) handle: RefCell<Option<SyncHandle>>,
    pub(super) snapshot: RefCell<Option<SyncSnapshot>>,
    pub(super) facts: RefCell<Option<(PageKey, LocalRevision, PageSyncFacts)>>,
    refreshing: Cell<bool>,
    refresh_needed: Cell<bool>,
    search_timer: slint::Timer,
}

impl LocalUi {
    pub fn set_sync(&self, sync: Option<SyncHandle>) -> Option<SyncHandle> {
        let previous = self.0.sync.handle.replace(sync.clone());
        self.0.sync.facts.borrow_mut().take();
        self.0
            .sync
            .snapshot
            .replace(sync.as_ref().map(SyncHandle::snapshot));
        self.0.update_sync_selection();
        self.0.render();
        previous
    }
    pub fn receive_sync(&self, snapshot: SyncSnapshot) {
        if snapshot.connection != *self.0.coordinator.borrow().connection() {
            return;
        }
        let connection = snapshot.connection.clone();
        self.0.receive(AppEvent::CatalogueProgress {
            connection: connection.clone(),
            indexed: snapshot.indexed,
            total: snapshot.catalogue_total,
            complete: snapshot.complete,
        });
        let state = match snapshot.error {
            Some(crate::domain::error::SyncError::Authentication) => SyncState::Authentication,
            Some(
                crate::domain::error::SyncError::Network
                | crate::domain::error::SyncError::UnknownMutationOutcome,
            ) => SyncState::Offline,
            Some(_) => SyncState::Failed,
            None if !snapshot.writable => SyncState::ReadOnly,
            None => SyncState::Synced,
        };
        self.0
            .receive(AppEvent::SyncStateChanged { connection, state });
        self.0.sync.snapshot.replace(Some(snapshot));
        self.0.update_selected_status();
        self.0.render();
        self.0.refresh_remote();
        if self.0.coordinator.borrow().view() == View::Recovery
            && let Some(window) = self.0.window.upgrade()
        {
            window.invoke_refresh_recovery();
        }
        if self.0.coordinator.borrow().view() == View::AllPages
            && !self.0.sync.search_timer.running()
        {
            let weak = Rc::downgrade(&self.0);
            self.0.sync.search_timer.start(
                slint::TimerMode::SingleShot,
                Duration::from_millis(100),
                move || {
                    if let Some(state) = weak.upgrade()
                        && state.coordinator.borrow().view() == View::AllPages
                    {
                        state.issue_search();
                    }
                },
            );
        }
    }
    pub fn refresh_sync(&self) {
        if let Some(sync) = self.0.sync.handle.borrow().as_ref() {
            sync.refresh();
        }
    }
    pub fn set_active(&self, active: bool) {
        if let Some(sync) = self.0.sync.handle.borrow().as_ref() {
            sync.set_active(active);
        }
    }
}

impl State {
    pub(super) fn update_sync_selection(&self) {
        if let Some(sync) = self.sync.handle.borrow().as_ref() {
            let app = self.coordinator.borrow();
            let selected = if app.view() == View::Note {
                app.editor().map(|editor| editor.key().clone())
            } else {
                None
            };
            sync.set_selected_page(selected);
        }
    }
    pub(super) fn update_selected_status(self: &Rc<Self>) {
        let selected = self.coordinator.borrow().selected_uncached().cloned();
        if let Some(key) = selected {
            let error =
                self.sync.snapshot.borrow().as_ref().and_then(|snapshot| {
                    snapshot.page_errors.get(&key).copied().or(snapshot.error)
                });
            self.coordinator
                .borrow_mut()
                .receive(AppEvent::SelectedPageStatus { key, error });
        }
    }
    pub(super) fn refresh_remote(self: &Rc<Self>) {
        self.sync.refresh_needed.set(true);
        if self.sync.refreshing.replace(true) {
            return;
        }
        let weak = Rc::downgrade(self);
        if slint::spawn_local(async move {
            loop {
                let Some(state) = weak.upgrade() else {
                    break;
                };
                state.sync.refresh_needed.set(false);
                let dirty = state
                    .coordinator
                    .borrow()
                    .editor()
                    .is_some_and(|editor| editor.dirty() && !editor.writing());
                if dirty {
                    state.dispatch(AppAction::Flush);
                }
                let requested = state
                    .coordinator
                    .borrow()
                    .editor()
                    .filter(|editor| !editor.writing() && !editor.dirty())
                    .map(|editor| (editor.rebase_token(), editor.rebase_input()));
                if let Some((expected, (baseline, input))) = requested {
                    let replay = state.store.replay_editor_handoff(&expected.key).await;
                    if replay.is_ok() {
                        let result = state.store.load_sync_page(&expected.key).await;
                        if let Ok(Some((stored, facts))) = result {
                            if let Some(id) = facts.conflict_id {
                                state.receive(AppEvent::ConflictDetected {
                                    key: expected.key.clone(),
                                    local_revision: input.local_revision,
                                    id,
                                });
                            }
                            state.sync.facts.replace(Some((
                                expected.key.clone(),
                                input.local_revision,
                                facts,
                            )));
                            let needs_rebase = stored.source_generation
                                != expected.source_generation
                                || stored.page.body != input.body;
                            if needs_rebase {
                                match state.store.rebase_editor(baseline, input, stored).await {
                                    Ok((stored, edits)) => {
                                        let current =
                                            state.coordinator.borrow().editor().is_some_and(
                                                |editor| {
                                                    editor.rebase_token() == expected
                                                        && !editor.writing()
                                                },
                                            );
                                        state.receive(AppEvent::RebaseReady {
                                            expected,
                                            stored,
                                            edits,
                                        });
                                        if !current {
                                            state.sync.refresh_needed.set(true);
                                        }
                                    }
                                    Err(crate::domain::error::StoreError::StaleRevision) => {
                                        state.sync.refresh_needed.set(true)
                                    }
                                    Err(_) => {}
                                }
                            }
                        }
                    }
                }
                let selected = state.coordinator.borrow().selected_uncached().cloned();
                if let Some(selected) = selected
                    && !state.lifecycle.quitting()
                {
                    let cached = state
                        .store
                        .load_page(&selected)
                        .await
                        .ok()
                        .flatten()
                        .is_some();
                    let still_selected =
                        state.coordinator.borrow().selected_uncached() == Some(&selected);
                    if cached && still_selected {
                        state.dispatch(AppAction::OpenPage(selected));
                    }
                }
                if !state.sync.refresh_needed.get() {
                    state.sync.refreshing.set(false);
                    break;
                }
            }
        })
        .is_err()
        {
            self.sync.refreshing.set(false);
        }
    }
    pub(super) fn sync_state(&self, editor: &EditorSession) -> SyncState {
        let snapshot = self.sync.snapshot.borrow();
        let Some(snapshot) = snapshot.as_ref() else {
            return SyncState::LocalOnly;
        };
        let facts = self.sync.facts.borrow();
        let facts = facts
            .as_ref()
            .filter(|(key, revision, _)| key == editor.key() && *revision == editor.revision())
            .map(|(_, _, facts)| facts);
        if facts.is_some_and(|facts| facts.conflict)
            || snapshot.page_errors.get(editor.key())
                == Some(&crate::domain::error::SyncError::Conflict)
        {
            return SyncState::Conflict;
        }
        let error = snapshot
            .error
            .or_else(|| snapshot.page_errors.get(editor.key()).copied());
        if let Some(error) = error {
            return match error {
                crate::domain::error::SyncError::Authentication => SyncState::Authentication,
                crate::domain::error::SyncError::Permission => SyncState::ReadOnly,
                crate::domain::error::SyncError::Network
                | crate::domain::error::SyncError::UnknownMutationOutcome => SyncState::Offline,
                _ => SyncState::Failed,
            };
        }
        if !snapshot.writable || editor.permission() == Permission::ReadOnly {
            return SyncState::ReadOnly;
        }
        if facts.is_none_or(|facts| facts.pending || facts.deleted) {
            SyncState::Pending
        } else {
            SyncState::Synced
        }
    }
}
