mod support;
use bulletnotes::{
    domain::{connection::ConnectionConfig, page::*},
    store::StoreHandle,
};
use support::storage_fixture::{database, key};
fn metadata(path: &str, modified: Option<u64>) -> RemoteMetadata {
    RemoteMetadata {
        key: key(path),
        modified_ms: modified,
        content_length: None,
        permission: Permission::ReadWrite,
        revision: None,
    }
}
#[tokio::test]
async fn membership_is_exact_and_connection_scoped() {
    let dir = tempfile::tempdir().unwrap();
    let store = StoreHandle::open(&database(&dir)).await.unwrap();
    for path in [
        "Quick Notes/a.md",
        "Quick Notes.md",
        "Quick Notes2/a.md",
        "quick notes/a.md",
        "Quick Notes/a.txt",
    ] {
        store.create_draft(key(path)).await.unwrap();
    }
    let mut other = key("Quick Notes/other.md");
    other.connection = ConnectionConfig::parse("https://other.example")
        .unwrap()
        .id()
        .clone();
    store.create_draft(other).await.unwrap();
    let entries = store
        .scratchpad_stack(&key("a.md").connection)
        .await
        .unwrap();
    assert_eq!(
        entries.iter().map(|e| e.key.clone()).collect::<Vec<_>>(),
        vec![key("Quick Notes/a.md")]
    );
    store.shutdown().await.unwrap();
}
#[tokio::test]
async fn catalogue_only_and_large_stacks_are_complete() {
    let dir = tempfile::tempdir().unwrap();
    let store = StoreHandle::open(&database(&dir)).await.unwrap();
    store
        .record_catalogue(
            &key("a.md").connection,
            (0..125)
                .map(|n| metadata(&format!("Quick Notes/{n:03}.md"), Some(1000)))
                .collect(),
            true,
        )
        .await
        .unwrap();
    let entries = store
        .scratchpad_stack(&key("a.md").connection)
        .await
        .unwrap();
    assert_eq!(entries.len(), 125);
    assert_eq!(entries[0].key, key("Quick Notes/000.md"));
    assert!(store.load_page(&entries[0].key).await.unwrap().is_none());
    store.shutdown().await.unwrap();
}
#[tokio::test]
async fn order_survives_edits_visits_acknowledgements_and_restart() {
    let dir = tempfile::tempdir().unwrap();
    let path = database(&dir);
    let store = StoreHandle::open(&path).await.unwrap();
    for name in ["Quick Notes/a.md", "Quick Notes/b.md"] {
        store.create_draft(key(name)).await.unwrap();
    }
    let before = store
        .scratchpad_stack(&key("a.md").connection)
        .await
        .unwrap();
    assert_eq!(before[0].key, key("Quick Notes/b.md"));
    store
        .save_local(
            key("Quick Notes/a.md"),
            "日本語\r\n🙂".into(),
            LocalRevision(1),
        )
        .await
        .unwrap();
    store
        .record_visit(&key("Quick Notes/a.md"), 2000)
        .await
        .unwrap();
    let pending = store
        .claim_pending(&key("a.md").connection, 10)
        .await
        .unwrap();
    store
        .acknowledge(
            pending[0].id,
            MutationAck {
                remote_revision: Some(RemoteRevision("one".into())),
            },
        )
        .await
        .unwrap();
    assert_eq!(
        store
            .scratchpad_stack(&key("a.md").connection)
            .await
            .unwrap(),
        before
    );
    store.shutdown().await.unwrap();
    let reopened = StoreHandle::open(&path).await.unwrap();
    assert_eq!(
        reopened
            .scratchpad_stack(&key("a.md").connection)
            .await
            .unwrap(),
        before
    );
    reopened.shutdown().await.unwrap();
}
#[tokio::test]
async fn legacy_admission_is_once_and_dates_are_validated() {
    let dir = tempfile::tempdir().unwrap();
    let store = StoreHandle::open(&database(&dir)).await.unwrap();
    let generated = "Quick Notes/2026-10-19/120000-00000000-0000-4000-8000-000000000001.md";
    let invalid = "Quick Notes/2026-02-30/120000-00000000-0000-4000-8000-000000000002.md";
    store
        .record_catalogue(
            &key("a.md").connection,
            vec![
                metadata(generated, Some(5)),
                metadata(invalid, Some(10)),
                metadata("Quick Notes/legacy.md", Some(7)),
            ],
            true,
        )
        .await
        .unwrap();
    let before = store
        .scratchpad_stack(&key("a.md").connection)
        .await
        .unwrap();
    assert_eq!(before[0].key, key(generated));
    assert_eq!(before[1].created_ms, 10);
    assert_eq!(before[2].created_ms, 7);
    store
        .record_catalogue(
            &key("a.md").connection,
            vec![metadata("Quick Notes/legacy.md", Some(999999))],
            true,
        )
        .await
        .unwrap();
    assert_eq!(
        store
            .scratchpad_stack(&key("a.md").connection)
            .await
            .unwrap(),
        before
    );
    store.shutdown().await.unwrap();
}
#[tokio::test]
async fn restore_preserves_order_after_page_row_replacement() {
    let dir = tempfile::tempdir().unwrap();
    let store = StoreHandle::open(&database(&dir)).await.unwrap();
    let first = key("Quick Notes/first.md");
    store.create_draft(first.clone()).await.unwrap();
    store
        .save_local(first.clone(), "keep\r\n🙂".into(), LocalRevision(1))
        .await
        .unwrap();
    store
        .create_draft(key("Quick Notes/second.md"))
        .await
        .unwrap();
    let before = store.scratchpad_stack(&first.connection).await.unwrap();
    let trash = store
        .move_to_trash(first.clone(), LocalRevision(1))
        .await
        .unwrap();
    assert_eq!(
        store
            .scratchpad_stack(&first.connection)
            .await
            .unwrap()
            .len(),
        1
    );
    let restored = store
        .restore_trash(
            trash,
            Some(PagePath::parse("Quick Notes/restored.md").unwrap()),
        )
        .await
        .unwrap();
    let after = store.scratchpad_stack(&first.connection).await.unwrap();
    assert_eq!(after[1].key, restored);
    assert_eq!(after[1].created_ms, before[1].created_ms);
    assert_eq!(
        store.load_page(&restored).await.unwrap().unwrap().body,
        "keep\r\n🙂"
    );
    store.shutdown().await.unwrap();
}

