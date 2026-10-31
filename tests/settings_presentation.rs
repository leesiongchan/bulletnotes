use bulletnotes::{
    AppWindow,
    app::{appearance_ui, ui::present_settings},
    store::{StoreHandle, settings::AppSettings},
    theme::{AppearanceMode, Color, SystemAppearance, ThemeId, contrast::contrast_ratio},
};
use slint::ComponentHandle;
use std::{cell::Cell, rc::Rc};

fn window(threading: bool) -> AppWindow {
    slint::platform::set_platform(Box::new(i_slint_backend_testing::TestingBackend::new(
        i_slint_backend_testing::TestingBackendOptions {
            mock_time: !threading,
            threading,
            renderer_name: Some("skia".into()),
        },
    )))
    .unwrap();
    AppWindow::new().unwrap()
}

#[test]
fn recovery_is_keyboard_accessible_from_general_settings_at_minimum_size() {
    use slint::{
        LogicalSize,
        platform::{Key, WindowEvent},
    };
    let window = window(false);
    present_settings(&window, &AppSettings::default(), SystemAppearance::Light).unwrap();
    window.window().set_size(LogicalSize::new(400.0, 440.0));
    window.set_settings_category("general".into());
    window.set_active_view("appearance".into());
    let opened = Rc::new(Cell::new(false));
    let observed = opened.clone();
    window.on_show_recovery(move || observed.set(true));
    window.show().unwrap();
    for _ in 0..30 {
        let _ = window.window().take_snapshot().unwrap();
        let inner = i_slint_core::window::WindowInner::from_pub(window.window());
        let label = inner.focus_item.borrow().upgrade().and_then(|item| {
            item.accessible_string_property(
                i_slint_core::accessibility::AccessibleStringProperty::Label,
            )
        });
        if label.as_deref() == Some("Changes, Trash, and kept versions") {
            window.window().dispatch_event(WindowEvent::KeyPressed {
                text: Key::Return.into(),
            });
            window.window().dispatch_event(WindowEvent::KeyReleased {
                text: Key::Return.into(),
            });
            break;
        }
        window.window().dispatch_event(WindowEvent::KeyPressed {
            text: Key::Tab.into(),
        });
        window.window().dispatch_event(WindowEvent::KeyReleased {
            text: Key::Tab.into(),
        });
    }
    assert!(
        opened.get(),
        "recovery must remain reachable with the keyboard after leaving the footer"
    );
}

fn rgb(color: slint::Color) -> Color {
    Color {
        r: color.red(),
        g: color.green(),
        b: color.blue(),
    }
}

#[test]
fn visible_about_tracks_appearance_and_accessibility_without_reopening() {
    use bulletnotes::{AboutWindow, app::ui::present_about};
    let window = window(false);
    let about = AboutWindow::new().unwrap();
    let settings = AppSettings::default();
    present_settings(&window, &settings, SystemAppearance::Light).unwrap();
    window.show().unwrap();
    present_about(&about, &window);
    about.show().unwrap();
    let _ = window.window().take_snapshot().unwrap();
    let _ = about.window().take_snapshot().unwrap();
    window.set_prefers_reduced_transparency(true);
    let _ = window.window().take_snapshot().unwrap();
    let _ = about.window().take_snapshot().unwrap();
    assert!(about.get_reduce_transparency());
    window.set_system_high_contrast(true);
    present_settings(&window, &settings, SystemAppearance::Dark).unwrap();
    let _ = window.window().take_snapshot().unwrap();
    let _ = about.window().take_snapshot().unwrap();
    assert_eq!(about.get_theme(), window.get_theme());
    assert!(about.get_high_contrast());
    assert!(about.get_reduce_transparency());
    window.set_prefers_reduced_transparency(false);
    window.set_system_high_contrast(false);
    present_settings(&window, &settings, SystemAppearance::Light).unwrap();
    let _ = window.window().take_snapshot().unwrap();
    let _ = about.window().take_snapshot().unwrap();
    assert_eq!(about.get_theme(), window.get_theme());
    assert!(!about.get_high_contrast());
    assert!(!about.get_reduce_transparency());
}

