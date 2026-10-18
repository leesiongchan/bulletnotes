//! Local durability, guarded sync presentation, and native UI coordination.
use super::{
    appearance_ui::AppearanceUi,
    coordinator::{AppAction, AppEffect, AppEvent, Coordinator, SearchRequest},
    find::NoteFind,
    model::View,
};
use crate::{
    AppWindow, PageRow,
    domain::{
        connection::ConnectionId,
        error::StoreError,
        page::*,
        status::{LocalSaveState, SyncState, status_text},
    },
    store::{PageSyncFacts, StoreHandle, search::SearchQuery},
    sync::worker::{SyncHandle, SyncSnapshot},
};
use slint::ComponentHandle;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tokio::sync::{mpsc, oneshot, watch};

#[derive(Debug, thiserror::Error)]
pub enum LocalUiError {
    #[error(transparent)]
    EventLoop(#[from] slint::EventLoopError),
    #[error("local save failed; the editing buffer is still available")]
    SaveFailed,
    #[error("local worker is unavailable")]
    Closed,
    #[error("page is unavailable locally")]
    LoadFailed,
    #[error("search could not be completed")]
    SearchFailed,
}
enum Work {
    Effect(AppEffect),
    Barrier(oneshot::Sender<()>),
}
#[derive(Clone)]
struct LoadRequest {
    id: u64,
    key: PageKey,
}
struct State {
    coordinator: RefCell<Coordinator>,
    find: RefCell<NoteFind>,
    window: slint::Weak<AppWindow>,
    writes: mpsc::Sender<Work>,
    loads: watch::Sender<Option<LoadRequest>>,
    searches: watch::Sender<Option<SearchRequest>>,
    updates: watch::Sender<()>,
    search_id: Cell<u64>,
    invalid_folder: Cell<bool>,
    rendered_key: RefCell<Option<PageKey>>,
    rendered_revision: Cell<Option<LocalRevision>>,
    rendered_search: Cell<Option<u64>>,
    timer: slint::Timer,
    started: Instant,
    appearance: AppearanceUi,
    store: StoreHandle,
    quitting: Cell<bool>,
    lifecycle_dispatched: Cell<bool>,
    changing_connection: Cell<bool>,
    sync: RefCell<Option<SyncHandle>>,
    sync_snapshot: RefCell<Option<SyncSnapshot>>,
    sync_facts: RefCell<Option<(PageKey, LocalRevision, PageSyncFacts)>>,
    remote_refreshing: Cell<bool>,
    remote_refresh_needed: Cell<bool>,
    sync_search_timer: slint::Timer,
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
    pub fn set_sync(&self, sync: Option<SyncHandle>) -> Option<SyncHandle> {
        let previous = self.0.sync.replace(sync.clone());
        self.0.sync_facts.borrow_mut().take();
        self.0
            .sync_snapshot
            .replace(sync.as_ref().map(SyncHandle::snapshot));
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
        self.0.sync_snapshot.replace(Some(snapshot));
        self.0.update_selected_status();
        self.0.render();
        self.0.refresh_remote();
        if self.0.coordinator.borrow().view() == View::Recovery
            && let Some(window) = self.0.window.upgrade()
        {
            window.invoke_refresh_recovery();
        }
        if self.0.coordinator.borrow().view() == View::AllPages
            && !self.0.sync_search_timer.running()
        {
            let weak = Rc::downgrade(&self.0);
            self.0.sync_search_timer.start(
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
        if let Some(sync) = self.0.sync.borrow().as_ref() {
            sync.refresh();
        }
    }
    pub fn set_active(&self, active: bool) {
        if let Some(sync) = self.0.sync.borrow().as_ref() {
            sync.set_active(active);
        }
    }
    pub async fn change_connection(&self, connection: ConnectionId) -> Result<(), LocalUiError> {
        if self
            .0
            .window
            .upgrade()
            .is_some_and(|window| window.get_recovery_busy() || window.get_lifecycle_pending())
        {
            return Err(LocalUiError::SaveFailed);
        }
        if connection == self.connection_id() {
            return self
                .0
                .appearance
                .save_connection(connection)
                .await
                .map_err(|_| LocalUiError::SaveFailed);
        }
        self.0.changing_connection.set(true);
        self.0.render();
        let result =
            async {
                self.flush().await?;
                self.0
                    .appearance
                    .save_connection(connection.clone())
                    .await
                    .map_err(|_| LocalUiError::SaveFailed)?;
                let draft_connection = connection.clone();
                if !self.0.coordinator.borrow_mut().change_connection(
                    connection.clone(),
                    move || PageKey {
                        connection: draft_connection.clone(),
                        path: new_note_path(&jiff::Zoned::now(), uuid::Uuid::new_v4()),
                    },
                ) {
                    return Err(LocalUiError::SaveFailed);
                }
                self.0.rendered_key.borrow_mut().take();
                self.0.rendered_revision.set(None);
                self.0.rendered_search.set(None);
                self.0.sync_facts.borrow_mut().take();
                let last = self
                    .0
                    .store
                    .last_cached_page(&connection)
                    .await
                    .map_err(|_| LocalUiError::LoadFailed)?;
                // The connection is already replaced and durable; admit its initial open.
                self.0.changing_connection.set(false);
                self.0.dispatch(match last {
                    Some(key) => AppAction::OpenPage(key),
                    None => AppAction::NewNote,
                });
                Ok(())
            }
            .await;
        self.0.changing_connection.set(false);
        self.0.render();
        result
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
                let (sender, receiver) = oneshot::channel();
                self.0
                    .writes
                    .send(Work::Barrier(sender))
                    .await
                    .map_err(|_| LocalUiError::Closed)?;
                receiver.await.map_err(|_| LocalUiError::Closed)?;
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
    pub async fn search_current(&self) -> Result<(), LocalUiError> {
        let mut updates = self.0.updates.subscribe();
        let id = self.0.issue_search().ok_or(LocalUiError::SearchFailed)?;
        loop {
            if self.0.coordinator.borrow().search_generation() == Some(id) {
                return Ok(());
            }
            if self.0.coordinator.borrow().notice().is_some() || self.0.search_id.get() != id {
                return Err(LocalUiError::SearchFailed);
            }
            updates.changed().await.map_err(|_| LocalUiError::Closed)?;
        }
    }
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
    fn finish_lifecycle(&self) {
        self.lifecycle_dispatched.set(false);
        if let Some(window) = self.window.upgrade() {
            window.set_lifecycle_pending(false);
        }
    }
    fn update_selected_status(self: &Rc<Self>) {
        let selected = self.coordinator.borrow().selected_uncached().cloned();
        if let Some(key) = selected {
            let error =
                self.sync_snapshot.borrow().as_ref().and_then(|snapshot| {
                    snapshot.page_errors.get(&key).copied().or(snapshot.error)
                });
            self.coordinator
                .borrow_mut()
                .receive(AppEvent::SelectedPageStatus { key, error });
        }
    }
    fn dispatch(self: &Rc<Self>, action: AppAction) {
        if self
            .window
            .upgrade()
            .is_some_and(|window| window.get_lifecycle_pending())
            && !matches!(
                &action,
                AppAction::Tick(_)
                    | AppAction::Flush
                    | AppAction::RequestHide
                    | AppAction::RequestQuit
            )
        {
            return;
        }
        if self.quitting.get() {
            return;
        }
        if self.changing_connection.get()
            && !matches!(
                &action,
                AppAction::Flush
                    | AppAction::Tick(_)
                    | AppAction::RequestHide
                    | AppAction::RequestQuit
            )
        {
            return;
        }
        if matches!(&action, AppAction::EditText(_)) {
            self.sync_facts.borrow_mut().take();
        }
        if matches!(&action, AppAction::RequestHide | AppAction::RequestQuit) {
            self.lifecycle_dispatched.set(true);
            if let Some(window) = self.window.upgrade() {
                window.set_lifecycle_pending(true);
            }
        }
        let tick = matches!(&action, AppAction::Tick(_));
        let mut app = self.coordinator.borrow_mut();
        let mut effects = if tick {
            Vec::new()
        } else {
            app.dispatch(AppAction::Tick(self.started.elapsed()))
        };
        effects.extend(app.dispatch(action));
        drop(app);
        self.execute(effects);
        self.render();
        self.arm_save();
    }
    fn receive(self: &Rc<Self>, event: AppEvent) {
        let save_failed = matches!(&event, AppEvent::LocalSaveFailed(..));
        let recovery = match &event {
            AppEvent::RecoveryFinished { connection, result }
                if connection == self.coordinator.borrow().connection() =>
            {
                Some((
                    result.is_ok(),
                    match result {
                        Ok(_) => "Recovery action saved locally.".to_owned(),
                        Err(error) => format!(
                            "{error}. Review the latest version or choose another destination."
                        ),
                    },
                ))
            }
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
            AppEvent::LocalSaved(_) | AppEvent::PageLoaded { .. }
        );
        if refresh {
            self.sync_facts.borrow_mut().take();
        }
        if self.invalid_folder.get() && matches!(&event, AppEvent::SearchReady(..)) {
            return;
        }
        let mut app = self.coordinator.borrow_mut();
        let mut effects = app.dispatch(AppAction::Tick(self.started.elapsed()));
        effects.extend(app.receive(event));
        drop(app);
        if save_failed && self.lifecycle_dispatched.get() {
            self.finish_lifecycle();
        }
        self.update_selected_status();
        self.execute(effects);
        self.render();
        self.arm_save();
        if let Some((success, message)) = recovery
            && let Some(window) = self.window.upgrade()
        {
            self.sync_facts.borrow_mut().take();
            window.invoke_recovery_completed(success, message.into());
            if let Some(sync) = self.sync.borrow().as_ref() {
                sync.refresh();
            }
            self.refresh_remote();
        }
        if let Some(key) = uncached
            && self.coordinator.borrow().selected_uncached() == Some(&key)
            && let Some(sync) = self.sync.borrow().as_ref()
        {
            sync.request_page(key);
        }
        if refresh && self.sync_snapshot.borrow().is_some() {
            self.refresh_remote();
        }
    }
    fn refresh_remote(self: &Rc<Self>) {
        self.remote_refresh_needed.set(true);
        if self.remote_refreshing.replace(true) {
            return;
        }
        let weak = Rc::downgrade(self);
        if slint::spawn_local(async move {
            loop {
                let Some(state) = weak.upgrade() else {
                    break;
                };
                state.remote_refresh_needed.set(false);
                let requested = state
                    .coordinator
                    .borrow()
                    .editor()
                    .map(|editor| (editor.key().clone(), editor.revision()));
                if let Some((key, revision)) = requested {
                    let result = state.store.load_sync_page(&key).await;
                    let current = state.coordinator.borrow().editor().is_some_and(|editor| {
                        editor.key() == &key && editor.revision() == revision
                    });
                    if current
                        && let Ok(Some((page, facts))) = result
                        && page.local_revision == revision
                    {
                        if let Some(id) = facts.conflict_id {
                            state.receive(AppEvent::ConflictDetected {
                                key: key.clone(),
                                local_revision: revision,
                                id,
                            });
                        }
                        state.sync_facts.replace(Some((key, revision, facts)));
                        state.receive(AppEvent::RemoteApplied(page));
                    }
                }
                let selected = state.coordinator.borrow().selected_uncached().cloned();
                if let Some(selected) = selected
                    && !state.quitting.get()
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
                if !state.remote_refresh_needed.get() {
                    state.remote_refreshing.set(false);
                    break;
                }
            }
        })
        .is_err()
        {
            self.remote_refreshing.set(false);
        }
    }
    fn sync_state(&self, editor: &super::editor::EditorSession) -> SyncState {
        let snapshot = self.sync_snapshot.borrow();
        let Some(snapshot) = snapshot.as_ref() else {
            return SyncState::LocalOnly;
        };
        let facts = self.sync_facts.borrow();
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
    fn arm_save(self: &Rc<Self>) {
        self.timer.stop();
        let deadline = self
            .coordinator
            .borrow()
            .editor()
            .and_then(|editor| editor.next_save_at());
        if let Some(deadline) = deadline {
            let weak = Rc::downgrade(self);
            self.timer.start(
                slint::TimerMode::SingleShot,
                deadline
                    .saturating_sub(self.started.elapsed())
                    .max(Duration::from_millis(1)),
                move || {
                    if let Some(state) = weak.upgrade() {
                        state.dispatch(AppAction::Tick(state.started.elapsed()));
                    }
                },
            );
        }
    }
    fn execute(self: &Rc<Self>, effects: Vec<AppEffect>) {
        for effect in effects {
            match effect {
                AppEffect::Search(request) => {
                    self.searches.send_replace(Some(request));
                }
                AppEffect::LoadPage { request_id, key } => {
                    self.loads.send_replace(Some(LoadRequest {
                        id: request_id,
                        key,
                    }));
                }
                AppEffect::QuitReady => self.quit(),
                AppEffect::HideReady => {
                    if let Some(window) = self.window.upgrade() {
                        if let Some(sync) = self.sync.borrow().as_ref() {
                            sync.set_active(false);
                        }
                        window.invoke_window_will_hide();
                        let _ = window.hide();
                    }
                    self.finish_lifecycle();
                }
                effect => {
                    if let Err(error) = self.writes.try_send(Work::Effect(effect))
                        && let Work::Effect(effect) = error.into_inner()
                    {
                        let event = match effect {
                            AppEffect::Recovery(_) => AppEvent::RecoveryFinished {
                                connection: self.coordinator.borrow().connection().clone(),
                                result: Err(StoreError::Closed),
                            },
                            AppEffect::CreateDraft(key) => {
                                AppEvent::LocalSaveFailed(key, LocalRevision(0), StoreError::Closed)
                            }
                            AppEffect::SaveLocal { key, revision, .. } => {
                                AppEvent::LocalSaveFailed(key, revision, StoreError::Closed)
                            }
                            AppEffect::SavePosition { key, .. } | AppEffect::RecordVisit(key) => {
                                AppEvent::MetadataFailed(key)
                            }
                            _ => continue,
                        };
                        self.receive(event);
                    }
                }
            }
        }
    }
    fn issue_search(self: &Rc<Self>) -> Option<u64> {
        self.issue_search_page(false)
    }
    fn issue_search_page(self: &Rc<Self>, more: bool) -> Option<u64> {
        let offset = if more {
            let app = self.coordinator.borrow();
            if app.search_pending() {
                return None;
            }
            let results = app.search_results()?;
            if results.items.len() >= results.total {
                return None;
            }
            results.items.len()
        } else {
            0
        };
        let window = self.window.upgrade()?;
        let folder = window.get_search_folder();
        let folder = if folder.is_empty() {
            None
        } else {
            match PagePath::parse(folder.trim_end_matches('/')) {
                Ok(folder) => Some(folder),
                Err(_) => {
                    self.invalid_folder.set(true);
                    window.set_page_results(Default::default());
                    window.set_index_status("Enter a relative folder path".into());
                    return None;
                }
            }
        };
        let Some(id) = self.search_id.get().checked_add(1) else {
            window.set_index_status("Search limit reached. Restart after saving.".into());
            return None;
        };
        self.invalid_folder.set(false);
        self.search_id.set(id);
        let query = SearchQuery {
            connection: self.coordinator.borrow().connection().clone(),
            text: window.get_search_query().to_string(),
            folder,
            offset,
            limit: 200,
        };
        self.dispatch(AppAction::Search(SearchRequest { id, query }));
        Some(id)
    }
    fn present_find(&self, window: &AppWindow) {
        let find = self.find.borrow();
        window.set_find_status(
            match find.current() {
                Some(current) => format!("{current} of {}", find.total()),
                None => format!("{} matches", find.total()),
            }
            .into(),
        );
    }
    fn find(&self, direction: Option<i32>) {
        let Some(window) = self.window.upgrade() else {
            return;
        };
        let app = self.coordinator.borrow();
        let Some(editor) = app.editor() else {
            return;
        };
        let mut find = self.find.borrow_mut();
        if direction.is_none() {
            find.set_query(editor.body(), window.get_find_query().to_string());
        }
        let selection = find.step(editor.body(), direction.unwrap_or(1));
        drop(find);
        self.present_find(&window);
        if let Some(selection) = selection {
            if let (Ok(anchor), Ok(caret)) =
                (i32::try_from(selection.start), i32::try_from(selection.end))
            {
                window.invoke_select_find_match(anchor, caret);
            } else {
                window.set_find_status("Match too far".into());
            }
        }
    }
    fn render(&self) {
        let presentation = crate::diagnostics::OperationTimer::start(
            crate::diagnostics::Operation::EditorPresentation,
        );
        let Some(window) = self.window.upgrade() else {
            return;
        };
        let app = self.coordinator.borrow();
        window.set_active_view(
            match app.view() {
                View::Note => "note",
                View::AllPages => "pages",
                View::Appearance => "appearance",
                View::Connection => "connection",
                View::Recovery => "recovery",
            }
            .into(),
        );
        window.set_can_go_back(app.can_go_back());
        window.set_can_go_forward(app.can_go_forward());
        if let Some(editor) = app.editor() {
            let changed = self.rendered_key.borrow().as_ref() != Some(editor.key());
            if changed
                || self.rendered_revision.get() != Some(editor.revision())
                || window.get_note_text().as_str() != editor.body()
            {
                if window.get_note_text().as_str() != editor.body() {
                    let timing = crate::diagnostics::OperationTimer::start(
                        crate::diagnostics::Operation::EditorSourceUpdate,
                    );
                    window.set_note_text(editor.body().into());
                    timing.success();
                }
                self.find.borrow_mut().refresh(editor.body());
                self.present_find(&window);
                window.set_current_path(editor.key().path.as_str().into());
                self.rendered_key.replace(Some(editor.key().clone()));
                self.rendered_revision.set(Some(editor.revision()));
                if changed {
                    if app.view() == View::Note {
                        let timing = crate::diagnostics::OperationTimer::start(
                            crate::diagnostics::Operation::EditorFocus,
                        );
                        window.invoke_focus_editor();
                        timing.success();
                    }
                    let position = editor.restore_target();
                    if let (Ok(anchor), Ok(caret)) = (
                        i32::try_from(position.anchor),
                        i32::try_from(position.caret),
                    ) {
                        let timing = crate::diagnostics::OperationTimer::start(
                            crate::diagnostics::Operation::SelectionRestoration,
                        );
                        window.invoke_restore_editor_position(anchor, caret, position.scroll_y);
                        timing.success();
                    }
                }
            }
            window.set_note_read_only(
                self.quitting.get()
                    || self.changing_connection.get()
                    || editor.permission() == Permission::ReadOnly,
            );
            let status = app.notice().unwrap_or_else(|| {
                status_text(
                    editor.save_state(),
                    self.sync_state(editor),
                    editor.matches_remote_base(),
                )
            });
            window.set_status_text(status.into());
        } else {
            window.set_note_read_only(true);
            window.set_status_text("Open or create a local note".into());
        }
        if self.rendered_search.get() != app.search_generation() {
            let rows = app
                .search_results()
                .map(|results| {
                    results
                        .items
                        .iter()
                        .map(|page| {
                            Ok(PageRow {
                                key: serde_json::to_string(&page.key)?.into(),
                                title: page.title.as_str().into(),
                                path: page.key.path.as_str().into(),
                                excerpt: page.excerpt.as_deref().unwrap_or("").into(),
                                availability: if !page.cached {
                                    "Online only"
                                } else if page.permission == Permission::ReadOnly {
                                    "Read only"
                                } else {
                                    "Cached"
                                }
                                .into(),
                            })
                        })
                        .collect::<Result<Vec<_>, serde_json::Error>>()
                })
                .transpose();
            match rows {
                Ok(rows) => window.set_page_results(slint::ModelRc::new(slint::VecModel::from(
                    rows.unwrap_or_default(),
                ))),
                Err(_) => window.set_index_status("Page list could not be displayed".into()),
            }
            self.rendered_search.set(app.search_generation());
        }
        window.set_can_load_more_pages(
            !self.invalid_folder.get()
                && !app.search_pending()
                && app
                    .search_results()
                    .is_some_and(|results| results.items.len() < results.total),
        );
        if self.invalid_folder.get() {
            window.set_index_status("Enter a relative folder path".into());
        } else if let Some(message) = app.notice() {
            window.set_index_status(message.into());
        } else if let Some(results) = app.search_results() {
            let (indexed, total, complete) = app.catalogue_progress().unwrap_or((
                results.indexed,
                results.catalogue_total,
                results.complete,
            ));
            window.set_index_status(
                format!(
                    "{} pages · {} of {} indexed{}",
                    results.total,
                    indexed,
                    total,
                    if self.sync_snapshot.borrow().is_none() {
                        " · not syncing"
                    } else if !complete {
                        " · content search incomplete"
                    } else {
                        ""
                    }
                )
                .into(),
            );
        } else {
            window.set_index_status("Searching local pages…".into());
        }
        self.updates.send_replace(());
        presentation.success();
    }
    fn quit(self: &Rc<Self>) {
        if self.quitting.replace(true) {
            return;
        }
        let state = self.clone();
        if slint::spawn_local(async move {
            let result = async {
                state.appearance.flush().await?;
                let (sender, receiver) = oneshot::channel();
                state
                    .writes
                    .send(Work::Barrier(sender))
                    .await
                    .map_err(|_| StoreError::Closed)?;
                receiver.await.map_err(|_| StoreError::Closed)?;
                let sync = state.sync.borrow_mut().take();
                if let Some(sync) = sync {
                    sync.shutdown().await.map_err(|_| StoreError::Closed)?;
                }
                state.store.shutdown().await
            }
            .await;
            if result.is_ok() {
                let _ = slint::quit_event_loop();
            } else {
                state.quitting.set(false);
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
            self.quitting.set(false);
            self.finish_lifecycle();
        }
    }
}
fn now_ms() -> Result<u64, StoreError> {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| StoreError::InvalidData)?;
    u64::try_from(elapsed.as_millis()).map_err(|_| StoreError::InvalidData)
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
    let (writes, mut work) = mpsc::channel(16);
    let (loads, mut load_requests) = watch::channel(None::<LoadRequest>);
    let (searches, mut search_requests) = watch::channel(None::<SearchRequest>);
    let (updates, _) = watch::channel(());
    let state = Rc::new(State {
        coordinator: RefCell::new(coordinator),
        find: RefCell::new(NoteFind::default()),
        window: window.as_weak(),
        writes,
        loads,
        searches,
        updates,
        search_id: Cell::new(0),
        invalid_folder: Cell::new(false),
        rendered_key: RefCell::new(None),
        rendered_revision: Cell::new(None),
        rendered_search: Cell::new(None),
        timer: slint::Timer::default(),
        started: Instant::now(),
        appearance,
        store: store.clone(),
        quitting: Cell::new(false),
        lifecycle_dispatched: Cell::new(false),
        changing_connection: Cell::new(false),
        sync: RefCell::new(None),
        sync_snapshot: RefCell::new(None),
        sync_facts: RefCell::new(None),
        remote_refreshing: Cell::new(false),
        remote_refresh_needed: Cell::new(false),
        sync_search_timer: slint::Timer::default(),
    });
    {
        let weak = Rc::downgrade(&state);
        let store = store.clone();
        slint::spawn_local(async move {
            while let Some(work) = work.recv().await {
                let event = match work {
                    Work::Barrier(sender) => {
                        let _ = sender.send(());
                        continue;
                    }
                    Work::Effect(AppEffect::Recovery(intent)) => {
                        let Some(state) = weak.upgrade() else {
                            break;
                        };
                        let connection = state.coordinator.borrow().connection().clone();
                        drop(state);
                        Some(AppEvent::RecoveryFinished {
                            connection: connection.clone(),
                            result: super::recovery::execute(&store, &connection, intent).await,
                        })
                    }
                    Work::Effect(AppEffect::CreateDraft(key)) => {
                        Some(match store.create_draft(key.clone()).await {
                            Ok(saved) => AppEvent::LocalSaved(saved),
                            Err(error) => AppEvent::LocalSaveFailed(key, LocalRevision(0), error),
                        })
                    }
                    Work::Effect(AppEffect::SaveLocal {
                        key,
                        body,
                        revision,
                        baseline,
                    }) => Some(
                        match store
                            .save_local_from_base(key.clone(), body, revision, *baseline)
                            .await
                        {
                            Ok(saved) => AppEvent::LocalSaved(saved),
                            Err(error) => AppEvent::LocalSaveFailed(key, revision, error),
                        },
                    ),
                    Work::Effect(AppEffect::SavePosition { key, position }) => {
                        let result = match now_ms() {
                            Ok(now) => store.save_position(&key, position, now).await,
                            Err(error) => Err(error),
                        };
                        result.err().map(|_| AppEvent::MetadataFailed(key))
                    }
                    Work::Effect(AppEffect::RecordVisit(key)) => {
                        let result = match now_ms() {
                            Ok(now) => store.record_visit(&key, now).await,
                            Err(error) => Err(error),
                        };
                        result.err().map(|_| AppEvent::MetadataFailed(key))
                    }
                    _ => None,
                };
                if let Some(state) = weak.upgrade()
                    && let Some(event) = event
                {
                    state.receive(event);
                }
            }
        })?;
    }
    {
        let weak = Rc::downgrade(&state);
        let store = store.clone();
        slint::spawn_local(async move {
            while load_requests.changed().await.is_ok() {
                let request = load_requests.borrow_and_update().clone();
                if let Some(request) = request {
                    let result = store.load_active_page(&request.key).await;
                    if let Some(state) = weak.upgrade() {
                        state.receive(AppEvent::PageLoaded {
                            request_id: request.id,
                            key: request.key,
                            result,
                        });
                    }
                }
            }
        })?;
    }
    {
        let weak = Rc::downgrade(&state);
        slint::spawn_local(async move {
            while search_requests.changed().await.is_ok() {
                let request = search_requests.borrow_and_update().clone();
                if let Some(request) = request {
                    let result = store.search(request.query).await;
                    if let Some(state) = weak.upgrade() {
                        state.receive(AppEvent::SearchReady(request.id, result));
                    }
                }
            }
        })?;
    }
    {
        let state = state.clone();
        window.on_find_opened(move || state.find(None));
    }
    {
        let state = state.clone();
        window.on_find_query_changed(move |_| state.find(None));
    }
    {
        let state = state.clone();
        window.on_find_step(move |direction| state.find(Some(direction)));
    }
    {
        let state = state.clone();
        window.on_note_edited(move |text| state.dispatch(AppAction::EditText(text.to_string())));
    }
    {
        let state = state.clone();
        window.on_new_note(move || state.dispatch(AppAction::NewNote));
    }
    {
        let state = state.clone();
        window.on_browse_pages(move || {
            state.dispatch(AppAction::CancelOpen);
            state.dispatch(AppAction::Navigate(View::AllPages));
            state.issue_search();
        });
    }
    {
        let state = state.clone();
        window.on_load_more_pages(move || {
            state.issue_search_page(true);
        });
    }
    {
        let state = state.clone();
        window.on_show_appearance(move || state.dispatch(AppAction::Navigate(View::Appearance)));
    }
    {
        let state = state.clone();
        window.on_show_connection(move || state.dispatch(AppAction::Navigate(View::Connection)));
    }
    {
        let state = state.clone();
        window.on_return_to_note(move || state.dispatch(AppAction::Navigate(View::Note)));
    }
    {
        let state = state.clone();
        window.on_history_step(move |step| {
            state.dispatch(AppAction::StepHistory(step.signum() as i8))
        });
    }
    {
        let state = state.clone();
        window.on_query_changed(move |_| {
            state.dispatch(AppAction::CancelOpen);
            state.issue_search();
        });
    }
    {
        let state = state.clone();
        window.on_folder_changed(move |_| {
            state.dispatch(AppAction::CancelOpen);
            state.issue_search();
        });
    }
    {
        let state = state.clone();
        window.on_open_page(move |key| {
            if let Ok(key) = serde_json::from_str::<PageKey>(&key) {
                state.dispatch(AppAction::OpenPage(key));
            }
        });
    }
    {
        let weak = window.as_weak();
        window.on_toggle_pin(move || {
            if let Some(window) = weak.upgrade() {
                window.set_is_pinned(!window.get_is_pinned());
            }
        });
    }
    {
        let state = state.clone();
        window.on_save_now(move || state.dispatch(AppAction::Flush));
    }
    {
        let state = state.clone();
        window.on_request_quit(move || state.dispatch(AppAction::RequestQuit));
    }
    {
        let state = state.clone();
        window.on_request_hide(move || state.dispatch(AppAction::RequestHide));
    }
    {
        let weak = window.as_weak();
        window.window().on_close_requested(move || {
            if let Some(window) = weak.upgrade() {
                window.invoke_request_hide();
            }
            slint::CloseRequestResponse::KeepWindowShown
        });
    }
    state.render();
    Ok(LocalUi(state))
}
