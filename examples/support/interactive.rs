//! Disposable interactive native gate. No note persistence or HTTP client.
use bulletnotes::{AppWindow, NativePreviewTray};
use global_hotkey::{
    GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState,
    hotkey::{Code, HotKey, Modifiers},
};
use objc2::{MainThreadMarker, rc::Retained};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationOptions, NSRunningApplication, NSView, NSWindow,
    NSWorkspace,
};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use slint::ComponentHandle;
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

type ProbeResult<T> = Result<T, Box<dyn std::error::Error>>;

fn native_window(window: &slint::Window) -> ProbeResult<Retained<NSWindow>> {
    MainThreadMarker::new().ok_or("native preview requires main thread")?;
    let provider = window.window_handle();
    let handle = provider.window_handle()?;
    let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
        return Err("AppKit window required".into());
    };
    // SAFETY: the live Slint window owns this NSView, and the public handle provider
    // outlives the borrow. Access stays on the main thread; window() retains its result.
    let view = unsafe { handle.ns_view.cast::<NSView>().as_ref() };
    view.window()
        .ok_or_else(|| "native window not available".into())
}

struct FocusState {
    previous: Option<Retained<NSRunningApplication>>,
    current: Retained<NSRunningApplication>,
    mtm: MainThreadMarker,
}

impl FocusState {
    fn new(mtm: MainThreadMarker) -> Self {
        let mut state = Self {
            previous: None,
            current: NSRunningApplication::currentApplication(),
            mtm,
        };
        state.remember_previous();
        state
    }

    fn remember_previous(&mut self) {
        if !self.current.isActive() {
            self.previous = NSWorkspace::sharedWorkspace()
                .frontmostApplication()
                .filter(|app| app != &self.current);
        }
    }

    fn show(&mut self, window: &AppWindow) -> ProbeResult<()> {
        self.remember_previous();
        window.show()?;
        let weak = window.as_weak();
        let mtm = self.mtm;
        slint::Timer::single_shot(Duration::from_millis(100), move || {
            if let Some(window) = weak.upgrade() {
                match native_window(window.window()) {
                    Ok(native) => {
                        NSApplication::sharedApplication(mtm).activate();
                        native.makeKeyAndOrderFront(None);
                        window.invoke_focus_editor();
                    }
                    Err(error) => {
                        window.set_probe_status(format!("Activation unavailable: {error}").into())
                    }
                }
            }
        });
        Ok(())
    }

    fn hide(&mut self, window: &AppWindow) -> ProbeResult<()> {
        let was_active = self.current.isActive();
        window.hide()?;
        if was_active
            && let Some(previous) = self.previous.as_ref().filter(|app| !app.isTerminated())
        {
            let app = NSApplication::sharedApplication(self.mtm);
            app.yieldActivationToApplication(previous);
            if !previous.activateFromApplication_options(
                &self.current,
                NSApplicationActivationOptions::ActivateAllWindows,
            ) {
                window.set_probe_status(
                    "Previous-app activation was declined; text remains in the preview.".into(),
                );
            }
        }
        Ok(())
    }
}

struct HotkeyGuard {
    manager: GlobalHotKeyManager,
    key: HotKey,
}

impl HotkeyGuard {
    fn register(window: &AppWindow) -> ProbeResult<Self> {
        let manager = GlobalHotKeyManager::new()?;
        let key = HotKey::new(Some(Modifiers::ALT), Code::KeyA);
        manager.register(key)?;
        let id = key.id();
        let weak = window.as_weak();
        let down = Arc::new(AtomicBool::new(false));
        GlobalHotKeyEvent::set_event_handler(Some(move |event: GlobalHotKeyEvent| {
            if event.id != id {
                return;
            }
            match event.state {
                HotKeyState::Released => {
                    down.store(false, Ordering::Relaxed);
                }
                HotKeyState::Pressed if !down.swap(true, Ordering::Relaxed) => {
                    // Only the weak handle crosses the library event callback boundary.
                    // Its component and all AppKit state are accessed on Slint's UI thread.
                    let _ = weak.upgrade_in_event_loop(|window| window.invoke_toggle_activation());
                }
                HotKeyState::Pressed => {}
            }
        }));
        Ok(Self { manager, key })
    }
}

impl Drop for HotkeyGuard {
    fn drop(&mut self) {
        if let Err(error) = self.manager.unregister(self.key) {
            eprintln!("preview hotkey teardown: {error}");
        }
    }
}

pub fn run() -> ProbeResult<()> {
    let mtm = MainThreadMarker::new().ok_or("native preview requires main thread")?;
    let focus = Rc::new(RefCell::new(FocusState::new(mtm)));
    let window = AppWindow::new()?;
    let tray = NativePreviewTray::new()?;

    let weak = window.as_weak();
    let state = focus.clone();
    window.on_request_hide(move || {
        if let Some(window) = weak.upgrade()
            && let Err(error) = state.borrow_mut().hide(&window)
        {
            window.set_probe_status(format!("Hide failed: {error}").into());
        }
    });
    let weak = window.as_weak();
    let state = focus.clone();
    window.on_toggle_activation(move || {
        if let Some(window) = weak.upgrade() {
            let result = if window.window().is_visible()
                && NSRunningApplication::currentApplication().isActive()
            {
                state.borrow_mut().hide(&window)
            } else {
                state.borrow_mut().show(&window)
            };
            if let Err(error) = result {
                window.set_probe_status(format!("Activation failed: {error}").into());
            }
        }
    });
    let weak = window.as_weak();
    let state = focus.clone();
    tray.on_reopen(move || {
        if let Some(window) = weak.upgrade()
            && let Err(error) = state.borrow_mut().show(&window)
        {
            window.set_probe_status(format!("Reopen failed: {error}").into());
        }
    });
    let weak = window.as_weak();
    window.window().on_close_requested(move || {
        if let Some(window) = weak.upgrade() {
            window.invoke_request_hide();
        }
        slint::CloseRequestResponse::KeepWindowShown
    });
    window.on_request_quit(|| {
        let _ = slint::quit_event_loop();
    });
    tray.on_quit(|| {
        let _ = slint::quit_event_loop();
    });
    let _hotkey = match HotkeyGuard::register(&window) {
        Ok(guard) => {
            window.set_probe_status(
                "Temporary text · Option+A toggles · BN test menu reopens".into(),
            );
            println!("preview_hotkey_registered: true");
            Some(guard)
        }
        Err(error) => {
            eprintln!("preview hotkey unavailable: {error}");
            window.set_probe_status(
                "Option+A unavailable; use the BN test menu. Temporary text is not saved.".into(),
            );
            println!("preview_hotkey_registered: false");
            None
        }
    };
    // Keep tray, component and hotkey owner alive throughout the main-thread event loop.
    tray.show()?;
    focus.borrow_mut().show(&window)?;
    if std::env::args().any(|arg| arg == "--auto-close") {
        slint::Timer::single_shot(Duration::from_secs(3), || {
            let _ = slint::quit_event_loop();
        });
    }
    slint::run_event_loop()?;
    Ok(())
}
