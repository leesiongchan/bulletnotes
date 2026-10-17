//! Disposable in-process public-AppKit selection probe; generated text only.
#[cfg(all(target_os = "macos", feature = "native-validation"))]
#[expect(
    dead_code,
    reason = "shared helpers also serve the geometry companion probe"
)]
#[path = "support/selection_ax.rs"]
mod selection_ax;
#[cfg(all(target_os = "macos", feature = "native-validation"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use bulletnotes::{
        AppWindow, app::ui::present_settings, store::settings::AppSettings, theme::SystemAppearance,
    };
    use selection_ax::{find_editor, root, selection};
    use slint::ComponentHandle;
    use std::{cell::Cell, rc::Rc, time::Duration};

    let window = AppWindow::new()?;
    present_settings(&window, &AppSettings::default(), SystemAppearance::Light)?;
    let source = "A café 👩🏽‍💻 日本語\r\nsecond line";
    let start = source.find('👩').ok_or("fixture start")?;
    let end = source.find(" 日本語").ok_or("fixture end")?;
    let utf16_start = source[..start].encode_utf16().count();
    let utf16_length = source[start..end].encode_utf16().count();
    window.set_note_text(source.into());
    window.invoke_focus_editor();
    let passed = Rc::new(Cell::new(false));
    let weak = window.as_weak();
    slint::Timer::single_shot(Duration::from_millis(250), move || {
        if let Some(window) = weak.upgrade() {
            match root(window.window()) {
                Ok(view) => {
                    let _ = find_editor(&view, 0);
                }
                Err(error) => eprintln!("AX activate: {error}"),
            }
        }
    });
    for (phase, delay) in [(0, 500), (1, 900), (2, 1300), (3, 1700)] {
        let weak = window.as_weak();
        let passed = passed.clone();
        slint::Timer::single_shot(Duration::from_millis(delay), move || {
            let check = || -> Result<(), Box<dyn std::error::Error>> {
                let window = weak.upgrade().ok_or("window closed")?;
                if phase == 0 {
                    window.invoke_restore_editor_position(start as i32, end as i32, 0.0);
                    return Ok(());
                }
                let (range, line) = selection(window.window())?;
                println!(
                    "phase={phase} AX_UTF16_range=({}, {}) insertion_line={line}",
                    range.location, range.length
                );
                let expected_length = if phase == 3 { 0 } else { utf16_length };
                if range.location != utf16_start || range.length != expected_length {
                    return Err("AX selected range did not match generated fixture".into());
                }
                if window.get_note_text().as_bytes() != source.as_bytes() {
                    return Err("source changed".into());
                }
                if phase == 1 {
                    window.invoke_restore_editor_position(end as i32, start as i32, 0.0);
                }
                if phase == 2 {
                    window.invoke_restore_editor_position(start as i32, start as i32, 0.0);
                }
                if phase == 3 {
                    passed.set(true);
                    println!("public_ax_range_roundtrip: true");
                }
                Ok(())
            };
            if let Err(error) = check() {
                eprintln!("AX phase {phase}: {error}");
            }
            if phase == 3 {
                let _ = slint::quit_event_loop();
            }
        });
    }
    window.run()?;
    if !passed.get() {
        return Err("public AX range probe did not pass".into());
    }
    Ok(())
}
#[cfg(not(all(target_os = "macos", feature = "native-validation")))]
fn main() {
    eprintln!("selection_probe requires macOS and --features native-validation");
}
