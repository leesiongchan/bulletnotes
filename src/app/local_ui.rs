//! Local-only UI wiring. Network access and the production macOS lifecycle are separate tasks.
use super::{
    appearance_ui::AppearanceUi,
    coordinator::{AppAction, AppEffect, AppEvent, Coordinator, SearchRequest},
    find::NoteFind,
    model::View,
};
use crate::{
    AppWindow, PageRow,
    domain::{connection::ConnectionId, error::StoreError, page::*, status::LocalSaveState},
    store::{StoreHandle, search::SearchQuery},
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
}
#[derive(Clone)]
pub struct LocalUi(Rc<State>);
impl LocalUi {
    pub fn dispatch(&self, action: AppAction) {
        self.0.dispatch(action);
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
    fn dispatch(self: &Rc<Self>, action: AppAction) {
        if self.quitting.get() {
            return;
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
        if self.invalid_folder.get() && matches!(&event, AppEvent::SearchReady(..)) {
            return;
        }
        let mut app = self.coordinator.borrow_mut();
        let mut effects = app.dispatch(AppAction::Tick(self.started.elapsed()));
        effects.extend(app.receive(event));
        drop(app);
        self.execute(effects);
        self.render();
        self.arm_save();
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
                        let _ = window.hide();
                    }
                }
                effect => {
                    if let Err(error) = self.writes.try_send(Work::Effect(effect))
                        && let Work::Effect(effect) = error.into_inner()
                    {
                        let event = match effect {
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
            offset: 0,
            limit: 5000,
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
            if changed || self.rendered_revision.get() != Some(editor.revision()) {
                if window.get_note_text().as_str() != editor.body() {
                    window.set_note_text(editor.body().into());
                }
                self.find.borrow_mut().refresh(editor.body());
                self.present_find(&window);
                window.set_current_path(editor.key().path.as_str().into());
                self.rendered_key.replace(Some(editor.key().clone()));
                self.rendered_revision.set(Some(editor.revision()));
                if changed {
                    if app.view() == View::Note {
                        window.invoke_focus_editor();
                    }
                    let position = editor.restore_target();
                    if let (Ok(anchor), Ok(caret)) = (
                        i32::try_from(position.anchor),
                        i32::try_from(position.caret),
                    ) {
                        window.invoke_restore_editor_position(anchor, caret, position.scroll_y);
                    }
                }
            }
            window.set_note_read_only(
                self.quitting.get() || editor.permission() == Permission::ReadOnly,
            );
            let status = app.notice().unwrap_or_else(|| match editor.save_state() {
                LocalSaveState::Clean(_) => "Saved locally · not syncing",
                LocalSaveState::Dirty(_) => "Unsaved changes",
                LocalSaveState::Saving(_) => "Saving locally…",
                LocalSaveState::Failed(_) => "Local save failed · ⌘S to retry",
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
        if self.invalid_folder.get() {
            window.set_index_status("Enter a relative folder path".into());
        } else if let Some(message) = app.notice() {
            window.set_index_status(message.into());
        } else if let Some(results) = app.search_results() {
            window.set_index_status(
                format!(
                    "{} local pages · {} of {} indexed · not syncing",
                    results.total, results.indexed, results.catalogue_total
                )
                .into(),
            );
        } else {
            window.set_index_status("Searching local pages…".into());
        }
        self.updates.send_replace(());
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
                state.store.shutdown().await
            }
            .await;
            if result.is_ok() {
                let _ = slint::quit_event_loop();
            } else {
                state.quitting.set(false);
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
                    }) => Some(match store.save_local(key.clone(), body, revision).await {
                        Ok(saved) => AppEvent::LocalSaved(saved),
                        Err(error) => AppEvent::LocalSaveFailed(key, revision, error),
                    }),
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
                    let result = store.load_page(&request.key).await;
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
            state.dispatch(AppAction::Navigate(View::AllPages));
            state.issue_search();
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
            state.issue_search();
        });
    }
    {
        let state = state.clone();
        window.on_folder_changed(move |_| {
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
