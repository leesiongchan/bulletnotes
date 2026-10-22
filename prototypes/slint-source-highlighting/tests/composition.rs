use i_slint_core::{
    input::{CompositionStatus, InternalKeyEvent, KeyEventType},
    platform::InternalEvent,
};
use slint::{ComponentHandle, platform::WindowEvent};
use slint_source_highlighting_probe::EditorHarness;

fn setup() -> EditorHarness {
    slint::platform::set_platform(Box::new(i_slint_backend_testing::TestingBackend::new(
        i_slint_backend_testing::TestingBackendOptions {
            mock_time: true,
            threading: false,
            renderer_name: Some("skia".into()),
        },
    )))
    .unwrap();
    let ui = EditorHarness::new().unwrap();
    ui.set_source("original".into());
    ui.show().unwrap();
    ui.window()
        .dispatch_event(WindowEvent::WindowActiveChanged(true));
    ui
}
fn status(ui: &EditorHarness, context_id: u64, status: CompositionStatus) {
    ui.window().dispatch_event(WindowEvent::internal(
        InternalEvent::CompositionStatusChanged { context_id, status },
    ));
}
fn preedit(ui: &EditorHarness, text: &str) {
    ui.window()
        .dispatch_event(WindowEvent::internal(InternalKeyEvent {
            event_type: KeyEventType::UpdateComposition,
            preedit_text: text.into(),
            ..Default::default()
        }));
}
#[test]
fn empty_preedit_does_not_admit_commands() {
    let ui = setup();
    assert!(!ui.get_source_edits_allowed());
    status(&ui, 1, CompositionStatus::Idle);
    assert!(ui.get_source_edits_allowed());
    status(&ui, 1, CompositionStatus::Active);
    preedit(&ui, "ni");
    preedit(&ui, "");
    assert!(!ui.get_source_edits_allowed());
    assert_eq!(ui.get_source(), "original");
    assert_eq!(ui.get_edits(), 0);
}
#[test]
fn commit_releases_guard_after_text_change() {
    let ui = setup();
    ui.invoke_select(8, 8);
    status(&ui, 1, CompositionStatus::Active);
    preedit(&ui, "ni");
    preedit(&ui, "");
    assert!(!ui.get_source_edits_allowed());
    let mut event = InternalKeyEvent {
        event_type: KeyEventType::CommitComposition,
        ..Default::default()
    };
    event.key_event.text = "你".into();
    ui.window().dispatch_event(WindowEvent::internal(event));
    assert_eq!(ui.get_source(), "original你");
    assert!(!ui.get_source_edits_allowed());
    status(&ui, 1, CompositionStatus::Idle);
    assert!(ui.get_source_edits_allowed());
    assert_eq!(ui.get_edits(), 1);
}
#[test]
fn context_cancellation_reestablishes_idle() {
    let ui = setup();
    status(&ui, 1, CompositionStatus::Active);
    status(&ui, 2, CompositionStatus::Unknown);
    status(&ui, 1, CompositionStatus::Idle);
    assert!(!ui.get_source_edits_allowed());
    status(&ui, 2, CompositionStatus::Idle);
    assert!(ui.get_source_edits_allowed());
    ui.set_read_only(true);
    assert!(!ui.get_source_edits_allowed());
    ui.set_read_only(false);
    ui.set_input_enabled(false);
    assert!(!ui.get_source_edits_allowed());
    ui.set_input_enabled(true);
    assert!(ui.get_source_edits_allowed());
}
