//! Native appearance actions with a bounded, serialized durable-settings queue.
use super::{
    appearance::{AppearanceCommit, AppearanceController, MetadataChange},
    ui::present_settings,
};
use crate::{
    AppWindow,
    domain::error::StoreError,
    store::{StoreHandle, settings::AppSettings},
    theme::{
        AppearanceMode, PaperIntensity, PaperStyle, PaperType, SystemAppearance, Theme, ThemeError,
        ThemeId,
    },
};
use slint::ComponentHandle;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use tokio::sync::{mpsc, oneshot};

enum Write {
    Settings(Box<AppearanceCommit>),
    Metadata(MetadataChange, oneshot::Sender<Result<(), StoreError>>),
    Barrier(oneshot::Sender<()>),
}
enum Pending {
    Import(Theme),
    Selection(ThemeId),
}
struct State {
    controller: RefCell<AppearanceController>,
    system: Cell<SystemAppearance>,
    sender: mpsc::Sender<Write>,
    window: slint::Weak<AppWindow>,
    pending: RefCell<Option<Pending>>,
    dialog_open: Cell<bool>,
}
#[derive(Clone)]
pub struct AppearanceUi(Rc<State>);
impl AppearanceUi {
    /// Await every preceding appearance write, including its UI acknowledgement.
    pub async fn flush(&self) -> Result<(), StoreError> {
        let (sender, receiver) = oneshot::channel();
        self.0
            .sender
            .send(Write::Barrier(sender))
            .await
            .map_err(|_| StoreError::Closed)?;
        receiver.await.map_err(|_| StoreError::Closed)
    }
    /// Metadata patches never inherit an unacknowledged appearance/shortcut preview.
    async fn save_metadata(&self, change: MetadataChange) -> Result<(), StoreError> {
        let (sender, receiver) = oneshot::channel();
        self.0
            .sender
            .try_send(Write::Metadata(change, sender))
            .map_err(|_| StoreError::Closed)?;
        receiver.await.map_err(|_| StoreError::Closed)?
    }
    pub async fn save_search_view(
        &self,
        view: crate::store::settings::SearchView,
    ) -> Result<(), StoreError> {
        self.save_metadata(MetadataChange::SearchView(view)).await
    }
    pub async fn save_window(
        &self,
        placement: crate::store::settings::WindowPlacement,
    ) -> Result<(), StoreError> {
        self.save_metadata(MetadataChange::Window(placement)).await
    }
    pub async fn save_hotkey(
        &self,
        hotkey: crate::store::settings::HotkeySettings,
    ) -> Result<(), StoreError> {
        self.save_metadata(MetadataChange::Hotkey(hotkey)).await
    }
    pub async fn save_connection(
        &self,
        connection: crate::domain::connection::ConnectionId,
    ) -> Result<(), StoreError> {
        self.save_metadata(MetadataChange::Connection(connection))
            .await
    }
    pub fn set_system_appearance(&self, system: SystemAppearance) {
        self.0.system.set(system);
        self.0.controller.borrow_mut().set_system_appearance(system);
        self.0.present();
    }
}
impl State {
    fn message(&self, message: &str) {
        if let Some(window) = self.window.upgrade() {
            window.set_appearance_message(message.into());
        }
    }
    fn present(&self) {
        if let Some(window) = self.window.upgrade()
            && let Err(error) = present_settings(
                &window,
                self.controller.borrow().displayed(),
                self.system.get(),
            )
        {
            self.message(&error.to_string());
        }
    }
    fn clear_pending(&self) {
        self.pending.borrow_mut().take();
        if let Some(window) = self.window.upgrade() {
            window.set_low_contrast_warning(false);
        }
    }
    fn submit(&self, candidate: AppSettings, allow: bool) {
        let ticket = self
            .controller
            .borrow_mut()
            .begin_settings_commit(candidate, allow);
        match ticket {
            Ok(ticket) => {
                match self.sender.try_send(Write::Settings(Box::new(ticket))) {
                    Ok(()) => self.message("Saving settings…"),
                    Err(error) => {
                        if let Write::Settings(ticket) = error.into_inner() {
                            let _ = self
                                .controller
                                .borrow_mut()
                                .complete_settings_commit(*ticket, Err(StoreError::Closed));
                        }
                        self.message("Settings could not be saved. Try again.");
                    }
                }
                self.present();
            }
            Err(error) => self.message(&error.to_string()),
        }
    }
    fn choose(&self, pending: Pending, allow: bool) {
        let candidate = match &pending {
            Pending::Import(theme) => self
                .controller
                .borrow_mut()
                .confirm_import(theme.clone(), allow),
            Pending::Selection(id) => self
                .controller
                .borrow()
                .selection_candidate(id.clone(), allow),
        };
        match candidate {
            Ok(candidate) => {
                self.clear_pending();
                self.submit(candidate, allow);
            }
            Err(ThemeError::LowContrast) => {
                *self.pending.borrow_mut() = Some(pending);
                if let Some(window) = self.window.upgrade() {
                    window.set_low_contrast_warning(true);
                }
                self.message("This theme has insufficient text or control contrast. Apply anyway?");
            }
            Err(error) => self.message(&error.to_string()),
        }
    }
}
#[derive(Debug, thiserror::Error)]
pub enum AppearanceUiError {
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error(transparent)]
    EventLoop(#[from] slint::EventLoopError),
}
pub fn bind(
    window: &AppWindow,
    store: StoreHandle,
    settings: AppSettings,
    system: SystemAppearance,
) -> Result<AppearanceUi, AppearanceUiError> {
    let controller = AppearanceController::new(settings, system)?;
    let (sender, mut receiver) = mpsc::channel(32);
    let state = Rc::new(State {
        controller: RefCell::new(controller),
        system: Cell::new(system),
        sender,
        window: window.as_weak(),
        pending: RefCell::new(None),
        dialog_open: Cell::new(false),
    });
    state.present();
    // The writer owns no sender or strong state reference; dropping the UI closes it.
    let weak = Rc::downgrade(&state);
    slint::spawn_local(async move {
        while let Some(write) = receiver.recv().await {
            let Some(state) = weak.upgrade() else {
                break;
            };
            match write {
                Write::Settings(mut ticket) => {
                    ticket.rebase_metadata(state.controller.borrow().committed());
                    let result = store.save_settings(ticket.settings().clone()).await;
                    let result = state
                        .controller
                        .borrow_mut()
                        .complete_settings_commit(*ticket, result);
                    state.present();
                    state.message(if result.is_ok() {
                        "Settings saved"
                    } else {
                        "Settings could not be saved. Try changing the setting again."
                    });
                }
                Write::Metadata(change, reply) => {
                    let mut candidate = state.controller.borrow().committed().clone();
                    change.apply(&mut candidate);
                    let result = store.save_settings(candidate).await;
                    if result.is_ok() {
                        state.controller.borrow_mut().acknowledge_metadata(&change);
                    }
                    state.present();
                    let _ = reply.send(result);
                }
                Write::Barrier(reply) => {
                    let _ = reply.send(());
                }
            }
        }
    })?;
    {
        let state = state.clone();
        window.on_theme_selected(move |id| {
            // Choosing an existing theme accepts its palette; only new imports need review.
            state.choose(Pending::Selection(ThemeId(id.to_string())), true);
        });
    }
    {
        let state = state.clone();
        window.on_theme_assigned(move |id, dark| {
            let slot = if dark {
                SystemAppearance::Dark
            } else {
                SystemAppearance::Light
            };

            let candidate = state
                .controller
                .borrow()
                .assignment_candidate(ThemeId(id.to_string()), slot);
            match candidate {
                Ok(candidate) => {
                    state.clear_pending();
                    state.submit(candidate, true);
                }
                Err(error) => state.message(&error.to_string()),
            }
        });
    }
    {
        let state = state.clone();
        window.on_cycle_theme_assignment(move |id| {
            let candidate = state
                .controller
                .borrow()
                .cycle_assignment_candidate(ThemeId(id.to_string()));
            match candidate {
                Ok(candidate) => {
                    state.clear_pending();
                    state.submit(candidate, true);
                }
                Err(error) => state.message(&error.to_string()),
            }
        });
    }
    {
        let state = state.clone();
        window.on_appearance_changed(move || {
            if let Some(window) = state.window.upgrade() {
                let mut candidate = state.controller.borrow().displayed().clone();
                candidate.appearance.mode = match window.get_appearance_mode().as_str() {
                    "light" => AppearanceMode::Light,
                    "dark" => AppearanceMode::Dark,
                    _ => AppearanceMode::System,
                };
                candidate.appearance.paper = match window.get_paper_style().as_str() {
                    "dots" => PaperStyle::Dots,
                    "ruled" => PaperStyle::Ruled,
                    "grid" => PaperStyle::Grid,
                    _ => PaperStyle::Plain,
                };
                candidate.appearance.font_size = window.get_font_size_px();
                candidate.appearance.paper_type = match window.get_paper_type().as_str() {
                    "smooth" => PaperType::Smooth,
                    "bone" => PaperType::Bone,
                    "eggshell" => PaperType::Eggshell,
                    "linen" => PaperType::Linen,
                    _ => PaperType::Smooth,
                };
                candidate.appearance.paper_intensity = match window.get_paper_intensity().as_str() {
                    "subtle" => PaperIntensity::Subtle,
                    "bold" => PaperIntensity::Bold,
                    _ => PaperIntensity::Clear,
                };
                candidate.start_with_new_note = window.get_start_with_new_note();
                candidate.appearance.keep_bottom_bar_visible = window.get_keep_bottom_bar_visible();
                candidate.appearance.show_note_count = window.get_show_note_count();
                candidate.appearance.show_note_date = window.get_show_note_date();
                candidate.appearance.show_newest_indicator = window.get_show_newest_indicator();
                state.clear_pending();
                state.submit(candidate, true);
            }
        });
    }
    {
        let state = state.clone();
        window.on_apply_low_contrast(move || {
            let pending = state.pending.borrow_mut().take();
            if let Some(pending) = pending {
                state.choose(pending, true);
            }
        });
    }
    {
        let state = state.clone();
        window.on_cancel_theme_import(move || {
            state.clear_pending();
            state.message("Theme import cancelled");
        });
    }
    bind_dialogs(window, &state);
    Ok(AppearanceUi(state))
}

