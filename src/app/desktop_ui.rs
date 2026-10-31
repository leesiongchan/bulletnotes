//! Desktop events and shortcuts; durable transitions remain owned by LocalUi.
use super::{appearance_ui::AppearanceUi, local_ui::LocalUi};
use crate::{
    AppWindow,
    platform::{
        desktop,
        hotkey::{HotkeyBackend, HotkeyError, HotkeyRegistration},
    },
    store::settings::{AppSettings, HotkeySettings},
};
use global_hotkey::{
    GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState,
    hotkey::{HotKey, Modifiers},
};
use slint::{
    ComponentHandle,
    winit_030::{EventResult, WinitWindowAccessor, winit::event::WindowEvent},
};
use std::{cell::RefCell, rc::Rc, time::Duration};

struct Backend(GlobalHotKeyManager);
impl HotkeyBackend for Backend {
    fn register(&self, key: HotKey) -> Result<(), HotkeyError> {
        self.0.register(key).map_err(|_| HotkeyError::Unavailable)
    }
    fn unregister(&self, key: HotKey) -> Result<(), HotkeyError> {
        self.0.unregister(key).map_err(|_| HotkeyError::Unavailable)
    }
}
struct State {
    window: slint::Weak<AppWindow>,
    local: LocalUi,
    appearance: AppearanceUi,
    hotkey: RefCell<Option<HotkeyRegistration<Backend>>>,
    settings: RefCell<HotkeySettings>,
    events: slint::Timer,
    geometry: slint::Timer,
}
#[derive(Clone)]
pub struct NativeUi(Rc<State>);
impl NativeUi {
    pub fn attach(
        &self,
        settings: &AppSettings,
    ) -> Result<(), crate::domain::error::PlatformError> {
        let state = &self.0;
        let window = state
            .window
            .upgrade()
            .ok_or(crate::domain::error::PlatformError::WindowUnavailable)?;
        state
            .appearance
            .set_system_appearance(desktop::system_appearance(window.window()));
        let wayland = cfg!(target_os = "linux") && std::env::var_os("WAYLAND_DISPLAY").is_some();
        let registration = if wayland {
            None
        } else {
            GlobalHotKeyManager::new()
                .ok()
                .map(|backend| HotkeyRegistration::new(Rc::new(Backend(backend))))
        };
        if let Some(mut registration) = registration {
            if let Ok(key) = accelerator(&settings.hotkey).parse::<HotKey>()
                && let Ok(pending) = registration.prepare(key)
                && registration.commit(pending).is_ok()
            {
                *state.hotkey.borrow_mut() = Some(registration);
            } else {
                // Keep the manager so a conflicting default can be changed in General settings.
                *state.hotkey.borrow_mut() = Some(registration);
                state.message(
                    "Shortcut unavailable. Choose another in Settings → General.",
                    true,
                );
            }
        } else {
            state.message(
                if wayland {
                    "Global shortcuts are unavailable on Wayland. Use the app or tray menu."
                } else {
                    "Global shortcuts are unavailable on this desktop."
                },
                true,
            );
        }
        if state.hotkey.borrow().is_none() {
            crate::diagnostics::record(
                crate::diagnostics::DiagnosticEvent::DesktopCapabilityUnavailable {
                    capability: crate::diagnostics::DesktopCapability::GlobalShortcut,
                },
            );
        }
        let weak = Rc::downgrade(state);
        state.events.start(
            slint::TimerMode::Repeated,
            Duration::from_millis(40),
            move || {
                let Some(state) = weak.upgrade() else {
                    return;
                };
                while let Ok(event) = GlobalHotKeyEvent::receiver().try_recv() {
                    if event.state == HotKeyState::Pressed
                        && state
                            .hotkey
                            .borrow()
                            .as_ref()
                            .and_then(HotkeyRegistration::current)
                            .is_some_and(|key| key.id() == event.id)
                    {
                        state.toggle();
                    }
                }
            },
        );
        Ok(())
    }
    pub fn show(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.0.show()
    }
}
impl State {
    fn message(&self, message: &str, warning: bool) {
        if let Some(window) = self.window.upgrade() {
            window.set_native_message(message.into());
            window.set_native_warning(warning);
        }
    }
    fn show(&self) -> Result<(), Box<dyn std::error::Error>> {
        let window = self.window.upgrade().ok_or("window was closed")?;
        window.show()?;
        window
            .window()
            .with_winit_window(|native| native.focus_window());
        self.local.set_active(true);
        if window.get_active_view() == "note" {
            window.invoke_focus_editor();
        }
        Ok(())
    }
    fn toggle(&self) {
        if let Some(window) = self.window.upgrade() {
            let active = window
                .window()
                .with_winit_window(|native| native.has_focus())
                .unwrap_or(false);
            if window.window().is_visible() && active && window.get_can_hide_to_tray() {
                window.invoke_request_hide();
            } else if self.show().is_err() {
                self.message("Window could not be shown. Try reopening it.", true);
            }
        }
    }
    fn save_geometry(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        self.geometry.start(
            slint::TimerMode::SingleShot,
            Duration::from_millis(150),
            move || {
                let Some(state) = weak.upgrade() else {
                    return;
                };
                let Some(window) = state.window.upgrade() else {
                    return;
                };
                if let Ok(placement) = desktop::capture(&window) {
                    let _ = slint::spawn_local(async move {
                        if state.appearance.save_window(placement).await.is_err() {
                            state.message(
                                "Window position could not be saved. Retry closing.",
                                true,
                            );
                        }
                    });
                }
            },
        );
    }
    fn apply_hotkey(self: &Rc<Self>) {
        let Some(window) = self.window.upgrade() else {
            return;
        };
        if window.get_hotkey_busy() {
            return;
        }
        let parsed = window.get_hotkey_accelerator().parse::<HotKey>();
        let Ok(key) = parsed else {
            self.message("Shortcut is invalid. The current shortcut is kept.", true);
            return;
        };
        let pending = self
            .hotkey
            .borrow()
            .as_ref()
            .map(|registration| registration.prepare(key));
        let Some(Ok(pending)) = pending else {
            self.message("Shortcut unavailable. The current shortcut is kept.", true);
            return;
        };
        let settings = HotkeySettings {
            key: format!("{:?}", key.key),
            alt: key.mods.contains(Modifiers::ALT),
            control: key.mods.contains(Modifiers::CONTROL),
            shift: key.mods.contains(Modifiers::SHIFT),
            command: key.mods.contains(Modifiers::META),
        };
        let old = self.settings.borrow().clone();
        window.set_hotkey_busy(true);
        let state = self.clone();
        if slint::spawn_local(async move {
            if state
                .appearance
                .save_hotkey(settings.clone())
                .await
                .is_err()
            {
                drop(pending);
                state.message(
                    "Shortcut could not be saved. The current shortcut is kept.",
                    true,
                );
            } else {
                let committed = state
                    .hotkey
                    .borrow_mut()
                    .as_mut()
                    .is_some_and(|registration| registration.commit(pending).is_ok());
                if committed {
                    state.settings.replace(settings.clone());
                    if let Some(window) = state.window.upgrade() {
                        window.set_hotkey_accelerator(accelerator(&settings).into());
                    }
                    state.message("Shortcut saved.", false);
                } else {
                    let _ = state.appearance.save_hotkey(old).await;
                    state.message(
                        "Shortcut could not be registered. The previous shortcut is kept.",
                        true,
                    );
                }
            }
            if let Some(window) = state.window.upgrade() {
                window.set_hotkey_busy(false);
            }
        })
        .is_err()
        {
            window.set_hotkey_busy(false);
        }
    }
}

