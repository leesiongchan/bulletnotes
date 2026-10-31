use bulletnotes::{
    AppWindow, app::ui::present_settings, store::settings::AppSettings, theme::SystemAppearance,
};
use slint::{ComponentHandle, platform::WindowEvent};
use std::time::Duration;

fn window() -> AppWindow {
    slint::platform::set_platform(Box::new(i_slint_backend_testing::TestingBackend::new(
        i_slint_backend_testing::TestingBackendOptions {
            mock_time: true,
            threading: false,
            renderer_name: Some("skia".into()),
        },
    )))
    .unwrap();
    let window = AppWindow::new().unwrap();
    present_settings(&window, &AppSettings::default(), SystemAppearance::Light).unwrap();
    window.set_note_text("A long note\n".repeat(120).into());
    window.show().unwrap();
    window.invoke_reset_chrome();
    window
}

fn edge_ink(window: &AppWindow) -> usize {
    let pixels = window.window().take_snapshot().unwrap();
    let paper = window.get_theme().paper;
    pixels
        .as_bytes()
        .as_chunks::<4>()
        .0
        .iter()
        .enumerate()
        .filter(|(index, p)| {
            let x = *index % pixels.width() as usize;
            let y = *index / pixels.width() as usize;
            x > pixels.width() as usize - 12
                && x < pixels.width() as usize - 2
                && y > 50
                && y < pixels.height() as usize - 70
                && (p[0].abs_diff(paper.red()) > 10
                    || p[1].abs_diff(paper.green()) > 10
                    || p[2].abs_diff(paper.blue()) > 10)
        })
        .count()
}

#[test]
fn scrollbar_hides_when_idle_and_reappears_on_scroll_and_edge_hover() {
    let window = window();
    let _ = edge_ink(&window);
    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(950));
    let _ = edge_ink(&window);
    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(220));
    assert_eq!(edge_ink(&window), 0, "idle scrollbar must disappear");
    window.invoke_restore_editor_position(0, 0, 100.0);
    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(20));
    assert!(edge_ink(&window) > 0, "scrolling reveals the thumb");
    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(950));
    let _ = edge_ink(&window);
    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(220));
    assert_eq!(edge_ink(&window), 0);
    let size = window.window().size();
    window.window().dispatch_event(WindowEvent::PointerMoved {
        position: slint::LogicalPosition::new(size.width as f32 - 6.0, 120.0),
    });
    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(20));
    assert!(
        edge_ink(&window) > 0,
        "hidden thumb retains its hover target"
    );
    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(1400));
    window.window().dispatch_event(WindowEvent::PointerMoved {
        position: slint::LogicalPosition::new(200.0, 120.0),
    });
    let _ = edge_ink(&window);
    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(300));
    assert!(
        edge_ink(&window) > 0,
        "leaving the edge starts a fresh idle delay"
    );
    let position = slint::LogicalPosition::new(size.width as f32 - 6.0, 120.0);
    window.window().dispatch_event(WindowEvent::PointerPressed {
        position,
        button: slint::platform::PointerEventButton::Left,
    });
    let previous = window.get_editor_scroll_y();
    let position = slint::LogicalPosition::new(size.width as f32 - 6.0, 180.0);
    window
        .window()
        .dispatch_event(WindowEvent::PointerMoved { position });
    assert!(
        window.get_editor_scroll_y() > previous,
        "the revealed scrollbar remains draggable"
    );
    window
        .window()
        .dispatch_event(WindowEvent::PointerReleased {
            position,
            button: slint::platform::PointerEventButton::Left,
        });
    window.set_prefers_reduced_motion(true);
    window.window().dispatch_event(WindowEvent::PointerMoved {
        position: slint::LogicalPosition::new(200.0, 120.0),
    });
    let _ = edge_ink(&window);
    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(950));
    assert_eq!(
        edge_ink(&window),
        0,
        "reduced motion hides immediately after idle"
    );
    window.set_note_text("short note".into());
    let _ = edge_ink(&window);
    window.window().dispatch_event(WindowEvent::PointerMoved {
        position: slint::LogicalPosition::new(size.width as f32 - 6.0, 120.0),
    });
    assert_eq!(edge_ink(&window), 0, "short notes never show a scrollbar");
}

#[test]
fn code_panels_follow_wrapping_empty_lines_and_eof_without_moving_selection() {
    use slint::Model;
    let window = window();
    let source = format!(
        "# notes\n\n```rust\nlet message = \"{}\";\n\n```",
        "wrapped 日本語 ".repeat(12)
    );
    window.set_note_text(source.as_str().into());
    window.invoke_select_find_match(9, 2);
    let _ = window.window().take_snapshot().unwrap();
    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(20));
    let _ = window.window().take_snapshot().unwrap();
    let panels = window.get_code_panels();
    assert_eq!(panels.row_count(), 1);
    let panel = panels.row_data(0).unwrap();
    assert!(
        panel.bottom - panel.top > 4.0 * 24.0,
        "wrapped code must occupy multiple measured rows: {panel:?}"
    );
    assert_eq!(
        (window.get_editor_anchor(), window.get_editor_caret()),
        (9, 2)
    );
    assert_eq!(window.get_note_text(), source);
    let code = "```rust\r\nlet x = 1;\r\n\r\n```\r\nafter";
    window.set_note_text(code.into());
    let _ = window.window().take_snapshot().unwrap();
    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(20));
    let panel = window.get_code_panels().row_data(0).unwrap();
    assert!(
        panel.bottom > window.invoke_measure_code_offset(code.rfind("```").unwrap() as i32),
        "background must include the closing fence before CRLF"
    );
    window.set_note_text(source.as_str().into());
    let _ = window.window().take_snapshot().unwrap();
    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(20));
    let panel = window.get_code_panels().row_data(0).unwrap();
    window
        .window()
        .set_size(slint::LogicalSize::new(400.0, 440.0));
    let _ = window.window().take_snapshot().unwrap();
    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(20));
    let _ = window.window().take_snapshot().unwrap();
    assert!(
        window.get_code_panels().row_data(0).unwrap().bottom > panel.bottom,
        "narrower viewport must remeasure wrapped rows"
    );
    let previous_height = window.get_code_panels().row_data(0).unwrap().bottom;
    window.set_font_size_px(24.0);
    let _ = window.window().take_snapshot().unwrap();
    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(20));
    let _ = window.window().take_snapshot().unwrap();
    assert!(
        window.get_code_panels().row_data(0).unwrap().bottom > previous_height,
        "text-size changes must remeasure code panels"
    );
}
