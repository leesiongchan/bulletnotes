use bulletnotes::{
    AppWindow, PageRow, SearchAdapter, UiView, app::ui::present_settings,
    store::settings::AppSettings, theme::SystemAppearance,
};
use slint::{
    ComponentHandle, ModelRc,
    platform::{Key, WindowEvent},
};
use std::{cell::RefCell, rc::Rc};
fn setup() -> AppWindow {
    slint::platform::set_platform(Box::new(i_slint_backend_testing::TestingBackend::new(
        i_slint_backend_testing::TestingBackendOptions {
            mock_time: true,
            threading: false,
            renderer_name: Some("skia".into()),
        },
    )))
    .unwrap();
    let ui = AppWindow::new().unwrap();
    present_settings(&ui, &AppSettings::default(), SystemAppearance::Light).unwrap();
    ui.global::<SearchAdapter>().set_results(ModelRc::from(
        (0..12).map(row).collect::<Vec<_>>().as_slice(),
    ));
    ui.show().unwrap();
    ui.window()
        .dispatch_event(WindowEvent::WindowActiveChanged(true));
    ui.set_active_view(UiView::Pages);
    ui.window().dispatch_event(WindowEvent::internal(
        i_slint_core::platform::InternalEvent::CompositionStatusChanged {
            context_id: 1,
            status: i_slint_core::input::CompositionStatus::Idle,
        },
    ));
    settle();
    ui
}
fn row(n: usize) -> PageRow {
    PageRow {
        key: n.to_string().into(),
        title: format!("Generated {n}").into(),
        path: format!("generated/{n}.md").into(),
        excerpt: "…needle <b>literal</b> **markup** 日本語 👩🏽‍💻 and more context for wrapping".into(),
        ..Default::default()
    }
}
fn settle() {
    i_slint_backend_testing::mock_elapsed_time(std::time::Duration::from_millis(20));
    i_slint_backend_testing::mock_elapsed_time(std::time::Duration::from_millis(20));
}
fn key(ui: &AppWindow, key: Key) {
    ui.window()
        .dispatch_event(WindowEvent::KeyPressed { text: key.into() });
    ui.window()
        .dispatch_event(WindowEvent::KeyReleased { text: key.into() });
    settle();
}
fn text_item(ui: &AppWindow, text: &str) -> i_slint_core::item_tree::ItemRc {
    let inner = i_slint_core::window::WindowInner::from_pub(ui.window());
    inner
        .window_item_rc()
        .unwrap()
        .visit_descendants(|item| {
            if item
                .downcast::<i_slint_core::items::ComplexText>()
                .is_some_and(|t| t.as_pin_ref().text().as_str() == text)
            {
                std::ops::ControlFlow::Break(item.clone())
            } else {
                std::ops::ControlFlow::Continue(())
            }
        })
        .expect("generated literal Text not mounted")
}
#[test]
fn keyboard_reveals_results_after_resize_and_replacement() {
    let ui = setup();
    let y = |item: &i_slint_core::item_tree::ItemRc| item.map_to_window(item.geometry().origin).y;
    let opened = Rc::new(RefCell::new(String::new()));
    let observed = opened.clone();
    ui.on_open_page(move |key| *observed.borrow_mut() = key.to_string());
    for _ in 0..8 {
        key(&ui, Key::DownArrow);
    }
    ui.window().set_size(slint::LogicalSize::new(400.0, 440.0));
    settle();
    key(&ui, Key::DownArrow);
    key(&ui, Key::Return);
    assert_eq!(&*opened.borrow(), "8");
    let selected = text_item(&ui, "generated/8.md");
    assert!(y(&selected) >= 180.0 && y(&selected) + 16.0 <= 406.0);
    ui.global::<SearchAdapter>()
        .set_results(ModelRc::from([row(0)]));
    settle();
    key(&ui, Key::Return);
    assert_eq!(&*opened.borrow(), "0");
}
#[test]
fn escape_returns_from_idle_query_but_preserves_composition() {
    let ui = setup();
    let weak = ui.as_weak();
    ui.on_return_to_note(move || weak.upgrade().unwrap().set_active_view(UiView::Note));
    let status = |value| {
        ui.window().dispatch_event(WindowEvent::internal(
            i_slint_core::platform::InternalEvent::CompositionStatusChanged {
                context_id: 1,
                status: value,
            },
        ))
    };
    status(i_slint_core::input::CompositionStatus::Active);
    key(&ui, Key::Escape);
    assert_eq!(ui.get_active_view(), UiView::Pages);
    status(i_slint_core::input::CompositionStatus::Idle);
    key(&ui, Key::Escape);
    assert_eq!(ui.get_active_view(), UiView::Note);
}