#[test]
fn high_contrast_import_assignment_has_visible_keyboard_focus() {
    use bulletnotes::{ThemePair, theme::palette::ThemeEntry};
    use slint::{
        Model,
        platform::{Key, WindowEvent},
    };
    let window = window(false);
    let mut settings = AppSettings::default();
    let mut imported = settings
        .themes()
        .unwrap()
        .into_iter()
        .find(|t| t.id.0 == "paper")
        .unwrap()
        .theme;
    imported.name = "Invisible ink".into();
    imported.ink = imported.paper;
    let imported = ThemeEntry::import(imported);
    let id = imported.id.0.clone();
    settings.imported_themes.push(imported);
    window.set_system_high_contrast(true);
    present_settings(&window, &settings, SystemAppearance::Light).unwrap();
    let option = window
        .get_theme_options()
        .iter()
        .flat_map(|p| [p.left, p.right])
        .find(|t| t.id == id)
        .unwrap();
    window.set_theme_options(slint::ModelRc::from(
        [ThemePair {
            left: option.clone(),
            right: option,
            has_right: false,
        }]
        .as_slice(),
    ));
    window.show().unwrap();
    window.set_active_view("appearance".into());
    let _ = window.window().take_snapshot().unwrap();
    window.invoke_reveal_theme_choices();
    let image = window.window().take_snapshot().unwrap();
    let card_pixels = image
        .as_bytes()
        .as_chunks::<4>()
        .0
        .iter()
        .enumerate()
        .filter(|(_, p)| p[..3] == [252, 250, 244]);
    let (mut x, mut y) = (image.width() as usize, image.height() as usize);
    let mut right = 0;
    for (i, _) in card_pixels {
        x = x.min(i % image.width() as usize);
        right = right.max(i % image.width() as usize);
        y = y.min(i / image.width() as usize);
    }
    assert!(x < image.width() as usize && y < image.height() as usize);
    let tab = || {
        window.window().dispatch_event(WindowEvent::KeyPressed {
            text: Key::Tab.into(),
        });
        window.window().dispatch_event(WindowEvent::KeyReleased {
            text: Key::Tab.into(),
        });
    };
    let visible_ink = || {
        let image = window.window().take_snapshot().unwrap();
        (y + 8..y + 40)
            .flat_map(|y| (right - 41..right - 9).map(move |x| (x, y)))
            .filter(|(x, y)| {
                let i = (y * image.width() as usize + x) * 4;
                let p = &image.as_bytes()[i..i + 3];
                contrast_ratio(
                    Color {
                        r: p[0],
                        g: p[1],
                        b: p[2],
                    },
                    Color::parse("#F7F3EB").unwrap(),
                ) >= 7.0
            })
            .count()
    };
    let unfocused = visible_ink();
    tab();
    let focused = visible_ink();
    assert!(
        focused > 100,
        "the assignment icon and focus ring must remain visible over low-contrast imports: {focused}"
    );
    assert!(
        focused > unfocused,
        "keyboard focus must strengthen the assignment boundary"
    );
}

