//! Main-thread review state. Metadata lists load without fetching every saved body.
use super::{coordinator::AppAction, local_ui::LocalUi, model::View};
use crate::{
    AppWindow, PageRow,
    domain::{error::StoreError, page::*},
    store::StoreHandle,
};
use slint::{ComponentHandle, ModelRc, VecModel};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
    time::Duration,
};
struct State {
    window: slint::Weak<AppWindow>,
    local: LocalUi,
    store: StoreHandle,
    selected: RefCell<Option<RecoveryItem>>,
    rows: RefCell<Vec<RecoverySummary>>,
    drafts: RefCell<HashMap<PageKey, String>>,
    pending_drafts: RefCell<HashMap<PageKey, (u64, String)>>,
    draft_generation: Cell<u64>,
    draft_writing: Cell<bool>,
    draft_timer: slint::Timer,
    draft_updates: tokio::sync::watch::Sender<()>,
    epoch: Cell<u64>,
    loading: Cell<bool>,
    again: Cell<bool>,
    timer: slint::Timer,
}
#[derive(Clone)]
pub struct RecoveryUi(Rc<State>);
impl RecoveryUi {
    pub async fn flush(&self) -> Result<(), StoreError> {
        let mut updates = self.0.draft_updates.subscribe();
        loop {
            self.0.flush_drafts().await?;
            if self
                .0
                .window
                .upgrade()
                .is_none_or(|window| !window.get_recovery_busy())
            {
                return Ok(());
            }
            updates.changed().await.map_err(|_| StoreError::Closed)?;
        }
    }
    pub fn refresh(&self) {
        self.0.schedule();
    }
}
fn item_key(item: &RecoveryItem) -> &PageKey {
    match item {
        RecoveryItem::Conflict(record) => &record.key,
        RecoveryItem::Trash(record) => &record.page.key,
        RecoveryItem::Copy(record) => &record.page.key,
    }
}
fn kind(window: &AppWindow) -> RecoveryKind {
    match window.get_recovery_section().as_str() {
        "trash" => RecoveryKind::Trash,
        "copies" => RecoveryKind::Copy,
        _ => RecoveryKind::Conflict,
    }
}
fn item_id(item: &RecoveryItem) -> uuid::Uuid {
    match item {
        RecoveryItem::Conflict(record) => record.id,
        RecoveryItem::Trash(record) => record.id.0,
        RecoveryItem::Copy(record) => record.id,
    }
}
impl State {
    async fn flush_drafts(&self) -> Result<(), StoreError> {
        let mut updates = self.draft_updates.subscribe();
        loop {
            if self.draft_writing.get() {
                updates.changed().await.map_err(|_| StoreError::Closed)?;
                continue;
            }
            let pending = self.pending_drafts.borrow().clone();
            if pending.is_empty() {
                return Ok(());
            }
            self.draft_writing.set(true);
            let values = pending
                .iter()
                .map(|(key, (_, body))| (key.clone(), body.clone()))
                .collect();
            let result = self.store.save_merge_drafts(values).await;
            self.draft_writing.set(false);
            if result.is_ok() {
                let mut remaining = self.pending_drafts.borrow_mut();
                for (key, (generation, _)) in pending {
                    if remaining
                        .get(&key)
                        .is_some_and(|(current, _)| *current == generation)
                    {
                        remaining.remove(&key);
                    }
                }
            }
            self.draft_updates.send_replace(());
            if let Err(error) = result {
                self.message("Merged draft is not saved. Keep it available to copy, or retry before closing.");
                return Err(error);
            }
        }
    }
    fn edit_draft(self: &Rc<Self>, key: PageKey, body: String) {
        self.draft_generation
            .set(self.draft_generation.get().wrapping_add(1));
        self.drafts.borrow_mut().insert(key.clone(), body.clone());
        self.pending_drafts
            .borrow_mut()
            .insert(key, (self.draft_generation.get(), body));
        if !self.draft_timer.running() {
            let weak = Rc::downgrade(self);
            self.draft_timer.start(
                slint::TimerMode::SingleShot,
                Duration::from_millis(250),
                move || {
                    if let Some(state) = weak.upgrade() {
                        let _ = slint::spawn_local(async move {
                            let _ = state.flush_drafts().await;
                        });
                    }
                },
            );
        }
    }
    fn schedule(self: &Rc<Self>) {
        self.again.set(true);
        if self.timer.running() || self.loading.get() {
            return;
        }
        let weak = Rc::downgrade(self);
        self.timer.start(
            slint::TimerMode::SingleShot,
            Duration::from_millis(100),
            move || {
                if let Some(state) = weak.upgrade() {
                    state.refresh();
                }
            },
        );
    }
    fn message(&self, message: &str) {
        if let Some(window) = self.window.upgrade() {
            window.set_recovery_message(message.into());
        }
    }
    fn refresh(self: &Rc<Self>) {
        if self.loading.replace(true) {
            self.again.set(true);
            return;
        }
        self.again.set(false);
        let store = self.store.clone();
        let connection = self.local.connection_id();
        let weak = Rc::downgrade(self);
        if slint::spawn_local(async move {
            let result=store.recovery_index(&connection).await;let Some(state)=weak.upgrade()else{return;};state.loading.set(false);
            if connection==state.local.connection_id() && let Some(window)=state.window.upgrade(){
                match result {
                    Ok(rows)=>{
                        let visible:Vec<_>=rows.iter().filter(|row|row.kind==kind(&window)).collect();
                        let model=visible.iter().map(|row|PageRow{key:row.id.to_string().into(),title:format!("{} · {}",row.label,row.key.path.as_str()).into(),path:row.key.path.as_str().into(),excerpt:Default::default(),availability:Default::default()}).collect::<Vec<_>>();
                        window.set_recovery_items(ModelRc::new(VecModel::from(model)));
                        if let Some(selected)=state.selected.borrow().as_ref(){
                            if item_key(selected).connection!=connection{window.set_recovery_selected(false);}
                            else if !visible.iter().any(|row|row.id==item_id(selected)){
                                window.set_recovery_stale(true);state.message("Changes arrived during review. Reload to review them; your merged draft is kept.");
                            }
                        }
                        let first=visible.first().map(|row|row.id);state.rows.replace(rows);
                        if state.selected.borrow().is_none() && let Some(id)=first{state.select(id);}
                    },
                    Err(_)=>state.message("Recovery copies could not be loaded. Your current note is still available."),
                }
            }
            if state.again.get(){state.schedule();}
        }).is_err(){self.loading.set(false);self.message("Recovery could not start. Try again.");}
    }
    fn select(self: &Rc<Self>, id: uuid::Uuid) {
        let Some(window) = self.window.upgrade() else {
            return;
        };
        let selected_kind = kind(&window);
        let connection = self.local.connection_id();
        if !self.rows.borrow().iter().any(|row| {
            row.id == id && row.kind == selected_kind && row.key.connection == connection
        }) {
            return;
        }
        self.epoch.set(self.epoch.get().wrapping_add(1));
        let epoch = self.epoch.get();
        window.set_recovery_selected(false);
        let store = self.store.clone();
        let weak = Rc::downgrade(self);
        if slint::spawn_local(async move {
            let result=store.recovery_item(&connection,selected_kind,id).await;
            let saved_draft=match &result{Ok(RecoveryItem::Conflict(record))=>store.merge_draft(&record.key).await, _=>Ok(None)};
            let Some(state)=weak.upgrade()else{return;};
            let saved_draft=match saved_draft{Ok(draft)=>draft,Err(_)=>{state.message("Saved merged draft could not be loaded. Try again before editing it.");return;}};
            if state.epoch.get()!=epoch||state.local.connection_id()!=connection{return;}
            let Some(window)=state.window.upgrade()else{return;};
            match result {
                Ok(item)=>{
                    window.set_recovery_selected_id(id.to_string().into());window.set_recovery_path(item_key(&item).path.as_str().into());
                    window.set_recovery_destination(item_key(&item).path.as_str().into());window.set_recovery_stale(false);
                    window.set_recovery_remote("".into());window.set_recovery_base("".into());window.set_recovery_has_base(false);window.set_recovery_remote_deleted(false);window.set_recovery_writable(false);
                    match &item {
                        RecoveryItem::Conflict(record)=>{
                            window.set_recovery_local(record.local.body.as_str().into());
                            let draft=state.drafts.borrow().get(&record.key).cloned().or(saved_draft).unwrap_or_else(||record.local.body.clone());window.set_recovery_merge(draft.into());
                            if let Some(base)=&record.base{window.set_recovery_base(base.body.as_str().into());window.set_recovery_has_base(true);}
                            match &record.remote {RemoteRead::Missing(_)=>{window.set_recovery_remote_deleted(true);window.set_recovery_writable(true);},RemoteRead::Present(file)=>{window.set_recovery_remote(file.body.as_str().into());window.set_recovery_writable(file.permission==Permission::ReadWrite&&file.revision.is_some());}}
                            state.message("Review both versions. Resolved versions remain in Kept versions.");
                        },
                        RecoveryItem::Trash(record)=>{window.set_recovery_local(record.page.body.as_str().into());state.message("Restoration creates a page without replacing an existing path. The Trash copy remains until you delete it.");},
                        RecoveryItem::Copy(record)=>{window.set_recovery_local(record.page.body.as_str().into());state.message("This version is retained locally. Choose an unused path to restore it.");},
                    }
                    state.selected.replace(Some(item));window.set_recovery_selected(true);
                },
                Err(_)=>{window.set_recovery_stale(true);state.message("This review changed. Refresh and select its current version.");state.schedule();}
            }
        }).is_err(){self.message("Review could not load. Try again.");}
    }
    fn begin(self: &Rc<Self>, action: AppAction) {
        let Some(window) = self.window.upgrade() else {
            return;
        };
        if window.get_recovery_busy() {
            return;
        }
        window.set_recovery_busy(true);
        self.message("Saving recovery action locally…");
        let weak = Rc::downgrade(self);
        let local = self.local.clone();
        let connection = local.connection_id();
        if slint::spawn_local(async move {
            let result = local.flush().await;
            let Some(state) = weak.upgrade() else {
                return;
            };
            let result = match result {
                Ok(()) => state
                    .flush_drafts()
                    .await
                    .map_err(|_| super::local_ui::LocalUiError::SaveFailed),
                Err(error) => Err(error),
            };
            if result.is_err() || connection != local.connection_id() {
                if let Some(window) = state.window.upgrade() {
                    window.set_recovery_busy(false);
                }
                state.draft_updates.send_replace(());
                state.message(
                    "Your latest text could not be saved. Return to the note to retry or copy it.",
                );
                return;
            }
            local.dispatch(action);
        })
        .is_err()
        {
            window.set_recovery_busy(false);
            self.message("Recovery action could not start. Try again.");
        }
    }
    fn reload(self: &Rc<Self>) {
        let key = self
            .selected
            .borrow()
            .as_ref()
            .map(|item| item_key(item).clone());
        let selected_kind = self.window.upgrade().map(|window| kind(&window));
        let id = self
            .rows
            .borrow()
            .iter()
            .find(|row| Some(&row.key) == key.as_ref() && Some(row.kind) == selected_kind)
            .map(|row| row.id);
        if let Some(id) = id {
            self.select(id);
        } else {
            self.selected.borrow_mut().take();
            self.schedule();
        }
    }
}
pub fn bind(
    window: &AppWindow,
    local: LocalUi,
    store: StoreHandle,
) -> Result<RecoveryUi, slint::EventLoopError> {
    let state = Rc::new(State {
        window: window.as_weak(),
        local,
        store,
        selected: RefCell::new(None),
        rows: RefCell::new(Vec::new()),
        drafts: RefCell::new(HashMap::new()),
        pending_drafts: RefCell::new(HashMap::new()),
        draft_generation: Cell::new(0),
        draft_writing: Cell::new(false),
        draft_timer: slint::Timer::default(),
        draft_updates: tokio::sync::watch::channel(()).0,
        epoch: Cell::new(0),
        loading: Cell::new(false),
        again: Cell::new(false),
        timer: slint::Timer::default(),
    });
    {
        let state = state.clone();
        window.on_show_recovery(move || {
            state.selected.borrow_mut().take();
            if let Some(window) = state.window.upgrade() {
                window.set_recovery_selected(false);
            }
            state.local.dispatch(AppAction::Navigate(View::Recovery));
            state.schedule();
        });
    }
    {
        let state = state.clone();
        window.on_refresh_recovery(move || state.schedule());
    }
    {
        let state = state.clone();
        window.on_recovery_section_changed(move || {
            state.epoch.set(state.epoch.get().wrapping_add(1));
            state.selected.borrow_mut().take();
            if let Some(window) = state.window.upgrade() {
                window.set_recovery_selected(false);
                window.set_recovery_message("".into());
            }
            state.schedule();
        });
    }
    {
        let state = state.clone();
        window.on_recovery_select(move |id| {
            if let Ok(id) = uuid::Uuid::parse_str(&id) {
                state.select(id);
            }
        });
    }
    {
        let state = state.clone();
        window.on_recovery_merge_edited(move || {
            if let (Some(RecoveryItem::Conflict(record)), Some(window)) =
                (state.selected.borrow().as_ref(), state.window.upgrade())
            {
                state.edit_draft(record.key.clone(), window.get_recovery_merge().to_string());
            }
        });
    }
    {
        let state = state.clone();
        window.on_recovery_reload(move || state.reload());
    }
    {
        let state = state.clone();
        window.on_recovery_apply(move |choice| {
            let Some(window) = state.window.upgrade() else {
                return;
            };
            if window.get_recovery_stale() {
                return;
            }
            let Some(RecoveryItem::Conflict(record)) = state.selected.borrow().as_ref().cloned()
            else {
                return;
            };
            let choice = match choice.as_str() {
                "remote" => ResolutionChoice::AcceptRemote,
                "local" => ResolutionChoice::KeepLocal,
                "merge" => ResolutionChoice::Merge(window.get_recovery_merge().to_string()),
                _ => return,
            };
            state.begin(AppAction::ResolveConflict(ConflictResolution {
                conflict_id: record.id,
                expected_local: record.local.local_revision,
                choice,
            }));
        });
    }
    {
        let state = state.clone();
        window.on_recovery_restore(move || {
            let Some(window) = state.window.upgrade() else {
                return;
            };
            let raw = window.get_recovery_destination();
            let destination = if raw.is_empty() {
                None
            } else {
                match PagePath::parse(&raw) {
                    Ok(path) => Some(path),
                    Err(_) => {
                        state.message("Enter a relative Markdown page path.");
                        return;
                    }
                }
            };
            let action = match state.selected.borrow().as_ref() {
                Some(RecoveryItem::Trash(record)) => {
                    AppAction::RestoreTrash(record.id, destination)
                }
                Some(RecoveryItem::Copy(record)) => {
                    AppAction::RestoreRecovery(record.id, destination)
                }
                _ => return,
            };
            state.begin(action);
        });
    }
    {
        let state = state.clone();
        window.on_recovery_purge(move || {
            let id = match state.selected.borrow().as_ref() {
                Some(RecoveryItem::Trash(record)) => record.id,
                _ => return,
            };
            state.begin(AppAction::EmptyTrash(vec![id]));
        });
    }
    {
        let state = state.clone();
        window.on_recovery_completed(move |success, message| {
            if let Some(window) = state.window.upgrade() {
                window.set_recovery_busy(false);
            }
            state.message(&message);
            if success {
                if let Some(selected) = state.selected.borrow_mut().take() {
                    state.drafts.borrow_mut().remove(item_key(&selected));
                    state
                        .pending_drafts
                        .borrow_mut()
                        .remove(item_key(&selected));
                }
                if let Some(window) = state.window.upgrade() {
                    window.set_recovery_selected(false);
                }
            } else if let Some(window) = state.window.upgrade() {
                window.set_recovery_stale(true);
            }
            state.draft_updates.send_replace(());
            state.schedule();
        });
    }
    {
        let state = state.clone();
        window.on_trash_current_note(move || {
            let local = state.local.clone();
            let weak = Rc::downgrade(&state);
            let connection = local.connection_id();
            if slint::spawn_local(async move {
                let result = local.flush().await;
                let Some(state) = weak.upgrade() else {
                    return;
                };
                if result.is_err() || local.connection_id() != connection {
                    state.message("Save or copy your latest text before moving the note to Trash.");
                    return;
                }
                if let (Some(key), Some(revision)) = (local.current_key(), local.current_revision())
                {
                    local.dispatch(AppAction::Navigate(View::Recovery));
                    if let Some(window) = state.window.upgrade() {
                        window.set_recovery_section("trash".into());
                    }
                    state.selected.borrow_mut().take();
                    state.begin(AppAction::MoveToTrash(key, revision));
                }
            })
            .is_err()
            {
                state.message("Trash action could not start. Try again.");
            }
        });
    }
    Ok(RecoveryUi(state))
}
