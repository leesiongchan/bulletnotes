use bulletnotes::AppWindow;
use slint::ComponentHandle;
use std::{
    cell::Cell,
    io::{self, Read, Write},
    rc::Rc,
    time::Duration,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let window = AppWindow::new()?;
    window.set_styled_preview(slint::StyledText::from_markdown(
        "A **styled title** and [a link](https://example.invalid)",
    )?);
    let acceptance_failed = Rc::new(Cell::new(false));
    if std::env::args().any(|arg| arg == "--roundtrip") {
        let mut original = String::new();
        io::stdin().read_to_string(&mut original)?;
        window.set_source(original.into());
        let weak = window.as_weak();
        slint::Timer::single_shot(Duration::from_millis(300), move || {
            if let Some(window) = weak.upgrade() {
                let result = io::stdout().write_all(window.get_source().as_bytes());
                if let Err(error) = result {
                    eprintln!("roundtrip output: {error}");
                }
                if let Err(error) = slint::quit_event_loop() {
                    eprintln!("event loop: {error}");
                }
            }
        });
    } else {
        eprintln!(
            "UNVERIFIED: composition, undo/redo, selection, wrapped scrolling, editable accents, materials, activation, full-screen focus, VoiceOver"
        );
        if std::env::args().any(|arg| arg == "--acceptance") {
            let weak = window.as_weak();
            let failed = acceptance_failed.clone();
            slint::Timer::single_shot(Duration::from_millis(300), move || {
                #[cfg(target_os = "macos")]
                let passed = weak
                    .upgrade()
                    .and_then(|window| {
                        bulletnotes::platform::macos::probe_window(window.window()).ok()
                    })
                    .is_some_and(|report| {
                        eprintln!("{report:#?}");
                        report.passed()
                    });
                #[cfg(not(target_os = "macos"))]
                let passed = false;
                failed.set(!passed);
                if let Err(error) = slint::quit_event_loop() {
                    eprintln!("event loop: {error}");
                }
            });
        }
    }
    window.run()?;
    if acceptance_failed.get() {
        return Err("native fidelity gate has unmet requirements".into());
    }
    Ok(())
}
