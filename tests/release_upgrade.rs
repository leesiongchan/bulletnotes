use bulletnotes::{
    domain::{connection::ConnectionConfig, page::*},
    store::StoreHandle,
    theme::{
        PaperStyle,
        palette::{ThemeEntry, builtin_themes},
    },
};
const OFFLINE: &str = "unsynced café\r\n👩🏽‍💻 日本語";
fn key(path: &str) -> PageKey {
    PageKey {
        connection: ConnectionConfig::parse("https://upgrade.example")
            .unwrap()
            .id()
            .clone(),
        path: PagePath::parse(path).unwrap(),
    }
}
#[tokio::test]
async fn a_new_process_preserves_committed_notes_pending_edits_and_imported_appearance() {
    let directory = tempfile::Builder::new()
        .prefix("bulletnotes-upgrade-")
        .tempdir()
        .unwrap();
    let path = directory.path().join("owned.sqlite");
    let nonce = uuid::Uuid::new_v4().to_string();
    std::fs::write(path.with_extension("fixture-marker"), &nonce).unwrap();
    let store = StoreHandle::open(&path).await.unwrap();
    for name in ["stable.md", "edited.md"] {
        let key = key(name);
        store.create_draft(key.clone()).await.unwrap();
        store
            .save_local(key, "committed source".into(), LocalRevision(1))
            .await
            .unwrap();
    }
    for mutation in store
        .claim_pending(&key("edited.md").connection, 10)
        .await
        .unwrap()
    {
        store
            .acknowledge(
                mutation.id,
                MutationAck {
                    remote_revision: Some(RemoteRevision("\"baseline\"".into())),
                },
            )
            .await
            .unwrap();
    }
    store
        .save_local(key("edited.md"), OFFLINE.into(), LocalRevision(2))
        .await
        .unwrap();
    let mut settings = store.load_settings().await.unwrap();
    let mut theme = builtin_themes().unwrap().remove(0).theme;
    theme.name = "Own imported theme".into();
    let imported = ThemeEntry::import(theme);
    settings.appearance.light_theme = imported.id.clone();
    settings.imported_themes.push(imported);
    settings.appearance.paper = PaperStyle::Grid;
    settings.appearance.font_size = 21.0;
    settings.active_connection = Some(key("edited.md").connection);
    store.save_settings(settings).await.unwrap();
    store.shutdown().await.unwrap();
    let child = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "upgrade_child_process", "--ignored"])
        .env("BULLETNOTES_UPGRADE_FIXTURE", &path)
        .env("BULLETNOTES_UPGRADE_NONCE", &nonce)
        .output()
        .unwrap();
    assert!(
        child.status.success(),
        "{}",
        String::from_utf8_lossy(&child.stdout)
    );
}
#[tokio::test]
#[ignore = "owned separate-process upgrade helper; invoked by the parent regression"]
async fn upgrade_child_process() {
    let path = std::path::PathBuf::from(
        std::env::var_os("BULLETNOTES_UPGRADE_FIXTURE").expect("owned fixture required"),
    );
    assert!(
        path.parent()
            .and_then(std::path::Path::file_name)
            .is_some_and(|name| name.to_string_lossy().starts_with("bulletnotes-upgrade-"))
    );
    assert_eq!(
        std::fs::read_to_string(path.with_extension("fixture-marker")).unwrap(),
        std::env::var("BULLETNOTES_UPGRADE_NONCE").unwrap()
    );
    let store = StoreHandle::open(&path).await.unwrap();
    assert_eq!(
        store
            .load_page(&key("stable.md"))
            .await
            .unwrap()
            .unwrap()
            .body,
        "committed source"
    );
    assert_eq!(
        store
            .load_page(&key("edited.md"))
            .await
            .unwrap()
            .unwrap()
            .body,
        OFFLINE
    );
    let settings = store.load_settings().await.unwrap();
    assert_eq!(settings.imported_themes.len(), 1);
    assert_eq!(settings.imported_themes[0].theme.name, "Own imported theme");
    assert_eq!(
        settings.appearance.light_theme,
        settings.imported_themes[0].id
    );
    assert_eq!(settings.appearance.paper, PaperStyle::Grid);
    assert_eq!(settings.appearance.font_size, 21.0);
    let queued = store
        .claim_pending(&key("edited.md").connection, 10)
        .await
        .unwrap();
    assert_eq!(queued.len(), 1);
    assert_eq!(queued[0].key, key("edited.md"));
    assert_eq!(queued[0].kind, MutationKind::Put(OFFLINE.into()));
    assert_eq!(
        queued[0].condition,
        WriteCondition::Match(RemoteRevision("\"baseline\"".into()))
    );
    store.shutdown().await.unwrap();
}