#[test]
fn settings_minimum_size_keeps_actions_reachable() {
    use slint::platform::{Key, WindowEvent};
    let window = window(false);
    present_settings(&window, &AppSettings::default(), SystemAppearance::Light).unwrap();
    window.set_note_text("unsaved café\r\n👩🏽‍💻".into());
    window.set_prefers_reduced_motion(true);
    let applied = Rc::new(Cell::new(false));
    let connected = Rc::new(Cell::new(false));
    let observed = applied.clone();
    window.on_apply_hotkey(move || observed.set(true));
    let observed = connected.clone();
    window.on_show_connection(move || observed.set(true));
    window.show().unwrap();
    window
        .window()
        .set_size(slint::LogicalSize::new(400.0, 440.0));
    window.set_settings_category("general".into());
    window.set_active_view("appearance".into());
    let key = |text: slint::SharedString| {
        window
            .window()
            .dispatch_event(WindowEvent::KeyPressed { text: text.clone() });
        window
            .window()
            .dispatch_event(WindowEvent::KeyReleased { text });
    };
    // Pass the Appearance category without activating it.
    key(Key::Tab.into());
    let mut reached_apply = false;
    for _ in 0..160 {
        let before = window.window().take_snapshot().unwrap();
        key(Key::Tab.into());
        let focused = window.window().take_snapshot().unwrap();
        key(Key::Space.into());
        if applied.get() && !reached_apply {
            assert_ne!(
                before.as_bytes(),
                focused.as_bytes(),
                "focused shortcut action must be visible after scrolling"
            );
            reached_apply = true;
        }
        if connected.get() {
            break;
        }
    }
    assert!(
        reached_apply && connected.get(),
        "shortcut and connection must be keyboard reachable"
    );
    assert_eq!(window.get_note_text(), "unsaved café\r\n👩🏽‍💻");
}

#[test]
fn selected_scheme_and_palette_apply_to_all_settings() {
    let window = window(false);
    let source = "# café\r\n日本語 👩🏽‍💻";
    window.set_note_text(source.into());
    let mut settings = AppSettings::default();
    settings.appearance.light_theme = ThemeId("matcha".into());
    settings.appearance.dark_theme = ThemeId("graphite".into());
    for (mode, system, dark) in [
        (AppearanceMode::Light, SystemAppearance::Light, false),
        (AppearanceMode::Light, SystemAppearance::Dark, false),
        (AppearanceMode::Dark, SystemAppearance::Light, true),
        (AppearanceMode::Dark, SystemAppearance::Dark, true),
        (AppearanceMode::System, SystemAppearance::Light, false),
        (AppearanceMode::System, SystemAppearance::Dark, true),
    ] {
        settings.appearance.mode = mode;
        present_settings(&window, &settings, system).unwrap();
        assert_eq!(window.get_theme().dark, dark);
        let expected = settings
            .themes()
            .unwrap()
            .into_iter()
            .find(|t| t.id.0 == if dark { "graphite" } else { "matcha" })
            .unwrap();
        assert_eq!(rgb(window.get_theme().paper), expected.theme.paper);
        assert_eq!(window.get_note_text().as_bytes(), source.as_bytes());
        let catalog = window.get_theme_options();
        let before = settings.clone();
        window.set_system_high_contrast(true);
        present_settings(&window, &settings, system).unwrap();
        let palette = window.get_theme();
        assert!(contrast_ratio(rgb(palette.ink), rgb(palette.paper)) >= 7.0);
        assert_eq!(window.get_theme_options(), catalog);
        assert_eq!(settings, before);
        assert_eq!(window.get_note_text().as_bytes(), source.as_bytes());
        window.set_system_high_contrast(false);
    }
}

