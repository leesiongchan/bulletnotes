mod support;
use bulletnotes::{
    domain::{
        error::StoreError,
        page::{LocalRevision, MutationAck, MutationKind, RemoteRevision, WriteCondition},
    },
    store::{
        StoreHandle,
        recovery::{Migration, migrate},
    },
};
use std::process::Command;
use support::storage_fixture::{database, key};

#[tokio::test]
async fn batch_rejects_ineligible_or_duplicate_members_before_mutation() {
    let directory = tempfile::tempdir().unwrap();
    let path = database(&directory);
    let store = StoreHandle::open(&path).await.unwrap();
    let page = key("a.md");
    store.create_draft(page.clone()).await.unwrap();
    let connection = page.connection.clone();
    assert!(store.prepare_trash(&connection, vec![]).await.is_err());
    assert!(
        store
            .prepare_trash(&connection, vec![page.clone(), page.clone()])
            .await
            .is_err()
    );
    let mut other = page.clone();
    other.connection =
        bulletnotes::domain::connection::ConnectionConfig::parse("https://other.example")
            .unwrap()
            .id()
            .clone();
    assert!(store.prepare_trash(&connection, vec![other]).await.is_err());
    let db = rusqlite::Connection::open(&path).unwrap();
    db.execute("UPDATE pages SET permission=0", []).unwrap();
    assert!(matches!(
        store.prepare_trash(&connection, vec![page.clone()]).await,
        Err(StoreError::ReadOnly)
    ));
    db.execute("UPDATE pages SET permission=1, body=NULL", [])
        .unwrap();
    assert!(store.prepare_trash(&connection, vec![page]).await.is_err());
    assert!(store.trash(&connection).await.unwrap().is_empty());
    store.shutdown().await.unwrap();
}

