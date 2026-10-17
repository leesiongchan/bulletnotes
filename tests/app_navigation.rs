use bulletnotes::{
    app::{
        model::{EditorPosition, View},
        navigation::NavigationState,
    },
    domain::{
        connection::ConnectionConfig,
        page::{PageKey, PagePath},
    },
};
fn key(path: &str) -> PageKey {
    PageKey {
        connection: ConnectionConfig::parse("https://notes.example")
            .unwrap()
            .id()
            .clone(),
        path: PagePath::parse(path).unwrap(),
    }
}

#[test]
fn history_restores_selection() {
    let text = "café 👩🏽‍💻\nsecond line";
    let pos = EditorPosition {
        anchor: 3,
        caret: 5,
        scroll_y: 92.5,
    };
    assert!(pos.validate(text).is_ok());
    let mut nav = NavigationState::default();
    nav.open(key("a.md"), pos);
    nav.search_query = "café".into();
    nav.set_view(View::AllPages);
    nav.set_view(View::Note);
    assert_eq!(nav.position(), Some(pos));
    assert_eq!(nav.search_query, "café");
    nav.open(key("b.md"), EditorPosition::default());
    assert_eq!(nav.back(), Some(key("a.md")));
    assert_eq!(nav.position(), Some(pos));
    assert_eq!(nav.forward(), Some(key("b.md")));
    assert_eq!(nav.back(), Some(key("a.md")));
    assert_eq!(nav.search_query, "café");
}

#[test]
fn invalid_utf8_offsets_or_scroll_are_rejected() {
    for pos in [
        EditorPosition {
            anchor: 1,
            caret: 2,
            scroll_y: 0.0,
        },
        EditorPosition {
            anchor: 0,
            caret: 3,
            scroll_y: 0.0,
        },
        EditorPosition {
            anchor: 0,
            caret: 0,
            scroll_y: f32::NAN,
        },
        EditorPosition {
            anchor: 0,
            caret: 0,
            scroll_y: -1.0,
        },
    ] {
        assert!(pos.validate("é").is_err());
    }
}

#[test]
fn opening_new_page_after_back_discards_forward_history() {
    let mut nav = NavigationState::default();
    nav.open(key("a.md"), EditorPosition::default());
    nav.open(key("b.md"), EditorPosition::default());
    nav.back();
    nav.open(key("c.md"), EditorPosition::default());
    assert_eq!(nav.forward(), None);
    assert_eq!(nav.back(), Some(key("a.md")));
    assert_eq!(nav.back(), None);
}

#[test]
fn reopening_current_page_does_not_duplicate_history() {
    let mut nav = NavigationState::default();
    nav.open(key("a.md"), EditorPosition::default());
    nav.open(key("a.md"), EditorPosition::default());
    assert_eq!(nav.back(), None);
    assert_eq!(nav.forward(), None);
}

#[test]
fn save_position_follows_page_identity_across_visits() {
    let mut nav = NavigationState::default();
    let updated = EditorPosition {
        anchor: 0,
        caret: 3,
        scroll_y: 10.0,
    };
    nav.open(key("a.md"), EditorPosition::default());
    nav.save_position(updated);
    nav.open(key("b.md"), EditorPosition::default());
    nav.back();
    assert_eq!(nav.position(), Some(updated));
}
