use bulletnotes::{
    AppWindow,
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
fn paper_material_is_builtin_only_and_preference_gated() {
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
    for (id, textured) in [
        ("bulletnotes", true),
        ("bulletnotes-dark", true),
        ("paper", false),
        (imported_id.as_str(), false),
    ] {
        settings.appearance.light_theme = ThemeId(id.into());
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
            assert_eq!(grain != flat, textured, "{id} / {paper:?}");
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

#[test]
fn rendered_material_preserves_text_contrast() {
    let window = window();
    for (id, ink, accent) in [
        ("bulletnotes", "#332E29", "#C3431C"),
        ("bulletnotes-dark", "#F4EEE5", "#FF9A71"),
    ] {
        let mut settings = AppSettings::default();
        settings.appearance.light_theme = ThemeId(id.into());
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
