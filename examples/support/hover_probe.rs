//! Synthetic NSEvents are queued only to this throwaway process, never globally.
use bulletnotes::NativePreviewWindow as AppWindow;
use objc2::MainThreadMarker;
use objc2_app_kit::{NSApplication, NSEvent, NSEventModifierFlags, NSEventType};
use objc2_foundation::{NSPoint, NSProcessInfo};
use slint::ComponentHandle;
use std::time::Duration;

fn post_mouse(window: &AppWindow, at_top: bool) -> Result<(), Box<dyn std::error::Error>> {
    let mtm = MainThreadMarker::new().ok_or("main thread required")?;
    let native = crate::chrome::native_window(window.window())?;
    let frame = native.frame();
    let point = NSPoint::new(
        frame.size.width / 2.0,
        frame.size.height - if at_top { 16.0 } else { 120.0 },
    );
    let event=NSEvent::mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure(
        NSEventType::MouseMoved,point,NSEventModifierFlags::empty(),NSProcessInfo::processInfo().systemUptime(),native.windowNumber(),None,if at_top { 0x1001 } else { 0x1002 },0,0.0
    ).ok_or("failed to construct owned-window event")?;
    NSApplication::sharedApplication(mtm).postEvent_atStart(&event, false);
    Ok(())
}
pub fn observer(window: &AppWindow) -> Box<dyn Fn(bool, isize)> {
    window.set_chrome_revealed(false);
    println!("hover_initial: {}", window.get_chrome_revealed());
    let original = window.get_source();
    let weak = window.as_weak();
    slint::Timer::single_shot(Duration::from_millis(40), move || {
        if let Some(window) = weak.upgrade()
            && let Err(error) = post_mouse(&window, true)
        {
            eprintln!("hover event: {error}");
        }
    });
    let phase = std::cell::Cell::new(0);
    let weak = window.as_weak();
    Box::new(move |_hover, event_id| {
        if let Some(window) = weak.upgrade() {
            if event_id == 0x1001 && phase.get() == 0 {
                println!("hover_revealed: {}", window.get_chrome_revealed());
                phase.set(1);
                let weak = window.as_weak();
                slint::Timer::single_shot(Duration::from_millis(40), move || {
                    if let Some(window) = weak.upgrade()
                        && let Err(error) = post_mouse(&window, false)
                    {
                        eprintln!("hover event: {error}");
                    }
                });
            } else if event_id == 0x1002 && phase.get() == 1 {
                println!("hover_returned: {}", window.get_chrome_revealed());
                println!("editor_preserved: {}", window.get_source() == original);
                phase.set(2);
                let _ = slint::quit_event_loop();
            }
        }
    })
}
