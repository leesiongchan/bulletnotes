//! Composition of the durable local app and its public macOS lifecycle adapter.
use super::{
    appearance_ui::{self, AppearanceUi},
    coordinator::AppAction,
    local_ui::{self, LocalUi},
    startup::Startup,
    window_placement::{self, ScreenFrame},
};
use crate::{
    AppWindow, LocalTray,
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
    window: AppWindow,
    tray: LocalTray,
    local: LocalUi,
    native: Rc<NativeApplication>,
    _appearance: AppearanceUi,
    _recovery: super::recovery_ui::RecoveryUi,
    _chrome: Rc<RefCell<Option<ChromeMonitor>>>,
}
impl LocalApplication {
    pub fn new(startup: Startup, mut native: NativeApplication) -> AppResult<Self> {
        let window = AppWindow::new()?;
        let tray = LocalTray::new()?;
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
            let native = native.clone();
            let local = local.clone();
            tray.on_reopen(move || {
                if let Some(window) = weak.upgrade() {
                    match window.show() {
                        Ok(()) => {
                            local.set_active(true);
                            native.activate();
                            if let Ok(native) = native_window(window.window()) {
                                native.makeKeyAndOrderFront(None);
                            }
                        }
                        Err(_) => {
                            window.set_status_text("Window could not be shown. Try again.".into())
                        }
                    }
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
        let chrome = Rc::new(RefCell::new(None));
        {
            let owner = chrome.clone();
            let weak = window.as_weak();
            let local = local.clone();
            let mut initial = Some((placement, startup.last_page));
            window.on_native_window_ready(move || {
                let Some((placement, last_page)) = initial.take() else {
                    return;
                };
                if let Some(window) = weak.upgrade() {
                    let result = (|| -> AppResult<()> {
                        *owner.borrow_mut() = Some(ChromeMonitor::install(&window, None)?);
                        restore_window(&window, placement.as_ref())?;
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
            window,
            tray,
            local,
            native,
            _appearance: appearance,
            _recovery: recovery,
            _chrome: chrome,
        })
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
        self.window.show()?;
        self.local.set_active(true);
        self.native.activate();
        if let Ok(native) = native_window(self.window.window()) {
            native.makeKeyAndOrderFront(None);
        }
        Ok(())
    }
    pub fn run(&self) -> AppResult<()> {
        self.tray.show()?;
        self.show()?;
        slint::run_event_loop_until_quit()?;
        Ok(())
    }
}
fn capture(window: &AppWindow) -> AppResult<WindowPlacement> {
    let frame = native_window(window.window())?.frame();
    Ok(WindowPlacement {
        x: frame.origin.x,
        y: frame.origin.y,
        width: frame.size.width,
        height: frame.size.height,
        pinned: window.get_is_pinned(),
    })
}
fn restore_window(window: &AppWindow, saved: Option<&WindowPlacement>) -> AppResult<()> {
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