#[test]
fn settings_callbacks_and_failed_save_restore_palette_and_preferences() {
    use slint::Model;
    let window = window(true);
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let _entered = runtime.enter();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("settings.sqlite");
    let store = runtime.block_on(StoreHandle::open(&path)).unwrap();
    let settings = AppSettings::default();
    runtime
        .block_on(store.save_settings(settings.clone()))
        .unwrap();
    let binding =
        appearance_ui::bind(&window, store.clone(), settings, SystemAppearance::Dark).unwrap();
    window.set_note_text("unsaved café\r\n👩🏽‍💻".into());
    window.show().unwrap();
    let completed = Rc::new(Cell::new(false));
    let done = completed.clone();
    slint::spawn_local(async move {
        window.set_appearance_mode("light".into());
        window.set_paper_type("bone".into());
        window.set_paper_style("grid".into());
        window.set_paper_intensity("bold".into());
        window.set_start_with_new_note(true);
        window.set_keep_bottom_bar_visible(true);
        window.set_show_note_count(false);
        window.set_show_note_date(false);
        window.set_show_newest_indicator(false);
        window.invoke_appearance_changed();
        binding.flush().await.unwrap();
        assert!(!window.get_theme().dark);
        binding.set_system_appearance(SystemAppearance::Dark);
        assert!(!window.get_theme().dark);
        let saved = store.load_settings().await.unwrap();
        assert_eq!(saved.appearance.paper_type, bulletnotes::theme::PaperType::Bone);
        assert_eq!(saved.appearance.paper, bulletnotes::theme::PaperStyle::Grid);
        assert_eq!(saved.appearance.paper_intensity, bulletnotes::theme::PaperIntensity::Bold);
        assert!(saved.start_with_new_note && saved.appearance.keep_bottom_bar_visible);
        assert!(!saved.appearance.show_note_count && !saved.appearance.show_note_date && !saved.appearance.show_newest_indicator);
        let catalog:Vec<_> = window.get_theme_options().iter().collect();
        let injector = rusqlite::Connection::open(path).unwrap();
        injector.execute_batch("CREATE TRIGGER fail_settings BEFORE UPDATE ON settings BEGIN SELECT RAISE(ABORT,'fixture failure'); END;").unwrap();
        window.set_appearance_mode("dark".into());
        window.set_paper_type("linen".into());
        window.set_paper_style("dots".into());
        window.set_paper_intensity("subtle".into());
        window.set_start_with_new_note(false);
        window.set_keep_bottom_bar_visible(false);
        window.set_show_note_count(true);
        window.set_show_note_date(true);
        window.set_show_newest_indicator(true);
        window.invoke_appearance_changed();
        binding.flush().await.unwrap();
        assert!(!window.get_theme().dark);
        assert_eq!(store.load_settings().await.unwrap(), saved);
        assert_eq!(window.get_paper_type(), "bone");
        assert_eq!(window.get_paper_style(), "grid");
        assert_eq!(window.get_paper_intensity(), "bold");
        assert!(window.get_start_with_new_note() && window.get_keep_bottom_bar_visible());
        assert!(!window.get_show_note_count() && !window.get_show_note_date() && !window.get_show_newest_indicator());
        assert_eq!(window.get_theme_options().iter().collect::<Vec<_>>(), catalog);
        assert_eq!(window.get_note_text(), "unsaved café\r\n👩🏽‍💻");
        assert!(window.get_appearance_message().contains("could not be saved"));
        store.shutdown().await.unwrap();
        done.set(true);
        slint::quit_event_loop().unwrap();
    }).unwrap();
    slint::Timer::single_shot(std::time::Duration::from_secs(15), || {
        slint::quit_event_loop().unwrap();
    });
    slint::run_event_loop_until_quit().unwrap();
    assert!(completed.get());
}

#[test]
fn bottom_bar_and_note_details_follow_preferences_without_touching_source() {
    let window = window(false);
    let mut settings = AppSettings::default();
    settings.appearance.keep_bottom_bar_visible = true;
    window.set_note_text("café 👩🏽‍💻".into());
    window.set_note_counter("2 of 5".into());
    window.set_note_date("31 Oct 2026".into());
    window.set_prefers_reduced_motion(true);
    window.show().unwrap();
    present_settings(&window, &settings, SystemAppearance::Light).unwrap();
    let shown = window.window().take_snapshot().unwrap();
    assert!(window.get_footer_revealed());
    assert_eq!(window.get_footer_opacity(), 1.0);
    settings.appearance.show_note_count = false;
    settings.appearance.show_note_date = false;
    present_settings(&window, &settings, SystemAppearance::Light).unwrap();
    let hidden = window.window().take_snapshot().unwrap();
    assert_ne!(shown.as_bytes(), hidden.as_bytes());
    assert_eq!(window.get_note_text(), "café 👩🏽‍💻");
}

