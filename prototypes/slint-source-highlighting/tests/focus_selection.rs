use i_slint_core::{input::FocusReason, window::WindowInner};
use slint::{ComponentHandle, platform::WindowEvent};
use slint_source_highlighting_probe::EditorHarness;

fn focus_loss(preserve: bool) -> (i32, i32) {
    slint::platform::set_platform(Box::new(i_slint_backend_testing::TestingBackend::new(
        i_slint_backend_testing::TestingBackendOptions {
            mock_time: true,
            threading: false,
            renderer_name: Some("skia".into()),
        },
    )))
    .unwrap();
    let ui = EditorHarness::new().unwrap();
    ui.set_source("café 日本語".into());
    ui.set_preserve_selection_on_focus_loss(preserve);
    ui.show().unwrap();
    ui.window()
        .dispatch_event(WindowEvent::WindowActiveChanged(true));
    ui.invoke_select(5, 0);
    let inner = WindowInner::from_pub(ui.window());
    let focused = inner.focus_item.borrow().upgrade().unwrap();
    inner.set_focus_item(&focused, false, FocusReason::Programmatic);
    assert_eq!(ui.get_source(), "café 日本語");
    (ui.get_anchor(), ui.get_caret())
}
#[test]
fn ordinary_inputs_keep_the_default_focus_loss_behavior() {
    assert_eq!(focus_loss(false), (0, 0));
}
#[test]
fn opted_in_editor_preserves_backward_utf8_selection_on_focus_loss() {
    assert_eq!(focus_loss(true), (5, 0));
}
