//! Native lifecycle binding; note durability remains owned by LocalUi.
use super::{
    appearance_ui::AppearanceUi,
    local_ui::LocalUi,
    native_app::{capture, restore_window},
};
use crate::{
    AppWindow,
    platform::{
        application::NativeApplication,
        hotkey::{NativeHotkeyBackend, PendingHotkey},
        integration::{self, MacIntegration},
    },
    store::settings::{AppSettings, HotkeySettings},
};
use slint::ComponentHandle;
use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
    time::Duration,
};
struct State {
    window: slint::Weak<AppWindow>,
    local: LocalUi,
    appearance: AppearanceUi,
    native: Rc<NativeApplication>,
    integration: RefCell<Option<MacIntegration>>,
    settings: RefCell<HotkeySettings>,
    ready: Cell<bool>,
    geometry: slint::Timer,
}
#[derive(Clone)]
pub struct NativeUi(Rc<State>);
impl NativeUi {
    pub fn attach(
        &self,
        settings: &AppSettings,
    ) -> Result<(), crate::domain::error::PlatformError> {
        let window = self
            .0
            .window
            .upgrade()
            .ok_or(crate::domain::error::PlatformError::WindowUnavailable)?;
        let mut native = MacIntegration::attach(window.window(), settings)?;
        native.bind_events(&window)?;
        native.set_material(false)?;
        let result = native.register_hotkey(&integration::accelerator(&settings.hotkey));
        self.0.integration.replace(Some(native));
        self.0.ready.set(true);
        if result.is_err() {
            self.0.message(
                "Shortcut unavailable · choose another in Settings → General.",
                true,
            );
        }
        self.0.preferences();
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
        let window = self.window.upgrade().ok_or("native window was closed")?;
        integration::remember_foreign_app();
        window.show()?;
        self.local.set_active(true);
        if let Some(native) = self.integration.borrow().as_ref() {
            native.activate()?;
        } else {
            self.native.activate();
        }
        if window.get_active_view() == "note" {
            window.invoke_focus_editor();
        }
        Ok(())
    }
    fn preferences(&self) {
        let Some(window) = self.window.upgrade() else {
            return;
        };
        let (motion, transparency, contrast) = MacIntegration::preferences();
        window.set_prefers_reduced_motion(motion);
        window.set_prefers_reduced_transparency(transparency);
        window.set_system_high_contrast(contrast);
        self.appearance
            .set_system_appearance(self.native.system_appearance());
    }
    fn geometry(self: &Rc<Self>) {
        if !self.ready.get() {
            return;
        }
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
                let Ok(placement) = capture(&window) else {
                    return;
                };
                if slint::spawn_local(async move {
                    if state.appearance.save_window(placement).await.is_err() {
                        state.message(
                            "Window position could not be saved. Try closing again.",
                            true,
                        );
                    }
                })
                .is_err()
                { /* Normal local quit still owns a final metadata barrier. */
                }
            },
        );
    }
    fn hotkey(self: &Rc<Self>) {
        let Some(window) = self.window.upgrade() else {
            return;
        };
        if window.get_hotkey_busy() {
            return;
        }
        let prepared = self
            .integration
            .borrow()
            .as_ref()
            .map(|native| native.prepare_hotkey(&window.get_hotkey_accelerator()));
        let Some(Ok((settings, pending))) = prepared else {
            self.message(
                "Shortcut unavailable or invalid. Choose another; the current shortcut is kept.",
                true,
            );
            return;
        };
        window.set_hotkey_busy(true);
        let old = self.settings.borrow().clone();
        let weak = Rc::downgrade(self);
        let appearance = self.appearance.clone();
        if slint::spawn_local(Self::save_hotkey(weak, appearance, settings, pending, old)).is_err()
        {
            window.set_hotkey_busy(false);
            self.message(
                "Shortcut change could not start. The current shortcut is kept.",
                true,
            );
        }
    }
    async fn save_hotkey(
        weak: Weak<Self>,
        appearance: AppearanceUi,
        settings: HotkeySettings,
        pending: PendingHotkey<NativeHotkeyBackend>,
        old: HotkeySettings,
    ) {
        let saved = appearance.save_hotkey(settings.clone()).await;
        let Some(state) = weak.upgrade() else {
            return;
        };
        let Some(window) = state.window.upgrade() else {
            return;
        };
        if saved.is_err() {
            drop(pending);
            state.message(
                "Shortcut could not be saved. The current shortcut is kept; try again.",
                true,
            );
        } else {
            let committed = state
                .integration
                .borrow_mut()
                .as_mut()
                .is_some_and(|native| native.commit_hotkey(pending).is_ok());
            if committed {
                state.settings.replace(settings.clone());
                window.set_hotkey_accelerator(integration::accelerator(&settings).into());
                state.message("Shortcut saved.", false);
            } else {
                let rollback = appearance.save_hotkey(old).await;
                state.message(if rollback.is_ok() {
                    "Shortcut could not be changed. The previous shortcut is kept."
                } else {
                    "Shortcut could not be changed and its settings need another save. Use the menu to reopen."
                }, true);
            }
        }
        window.set_hotkey_busy(false);
    }
}
pub fn bind(
    window: &AppWindow,
    local: LocalUi,
    appearance: AppearanceUi,
    native: Rc<NativeApplication>,
    settings: &AppSettings,
) -> NativeUi {
    window.set_hotkey_accelerator(integration::accelerator(&settings.hotkey).into());
    let state = Rc::new(State {
        window: window.as_weak(),
        local,
        appearance,
        native,
        integration: RefCell::new(None),
        settings: RefCell::new(settings.hotkey.clone()),
        ready: Cell::new(false),
        geometry: slint::Timer::default(),
    });
    {
        let state = state.clone();
        window.on_toggle_activation(move || {
            let visible = state
                .window
                .upgrade()
                .is_some_and(|window| window.window().is_visible());
            let active = state
                .integration
                .borrow()
                .as_ref()
                .is_some_and(MacIntegration::is_active);
            if visible && active {
                if let Some(window) = state.window.upgrade() {
                    window.invoke_request_hide();
                }
            } else if state.show().is_err() {
                state.message(
                    "Window could not be shown. Use the menu to try again.",
                    true,
                );
            }
        });
    }
    {
        let state = state.clone();
        window.on_window_will_hide(move || {
            if let Some(native) = state.integration.borrow().as_ref()
                && native.hide_and_restore_previous_app().is_err()
            {
                state.message(
                    "Previous app declined focus. Your note is still available.",
                    true,
                );
            }
        });
    }
    {
        let state = state.clone();
        window.on_native_reopen_requested(move || {
            if state.show().is_err() {
                state.message(
                    "Window could not be reopened. Use the menu to try again.",
                    true,
                );
            }
        });
    }
    {
        let state = state.clone();
        window.on_native_preferences_changed(move || state.preferences());
    }
    {
        window.on_native_activation_changed(move || {
            integration::remember_foreign_app();
            // Activation also occurs when About opens; only explicit reopen
            // requests, menu actions and the hotkey reveal the hidden note.
        });
    }
    {
        let state = state.clone();
        window.on_native_geometry_changed(move || state.geometry());
    }
    {
        let state = state.clone();
        window.on_native_displays_changed(move || {
            if let Some(window) = state.window.upgrade()
                && let Ok(current) = capture(&window)
            {
                let _ = restore_window(&window, Some(&current));
            }
            state.geometry();
        });
    }
    {
        let state = state.clone();
        window.on_apply_hotkey(move || state.hotkey());
    }
    NativeUi(state)
}
