use bulletnotes::{
    AppWindow, PageRow, app::ui::present_settings, store::settings::AppSettings,
    theme::SystemAppearance,
};
use slint::{
    ComponentHandle, Model, ModelRc,
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
    ui.set_page_results(ModelRc::from(
        (0..12).map(row).collect::<Vec<_>>().as_slice(),
    ));
    ui.show().unwrap();
    ui.window()
        .dispatch_event(WindowEvent::WindowActiveChanged(true));
    ui.set_active_view("pages".into());
    settle();
    ui
}
fn row(n: usize) -> PageRow {
    let mut hit = slint::language::TextInputDecoration::default();
    hit.start = 3;
    hit.end = 9;
    hit.bold = true;
    hit.underline = true;
    hit.foreground = slint::Color::from_rgb_u8(255, 0, 0);
    PageRow {
        key: n.to_string().into(),
        title: format!("Generated {n}").into(),
        path: format!("generated/{n}.md").into(),
        excerpt: "…needle <b>literal</b> **markup** 日本語 👩🏽‍💻 and more context for wrapping".into(),
        excerpt_highlights: ModelRc::from([hit]),
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
fn invalid_ranges_render_plain_text() {
    let ui = setup();
    let mut rows = vec![row(0)];
    rows[0].excerpt_highlights = ModelRc::default();
    ui.set_page_results(ModelRc::from(rows.as_slice()));
    let plain = ui.window().take_snapshot().unwrap();
    let mut invalid = slint::language::TextInputDecoration::default();
    invalid.start = 1;
    invalid.end = 2;
    invalid.bold = true;
    invalid.underline = true;
    rows[0].excerpt_highlights = ModelRc::from([invalid]);
    ui.set_page_results(ModelRc::from(rows.as_slice()));
    assert!(plain.as_bytes() == ui.window().take_snapshot().unwrap().as_bytes());
}
#[test]
fn row_keyboard_geometry_uses_112_pixels() {
    let ui = setup();
    let first = text_item(&ui, "Generated 0");
    let second = text_item(&ui, "Generated 1");
    let y = |item: &i_slint_core::item_tree::ItemRc| item.map_to_window(item.geometry().origin).y;
    assert_eq!(y(&second) - y(&first), 112.0);
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
    let selected = text_item(&ui, "Generated 8");
    assert!(y(&selected) >= 180.0 && y(&selected) + 16.0 <= 406.0);
    ui.set_page_results(ModelRc::from([row(0)]));
    settle();
    key(&ui, Key::Return);
    assert_eq!(&*opened.borrow(), "0");
}
#[test]
fn two_line_preview_starts_with_visible_match() {
    let ui = setup();
    ui.set_page_results(ModelRc::from([row(0)]));
    for width in [505.0, 400.0] {
        ui.window().set_size(slint::LogicalSize::new(width, 440.0));
        let _ = ui.window().take_snapshot().unwrap();
        let item = text_item(&ui, row(0).excerpt.as_str());
        assert_eq!(item.geometry().height(), 30.0);
        let text = item.downcast::<i_slint_core::items::ComplexText>().unwrap();
        assert_eq!(text.as_pin_ref().max_lines(), 2);
        assert!(text.as_pin_ref().text().starts_with("…needle"));
        assert_eq!(row(0).excerpt_highlights.row_count(), 1);
    }
}
#[test]
fn stale_query_cannot_reuse_old_highlights() {
    use bulletnotes::{
        app::coordinator::{AppAction, AppEvent, Coordinator, SearchRequest},
        domain::{connection::ConnectionConfig, page::PagePath},
        store::search::{PageSummary, SearchHighlights, SearchQuery, SearchResults},
    };
    let connection = ConnectionConfig::parse("https://search-preview.example/")
        .unwrap()
        .id()
        .clone();
    let page = bulletnotes::domain::page::PageKey {
        connection: connection.clone(),
        path: PagePath::parse("generated.md").unwrap(),
    };
    let mut app = Coordinator::new(connection.clone(), move || page.clone());
    let request = |id, text: &str| {
        AppAction::Search(SearchRequest {
            id,
            query: SearchQuery {
                connection: connection.clone(),
                text: text.into(),
                folder: None,
                offset: 0,
                limit: 50,
            },
        })
    };
    app.dispatch(request(1, "old"));
    app.dispatch(request(2, "new"));
    let result = SearchResults {
        items: vec![PageSummary {
            key: bulletnotes::domain::page::PageKey {
                connection: connection.clone(),
                path: PagePath::parse("old.md").unwrap(),
            },
            title: "old".into(),
            excerpt: Some("old".into()),
            highlights: SearchHighlights {
                title: std::iter::once(0..3).collect(),
                ..Default::default()
            },
            modified_ms: None,
            cached: true,
            permission: bulletnotes::domain::page::Permission::ReadWrite,
            last_visited_ms: None,
        }],
        total: 1,
        indexed: 1,
        catalogue_total: 1,
        complete: true,
    };
    app.receive(AppEvent::SearchReady(1, Ok(result)));
    assert!(app.search_results().is_none());
}

#[test]
fn escape_returns_from_idle_query_but_preserves_composition() {
    let ui = setup();
    let weak = ui.as_weak();
    ui.on_return_to_note(move || weak.upgrade().unwrap().set_active_view("note".into()));
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
    assert_eq!(ui.get_active_view(), "pages");
    status(i_slint_core::input::CompositionStatus::Idle);
    key(&ui, Key::Escape);
    assert_eq!(ui.get_active_view(), "note");
}

#[test]
fn search_reentry_keeps_query_focused_after_result_replacement() {
    let ui = setup();
    let weak = ui.as_weak();
    ui.on_open_page(move |_| weak.upgrade().unwrap().set_active_view("note".into()));
    for _ in 0..8 {
        key(&ui, Key::DownArrow);
    }
    key(&ui, Key::Return);
    assert_eq!(ui.get_active_view(), "note");
    ui.set_active_view("pages".into());
    settle();
    ui.set_page_results(ModelRc::from(
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
    assert_eq!(ui.get_search_query(), "x");
}