#[test]
fn minimum_size_paper_controls_are_keyboard_reachable_and_visible() {
    use slint::platform::{Key, WindowEvent};
    let window = window(false);
    present_settings(&window, &AppSettings::default(), SystemAppearance::Light).unwrap();
    window.set_note_text("unsaved café\r\n👩🏽‍💻".into());
    window.set_prefers_reduced_motion(true);
    window.show().unwrap();
    window
        .window()
        .set_size(slint::LogicalSize::new(400.0, 440.0));
    window.set_active_view("appearance".into());
    let _ = window.window().take_snapshot().unwrap();
    let key = |text: slint::SharedString| {
        window
            .window()
            .dispatch_event(WindowEvent::KeyPressed { text: text.clone() });
        window
            .window()
            .dispatch_event(WindowEvent::KeyReleased { text });
    };
    let mut reached_bone = false;
    let mut reached_grid = false;
    let mut reached_bold = false;
    for _ in 0..32 {
        let before = window.window().take_snapshot().unwrap();
        key(Key::Tab.into());
        let focused = window.window().take_snapshot().unwrap();
        key(Key::Space.into());
        if window.get_paper_type() == "bone" && !reached_bone {
            assert_ne!(
                before.as_bytes(),
                focused.as_bytes(),
                "Bone focus must be visible"
            );
            reached_bone = true;
        }
        reached_grid |= window.get_paper_style() == "grid";
        reached_bold |= window.get_paper_intensity() == "bold";
        if reached_bone && reached_grid && reached_bold {
            break;
        }
    }
    assert!(reached_bone && reached_grid && reached_bold);
    assert_eq!(window.get_note_text(), "unsaved café\r\n👩🏽‍💻");
}

#[test]
fn selected_palette_is_used_by_every_app_view_and_about() {
    use bulletnotes::{AboutWindow, app::ui::present_about, theme::PaperType};
    let window = window(false);
    let about = AboutWindow::new().unwrap();
    let mut settings = AppSettings::default();
    settings.appearance.paper_type = PaperType::Smooth;
    settings.appearance.light_theme = ThemeId("matcha".into());
    settings.appearance.dark_theme = ThemeId("graphite".into());
    window.set_prefers_reduced_motion(true);
    window.set_note_text("unsaved café 👩🏽‍💻".into());
    window.show().unwrap();
    present_about(&about, &window);
    about.show().unwrap();
    for system in [SystemAppearance::Light, SystemAppearance::Dark] {
        present_settings(&window, &settings, system).unwrap();
        let expected = settings
            .themes()
            .unwrap()
            .into_iter()
            .find(|entry| {
                entry.id.0
                    == if system == SystemAppearance::Light {
                        "matcha"
                    } else {
                        "graphite"
                    }
            })
            .unwrap()
            .theme
            .paper;
        for view in ["note", "pages", "appearance", "connection", "recovery"] {
            window.set_active_view(view.into());
            let image = window.window().take_snapshot().unwrap();
            let i = (180 * image.width() as usize + 5) * 4;
            assert_eq!(
                &image.as_bytes()[i..i + 3],
                &[expected.r, expected.g, expected.b],
                "{view}/{system:?}"
            );
        }
        let _ = about.window().take_snapshot().unwrap();
        assert_eq!(about.get_theme(), window.get_theme());
        assert_eq!(window.get_note_text(), "unsaved café 👩🏽‍💻");
    }
}