#[tokio::test]
async fn migration_seven_backfill_preserves_source_and_outbox() {
    let dir = tempfile::tempdir().unwrap();
    let path = database(&dir);
    let store = StoreHandle::open(&path).await.unwrap();
    let page = key("Quick Notes/legacy.md");
    store.create_draft(page.clone()).await.unwrap();
    store
        .save_local(
            page.clone(),
            "---\r\ntitle: café\r\n---\r\n🙂".into(),
            LocalRevision(1),
        )
        .await
        .unwrap();
    let before = store.load_page(&page).await.unwrap().unwrap();
    store.shutdown().await.unwrap();
    let db = rusqlite::Connection::open(&path).unwrap();
    db.execute_batch("DROP TABLE scratchpad_order; PRAGMA user_version=7;")
        .unwrap();
    drop(db);
    let upgraded = StoreHandle::open(&path).await.unwrap();
    assert_eq!(upgraded.load_page(&page).await.unwrap().unwrap(), before);
    assert_eq!(
        upgraded
            .scratchpad_stack(&page.connection)
            .await
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        upgraded
            .claim_pending(&page.connection, 10)
            .await
            .unwrap()
            .len(),
        1
    );
    assert!(
        std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(Result::ok)
            .any(|e| e.file_name().to_string_lossy().contains("pre-migration"))
    );
    upgraded.shutdown().await.unwrap();
}
#[tokio::test]
async fn direct_remote_admission_and_original_path_restoration_keep_order() {
    let dir = tempfile::tempdir().unwrap();
    let store = StoreHandle::open(&database(&dir)).await.unwrap();
    let page = key("Quick Notes/direct.md");
    store
        .apply_remote(RemoteRead::Present(RemoteFile {
            key: page.clone(),
            body: "source".into(),
            revision: Some(RemoteRevision("one".into())),
            permission: Permission::ReadWrite,
            modified_ms: Some(100),
        }))
        .await
        .unwrap();
    let before = store.scratchpad_stack(&page.connection).await.unwrap();
    assert_eq!(before[0].created_ms, 100);
    store
        .apply_remote(RemoteRead::Missing(page.clone()))
        .await
        .unwrap();
    assert!(
        store
            .scratchpad_stack(&page.connection)
            .await
            .unwrap()
            .is_empty()
    );
    let records = store.recovery_copies(&page.connection).await.unwrap();
    store
        .restore_recovery(records.last().unwrap().id, None)
        .await
        .unwrap();
    assert_eq!(
        store.scratchpad_stack(&page.connection).await.unwrap(),
        before
    );
    store.shutdown().await.unwrap();
}
