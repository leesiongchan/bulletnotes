use slint::{ComponentHandle, ModelRc};
use slint_source_highlighting_probe::LiteralTextHarness;
#[test]
fn ordinary_text_decorations_are_literal_and_optional() {
    slint::platform::set_platform(Box::new(i_slint_backend_testing::TestingBackend::new(
        i_slint_backend_testing::TestingBackendOptions {
            mock_time: true,
            threading: false,
            renderer_name: Some("skia".into()),
        },
    )))
    .unwrap();
    let ui = LiteralTextHarness::new().unwrap();
    let source = "<b>needle</b> **text** https://example.invalid";
    ui.set_source(source.into());
    ui.show().unwrap();
    let plain = ui.window().take_snapshot().unwrap();
    let mut hit = slint::language::TextInputDecoration::default();
    hit.start = 3;
    hit.end = 9;
    hit.bold = true;
    hit.underline = true;
    hit.foreground = slint::Color::from_rgb_u8(255, 0, 0);
    ui.set_decorations(ModelRc::from([hit]));
    let decorated = ui.window().take_snapshot().unwrap();
    assert!(plain.as_bytes() != decorated.as_bytes());
    assert_eq!(ui.get_source(), source);
    ui.set_decorations(ModelRc::default());
    assert!(plain.as_bytes() == ui.window().take_snapshot().unwrap().as_bytes());
    assert_eq!(ui.get_source(), source);
}

#[test]
fn emoji_on_first_line_keeps_highlight_visible() {
    slint::platform::set_platform(Box::new(i_slint_backend_testing::TestingBackend::new(
        i_slint_backend_testing::TestingBackendOptions {
            mock_time: true,
            threading: false,
            renderer_name: Some("skia".into()),
        },
    )))
    .unwrap();
    let ui = LiteralTextHarness::new().unwrap();
    ui.set_source(format!("…needle café 日本語 👩🏽‍💻 {}", "literal context ".repeat(20)).into());
    ui.show().unwrap();
    let mut hit = slint::language::TextInputDecoration::default();
    hit.start = 3;
    hit.end = 9;
    hit.foreground = slint::Color::from_rgb_u8(255, 0, 0);
    hit.bold = true;
    hit.underline = true;
    ui.set_decorations(ModelRc::from([hit]));
    let snapshot = ui.window().take_snapshot().unwrap();
    assert!(
        snapshot
            .as_bytes()
            .as_chunks::<4>()
            .0
            .iter()
            .any(|p| p[0] > 150 && p[1] < 100 && p[2] < 100),
        "the first-line match must paint visible emphasis"
    );
}
