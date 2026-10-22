use i_slint_core::{
    input::{CompositionStatus, InternalKeyEvent, KeyEventType},
    platform::InternalEvent,
};
use slint::{ComponentHandle, platform::WindowEvent};
use slint_source_highlighting_probe::EditorHarness;

fn status(ui: &EditorHarness, value: CompositionStatus) {
    ui.window().dispatch_event(WindowEvent::internal(
        InternalEvent::CompositionStatusChanged {
            context_id: 1,
            status: value,
        },
    ));
}
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
    status(&ui, CompositionStatus::Idle);
    ui
}
fn typing(ui: &EditorHarness, text: &str) {
    ui.window()
        .dispatch_event(WindowEvent::KeyPressed { text: text.into() });
    ui.window()
        .dispatch_event(WindowEvent::KeyReleased { text: text.into() });
}
#[test]
fn atomic_edit_restores_both_directed_selections() {
    let ui = setup("aéz");
    ui.invoke_select(3, 1);
    assert!(ui.invoke_apply_edit(1, 3, "**é**".into(), 5, 3));
    assert_eq!(ui.get_source(), "a**é**z");
    assert_eq!((ui.get_anchor(), ui.get_caret(), ui.get_edits()), (5, 3, 1));
    ui.invoke_undo_input();
    assert_eq!(ui.get_source(), "aéz");
    assert_eq!((ui.get_anchor(), ui.get_caret(), ui.get_edits()), (3, 1, 2));
    ui.invoke_redo_input();
    assert_eq!(ui.get_source(), "a**é**z");
    assert_eq!((ui.get_anchor(), ui.get_caret(), ui.get_edits()), (5, 3, 3));
}
#[test]
fn pure_insert_and_delete_do_not_merge_with_typing() {
    let ui = setup("abc");
    ui.invoke_select(3, 3);
    typing(&ui, "a");
    assert!(ui.invoke_apply_edit(4, 4, "**".into(), 6, 6));
    typing(&ui, "b");
    for expected in ["abca**", "abca", "abc"] {
        ui.invoke_undo_input();
        assert_eq!(ui.get_source(), expected);
    }
    for expected in ["abca", "abca**", "abca**b"] {
        ui.invoke_redo_input();
        assert_eq!(ui.get_source(), expected);
    }
    ui.set_source("abcd".into());
    ui.invoke_select(4, 4);
    typing(&ui, "X");
    assert!(ui.invoke_apply_edit(3, 4, "".into(), 3, 3));
    typing(&ui, "Y");
    for expected in ["abcX", "abcdX", "abcd"] {
        ui.invoke_undo_input();
        assert_eq!(ui.get_source(), expected);
    }
}
#[test]
fn rejected_edit_preserves_history_and_composition() {
    let ui = setup("aé👩🏽‍💻\r\nz");
    let end = ui.get_source().len() as i32;
    ui.invoke_select(end, end);
    typing(&ui, "!");
    let before = ui.get_source();
    let selection = (ui.get_anchor(), ui.get_caret());
    let edits = ui.get_edits();
    for (start, end, replacement, anchor, caret) in [
        (-1, 0, "x", 1, 1),
        (2, 3, "x", 1, 1),
        (3, 7, "x", 1, 1),
        (19, 20, "x", 1, 1),
        (0, 100, "x", 1, 1),
        (0, 1, "x", 2, 2),
        (0, 1, "a", 0, 0),
        (3, 1, "x", 1, 1),
    ] {
        assert!(!ui.invoke_apply_edit(start, end, replacement.into(), anchor, caret));
        assert_eq!(ui.get_source(), before);
        assert_eq!((ui.get_anchor(), ui.get_caret()), selection);
        assert_eq!(ui.get_edits(), edits);
    }
    ui.set_read_only(true);
    assert!(!ui.invoke_apply_edit(0, 1, "x".into(), 1, 1));
    ui.set_read_only(false);
    ui.set_input_enabled(false);
    assert!(!ui.invoke_apply_edit(0, 1, "x".into(), 1, 1));
    ui.set_input_enabled(true);
    for state in [CompositionStatus::Unknown, CompositionStatus::Active] {
        status(&ui, state);
        ui.window()
            .dispatch_event(WindowEvent::internal(InternalKeyEvent {
                event_type: KeyEventType::UpdateComposition,
                preedit_text: "ni".into(),
                ..Default::default()
            }));
        assert!(!ui.invoke_apply_edit(0, 1, "x".into(), 1, 1));
        assert_eq!(ui.get_preedit(), "ni");
        assert_eq!(ui.get_source(), before);
    }
    status(&ui, CompositionStatus::Idle);
    ui.invoke_undo_input();
    assert_eq!(ui.get_source(), "aé👩🏽‍💻\r\nz");
}
#[test]
fn new_command_discards_redo() {
    let ui = setup("abc");
    assert!(ui.invoke_apply_edit(0, 1, "A".into(), 1, 1));
    ui.invoke_undo_input();
    assert!(ui.invoke_apply_edit(0, 1, "Z".into(), 1, 1));
    ui.invoke_redo_input();
    assert_eq!(ui.get_source(), "Zbc");
}
