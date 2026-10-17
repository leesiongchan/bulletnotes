use slint::ComponentHandle;
use std::{cell::Cell, rc::Rc, time::Duration};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let window = bulletnotes::AppWindow::new()?;
    window.set_frameless_probe(std::env::args().any(|arg| arg == "--frameless"));
    window.set_styled_preview(slint::StyledText::from_markdown(
        "A **styled title** and [a link](https://example.invalid)",
    )?);
    // Winit creates the native window during event-loop startup, not at show().
    let weak = window.as_weak();
    let failed = Rc::new(Cell::new(false));
    let probe_failed = failed.clone();
    slint::Timer::single_shot(Duration::from_millis(300), move || {
        #[cfg(target_os = "macos")]
        if let Some(window) = weak.upgrade() {
            match bulletnotes::platform::macos::probe_window(window.window()) {
                Ok(report) => println!("{report:#?}"),
                Err(error) => {
                    probe_failed.set(true);
                    eprintln!("{error}");
                    let _ = slint::quit_event_loop();
                }
            }
        }
    });
    if std::env::args().any(|arg| arg == "--auto-close") {
        slint::Timer::single_shot(Duration::from_secs(15), || {
            if let Err(error) = slint::quit_event_loop() {
                eprintln!("event loop: {error}");
            }
        });
    }
    window.run()?;
    if failed.get() {
        return Err("native window probe failed".into());
    }
    Ok(())
}