fn accelerator(settings: &HotkeySettings) -> String {
    let mut parts = Vec::new();
    if settings.alt {
        parts.push("Alt");
    }
    if settings.control {
        parts.push("Control");
    }
    if settings.shift {
        parts.push("Shift");
    }
    if settings.command {
        parts.push("Super");
    }
    parts.push(&settings.key);
    parts.join("+")
}

pub fn bind(
    window: &AppWindow,
    local: LocalUi,
    appearance: AppearanceUi,
    _native: Rc<crate::platform::application::NativeApplication>,
    settings: &AppSettings,
) -> NativeUi {
    let state = Rc::new(State {
        window: window.as_weak(),
        local,
        appearance,
        hotkey: RefCell::new(None),
        settings: RefCell::new(settings.hotkey.clone()),
        events: slint::Timer::default(),
        geometry: slint::Timer::default(),
    });
    window.set_hotkey_accelerator(accelerator(&settings.hotkey).into());
    let events = state.clone();
    window.window().on_winit_window_event(move |_, event| {
        match event {
            WindowEvent::Focused(active) => events.local.set_active(*active),
            WindowEvent::ThemeChanged(_) => {
                if let Some(window) = events.window.upgrade() {
                    events
                        .appearance
                        .set_system_appearance(desktop::system_appearance(window.window()));
                }
            }
            WindowEvent::Moved(_)
            | WindowEvent::Resized(_)
            | WindowEvent::ScaleFactorChanged { .. } => events.save_geometry(),
            _ => {}
        }
        EventResult::Propagate
    });
    let current = state.clone();
    window.on_native_reopen_requested(move || {
        let _ = current.show();
    });
    let current = state.clone();
    window.on_apply_hotkey(move || current.apply_hotkey());
    NativeUi(state)
}