#[test]
fn search_reentry_keeps_query_focused_after_result_replacement() {
    let ui = setup();
    let weak = ui.as_weak();
    ui.on_open_page(move |_| weak.upgrade().unwrap().set_active_view(UiView::Note));
    for _ in 0..8 {
        key(&ui, Key::DownArrow);
    }
    key(&ui, Key::Return);
    assert_eq!(ui.get_active_view(), UiView::Note);
    ui.set_active_view(UiView::Pages);
    settle();
    ui.global::<SearchAdapter>().set_results(ModelRc::from(
        (0..12).map(row).collect::<Vec<_>>().as_slice(),
    ));
    settle();
    let inner = i_slint_core::window::WindowInner::from_pub(ui.window());
    let item = inner
        .focus_item
        .borrow()
        .upgrade()
        .expect("query focus lost on search reentry");
    let input = item
        .downcast::<i_slint_core::items::TextInput>()
        .expect("search reentry did not focus query");
    assert!(input.as_pin_ref().has_focus());
    ui.window()
        .dispatch_event(WindowEvent::KeyPressed { text: "x".into() });
    assert_eq!(ui.global::<SearchAdapter>().get_query(), "x");
}

#[test]
fn cards_navigate_columns_and_escape_exits_selection_before_closing() {
    let ui = setup();
    ui.window().set_size(slint::LogicalSize::new(505.0, 552.0));
    ui.global::<SearchAdapter>().set_cards(true);
    settle();
    let opened = Rc::new(RefCell::new(String::new()));
    let observed = opened.clone();
    ui.on_open_page(move |key| *observed.borrow_mut() = key.to_string());
    key(&ui, Key::DownArrow);
    key(&ui, Key::RightArrow);
    key(&ui, Key::Return);
    assert_eq!(&*opened.borrow(), "1");
    ui.global::<SearchAdapter>().set_selecting(true);
    key(&ui, Key::Escape);
    assert!(!ui.global::<SearchAdapter>().get_selecting());
    assert_eq!(ui.get_active_view(), UiView::Pages);
}

#[test]
fn busy_search_cannot_open_a_result_with_return() {
    let ui = setup();
    let opened = Rc::new(RefCell::new(String::new()));
    let observed = opened.clone();
    ui.on_open_page(move |key| *observed.borrow_mut() = key.to_string());
    key(&ui, Key::DownArrow);
    ui.global::<SearchAdapter>().set_busy(true);
    key(&ui, Key::Return);
    assert!(opened.borrow().is_empty());
}