#[test]
fn connection_remember_control_uses_the_shared_accent_and_respects_busy() {
    use slint::platform::{PointerEventButton, WindowEvent};
    let window = window(false);
    let mut settings = AppSettings::default();
    settings.appearance.light_theme = ThemeId("matcha".into());
    present_settings(&window, &settings, SystemAppearance::Light).unwrap();
    window.set_prefers_reduced_motion(true);
    window.show().unwrap();
    window.set_active_view("connection".into());
    let before = window.window().take_snapshot().unwrap();
    window.set_connection_remember(true);
    let selected = window.window().take_snapshot().unwrap();
    let accent = window.get_theme().accent;
    let rgb = [accent.red(), accent.green(), accent.blue()];
    let changed: Vec<_> = selected
        .as_bytes()
        .as_chunks::<4>()
        .0
        .iter()
        .zip(before.as_bytes().as_chunks::<4>().0.iter())
        .enumerate()
        .filter(|(_, (a, b))| a[..3] == rgb && b[..3] != rgb)
        .map(|(i, _)| (i % selected.width() as usize, i / selected.width() as usize))
        .collect();
    assert!(
        changed.len() > 200,
        "remember control must visibly use the selected accent: {}",
        changed.len()
    );
    let x = changed.iter().map(|p| p.0).sum::<usize>() as f32 / changed.len() as f32;
    let y = changed.iter().map(|p| p.1).sum::<usize>() as f32 / changed.len() as f32;
    let click = || {
        window.window().dispatch_event(WindowEvent::PointerPressed {
            position: slint::LogicalPosition::new(x, y),
            button: PointerEventButton::Left,
        });
        window
            .window()
            .dispatch_event(WindowEvent::PointerReleased {
                position: slint::LogicalPosition::new(x, y),
                button: PointerEventButton::Left,
            });
    };
    window.set_connection_busy(true);
    let _ = window.window().take_snapshot().unwrap();
    click();
    assert!(window.get_connection_remember());
    window.set_connection_busy(false);
    let _ = window.window().take_snapshot().unwrap();
    click();
    assert!(!window.get_connection_remember());
}

#[test]
fn about_standard_widget_text_remains_readable_in_both_schemes() {
    use bulletnotes::{AboutWindow, app::ui::present_about};
    let window = window(false);
    let about = AboutWindow::new().unwrap();
    let mut settings = AppSettings::default();
    window.show().unwrap();
    present_about(&about, &window);
    about.show().unwrap();
    for id in ["graphite", "matcha"] {
        settings.appearance.light_theme = ThemeId(id.into());
        present_settings(&window, &settings, SystemAppearance::Light).unwrap();
        let _ = window.window().take_snapshot().unwrap();
        let image = about.window().take_snapshot().unwrap();
        let paper = rgb(about.get_theme().paper);
        let readable = (image.height() as usize - 60..image.height() as usize - 34)
            .flat_map(|y| (100..260).map(move |x| (x, y)))
            .filter(|(x, y)| {
                let i = (y * image.width() as usize + x) * 4;
                let p = &image.as_bytes()[i..i + 3];
                contrast_ratio(
                    Color {
                        r: p[0],
                        g: p[1],
                        b: p[2],
                    },
                    paper,
                ) >= 4.5
            })
            .count();
        assert!(
            readable > 50,
            "About attribution text must follow its own window's {id} scheme: {readable}"
        );
    }
}

