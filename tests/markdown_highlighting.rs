use bulletnotes::{
    AppWindow, app::ui::present_settings, store::settings::AppSettings, theme::SystemAppearance,
};
use slint::{ComponentHandle, platform::WindowEvent};
use std::{cell::Cell, rc::Rc};

fn setup(source: &str) -> AppWindow {
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
    let mut theme = window.get_theme();
    theme.accent = slint::Color::from_rgb_u8(180, 0, 0);
    window.set_theme(theme);
    window.set_note_text(source.into());
    window.show().unwrap();
    window
        .window()
        .dispatch_event(WindowEvent::WindowActiveChanged(true));
    window.invoke_focus_editor();
    window
}

fn red_pixels(window: &AppWindow) -> usize {
    let pixels = window.window().take_snapshot().unwrap();
    pixels
        .as_bytes()
        .as_chunks::<4>()
        .0
        .iter()
        .enumerate()
        .filter(|(i, p)| {
            let y = *i / pixels.width() as usize;
            y > 30
                && y < pixels.height() as usize - 100
                && p[0] > 100
                && p[1] < 70
                && p[2] < 70
                && p[3] > 150
        })
        .count()
}

#[test]
fn loaded_and_replaced_source_receive_colors_without_edit_events() {
    let source = "# café 👩🏽‍💻\r\n\n[site](https://example.com)\r\n";
    let window = setup(source);
    let edits = Rc::new(Cell::new(0));
    let observed = edits.clone();
    window.on_note_edited(move |_| observed.set(observed.get() + 1));
    assert!(
        red_pixels(&window) > 10,
        "loaded Markdown should be colored"
    );
    assert_eq!(window.get_note_text().as_bytes(), source.as_bytes());
    window.set_note_text("ordinary source 日本語".into());
    assert_eq!(red_pixels(&window), 0);
    window.set_note_text("## replacement\n".into());
    assert!(red_pixels(&window) > 10);
    window.set_note_text("[link](https://example.org)".into());
    assert!(red_pixels(&window) > 10, "a link alone should be colored");
    assert_eq!(edits.get(), 0);
}

#[test]
fn typing_recomputes_ranges_using_the_actual_input_binding() {
    let window = setup("ordinary");
    assert_eq!(red_pixels(&window), 0);
    window.invoke_select_find_match(0, 0);
    window
        .window()
        .dispatch_event(WindowEvent::KeyPressed { text: "# ".into() });
    window
        .window()
        .dispatch_event(WindowEvent::KeyReleased { text: "# ".into() });
    assert_eq!(window.get_note_text(), "# ordinary");
    assert!(red_pixels(&window) > 10);
}

#[test]
fn source_is_not_colored_inside_code_blocks() {
    let window = setup("```\n# literal\n[link](https://example.com)\n```\n");
    assert_eq!(red_pixels(&window), 0);
}

#[test]
fn theme_updates_and_high_contrast_recolor_without_touching_source() {
    let source = "# Heading\n[link](https://example.com)\n";
    let window = setup(source);
    assert!(red_pixels(&window) > 10);
    let mut theme = window.get_theme();
    theme.accent = theme.paper;
    window.set_theme(theme.clone());
    assert_eq!(
        red_pixels(&window),
        0,
        "low contrast accent should fall back to ink"
    );
    theme.accent = slint::Color::from_rgb_u8(180, 0, 0);
    window.set_theme(theme);
    assert!(red_pixels(&window) > 10);
    window.set_system_high_contrast(true);
    assert_eq!(red_pixels(&window), 0);
    assert_eq!(window.get_note_text(), source);
}

#[test]
fn frontmatter_metadata_remains_plain() {
    let window =
        setup("---\n# metadata comment\nurl: [literal](https://example.com)\n---\nordinary\n");
    assert_eq!(red_pixels(&window), 0);
}

#[test]
fn recoloring_preserves_a_backwards_unicode_selection() {
    let source = "# café 日本語";
    let window = setup(source);
    window.invoke_select_find_match(source.len() as i32, 2);
    let mut theme = window.get_theme();
    theme.accent = slint::Color::from_rgb_u8(0, 0, 180);
    window.set_theme(theme);
    let _ = window.window().take_snapshot().unwrap();
    window
        .window()
        .dispatch_event(WindowEvent::KeyPressed { text: "X".into() });
    window
        .window()
        .dispatch_event(WindowEvent::KeyReleased { text: "X".into() });
    assert_eq!(window.get_note_text(), "# X");
}

#[test]
fn undo_and_redo_survive_theme_recoloring() {
    use slint::platform::Key;
    let window = setup("# Heading");
    window.invoke_select_find_match(9, 9);
    let key = |text: slint::SharedString, pressed| {
        window.window().dispatch_event(if pressed {
            WindowEvent::KeyPressed { text }
        } else {
            WindowEvent::KeyReleased { text }
        });
    };
    key("X".into(), true);
    key("X".into(), false);
    assert_eq!(window.get_note_text(), "# HeadingX");
    let mut theme = window.get_theme();
    theme.accent = slint::Color::from_rgb_u8(0, 0, 180);
    window.set_theme(theme);
    let _ = window.window().take_snapshot().unwrap();
    key(Key::Control.into(), true);
    key("z".into(), true);
    key("z".into(), false);
    key(Key::Control.into(), false);
    assert_eq!(window.get_note_text(), "# Heading");
    key(Key::Control.into(), true);
    if cfg!(target_os = "windows") {
        key("y".into(), true);
        key("y".into(), false);
    } else {
        key(Key::Shift.into(), true);
        key("z".into(), true);
        key("z".into(), false);
        key(Key::Shift.into(), false);
    }
    key(Key::Control.into(), false);
    assert_eq!(window.get_note_text(), "# HeadingX");
}