#[test]
fn drag_reorders_without_opening_and_filtered_drag_stays_disabled() {
    let ui = setup();
    for filtered in [false, true] {
        ui.global::<SearchAdapter>().set_scratchpads(true);
        ui.global::<SearchAdapter>().set_can_reorder(true);
        if filtered {
            ui.global::<SearchAdapter>().set_query("needle".into());
        }
        settle();
        let started = Rc::new(RefCell::new(String::new()));
        let moved = Rc::new(RefCell::new(String::new()));
        let opened = Rc::new(RefCell::new(String::new()));
        let observed = started.clone();
        ui.global::<SearchAdapter>()
            .on_drag_started(move |key| *observed.borrow_mut() = key.to_string());
        let observed = moved.clone();
        ui.global::<SearchAdapter>()
            .on_reordered(move |key, _| *observed.borrow_mut() = key.to_string());
        let observed = opened.clone();
        ui.on_open_page(move |key| *observed.borrow_mut() = key.to_string());
        let from = slint::LogicalPosition::new(100.0, 128.0);
        let to = slint::LogicalPosition::new(100.0, 246.0);
        ui.window().dispatch_event(WindowEvent::PointerPressed {
            position: from,
            button: slint::platform::PointerEventButton::Left,
        });
        ui.window()
            .dispatch_event(WindowEvent::PointerMoved { position: to });
        settle();
        ui.window().dispatch_event(WindowEvent::PointerReleased {
            position: to,
            button: slint::platform::PointerEventButton::Left,
        });
        settle();
        if !filtered {
            assert_eq!(&*started.borrow(), "0");
            assert!(!moved.borrow().is_empty());
            assert!(opened.borrow().is_empty());
        } else {
            assert!(started.borrow().is_empty());
            assert!(moved.borrow().is_empty());
        }
    }
}

#[test]
fn disabled_result_action_does_not_open_the_note_beneath_it() {
    let ui = setup();
    ui.window().set_size(slint::LogicalSize::new(505.0, 552.0));
    let opened = Rc::new(RefCell::new(String::new()));
    let observed = opened.clone();
    ui.on_open_page(move |key| *observed.borrow_mut() = key.to_string());
    let point = slint::LogicalPosition::new(452.0, 127.0);
    ui.window().dispatch_event(WindowEvent::PointerPressed {
        position: point,
        button: slint::platform::PointerEventButton::Left,
    });
    ui.window().dispatch_event(WindowEvent::PointerReleased {
        position: point,
        button: slint::platform::PointerEventButton::Left,
    });
    assert!(opened.borrow().is_empty());
}

#[test]
fn row_actions_and_shift_return_do_not_open_before_promotion_finishes() {
    let ui = setup();
    ui.window().set_size(slint::LogicalSize::new(505.0, 552.0));
    ui.global::<SearchAdapter>().set_scratchpads(true);
    let mut item = row(0);
    item.can_promote = true;
    item.can_trash = true;
    ui.global::<SearchAdapter>()
        .set_results(ModelRc::from([item]));
    settle();
    let promoted = Rc::new(RefCell::new(String::new()));
    let observed = promoted.clone();
    ui.global::<SearchAdapter>()
        .on_promote(move |key, open| *observed.borrow_mut() = format!("{key}:{open}"));
    let trashed = Rc::new(RefCell::new(String::new()));
    let observed = trashed.clone();
    ui.global::<SearchAdapter>()
        .on_trash(move |key| *observed.borrow_mut() = key.to_string());
    let opened = Rc::new(RefCell::new(String::new()));
    let observed = opened.clone();
    ui.on_open_page(move |key| *observed.borrow_mut() = key.to_string());
    for x in [419.0, 453.0] {
        let point = slint::LogicalPosition::new(x, 127.0);
        ui.window().dispatch_event(WindowEvent::PointerPressed {
            position: point,
            button: slint::platform::PointerEventButton::Left,
        });
        ui.window().dispatch_event(WindowEvent::PointerReleased {
            position: point,
            button: slint::platform::PointerEventButton::Left,
        });
    }
    assert_eq!(&*promoted.borrow(), "0:false");
    assert_eq!(&*trashed.borrow(), "0");
    assert!(opened.borrow().is_empty());
    key(&ui, Key::DownArrow);
    ui.window().dispatch_event(WindowEvent::KeyPressed {
        text: Key::Shift.into(),
    });
    key(&ui, Key::Return);
    ui.window().dispatch_event(WindowEvent::KeyReleased {
        text: Key::Shift.into(),
    });
    assert_eq!(&*promoted.borrow(), "0:true");
    assert!(opened.borrow().is_empty());
}

