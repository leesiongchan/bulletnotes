//! The recovery view talks to Rust only through `RecoveryAdapter`; user actions in the
//! panel must arrive there, and adapter state must drive what the panel allows.
use bulletnotes::{
    AppWindow, PageRow, RecoveryAdapter, UiRecoveryKind, UiView, app::ui::present_settings,
    store::settings::AppSettings, theme::SystemAppearance,
};
use i_slint_backend_testing::{AccessibleRole, ElementHandle};
use slint::{ComponentHandle, ModelRc};
use std::{cell::RefCell, rc::Rc};

fn element(window: &AppWindow, label: &str) -> ElementHandle {
    let _ = window.window().take_snapshot().unwrap();
    let mut found = ElementHandle::find_by_accessible_label(window, label)
        .filter(|element| element.accessible_role() != Some(AccessibleRole::Text));
    let element = found
        .next()
        .unwrap_or_else(|| panic!("no element labelled {label:?}"));
    assert!(found.next().is_none(), "{label:?} is ambiguous");
    element
}
fn row(key: &str, title: &str) -> PageRow {
    PageRow {
        key: key.into(),
        title: title.into(),
        ..Default::default()
    }
}

#[test]
fn panel_actions_and_edits_reach_the_adapter() {
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
    window
        .window()
        .set_size(slint::LogicalSize::new(900.0, 1600.0));
    window.show().unwrap();

    let recovery = window.global::<RecoveryAdapter>();
    recovery.set_items(ModelRc::from(
        [row("k3", "Note 3"), row("k4", "Note 4")].as_slice(),
    ));
    recovery.set_selected_id("k3".into());
    recovery.set_selected(true);
    recovery.set_writable(true);
    recovery.set_merge_text("merged".into());
    window.set_active_view(UiView::Recovery);

    let events = Rc::new(RefCell::new(Vec::<String>::new()));
    let log = |events: &Rc<RefCell<Vec<String>>>| {
        let events = events.clone();
        move |entry: String| events.borrow_mut().push(entry)
    };
    let push = log(&events);
    recovery.on_select(move |id| push(format!("select {id}")));
    let push = log(&events);
    recovery.on_apply(move |choice| push(format!("apply {choice}")));
    let push = log(&events);
    recovery.on_merge_edited(move || push("merge-edited".into()));
    let push = log(&events);
    recovery.on_section_changed(move || push("section-changed".into()));
    let push = log(&events);
    recovery.on_restore(move || push("restore".into()));

    element(&window, "Review Note 4").invoke_accessible_default_action();
    element(&window, "Merged result").set_accessible_value("edited merge");
    assert_eq!(recovery.get_merge_text(), "edited merge");
    element(&window, "Use merge").invoke_accessible_default_action();

    element(&window, "Trash").invoke_accessible_default_action();
    assert_eq!(recovery.get_section(), UiRecoveryKind::Trash);
    element(&window, "Recovery destination path").set_accessible_value("Restored/note.md");
    assert_eq!(recovery.get_destination(), "Restored/note.md");
    element(&window, "Restore as a new page").invoke_accessible_default_action();
    assert_eq!(
        *events.borrow(),
        [
            "select k4",
            "merge-edited",
            "apply merge",
            "section-changed",
            "restore"
        ]
    );

    // Recovery work and app shutdown both block further actions.
    let restore = || element(&window, "Restore as a new page").accessible_enabled();
    assert_eq!(restore(), Some(true));
    recovery.set_busy(true);
    assert_eq!(restore(), Some(false));
    recovery.set_busy(false);
    window.set_lifecycle_pending(true);
    assert_eq!(restore(), Some(false));
}
