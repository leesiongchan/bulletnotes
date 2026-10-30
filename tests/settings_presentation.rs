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
    assert_eq!(about.get_theme(), window.get_settings_theme());
    assert!(about.get_high_contrast());
    assert!(about.get_reduce_transparency());
    window.set_prefers_reduced_transparency(false);
    window.set_system_high_contrast(false);
    present_settings(&window, &settings, SystemAppearance::Light).unwrap();
    let _ = window.window().take_snapshot().unwrap();
    let _ = about.window().take_snapshot().unwrap();
    assert_eq!(about.get_theme(), window.get_settings_theme());
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
    for (i, _) in card_pixels {
        x = x.min(i % image.width() as usize);
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
        (y + 7..y + 39)
            .flat_map(|y| (x + 165..x + 197).map(move |x| (x, y)))
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
    tab();
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
    window.set_active_view("appearance".into());
    let key = |text: slint::SharedString| {
        window
            .window()
            .dispatch_event(WindowEvent::KeyPressed { text: text.clone() });
        window
            .window()
            .dispatch_event(WindowEvent::KeyReleased { text });
    };
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
fn settings_scheme_follows_app_mode_not_note_palette() {
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
        assert_eq!(window.get_settings_theme().dark, dark);
        assert_eq!(
            rgb(window.get_settings_theme().paper).hex(),
            if dark { "#25272B" } else { "#F7F3EB" }
        );
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
        for palette in [window.get_theme(), window.get_settings_theme()] {
            assert!(contrast_ratio(rgb(palette.ink), rgb(palette.paper)) >= 7.0);
        }
        assert_eq!(window.get_theme_options(), catalog);
        assert_eq!(settings, before);
        assert_eq!(window.get_note_text().as_bytes(), source.as_bytes());
        window.set_system_high_contrast(false);
    }
}

#[test]
fn settings_callbacks_and_failed_save_restore_both_palettes() {
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
        let catalog = window.get_theme_options();
        window.set_appearance_mode("light".into());
        window.invoke_appearance_changed();
        binding.flush().await.unwrap();
        assert!(!window.get_settings_theme().dark);
        binding.set_system_appearance(SystemAppearance::Dark);
        assert!(!window.get_settings_theme().dark);
        let saved = store.load_settings().await.unwrap();
        let injector = rusqlite::Connection::open(path).unwrap();
        injector.execute_batch("CREATE TRIGGER fail_settings BEFORE UPDATE ON settings BEGIN SELECT RAISE(ABORT,'fixture failure'); END;").unwrap();
        window.set_appearance_mode("dark".into());
        window.invoke_appearance_changed();
        binding.flush().await.unwrap();
        assert!(!window.get_settings_theme().dark);
        assert!(!window.get_theme().dark);
        assert_eq!(store.load_settings().await.unwrap(), saved);
        assert_eq!(window.get_theme_options(), catalog);
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