#[test]
fn theme_tiles_are_square_and_the_palette_strip_is_contiguous() {
    use bulletnotes::ThemePair;
    use slint::Model;
    let window = window(false);
    present_settings(&window, &AppSettings::default(), SystemAppearance::Light).unwrap();
    let option = window
        .get_theme_options()
        .iter()
        .flat_map(|pair| [pair.left, pair.right])
        .find(|item| item.id == "paper")
        .unwrap();
    let swatches = &option.swatches;
    let colors = [
        swatches.primary,
        swatches.secondary,
        swatches.tertiary,
        swatches.accent2,
        swatches.accent3,
        swatches.accent4,
        swatches.accent5,
        swatches.highlight,
    ]
    .map(|c| [c.red(), c.green(), c.blue()]);
    window.set_theme_options(slint::ModelRc::from(
        [ThemePair {
            left: option.clone(),
            right: option,
            has_right: false,
        }]
        .as_slice(),
    ));
    window.set_prefers_reduced_motion(true);
    window.show().unwrap();
    window.set_active_view("appearance".into());
    let _ = window.window().take_snapshot().unwrap();
    window.invoke_reveal_theme_choices();
    let image = window.window().take_snapshot().unwrap();
    let mut left = image.width() as usize;
    let mut top = image.height() as usize;
    let mut right = 0;
    let mut bottom = 0;
    for (i, _) in image
        .as_bytes()
        .as_chunks::<4>()
        .0
        .iter()
        .enumerate()
        .filter(|(_, p)| p[..3] == [252, 250, 244])
    {
        left = left.min(i % image.width() as usize);
        right = right.max(i % image.width() as usize);
        top = top.min(i / image.width() as usize);
        bottom = bottom.max(i / image.width() as usize);
    }
    assert!(left < right && top < bottom);
    // Fractional placement can change the fully covered bounds by one physical pixel.
    assert!(
        (right - left).abs_diff(bottom - top) <= 1,
        "theme preview must be square"
    );
    let start = left + 10;
    let end = right - 9;
    let y = bottom - 20;
    let pixel = |x| {
        let i = (y * image.width() as usize + x) * 4;
        let p = &image.as_bytes()[i..i + 3];
        [p[0], p[1], p[2]]
    };
    for x in start + 1..end - 1 {
        assert!(
            colors.contains(&pixel(x)),
            "palette strip must have no gaps"
        );
    }
    for (index, color) in colors.iter().enumerate() {
        let x = start + (2 * index + 1) * (end - start) / 16;
        assert_eq!(&pixel(x), color, "palette segment {index}");
    }
}

#[test]
fn theme_card_patterns_follow_the_shared_paper_intensity() {
    use bulletnotes::{
        ThemePair,
        theme::{PaperIntensity, PaperStyle},
    };
    use slint::Model;
    let window = window(false);
    let mut settings = AppSettings::default();
    settings.appearance.paper = PaperStyle::Grid;
    window.set_prefers_reduced_motion(true);
    window.show().unwrap();
    window.set_active_view("appearance".into());
    let mut previous = None;
    for intensity in [
        PaperIntensity::Subtle,
        PaperIntensity::Clear,
        PaperIntensity::Bold,
    ] {
        settings.appearance.paper_intensity = intensity;
        present_settings(&window, &settings, SystemAppearance::Light).unwrap();
        let option = window
            .get_theme_options()
            .iter()
            .flat_map(|pair| [pair.left, pair.right])
            .find(|item| item.id == "paper")
            .unwrap();
        window.set_theme_options(slint::ModelRc::from(
            [ThemePair {
                left: option.clone(),
                right: option,
                has_right: false,
            }]
            .as_slice(),
        ));
        let _ = window.window().take_snapshot().unwrap();
        window.invoke_reveal_theme_choices();
        let image = window.window().take_snapshot().unwrap();
        let mut left = image.width() as usize;
        let mut top = image.height() as usize;
        for (i, _) in image
            .as_bytes()
            .as_chunks::<4>()
            .0
            .iter()
            .enumerate()
            .filter(|(_, p)| p[..3] == [252, 250, 244])
        {
            left = left.min(i % image.width() as usize);
            top = top.min(i / image.width() as usize);
        }
        let crop: Vec<_> = (top + 60..top + 104)
            .flat_map(|y| (left + 14..left + 114).map(move |x| (x, y)))
            .flat_map(|(x, y)| {
                let i = (y * image.width() as usize + x) * 4;
                image.as_bytes()[i..i + 3].to_vec()
            })
            .collect();
        if let Some(old) = previous {
            assert!(crop != old, "theme card must update for {intensity:?}");
        }
        previous = Some(crop);
    }
}
