#[cfg(all(target_os = "macos", feature = "native-validation"))]
use bulletnotes::platform::window_chrome as chrome;
#[cfg(all(target_os = "macos", feature = "native-validation"))]
#[path = "support/hover_probe.rs"]
mod hover_probe;
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
    let window = bulletnotes::NativePreviewWindow::new()?;
    window.set_frameless_probe(std::env::args().any(|arg| arg == "--frameless"));
    window.set_styled_preview(slint::StyledText::from_markdown(
        "A **styled title** and [a link](https://example.invalid)",
    )?);
    // Winit creates the native window during event-loop startup, not at show().
    let weak = window.as_weak();
    let failed = Rc::new(Cell::new(false));
    let probe_failed = failed.clone();
    #[cfg(all(target_os = "macos", feature = "native-validation"))]
    let chrome_monitor = Rc::new(std::cell::RefCell::new(None));
    #[cfg(all(target_os = "macos", feature = "native-validation"))]
    let chrome_owner = chrome_monitor.clone();
    slint::Timer::single_shot(Duration::from_millis(300), move || {
        #[cfg(target_os = "macos")]
        if let Some(window) = weak.upgrade() {
            #[cfg(feature = "native-validation")]
            if std::env::args().any(|arg| arg == "--chrome-probe") {
                let observer = std::env::args()
                    .any(|arg| arg == "--hover-probe")
                    .then(|| hover_probe::observer(&window));
                match chrome::ChromeMonitor::install(&window, observer) {
                    Ok(monitor) => {
                        *chrome_owner.borrow_mut() = Some(monitor);
                    }
                    Err(error) => {
                        eprintln!("{error}");
                        probe_failed.set(true);
                    }
                }
            }
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
    if std::env::args().any(|arg| arg == "--chrome-probe")
        && !std::env::args().any(|arg| matches!(arg.as_str(), "--hover-probe" | "--pin-probe"))
    {
        slint::Timer::single_shot(Duration::from_millis(600), || {
            let _ = slint::quit_event_loop();
        });
    }
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
    if std::env::args().any(|arg| matches!(arg.as_str(), "--auto-close" | "--chrome-probe")) {
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
