use bulletnotes::{
    app::startup,
    domain::{connection::ConnectionConfig, page::*},
    store::StoreHandle,
};
fn key(base: &str, path: &str) -> PageKey {
    PageKey {
        connection: ConnectionConfig::parse(base).unwrap().id().clone(),
        path: PagePath::parse(path).unwrap(),
    }
}
#[tokio::test]
async fn startup_restores_last_cached_note_for_the_selected_space() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("startup.sqlite");
    let selected = key("https://notes.example/selected", "last.md");
    let other = key("https://notes.example/other", "other.md");
    let first = key("https://notes.example/selected", "first.md");
    let store = StoreHandle::open(&path).await.unwrap();
    for page in [&selected, &other, &first] {
        store.create_draft(page.clone()).await.unwrap();
    }
    store
        .save_local(
            selected.clone(),
            "café\r\n👩🏽‍💻 日本語".into(),
            LocalRevision(1),
        )
        .await
        .unwrap();
    store.record_visit(&first, 1).await.unwrap();
    store.record_visit(&selected, 10).await.unwrap();
    store.record_visit(&other, 100).await.unwrap();
    let mut settings = store.load_settings().await.unwrap();
    settings.active_connection = Some(selected.connection.clone());
    settings.appearance.font_size = 19.0;
    store.save_settings(settings).await.unwrap();
    store.shutdown().await.unwrap();
    let fallback = ConnectionConfig::parse("https://fallback.example")
        .unwrap()
        .id()
        .clone();
    let restored = startup::open(&path, fallback).await.unwrap();
    assert_eq!(restored.connection, selected.connection);
    assert_eq!(restored.last_page, Some(selected.clone()));
    assert_eq!(restored.settings.appearance.font_size, 19.0);
    assert_eq!(
        restored
            .store
            .load_page(&selected)
            .await
            .unwrap()
            .unwrap()
            .body,
        "café\r\n👩🏽‍💻 日本語"
    );
    restored.store.shutdown().await.unwrap();
}
#[tokio::test]
async fn fresh_startup_uses_configured_space_without_inventing_a_page() {
    let directory = tempfile::tempdir().unwrap();
    let connection = key("https://notes.example", "test.md").connection;
    let startup = startup::open(&directory.path().join("fresh.sqlite"), connection.clone())
        .await
        .unwrap();
    assert_eq!(startup.connection, connection);
    assert_eq!(startup.settings.active_connection, Some(connection));
    assert!(startup.last_page.is_none());
    startup.store.shutdown().await.unwrap();
}
#[tokio::test]
async fn startup_does_not_replace_an_invalid_settings_database() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("invalid.sqlite");
    let store = StoreHandle::open(&path).await.unwrap();
    store.shutdown().await.unwrap();
    let db = rusqlite::Connection::open(&path).unwrap();
    db.execute("INSERT INTO settings VALUES (1, 'invalid-json')", [])
        .unwrap();
    drop(db);
    let connection = key("https://notes.example", "test.md").connection;
    assert!(startup::open(&path, connection).await.is_err());
    let db = rusqlite::Connection::open(&path).unwrap();
    assert_eq!(
        db.query_row("SELECT value FROM settings", [], |row| row
            .get::<_, String>(0))
            .unwrap(),
        "invalid-json"
    );
}

#[test]
fn a_disconnected_monitor_restores_the_window_inside_a_visible_screen() {
    use bulletnotes::{
        app::window_placement::{ScreenFrame, restore},
        store::settings::WindowPlacement,
    };
    let saved = WindowPlacement {
        x: 9000.0,
        y: -6000.0,
        width: 1200.0,
        height: 900.0,
        pinned: true,
    };
    let screen = ScreenFrame {
        x: -1440.0,
        y: 24.0,
        width: 1440.0,
        height: 850.0,
    };
    let restored = restore(&saved, &[screen]).unwrap();
    assert!(restored.x >= screen.x && restored.y >= screen.y);
    assert!(restored.x + restored.width <= screen.x + screen.width);
    assert!(restored.y + restored.height <= screen.y + screen.height);
    assert!(restored.pinned);
}
#[test]
fn window_restoration_keeps_a_valid_secondary_display_position() {
    use bulletnotes::{
        app::window_placement::{ScreenFrame, restore},
        store::settings::WindowPlacement,
    };
    let screens = [
        ScreenFrame {
            x: 0.0,
            y: 0.0,
            width: 1440.0,
            height: 900.0,
        },
        ScreenFrame {
            x: -1920.0,
            y: 0.0,
            width: 1920.0,
            height: 1080.0,
        },
    ];
    let saved = WindowPlacement {
        x: -1800.0,
        y: 100.0,
        width: 505.0,
        height: 552.0,
        pinned: false,
    };
    assert_eq!(restore(&saved, &screens), Some(saved));
}

