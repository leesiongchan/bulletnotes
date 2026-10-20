use i_slint_core::input::{InternalKeyEvent, KeyEventType};
use slint::{ComponentHandle, Model, ModelRc, VecModel, platform::WindowEvent};
use slint_source_highlighting_probe::{EditorHarness, markdown_decorations};
use std::rc::Rc;

fn setup(source: &str) -> EditorHarness {
    slint::platform::set_platform(Box::new(i_slint_backend_testing::TestingBackend::new(
        i_slint_backend_testing::TestingBackendOptions {
            mock_time: true,
            threading: false,
            renderer_name: Some("skia".into()),
        },
    )))
    .unwrap();
    let ui = EditorHarness::new().unwrap();
    ui.set_source(source.into());
    ui.show().unwrap();
    ui.window()
        .dispatch_event(WindowEvent::WindowActiveChanged(true));
    ui
}

fn span(start: i32, end: i32) -> slint::language::TextInputDecoration {
    let mut value = slint::language::TextInputDecoration::default();
    value.start = start;
    value.end = end;
    value.foreground = slint::Color::from_rgb_u8(255, 0, 0);
    value
}

fn red_pixels(ui: &EditorHarness) -> usize {
    ui.window()
        .take_snapshot()
        .unwrap()
        .as_bytes()
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|p| p[0] > 150 && p[1] < 100 && p[2] < 100 && p[3] > 150)
        .count()
}

#[test]
fn recoloring_changes_pixels_and_tracks_model_mutation() {
    let ui = setup("Hello café 👩🏽‍💻\r\nsecond line");
    let before = ui.window().take_snapshot().unwrap();
    assert_eq!(red_pixels(&ui), 0);
    let model = Rc::new(VecModel::from(vec![span(0, 5)]));
    ui.set_decorations(ModelRc::from(model.clone()));
    assert!(red_pixels(&ui) > 10);
    let mut updated = span(0, 5);
    updated.foreground = slint::Color::from_rgb_u8(0, 0, 255);
    model.set_row_data(0, updated);
    assert_eq!(red_pixels(&ui), 0);
    model.set_row_data(0, span(0, 5));
    assert!(red_pixels(&ui) > 10);
    model.remove(0);
    assert_eq!(
        before.as_bytes(),
        ui.window().take_snapshot().unwrap().as_bytes()
    );
}

#[test]
fn recoloring_preserves_source_directed_selection_and_undo() {
    let source = "café\r\n👩🏽‍💻日本語";
    let ui = setup(source);
    ui.invoke_select(source.len() as i32, source.len() as i32);
    ui.window()
        .dispatch_event(WindowEvent::KeyPressed { text: "X".into() });
    ui.window()
        .dispatch_event(WindowEvent::KeyReleased { text: "X".into() });
    assert_eq!(ui.get_source(), format!("{source}X"));
    ui.invoke_select(source.len() as i32, 0);
    let state = (
        ui.get_anchor(),
        ui.get_caret(),
        ui.get_edits(),
        ui.get_text_width(),
    );
    ui.set_decorations(ModelRc::from([span(0, source.len() as i32)]));
    let _ = ui.window().take_snapshot().unwrap();
    assert_eq!(
        (
            ui.get_anchor(),
            ui.get_caret(),
            ui.get_edits(),
            ui.get_text_width()
        ),
        state
    );
    assert_eq!(ui.get_source().as_bytes(), format!("{source}X").as_bytes());
    ui.invoke_undo_input();
    assert_eq!(ui.get_source(), source);
    ui.invoke_redo_input();
    assert_eq!(ui.get_source(), format!("{source}X"));
}

