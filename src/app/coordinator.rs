//! Revision-tagged application transitions; dispatch never performs blocking I/O.
use super::{
    editor::EditorSession,
    model::{EditorPosition, View},
    navigation::NavigationState,
    recovery::{RecoveryIntent, RecoveryOutcome},
};
use crate::{
    domain::{connection::ConnectionId, error::StoreError, page::*},
    store::search::{SearchQuery, SearchResults},
};
use std::time::Duration;

#[derive(Clone)]
pub struct SearchRequest {
    pub id: u64,
    pub query: SearchQuery,
}
pub enum AppAction {
    ResolveConflict(ConflictResolution),
    MoveToTrash(PageKey, LocalRevision),
    RestoreTrash(TrashId, Option<PagePath>),
    RestoreRecovery(uuid::Uuid, Option<PagePath>),
    EmptyTrash(Vec<TrashId>),
    EditText(String),
    CancelOpen,
    Navigate(View),
    OpenPage(PageKey),
    NewNote,
    StepHistory(i8),
    Search(SearchRequest),
    SavePosition(EditorPosition),
    Flush,
    RequestQuit,
    RequestHide,
    Tick(Duration),
}
pub enum AppEvent {
    RecoveryFinished {
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
    LocalSaved(SavedRevision),
    LocalSaveFailed(PageKey, LocalRevision, StoreError),
    SearchReady(u64, Result<SearchResults, StoreError>),
    PageLoaded {
        request_id: u64,
        key: PageKey,
        result: Result<Option<PageSnapshot>, StoreError>,
    },
}
pub enum AppEffect {
    Recovery(RecoveryIntent),
    CreateDraft(PageKey),
    SaveLocal {
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
    Open(PageKey, Option<NavigationState>),
    New,
    Quit,
    Hide,
}
struct PendingLoad {
    id: u64,
    key: PageKey,
    history: Option<NavigationState>,
}
pub struct Coordinator {
    connection: ConnectionId,
    editor: Option<EditorSession>,
    navigation: NavigationState,
    pending_transition: Option<Transition>,
    pending_load: Option<PendingLoad>,
    next_load: u64,
    now: Duration,
    latest_search: Option<u64>,
    completed_search: Option<u64>,
    flush_target: Option<LocalRevision>,
    results: Option<SearchResults>,
    notice: Option<&'static str>,
    make_draft: Box<dyn FnMut() -> PageKey>,
    selected_uncached: Option<PageKey>,
    catalogue_progress: Option<(usize, usize, bool)>,
    remote_state: crate::domain::status::SyncState,
    active_conflict: Option<(PageKey, LocalRevision, uuid::Uuid)>,
    search_folder: Option<PagePath>,
}
impl Coordinator {
    pub fn new(connection: ConnectionId, make_draft: impl FnMut() -> PageKey + 'static) -> Self {
        Self {
            connection,
            editor: None,
            navigation: NavigationState::default(),
            pending_transition: None,
            pending_load: None,
            next_load: 0,
            now: Duration::ZERO,
            latest_search: None,
            completed_search: None,
            flush_target: None,
            results: None,
            notice: None,
            make_draft: Box::new(make_draft),
            selected_uncached: None,
            catalogue_progress: None,
            remote_state: Default::default(),
            active_conflict: None,
            search_folder: None,
        }
    }
    pub fn editor(&self) -> Option<&EditorSession> {
        self.editor.as_ref()
    }
    pub fn view(&self) -> View {
        self.navigation.view()
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
        self.pending_load.is_some()
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
        if self.selected_uncached.is_some() {
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
    pub fn dispatch(&mut self, action: AppAction) -> Vec<AppEffect> {
        let mut effects = Vec::new();
        match action {
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
                self.pending_load = None;
                self.pending_transition = None;
                if self.selected_uncached.take().is_some() {
                    self.notice = None;
                }
            }
            AppAction::EditText(text) => {
                if let Some(editor) = &mut self.editor {
                    match editor.edit(text, self.now) {
                        Ok(true) => {
                            self.pending_load = None;
                            self.pending_transition = None;
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
                self.pending_load = None;
                self.pending_transition = None;
                if view != View::AllPages && self.selected_uncached.take().is_some() {
                    self.notice = None;
                }
                self.navigation.set_view(view);
                self.flush(&mut effects);
            }
            AppAction::OpenPage(key) => {
                self.selected_uncached = None;
                if key.connection == self.connection {
                    if self.pending_load.is_none()
                        && self
                            .editor
                            .as_ref()
                            .is_some_and(|editor| editor.key() == &key)
                    {
                        self.navigation.set_view(View::Note);
                        self.flush(&mut effects);
                    } else {
                        self.transition(Transition::Open(key, None), &mut effects);
                    }
                }
            }
            AppAction::NewNote => {
                self.selected_uncached = None;
                self.transition(Transition::New, &mut effects);
            }
            AppAction::StepHistory(step) => {
                let mut history = self.navigation.clone();
                let key = if step < 0 {
                    history.back()
                } else if step > 0 {
                    history.forward()
                } else {
                    None
                };
                if let Some(key) = key {
                    self.transition(Transition::Open(key, Some(history)), &mut effects);
                }
            }
            AppAction::Search(request) => {
                if request.query.connection == self.connection
                    && self.latest_search.is_none_or(|id| request.id > id)
                {
                    let same_query = self.navigation.search_query == request.query.text
                        && self.search_folder == request.query.folder;
                    self.latest_search = Some(request.id);
                    if !same_query {
                        self.completed_search = None;
                        self.results = None;
                    }
                    self.search_folder = request.query.folder.clone();
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
            }
            AppAction::Flush => {
                self.flush_target = self.editor.as_ref().map(|editor| editor.revision());
                self.flush(&mut effects);
            }
            AppAction::RequestHide => self.transition(Transition::Hide, &mut effects),
            AppAction::RequestQuit => self.transition(Transition::Quit, &mut effects),
        }
        effects
    }
    pub fn receive(&mut self, event: AppEvent) -> Vec<AppEffect> {
        let mut effects = Vec::new();
        match event {
            AppEvent::RecoveryFinished { connection, result } => {
                if connection != self.connection {
                    return effects;
                }
                match result {
                    Ok(outcome) => {
                        self.active_conflict = None;
                        if let Some(key) = outcome.restored {
                            self.transition(Transition::Open(key, None), &mut effects);
                        } else if outcome.affected.as_ref().is_some_and(|key| {
                            self.editor.as_ref().is_some_and(|editor| {
                                editor.key() == key && !editor.dirty() && !editor.writing()
                            })
                        }) {
                            match outcome.page {
                                Some(page) => self.editor = Some(EditorSession::restore(page)),
                                None => {
                                    self.editor = None;
                                    self.finish_new_after_recovery(&mut effects);
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
            AppEvent::MetadataFailed(key) => {
                if self
                    .editor
                    .as_ref()
                    .is_some_and(|editor| editor.key() == &key)
                {
                    self.notice =
                        Some("Page history could not be saved. Note text is still available.");
                }
            }
            AppEvent::LocalSaved(saved) => {
                let newly_created = self
                    .editor
                    .as_ref()
                    .is_some_and(|editor| editor.needs_create());
                if self
                    .editor
                    .as_mut()
                    .is_some_and(|editor| editor.acknowledge(&saved))
                {
                    self.notice = None;
                    if newly_created {
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
            AppEvent::LocalSaveFailed(key, revision, _) => {
                if self
                    .editor
                    .as_mut()
                    .is_some_and(|editor| editor.fail(&key, revision))
                {
                    self.pending_transition = None;
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
                    Ok(Some(mut page)) if page.key == key => {
                        if let Some(history) = request.history {
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
                        self.navigation.set_view(View::Note);
                        self.editor = Some(EditorSession::restore(page));
                        self.notice = None;
                        effects.push(AppEffect::RecordVisit(key));
                    }
                    Ok(None) => {
                        self.selected_uncached = Some(key);
                        self.uncached_notice();
                    }
                    _ => {
                        self.notice = Some(
                            "Page could not be opened locally. Your current note is still here.",
                        );
                    }
                }
            }
            AppEvent::SearchReady(id, result) => {
                if Some(id) == self.latest_search {
                    match result {
                        Ok(results)
                            if results
                                .items
                                .iter()
                                .all(|page| page.key.connection == self.connection) =>
                        {
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
        self.pending_transition = Some(Transition::New);
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
            editor.start_write(LocalRevision(0));
            effects.push(AppEffect::CreateDraft(editor.key().clone()));
        } else {
            let revision = editor.revision();
            let body = editor.body().to_owned();
            let key = editor.key().clone();
            let baseline = Box::new(editor.baseline());
            editor.start_write(revision);
            effects.push(AppEffect::SaveLocal {
                key,
                body,
                revision,
                baseline,
            });
        }
    }
    fn transition(&mut self, transition: Transition, effects: &mut Vec<AppEffect>) {
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
            self.flush(effects);
            return;
        }
        let Some(transition) = self.pending_transition.take() else {
            return;
        };
        match transition {
            Transition::Recovery(intent) => effects.push(AppEffect::Recovery(intent)),
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
            Transition::New => {
                let key = (self.make_draft)();
                if key.connection != self.connection {
                    self.notice = Some("Quick note could not be created.");
                    return;
                }
                self.editor = Some(EditorSession::draft(key.clone()));
                self.navigation.open(key, EditorPosition::default());
                self.notice = None;
                self.flush(effects);
            }
        }
    }
}
