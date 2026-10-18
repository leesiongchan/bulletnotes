//! Composition of the durable local app and its public macOS lifecycle adapter.
use super::{
    appearance_ui::{self, AppearanceUi},
    coordinator::AppAction,
    local_ui::{self, LocalUi},
    startup::Startup,
    window_placement::{self, ScreenFrame},
};
use crate::{
    AboutWindow, AppWindow, LocalTray,
    platform::{
        application::NativeApplication,
        window_chrome::{ChromeMonitor, native_window},
    },
    store::settings::WindowPlacement,
};
use objc2::MainThreadMarker;
use objc2_app_kit::NSScreen;
use objc2_foundation::{NSPoint, NSRect, NSSize};
use slint::ComponentHandle;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
type AppResult<T> = Result<T, Box<dyn std::error::Error>>;

pub struct LocalApplication {
    frames: std::sync::Arc<std::sync::atomic::AtomicU64>,
    window: AppWindow,
    tray: LocalTray,
    local: LocalUi,
    native: Rc<NativeApplication>,
    integration: super::native_ui::NativeUi,
    _appearance: AppearanceUi,
    _recovery: super::recovery_ui::RecoveryUi,
    _chrome: Rc<RefCell<Option<ChromeMonitor>>>,
}
impl LocalApplication {
    pub fn new(startup: Startup, mut native: NativeApplication) -> AppResult<Self> {
        let window = AppWindow::new()?;
        let tray = LocalTray::new()?;
        let settings = startup.settings.clone();
        let placement = startup.settings.window.clone();
        let appearance = appearance_ui::bind(
            &window,
            startup.store.clone(),
            startup.settings,
            native.system_appearance(),
        )?;
        let local = local_ui::bind(
            &window,
            startup.store.clone(),
            startup.connection,
            appearance.clone(),
        )?;
        let recovery = super::recovery_ui::bind(&window, local.clone(), startup.store)?;
        native.bind_quit(&window);
        let native = Rc::new(native);
        let integration = super::native_ui::bind(
            &window,
            local.clone(),
            appearance.clone(),
            native.clone(),
            &settings,
        );
        {
            let about = Rc::new(RefCell::new(None::<AboutWindow>));
            let native = native.clone();
            let weak = window.as_weak();
            window.on_show_about(move || {
                let result = (|| -> AppResult<()> {
                    let mut about = about.borrow_mut();
                    if about.is_none() {
                        let window = AboutWindow::new()?;
                        window.set_app_version(
                            concat!("Version ", env!("CARGO_PKG_VERSION"), " · development").into(),
                        );
                        *about = Some(window);
                    }
                    about.as_ref().ok_or("About is unavailable")?.show()?;
                    native.activate();
                    Ok(())
                })();
                if result.is_err()
                    && let Some(window) = weak.upgrade()
                {
                    window.set_native_message("About could not be shown. Try again.".into());
                    window.set_native_warning(true);
                }
            });
            let weak = window.as_weak();
            tray.on_show_about(move || {
                if let Some(window) = weak.upgrade() {
                    window.invoke_show_about();
                }
            });
        }
        let transition_pending = Rc::new(Cell::new(false));
        for quit in [false, true] {
            let weak = window.as_weak();
            let local = local.clone();
            let appearance = appearance.clone();
            let recovery = recovery.clone();
            let pending = transition_pending.clone();
            let action = move || {
                if pending.replace(true) {
                    return;
                }
                let weak = weak.clone();
                let local = local.clone();
                let appearance = appearance.clone();
                let recovery = recovery.clone();
                let pending = pending.clone();
                let rollback = pending.clone();
                if slint::spawn_local(async move {
                    if let Some(window) = weak.upgrade() {
                        if recovery.flush().await.is_err(){
                            let _=window.show();window.set_status_text("Merged draft could not be saved. Keep it available to copy, or retry closing.".into());pending.set(false);return;
                        }
                        let metadata = match capture(&window) {
                            Ok(placement) => appearance.save_window(placement).await.map_err(|_| ()),
                            Err(_) => Err(()),
                        };
                        if metadata.is_ok() {
                            local.dispatch(if quit { AppAction::RequestQuit } else { AppAction::RequestHide });
                        } else {
                            let _ = window.show();
                            window.set_status_text("Window settings could not be saved. Your note is still here; try again.".into());
                        }
                    }
                    pending.set(false);
                }).is_err() { rollback.set(false); }
            };
            if quit {
                window.on_request_quit(action);
            } else {
                window.on_request_hide(action);
            }
        }
        {
            let weak = window.as_weak();
            let integration = integration.clone();
            tray.on_reopen(move || {
                if integration.show().is_err()
                    && let Some(window) = weak.upgrade()
                {
                    window.set_native_message("Window could not be shown. Try again.".into());
                    window.set_native_warning(true);
                }
            });
        }
        {
            let weak = window.as_weak();
            tray.on_quit(move || {
                if let Some(window) = weak.upgrade() {
                    window.invoke_request_quit();
                }
            });
        }
        let frames = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
        let rendered = frames.clone();
        {
            let weak = window.as_weak();
            tray.on_export_diagnostics(move || {
                if let Some(window) = weak.upgrade() {
                    window.invoke_export_diagnostics();
                }
            });
        }
        let chrome = Rc::new(RefCell::new(None));
        {
            let owner = chrome.clone();
            let weak = window.as_weak();
            let local = local.clone();
            let integration = integration.clone();
            let mut initial = Some((placement, startup.last_page));
            window.on_native_window_ready(move || {
                let Some((placement, last_page)) = initial.take() else {
                    return;
                };
                if let Some(window) = weak.upgrade() {
                    let result = (|| -> AppResult<()> {
                        *owner.borrow_mut() = Some(ChromeMonitor::install(&window, None)?);
                        restore_window(&window, placement.as_ref())?;
                        integration.attach(&settings)?;
                        Ok(())
                    })();
                    if result.is_err() {
                        window.set_status_text(
                            "Native window setup failed. Reopen Bulletnotes to try again.".into(),
                        );
                        return;
                    }
                    if let Some(key) = last_page {
                        local.dispatch(AppAction::OpenPage(key));
                    } else {
                        local.dispatch(AppAction::NewNote);
                    }
                    window.invoke_local_startup_ready();
                }
            });
            let weak = window.as_weak();
            let mut notified = false;
            window.window().set_rendering_notifier(move |state, _| {
                if matches!(state, slint::RenderingState::AfterRendering) {
                    rendered.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                }
                if matches!(state, slint::RenderingState::AfterRendering) && !notified {
                    notified = true;
                    // The first frame proves that the native window exists. Queue UI
                    // work after rendering; Slint APIs must not be reentered here.
                    let _ =
                        weak.upgrade_in_event_loop(|window| window.invoke_native_window_ready());
                }
            })?;
        }
        Ok(Self {
            frames,
            window,
            tray,
            local,
            native,
            integration,
            _appearance: appearance,
            _recovery: recovery,
            _chrome: chrome,
        })
    }
    /// Public rendering boundary for the native interaction harness; no editor internals.
    pub fn rendered_frame_count(&self) -> u64 {
        self.frames.load(std::sync::atomic::Ordering::Relaxed)
    }
    pub fn window(&self) -> &AppWindow {
        &self.window
    }
    pub fn appearance(&self) -> &AppearanceUi {
        &self._appearance
    }
    pub fn local(&self) -> &LocalUi {
        &self.local
    }
    pub fn request_native_termination(&self) {
        self.native.request_termination();
    }
    pub fn show(&self) -> AppResult<()> {
        self.integration.show()?;
        Ok(())
    }
    pub fn run(&self) -> AppResult<()> {
        self.tray.show()?;
        self.show()?;
        slint::run_event_loop_until_quit()?;
        Ok(())
    }
}
pub(super) fn capture(window: &AppWindow) -> AppResult<WindowPlacement> {
    let frame = native_window(window.window())?.frame();
    Ok(WindowPlacement {
        x: frame.origin.x,
        y: frame.origin.y,
        width: frame.size.width,
        height: frame.size.height,
        pinned: window.get_is_pinned(),
    })
}
pub(super) fn restore_window(window: &AppWindow, saved: Option<&WindowPlacement>) -> AppResult<()> {
    let mtm = MainThreadMarker::new().ok_or("window restoration requires main thread")?;
    let native = native_window(window.window())?;
    let screens: Vec<_> = NSScreen::screens(mtm)
        .iter()
        .map(|screen| {
            let frame = screen.visibleFrame();
            ScreenFrame {
                x: frame.origin.x,
                y: frame.origin.y,
                width: frame.size.width,
                height: frame.size.height,
            }
        })
        .collect();
    if let Some(saved) = saved.and_then(|saved| window_placement::restore(saved, &screens)) {
        native.setFrame_display(
            NSRect::new(
                NSPoint::new(saved.x, saved.y),
                NSSize::new(saved.width, saved.height),
            ),
            true,
        );
        window.set_is_pinned(saved.pinned);
    } else {
        native.center();
    }
    Ok(())
}