#[tokio::test]
async fn batch_trash_is_atomic_and_restores_every_full_snapshot() {
    let directory = tempfile::tempdir().unwrap();
    let path = database(&directory);
    let store = StoreHandle::open(&path).await.unwrap();
    let keys = vec![key("a.md"), key("b.md")];
    for page in &keys {
        store.create_draft(page.clone()).await.unwrap();
        store
            .save_local(page.clone(), "café\r\n日本語".into(), LocalRevision(1))
            .await
            .unwrap();
    }
    let pins = store
        .prepare_trash(&keys[0].connection, keys.clone())
        .await
        .unwrap();
    store
        .save_local(keys[1].clone(), "newer".into(), LocalRevision(2))
        .await
        .unwrap();
    assert!(
        store
            .move_many_to_trash(&keys[0].connection, pins)
            .await
            .is_err()
    );
    assert!(store.trash(&keys[0].connection).await.unwrap().is_empty());
    let pins = store
        .prepare_trash(&keys[0].connection, keys.clone())
        .await
        .unwrap();
    let injector = rusqlite::Connection::open(&path).unwrap();
    injector.execute_batch("CREATE TRIGGER fail_second_trash BEFORE UPDATE OF trashed ON pages WHEN new.path='b.md' BEGIN SELECT RAISE(ABORT,'injected batch failure'); END;").unwrap();
    let pending: i64 = injector
        .query_row("SELECT count(*) FROM outbox", [], |r| r.get(0))
        .unwrap();
    assert!(
        store
            .move_many_to_trash(&keys[0].connection, pins.clone())
            .await
            .is_err()
    );
    assert!(store.trash(&keys[0].connection).await.unwrap().is_empty());
    assert_eq!(
        injector
            .query_row("SELECT count(*) FROM outbox", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        pending
    );
    assert_eq!(
        injector
            .query_row("SELECT count(*) FROM pages WHERE trashed=1", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
    injector
        .execute_batch("DROP TRIGGER fail_second_trash;")
        .unwrap();
    let ids = store
        .move_many_to_trash(&keys[0].connection, pins)
        .await
        .unwrap();
    assert_eq!(ids.len(), 2);
    store.shutdown().await.unwrap();
    let store = StoreHandle::open(&path).await.unwrap();
    assert_eq!(store.trash(&keys[0].connection).await.unwrap().len(), 2);
    for id in ids {
        store.restore_trash(id, None).await.unwrap();
    }
    assert_eq!(
        store.load_page(&keys[0]).await.unwrap().unwrap().body,
        "café\r\n日本語"
    );
    assert_eq!(
        store.load_page(&keys[1]).await.unwrap().unwrap().body,
        "newer"
    );
    store.shutdown().await.unwrap();
}

#[tokio::test]
async fn acknowledged_text_survives_process_exit() {
    let directory = tempfile::tempdir().unwrap();
    let path = database(&directory);
    let output = child(&path, "commit");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let reopened = StoreHandle::open(&path).await.unwrap();
    assert_eq!(
        reopened
            .load_page(&key("a.md"))
            .await
            .unwrap()
            .unwrap()
            .body,
        "---\r\ntitle: café\r\n---\r\n\t👩🏽‍💻 日本語\r\n"
    );
    let queue = reopened
        .claim_pending(&key("a.md").connection, 4)
        .await
        .unwrap();
    assert_eq!(queue.len(), 1);
    assert_eq!(queue[0].local_revision, LocalRevision(1));
    reopened.shutdown().await.unwrap();
}
#[tokio::test]
async fn failed_transaction_preserves_previous_revision() {
    let directory = tempfile::tempdir().unwrap();
    let path = database(&directory);
    let store = StoreHandle::open(&path).await.unwrap();
    let page = key("a.md");
    store.create_draft(page.clone()).await.unwrap();
    store
        .save_local(page.clone(), "committed".into(), LocalRevision(1))
        .await
        .unwrap();
    let injector = rusqlite::Connection::open(&path).unwrap();
    injector.execute_batch("CREATE TRIGGER fail_intent BEFORE INSERT ON outbox BEGIN SELECT RAISE(ABORT,'simulated failure'); END;").unwrap();
    assert!(
        store
            .save_local(page.clone(), "uncommitted".into(), LocalRevision(2))
            .await
            .is_err()
    );
    let snapshot = store.load_page(&page).await.unwrap().unwrap();
    assert_eq!(snapshot.body, "committed");
    assert_eq!(snapshot.local_revision, LocalRevision(1));
    let operations = store.claim_pending(&page.connection, 10).await.unwrap();
    assert_eq!(operations.len(), 1);
    assert_eq!(operations[0].kind, MutationKind::Put("committed".into()));
    store.shutdown().await.unwrap();
}
#[tokio::test]
async fn second_writer_is_refused() {
    let directory = tempfile::tempdir().unwrap();
    let path = database(&directory);
    let store = StoreHandle::open(&path).await.unwrap();
    assert!(matches!(
        StoreHandle::open(&path).await,
        Err(StoreError::AlreadyRunning)
    ));
    assert!(child(&path, "second-writer").status.success());
    store.shutdown().await.unwrap();
    let reopened = StoreHandle::open(&path).await.unwrap();
    reopened.shutdown().await.unwrap();
}
#[test]
fn failed_migration_preserves_original() {
    let directory = tempfile::tempdir().unwrap();
    let path = database(&directory);
    let mut db = rusqlite::Connection::open(&path).unwrap();
    db.execute_batch("PRAGMA journal_mode=WAL; CREATE TABLE original(body TEXT); INSERT INTO original VALUES('committed'); PRAGMA user_version=1;").unwrap();
    let backup = directory.path().join("before.sqlite");
    let steps = [Migration {
        version: 2,
        sql: "DROP TABLE original; invalid SQL;",
    }];
    assert!(migrate(&mut db, &steps, Some(&backup)).is_err());
    assert_eq!(
        db.query_row("SELECT body FROM original", [], |row| row
            .get::<_, String>(0))
            .unwrap(),
        "committed"
    );
    assert_eq!(
        db.query_row("PRAGMA user_version", [], |row| row.get::<_, u32>(0))
            .unwrap(),
        1
    );
    let backup_db = rusqlite::Connection::open(backup).unwrap();
    assert_eq!(
        backup_db
            .query_row("SELECT body FROM original", [], |row| row
                .get::<_, String>(0))
            .unwrap(),
        "committed"
    );
}
#[tokio::test]
async fn corrupt_database_is_never_reset() {
    let directory = tempfile::tempdir().unwrap();
    let path = database(&directory);
    let original = b"This is not SQLite. Preserve for recovery.";
    std::fs::write(&path, original).unwrap();
    assert!(StoreHandle::open(&path).await.is_err());
    assert_eq!(std::fs::read(path).unwrap(), original);
}
#[tokio::test]
async fn durable_configuration_is_verified() {
    let directory = tempfile::tempdir().unwrap();
    let path = database(&directory);
    let store = StoreHandle::open(&path).await.unwrap();
    let configuration = store.durability().await.unwrap();
    assert_eq!(configuration.journal_mode, "wal");
    assert_eq!(configuration.synchronous, 2);
    assert!(configuration.foreign_keys);
    #[cfg(target_os = "macos")]
    assert!(configuration.fullfsync);
    store.shutdown().await.unwrap();
}
#[tokio::test]
async fn delayed_acknowledgement_never_marks_newer_edits_synced() {
    let directory = tempfile::tempdir().unwrap();
    let store = StoreHandle::open(&database(&directory)).await.unwrap();
    let page = key("a.md");
    store.create_draft(page.clone()).await.unwrap();
    store
        .save_local(page.clone(), "sent".into(), LocalRevision(1))
        .await
        .unwrap();
    let sent = store
        .claim_pending(&page.connection, 4)
        .await
        .unwrap()
        .remove(0);
    store
        .save_local(page.clone(), "newer".into(), LocalRevision(2))
        .await
        .unwrap();
    assert!(
        store
            .claim_pending(&page.connection, 4)
            .await
            .unwrap()
            .is_empty()
    );
    let current = store
        .acknowledge(
            sent.id,
            MutationAck {
                remote_revision: Some(RemoteRevision("etag-1".into())),
            },
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(current.body, "newer");
    assert_eq!(current.local_revision, LocalRevision(2));
    assert_eq!(current.remote_base.unwrap().body, "sent");
    let next = store.claim_pending(&page.connection, 4).await.unwrap();
    assert_eq!(next.len(), 1);
    assert_eq!(next[0].kind, MutationKind::Put("newer".into()));
    assert_eq!(
        next[0].condition,
        WriteCondition::Match(RemoteRevision("etag-1".into()))
    );
    store.shutdown().await.unwrap();
}
#[tokio::test]
async fn leased_operation_becomes_unknown_on_restart() {
    let directory = tempfile::tempdir().unwrap();
    let path = database(&directory);
    let page = key("a.md");
    let store = StoreHandle::open(&path).await.unwrap();
    store.create_draft(page.clone()).await.unwrap();
    store
        .save_local(page.clone(), "sent".into(), LocalRevision(1))
        .await
        .unwrap();
    let sent = store
        .claim_pending(&page.connection, 1)
        .await
        .unwrap()
        .remove(0);
    store.shutdown().await.unwrap();
    let reopened = StoreHandle::open(&path).await.unwrap();
    assert!(
        reopened
            .claim_pending(&page.connection, 4)
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        reopened.unknown_mutations(&page.connection).await.unwrap(),
        vec![sent]
    );
    reopened.shutdown().await.unwrap();
}
#[tokio::test]
async fn empty_new_draft_is_local_but_clearing_remote_page_is_queued() {
    let directory = tempfile::tempdir().unwrap();
    let store = StoreHandle::open(&database(&directory)).await.unwrap();
    let page = key("a.md");
    store.create_draft(page.clone()).await.unwrap();
    assert!(
        store
            .claim_pending(&page.connection, 1)
            .await
            .unwrap()
            .is_empty()
    );
    store
        .save_local(page.clone(), "temporary".into(), LocalRevision(1))
        .await
        .unwrap();
    store
        .save_local(page.clone(), String::new(), LocalRevision(2))
        .await
        .unwrap();
    assert!(
        store
            .claim_pending(&page.connection, 1)
            .await
            .unwrap()
            .is_empty()
    );
    store
        .save_local(page.clone(), "published".into(), LocalRevision(3))
        .await
        .unwrap();
    let sent = store
        .claim_pending(&page.connection, 1)
        .await
        .unwrap()
        .remove(0);
    store
        .acknowledge(
            sent.id,
            MutationAck {
                remote_revision: Some(RemoteRevision("base".into())),
            },
        )
        .await
        .unwrap();
    store
        .save_local(page.clone(), String::new(), LocalRevision(4))
        .await
        .unwrap();
    let next = store.claim_pending(&page.connection, 1).await.unwrap();
    assert_eq!(next[0].kind, MutationKind::Put(String::new()));
    assert_eq!(
        next[0].condition,
        WriteCondition::Match(RemoteRevision("base".into()))
    );
    store.shutdown().await.unwrap();
}
#[tokio::test]
async fn stale_save_cannot_rewind_a_page() {
    let directory = tempfile::tempdir().unwrap();
    let store = StoreHandle::open(&database(&directory)).await.unwrap();
    let page = key("a.md");
    store.create_draft(page.clone()).await.unwrap();
    store
        .save_local(page.clone(), "newer".into(), LocalRevision(2))
        .await
        .unwrap();
    assert!(matches!(
        store
            .save_local(page.clone(), "older".into(), LocalRevision(1))
            .await,
        Err(StoreError::StaleRevision)
    ));
    assert_eq!(store.load_page(&page).await.unwrap().unwrap().body, "newer");
    store.shutdown().await.unwrap();
}
fn child(path: &std::path::Path, mode: &str) -> std::process::Output {
    Command::new(std::env::current_exe().unwrap())
        .args(["--ignored", "--exact", "storage_child_process"])
        .env("BULLETNOTES_STORE_FIXTURE_PATH", path)
        .env("BULLETNOTES_STORE_FIXTURE_MODE", mode)
        .env(
            "BULLETNOTES_STORE_FIXTURE_PARENT_PID",
            std::process::id().to_string(),
        )
        .output()
        .unwrap()
}
#[test]
#[ignore = "only run as an isolated subprocess with a temporary database"]
fn storage_child_process() {
    let path =
        std::path::PathBuf::from(std::env::var_os("BULLETNOTES_STORE_FIXTURE_PATH").unwrap());
    let mode = std::env::var("BULLETNOTES_STORE_FIXTURE_MODE").unwrap();
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        if mode == "second-writer" {
            assert!(matches!(
                StoreHandle::open(&path).await,
                Err(StoreError::AlreadyRunning)
            ));
            assert_eq!(
                bulletnotes::store::owner_process_id(&path).unwrap(),
                std::env::var("BULLETNOTES_STORE_FIXTURE_PARENT_PID")
                    .unwrap()
                    .parse::<u32>()
                    .unwrap()
            );
            return;
        }
        let store = StoreHandle::open(&path).await.unwrap();
        let page = key("a.md");
        store.create_draft(page.clone()).await.unwrap();
        store
            .save_local(
                page,
                "---\r\ntitle: café\r\n---\r\n\t👩🏽‍💻 日本語\r\n".into(),
                LocalRevision(1),
            )
            .await
            .unwrap();
        std::process::exit(0); // Intentionally skips every Rust destructor and SQLite close.
    });
}

#[tokio::test]
async fn settings_commit_failure_or_invalid_candidate_keeps_previous() {
    use bulletnotes::{store::settings::AppSettings, theme::PaperStyle};
    let directory = tempfile::tempdir().unwrap();
    let path = database(&directory);
    let store = StoreHandle::open(&path).await.unwrap();
    let previous = AppSettings::default();
    store.save_settings(previous.clone()).await.unwrap();
    let mut invalid = previous.clone();
    invalid.appearance.font_size = f32::NAN;
    assert!(store.save_settings(invalid).await.is_err());
    assert_eq!(store.load_settings().await.unwrap(), previous);
    let injector = rusqlite::Connection::open(&path).unwrap();
    injector.execute_batch("CREATE TRIGGER fail_settings BEFORE UPDATE ON settings BEGIN SELECT RAISE(ABORT,'simulated settings failure'); END;").unwrap();
    let mut changed = previous.clone();
    changed.appearance.paper = PaperStyle::Grid;
    assert!(store.save_settings(changed).await.is_err());
    assert_eq!(store.load_settings().await.unwrap(), previous);
    store.shutdown().await.unwrap();
}

#[tokio::test]
async fn clearing_new_note_during_first_upload_remains_pending_after_ack() {
    let directory = tempfile::tempdir().unwrap();
    let store = StoreHandle::open(&database(&directory)).await.unwrap();
    let page = key("a.md");
    store.create_draft(page.clone()).await.unwrap();
    store
        .save_local(page.clone(), "sent".into(), LocalRevision(1))
        .await
        .unwrap();
    let sent = store
        .claim_pending(&page.connection, 1)
        .await
        .unwrap()
        .remove(0);
    store
        .save_local(page.clone(), String::new(), LocalRevision(2))
        .await
        .unwrap();
    store
        .acknowledge(
            sent.id,
            MutationAck {
                remote_revision: Some(RemoteRevision("sent-etag".into())),
            },
        )
        .await
        .unwrap();
    let next = store.claim_pending(&page.connection, 1).await.unwrap();
    assert_eq!(next.len(), 1);
    assert_eq!(next[0].kind, MutationKind::Put(String::new()));
    assert_eq!(
        next[0].condition,
        WriteCondition::Match(RemoteRevision("sent-etag".into()))
    );
    store.shutdown().await.unwrap();
}
#[tokio::test]
async fn acknowledgement_without_etag_preserves_edits_and_blocks_new_writes() {
    let directory = tempfile::tempdir().unwrap();
    let store = StoreHandle::open(&database(&directory)).await.unwrap();
    let page = key("a.md");
    store.create_draft(page.clone()).await.unwrap();
    store
        .save_local(page.clone(), "sent".into(), LocalRevision(1))
        .await
        .unwrap();
    let sent = store
        .claim_pending(&page.connection, 1)
        .await
        .unwrap()
        .remove(0);
    store
        .save_local(page.clone(), "newer".into(), LocalRevision(2))
        .await
        .unwrap();
    store
        .acknowledge(
            sent.id,
            MutationAck {
                remote_revision: None,
            },
        )
        .await
        .unwrap();
    let current = store.load_page(&page).await.unwrap().unwrap();
    assert_eq!(current.body, "newer");
    assert_eq!(current.remote_base.unwrap().revision, None);
    assert!(
        store
            .claim_pending(&page.connection, 1)
            .await
            .unwrap()
            .is_empty()
    );
    store.shutdown().await.unwrap();
}
