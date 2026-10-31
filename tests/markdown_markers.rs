use bulletnotes::{
    AppWindow, app::ui::present_settings, store::settings::AppSettings, theme::SystemAppearance,
};
use slint::{ComponentHandle, Model, platform::WindowEvent};
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
    window.set_note_text(source.into());
    window.show().unwrap();
    window
        .window()
        .dispatch_event(WindowEvent::WindowActiveChanged(true));
    window.invoke_focus_editor();
    window.invoke_select_find_match(source.len() as i32, source.len() as i32);
    window
}

fn snapshot(window: &AppWindow) -> Vec<u8> {
    window.window().take_snapshot().unwrap().as_bytes().to_vec()
}

fn key(window: &AppWindow, text: slint::SharedString, pressed: bool) {
    window.window().dispatch_event(if pressed {
        WindowEvent::KeyPressed { text }
    } else {
        WindowEvent::KeyReleased { text }
    });
}

#[test]
fn loaded_and_replaced_markers_preserve_source_without_edit_events() {
    let source = "- [ ] café 👩🏽‍💻\r\n- [x] Done\r\n- List 日本語\r\n";
    let window = setup(source);
    let edits = Rc::new(Cell::new(0));
    let observed = edits.clone();
    window.on_note_edited(move |_| observed.set(observed.get() + 1));
    let painted = snapshot(&window);
    window.on_mark_markdown(|_, _| Default::default());
    assert!(
        painted != snapshot(&window),
        "loaded source must render markers"
    );
    assert_eq!(window.get_note_text().as_bytes(), source.as_bytes());

    present_settings(&window, &AppSettings::default(), SystemAppearance::Light).unwrap();
    let replacement = "ordinary\n- replacement\n";
    window.set_note_text(replacement.into());
    window.invoke_select_find_match(replacement.len() as i32, replacement.len() as i32);
    let painted = snapshot(&window);
    window.on_mark_markdown(|_, _| Default::default());
    assert!(
        painted != snapshot(&window),
        "replacing the source must refresh markers"
    );
    assert_eq!(window.get_note_text(), replacement);
    assert_eq!(edits.get(), 0);
}

#[test]
fn typed_task_states_and_undo_redo_refresh_the_actual_editor() {
    use slint::language::TextInputMarkerKind as Kind;
    use slint::platform::Key;
    let window = setup("Task");
    window.invoke_select_find_match(0, 0);
    key(&window, "- [ ] ".into(), true);
    key(&window, "- [ ] ".into(), false);
    assert_eq!(window.get_note_text(), "- [ ] Task");
    window.invoke_select_find_match(3, 4);
    key(&window, "x".into(), true);
    key(&window, "x".into(), false);
    assert_eq!(window.get_note_text(), "- [x] Task");
    window.invoke_select_find_match(10, 10);
    key(&window, "!".into(), true);
    key(&window, "!".into(), false);
    assert_eq!(window.get_note_text(), "- [x] Task!");
    let checked = snapshot(&window);
    let markers = window.invoke_mark_markdown(window.get_note_text(), window.get_theme());
    assert_eq!(markers.row_data(0).unwrap().kind, Kind::CheckedCheckbox);
    let mut theme = window.get_theme();
    theme.ink = slint::Color::from_rgb_u8(0, 0, 180);
    window.set_theme(theme);
    let _ = snapshot(&window);
    key(&window, Key::Control.into(), true);
    key(&window, "z".into(), true);
    key(&window, "z".into(), false);
    key(&window, Key::Control.into(), false);
    assert_eq!(window.get_note_text(), "- [x] Task");
    key(&window, Key::Control.into(), true);
    if cfg!(target_os = "windows") {
        key(&window, "y".into(), true);
        key(&window, "y".into(), false);
    } else {
        key(&window, Key::Shift.into(), true);
        key(&window, "z".into(), true);
        key(&window, "z".into(), false);
        key(&window, Key::Shift.into(), false);
    }
    key(&window, Key::Control.into(), false);
    assert_eq!(window.get_note_text(), "- [x] Task!");
    window.invoke_select_find_match(11, 11);
    let painted = snapshot(&window);
    window.on_mark_markdown(|_, _| Default::default());
    assert!(
        painted != snapshot(&window),
        "edited source must paint a task box"
    );
    assert!(checked != painted, "theme changes must recolor the editor");
}

#[test]
fn caret_and_backwards_selection_reveal_raw_syntax_after_theme_changes() {
    let source = "- [ ] café 日本語";
    let window = setup(source);
    for (anchor, caret) in [(2, 2), (source.len() as i32, 2)] {
        present_settings(&window, &AppSettings::default(), SystemAppearance::Light).unwrap();
        window.invoke_select_find_match(anchor, caret);
        let visible = snapshot(&window);
        window.on_mark_markdown(|_, _| Default::default());
        assert!(
            visible == snapshot(&window),
            "editing a prefix must show exact raw syntax"
        );
    }
    present_settings(&window, &AppSettings::default(), SystemAppearance::Light).unwrap();
    let mut theme = window.get_theme();
    theme.ink = slint::Color::from_rgb_u8(0, 0, 180);
    window.set_theme(theme);
    let _ = snapshot(&window);
    key(&window, "X".into(), true);
    key(&window, "X".into(), false);
    assert_eq!(window.get_note_text(), "- X");
}

