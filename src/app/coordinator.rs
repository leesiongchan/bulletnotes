//! Revision-tagged application transitions; dispatch never performs blocking I/O.
use super::{
    editor::EditorSession,
    model::{EditorPosition, View},
    navigation::NavigationState,
    recovery::{RecoveryIntent, RecoveryOutcome},
};
use crate::{
    domain::{
        connection::ConnectionId,
        error::StoreError,
        page::*,
        reconciliation::{EditorRestore, EditorToken, LocalSaveOutcome, StoredPage},
    },
    store::search::{SearchQuery, SearchResults},
};
use std::time::Duration;

const METADATA_FAILURE: &str = "Page history could not be saved. Note text is still available.";

#[derive(Clone)]
pub struct SearchRequest {
    pub id: u64,
    pub query: SearchQuery,
}
pub enum AppAction {
    ResolveConflict(ConflictResolution),
    MoveToTrash(PageKey, LocalRevision),
    PrepareSearchTrash(Vec<PageKey>),
    ConfirmSearchTrash(u64),
    CancelSearchTrash,
    MoveSearchScratchpads {
        expected: Vec<PageKey>,
        selected: Vec<PageKey>,
        before: Option<PageKey>,
        open_after: Option<PageKey>,
    },
    NewNoteFromSearch(String),
    RestoreTrash(TrashId, Option<PagePath>),
    RestoreRecovery(uuid::Uuid, Option<PagePath>),
    EmptyTrash(Vec<TrashId>),
    EditText(String),
    CancelOpen,
    Navigate(View),
    OpenPage(PageKey),
    NewNote,
    StepHistory(i8),
    RefreshScratchpads,
    StepScratchpad(i8),
    JumpToNewest,
    Search(SearchRequest),
    SavePosition(EditorPosition),
    Flush,
    RequestQuit,
    RequestHide,
    Tick(Duration),
}
pub enum AppEvent {
    ScratchpadsLoaded {
        request_id: u64,
        connection: ConnectionId,
        result: Result<Vec<crate::store::scratchpads::ScratchpadEntry>, StoreError>,
    },
    TransitionChecked {
        request_id: u64,
        connection: ConnectionId,
        result: Result<(), StoreError>,
    },
    SelectedPageStatus {
        key: PageKey,
        error: Option<crate::domain::error::SyncError>,
    },
    RecoveryFinished {
        request_id: u64,
        connection: ConnectionId,
        result: Result<RecoveryOutcome, StoreError>,
    },
    RemoteApplied(PageSnapshot),
    CatalogueProgress {
        connection: ConnectionId,
        indexed: usize,
        total: usize,
        complete: bool,
    },
    SyncStateChanged {
        connection: ConnectionId,
        state: crate::domain::status::SyncState,
    },
    ConflictDetected {
        key: PageKey,
        local_revision: LocalRevision,
        id: uuid::Uuid,
    },
    MetadataFailed(PageKey),
    MetadataDurable,
    LocalSaved {
        expected: EditorToken,
        outcome: LocalSaveOutcome,
    },
    LocalSaveFailed(EditorToken, LocalRevision, StoreError),
    RebaseReady {
        expected: EditorToken,
        stored: StoredPage,
        edits: Vec<crate::sync::text_merge::SourceEdit>,
    },
    SearchReady(u64, Result<SearchResults, StoreError>),
    PageLoaded {
        request_id: u64,
        key: PageKey,
        result: Result<Option<EditorRestore>, StoreError>,
    },
}
pub enum AppEffect {
    CloseCancelled,
    RecoveryCancelled {
        connection: ConnectionId,
    },
    LoadScratchpads {
        request_id: u64,
        connection: ConnectionId,
    },
    CheckTransition {
        request_id: u64,
        connection: ConnectionId,
    },
    Recovery {
        request_id: u64,
        connection: ConnectionId,
        intent: RecoveryIntent,
    },
    CreateDraft {
        key: PageKey,
        expected: EditorToken,
    },
    SaveLocal {
        expected: EditorToken,
        key: PageKey,
        body: String,
        revision: LocalRevision,
        baseline: Box<EditorBaseline>,
    },
    LoadPage {
        request_id: u64,
        key: PageKey,
    },
    Search(SearchRequest),
    SavePosition {
        key: PageKey,
        position: EditorPosition,
    },
    RecordVisit(PageKey),
    QuitReady,
    HideReady,
}
enum Transition {
    Recovery(RecoveryIntent),
    Open(PageKey, Option<(NavigationState, i8)>),
    New(Option<String>),
    Quit,
    Hide,
}
struct PendingLoad {
    id: u64,
    key: PageKey,
    history: Option<(NavigationState, i8)>,
}
pub struct Coordinator {
    pub search_management: super::search_management::SearchManagement,
    background_view: Option<View>,
    stack: Option<super::scratchpads::ScratchpadStack>,
    stack_request: u64,
    stack_loading: bool,
    stack_again: bool,
    stack_intent: Option<i8>,
    stack_cursor: Option<PageKey>,
    stack_step: i8,
    transition_id: u64,
    barrier_waiting: bool,
    barrier_ready: bool,
    recovery_successors: Option<(u64, Vec<PageKey>)>,
    after_delete: Option<Vec<PageKey>>,
    connection: ConnectionId,
    editor: Option<EditorSession>,
    navigation: NavigationState,
    pending_transition: Option<Transition>,
    pending_load: Option<PendingLoad>,
    next_load: u64,
    now: Duration,
    latest_search: Option<u64>,
    search_offset: usize,
    search_pending: bool,
    completed_search: Option<u64>,
    flush_target: Option<LocalRevision>,
    results: Option<SearchResults>,
    notice: Option<&'static str>,
    make_draft: Box<dyn FnMut() -> PageKey>,
    selected_uncached: Option<PageKey>,
    selected_page_error: Option<(PageKey, crate::domain::error::SyncError)>,
    catalogue_progress: Option<(usize, usize, bool)>,
    remote_state: crate::domain::status::SyncState,
    active_conflict: Option<(PageKey, LocalRevision, uuid::Uuid)>,
    search_folder: Option<PagePath>,
    search_scope: crate::store::search::SearchScope,
}
impl Coordinator {
    pub fn new(connection: ConnectionId, make_draft: impl FnMut() -> PageKey + 'static) -> Self {
        Self {
            search_management: Default::default(),
            background_view: None,
            stack: None,
            stack_request: 0,
            stack_loading: false,
            stack_again: false,
            stack_intent: None,
            stack_cursor: None,
            stack_step: 0,
            transition_id: 0,
            barrier_waiting: false,
            barrier_ready: false,
            recovery_successors: None,
            after_delete: None,
            connection,
            editor: None,
            navigation: NavigationState::default(),
            pending_transition: None,
            pending_load: None,
            next_load: 0,
            now: Duration::ZERO,
            latest_search: None,
            search_offset: 0,
            search_pending: false,
            completed_search: None,
            flush_target: None,
            results: None,
            notice: None,
            make_draft: Box::new(make_draft),
            selected_uncached: None,
            selected_page_error: None,
            catalogue_progress: None,
            remote_state: Default::default(),
            active_conflict: None,
            search_folder: None,
            search_scope: Default::default(),
        }
    }
    pub fn editor(&self) -> Option<&EditorSession> {
        self.editor.as_ref()
    }
    pub fn view(&self) -> View {
        self.navigation.view()
    }
    pub fn search_pending(&self) -> bool {
        self.search_pending
    }
    pub fn search_generation(&self) -> Option<u64> {
        self.completed_search
    }
    pub fn search_results(&self) -> Option<&SearchResults> {
        self.results.as_ref()
    }
    pub fn notice(&self) -> Option<&'static str> {
        self.notice
    }
    pub fn loading(&self) -> bool {
        self.pending_load.is_some() || self.pending_transition.is_some()
    }
    pub fn selected_uncached(&self) -> Option<&PageKey> {
        self.selected_uncached.as_ref()
    }
    pub fn catalogue_progress(&self) -> Option<(usize, usize, bool)> {
        self.catalogue_progress
    }
    pub fn active_conflict(&self) -> Option<uuid::Uuid> {
        self.active_conflict
            .as_ref()
            .filter(|(key, revision, _)| {
                self.editor
                    .as_ref()
                    .is_some_and(|editor| editor.key() == key && editor.revision() >= *revision)
            })
            .map(|(_, _, id)| *id)
    }
    fn uncached_notice(&mut self) {
        if let Some(selected) = &self.selected_uncached {
            if let Some((_, error)) = self
                .selected_page_error
                .as_ref()
                .filter(|(key, _)| key == selected)
            {
                use crate::domain::error::SyncError;
                self.notice = Some(match error {
                    SyncError::Permission => {
                        "Access to this page was denied. Your current note is still here."
                    }
                    SyncError::ResponseTooLarge => {
                        "This page exceeds client resource limits. Your current note is still here."
                    }
                    SyncError::Authentication => {
                        "Sign in to download this page. Your current note is still here."
                    }
                    SyncError::Network | SyncError::UnknownMutationOutcome => {
                        "Page is not cached. Connect to download it; your current note is still here."
                    }
                    SyncError::Missing => {
                        "This page is no longer available remotely. Your current note is still here."
                    }
                    SyncError::InvalidResponse | SyncError::Incompatible => {
                        "This page returned invalid or unsupported data. Your current note is still here."
                    }
                    _ => "This page could not be downloaded. Your current note is still here.",
                });
                return;
            }
            self.notice = Some(
                if matches!(
                    self.remote_state,
                    crate::domain::status::SyncState::LocalOnly
                        | crate::domain::status::SyncState::Offline
                        | crate::domain::status::SyncState::Authentication
                ) {
                    "Page is not cached. Connect to download it; your current note is still here."
                } else {
                    "Downloading this page. Your current note is still here."
                },
            );
        }
    }
    pub fn change_connection(
        &mut self,
        connection: ConnectionId,
        make_draft: impl FnMut() -> PageKey + 'static,
    ) -> bool {
        if self
            .editor
            .as_ref()
            .is_some_and(|editor| editor.dirty() || editor.writing())
        {
            return false;
        }
        self.search_management = Default::default();
        self.background_view = None;
        self.stack = None;
        self.stack_loading = false;
        self.stack_again = false;
        self.stack_intent = None;
        self.stack_cursor = None;
        self.barrier_waiting = false;
        self.barrier_ready = false;
        self.recovery_successors = None;
        self.after_delete = None;
        self.connection = connection;
        self.editor = None;
        self.navigation = NavigationState::default();
        self.pending_load = None;
        self.pending_transition = None;
        self.latest_search = None;
        self.completed_search = None;
        self.flush_target = None;
        self.results = None;
        self.notice = None;
        self.make_draft = Box::new(make_draft);
        self.selected_uncached = None;
        self.catalogue_progress = None;
        self.remote_state = Default::default();
        self.active_conflict = None;
        self.search_folder = None;
        true
    }
    pub fn connection(&self) -> &ConnectionId {
        &self.connection
    }
    pub fn can_go_back(&self) -> bool {
        self.navigation.can_go_back()
    }
    pub fn can_go_forward(&self) -> bool {
        self.navigation.can_go_forward()
    }
    pub fn scratchpads_pending(&self) -> bool {
        self.stack_loading
    }
    pub fn scratchpads(&self) -> Option<&super::scratchpads::ScratchpadStack> {
        self.stack.as_ref()
    }
    pub fn can_step_scratchpad(&self, step: i8) -> bool {
        let Some(stack) = &self.stack else {
            return false;
        };
        let Some(key) = self
            .stack_cursor
            .as_ref()
            .or_else(|| self.editor.as_ref().map(|e| e.key()))
        else {
            return false;
        };
        if step < 0 {
            stack.older(key).is_some()
        } else if step > 0 {
            stack.newer(key).is_some()
        } else {
            false
        }
    }
    fn refresh_scratchpads(&mut self, effects: &mut Vec<AppEffect>) {
        if self.stack_loading {
            self.stack_again = true;
            return;
        }
        let Some(id) = self.stack_request.checked_add(1) else {
            self.notice = Some("Note stack could not be refreshed. Restart after saving.");
            return;
        };
        self.stack_request = id;
        self.stack_loading = true;
        effects.push(AppEffect::LoadScratchpads {
            request_id: id,
            connection: self.connection.clone(),
        });
    }
    fn step_scratchpad(&mut self, step: i8, effects: &mut Vec<AppEffect>) {
        let timing = crate::diagnostics::OperationTimer::start(
            crate::diagnostics::Operation::ScratchpadNavigation,
        );
        timing.success();
        let Some(stack) = &self.stack else {
            self.stack_intent = Some(step);
            self.refresh_scratchpads(effects);
            return;
        };
        let current = self
            .stack_cursor
            .as_ref()
            .or_else(|| self.editor.as_ref().map(|e| e.key()));
        let destination = if step == 0 {
            stack.newest()
        } else {
            current.and_then(|key| {
                if step < 0 {
                    stack.older(key)
                } else {
                    stack.newer(key)
                }
            })
        };
        if let Some(key) = destination {
            self.stack_step = step;
            self.selected_uncached = None;
            self.selected_page_error = None;
            self.transition(Transition::Open(key.clone(), None), effects);
            self.stack_cursor = Some(key);
        } else if step == 0 && stack.is_empty() {
            self.transition(Transition::New(None), effects);
        }
    }
    pub fn dispatch(&mut self, action: AppAction) -> Vec<AppEffect> {
        let mut effects = Vec::new();
        match action {
            AppAction::PrepareSearchTrash(keys) => {
                if !self.search_management.busy()
                    && self.view() == View::AllPages
                    && !keys.is_empty()
                    && keys.iter().all(|key| key.connection == self.connection)
                {
                    self.transition(
                        Transition::Recovery(RecoveryIntent::PrepareTrash(keys)),
                        &mut effects,
                    );
                    self.search_management.operation = Some(self.transition_id);
                }
            }
            AppAction::ConfirmSearchTrash(id) => {
                if !self.search_management.busy()
                    && self.view() == View::AllPages
                    && self
                        .search_management
                        .prepared
                        .as_ref()
                        .is_some_and(|(pin, _)| *pin == id)
                {
                    let (_, pages) = self
                        .search_management
                        .prepared
                        .take()
                        .expect("checked confirmation");
                    self.transition(
                        Transition::Recovery(RecoveryIntent::TrashMany(pages)),
                        &mut effects,
                    );
                    self.search_management.operation = Some(self.transition_id);
                }
            }
            AppAction::CancelSearchTrash => {
                self.search_management.prepared = None;
            }
            AppAction::MoveSearchScratchpads {
                expected,
                selected,
                before,
                open_after,
            } => {
                if !self.search_management.busy() && self.view() == View::AllPages {
                    self.transition(
                        Transition::Recovery(RecoveryIntent::MoveScratchpads {
                            expected,
                            selected,
                            before,
                        }),
                        &mut effects,
                    );
                    self.search_management.operation = Some(self.transition_id);
                    self.search_management.open_after =
                        open_after.map(|key| (self.transition_id, key));
                }
            }
            AppAction::NewNoteFromSearch(text) => {
                if !text.trim().is_empty() && !self.search_management.busy() {
                    self.transition(Transition::New(Some(text)), &mut effects);
                }
            }
            AppAction::RefreshScratchpads => self.refresh_scratchpads(&mut effects),
            AppAction::StepScratchpad(step) => {
                if step != 0 {
                    self.step_scratchpad(step, &mut effects);
                }
            }
            AppAction::JumpToNewest => self.step_scratchpad(0, &mut effects),
            AppAction::ResolveConflict(request) => self.transition(
                Transition::Recovery(RecoveryIntent::Resolve(request)),
                &mut effects,
            ),
            AppAction::MoveToTrash(key, revision) => {
                if key.connection == self.connection {
                    self.transition(
                        Transition::Recovery(RecoveryIntent::Trash(key, revision)),
                        &mut effects,
                    );
                }
            }
            AppAction::RestoreTrash(id, path) => self.transition(
                Transition::Recovery(RecoveryIntent::RestoreTrash(id, path)),
                &mut effects,
            ),
            AppAction::RestoreRecovery(id, path) => self.transition(
                Transition::Recovery(RecoveryIntent::RestoreCopy(id, path)),
                &mut effects,
            ),
            AppAction::EmptyTrash(ids) => self.transition(
                Transition::Recovery(RecoveryIntent::EmptyTrash(ids)),
                &mut effects,
            ),
            AppAction::CancelOpen => {
                self.stack_intent = None;
                self.stack_cursor = None;
                self.barrier_waiting = false;
                self.after_delete = None;
                self.transition_id = self.transition_id.saturating_add(1);
                self.pending_load = None;
                self.cancel_transition(&mut effects);
                if self.selected_uncached.take().is_some() {
                    self.notice = None;
                }
            }
            AppAction::EditText(text) => {
                if let Some(editor) = &mut self.editor {
                    match editor.edit(text, self.now) {
                        Ok(true) => {
                            self.pending_load = None;
                            if !self.barrier_waiting {
                                self.cancel_transition(&mut effects);
                            }
                            self.stack_cursor = None;
                            self.notice = None;
                            self.selected_uncached = None;
                        }
                        Ok(false) => {}
                        Err(_) => self.notice = Some("This page cannot be edited."),
                    }
                }
            }
            AppAction::Tick(now) => {
                self.now = self.now.max(now);
                if self
                    .editor
                    .as_ref()
                    .is_some_and(|editor| editor.due(self.now))
                {
                    self.flush(&mut effects);
                }
            }
            AppAction::Navigate(view) => {
                self.background_view = None;
                self.search_management.prepared = None;
                self.stack_intent = None;
                self.stack_cursor = None;
                self.barrier_waiting = false;
                self.after_delete = None;
                self.transition_id = self.transition_id.saturating_add(1);
                self.pending_load = None;
                self.cancel_transition(&mut effects);
                if view != View::AllPages && self.selected_uncached.take().is_some() {
                    self.notice = None;
                }
                self.navigation.set_view(view);
                self.flush(&mut effects);
            }
            AppAction::OpenPage(key) => {
                self.background_view = None;
                self.stack_intent = None;
                self.stack_cursor = None;
                self.selected_uncached = None;
                if key.connection == self.connection {
                    if self.pending_load.is_none()
                        && self
                            .editor
                            .as_ref()
                            .is_some_and(|editor| editor.key() == &key)
                    {
                        self.cancel_transition(&mut effects);
                        self.navigation.set_view(View::Note);
                        self.flush(&mut effects);
                    } else {
                        self.transition(Transition::Open(key, None), &mut effects);
                    }
                }
            }
            AppAction::NewNote => {
                self.background_view = None;
                self.stack_intent = None;
                self.stack_cursor = None;
                self.selected_uncached = None;
                self.transition(Transition::New(None), &mut effects);
            }
            AppAction::StepHistory(step) => {
                self.selected_uncached = None;
                self.selected_page_error = None;
                self.notice = None;
                let mut history = self.navigation.clone();
                let key = if step < 0 {
                    history.back()
                } else if step > 0 {
                    history.forward()
                } else {
                    None
                };
                if let Some(key) = key {
                    self.transition(Transition::Open(key, Some((history, step))), &mut effects);
                }
            }
            AppAction::Search(request) => {
                if request.query.connection == self.connection
                    && self.latest_search.is_none_or(|id| request.id > id)
                {
                    let same_query = self.navigation.search_query == request.query.text
                        && self.search_folder == request.query.folder
                        && self.search_scope == request.query.scope;
                    if request.query.offset > 0
                        && (!same_query
                            || self.search_pending
                            || self
                                .results
                                .as_ref()
                                .is_none_or(|results| results.items.len() != request.query.offset))
                    {
                        return effects;
                    }
                    self.search_offset = request.query.offset;
                    self.search_pending = true;
                    self.latest_search = Some(request.id);
                    if !same_query {
                        self.completed_search = None;
                        self.results = None;
                    }
                    self.search_folder = request.query.folder.clone();
                    self.search_scope = request.query.scope;
                    self.navigation.search_query = request.query.text.clone();
                    effects.push(AppEffect::Search(request));
                }
            }
            AppAction::SavePosition(position) => {
                if let Some(editor) = &mut self.editor {
                    if editor.observe_position(position).is_ok() {
                        self.navigation.save_position(position);
                        if !editor.dirty() {
                            effects.push(AppEffect::SavePosition {
                                key: editor.key().clone(),
                                position,
                            });
                        }
                    } else {
                        self.notice = Some("Editing position could not be recorded.");
                    }
                }
                if let Some(request) = self.pending_load.take() {
                    self.pending_transition = Some(Transition::Open(request.key, request.history));
                }
                if self.pending_transition.is_some() {
                    self.transition_id = self.transition_id.saturating_add(1);
                    self.barrier_waiting = false;
                    self.barrier_ready = false;
                    self.finish_transition(&mut effects);
                }
            }
            AppAction::Flush => {
                self.flush_target = self.editor.as_ref().map(|editor| editor.revision());
                self.flush(&mut effects);
                if self.pending_transition.is_some() {
                    self.finish_transition(&mut effects);
                }
            }
            AppAction::RequestHide => self.transition(Transition::Hide, &mut effects),
            AppAction::RequestQuit => self.transition(Transition::Quit, &mut effects),
        }
        effects
    }
    pub fn receive(&mut self, event: AppEvent) -> Vec<AppEffect> {
        let mut effects = Vec::new();
        match event {
            AppEvent::TransitionChecked {
                request_id,
                connection,
                result,
            } => {
                if !self.barrier_waiting
                    || request_id != self.transition_id
                    || connection != self.connection
                    || self.pending_transition.is_none()
                {
                    return effects;
                }
                self.barrier_waiting = false;
                if result.is_err() {
                    self.cancel_transition(&mut effects);
                    self.stack_cursor = None;
                    self.notice = Some(METADATA_FAILURE);
                    return effects;
                }
                self.barrier_ready = true;
                self.finish_transition(&mut effects);
            }
            AppEvent::ScratchpadsLoaded {
                request_id,
                connection,
                result,
            } => {
                if !self.stack_loading
                    || request_id != self.stack_request
                    || connection != self.connection
                {
                    return effects;
                }
                self.stack_loading = false;
                match result {
                    Ok(entries)
                        if entries.iter().all(|e| e.key.connection == self.connection)
                            && entries
                                .iter()
                                .map(|e| &e.key)
                                .collect::<std::collections::HashSet<_>>()
                                .len()
                                == entries.len() =>
                    {
                        self.stack = Some(super::scratchpads::ScratchpadStack::new(entries));
                        if let Some(candidates) = self.after_delete.take() {
                            let destination = self.stack.as_ref().and_then(|stack| {
                                candidates
                                    .into_iter()
                                    .find(|k| stack.position(k).is_some())
                                    .or_else(|| stack.newest())
                            });
                            if let Some(key) = destination {
                                self.transition(Transition::Open(key, None), &mut effects);
                            } else {
                                self.transition(Transition::New(None), &mut effects);
                            }
                        } else if let Some(step) = self.stack_intent.take() {
                            self.step_scratchpad(step, &mut effects);
                        }
                    }
                    _ => self.notice = Some("Note stack could not be loaded. Try again."),
                }
                if std::mem::take(&mut self.stack_again) {
                    self.refresh_scratchpads(&mut effects);
                }
            }

            AppEvent::SelectedPageStatus { key, error } => {
                if self.selected_uncached.as_ref() == Some(&key) {
                    self.selected_page_error = error.map(|error| (key, error));
                    self.uncached_notice();
                }
            }
            AppEvent::RecoveryFinished {
                request_id,
                connection,
                result,
            } => {
                if connection != self.connection {
                    return effects;
                }
                let search_operation = self.search_management.operation == Some(request_id);
                if search_operation {
                    self.search_management.operation = None;
                }
                match result {
                    Ok(outcome) => {
                        if request_id != self.transition_id {
                            self.refresh_scratchpads(&mut effects);
                            return effects;
                        }
                        if let Some(pages) = outcome.prepared_trash {
                            self.search_management.prepared = Some((request_id, pages));
                            return effects;
                        }
                        if let Some(entries) = outcome.stack {
                            self.stack = Some(super::scratchpads::ScratchpadStack::new(entries));
                            if let Some((_, key)) = self
                                .search_management
                                .open_after
                                .take()
                                .filter(|(id, _)| *id == request_id)
                            {
                                if self.view() == View::AllPages {
                                    effects.extend(self.dispatch(AppAction::OpenPage(key)));
                                }
                            }
                            return effects;
                        }
                        if outcome.page.is_none() {
                            for key in &outcome.affected {
                                self.navigation.remove_unavailable(key);
                            }
                        }
                        self.active_conflict = None;
                        self.refresh_scratchpads(&mut effects);
                        if request_id != self.transition_id {
                            return effects;
                        }
                        if let Some(key) = outcome.restored {
                            self.transition(Transition::Open(key, None), &mut effects);
                        } else if outcome.affected.iter().any(|key| {
                            self.editor.as_ref().is_some_and(|editor| {
                                editor.key() == key && !editor.dirty() && !editor.writing()
                            })
                        }) {
                            match outcome.page {
                                Some(page) => self.editor = Some(EditorSession::restore(page)),
                                None => {
                                    if search_operation && self.view() == View::AllPages {
                                        self.background_view = Some(View::AllPages);
                                    }
                                    self.editor = None;
                                    if let Some((id, candidates)) = self
                                        .recovery_successors
                                        .take()
                                        .filter(|(id, _)| *id == request_id)
                                    {
                                        let _ = id;
                                        self.after_delete = Some(candidates);
                                    } else {
                                        self.editor = None;
                                        self.finish_new_after_recovery(&mut effects);
                                    }
                                }
                            }
                        }
                    }
                    Err(_) => {
                        self.notice = Some(
                            "Recovery action could not finish. Your note and recovery copies are still available.",
                        )
                    }
                }
            }
            AppEvent::CatalogueProgress {
                connection,
                indexed,
                total,
                complete,
            } => {
                if connection == self.connection {
                    self.catalogue_progress = Some((indexed, total, complete));
                }
            }
            AppEvent::SyncStateChanged { connection, state } => {
                if connection == self.connection {
                    self.remote_state = state;
                    self.uncached_notice();
                }
            }
            AppEvent::ConflictDetected {
                key,
                local_revision,
                id,
            } => {
                if key.connection == self.connection {
                    self.active_conflict = Some((key, local_revision, id));
                }
            }
            AppEvent::RemoteApplied(page) => {
                if let Some(editor) = &mut self.editor {
                    editor.apply_remote_snapshot(page);
                }
            }
            AppEvent::MetadataDurable => {
                if self.notice == Some(METADATA_FAILURE) {
                    self.notice = None;
                }
            }
            AppEvent::MetadataFailed(key) => {
                if self
                    .editor
                    .as_ref()
                    .is_some_and(|editor| editor.key() == &key)
                {
                    self.stack_cursor = None;
                    self.cancel_transition(&mut effects);
                    self.pending_load = None;
                    self.barrier_waiting = false;
                    self.notice = Some(METADATA_FAILURE);
                }
            }
            AppEvent::RebaseReady {
                expected,
                stored,
                edits,
            } => {
                if let Some(editor) = &mut self.editor
                    && editor.apply_rebase(&expected, stored, &edits)
                    && let Some(position) = editor.observed_position()
                {
                    self.navigation.save_position(position);
                    effects.push(AppEffect::SavePosition {
                        key: editor.key().clone(),
                        position,
                    });
                }
            }
            AppEvent::LocalSaved { expected, outcome } => {
                let saved = &outcome.attempt;
                let newly_created = self
                    .editor
                    .as_ref()
                    .is_some_and(|editor| editor.needs_create());
                if self
                    .editor
                    .as_mut()
                    .is_some_and(|editor| editor.acknowledge(&expected, &outcome))
                {
                    self.notice = None;
                    if newly_created {
                        self.refresh_scratchpads(&mut effects);
                        effects.push(AppEffect::RecordVisit(saved.key.clone()));
                    }
                    if self
                        .flush_target
                        .is_some_and(|target| saved.local_revision >= target)
                    {
                        self.flush_target = None;
                    }
                    if let Some(editor) = &self.editor
                        && !editor.dirty()
                        && let Some(position) = editor.observed_position()
                    {
                        effects.push(AppEffect::SavePosition {
                            key: editor.key().clone(),
                            position,
                        });
                    }
                    if self.pending_transition.is_some() {
                        self.finish_transition(&mut effects);
                    } else if self.flush_target.is_some()
                        || self
                            .editor
                            .as_ref()
                            .is_some_and(|editor| editor.due(self.now))
                    {
                        self.flush(&mut effects);
                    }
                }
            }
            AppEvent::LocalSaveFailed(expected, revision, _) => {
                if self
                    .editor
                    .as_mut()
                    .is_some_and(|editor| editor.fail(&expected, revision))
                {
                    self.cancel_transition(&mut effects);
                    self.pending_load = None;
                    self.notice = Some(
                        "Local save failed. Your text is still here; retry saving or copy it.",
                    );
                }
            }
            AppEvent::PageLoaded {
                request_id,
                key,
                result,
            } => {
                if !self
                    .pending_load
                    .as_ref()
                    .is_some_and(|request| request.id == request_id && request.key == key)
                {
                    return effects;
                }
                let Some(request) = self.pending_load.take() else {
                    return effects;
                };
                match result {
                    Ok(Some(mut restored)) if restored.presented().key == key => {
                        let page = restored
                            .authored
                            .as_mut()
                            .unwrap_or(&mut restored.stored.page);
                        self.selected_uncached = None;
                        self.selected_page_error = None;
                        if let Some((history, _)) = request.history {
                            if let Some(position) = history
                                .position()
                                .filter(|position| position.validate(&page.body).is_ok())
                            {
                                page.position = position;
                            }
                            self.navigation = history;
                        } else {
                            self.navigation.open(key.clone(), page.position);
                        }
                        self.navigation
                            .set_view(self.background_view.take().unwrap_or(View::Note));
                        self.editor = Some(EditorSession::restore(restored));
                        self.notice = None;
                        self.stack_cursor = None;
                        effects.push(AppEffect::RecordVisit(key));
                    }
                    Ok(None) => {
                        self.selected_uncached = Some(key);
                        self.uncached_notice();
                    }
                    Err(StoreError::PageMissing) if self.stack_cursor.as_ref() == Some(&key) => {
                        let destination = self.stack.as_mut().and_then(|stack| {
                            let next = if self.stack_step <= 0 {
                                stack.older(&key)
                            } else {
                                stack.newer(&key)
                            };
                            stack.remove(&key);
                            next
                        });
                        if let Some(next) = destination {
                            self.transition(Transition::Open(next.clone(), None), &mut effects);
                            self.stack_cursor = Some(next);
                        } else {
                            self.stack_cursor = None;
                            self.notice = Some("That scratchpad is no longer available.");
                        }
                        self.refresh_scratchpads(&mut effects);
                    }
                    Err(StoreError::PageMissing) if request.history.is_some() => {
                        let (_, step) = request.history.expect("guarded history request");
                        self.navigation.remove_unavailable(&key);
                        effects.extend(self.dispatch(AppAction::StepHistory(step)));
                    }
                    _ => {
                        self.notice = Some(
                            "Page could not be opened locally. Your current note is still here.",
                        );
                    }
                }
            }
            AppEvent::SearchReady(id, result) => {
                if Some(id) == self.latest_search && self.search_pending {
                    self.search_pending = false;
                    match result {
                        Ok(mut results)
                            if results
                                .items
                                .iter()
                                .all(|page| page.key.connection == self.connection) =>
                        {
                            if self.search_offset > 0 {
                                let Some(previous) = self
                                    .results
                                    .as_mut()
                                    .filter(|previous| previous.items.len() == self.search_offset)
                                else {
                                    self.notice =
                                        Some("Page list changed. Search again to reload it.");
                                    return effects;
                                };
                                let mut items = std::mem::take(&mut previous.items);
                                items.append(&mut results.items);
                                results.items = items;
                            }
                            self.completed_search = Some(id);
                            self.results = Some(results);
                            self.notice = None;
                        }
                        _ => self.notice = Some("Search could not be completed. Try again."),
                    }
                }
            }
        }
        effects
    }
    fn finish_new_after_recovery(&mut self, effects: &mut Vec<AppEffect>) {
        let view = self.navigation.view();
        self.pending_transition = Some(Transition::New(None));
        self.finish_transition(effects);
        self.navigation.set_view(view);
    }
    fn flush(&mut self, effects: &mut Vec<AppEffect>) {
        let Some(editor) = &mut self.editor else {
            return;
        };
        if !editor.dirty() {
            self.flush_target = None;
            return;
        }
        if editor.writing() {
            return;
        }
        if editor.needs_create() {
            let expected = editor.start_write(LocalRevision(0));
            effects.push(AppEffect::CreateDraft {
                key: editor.key().clone(),
                expected,
            });
        } else {
            let revision = editor.revision();
            let body = editor.body().to_owned();
            let key = editor.key().clone();
            let baseline = Box::new(editor.baseline());
            let expected = editor.start_write(revision);
            effects.push(AppEffect::SaveLocal {
                expected,
                key,
                body,
                revision,
                baseline,
            });
        }
    }
    fn cancel_transition(&mut self, effects: &mut Vec<AppEffect>) {
        match self.pending_transition.take() {
            Some(Transition::Recovery(_)) => {
                self.search_management.operation = None;
                effects.push(AppEffect::RecoveryCancelled {
                    connection: self.connection.clone(),
                });
            }
            Some(Transition::Hide | Transition::Quit) => effects.push(AppEffect::CloseCancelled),
            _ => {}
        }
        self.barrier_waiting = false;
        self.barrier_ready = false;
    }
    fn transition(&mut self, transition: Transition, effects: &mut Vec<AppEffect>) {
        self.cancel_transition(effects);
        self.after_delete = None;
        self.stack_intent = None;
        self.transition_id = self.transition_id.saturating_add(1);
        self.barrier_waiting = false;
        self.barrier_ready = false;
        self.pending_load = None;
        self.pending_transition = Some(transition);
        self.finish_transition(effects);
    }
    fn finish_transition(&mut self, effects: &mut Vec<AppEffect>) {
        if self
            .editor
            .as_ref()
            .is_some_and(|editor| editor.dirty() || editor.writing())
        {
            self.barrier_ready = false;
            self.flush(effects);
            return;
        }
        if self.pending_transition.as_ref().is_some_and(|t| {
            matches!(
                t,
                Transition::Open(..)
                    | Transition::New(_)
                    | Transition::Recovery(..)
                    | Transition::Hide
                    | Transition::Quit
            )
        }) && !self.barrier_ready
        {
            if !self.barrier_waiting {
                self.barrier_waiting = true;
                effects.push(AppEffect::CheckTransition {
                    request_id: self.transition_id,
                    connection: self.connection.clone(),
                });
            }
            return;
        }
        let Some(transition) = self.pending_transition.take() else {
            return;
        };
        match transition {
            Transition::Recovery(intent) => {
                let deleted: Vec<_> = match &intent {
                    RecoveryIntent::Trash(key, _) => vec![key.clone()],
                    RecoveryIntent::TrashMany(pages) => {
                        pages.iter().map(|(key, _)| key.clone()).collect()
                    }
                    _ => vec![],
                };
                if let Some(editor) = &self.editor
                    && deleted.contains(editor.key())
                    && let Some(stack) = &self.stack
                {
                    self.recovery_successors = Some((
                        self.transition_id,
                        stack
                            .deletion_successors(editor.key())
                            .into_iter()
                            .filter(|key| !deleted.contains(key))
                            .collect(),
                    ));
                }
                effects.push(AppEffect::Recovery {
                    request_id: self.transition_id,
                    connection: self.connection.clone(),
                    intent,
                });
            }
            Transition::Hide => effects.push(AppEffect::HideReady),
            Transition::Quit => effects.push(AppEffect::QuitReady),
            Transition::Open(key, history) => {
                let Some(request_id) = self.next_load.checked_add(1) else {
                    self.notice = Some("Page navigation limit reached. Restart after saving.");
                    return;
                };
                self.next_load = request_id;
                self.pending_load = Some(PendingLoad {
                    id: request_id,
                    key: key.clone(),
                    history,
                });
                effects.push(AppEffect::LoadPage { request_id, key });
            }
            Transition::New(seed) => {
                let key = (self.make_draft)();
                if key.connection != self.connection {
                    self.notice = Some("Quick note could not be created.");
                    return;
                }
                self.editor = Some(EditorSession::draft(key.clone()));
                if let Some(text) = seed
                    && let Some(editor) = &mut self.editor
                {
                    let _ = editor.edit(text, self.now);
                }
                self.navigation.open(key, EditorPosition::default());
                if let Some(view) = self.background_view.take() {
                    self.navigation.set_view(view);
                }
                self.notice = None;
                self.flush(effects);
            }
        }
    }
}