#[test]
fn confirmation_buttons_fit_inside_the_dialog_at_minimum_size() {
    let ui = setup();
    ui.window().set_size(slint::LogicalSize::new(400.0, 440.0));
    ui.global::<SearchAdapter>().set_confirm_count(2);
    ui.global::<SearchAdapter>().set_confirm_id("test".into());
    let _ = ui.window().take_snapshot().unwrap();
    let button = text_item(&ui, "Move to Trash");
    let point = button.map_to_window(button.geometry().origin);
    assert!(
        point.y + button.geometry().height() <= 330.0,
        "confirmation action extends beyond its dialog"
    );
}

#[test]
fn minimum_card_keeps_its_footer_above_the_browser_status() {
    let ui = setup();
    ui.window().set_size(slint::LogicalSize::new(400.0, 440.0));
    let mut item = row(0);
    item.modified_label = "Edited today".into();
    ui.global::<SearchAdapter>()
        .set_results(ModelRc::from([item]));
    ui.global::<SearchAdapter>()
        .set_index_status("Index status".into());
    ui.global::<SearchAdapter>().set_cards(true);
    let _ = ui.window().take_snapshot().unwrap();
    let date = text_item(&ui, "Edited today");
    let status = text_item(&ui, "Index status");
    assert!(
        date.map_to_window(date.geometry().origin).y + date.geometry().height()
            < status.map_to_window(status.geometry().origin).y - 6.0,
        "card actions/date require scrolling under the fixed footer"
    );
}

#[test]
fn result_refresh_during_press_cannot_open_a_replacement_note() {
    let ui = setup();
    let opened = Rc::new(RefCell::new(String::new()));
    let observed = opened.clone();
    ui.on_open_page(move |key| *observed.borrow_mut() = key.to_string());
    let point = slint::LogicalPosition::new(100.0, 165.0);
    ui.window().dispatch_event(WindowEvent::PointerPressed {
        position: point,
        button: slint::platform::PointerEventButton::Left,
    });
    ui.global::<SearchAdapter>()
        .set_results(ModelRc::from([row(99)]));
    ui.window().dispatch_event(WindowEvent::PointerReleased {
        position: point,
        button: slint::platform::PointerEventButton::Left,
    });
    assert!(opened.borrow().is_empty());
}

#[test]
fn list_and_card_drag_autoscroll_and_escape_cancel_are_stable() {
    let ui = setup();
    ui.window().set_size(slint::LogicalSize::new(505.0, 552.0));
    ui.global::<SearchAdapter>().set_scratchpads(true);
    ui.global::<SearchAdapter>().set_can_reorder(true);
    let moved = Rc::new(RefCell::new(String::new()));
    let observed = moved.clone();
    ui.global::<SearchAdapter>()
        .on_reordered(move |key, _| *observed.borrow_mut() = key.to_string());
    for cards in [false, true] {
        ui.global::<SearchAdapter>().set_cards(cards);
        ui.global::<SearchAdapter>().set_query("reset".into());
        ui.global::<SearchAdapter>().set_query("".into());
        settle();
        let from = slint::LogicalPosition::new(100.0, 128.0);
        let edge = slint::LogicalPosition::new(100.0, 450.0);
        ui.window().dispatch_event(WindowEvent::PointerPressed {
            position: from,
            button: slint::platform::PointerEventButton::Left,
        });
        ui.window()
            .dispatch_event(WindowEvent::PointerMoved { position: edge });
        for _ in 0..24 {
            settle();
        }
        assert!(ui.global::<SearchAdapter>().get_drag_active());
        ui.window().dispatch_event(WindowEvent::PointerReleased {
            position: edge,
            button: slint::platform::PointerEventButton::Left,
        });
        assert!(moved.borrow().parse::<usize>().unwrap() >= 4);
        moved.borrow_mut().clear();
    }
    ui.global::<SearchAdapter>().set_query("reset".into());
    ui.global::<SearchAdapter>().set_query("".into());
    settle();
    let from = slint::LogicalPosition::new(100.0, 128.0);
    let to = slint::LogicalPosition::new(120.0, 180.0);
    ui.window().dispatch_event(WindowEvent::PointerPressed {
        position: from,
        button: slint::platform::PointerEventButton::Left,
    });
    ui.window()
        .dispatch_event(WindowEvent::PointerMoved { position: to });
    assert!(ui.global::<SearchAdapter>().get_drag_active());
    key(&ui, Key::Escape);
    assert!(!ui.global::<SearchAdapter>().get_drag_active());
    assert_eq!(ui.get_active_view(), UiView::Pages);
    ui.window().dispatch_event(WindowEvent::PointerReleased {
        position: to,
        button: slint::platform::PointerEventButton::Left,
    });
    assert!(moved.borrow().is_empty());
}