#[tokio::test]
async fn last_note_follows_visits_when_the_wall_clock_stalls_or_moves_backwards() {
    let directory = tempfile::tempdir().unwrap();
    let store = StoreHandle::open(&directory.path().join("clock.sqlite"))
        .await
        .unwrap();
    let first = key("https://notes.example", "a.md");
    let second = key("https://notes.example", "z.md");
    store.create_draft(first.clone()).await.unwrap();
    store.create_draft(second.clone()).await.unwrap();
    store.record_visit(&first, 10).await.unwrap();
    store.record_visit(&second, 10).await.unwrap();
    assert_eq!(
        store.last_cached_page(&first.connection).await.unwrap(),
        Some(second)
    );
    store.record_visit(&first, 9).await.unwrap();
    assert_eq!(
        store.last_cached_page(&first.connection).await.unwrap(),
        Some(first)
    );
    store.shutdown().await.unwrap();
}

#[tokio::test]
async fn start_with_new_note_preserves_the_previous_note() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("startup.sqlite");
    let page = key("https://notes.example", "previous.md");
    let store = StoreHandle::open(&path).await.unwrap();
    store.create_draft(page.clone()).await.unwrap();
    store
        .save_local(page.clone(), "previous café".into(), LocalRevision(1))
        .await
        .unwrap();
    store.record_visit(&page, 1).await.unwrap();
    let mut json = serde_json::to_value(store.load_settings().await.unwrap()).unwrap();
    json["start_with_new_note"] = true.into();
    store
        .save_settings(serde_json::from_value(json).unwrap())
        .await
        .unwrap();
    store.shutdown().await.unwrap();
    let restored = startup::open(&path, page.connection.clone()).await.unwrap();
    assert!(
        restored.last_page.is_none(),
        "new-note startup must skip reopening the previous page"
    );
    assert_eq!(
        restored.store.load_page(&page).await.unwrap().unwrap().body,
        "previous café"
    );
    restored.store.shutdown().await.unwrap();
}

#[tokio::test]
async fn older_settings_keep_the_existing_appearance_and_startup_defaults() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("legacy.sqlite");
    let store = StoreHandle::open(&path).await.unwrap();
    store.shutdown().await.unwrap();
    let legacy = r#"{"schema_version":1,"appearance":{"mode":"System","light_theme":"bulletnotes","dark_theme":"bulletnotes-dark","paper":"Plain","font_size":15.0},"imported_themes":[],"active_connection":null,"window":null,"hotkey":{"key":"KeyA","alt":true,"control":false,"shift":false,"command":false}}"#;
    let db = rusqlite::Connection::open(&path).unwrap();
    db.execute("INSERT INTO settings VALUES (1, ?1)", [legacy])
        .unwrap();
    drop(db);
    let store = StoreHandle::open(&path).await.unwrap();
    assert_eq!(store.load_settings().await.unwrap(), Default::default());
    store.shutdown().await.unwrap();
}

#[tokio::test]
async fn legacy_paper_type_loads_as_smooth_and_saves_the_explicit_value() {
    use bulletnotes::theme::PaperType;
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("legacy-paper.sqlite");
    let store = StoreHandle::open(&path).await.unwrap();
    let mut json = serde_json::to_value(store.load_settings().await.unwrap()).unwrap();
    store.shutdown().await.unwrap();
    json["appearance"]["paper_type"] = "theme".into();
    let db = rusqlite::Connection::open(&path).unwrap();
    db.execute("INSERT INTO settings VALUES (1, ?1)", [json.to_string()])
        .unwrap();
    drop(db);
    let store = StoreHandle::open(&path).await.unwrap();
    let settings = store.load_settings().await.unwrap();
    assert_eq!(settings.appearance.paper_type, PaperType::Smooth);
    store.save_settings(settings).await.unwrap();
    store.shutdown().await.unwrap();
    let db = rusqlite::Connection::open(&path).unwrap();
    let json: String = db
        .query_row("SELECT value FROM settings", [], |row| row.get(0))
        .unwrap();
    let saved: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(saved["appearance"]["paper_type"], "smooth");
}
