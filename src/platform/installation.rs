//! Explicit installation smoke path: generated UI only, no store/credentials/network.
use crate::{
    AppWindow, app::ui::present_settings, store::settings::AppSettings, theme::SystemAppearance,
};
use slint::ComponentHandle;
use std::{cell::Cell, rc::Rc, time::Duration};
pub fn check() -> Result<(), Box<dyn std::error::Error>> {
    let _application = super::application::NativeApplication::prepare()?;
    let window = AppWindow::new()?;
    present_settings(&window, &AppSettings::default(), SystemAppearance::Light)?;
    window.set_current_path("Installation check".into());
    window.set_note_text("Bulletnotes installation check.\nGenerated content only.".into());
    window.set_status_text("Nothing is saved or synchronized".into());
    let checked = Rc::new(Cell::new(false));
    let result = checked.clone();
    let weak = window.as_weak();
    slint::Timer::single_shot(Duration::from_millis(400), move || {
        if let Some(window) = weak.upgrade() {
            result.set(super::window_chrome::native_window(window.window()).is_ok());
            let _ = window.hide();
        }
        let _ = slint::quit_event_loop();
    });
    window.show()?;
    slint::run_event_loop_until_quit()?;
    if !checked.get() {
        return Err("installation check did not observe a native window".into());
    }
    println!("installation_check: true");
    Ok(())
}
