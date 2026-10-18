//! Main-thread macOS activation and supported native window operations.
use super::{
    hotkey::{HotkeyError, HotkeyRegistration, NativeHotkeyBackend, PendingHotkey},
    native_events::NativeEvents,
    window_chrome::native_window,
};
use crate::{
    AppWindow,
    domain::error::PlatformError,
    store::settings::{AppSettings, HotkeySettings},
};
use global_hotkey::{
    GlobalHotKeyEvent, HotKeyState,
    hotkey::{HotKey, Modifiers},
};
use objc2::{MainThreadMarker, rc::Retained};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationOptions, NSRunningApplication, NSWindow, NSWorkspace,
};
use objc2_app_kit::{NSFloatingWindowLevel, NSNormalWindowLevel};
use std::{
    cell::RefCell,
    rc::Rc,
    str::FromStr,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU32, Ordering},
    },
};
thread_local! {static PREVIOUS:RefCell<Option<Retained<NSRunningApplication>>>=const{RefCell::new(None)};}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LifecycleAction {
    Activate,
    Hide,
    CloseRequested,
    QuitRequested,
    SystemAppearanceChanged,
    AccessibilityPreferencesChanged,
}
pub fn remember_foreign_app() {
    if MainThreadMarker::new().is_none() {
        return;
    }
    let current = NSRunningApplication::currentApplication();
    if !current.isActive()
        && let Some(previous) = NSWorkspace::sharedWorkspace()
            .frontmostApplication()
            .filter(|app| app != &current)
    {
        PREVIOUS.with(|slot| slot.replace(Some(previous)));
    }
}
pub fn accelerator(settings: &HotkeySettings) -> String {
    let mut parts = Vec::new();
    if settings.alt {
        parts.push("Option");
    }
    if settings.control {
        parts.push("Control");
    }
    if settings.shift {
        parts.push("Shift");
    }
    if settings.command {
        parts.push("Command");
    }
    let label = settings
        .key
        .strip_prefix("Key")
        .or_else(|| settings.key.strip_prefix("Digit"))
        .unwrap_or(&settings.key);
    parts.push(label);
    parts.join("+")
}
pub fn parse_accelerator(value: &str) -> Result<(HotkeySettings, HotKey), HotkeyError> {
    let key = HotKey::from_str(value).map_err(|_| HotkeyError::Invalid)?;
    let settings = HotkeySettings {
        key: format!("{:?}", key.key),
        alt: key.mods.contains(Modifiers::ALT),
        control: key.mods.contains(Modifiers::CONTROL),
        shift: key.mods.contains(Modifiers::SHIFT),
        command: key.mods.contains(Modifiers::META),
    };
    let candidate = AppSettings {
        hotkey: settings.clone(),
        ..Default::default()
    };
    candidate.validate().map_err(|_| HotkeyError::Invalid)?;
    if key
        .mods
        .difference(Modifiers::ALT | Modifiers::CONTROL | Modifiers::SHIFT | Modifiers::META)
        .bits()
        != 0
    {
        return Err(HotkeyError::Invalid);
    }
    Ok((settings, key))
}
pub struct MacIntegration {
    window: Retained<NSWindow>,
    app: Retained<NSApplication>,
    current: Retained<NSRunningApplication>,
    registration: Option<HotkeyRegistration<NativeHotkeyBackend>>,
    active: Arc<AtomicU32>,
    down: Arc<AtomicBool>,
    events: Option<NativeEvents>,
    _main: MainThreadMarker,
}
impl MacIntegration {
    pub fn attach(window: &slint::Window, settings: &AppSettings) -> Result<Self, PlatformError> {
        let main = MainThreadMarker::new().ok_or(PlatformError::WrongThread)?;
        let window = native_window(window).map_err(|_| PlatformError::WindowUnavailable)?;
        let integration = Self {
            window,
            app: NSApplication::sharedApplication(main),
            current: NSRunningApplication::currentApplication(),
            registration: NativeHotkeyBackend::new()
                .ok()
                .map(|backend| HotkeyRegistration::new(Rc::new(backend))),
            active: Arc::new(AtomicU32::new(0)),
            down: Arc::new(AtomicBool::new(false)),
            events: None,
            _main: main,
        };
        if let Some(placement) = &settings.window {
            integration.set_pinned(placement.pinned)?;
        }
        Ok(integration)
    }
    pub fn bind_events(&mut self, window: &AppWindow) -> Result<(), PlatformError> {
        self.events = Some(
            NativeEvents::new(window, &self.window)
                .map_err(|_| PlatformError::WindowUnavailable)?,
        );
        let weak = slint::ComponentHandle::as_weak(window);
        let active = self.active.clone();
        let down = self.down.clone();
        let Some(binding) = self.registration.as_ref() else {
            return Ok(());
        };
        binding
            .backend()
            .set_event_handler(move |event: GlobalHotKeyEvent| {
                if event.id != active.load(Ordering::Relaxed) {
                    return;
                }
                match event.state {
                    HotKeyState::Released => {
                        down.store(false, Ordering::Relaxed);
                    }
                    HotKeyState::Pressed if !down.swap(true, Ordering::Relaxed) => {
                        let _ =
                            weak.upgrade_in_event_loop(|window| window.invoke_toggle_activation());
                    }
                    HotKeyState::Pressed => {}
                }
            });
        Ok(())
    }
    pub fn prepare_hotkey(
        &self,
        value: &str,
    ) -> Result<(HotkeySettings, PendingHotkey<NativeHotkeyBackend>), HotkeyError> {
        let (settings, key) = parse_accelerator(value)?;
        Ok((
            settings,
            self.registration
                .as_ref()
                .ok_or(HotkeyError::Unavailable)?
                .prepare(key)?,
        ))
    }
    pub fn commit_hotkey(
        &mut self,
        pending: PendingHotkey<NativeHotkeyBackend>,
    ) -> Result<(), HotkeyError> {
        let binding = self.registration.as_mut().ok_or(HotkeyError::Unavailable)?;
        binding.commit(pending)?;
        self.active.store(
            binding.current().map_or(0, |key| key.id()),
            Ordering::Relaxed,
        );
        self.down.store(false, Ordering::Relaxed);
        Ok(())
    }
    pub fn register_hotkey(&mut self, value: &str) -> Result<(), PlatformError> {
        let (_, pending) = self
            .prepare_hotkey(value)
            .map_err(|_| PlatformError::HotkeyUnavailable)?;
        self.commit_hotkey(pending)
            .map_err(|_| PlatformError::HotkeyUnavailable)
    }
    pub fn set_pinned(&self, pinned: bool) -> Result<(), PlatformError> {
        self.window.setLevel(if pinned {
            NSFloatingWindowLevel
        } else {
            NSNormalWindowLevel
        });
        Ok(())
    }
    pub fn set_material(&self, enabled: bool) -> Result<(), PlatformError> {
        if enabled {
            return Err(PlatformError::MaterialUnavailable);
        }
        Ok(())
    }
    pub fn is_active(&self) -> bool {
        self.current.isActive()
    }
    pub fn activate(&self) -> Result<(), PlatformError> {
        remember_foreign_app();
        self.app.activate();
        self.window.makeKeyAndOrderFront(None);
        Ok(())
    }
    pub fn hide_and_restore_previous_app(&self) -> Result<(), PlatformError> {
        let active = self.current.isActive();
        self.window.orderOut(None);
        if active
            && let Some(previous) = PREVIOUS
                .with(|slot| slot.borrow().clone())
                .filter(|app| !app.isTerminated())
        {
            self.app.yieldActivationToApplication(&previous);
            if !previous.activateFromApplication_options(
                &self.current,
                NSApplicationActivationOptions::empty(),
            ) {
                return Err(PlatformError::ActivationDeclined);
            }
        }
        Ok(())
    }
    pub fn preferences() -> (bool, bool, bool) {
        let workspace = NSWorkspace::sharedWorkspace();
        (
            workspace.accessibilityDisplayShouldReduceMotion(),
            workspace.accessibilityDisplayShouldReduceTransparency(),
            workspace.accessibilityDisplayShouldIncreaseContrast(),
        )
    }
}

impl Drop for MacIntegration {
    fn drop(&mut self) {
        self.active.store(0, Ordering::Relaxed);
        self.events.take();
        self.registration.take();
    }
}