#[test]
fn marker_colors_follow_ink_and_code_remains_raw() {
    let source = "- [ ] Task\n";
    let window = setup(source);
    let mut theme = window.get_theme();
    theme.ink = slint::Color::from_rgb_u8(0, 0, 180);
    window.set_theme(theme.clone());
    window.set_system_high_contrast(true);
    let painted = snapshot(&window);
    let markers = window.invoke_mark_markdown(source.into(), theme);
    assert_eq!(markers.row_count(), 1);
    assert_eq!(
        markers.row_data(0).unwrap().foreground,
        slint::Color::from_rgb_u8(0, 0, 180)
    );
    window.on_mark_markdown(|_, _| Default::default());
    let raw = snapshot(&window);
    assert!(
        painted
            .as_chunks::<4>()
            .0
            .iter()
            .zip(raw.as_chunks::<4>().0)
            .any(|(a, b)| {
                a[2] > 100 && a[0] < 70 && a[1] < 70 && (b[2] <= 100 || b[0] >= 70 || b[1] >= 70)
            }),
        "painted marker pixels must use the updated blue ink"
    );
    present_settings(&window, &AppSettings::default(), SystemAppearance::Light).unwrap();
    let code = "---\nitems: - [ ] metadata\n---\n```\n- [ ] code\n```\n\\- escaped\n";
    window.set_note_text(code.into());
    window.invoke_select_find_match(code.len() as i32, code.len() as i32);
    let raw = snapshot(&window);
    window.on_mark_markdown(|_, _| Default::default());
    assert!(
        raw == snapshot(&window),
        "code and metadata must keep literal syntax"
    );
    assert_eq!(window.get_note_text(), code);
}

#[test]
fn tasks_and_bullets_align_their_body_text_in_compact_slots() {
    let source = "- [ ] MMMM\n- MMMM\n";
    let window = setup(source);
    let image = window.window().take_snapshot().unwrap();
    let scale = window.window().scale_factor();
    let line_height = window.get_editor_line_height() * scale;
    let ink = window.get_theme().ink;
    let last_body_pixel = |row| {
        image
            .as_bytes()
            .as_chunks::<4>()
            .0
            .iter()
            .enumerate()
            .filter_map(|(index, pixel)| {
                let x = index % image.width() as usize;
                let y = (index / image.width() as usize) as f32;
                (y >= 32.0 * scale + row as f32 * line_height
                    && y < 32.0 * scale + (row + 1) as f32 * line_height
                    && pixel[0..3] == [ink.red(), ink.green(), ink.blue()])
                .then_some(x)
            })
            .max()
            .unwrap()
    };
    assert!(last_body_pixel(0).abs_diff(last_body_pixel(1)) <= 1);
    assert_eq!(window.get_note_text().as_bytes(), source.as_bytes());
}

#[test]
fn editor_rows_follow_font_size_spacing() {
    let source = "MMMM\nMMMM\n";
    let window = setup(source);
    for (paper, spacing) in [("plain", 1.6), ("ruled", 1.8)] {
        window.set_paper_style(paper.into());
        for font in [11.0, 15.0, 24.0] {
            window.set_font_size_px(font);
            let _ = snapshot(&window);
            assert!(
                (window.get_editor_line_height() - font * spacing).abs() < 0.1,
                "{paper} {font}pt text must use {spacing}em rows; got {}",
                window.get_editor_line_height()
            );
        }
    }
    assert_eq!(window.get_note_text(), source);
}

#[test]
fn grid_pitch_and_text_inset_match_editor_geometry() {
    let window = setup("MMMM\nMMMM\n");
    window.set_font_size_px(14.0);
    window.set_paper_style("grid".into());
    let mut theme = window.get_theme();
    theme.paper = slint::Color::from_rgb_u8(255, 255, 255);
    theme.pattern = slint::Color::from_rgb_u8(0, 0, 255);
    theme.ink = slint::Color::from_rgb_u8(255, 0, 0);
    // Make a visible caret match the ink on every platform, including macOS.
    theme.cursor = theme.ink;
    window.set_theme(theme);
    // Exclude the trailing blank line's caret from the text-row measurement.
    window
        .window()
        .dispatch_event(WindowEvent::WindowActiveChanged(false));
    let image = window.window().take_snapshot().unwrap();
    let scale = window.window().scale_factor();
    let pixels = image.as_bytes().as_chunks::<4>().0;
    // Glyph bearings vary with the platform font; the inset is a layout contract.
    let inner = i_slint_core::window::WindowInner::from_pub(window.window());
    let input = inner.focus_item.borrow().upgrade().unwrap();
    assert!(input.downcast::<i_slint_core::items::TextInput>().is_some());
    assert_eq!(input.map_to_window(input.geometry().origin).x, 45.0);
    let ink_rows: Vec<_> = (0..image.height() as usize)
        .filter(|&y| {
            pixels[y * image.width() as usize..(y + 1) * image.width() as usize]
                .iter()
                .any(|p| p[0] > 180 && p[1] < 80 && p[2] < 80)
        })
        .collect();
    let row_starts: Vec<_> = ink_rows
        .iter()
        .enumerate()
        .filter_map(|(i, &y)| (i == 0 || y > ink_rows[i - 1] + 1).then_some(y as f32 / scale))
        .collect();
    assert_eq!(row_starts.len(), 2);
    assert!(
        (row_starts[1] - row_starts[0] - 22.4).abs() <= 0.65,
        "rendered text rows: {row_starts:?}"
    );
    let y = (300.0 * scale) as usize;
    let columns: Vec<_> = (0..image.width() as usize)
        .filter(|&x| {
            let p = pixels[y * image.width() as usize + x];
            p[2] > 180 && p[0] < 250 && p[1] < 250
        })
        .collect();
    let starts: Vec<_> = columns
        .iter()
        .enumerate()
        .filter_map(|(i, &x)| (i == 0 || x > columns[i - 1] + 1).then_some(x as f32 / scale))
        .collect();
    assert!(starts.len() > 10);
    assert!(
        starts
            .windows(2)
            .all(|x| (x[1] - x[0] - 27.0).abs() <= 0.65),
        "grid columns: {starts:?}"
    );
}
