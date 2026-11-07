use bulletnotes::{
    AppWindow, AppearanceAdapter, UiPaperType,
    app::ui::present_settings,
    store::settings::AppSettings,
    theme::{
        Color, PaperStyle, SystemAppearance, ThemeId, contrast::contrast_ratio, palette::ThemeEntry,
    },
};
use slint::ComponentHandle;

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
    window.set_note_text("café\r\n👩🏽‍💻".into());
    window.set_prefers_reduced_motion(true);
    window.show().unwrap();
    window.invoke_reset_chrome();
    window.invoke_restore_editor_position(5, 0, 0.0);
    window
}

fn paper_pixels(window: &AppWindow) -> Vec<Color> {
    let image = window.window().take_snapshot().unwrap();
    (160..288)
        .flat_map(|y| (8..136).map(move |x| (x, y)))
        .map(|(x, y)| {
            let i = (y * image.width() as usize + x) * 4;
            let p = &image.as_bytes()[i..i + 3];
            Color {
                r: p[0],
                g: p[1],
                b: p[2],
            }
        })
        .collect()
}

#[test]
fn paper_material_is_independent_of_theme_identity_and_preference_gated() {
    let window = window();
    let mut settings = AppSettings::default();
    let mut same_name = settings
        .themes()
        .unwrap()
        .into_iter()
        .find(|t| t.id.0 == "bulletnotes")
        .unwrap();
    same_name = ThemeEntry::import(same_name.theme);
    let imported_id = same_name.id.0.clone();
    settings.imported_themes.push(same_name);
    for id in [
        "bulletnotes",
        "bulletnotes-dark",
        "paper",
        imported_id.as_str(),
    ] {
        settings.appearance.light_theme = ThemeId(id.into());
        for kind in [
            bulletnotes::theme::PaperType::Smooth,
            bulletnotes::theme::PaperType::Bone,
            bulletnotes::theme::PaperType::Eggshell,
            bulletnotes::theme::PaperType::Linen,
        ] {
            settings.appearance.paper_type = kind;
            for paper in [
                PaperStyle::Plain,
                PaperStyle::Dots,
                PaperStyle::Ruled,
                PaperStyle::Grid,
            ] {
                settings.appearance.paper = paper;
                present_settings(&window, &settings, SystemAppearance::Light).unwrap();
                let source = window.get_note_text();
                let selection = (window.get_editor_anchor(), window.get_editor_caret());
                let grain = paper_pixels(&window);
                window.set_prefers_reduced_transparency(true);
                let flat = paper_pixels(&window);
                assert_eq!(
                    grain != flat,
                    kind != bulletnotes::theme::PaperType::Smooth,
                    "{id} / {paper:?} / {kind:?}"
                );
                window.set_prefers_reduced_transparency(false);
                window.set_system_high_contrast(true);
                assert_eq!(paper_pixels(&window), flat, "contrast must remove grain");
                window.set_system_high_contrast(false);
                assert_eq!(paper_pixels(&window), grain);
                assert_eq!(window.get_note_text(), source);
                assert_eq!(
                    (window.get_editor_anchor(), window.get_editor_caret()),
                    selection
                );
            }
        }
    }
}

#[test]
fn rendered_material_preserves_text_contrast() {
    let window = window();
    for (id, ink, accent) in [
        ("bulletnotes", "#332E29", "#C3431C"),
        ("bulletnotes-dark", "#F4EEE5", "#FF9A71"),
    ] {
        let mut settings = AppSettings::default();
        settings.appearance.light_theme = ThemeId(id.into());
        for kind in [
            bulletnotes::theme::PaperType::Bone,
            bulletnotes::theme::PaperType::Eggshell,
            bulletnotes::theme::PaperType::Linen,
        ] {
            settings.appearance.paper_type = kind;
            present_settings(&window, &settings, SystemAppearance::Light).unwrap();
            let pixels = paper_pixels(&window);
            assert!(
                pixels.iter().any(|p| *p != pixels[0]),
                "paper tooth must be rendered"
            );
            for p in pixels {
                for foreground in [ink, accent] {
                    assert!(
                        contrast_ratio(Color::parse(foreground).unwrap(), p) >= 4.5,
                        "{id} {foreground} on {}",
                        p.hex()
                    );
                }
            }
        }
    }
}

