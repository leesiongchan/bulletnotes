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
    let mut settings = AppSettings::default();
    settings.appearance.light_theme = bulletnotes::theme::ThemeId("paper".into());
    present_settings(&window, &settings, SystemAppearance::Light).unwrap();
    let mut theme = window.get_theme();
    theme.accent = slint::Color::from_rgb_u8(180, 0, 0);
    window.set_theme(theme);
    window.set_note_text(source.into());
    window.show().unwrap();
    window
        .window()
        .dispatch_event(WindowEvent::WindowActiveChanged(true));
    window.invoke_focus_editor();
    window.invoke_reset_chrome();
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
fn fenced_languages_highlight_tokens_without_changing_source() {
    use slint::Model;
    let source = "```rust\r\nlet café = \"日本語\"; // comment\r\n```\r\n";
    let window = setup(source);
    let spans = window.invoke_highlight_markdown(source.into(), window.get_theme(), false);
    let at = |offset: usize| {
        spans
            .iter()
            .find(|s| s.start <= offset as i32 && (offset as i32) < s.end)
            .map(|s| s.foreground)
    };
    assert_ne!(
        at(source.find("let").unwrap()),
        at(source.find("日本語").unwrap()),
        "keywords and strings need distinct styling"
    );
    assert_eq!(window.get_note_text(), source);
    let unknown = "```unknown-language\nlet x = \"literal\";\n```\n";
    let spans = window.invoke_highlight_markdown(unknown.into(), window.get_theme(), false);
    assert!(
        spans
            .iter()
            .filter(|s| s.start >= 20 && s.end < 42)
            .all(|s| s.foreground == window.get_theme().ink)
    );
}

#[test]
fn incomplete_code_unicode_ranges_and_budget_fallback_are_safe() {
    use slint::Model;
    let window = setup("");
    let source = "```rust\r\nlet café = \"日本語\";\r\n";
    let spans = window.invoke_highlight_markdown(source.into(), window.get_theme(), false);
    let mut end = 0;
    for span in spans.iter() {
        assert!(span.start >= end && span.end > span.start);
        assert!(
            source.is_char_boundary(span.start as usize)
                && source.is_char_boundary(span.end as usize)
        );
        end = span.end;
    }
    assert!(
        spans.row_count() > 2,
        "an unclosed fence still highlights valid code"
    );
    let source = format!("```rust\n{}\n```\n", "let value = \"text\";\n".repeat(2000));
    let spans = window.invoke_highlight_markdown(source.as_str().into(), window.get_theme(), false);
    assert!(
        spans
            .iter()
            .all(|span| span.end <= 7 || span.start as usize >= source.len() - 4),
        "oversized blocks fall back to plain code, retaining fence styling"
    );
    let source = format!(
        "{}\n```rust\nlet value = \"text\";\n```",
        "plain prose\n".repeat(12000)
    );
    let spans = window.invoke_highlight_markdown(source.as_str().into(), window.get_theme(), false);
    assert!(
        spans.iter().all(
            |span| span.end as usize <= source.find("let value").unwrap()
                || span.start as usize >= source.rfind("```").unwrap()
        ),
        "oversized notes retain source without tokenizing code"
    );
}

