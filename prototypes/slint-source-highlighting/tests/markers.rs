use i_slint_core::input::{InternalKeyEvent, KeyEventType};
use slint::language::{TextInputMarker, TextInputMarkerKind as Kind};
use slint::{ComponentHandle, Model, ModelRc, VecModel, platform::WindowEvent};
use slint_source_highlighting_probe::EditorHarness;
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
    ui.invoke_select(source.len() as i32, source.len() as i32);
    ui
}

fn marker(start: i32, end: i32, kind: Kind) -> TextInputMarker {
    let mut marker = TextInputMarker::default();
    marker.start = start;
    marker.end = end;
    marker.kind = kind;
    marker.foreground = slint::Color::from_rgb_u8(255, 0, 0);
    marker
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
fn marker_model_changes_pixels_without_changing_source_metrics_or_undo() {
    let source = "- [ ] café 👩🏽‍💻\r\n- List 日本語";
    let ui = setup(source);
    ui.window()
        .dispatch_event(WindowEvent::KeyPressed { text: "X".into() });
    ui.window()
        .dispatch_event(WindowEvent::KeyReleased { text: "X".into() });
    let before = ui.window().take_snapshot().unwrap();
    let state = (
        ui.get_anchor(),
        ui.get_caret(),
        ui.get_text_width(),
        ui.get_text_height(),
        ui.get_edits(),
        ui.get_caret_point(),
    );
    let model = Rc::new(VecModel::from(vec![marker(0, 6, Kind::Checkbox)]));
    ui.set_markers(ModelRc::from(model.clone()));
    let unchecked = red_mask(&ui);
    assert!(
        unchecked.iter().any(|pixel| *pixel),
        "checkbox must be rendered"
    );
    assert_eq!(
        (
            ui.get_anchor(),
            ui.get_caret(),
            ui.get_text_width(),
            ui.get_text_height(),
            ui.get_edits(),
            ui.get_caret_point()
        ),
        state
    );
    assert_eq!(ui.get_source(), format!("{source}X"));
    model.set_row_data(0, marker(0, 6, Kind::CheckedCheckbox));
    assert_ne!(red_mask(&ui), unchecked);
    model.remove(0);
    assert_eq!(
        ui.window().take_snapshot().unwrap().as_bytes(),
        before.as_bytes()
    );
    ui.invoke_undo_input();
    assert_eq!(ui.get_source(), source);
    ui.invoke_redo_input();
    assert_eq!(ui.get_source(), format!("{source}X"));
}

#[test]
fn caret_and_selection_reveal_the_exact_original_prefix() {
    let ui = setup("- [ ] Task");
    ui.set_markers(ModelRc::from([marker(0, 6, Kind::Checkbox)]));
    assert!(red_mask(&ui).iter().any(|pixel| *pixel));
    for position in [0, 2, 6] {
        ui.invoke_select(position, position);
        assert!(!red_mask(&ui).iter().any(|pixel| *pixel));
    }
    ui.invoke_select(9, 2);
    let with = ui.window().take_snapshot().unwrap();
    assert_eq!((ui.get_anchor(), ui.get_caret()), (9, 2));
    ui.set_markers(ModelRc::default());
    assert_eq!(
        with.as_bytes(),
        ui.window().take_snapshot().unwrap().as_bytes()
    );
}

#[test]
fn preedit_before_a_marker_keeps_source_offsets_and_inside_reveals_raw() {
    let source = "é \r\n- [ ] Task";
    let ui = setup(source);
    let start = source.find('-').unwrap() as i32;
    ui.invoke_select(0, 0);
    ui.set_markers(ModelRc::from([marker(start, start + 6, Kind::Checkbox)]));
    ui.window()
        .dispatch_event(WindowEvent::internal(InternalKeyEvent {
            event_type: KeyEventType::UpdateComposition,
            preedit_text: "にに".into(),
            ..Default::default()
        }));
    assert_eq!(ui.get_source(), source);
    assert_eq!(ui.get_preedit(), "にに");
    let composing = red_mask(&ui);
    assert!(composing.iter().any(|pixel| *pixel));
    ui.window()
        .dispatch_event(WindowEvent::internal(InternalKeyEvent {
            event_type: KeyEventType::UpdateComposition,
            ..Default::default()
        }));
    let committed = format!("にに{source}");
    ui.set_source(committed.clone().into());
    ui.invoke_select(committed.len() as i32, committed.len() as i32);
    ui.set_markers(ModelRc::from([marker(
        start + 6,
        start + 12,
        Kind::Checkbox,
    )]));
    assert_eq!(composing, red_mask(&ui));
    ui.invoke_select(start + 8, start + 8);
    ui.window()
        .dispatch_event(WindowEvent::internal(InternalKeyEvent {
            event_type: KeyEventType::UpdateComposition,
            preedit_text: "あ".into(),
            ..Default::default()
        }));
    assert!(!red_mask(&ui).iter().any(|pixel| *pixel));
    assert_eq!(ui.get_source(), committed);
    assert_eq!(ui.get_preedit(), "あ");
}

#[test]
fn invalid_or_fragmented_marker_ranges_remain_raw() {
    let ui = setup("é\n- [ ] Task");
    ui.set_markers(ModelRc::from([
        marker(-1, 2, Kind::Bullet),
        marker(1, 2, Kind::Bullet),
        marker(0, 3, Kind::Bullet),
        marker(4, 3, Kind::Bullet),
        marker(0, 99, Kind::Bullet),
    ]));
    assert!(!red_mask(&ui).iter().any(|pixel| *pixel));
    ui.set_source("- [ ] Task".into());
    ui.invoke_select(10, 10);
    ui.window().set_size(slint::LogicalSize::new(85.0, 420.0));
    ui.set_editor_width(13.0);
    ui.set_markers(ModelRc::from([marker(0, 6, Kind::Checkbox)]));
    assert_eq!(
        ui.window().take_snapshot().unwrap().width(),
        85,
        "narrow fixture size"
    );
    assert!(
        !red_mask(&ui).iter().any(|pixel| *pixel),
        "wrapped prefixes must stay raw"
    );
}

#[test]
fn parser_recognizes_nested_lists_and_task_states_without_touching_code() {
    use slint_source_highlighting_probe::markdown_markers;
    let source = "---\n# metadata\n---\n- [ ] Task\r\n- [x] Done\n  - nested\n> - quote\n1. [ ] ordered task\n\n```\n- [ ] code\n```\n\\- escaped\n";
    let markers = markdown_markers(source);
    let slices: Vec<_> = markers
        .iter()
        .map(|m| (&source[m.start as usize..m.end as usize], m.kind))
        .collect();
    assert_eq!(
        slices,
        vec![
            ("- [ ] ", Kind::Checkbox),
            ("- [x] ", Kind::CheckedCheckbox),
            ("- ", Kind::Bullet),
            ("- ", Kind::Bullet),
            ("[ ] ", Kind::Checkbox),
        ]
    );
    assert!(markers.windows(2).all(|m| m[0].end <= m[1].start));
}

#[test]
fn marker_painting_preserves_body_hit_testing_and_caret_geometry() {
    use slint::platform::PointerEventButton;
    let source = "- [ ] café 日本語";
    let ui = setup(source);
    for offset in [8, 11, source.len() as i32] {
        ui.set_markers(ModelRc::default());
        ui.invoke_select(offset, offset);
        let _ = ui.window().take_snapshot().unwrap();
        let point = ui.get_caret_point();
        assert!(point.x > 0.0, "fixture must expose actual caret geometry");
        ui.set_markers(ModelRc::from([marker(0, 6, Kind::Checkbox)]));
        assert!(red_mask(&ui).iter().any(|pixel| *pixel));
        assert_eq!(ui.get_caret_point(), point);
        let position = slint::LogicalPosition::new(point.x + 36.2, point.y + 42.0);
        ui.window().dispatch_event(WindowEvent::PointerPressed {
            position,
            button: PointerEventButton::Left,
        });
        ui.window().dispatch_event(WindowEvent::PointerReleased {
            position,
            button: PointerEventButton::Left,
        });
        assert_eq!((ui.get_anchor(), ui.get_caret()), (offset, offset));
    }
    assert_eq!(ui.get_source(), source);
}

#[test]
fn clipping_leaves_wrapped_continuations_and_transparent_markers_raw() {
    let source = format!("- [ ] {}", "café 日本語 wrapped body ".repeat(12));
    let ui = setup(&source);
    let plain = ui.window().take_snapshot().unwrap();
    ui.set_markers(ModelRc::from([marker(0, 6, Kind::Checkbox)]));
    let painted = ui.window().take_snapshot().unwrap();
    assert!(red_mask(&ui).iter().any(|pixel| *pixel));
    for (index, (a, b)) in plain
        .as_bytes()
        .as_chunks::<4>()
        .0
        .iter()
        .zip(painted.as_bytes().as_chunks::<4>().0)
        .enumerate()
    {
        let x = index % plain.width() as usize;
        let y = index / plain.width() as usize;
        if !(35..125).contains(&x) || !(35..80).contains(&y) {
            assert_eq!(
                a, b,
                "painting must preserve glyphs outside the prefix at {x},{y}"
            );
        }
    }
    let mut transparent = marker(0, 6, Kind::Checkbox);
    transparent.foreground = slint::Color::from_argb_u8(0, 255, 0, 0);
    ui.set_markers(ModelRc::from([transparent]));
    assert_eq!(
        plain.as_bytes(),
        ui.window().take_snapshot().unwrap().as_bytes()
    );
}

#[test]
fn endpoints_inside_unicode_clusters_remain_exact_raw_source() {
    let ui = setup("e\u{0301} tail");
    let before = ui.window().take_snapshot().unwrap();
    for (start, end) in [(1, 3), (0, 1)] {
        ui.set_markers(ModelRc::from([marker(start, end, Kind::Bullet)]));
        assert!(
            before.as_bytes() == ui.window().take_snapshot().unwrap().as_bytes(),
            "a marker splitting a grapheme must leave every pixel unchanged"
        );
    }
    ui.set_markers(ModelRc::from([marker(0, 3, Kind::Bullet)]));
    assert!(red_mask(&ui).iter().any(|pixel| *pixel));
}

#[test]
fn marker_clipping_preserves_italic_ink_beyond_the_final_advance() {
    let ui = setup("- f\n");
    ui.set_italic(true);
    let before = ui.window().take_snapshot().unwrap();
    ui.set_markers(ModelRc::from([marker(0, 2, Kind::Bullet)]));
    let after = ui.window().take_snapshot().unwrap();
    assert!(red_mask(&ui).iter().any(|pixel| *pixel));
    for (index, (a, b)) in before
        .as_bytes()
        .as_chunks::<4>()
        .0
        .iter()
        .zip(after.as_bytes().as_chunks::<4>().0)
        .enumerate()
    {
        let x = index % before.width() as usize;
        if x >= 65 {
            assert_eq!(a, b, "body ink changed at x={x}");
        }
    }
}