#[test]
fn explicit_paper_types_have_distinct_finishes_and_respect_accessibility() {
    let window = window();
    let mut finishes = Vec::new();
    for kind in ["smooth", "bone", "eggshell", "linen"] {
        let mut json = serde_json::to_value(AppSettings::default()).unwrap();
        json["appearance"]["paper_type"] = kind.into();
        json["appearance"]["light_theme"] = "paper".into();
        let settings = serde_json::from_value(json).unwrap();
        present_settings(&window, &settings, SystemAppearance::Light).unwrap();
        let finish = paper_pixels(&window);
        window.set_prefers_reduced_transparency(true);
        let flat = paper_pixels(&window);
        assert_eq!(finish != flat, kind != "smooth", "{kind}");
        window.set_prefers_reduced_transparency(false);
        window.set_system_high_contrast(true);
        assert_eq!(paper_pixels(&window), flat);
        window.set_system_high_contrast(false);
        for previous in &finishes {
            assert_ne!(&finish, previous, "paper types must look different: {kind}");
        }
        finishes.push(finish);
        assert_eq!(window.get_note_text(), "café\r\n👩🏽‍💻");
        assert_eq!(window.get_theme().paper.red(), 252);
    }
}

#[test]
fn paper_intensity_changes_lines_independently_of_finish_and_text() {
    let window = window();
    let mut settings = AppSettings::default();
    settings.appearance.paper = PaperStyle::Grid;
    settings.appearance.paper_type = bulletnotes::theme::PaperType::Bone;
    let mut previous = Vec::new();
    for intensity in [
        bulletnotes::theme::PaperIntensity::Subtle,
        bulletnotes::theme::PaperIntensity::Clear,
        bulletnotes::theme::PaperIntensity::Bold,
    ] {
        settings.appearance.paper_intensity = intensity;
        present_settings(&window, &settings, SystemAppearance::Light).unwrap();
        let pixels = paper_pixels(&window);
        for finish in &previous {
            assert_ne!(&pixels, finish);
        }
        previous.push(pixels);
        assert_eq!(
            window.global::<AppearanceAdapter>().get_paper_type(),
            UiPaperType::Bone
        );
        assert_eq!(window.get_note_text(), "café\r\n👩🏽‍💻");
    }
}

#[test]
fn switching_finishes_preserves_source_position_and_undo() {
    use bulletnotes::theme::PaperType;
    use slint::platform::{Key, WindowEvent};
    let window = window();
    let original = format!(
        "café\r\n👩🏽‍💻\n{}",
        "A line of text for scrolling.\n".repeat(100)
    );
    window.set_note_text(original.as_str().into());
    window.invoke_focus_editor();
    window.invoke_select_find_match(0, 0);
    let key = |text: slint::SharedString, pressed| {
        window.window().dispatch_event(if pressed {
            WindowEvent::KeyPressed { text }
        } else {
            WindowEvent::KeyReleased { text }
        })
    };
    key("X".into(), true);
    key("X".into(), false);
    let source = window.get_note_text();
    assert!(source.starts_with('X'));
    window.invoke_restore_editor_position(source.len() as i32 - 6, source.len() as i32 - 12, 120.0);
    let _ = window.window().take_snapshot().unwrap();
    let position = (
        window.get_editor_anchor(),
        window.get_editor_caret(),
        window.get_editor_scroll_y(),
    );
    for kind in [
        PaperType::Bone,
        PaperType::Eggshell,
        PaperType::Linen,
        PaperType::Smooth,
    ] {
        let mut settings = AppSettings::default();
        settings.appearance.paper_type = kind;
        present_settings(&window, &settings, SystemAppearance::Light).unwrap();
        let _ = window.window().take_snapshot().unwrap();
        assert_eq!(window.get_note_text(), source);
        assert_eq!(
            (
                window.get_editor_anchor(),
                window.get_editor_caret(),
                window.get_editor_scroll_y()
            ),
            position
        );
    }
    key(Key::Control.into(), true);
    key("z".into(), true);
    key("z".into(), false);
    key(Key::Control.into(), false);
    assert_eq!(window.get_note_text().as_str(), original);
}
