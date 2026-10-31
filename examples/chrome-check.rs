//! Native hover regression probe; uses generated text and no store or network.
#[cfg(target_os = "macos")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use bulletnotes::{
        AppWindow,
        app::ui::present_settings,
        platform::{
            application::NativeApplication,
            window_chrome::{ChromeMonitor, native_window},
        },
        store::settings::AppSettings,
        theme::SystemAppearance,
    };
    use objc2_app_kit::NSWindowButton;
    use slint::ComponentHandle;
    use std::{
        cell::{Cell, RefCell},
        rc::Rc,
        time::Duration,
    };
    let _application = NativeApplication::prepare()?;
    bulletnotes::platform::renderer::select()?;
    let window = AppWindow::new()?;
    present_settings(&window, &AppSettings::default(), SystemAppearance::Light)?;
    window.set_note_text("# A quieter workspace\n\nGenerated content only.".into());
    window.set_note_date("Today".into());
    window.set_note_counter("1 of 3".into());
    window.show()?;
    let monitor = RefCell::new(None::<ChromeMonitor>);
    let native_owner = RefCell::new(None::<objc2::rc::Retained<objc2_app_kit::NSWindow>>);
    let baseline = Rc::new(RefCell::new(None::<Vec<(f64, f64)>>));
    let failed = Rc::new(Cell::new(false));
    let failure = failed.clone();
    let tick = Cell::new(0);
    let weak = window.as_weak();
    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(16),
        move || {
            let current = tick.get() + 1;
            tick.set(current);
            let Some(window) = weak.upgrade() else {
                return;
            };
            if current == 5 {
                *monitor.borrow_mut() =
                    Some(ChromeMonitor::install(&window, None).expect("native monitor"));
                *native_owner.borrow_mut() =
                    Some(native_window(window.window()).expect("native window"));
            }
            let native_owner = native_owner.borrow();
            let Some(native) = native_owner.as_ref() else {
                return;
            };
            if current == 5 {
                println!("window_id={}", native.windowNumber());
            }
            if matches!(current, 25 | 45 | 88 | 130) {
                window.set_chrome_active(true);
                window.set_header_hovered(true);
                window.set_footer_hovered(true);
            }
            if current == 65 {
                window.set_header_hovered(false);
            }
            if current == 28 || current == 112 {
                window.invoke_reset_chrome();
            }
            if (20..150).contains(&current) {
                let points: Vec<_> = [
                    NSWindowButton::CloseButton,
                    NSWindowButton::MiniaturizeButton,
                    NSWindowButton::ZoomButton,
                ]
                .into_iter()
                .filter_map(|kind| {
                    let button = native.standardWindowButton(kind)?;
                    let parent = unsafe { button.superview() }?;
                    let frame = parent.convertRect_toView(button.frame(), None);
                    Some((
                        frame.origin.x,
                        native.frame().size.height - frame.origin.y - frame.size.height,
                    ))
                })
                .collect();
                assert_eq!(points.len(), 3, "all native controls must exist");
                if matches!(current, 60 | 110 | 145) {
                    assert!(
                        window.get_header_revealed(),
                        "probe must exercise the revealed header"
                    );
                    for kind in [
                        NSWindowButton::CloseButton,
                        NSWindowButton::MiniaturizeButton,
                        NSWindowButton::ZoomButton,
                    ] {
                        let button = native.standardWindowButton(kind).unwrap();
                        assert!(
                            button.isEnabled() && button.alphaValue() > 0.95,
                            "revealed controls must remain actionable"
                        );
                    }
                }
                if current == 44 || current == 128 {
                    assert!(!window.get_header_revealed());
                    for kind in [
                        NSWindowButton::CloseButton,
                        NSWindowButton::MiniaturizeButton,
                        NSWindowButton::ZoomButton,
                    ] {
                        let button = native.standardWindowButton(kind).unwrap();
                        assert!(
                            !button.isEnabled() && button.alphaValue() < 0.05,
                            "reset controls must remain invisible and disabled"
                        );
                    }
                }
                if current == 20 {
                    println!("baseline={points:?}");
                    *baseline.borrow_mut() = Some(points.clone());
                }
                if let Some(initial) = baseline.borrow().as_ref()
                    && initial
                        .iter()
                        .zip(&points)
                        .any(|(a, b)| (a.0 - b.0).abs() > 0.5 || (a.1 - b.1).abs() > 0.5)
                    && !failure.replace(true)
                {
                    eprintln!("Window controls moved at tick {current}: {points:?}");
                }
            }
            if current == 150 {
                slint::quit_event_loop().unwrap();
            }
        },
    );
    slint::run_event_loop_until_quit()?;
    if failed.get() {
        return Err("window controls moved during hover reveal".into());
    }
    println!("Window control hover frames remained stable.");
    Ok(())
}
#[cfg(not(target_os = "macos"))]
fn main() {
    println!("Native traffic-light verification requires macOS.");
}
