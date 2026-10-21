use i_slint_core::input::{InternalKeyEvent, KeyEventType};
use slint::{ComponentHandle, platform::WindowEvent};
use slint_source_highlighting_probe::EditorHarness;

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

fn type_text(ui: &EditorHarness, text: &str) {
    ui.window()
        .dispatch_event(WindowEvent::KeyPressed { text: text.into() });
    ui.window()
        .dispatch_event(WindowEvent::KeyReleased { text: text.into() });
}

fn assert_replacement_roundtrip(
    ui: &EditorHarness,
    original: &str,
    replaced: &str,
    selection: (i32, i32),
    end: i32,
) {
    assert_eq!(ui.get_source(), replaced);
    assert_eq!((ui.get_anchor(), ui.get_caret()), (end, end));
    assert_eq!(ui.get_edits(), 1);
    ui.invoke_undo_input();
    assert_eq!(ui.get_source().as_bytes(), original.as_bytes());
    assert_eq!((ui.get_anchor(), ui.get_caret()), selection);
    assert_eq!(ui.get_edits(), 2);
    ui.invoke_redo_input();
    assert_eq!(ui.get_source(), replaced);
    assert_eq!((ui.get_anchor(), ui.get_caret()), (end, end));
    assert_eq!(ui.get_edits(), 3);
}

#[test]
fn typing_over_forward_unicode_selection_undoes_and_redoes_once() {
    let source = "aé👩🏽‍💻\r\n日本語z";
    let ui = setup(source);
    let selection = (1, source.len() as i32 - 1);
    ui.invoke_select(selection.0, selection.1);
    type_text(&ui, "X");
    assert_replacement_roundtrip(&ui, source, "aXz", selection, 2);
}

#[test]
fn typing_over_backward_selection_restores_direction_and_collapses_redo() {
    let source = "aéz";
    let ui = setup(source);
    let selection = (3, 1);
    ui.invoke_select(selection.0, selection.1);
    type_text(&ui, "日本語");
    assert_replacement_roundtrip(&ui, source, "a日本語z", selection, 10);
}

#[test]
fn ime_commit_over_selection_undoes_and_redoes_once() {
    let source = "a👩🏽‍💻\r\n日本語z";
    let ui = setup(source);
    let selection = (source.len() as i32 - 1, 1);
    ui.invoke_select(selection.0, selection.1);
    ui.window()
        .dispatch_event(WindowEvent::internal(InternalKeyEvent {
            event_type: KeyEventType::UpdateComposition,
            preedit_text: "にほん".into(),
            preedit_selection: Some(3..9),
            ..Default::default()
        }));
    assert_eq!(ui.get_source(), source);
    assert_eq!(ui.get_edits(), 0);
    let mut commit = InternalKeyEvent {
        event_type: KeyEventType::CommitComposition,
        ..Default::default()
    };
    commit.key_event.text = "日本".into();
    ui.window().dispatch_event(WindowEvent::internal(commit));
    assert_replacement_roundtrip(&ui, source, "a日本z", selection, 7);
}

#[test]
fn pasting_over_selection_preserves_crlf_in_one_undo_step() {
    use slint::platform::Key;
    let source = "a日本語z";
    let ui = setup(source);
    let selection = (1, 10);
    ui.invoke_select(selection.0, selection.1);
    i_slint_core::context::with_global_context(
        || unreachable!("testing platform already initialized"),
        |context| {
            context.platform().set_clipboard_text(
                "café\r\n👩🏽‍💻",
                i_slint_core::platform::Clipboard::DefaultClipboard,
            );
        },
    )
    .unwrap();
    ui.window().dispatch_event(WindowEvent::KeyPressed {
        text: Key::Control.into(),
    });
    type_text(&ui, "v");
    ui.window().dispatch_event(WindowEvent::KeyReleased {
        text: Key::Control.into(),
    });
    assert_replacement_roundtrip(&ui, source, "acafé\r\n👩🏽‍💻z", selection, 23);
}

#[test]
fn replacement_does_not_absorb_the_preceding_deletion() {
    let ui = setup("a日本語Z");
    ui.invoke_select(10, 10);
    type_text(
        &ui,
        &slint::SharedString::from(slint::platform::Key::Backspace),
    );
    assert_eq!(ui.get_source(), "a日本Z");
    ui.invoke_select(1, 7);
    type_text(&ui, "x");
    assert_eq!(ui.get_source(), "axZ");
    ui.invoke_undo_input();
    assert_eq!(ui.get_source(), "a日本Z");
    ui.invoke_undo_input();
    assert_eq!(ui.get_source(), "a日本語Z");
    ui.invoke_redo_input();
    assert_eq!(ui.get_source(), "a日本Z");
    ui.invoke_redo_input();
    assert_eq!(ui.get_source(), "axZ");
}

#[test]
fn typing_after_replacement_keeps_both_history_entries_replayable() {
    let ui = setup("abc");
    ui.invoke_select(1, 3);
    type_text(&ui, "X");
    type_text(&ui, "y");
    assert_eq!(ui.get_source(), "aXy");
    ui.invoke_undo_input();
    assert_eq!(ui.get_source(), "aX");
    ui.invoke_undo_input();
    assert_eq!(ui.get_source(), "abc");
    ui.invoke_redo_input();
    assert_eq!(ui.get_source(), "aX");
    ui.invoke_redo_input();
    assert_eq!(ui.get_source(), "aXy");
}

#[test]
fn a_new_replacement_discards_the_undone_replacement() {
    let ui = setup("abc");
    ui.invoke_select(1, 3);
    type_text(&ui, "X");
    ui.invoke_undo_input();
    assert_eq!(ui.get_source(), "abc");
    type_text(&ui, "Y");
    assert_eq!(ui.get_source(), "aY");
    ui.invoke_redo_input();
    assert_eq!(ui.get_source(), "aY");
    ui.invoke_undo_input();
    assert_eq!(ui.get_source(), "abc");
}