#[test]
fn recoloring_preserves_preedit_and_maps_following_source() {
    let ui = setup("abWWWW");
    ui.invoke_select(2, 2);
    ui.set_decorations(ModelRc::from([span(2, 6)]));
    let without = red_pixels(&ui);
    ui.window()
        .dispatch_event(WindowEvent::internal(InternalKeyEvent {
            event_type: KeyEventType::UpdateComposition,
            preedit_text: "ににに".into(),
            ..Default::default()
        }));
    assert_eq!(ui.get_source(), "abWWWW");
    assert_eq!(ui.get_preedit(), "ににに");
    // The source span covers WWWW, not the inserted preedit bytes.
    assert!(red_pixels(&ui) >= without.saturating_sub(5));
    let state = (
        ui.get_anchor(),
        ui.get_caret(),
        ui.get_preedit(),
        ui.get_edits(),
    );
    ui.set_decorations(ModelRc::from([span(2, 6)]));
    let _ = ui.window().take_snapshot().unwrap();
    assert_eq!(
        (
            ui.get_anchor(),
            ui.get_caret(),
            ui.get_preedit(),
            ui.get_edits()
        ),
        state
    );
    ui.window()
        .dispatch_event(WindowEvent::internal(InternalKeyEvent {
            event_type: KeyEventType::UpdateComposition,
            ..Default::default()
        }));
    assert_eq!(ui.get_source(), "abWWWW");
    assert!(ui.get_preedit().is_empty());
}

#[test]
fn invalid_ranges_are_ignored_and_selection_color_wins() {
    let ui = setup("éHello");
    ui.set_decorations(ModelRc::from([
        span(-1, 3),
        span(1, 4),
        span(5, 2),
        span(0, 99),
    ]));
    assert_eq!(red_pixels(&ui), 0);
    ui.set_decorations(ModelRc::from([span(0, 7)]));
    assert!(red_pixels(&ui) > 10);
    ui.invoke_select(0, 7);
    assert_eq!(red_pixels(&ui), 0);
}

#[test]
fn markdown_parser_ignores_code_and_preserves_source_ranges() {
    let source = "# café\r\n\n```\n# not a heading\n```\n\n[site](https://example.com)\n";
    let spans = markdown_decorations(source);
    assert!(
        spans
            .iter()
            .any(|s| &source[s.start as usize..s.end as usize] == "# café")
    );
    assert!(
        !spans
            .iter()
            .any(|s| source[s.start as usize..s.end as usize].contains("not a heading"))
    );
    assert!(
        spans
            .iter()
            .any(|s| source[s.start as usize..s.end as usize].contains("https://example.com"))
    );
}

fn red_mask(ui: &EditorHarness) -> Vec<bool> {
    ui.window()
        .take_snapshot()
        .unwrap()
        .as_bytes()
        .as_chunks::<4>()
        .0
        .iter()
        .map(|p| p[0] > 150 && p[1] < 100 && p[2] < 100 && p[3] > 150)
        .collect()
}

#[test]
fn preedit_splits_source_decoration_without_coloring_composition() {
    let ui = setup("WWWWWWWW");
    ui.invoke_select(4, 4);
    ui.set_decorations(ModelRc::from([span(0, 8)]));
    ui.window()
        .dispatch_event(WindowEvent::internal(InternalKeyEvent {
            event_type: KeyEventType::UpdateComposition,
            preedit_text: "ににに".into(),
            ..Default::default()
        }));
    assert_eq!(ui.get_source(), "WWWWWWWW");
    let composing = red_mask(&ui);
    assert!(composing.iter().any(|pixel| *pixel));

    // Compare every decorated pixel with an explicit, composition-free reference.
    ui.window()
        .dispatch_event(WindowEvent::internal(InternalKeyEvent {
            event_type: KeyEventType::UpdateComposition,
            ..Default::default()
        }));
    ui.set_source("WWWWにににWWWW".into());
    ui.set_decorations(ModelRc::from([span(0, 4), span(13, 17)]));
    assert_eq!(composing, red_mask(&ui));
}

#[test]
fn empty_overlapping_and_out_of_order_ranges_are_ignored() {
    let ui = setup("Hello world");
    ui.set_decorations(ModelRc::from([span(6, 11)]));
    let expected = red_mask(&ui);
    assert!(expected.iter().any(|pixel| *pixel));
    ui.set_decorations(ModelRc::from([
        span(6, 11),
        span(6, 6),
        span(8, 11),
        span(0, 5),
    ]));
    assert_eq!(expected, red_mask(&ui));
}
