use bulletnotes::{
    AppWindow, app::ui::present_settings, store::settings::AppSettings, theme::SystemAppearance,
};
use i_slint_core::{
    input::{CompositionStatus, InternalKeyEvent, KeyEventType},
    platform::InternalEvent,
};
use slint::{
    ComponentHandle,
    platform::{Key, WindowEvent},
};
use std::{cell::Cell, rc::Rc};
fn status(window: &AppWindow, value: CompositionStatus) {
    window.window().dispatch_event(WindowEvent::internal(
        InternalEvent::CompositionStatusChanged {
            context_id: 1,
            status: value,
        },
    ));
}
fn setup(source: &str) -> AppWindow {
    slint::platform::set_platform(Box::new(i_slint_backend_testing::TestingBackend::new(
        i_slint_backend_testing::TestingBackendOptions {
            mock_time: true,
            threading: false,
            renderer_name: Some("skia".into()),
        },
    )))
    .unwrap();
    let window = AppWindow::new().unwrap();
    present_settings(&window, &AppSettings::default(), SystemAppearance::Light).unwrap();
    window.set_note_text(source.into());
    window.show().unwrap();
    window
        .window()
        .dispatch_event(WindowEvent::WindowActiveChanged(true));
    window.invoke_focus_editor();
    window.invoke_select_find_match(source.len() as i32, source.len() as i32);
    status(&window, CompositionStatus::Idle);
    window
}
fn event(window: &AppWindow, text: slint::SharedString, down: bool) {
    window.window().dispatch_event(if down {
        WindowEvent::KeyPressed { text }
    } else {
        WindowEvent::KeyReleased { text }
    });
}
fn shortcut(window: &AppWindow, modifiers: &[Key], text: slint::SharedString) {
    for modifier in modifiers {
        event(window, (*modifier).into(), true);
    }
    event(window, text.clone(), true);
    event(window, text, false);
    for modifier in modifiers.iter().rev() {
        event(window, (*modifier).into(), false);
    }
}
#[test]
fn command_shortcuts_edit_once_and_undo_once() {
    let window = setup("word");
    window.invoke_select_find_match(4, 0);
    let edits = Rc::new(Cell::new(0));
    let observed = edits.clone();
    window.on_note_edited(move |_| observed.set(observed.get() + 1));
    shortcut(&window, &[Key::Control], "b".into());
    assert_eq!(window.get_note_text(), "**word**");
    assert_eq!(edits.get(), 1);
    assert_eq!(
        (window.get_editor_anchor(), window.get_editor_caret()),
        (6, 2)
    );
    shortcut(&window, &[Key::Control], "z".into());
    assert_eq!(window.get_note_text(), "word");
    assert_eq!(
        (window.get_editor_anchor(), window.get_editor_caret()),
        (4, 0)
    );
    shortcut(&window, &[Key::Control, Key::Shift], "z".into());
    assert_eq!(window.get_note_text(), "**word**");
    assert_eq!(
        (window.get_editor_anchor(), window.get_editor_caret()),
        (6, 2)
    );
    assert_eq!(edits.get(), 3);
}
#[test]
fn extra_modifiers_and_altgr_do_not_format() {
    let window = setup("word");
    for modifiers in [
        &[Key::Control, Key::Shift][..],
        &[Key::Control, Key::Alt],
        &[Key::Control, Key::Meta],
    ] {
        shortcut(&window, modifiers, "b".into());
        assert_eq!(window.get_note_text(), "word");
    }
}
#[test]
fn task_shortcut_does_not_open_search() {
    let window = setup("- [ ] one");
    let searches = Rc::new(Cell::new(0));
    let count = searches.clone();
    window.on_browse_pages(move || count.set(count.get() + 1));
    shortcut(&window, &[Key::Control, Key::Shift], "k".into());
    assert_eq!(window.get_note_text(), "- [x] one");
    assert_eq!(window.get_active_view(), "note");
    assert_eq!(searches.get(), 0);
    shortcut(&window, &[Key::Control, Key::Shift, Key::Alt], "k".into());
    assert_eq!(searches.get(), 0);
    shortcut(&window, &[Key::Control], "k".into());
    assert_eq!(searches.get(), 1);
    shortcut(&window, &[Key::Control], "f".into());
    assert!(window.get_find_visible());
}
#[test]
fn readonly_and_composition_do_not_edit() {
    let window = setup("- [ ] one");
    for barrier in 0..3 {
        window.set_note_read_only(barrier == 0);
        window.set_recovery_busy(barrier == 1);
        window.set_lifecycle_pending(barrier == 2);
        shortcut(&window, &[Key::Control], "b".into());
        shortcut(&window, &[Key::Control, Key::Shift], "k".into());
        assert_eq!(window.get_note_text(), "- [ ] one");
    }
    window.set_lifecycle_pending(false);
    for value in [CompositionStatus::Unknown, CompositionStatus::Active] {
        status(&window, value);
        window
            .window()
            .dispatch_event(WindowEvent::internal(InternalKeyEvent {
                event_type: KeyEventType::UpdateComposition,
                preedit_text: "ni".into(),
                ..Default::default()
            }));
        shortcut(&window, &[Key::Control], "b".into());
        shortcut(&window, &[Key::Control, Key::Shift], "k".into());
        assert_eq!(window.get_note_text(), "- [ ] one");
        assert_eq!(window.get_active_view(), "note");
    }
}
#[test]
fn native_return_and_tab_survive_outside_list() {
    let window = setup("plain");
    shortcut(&window, &[], Key::Return.into());
    assert_eq!(window.get_note_text(), "plain\n");
    shortcut(&window, &[], Key::Tab.into());
    assert_eq!(window.get_note_text(), "plain\n");
    window.set_note_text("- a".into());
    window.invoke_focus_editor();
    window.invoke_select_find_match(3, 3);
    shortcut(&window, &[Key::Shift], Key::Return.into());
    assert_eq!(window.get_note_text(), "- a\n");
}

