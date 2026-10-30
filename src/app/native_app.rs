//! Composition of the durable local app and its desktop lifecycle adapters.
use super::{
    appearance_ui::{self, AppearanceUi},
    coordinator::AppAction,
    lifecycle::{CloseIntent, close_intent},
    local_ui::{self, LocalUi},
    startup::Startup,
};
use crate::{AboutWindow, AppWindow, LocalTray, platform::application::NativeApplication};
#[cfg(target_os = "macos")]
use crate::{
    app::window_placement::{self, ScreenFrame},
    platform::window_chrome::{ChromeMonitor, native_window},
    store::settings::WindowPlacement,
};
#[cfg(target_os = "macos")]
use objc2::MainThreadMarker;
#[cfg(target_os = "macos")]
use objc2_app_kit::NSScreen;
#[cfg(target_os = "macos")]
use objc2_foundation::{NSPoint, NSRect, NSSize};
use slint::ComponentHandle;
use std::{cell::RefCell, rc::Rc};
type AppResult<T> = Result<T, Box<dyn std::error::Error>>;

pub struct LocalApplication {
    window: AppWindow,
    tray: LocalTray,
    local: LocalUi,
    integration: super::native_ui::NativeUi,
    _appearance: AppearanceUi,
    _recovery: super::recovery_ui::RecoveryUi,
    #[cfg(target_os = "macos")]
    _chrome: Rc<RefCell<Option<ChromeMonitor>>>,
}
impl LocalApplication {
    pub fn new(startup: Startup, mut native: NativeApplication) -> AppResult<Self> {
        crate::platform::renderer::select()?;
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
            #[cfg(target_os = "macos")]
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
                    let about = about.as_ref().ok_or("About is unavailable")?;
                    let source = weak.upgrade().ok_or("Main window is unavailable")?;
                    super::ui::present_about(about, &source);
                    about.show()?;
                    #[cfg(target_os = "macos")]
                    native.activate();
                    #[cfg(not(target_os = "macos"))]
                    {
                        use slint::winit_030::WinitWindowAccessor;
                        about
                            .window()
                            .with_winit_window(|window| window.focus_window());
                    }
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
        bind_lifecycle(&window, local.clone(), appearance.clone(), recovery.clone());
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
        {
            let weak = window.as_weak();
            tray.on_export_diagnostics(move || {
                if let Some(window) = weak.upgrade() {
                    window.invoke_export_diagnostics();
                }
            });
        }
        #[cfg(target_os = "macos")]
        let chrome = Rc::new(RefCell::new(None));
        {
            #[cfg(target_os = "macos")]
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
                        #[cfg(target_os = "macos")]
                        {
                            *owner.borrow_mut() = Some(ChromeMonitor::install(&window, None)?);
                        }
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
            if !cfg!(target_os = "macos") && crate::platform::desktop::software_renderer() {
                // The software renderer has no rendering notifier. A queued timer
                // runs after show() has created the native desktop window.
                slint::Timer::single_shot(std::time::Duration::from_millis(16), move || {
                    if let Some(window) = weak.upgrade() {
                        window.invoke_native_window_ready();
                    }
                });
            } else {
                let mut notified = false;
                window.window().set_rendering_notifier(move |state, _| {
                    if matches!(state, slint::RenderingState::AfterRendering) && !notified {
                        notified = true;
                        // Queue UI work after rendering; Slint APIs must not be reentered here.
                        let _ = weak
                            .upgrade_in_event_loop(|window| window.invoke_native_window_ready());
                    }
                })?;
            }
        }
        Ok(Self {
            window,
            tray,
            local,
            integration,
            _appearance: appearance,
            _recovery: recovery,
            #[cfg(target_os = "macos")]
            _chrome: chrome,
        })
    }
    pub fn window(&self) -> &AppWindow {
        &self.window
    }
    pub fn local(&self) -> &LocalUi {
        &self.local
    }
    pub fn show(&self) -> AppResult<()> {
        self.integration.show()?;
        Ok(())
    }
    pub fn run(&self, runtime: &tokio::runtime::Runtime) -> AppResult<()> {
        // Slint polls local reconciliation futures on the main thread outside Tokio's executor.
        let _entered = runtime.enter();
        #[cfg(target_os = "macos")]
        self.tray.show()?;
        #[cfg(not(target_os = "macos"))]
        {
            let tray_available = self.tray.show().is_ok();
            self.window
                .set_can_hide_to_tray(tray_available && cfg!(target_os = "windows"));
            if !tray_available {
                crate::diagnostics::record(
                    crate::diagnostics::DiagnosticEvent::DesktopCapabilityUnavailable {
                        capability: crate::diagnostics::DesktopCapability::Tray,
                    },
                );
                self.window.set_native_message(
                    "Tray unavailable. Closing exits Bulletnotes after saving.".into(),
                );
                self.window.set_native_warning(true);
            }
        }
        self.show()?;
        slint::run_event_loop_until_quit()?;
        Ok(())
    }
}
#[cfg(target_os = "macos")]
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
#[cfg(target_os = "macos")]
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

#[cfg(not(target_os = "macos"))]
pub(super) use crate::platform::desktop::{capture, restore_window};

fn bind_lifecycle(
    window: &AppWindow,
    local: LocalUi,
    appearance: AppearanceUi,
    recovery: super::recovery_ui::RecoveryUi,
) {
    for intent in [CloseIntent::Hide, CloseIntent::Quit] {
        let weak = window.as_weak();
        let local = local.clone();
        let appearance = appearance.clone();
        let recovery = recovery.clone();
        let action = move || {
            let intent = weak.upgrade().map_or(intent, |window| {
                close_intent(intent, window.get_can_hide_to_tray())
            });
            if !local.prepare_close(intent) {
                return;
            }
            let task = prepare_native_close(
                weak.clone(),
                local.clone(),
                appearance.clone(),
                recovery.clone(),
                intent,
            );
            if slint::spawn_local(task).is_err() {
                local.cancel_close();
            }
        };
        match intent {
            CloseIntent::Hide => window.on_request_hide(action),
            CloseIntent::Quit => window.on_request_quit(action),
        }
    }
}

async fn prepare_native_close(
    weak: slint::Weak<AppWindow>,
    local: LocalUi,
    appearance: AppearanceUi,
    recovery: super::recovery_ui::RecoveryUi,
    intent: CloseIntent,
) {
    let Some(window) = weak.upgrade() else {
        local.cancel_close();
        return;
    };
    if recovery.flush().await.is_err() {
        local.cancel_close();
        let _ = window.show();
        window.set_status_text(
            "Merged draft could not be saved. Keep it available to copy, or retry closing.".into(),
        );
        return;
    }
    let metadata = match capture(&window) {
        Ok(placement) => appearance.save_window(placement).await.map_err(|_| ()),
        Err(_) => Err(()),
    };
    if metadata.is_ok() {
        local.dispatch(match intent {
            CloseIntent::Hide => AppAction::RequestHide,
            CloseIntent::Quit => AppAction::RequestQuit,
        });
    } else {
        local.cancel_close();
        let _ = window.show();
        window.set_status_text(
            "Window settings could not be saved. Your note is still here; try again.".into(),
        );
    }
}
