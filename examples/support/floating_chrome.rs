//! Owned generated-window checks; events never leave this fixture process.
use bulletnotes::{
    AppWindow,
    app::ui::{apply_theme, present_settings},
    platform::{
        application::NativeApplication,
        renderer,
        window_chrome::{ChromeMonitor, native_window},
    },
    store::settings::AppSettings,
    theme::{SystemAppearance, ThemeId, resolve_theme},
};
use objc2::MainThreadMarker;
use objc2_app_kit::{NSApplication, NSEvent, NSEventModifierFlags, NSEventType, NSWindowButton};
use objc2_foundation::{NSPoint, NSProcessInfo};
use slint::{
    ComponentHandle,
    platform::{Key, WindowEvent},
};
use std::{cell::Cell, rc::Rc, time::Duration};
thread_local! { static FRAMES: Cell<usize> = const { Cell::new(0) }; }
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
const OWNED_MOUSE_EVENT: isize = 0x7010;
async fn pause(ms: u64) -> Result<()> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    slint::Timer::single_shot(Duration::from_millis(ms), move || {
        let _ = tx.send(());
    });
    rx.await?;
    Ok(())
}
fn key(window: &AppWindow, text: slint::SharedString) {
    window
        .window()
        .dispatch_event(WindowEvent::KeyPressed { text: text.clone() });
    window
        .window()
        .dispatch_event(WindowEvent::KeyReleased { text });
}
fn undo(window: &AppWindow) {
    window.window().dispatch_event(WindowEvent::KeyPressed {
        text: Key::Control.into(),
    });
    key(window, "z".into());
    window.window().dispatch_event(WindowEvent::KeyReleased {
        text: Key::Control.into(),
    });
}
fn mouse(window: &AppWindow, x: f64, y: f64, kind: NSEventType) -> Result<()> {
    let native = native_window(window.window())?;
    let event=NSEvent::mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure(
        kind,NSPoint::new(x,y),NSEventModifierFlags::empty(),NSProcessInfo::processInfo().systemUptime(),native.windowNumber(),None,OWNED_MOUSE_EVENT,1,0.0
    ).ok_or("owned event construction failed")?;
    NSApplication::sharedApplication(MainThreadMarker::new().ok_or("main thread required")?)
        .postEvent_atStart(&event, false);
    Ok(())
}
fn capture(window: &AppWindow, name: &str) -> Result<()> {
    let native = native_window(window.window())?;
    eprintln!(
        "capture {name}: visible={} key={} alpha={} occlusion={:?} frame={:?} frames={} metal={:?}",
        native.isVisible(),
        native.isKeyWindow(),
        native.alphaValue(),
        native.occlusionState(),
        native.frame(),
        FRAMES.with(Cell::get),
        renderer::uses_metal(window.window())
    );
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(".superpowers/sdd/2026-10-18-automatic-sync")
        .join(format!("floating-{name}.png"));
    if !std::process::Command::new("/usr/sbin/screencapture")
        .args(["-x", "-o", &format!("-l{}", native.windowNumber())])
        .arg(path)
        .status()?
        .success()
    {
        return Err("owned window capture failed".into());
    }
    Ok(())
}
fn edges(window: &AppWindow, want: (bool, bool)) -> Result<()> {
    let got = (window.get_header_revealed(), window.get_footer_revealed());
    if got != want {
        let native = native_window(window.window())?;
        return Err(format!("edge visibility {got:?}, expected {want:?}; key={} active={} hovered={},{} opacity={},{}",native.isKeyWindow(),window.get_chrome_active(),window.get_header_hovered(),window.get_footer_hovered(),window.get_header_opacity(),window.get_footer_opacity()).into());
    }
    Ok(())
}
pub fn run() -> Result<()> {
    let application = NativeApplication::prepare()?;
    renderer::select()?;
    let window = AppWindow::new()?;
    window.window().set_rendering_notifier(|state, _| {
        if matches!(state, slint::RenderingState::AfterRendering) {
            FRAMES.with(|count| count.set(count.get() + 1));
        }
    })?;
    let mut settings = AppSettings::default();
    let imported =
        bulletnotes::theme::palette::ThemeEntry::import(bulletnotes::theme::format::decode_theme(
            include_bytes!("../../tests/fixtures/themes/valid.json"),
        )?);
    let custom = imported.id.0.clone();
    settings.imported_themes.push(imported);
    present_settings(&window, &settings, SystemAppearance::Light)?;
    let source = format!(
        "012345 café 日本語 👩🏽‍💻\r\n{}",
        "generated owned line\r\n".repeat(48)
    );
    window.set_note_text(source.clone().into());
    window.set_current_path("Generated/owned.md".into());
    window.set_status_text("Generated fixture".into());
    let pins = Rc::new(Cell::new(0));
    let count = pins.clone();
    window.on_toggle_pin(move || count.set(count.get() + 1));
    let browses = Rc::new(Cell::new(0));
    let count = browses.clone();
    window.on_browse_pages(move || count.set(count.get() + 1));
    let histories = Rc::new(Cell::new(0));
    let count = histories.clone();
    window.on_history_step(move |_| count.set(count.get() + 1));
    let weak = window.as_weak();
    let passed = Rc::new(Cell::new(false));
    let result = passed.clone();
    slint::spawn_local(async move {
        let check: Result<()> = async {
            pause(300).await?;
            let window = weak.upgrade().ok_or("fixture closed")?;
            for _ in 0..80 {
                if FRAMES.with(Cell::get) > 0 { break; }
                pause(100).await?;
            }
            if FRAMES.with(Cell::get) == 0 {
                let native=native_window(window.window())?;
                eprintln!("first-frame timeout: visible={} key={} alpha={} occlusion={:?} frame={:?} metal={:?}",native.isVisible(),native.isKeyWindow(),native.alphaValue(),native.occlusionState(),native.frame(),renderer::uses_metal(window.window()));
                return Err("owned fixture received no rendering frame within8s".into());
            }
            let unexpected_mouse = Rc::new(Cell::new(0));
            let observed = unexpected_mouse.clone();
            let _monitor = ChromeMonitor::install(&window, Some(Box::new(move |_, event| {
                if event != OWNED_MOUSE_EVENT { observed.set(observed.get() + 1); }
            })))?;
            // Match MacIntegration::activate after the frameless window gains its native style.
            native_window(window.window())?.makeKeyAndOrderFront(None);
            pause(100).await?;
            let app = NSApplication::sharedApplication(MainThreadMarker::new().ok_or("main thread required")?);
            let native = native_window(window.window())?;
            eprintln!("owned activation: active={} running={} policy={:?} can-key={} key={}", app.isActive(), app.isRunning(), app.activationPolicy(), native.canBecomeKeyWindow(), native.isKeyWindow());
            window.invoke_reset_chrome();
            window.set_chrome_active(true);
            window.invoke_focus_editor();
            edges(&window, (false, false))?;
            capture(&window, "hidden")?;
            let geometry = (window.get_editor_width(), window.get_editor_height());
            let native = native_window(window.window())?;
            assert!(native.isKeyWindow());
            let size = native.frame().size;
            let close = native
                .standardWindowButton(NSWindowButton::CloseButton)
                .ok_or("native close button missing")?;
            assert!(close.isHidden());
            // The first line remains clickable inside the transparent native titlebar area.
            window.invoke_restore_editor_position(6, 6, 0.0);
            mouse(&window, 42.0, size.height - 27.0, NSEventType::LeftMouseDown)?;
            mouse(&window, 42.0, size.height - 27.0, NSEventType::LeftMouseUp)?;
            pause(40).await?;
            edges(&window, (false, false))?;
            key(&window, "X".into());
            pause(30).await?;
            assert_eq!(window.get_note_text(), format!("X{source}"));
            undo(&window);
            pause(30).await?;
            assert_eq!(window.get_note_text(), source);
            println!("native_first_line_pointer_edit_passed: true");
            mouse(
                &window,
                size.width / 2.0,
                size.height - 4.0,
                NSEventType::MouseMoved,
            )?;
            pause(60).await?;
            edges(&window, (true, false))?;
            println!("reveal_midpoint_opacity: {}", window.get_header_opacity());
            capture(&window, "midpoint")?;
            pause(180).await?;
            assert!(!close.isHidden() && close.isEnabled(), "native header buttons hidden={},enabled={},revealed={},hovered={},opacity={},key={},unexpected_mouse={}", close.isHidden(), close.isEnabled(), window.get_header_revealed(), window.get_header_hovered(), window.get_header_opacity(), native.isKeyWindow(), unexpected_mouse.get());
            capture(&window, "header")?;
            // The small gap below the header belongs to the same retention region.
            mouse(
                &window,
                size.width / 2.0,
                size.height - 40.0,
                NSEventType::MouseMoved,
            )?;
            pause(140).await?;
            edges(&window, (true, false))?;
            // Reenter the still-visible surface after the dismissal delay has elapsed.
            mouse(&window, size.width / 2.0, size.height / 2.0, NSEventType::MouseMoved)?;
            for _ in 0..100 {
                if !window.get_header_revealed() { break; }
                pause(5).await?;
            }
            edges(&window, (false, false))?;
            assert!(window.get_header_opacity() > 0.0);
            println!("fade_retention_reentry_opacity: {}", window.get_header_opacity());
            mouse(&window, size.width / 2.0, size.height - 40.0, NSEventType::MouseMoved)?;
            pause(30).await?;
            edges(&window, (true, false))?;
            mouse(
                &window,
                size.width / 2.0,
                size.height / 2.0,
                NSEventType::MouseMoved,
            )?;
            pause(40).await?;
            edges(&window, (true, false))?;
            mouse(
                &window,
                size.width / 2.0,
                size.height - 4.0,
                NSEventType::MouseMoved,
            )?;
            pause(100).await?;
            edges(&window, (true, false))?;
            mouse(&window, size.width / 2.0, 4.0, NSEventType::MouseMoved)?;
            pause(240).await?;
            edges(&window, (false, true))?;
            capture(&window, "footer")?;
            // Focus holds the header while the pointer independently reveals the footer.
            key(&window, Key::F6.into());
            pause(40).await?;
            edges(&window, (true, true))?;
            key(&window, " ".into());
            assert_eq!(pins.get(), 1);
            pause(200).await?;
            assert_eq!(window.get_header_opacity(), 1.0);
            assert_eq!(window.get_footer_opacity(), 1.0);
            println!("settled_chrome_opacity: 1,1");
            capture(&window, "both")?;
            key(&window, Key::F6.into());
            pause(40).await?;
            key(&window, " ".into());
            assert_eq!(browses.get(), 1);
            // Disabled history actions cannot be triggered through pointer dispatch.
            for x in [size.width - 32.0, size.width - 62.0] {
                mouse(&window, x, 28.0, NSEventType::LeftMouseDown)?;
                mouse(&window, x, 28.0, NSEventType::LeftMouseUp)?;
            }
            pause(40).await?;
            assert_eq!(histories.get(), 0);
            key(&window, Key::Escape.into());
            mouse(
                &window,
                size.width / 2.0,
                size.height / 2.0,
                NSEventType::MouseMoved,
            )?;
            pause(360).await?;
            edges(&window, (false, false))?;
            // Actual button focus is a hold; moving focus back to the editor releases it.
            eprintln!(
                "focus-release phase: key={}, active={}, edges={:?}",
                native.isKeyWindow(),
                window.get_chrome_active(),
                (window.get_header_revealed(), window.get_footer_revealed())
            );
            window.invoke_focus_editor();
            key(&window, Key::F6.into());
            pause(220).await?;
            edges(&window, (true, false))?;
            window.invoke_focus_editor();
            pause(360).await?;
            edges(&window, (false, false))?;
            key(&window, Key::F6.into());
            key(&window, Key::F6.into());
            key(&window, Key::F6.into());
            window.invoke_restore_editor_position(0, 0, 0.0);
            key(&window, "Q".into());
            pause(30).await?;
            assert_eq!(window.get_note_text(), format!("Q{source}"));
            undo(&window);
            pause(30).await?;
            assert_eq!(window.get_note_text(), source);
            key(&window, Key::F6.into());
            key(&window, Key::Escape.into());
            window.invoke_restore_editor_position(0, 0, 0.0);
            key(&window, "R".into());
            pause(30).await?;
            assert_eq!(window.get_note_text(), format!("R{source}"));
            undo(&window);
            pause(30).await?;
            assert_eq!(window.get_note_text(), source);
            pause(360).await?;
            edges(&window, (false, false))?;
            // A selection drag starting in the editor cannot reveal either edge.
            mouse(&window, 80.0, size.height / 2.0, NSEventType::LeftMouseDown)?;
            mouse(
                &window,
                80.0,
                size.height - 4.0,
                NSEventType::LeftMouseDragged,
            )?;
            pause(60).await?;
            edges(&window, (false, false))?;
            mouse(&window, 80.0, 4.0, NSEventType::LeftMouseDragged)?;
            pause(60).await?;
            edges(&window, (false, false))?;
            mouse(&window, 80.0, 4.0, NSEventType::LeftMouseUp)?;
            pause(40).await?;
            edges(&window, (false, true))?;
            // Hover and F6 return to the same editor with selection/scroll/undo intact.
            window.invoke_focus_editor();
            window.invoke_restore_editor_position(2, 5, 220.0);
            pause(40).await?;
            let scroll = window.get_editor_scroll_y();
            window.set_header_hovered(true);
            window.set_footer_hovered(true);
            pause(240).await?;
            assert_eq!(
                (window.get_editor_width(), window.get_editor_height()),
                geometry
            );
            assert_eq!(window.get_editor_scroll_y(), scroll);
            assert_eq!(window.get_note_text().as_bytes(), source.as_bytes());
            key(&window, "X".into());
            pause(30).await?;
            assert_eq!(
                window.get_note_text(),
                format!("{}X{}", &source[..2], &source[5..])
            );
            undo(&window);
            pause(30).await?;
            assert_eq!(
                window.get_note_text(),
                format!("{}{}", &source[..2], &source[5..])
            );
            // Slint records deletion and insertion as two undo entries for a replacement.
            undo(&window);
            pause(30).await?;
            assert_eq!(window.get_note_text(), source);
            // Reverse a native fade-out before its completion; stale completion must not hide it.
            window.set_header_hovered(false);
            pause(130).await?;
            window.set_header_hovered(true);
            pause(260).await?;
            assert!(!close.isHidden() && close.isEnabled());
            native.resignKeyWindow();
            pause(80).await?;
            edges(&window, (false, false))?;
            assert!(close.isHidden());
            window.hide()?;
            pause(30).await?;
            window.show()?;
            pause(80).await?;
            edges(&window, (false, false))?;
            native.makeKeyAndOrderFront(None);
            pause(80).await?;
            edges(&window, (false, false))?;
            // Reduced motion applies visibility immediately and still honors keyboard holds.
            window.set_prefers_reduced_motion(true);
            window.set_chrome_active(true);
            window.set_header_hovered(true);
            window.set_footer_hovered(true);
            pause(30).await?;
            assert_eq!(window.get_header_opacity(), 1.0);
            assert_eq!(window.get_footer_opacity(), 1.0);
            capture(&window, "reduced-motion")?;
            window.invoke_reset_chrome();
            pause(30).await?;
            assert_eq!(window.get_header_opacity(), 0.0);
            assert!(close.isHidden());
            // Cancel a pending focus request before its first visible frame.
            key(&window, Key::F6.into());
            key(&window, Key::Escape.into());
            pause(360).await?;
            edges(&window, (false, false))?;
            key(&window, Key::F6.into());
            window.invoke_reset_chrome();
            pause(60).await?;
            edges(&window, (false, false))?;
            key(&window, Key::F6.into());
            pause(30).await?;
            key(&window, " ".into());
            assert_eq!(pins.get(), 2);
            key(&window, Key::F6.into());
            pause(30).await?;
            key(&window, " ".into());
            assert_eq!(browses.get(), 2);
            key(&window, Key::F6.into());
            window.invoke_restore_editor_position(0, 0, 0.0);
            key(&window, "P".into());
            pause(30).await?;
            assert_eq!(window.get_note_text(), format!("P{source}"));
            undo(&window);
            pause(30).await?;
            assert_eq!(window.get_note_text(), source);
            // F6/Escape and lifecycle reset return to the active panel, not the hidden editor.
            window.set_active_view("pages".into());
            pause(40).await?;
            window.set_search_query("".into());
            key(&window, Key::F6.into());
            pause(50).await?;
            key(&window, Key::Escape.into());
            key(&window, "S".into());
            pause(30).await?;
            assert_eq!(window.get_search_query(), "S");
            key(&window, Key::F6.into());
            pause(50).await?;
            key(&window, Key::F6.into());
            key(&window, "T".into());
            pause(30).await?;
            assert_eq!(window.get_search_query(), "ST");
            key(&window, Key::F6.into());
            pause(50).await?;
            window.invoke_reset_chrome();
            key(&window, "U".into());
            pause(30).await?;
            assert_eq!(window.get_search_query(), "STU");
            assert_eq!(window.get_note_text(), source);
            window.set_active_view("note".into());
            pause(40).await?;
            let themes = settings.themes()?;
            let mut cases = 0;
            for (width, height) in [(400.0, 440.0), (505.0, 552.0)] {
                window
                    .window()
                    .set_size(slint::LogicalSize::new(width, height));
                for font in [11.0, 15.0, 24.0] {
                    window.set_font_size_px(font);
                    for (index, id) in ["paper", "graphite", "matcha", "rose", custom.as_str()]
                        .into_iter()
                        .enumerate()
                    {
                        let mut appearance = settings.appearance.clone();
                        appearance.light_theme = ThemeId(id.into());
                        appearance.font_size = font;
                        window.set_system_high_contrast(index == 3);
                        apply_theme(
                            &window,
                            &resolve_theme(&appearance, SystemAppearance::Light, &themes)?,
                        );
                        window
                            .set_paper_style(["plain", "ruled", "dots", "grid"][index % 4].into());
                        window.set_find_visible(index % 2 == 1);
                        window.set_chrome_active(true);
                        window.set_header_hovered(true);
                        window.set_footer_hovered(true);
                        pause(30).await?;
                        assert!(window.get_editor_line_height() > font);
                        assert_eq!(window.get_note_text(), source);
                        assert_eq!(window.get_primary_view_count(), 1);
                        if width == 505.0 && font == 15.0 {
                            capture(&window, &format!("theme-{index}"))?;
                        }
                        cases += 1;
                    }
                    capture(&window, &format!("matrix-{}-{font}", width as u32))?;
                }
            }
            // Capture both scroll extremes with the maximum font and the chrome visible.
            window.set_find_visible(false);
            window.set_paper_style("plain".into());
            window.set_font_size_px(24.0);
            window.invoke_focus_editor();
            for (width, height) in [(400.0, 440.0), (505.0, 552.0)] {
                window.window().set_size(slint::LogicalSize::new(width, height));
                window.invoke_restore_editor_position(0, 0, 0.0);
                pause(40).await?;
                capture(&window, &format!("clearance-{}-first", width as u32))?;
                window.window().dispatch_event(WindowEvent::PointerScrolled {
                    position: slint::LogicalPosition::new(width / 2.0, height / 2.0),
                    delta_x: 0.0,
                    delta_y: -100_000.0,
                });
                pause(40).await?;
                assert!(window.get_editor_scroll_y() > 0.0);
                assert_eq!(window.get_note_text(), source);
                capture(&window, &format!("clearance-{}-last", width as u32))?;
            }
            println!("floating_matrix_cases: {cases}");
            assert_eq!(unexpected_mouse.get(), 0, "physical pointer input reached the scripted chrome driver");
            println!("floating_chrome_contract_passed: true");
            Ok(())
        }
        .await;
        match check {
            Ok(()) => result.set(true),
            Err(error) => eprintln!("floating chrome contract: {error}"),
        }
        let _ = slint::quit_event_loop();
    })?;
    // Bound this owned fixture even if a future event-loop regression stalls a phase.
    slint::Timer::single_shot(Duration::from_secs(30), || {
        eprintln!("floating fixture deadline");
        let _ = slint::quit_event_loop();
    });
    window.show()?;
    application.activate();
    slint::run_event_loop_until_quit()?;
    if !passed.get() {
        return Err("floating chrome contract failed".into());
    }
    Ok(())
}