#[test]
fn tabbed_list_arrows_follow_the_focused_row() {
    tabbed_arrows_follow_the_focused_row(false);
}
#[test]
fn tabbed_card_arrows_follow_the_focused_row() {
    tabbed_arrows_follow_the_focused_row(true);
}
fn tabbed_arrows_follow_the_focused_row(cards: bool) {
    let ui = setup();
    ui.window().set_size(slint::LogicalSize::new(505.0, 552.0));
    ui.global::<SearchAdapter>().set_cards(cards);
    settle();
    let opened = Rc::new(RefCell::new(String::new()));
    let observed = opened.clone();
    ui.on_open_page(move |key| *observed.borrow_mut() = key.to_string());
    for _ in 0..25 {
        key(&ui, Key::Tab);
        opened.borrow_mut().clear();
        key(&ui, Key::Return);
        if *opened.borrow() == "1" {
            key(&ui, Key::DownArrow);
            key(&ui, Key::Return);
            assert_eq!(&*opened.borrow(), if cards { "3" } else { "2" });
            return;
        }
    }
    panic!("second result was not reachable by Tab");
}
#[test]
fn reorder_hint_keeps_index_completeness_visible() {
    let ui = setup();
    ui.global::<SearchAdapter>().set_scratchpads(true);
    ui.global::<SearchAdapter>()
        .set_index_status("3 pages · 2 of 3 indexed · content search incomplete".into());
    ui.global::<SearchAdapter>()
        .set_action_message("Clear the search to reorder scratchpads".into());
    let combined = format!(
        "{} · {}",
        ui.global::<SearchAdapter>().get_index_status(),
        ui.global::<SearchAdapter>().get_action_message()
    );
    text_item(&ui, &combined);
}

#[test]
fn opening_search_clears_a_legacy_hidden_folder_filter() {
    let ui = setup();
    ui.set_active_view(UiView::Note);
    settle();
    ui.global::<SearchAdapter>().set_folder("old/folder".into());
    let cleared = Rc::new(RefCell::new(false));
    let observed = cleared.clone();
    ui.global::<SearchAdapter>()
        .on_folder_changed(move |folder| *observed.borrow_mut() = folder.is_empty());
    ui.set_active_view(UiView::Pages);
    settle();
    assert!(ui.global::<SearchAdapter>().get_folder().is_empty());
    assert!(*cleared.borrow());
}

#[test]
fn a_trash_confirmation_takes_keyboard_focus() {
    let ui = setup();
    let confirmed = Rc::new(RefCell::new(String::new()));
    let observed = confirmed.clone();
    let search = ui.global::<SearchAdapter>();
    search.on_trash_confirmed(move |id| *observed.borrow_mut() = id.to_string());
    search.set_confirm_count(1);
    search.set_confirm_id("c1".into());
    settle();
    key(&ui, Key::Return);
    assert_eq!(&*confirmed.borrow(), "c1");
}
