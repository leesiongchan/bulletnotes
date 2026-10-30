use bulletnotes::{
    AppWindow,
    app::appearance_ui,
    store::{StoreHandle, settings::AppSettings},
    theme::{AppearanceMode, Color, SystemAppearance, palette::ThemeEntry},
};
use slint::{
    ComponentHandle, Model,
    platform::{Key, WindowEvent},
};
use std::{cell::Cell, rc::Rc, time::Duration};

#[test]
fn search_view_defaults_for_old_settings_and_survives_roundtrip() {
    use bulletnotes::store::settings::SearchView;
    let mut old = serde_json::to_value(AppSettings::default()).unwrap();
    old.as_object_mut().unwrap().remove("search_view");
    let mut settings: AppSettings = serde_json::from_value(old).unwrap();
    assert_eq!(settings.search_view, SearchView::List);
    settings.search_view = SearchView::Cards;
    assert_eq!(
        serde_json::from_str::<AppSettings>(&serde_json::to_string(&settings).unwrap())
            .unwrap()
            .search_view,
        SearchView::Cards
    );
}

#[test]
fn every_theme_card_selects_saves_and_preserves_the_editor() {
    slint::platform::set_platform(Box::new(i_slint_backend_testing::TestingBackend::new(
        i_slint_backend_testing::TestingBackendOptions {
            mock_time: false,
            threading: true,
            renderer_name: Some("skia".into()),
        },
    )))
    .unwrap();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let _runtime = runtime.enter();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("themes.sqlite");
    let store = runtime.block_on(StoreHandle::open(&path)).unwrap();
    let mut settings = AppSettings::default();
    let mut imported = settings.themes().unwrap().remove(0).theme;
    imported.name = "Imported low contrast".into();
    imported.ink = imported.paper;
    settings.imported_themes.push(ThemeEntry::import(imported));
    runtime
        .block_on(store.save_settings(settings.clone()))
        .unwrap();
    let themes = settings.themes().unwrap();
    let window = AppWindow::new().unwrap();
    let source = "# Heading\r\n- [ ] café 日本語 👩🏽‍💻\r\n**bold**";
    window.set_note_text(source.into());
    let edits = Rc::new(Cell::new(0));
    let observed = edits.clone();
    window.on_note_edited(move |_| observed.set(observed.get() + 1));
    let binding =
        appearance_ui::bind(&window, store.clone(), settings, SystemAppearance::Light).unwrap();
    let cards = window.get_theme_options();
    assert_eq!(
        cards
            .iter()
            .map(|pair| 1 + usize::from(pair.has_right))
            .sum::<usize>(),
        themes.len()
    );
    assert_eq!(cards.row_data(0).unwrap().left.id, "antinote-vancouver");
    assert_eq!(cards.row_data(3).unwrap().left.id, "antinote-piccolo");
    let tartan = cards.row_data(4).unwrap().left;
    assert_eq!(tartan.id, "antinote-tartan");
    assert_eq!(
        tartan.swatches.accent2,
        slint::Color::from_rgb_u8(80, 170, 80)
    );
    assert_eq!(
        tartan.swatches.highlight,
        slint::Color::from_rgb_u8(246, 191, 159)
    );
    window.show().unwrap();
    let completed = Rc::new(Cell::new(false));
    let done = completed.clone();
    let weak = window.as_weak();
    slint::spawn_local(async move {
        let window = weak.upgrade().unwrap();
        for (mode, system, dark) in [
            (AppearanceMode::Light, SystemAppearance::Light, false),
            (AppearanceMode::Dark, SystemAppearance::Light, true),
            (AppearanceMode::System, SystemAppearance::Light, false),
            (AppearanceMode::System, SystemAppearance::Dark, true),
        ] {
            binding.set_system_appearance(system);
            window.set_appearance_mode(match mode {
                AppearanceMode::Light => "light",
                AppearanceMode::Dark => "dark",
                AppearanceMode::System => "system",
            }.into());
            window.invoke_appearance_changed();
            binding.flush().await.unwrap();
            for entry in &themes {
                let previous = store.load_settings().await.unwrap();
                window.invoke_theme_selected(entry.id.0.as_str().into());
                assert_eq!(window.get_selected_theme_id(), entry.id.0.as_str(), "{} in {mode:?}/{system:?}", entry.theme.name);
                assert!(!window.get_low_contrast_warning(), "existing themes must apply directly");
                binding.flush().await.unwrap();
                let saved = store.load_settings().await.unwrap();
                assert_eq!(saved.appearance.mode, mode);
                if dark {
                    assert_eq!(saved.appearance.dark_theme, entry.id);
                    assert_eq!(saved.appearance.light_theme, previous.appearance.light_theme);
                } else {
                    assert_eq!(saved.appearance.light_theme, entry.id);
                    assert_eq!(saved.appearance.dark_theme, previous.appearance.dark_theme);
                }
                let color = |value: Color| slint::Color::from_rgb_u8(value.r, value.g, value.b);
                let shown = window.get_theme();
                assert_eq!(shown.paper, color(entry.theme.paper));
                assert_eq!(shown.ink, color(entry.theme.ink));
                assert_eq!(shown.accent, color(entry.theme.accent));
                if let Some(markdown) = &entry.theme.markdown {
                    assert_eq!(shown.markdown.heading1, color(markdown.heading1));
                    assert_eq!(shown.markdown.link, color(markdown.link));
                }
                assert_eq!(window.get_note_text().as_bytes(), source.as_bytes());
                assert_eq!(edits.get(), 0);
                assert_eq!(saved.appearance.paper, previous.appearance.paper);
                assert_eq!(saved.appearance.font_size, previous.appearance.font_size);
                assert!(window.get_theme_options() == cards, "theme changes must retain card instances and keyboard focus");
            }
        }
        window.set_active_view("appearance".into());
        window.window().set_size(slint::LogicalSize::new(600.0, 850.0));
        window.window().dispatch_event(WindowEvent::WindowActiveChanged(true));
        let _ = window.window().take_snapshot().unwrap();
        binding.flush().await.unwrap();
        for (index, (id, activation)) in [("antinote-vancouver", Key::Space.into()), ("antinote-tokyo-drift", Key::Space.into()), ("antinote-gundam-wing", Key::Return.into())].into_iter().enumerate() {
            for _ in 0..if index == 0 { 0 } else { 2 } {
                window.window().dispatch_event(WindowEvent::KeyPressed { text: Key::Tab.into() });
                window.window().dispatch_event(WindowEvent::KeyReleased { text: Key::Tab.into() });
            }
            for text in [slint::SharedString::from(Key::Tab), activation] {
                window.window().dispatch_event(WindowEvent::KeyPressed { text: text.clone() });
                window.window().dispatch_event(WindowEvent::KeyReleased { text });
            }
            binding.flush().await.unwrap();
            assert_eq!(window.get_selected_theme_id(), id, "card focus must survive saving and continue with Tab");
        }
        for text in [slint::SharedString::from(Key::Tab), Key::Space.into()] {
            window.window().dispatch_event(WindowEvent::KeyPressed { text:text.clone() });
            window.window().dispatch_event(WindowEvent::KeyReleased { text });
        }
        binding.flush().await.unwrap();
        assert_eq!(store.load_settings().await.unwrap().appearance.light_theme.0, "antinote-gundam-wing");
        window.invoke_theme_assigned("antinote-tartan".into(), true);
        binding.flush().await.unwrap();
        for text in [slint::SharedString::from(Key::Tab), Key::Space.into()] {
            window.window().dispatch_event(WindowEvent::KeyPressed { text:text.clone() });
            window.window().dispatch_event(WindowEvent::KeyReleased { text });
        }
        binding.flush().await.unwrap();
        assert_eq!(store.load_settings().await.unwrap().appearance.dark_theme.0, "antinote-gundam-wing");
        let before = store.load_settings().await.unwrap();
        window.invoke_theme_assigned("antinote-tartan".into(), false);
        binding.flush().await.unwrap();
        window.invoke_theme_assigned("antinote-mononoke".into(), true);
        binding.flush().await.unwrap();
        let assigned = store.load_settings().await.unwrap();
        assert_eq!(assigned.appearance.light_theme.0, "antinote-tartan");
        assert_eq!(assigned.appearance.dark_theme.0, "antinote-mononoke");
        assert_eq!(assigned.appearance.mode, before.appearance.mode);
        assert_eq!(window.get_selected_light_theme_id(), "antinote-tartan");
        assert_eq!(window.get_selected_dark_theme_id(), "antinote-mononoke");
        assert_eq!(window.get_theme_options(), cards);
        assert_eq!(window.get_note_text().as_bytes(), source.as_bytes());
        window.invoke_theme_assigned("missing-theme".into(), true);
        binding.flush().await.unwrap();
        assert_eq!(store.load_settings().await.unwrap(), assigned);
        window.window().set_size(slint::LogicalSize::new(800.0,588.0));
        window.set_system_high_contrast(true);
        binding.set_system_appearance(SystemAppearance::Dark);
        window.invoke_reveal_theme_choices();
        let preview = window.window().take_snapshot().unwrap();
        assert!(preview.as_bytes().as_chunks::<4>().0.iter().filter(|p| p[..3] == [11,72,107]).count() > 100,
            "Increase Contrast must retain the authored preview swatches");
        window.set_system_high_contrast(false);
        binding.set_system_appearance(SystemAppearance::Dark);
        let saved = store.load_settings().await.unwrap();
        let selected = window.get_selected_theme_id();
        let injector = rusqlite::Connection::open(&path).unwrap();
        injector.execute_batch("CREATE TRIGGER fail_theme BEFORE UPDATE ON settings BEGIN SELECT RAISE(ABORT,'fixture failure'); END;").unwrap();
        window.invoke_theme_assigned("antinote-sanrio".into(), true);
        binding.flush().await.unwrap();
        assert_eq!(window.get_selected_theme_id(), selected);
        assert_eq!(store.load_settings().await.unwrap(), saved);
        assert!(window.get_appearance_message().contains("could not be saved"));
        drop(injector);
        store.shutdown().await.unwrap();
        let reopened = StoreHandle::open(&path).await.unwrap();
        assert_eq!(reopened.load_settings().await.unwrap(), saved);
        reopened.shutdown().await.unwrap();
        done.set(true);
        slint::quit_event_loop().unwrap();
    }).unwrap();
    slint::Timer::single_shot(Duration::from_secs(20), || {
        slint::quit_event_loop().unwrap()
    });
    slint::run_event_loop().unwrap();
    assert!(completed.get(), "theme-selection checks did not complete");
}
