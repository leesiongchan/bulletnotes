//! Screen readers must hear the same on/off and chosen state that the visuals show.
use bulletnotes::{
    AppWindow, PageRow, app::ui::present_settings, store::settings::AppSettings,
    theme::SystemAppearance,
};
use i_slint_backend_testing::{AccessibleRole, ElementHandle};
use slint::{ComponentHandle, ModelRc};

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
    present_settings(&window, &AppSettings::default(), SystemAppearance::Light).unwrap();
    // Tall enough that every settings control is inside the scroll viewport.
    window
        .window()
        .set_size(slint::LogicalSize::new(640.0, 2400.0));
    window.show().unwrap();
    window
}

fn element(window: &AppWindow, label: &str) -> ElementHandle {
    let _ = window.window().take_snapshot().unwrap();
    // Text elements are labelled by their own text; only controls are wanted.
    let mut found = ElementHandle::find_by_accessible_label(window, label)
        .filter(|element| element.accessible_role() != Some(AccessibleRole::Text));
    let element = found
        .next()
        .unwrap_or_else(|| panic!("no element labelled {label:?}"));
    assert!(found.next().is_none(), "{label:?} is ambiguous");
    element
}

#[test]
fn accessible_states_match_visual_state() {
    let window = window();
    window.set_active_view("appearance".into());
    window.set_settings_category("appearance".into());

    // Toggles report their switch state, and activation flips it.
    let count = element(&window, "Show note count");
    assert_eq!(count.accessible_role(), Some(AccessibleRole::Switch));
    assert_eq!(count.accessible_checkable(), Some(true));
    let before = window.get_show_note_count();
    assert_eq!(count.accessible_checked(), Some(before));
    count.invoke_accessible_default_action();
    assert_eq!(
        element(&window, "Show note count").accessible_checked(),
        Some(!before)
    );

    // One-of-N choices are radio buttons that expose which one is chosen.
    for (label, chosen) in [
        ("Smooth", true),
        ("Bone", false),
        ("Light", false),
        ("System", true),
    ] {
        let choice = element(&window, label);
        assert_eq!(
            choice.accessible_role(),
            Some(AccessibleRole::RadioButton),
            "{label}"
        );
        assert_eq!(choice.accessible_checkable(), Some(true), "{label}");
        assert_eq!(choice.accessible_checked(), Some(chosen), "{label}");
    }
    assert_eq!(
        element(&window, "Appearance").accessible_checked(),
        Some(true)
    );

    // Plain actions stay buttons without a checked state.
    let close = element(&window, "Return to note");
    assert_eq!(close.accessible_role(), Some(AccessibleRole::Button));
    assert_ne!(close.accessible_checkable(), Some(true));

    // Search view toggles and selectable rows.
    window.set_page_results(ModelRc::from(
        [PageRow {
            title: "Groceries".into(),
            path: "Quick Notes/groceries.md".into(),
            availability: "Cached".into(),
            checked: true,
            ..Default::default()
        }]
        .as_slice(),
    ));
    window.set_active_view("pages".into());
    let list = element(&window, "List view");
    assert_eq!(list.accessible_role(), Some(AccessibleRole::RadioButton));
    assert_eq!(list.accessible_checked(), Some(!window.get_search_cards()));
    let row_label = "Groceries, Quick Notes/groceries.md, Cached";
    assert_ne!(
        element(&window, row_label).accessible_checkable(),
        Some(true)
    );
    window.set_search_selecting(true);
    let row = element(&window, row_label);
    assert_eq!(row.accessible_checkable(), Some(true));
    assert_eq!(row.accessible_checked(), Some(true));
}