async fn blocking_file<T: Send + 'static>(
    job: impl FnOnce() -> Result<T, super::theme_files::ThemeFileError> + Send + 'static,
) -> Result<T, super::theme_files::ThemeFileError> {
    let (sender, receiver) = oneshot::channel();
    std::thread::Builder::new()
        .name("bulletnotes-theme-file".into())
        .spawn(move || {
            let _ = sender.send(job());
        })?;
    receiver
        .await
        .map_err(|_| std::io::Error::other("theme file worker stopped"))?
}
fn bind_dialogs(window: &AppWindow, state: &Rc<State>) {
    {
        let state = state.clone();
        window.on_import_theme(move || {
            if state.dialog_open.replace(true) {
                return;
            }
            let task = state.clone();
            if slint::spawn_local(async move {
                let file = rfd::AsyncFileDialog::new()
                    .set_title("Import Bulletnotes theme")
                    .add_filter("Bulletnotes theme", &["json"])
                    .pick_file()
                    .await;
                if let Some(file) = file {
                    let path = file.path().to_owned();
                    match blocking_file(move || super::theme_files::load_theme_file(&path)).await {
                        Ok(theme) => task.choose(Pending::Import(theme), false),
                        Err(error) => task.message(&error.to_string()),
                    }
                }
                task.dialog_open.set(false);
            })
            .is_err()
            {
                state.dialog_open.set(false);
                state.message("The theme dialog could not be opened. Try again.");
            }
        });
    }
    {
        let state = state.clone();
        window.on_export_theme(move || {
            if state.dialog_open.replace(true) {
                return;
            }
            let palette = match state.controller.borrow().resolved() {
                Ok(resolved) => resolved.palette,
                Err(error) => {
                    state.dialog_open.set(false);
                    state.message(&error.to_string());
                    return;
                }
            };
            let task = state.clone();
            if slint::spawn_local(async move {
                // Theme names are untrusted data, never default filenames or paths.
                let file = rfd::AsyncFileDialog::new()
                    .set_title("Export Bulletnotes theme")
                    .set_file_name("Bulletnotes Theme.json")
                    .add_filter("Bulletnotes theme", &["json"])
                    .save_file()
                    .await;
                if let Some(file) = file {
                    let path = file.path().to_owned();
                    match blocking_file(move || {
                        super::theme_files::save_theme_file(&path, &palette)
                    })
                    .await
                    {
                        Ok(()) => task.message("Theme exported"),
                        Err(error) => task.message(&error.to_string()),
                    }
                }
                task.dialog_open.set(false);
            })
            .is_err()
            {
                state.dialog_open.set(false);
                state.message("The theme dialog could not be opened. Try again.");
            }
        });
    }
}