#[test]
fn unchanged_code_reuses_colors_at_new_unicode_offsets() {
    use slint::Model;
    let window = setup("");
    let code = "```rust\nlet value = \"text\";\n```\n";
    let initial: Vec<_> = window
        .invoke_highlight_markdown(code.into(), window.get_theme(), false)
        .iter()
        .collect();
    let prefix = "日本語\n\n";
    let shifted = format!("{prefix}{code}");
    let shifted: Vec<_> = window
        .invoke_highlight_markdown(shifted.as_str().into(), window.get_theme(), false)
        .iter()
        .collect();
    assert_eq!(initial.len(), shifted.len());
    for (original, shifted) in initial.iter().zip(shifted) {
        assert_eq!(shifted.start, original.start + prefix.len() as i32);
        assert_eq!(shifted.end, original.end + prefix.len() as i32);
        assert_eq!(shifted.foreground, original.foreground);
    }
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

#[test]
fn emphasis_styles_the_body_glyphs_while_preserving_source() {
    let window = setup("");
    for syntax in ["*", "**", "***", "~~"] {
        present_settings(&window, &AppSettings::default(), SystemAppearance::Light).unwrap();
        let source = format!("{syntax}MMMMMM{syntax}");
        window.set_note_text(source.as_str().into());
        let styled = window.window().take_snapshot().unwrap();
        window.on_highlight_markdown(|_, _, _| Default::default());
        let plain = window.window().take_snapshot().unwrap();
        let scale = window.window().scale_factor();
        // Inspect the middle of the word so dimming punctuation alone cannot pass.
        let middle = (45.0 + (syntax.len() as f32 + 1.0) * 9.0) * scale;
        assert!(
            styled
                .as_bytes()
                .as_chunks::<4>()
                .0
                .iter()
                .zip(plain.as_bytes().as_chunks::<4>().0)
                .enumerate()
                .any(|(i, (a, b))| {
                    let x = (i % styled.width() as usize) as f32;
                    let y = (i / styled.width() as usize) as f32;
                    x > middle
                        && x < middle + 30.0 * scale
                        && y > 32.0 * scale
                        && y < 56.0 * scale
                        && a != b
                }),
            "{syntax} must change the actual word's appearance"
        );
        assert_eq!(window.get_note_text(), source);
    }
}

#[test]
fn emphasis_punctuation_is_muted_and_struck_text_is_quiet() {
    use slint::Model;
    let window = setup("**Bold**\n~~Done~~");
    let spans = window.invoke_highlight_markdown(window.get_note_text(), window.get_theme(), false);
    let foreground_at = |offset| {
        spans
            .iter()
            .find(|span| span.start <= offset && offset < span.end)
            .map(|span| span.foreground)
    };
    assert_eq!(foreground_at(0), Some(window.get_theme().muted));
    assert_eq!(foreground_at(2), Some(window.get_theme().ink));
    assert_eq!(foreground_at(9), Some(window.get_theme().muted));
    assert_eq!(window.get_note_text(), "**Bold**\n~~Done~~");
}

#[test]
fn tartan_preserves_native_palette_components() {
    use bulletnotes::theme::ThemeId;
    use slint::Model;
    let source = "# First\n## Second\n### Third\n*Italic*\n~~Done~~\n[link](https://example.com)";
    let window = setup(source);
    let mut settings = AppSettings::default();
    settings.appearance.light_theme = ThemeId("antinote-tartan".into());
    present_settings(&window, &settings, SystemAppearance::Light).unwrap();
    let theme = window.get_theme();
    assert_eq!(theme.paper, slint::Color::from_rgb_u8(243, 241, 235));
    assert_eq!(theme.ink, slint::Color::from_rgb_u8(60, 40, 10));
    let spans = window.invoke_highlight_markdown(source.into(), theme.clone(), false);
    for (needle, rgb) in [
        ("First", (255, 120, 40)),
        ("Second", (200, 90, 30)),
        ("Third", (255, 160, 80)),
        ("*Italic", (130, 130, 130)),
        ("Done", (130, 130, 130)),
        ("https://example.com", (140, 70, 15)),
    ] {
        let at = source.find(needle).unwrap() as i32;
        let span = spans
            .iter()
            .find(|span| span.start <= at && at < span.end)
            .unwrap();
        assert_eq!(
            span.foreground,
            slint::Color::from_rgb_u8(rgb.0, rgb.1, rgb.2),
            "{needle}"
        );
    }
    let contrast = window.invoke_highlight_markdown(source.into(), theme.clone(), true);
    assert!(contrast.iter().all(|span| span.foreground == theme.ink));
    assert_eq!(window.get_note_text(), source);
}
