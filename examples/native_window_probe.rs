#[cfg(all(target_os = "macos", feature = "native-validation"))]
#[path = "support/material.rs"]
mod material;

use slint::ComponentHandle;
use std::{cell::Cell, rc::Rc, time::Duration};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::args().any(|arg| arg == "--material") {
        #[cfg(all(target_os = "macos", feature = "native-validation"))]
        return material::run(std::env::args().any(|arg| arg == "--auto-close"));
        #[cfg(not(all(target_os = "macos", feature = "native-validation")))]
        return Err("--material requires macOS and --features native-validation".into());
    }
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
    if std::env::args().any(|arg| arg == "--pin-probe") {
        #[cfg(target_os = "macos")]
        {
            let weak = window.as_weak();
            slint::Timer::single_shot(Duration::from_millis(400), move || {
                if let Some(window) = weak.upgrade() {
                    let level = bulletnotes::platform::macos::probe_window(window.window())
                        .map(|r| r.native_window_level);
                    if let Ok(before) = level {
                        window.set_probe_pinned(true);
                        let weak = window.as_weak();
                        slint::Timer::single_shot(Duration::from_millis(300), move || {
                            if let Some(window) = weak.upgrade()
                                && let Ok(pinned) =
                                    bulletnotes::platform::macos::probe_window(window.window())
                            {
                                let raised = pinned.native_window_level;
                                window.set_probe_pinned(false);
                                let weak = window.as_weak();
                                slint::Timer::single_shot(Duration::from_millis(300), move || {
                                    if let Some(window) = weak.upgrade()
                                        && let Ok(restored) =
                                            bulletnotes::platform::macos::probe_window(
                                                window.window(),
                                            )
                                    {
                                        println!(
                                            "pin_levels: {before},{raised},{}",
                                            restored.native_window_level
                                        );
                                    }
                                    let _ = slint::quit_event_loop();
                                });
                            }
                        });
                    }
                }
            });
        }
    }
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