fn checkbox_point(window: &AppWindow) -> slint::LogicalPosition {
    let inner = i_slint_core::window::WindowInner::from_pub(window.window());
    let item = inner.focus_item.borrow().upgrade().unwrap();
    let input = item.downcast::<i_slint_core::items::TextInput>().unwrap();
    let adapter = inner.window_adapter();
    for y in 0..80 {
        for x in 0..45 {
            let local = i_slint_core::lengths::LogicalPoint::new(x as f32, y as f32);
            if let Some(hit) =
                adapter
                    .renderer()
                    .text_input_marker_at_position(input.as_pin_ref(), &item, local)
            {
                let point =
                    item.map_to_window(hit.box_rect.center() + item.geometry().origin.to_vector());
                return slint::LogicalPosition::new(point.x, point.y);
            }
        }
    }
    panic!("generated checkbox has no visible hit region");
}
#[test]
fn offscreen_caret_toggle_preserves_viewport() {
    let source = format!("- [ ] first\n{}", "body 日本語\n".repeat(90));
    let window = setup(&source);
    let end = source.len() as i32;
    window.invoke_restore_editor_position(end, end - 1, 0.0);
    let _ = window.window().take_snapshot().unwrap();
    let point = checkbox_point(&window);
    let before = (
        window.get_editor_anchor(),
        window.get_editor_caret(),
        window.get_editor_scroll_y(),
    );
    let edits = Rc::new(Cell::new(0));
    let observed = edits.clone();
    window.on_note_edited(move |_| observed.set(observed.get() + 1));
    window.window().dispatch_event(WindowEvent::PointerPressed {
        position: point,
        button: slint::platform::PointerEventButton::Left,
    });
    window
        .window()
        .dispatch_event(WindowEvent::PointerReleased {
            position: point,
            button: slint::platform::PointerEventButton::Left,
        });
    assert_eq!(window.get_note_text(), source.replacen("[ ]", "[x]", 1));
    assert_eq!(
        (
            window.get_editor_anchor(),
            window.get_editor_caret(),
            window.get_editor_scroll_y()
        ),
        before
    );
    assert_eq!(edits.get(), 1);
    shortcut(&window, &[Key::Control], "z".into());
    assert_eq!(window.get_note_text(), source);
    assert_eq!(
        (window.get_editor_anchor(), window.get_editor_caret()),
        (end, end - 1)
    );
}
