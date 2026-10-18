mod support;

use bulletnotes::domain::{connection::ConnectionConfig, error::StoreError, reconciliation::*};
use bulletnotes::{
    domain::{editor::EditorPosition, page::*},
    store::StoreHandle,
};
use support::storage_fixture::{database, key};

async fn observation(store: &StoreHandle, page: PageKey, body: &str) -> AutomaticRecoveryInput {
    store
        .apply_remote(RemoteRead::Present(RemoteFile {
            key: page.clone(),
            body: "base".into(),
            revision: Some(RemoteRevision("base-etag".into())),
            modified_ms: None,
            permission: Permission::ReadWrite,
        }))
        .await
        .unwrap();
    store
        .save_local(page.clone(), body.into(), LocalRevision(1))
        .await
        .unwrap();
    let local = store.load_page(&page).await.unwrap().unwrap();
    AutomaticRecoveryInput {
        base: local.remote_base.clone(),
        local,
        remote: RemoteRead::Present(RemoteFile {
            key: page,
            body: "remote".into(),
            revision: Some(RemoteRevision("remote-etag".into())),
            modified_ms: None,
            permission: Permission::ReadWrite,
        }),
        draft: None,
        kept_body: "chosen main source".into(),
        reason: AutomaticRecoveryReason::Overlap,
    }
}

#[tokio::test]
async fn recovery_first_create_keeps_exact_original_after_local_edits() {
    let directory = tempfile::tempdir().unwrap();
    let store = StoreHandle::open(&database(&directory)).await.unwrap();
    let original = "---\r\ntitle: 日本語\r\n---\r\n👩🏽‍💻 no final newline";
    let input = observation(&store, key("folder/original.md"), original).await;
    let event = store
        .register_automatic_recovery(input.clone())
        .await
        .unwrap();
    let heads = store
        .recovery_uploads(&input.local.key.connection, 10)
        .await
        .unwrap();
    assert_eq!(heads.len(), 1);
    assert_eq!(
        heads[0].key.path.as_str(),
        format!("Bulletnotes Recovery/original-{}.md", event.id)
    );
    let destination = heads[0].key.clone();
    let recovery_page = store.load_page(&destination).await.unwrap().unwrap();
    store
        .save_local(
            destination.clone(),
            "later edit".into(),
            recovery_page.local_revision.next().unwrap(),
        )
        .await
        .unwrap();
    assert!(
        store
            .claim_pending(&destination.connection, 100)
            .await
            .unwrap()
            .iter()
            .all(|op| op.key != destination)
    );
    let upload = store
        .claim_recovery_upload(event.id, 0)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(upload.operation.kind, MutationKind::Put(original.into()));
    assert_eq!(upload.operation.condition, WriteCondition::CreateOnly);
    assert!(
        store
            .claim_recovery_upload(event.id, 0)
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        store
            .automatic_recovery_event(event.id)
            .await
            .unwrap()
            .input,
        input
    );
    store
        .acknowledge_recovery_upload(
            event.id,
            0,
            upload.operation.id,
            MutationAck {
                remote_revision: Some(RemoteRevision("created-etag".into())),
            },
        )
        .await
        .unwrap();
    let ordinary = store
        .claim_pending(&destination.connection, 100)
        .await
        .unwrap();
    let edited = ordinary.iter().find(|op| op.key == destination).unwrap();
    assert_eq!(edited.kind, MutationKind::Put("later edit".into()));
    assert_eq!(
        edited.condition,
        WriteCondition::Match(RemoteRevision("created-etag".into()))
    );
    assert_eq!(
        store
            .automatic_recovery_event(event.id)
            .await
            .unwrap()
            .input
            .local
            .body,
        original
    );
    store.shutdown().await.unwrap();
}

#[tokio::test]
async fn duplicate_observations_reuse_persisted_ids_but_paths_and_connections_do_not() {
    let directory = tempfile::tempdir().unwrap();
    let store = StoreHandle::open(&database(&directory)).await.unwrap();
    let input = observation(&store, key("first/same.md"), "local").await;
    let first = store
        .register_automatic_recovery(input.clone())
        .await
        .unwrap();
    let original_heads = store
        .recovery_uploads(&input.local.key.connection, 100)
        .await
        .unwrap();
    let again = store
        .register_automatic_recovery(input.clone())
        .await
        .unwrap();
    assert_eq!(first.id, again.id);
    let mut recomputed = input.clone();
    recomputed.reason = AutomaticRecoveryReason::BudgetFallback;
    recomputed.kept_body = "different computation".into();
    recomputed.local.local_revision = LocalRevision(100);
    assert_eq!(
        store.register_automatic_recovery(recomputed).await.unwrap(),
        first
    );
    assert_eq!(
        store
            .recovery_uploads(&input.local.key.connection, 100)
            .await
            .unwrap(),
        original_heads
    );
    let second = observation(&store, key("second/same.md"), "local").await;
    assert_ne!(
        first.id,
        store.register_automatic_recovery(second).await.unwrap().id
    );
    let other_key = PageKey {
        connection: ConnectionConfig::parse("https://other.example/")
            .unwrap()
            .id()
            .clone(),
        path: input.local.key.path.clone(),
    };
    let other = observation(&store, other_key, "local").await;
    assert_ne!(
        first.id,
        store.register_automatic_recovery(other).await.unwrap().id
    );
    let mut changed = input;
    changed.remote = RemoteRead::Missing(changed.local.key.clone());
    assert_ne!(
        first.id,
        store.register_automatic_recovery(changed).await.unwrap().id
    );
    store.shutdown().await.unwrap();
}

#[tokio::test]
async fn relocation_retains_late_attempt_discovery_and_newer_local_edits_across_restart() {
    let directory = tempfile::tempdir().unwrap();
    let path = database(&directory);
    let store = StoreHandle::open(&path).await.unwrap();
    let input = observation(&store, key("late.md"), "immutable").await;
    let event = store.register_automatic_recovery(input).await.unwrap();
    let old = store
        .claim_recovery_upload(event.id, 0)
        .await
        .unwrap()
        .unwrap();
    store
        .save_local(
            old.operation.key.clone(),
            "edited recovery".into(),
            LocalRevision(2),
        )
        .await
        .unwrap();
    let occupant = RemoteFile {
        key: old.operation.key.clone(),
        body: "occupied".into(),
        revision: Some(RemoteRevision("occupied-etag".into())),
        modified_ms: None,
        permission: Permission::ReadOnly,
    };
    let next = store
        .relocate_recovery_upload(event.id, 0, old.operation.id, occupant.clone())
        .await
        .unwrap();
    assert_eq!(
        store
            .load_page(&next.operation.key)
            .await
            .unwrap()
            .unwrap()
            .body,
        "edited recovery"
    );
    assert_eq!(
        store
            .load_page(&old.operation.key)
            .await
            .unwrap()
            .unwrap()
            .body,
        occupant.body
    );
    store.shutdown().await.unwrap();
    let reopened = StoreHandle::open(&path).await.unwrap();
    let heads = reopened
        .recovery_uploads(&next.operation.key.connection, 10)
        .await
        .unwrap();
    assert_eq!(heads.len(), 2);
    assert!(
        heads
            .iter()
            .any(|head| head.operation_id == old.operation.id
                && head.state == RecoveryUploadState::Unknown)
    );
    assert!(
        heads
            .iter()
            .any(|head| head.operation_id == next.operation.id
                && head.state == RecoveryUploadState::Pending)
    );
    assert_eq!(
        reopened
            .read_recovery_upload(event.id, 0, old.operation.id)
            .await
            .unwrap(),
        old
    );
    assert!(matches!(
        reopened
            .read_recovery_upload(event.id, 1, old.operation.id)
            .await,
        Err(StoreError::UnknownOperation)
    ));
    reopened.shutdown().await.unwrap();
}

#[tokio::test]
async fn filename_stems_are_bounded_ascii_and_empty_drafts_remain_exact() {
    let directory = tempfile::tempdir().unwrap();
    let store = StoreHandle::open(&database(&directory)).await.unwrap();
    for (filename, expected) in [
        ("folder/日本語💾.md".to_string(), "-".to_string()),
        ("folder/.md".to_string(), "note".to_string()),
        (format!("folder/{}.md", "A".repeat(80)), "A".repeat(64)),
        ("folder/a  💾b_C-2.md".to_string(), "a-b_C-2".to_string()),
    ] {
        let mut input = observation(&store, key(&filename), "local").await;
        input.draft = Some(String::new());
        let event = store.register_automatic_recovery(input).await.unwrap();
        let upload = store
            .claim_recovery_upload(event.id, 0)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            upload.operation.key.path.as_str(),
            format!("Bulletnotes Recovery/{expected}-{}.md", event.id)
        );
        let draft = store
            .claim_recovery_upload(event.id, 1)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(draft.operation.kind, MutationKind::Put(String::new()));
    }
    store.shutdown().await.unwrap();
}

#[tokio::test]
async fn digest_collision_does_not_reuse_a_different_original_tuple() {
    let directory = tempfile::tempdir().unwrap();
    let path = database(&directory);
    let store = StoreHandle::open(&path).await.unwrap();
    let first_input = observation(&store, key("first.md"), "first").await;
    let first = store
        .register_automatic_recovery(first_input.clone())
        .await
        .unwrap();
    let second_input = observation(&store, key("second.md"), "second").await;
    let second = store
        .register_automatic_recovery(second_input.clone())
        .await
        .unwrap();
    let db = rusqlite::Connection::open(&path).unwrap();
    // Simulate a digest collision in generated storage without weakening production hashing.
    let fingerprint: Vec<u8> = db
        .query_row(
            "SELECT fingerprint FROM automatic_events WHERE id=?1",
            [second.id.to_string()],
            |row| row.get(0),
        )
        .unwrap();
    db.execute_batch("DROP TRIGGER automatic_events_immutable")
        .unwrap();
    db.execute(
        "UPDATE automatic_events SET fingerprint=?1 WHERE id=?2",
        rusqlite::params![fingerprint, first.id.to_string()],
    )
    .unwrap();
    db.execute(
        "DELETE FROM recovery_uploads WHERE event_id=?1",
        [second.id.to_string()],
    )
    .unwrap();
    db.execute(
        "DELETE FROM automatic_events WHERE id=?1",
        [second.id.to_string()],
    )
    .unwrap();
    let recreated = store
        .register_automatic_recovery(second_input.clone())
        .await
        .unwrap();
    assert_ne!(recreated.id, first.id);
    assert_eq!(recreated.input, second_input);
    assert_eq!(
        store
            .automatic_recovery_event(first.id)
            .await
            .unwrap()
            .input,
        first_input
    );
    store.shutdown().await.unwrap();
}

#[tokio::test]
async fn database_constraints_reject_mutating_event_or_attempt_sources() {
    let directory = tempfile::tempdir().unwrap();
    let path = database(&directory);
    let store = StoreHandle::open(&path).await.unwrap();
    let input = observation(&store, key("immutable.md"), "original").await;
    let event = store
        .register_automatic_recovery(input.clone())
        .await
        .unwrap();
    let db = rusqlite::Connection::open(&path).unwrap();
    for sql in [
        "UPDATE automatic_events SET snapshot='replacement'",
        "UPDATE recovery_uploads SET source='replacement'",
        "UPDATE recovery_uploads SET state='invalid'",
        "UPDATE pages SET source_generation=-1",
        "INSERT INTO merge_drafts(connection,path,body,revision) VALUES('fixture','a.md','x',-1)",
    ] {
        assert!(db.execute(sql, []).is_err(), "accepted {sql}");
    }
    assert_eq!(
        store
            .automatic_recovery_event(event.id)
            .await
            .unwrap()
            .input,
        input
    );
    assert_eq!(
        store
            .claim_recovery_upload(event.id, 0)
            .await
            .unwrap()
            .unwrap()
            .operation
            .kind,
        MutationKind::Put("original".into())
    );
    store.shutdown().await.unwrap();
}

#[tokio::test]
async fn restart_recovers_leased_immutable_upload_as_unknown() {
    let directory = tempfile::tempdir().unwrap();
    let path = database(&directory);
    let store = StoreHandle::open(&path).await.unwrap();
    let input = observation(&store, key("restart.md"), "local").await;
    let event = store
        .register_automatic_recovery(input.clone())
        .await
        .unwrap();
    let upload = store
        .claim_recovery_upload(event.id, 0)
        .await
        .unwrap()
        .unwrap();
    store.shutdown().await.unwrap();
    let reopened = StoreHandle::open(&path).await.unwrap();
    let heads = reopened
        .recovery_uploads(&input.local.key.connection, 1)
        .await
        .unwrap();
    assert_eq!(heads[0].state, RecoveryUploadState::Unknown);
    assert_eq!(heads[0].operation_id, upload.operation.id);
    assert_eq!(
        reopened
            .read_recovery_upload(event.id, 0, upload.operation.id)
            .await
            .unwrap(),
        upload
    );
    assert!(
        reopened
            .claim_recovery_upload(event.id, 0)
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        reopened
            .register_automatic_recovery(input)
            .await
            .unwrap()
            .id,
        event.id
    );
    reopened.shutdown().await.unwrap();
}

#[tokio::test]
async fn occupied_destinations_relocate_without_accepting_stale_attempt_completions() {
    let directory = tempfile::tempdir().unwrap();
    let store = StoreHandle::open(&database(&directory)).await.unwrap();
    let input = observation(&store, key("occupied.md"), "original local").await;
    let event = store
        .register_automatic_recovery(input.clone())
        .await
        .unwrap();
    let old = store
        .claim_recovery_upload(event.id, 0)
        .await
        .unwrap()
        .unwrap();
    store
        .mark_recovery_unknown(event.id, 0, old.operation.id)
        .await
        .unwrap();
    let occupant = RemoteFile {
        key: old.operation.key.clone(),
        body: "someone else's source".into(),
        revision: Some(RemoteRevision("occupied-etag".into())),
        modified_ms: None,
        permission: Permission::ReadWrite,
    };
    let replacement = store
        .relocate_recovery_upload(event.id, 0, old.operation.id, occupant.clone())
        .await
        .unwrap();
    assert_ne!(replacement.operation.id, old.operation.id);
    assert_ne!(replacement.operation.key, old.operation.key);
    assert_eq!(replacement.operation.kind, old.operation.kind);
    assert_eq!(replacement.operation.condition, WriteCondition::CreateOnly);
    assert_eq!(
        store
            .load_page(&old.operation.key)
            .await
            .unwrap()
            .unwrap()
            .body,
        occupant.body
    );
    assert_eq!(
        store
            .read_recovery_upload(event.id, 0, old.operation.id)
            .await
            .unwrap(),
        old
    );
    assert!(
        store
            .relocate_recovery_upload(event.id, 0, old.operation.id, occupant)
            .await
            .is_err()
    );
    store
        .acknowledge_recovery_upload(
            event.id,
            0,
            old.operation.id,
            MutationAck {
                remote_revision: Some(RemoteRevision("late-old".into())),
            },
        )
        .await
        .unwrap();
    let heads = store
        .recovery_uploads(&input.local.key.connection, 10)
        .await
        .unwrap();
    assert_eq!(heads.len(), 1);
    assert_eq!(heads[0].operation_id, replacement.operation.id);
    assert_eq!(heads[0].state, RecoveryUploadState::Pending);
    assert!(
        store
            .mark_recovery_unknown(event.id, 0, old.operation.id)
            .await
            .is_err()
    );
    assert_eq!(
        store
            .claim_recovery_upload(event.id, 0)
            .await
            .unwrap()
            .unwrap(),
        replacement
    );
    store.shutdown().await.unwrap();
}

#[tokio::test]
async fn event_and_obligation_survive_original_page_deletion() {
    let directory = tempfile::tempdir().unwrap();
    let store = StoreHandle::open(&database(&directory)).await.unwrap();
    let input = observation(&store, key("deleted.md"), "local").await;
    let event = store
        .register_automatic_recovery(input.clone())
        .await
        .unwrap();
    store
        .move_to_trash(input.local.key.clone(), input.local.local_revision)
        .await
        .unwrap();
    let delete = store
        .claim_pending(&input.local.key.connection, 10)
        .await
        .unwrap()
        .into_iter()
        .find(|op| op.key == input.local.key)
        .unwrap();
    assert_eq!(delete.kind, MutationKind::Delete);
    store
        .acknowledge(
            delete.id,
            MutationAck {
                remote_revision: None,
            },
        )
        .await
        .unwrap();
    assert!(store.load_page(&input.local.key).await.unwrap().is_none());
    assert_eq!(
        store
            .automatic_recovery_event(event.id)
            .await
            .unwrap()
            .input,
        input
    );
    assert_eq!(
        store
            .recovery_uploads(&input.local.key.connection, 10)
            .await
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        store
            .cache_progress(&input.local.key.connection)
            .await
            .unwrap()
            .pending,
        1
    );
    store.shutdown().await.unwrap();
}

#[tokio::test]
async fn authored_draft_has_distinct_exact_source_and_trash_waits_for_first_create() {
    let directory = tempfile::tempdir().unwrap();
    let store = StoreHandle::open(&database(&directory)).await.unwrap();
    let mut input = observation(&store, key("draft.md"), "").await;
    input.draft = Some("authored draft\r\n💾".into());
    let event = store
        .register_automatic_recovery(input.clone())
        .await
        .unwrap();
    let uploads = store
        .recovery_uploads(&input.local.key.connection, 10)
        .await
        .unwrap();
    assert_eq!(uploads.len(), 2);
    assert_ne!(uploads[0].key, uploads[1].key);
    for head in uploads {
        let page = store.load_page(&head.key).await.unwrap().unwrap();
        store
            .move_to_trash(head.key.clone(), page.local_revision)
            .await
            .unwrap();
        assert!(
            store
                .claim_pending(&head.key.connection, 10)
                .await
                .unwrap()
                .iter()
                .all(|op| op.key != head.key)
        );
        let upload = store
            .claim_recovery_upload(event.id, head.ordinal)
            .await
            .unwrap()
            .unwrap();
        let expected = if head.ordinal == 0 {
            ""
        } else {
            input.draft.as_deref().unwrap()
        };
        assert_eq!(upload.operation.kind, MutationKind::Put(expected.into()));
        store
            .acknowledge_recovery_upload(
                event.id,
                head.ordinal,
                upload.operation.id,
                MutationAck {
                    remote_revision: Some(RemoteRevision("created".into())),
                },
            )
            .await
            .unwrap();
        let deleted = store
            .claim_pending(&head.key.connection, 10)
            .await
            .unwrap()
            .into_iter()
            .find(|op| op.key == head.key)
            .unwrap();
        assert_eq!(deleted.kind, MutationKind::Delete);
        assert_eq!(
            deleted.condition,
            WriteCondition::Match(RemoteRevision("created".into()))
        );
    }
    store.shutdown().await.unwrap();
}

#[tokio::test]
async fn blocked_and_unknown_jobs_keep_original_and_connection_pending() {
    let directory = tempfile::tempdir().unwrap();
    let path = database(&directory);
    let store = StoreHandle::open(&path).await.unwrap();
    let input = observation(&store, key("pending.md"), "local").await;
    let original = store
        .claim_pending(&input.local.key.connection, 1)
        .await
        .unwrap()
        .remove(0);
    store
        .acknowledge(
            original.id,
            MutationAck {
                remote_revision: Some(RemoteRevision("synced-original".into())),
            },
        )
        .await
        .unwrap();
    let event = store
        .register_automatic_recovery(input.clone())
        .await
        .unwrap();
    let db = rusqlite::Connection::open(&path).unwrap();
    for state in ["blocked", "unknown"] {
        db.execute("UPDATE recovery_uploads SET state=?1", [state])
            .unwrap();
        assert!(
            store
                .load_sync_page(&input.local.key)
                .await
                .unwrap()
                .unwrap()
                .1
                .pending,
            "{state} recovery cannot appear synced"
        );
        assert_eq!(
            store
                .cache_progress(&input.local.key.connection)
                .await
                .unwrap()
                .pending,
            1
        );
    }
    let head = store
        .recovery_uploads(&input.local.key.connection, 1)
        .await
        .unwrap()
        .remove(0);
    assert_eq!(head.event_id, event.id);
    assert_eq!(head.state, RecoveryUploadState::Unknown);
    assert!(matches!(
        store
            .recovery_uploads(&input.local.key.connection, usize::MAX)
            .await,
        Err(StoreError::InvalidQuery)
    ));
    store.shutdown().await.unwrap();
}

#[tokio::test]
async fn stored_snapshots_expose_current_page_and_draft_tokens() {
    let directory = tempfile::tempdir().unwrap();
    let store = StoreHandle::open(&database(&directory)).await.unwrap();
    let input = observation(&store, key("tokens.md"), "local").await;
    let first = store
        .load_stored_page(&input.local.key)
        .await
        .unwrap()
        .expect("stored page must expose its source generation");
    assert_eq!(first.page, input.local);
    store
        .save_local(input.local.key.clone(), "newer".into(), LocalRevision(2))
        .await
        .unwrap();
    let next = store
        .load_stored_page(&input.local.key)
        .await
        .unwrap()
        .unwrap();
    assert!(next.source_generation > first.source_generation);
    store
        .save_conflict(ConflictRecord {
            id: uuid::Uuid::new_v4(),
            key: input.local.key.clone(),
            base: input.base,
            local: next.page,
            remote: input.remote,
        })
        .await
        .unwrap();
    store
        .save_merge_drafts(vec![(input.local.key.clone(), "authored".into())])
        .await
        .unwrap();
    let draft = store
        .load_stored_merge_draft(&input.local.key)
        .await
        .unwrap()
        .expect("draft must expose its revision");
    assert_eq!(draft.body, "authored");
    store
        .save_merge_drafts(vec![(input.local.key.clone(), "newer authored".into())])
        .await
        .unwrap();
    assert!(
        store
            .load_stored_merge_draft(&input.local.key)
            .await
            .unwrap()
            .unwrap()
            .revision
            > draft.revision
    );
    store.shutdown().await.unwrap();
}

#[tokio::test]
async fn conflict_observation_changes_invalidate_source_generation_even_with_same_id() {
    let directory = tempfile::tempdir().unwrap();
    let path = database(&directory);
    let store = StoreHandle::open(&path).await.unwrap();
    let input = observation(&store, key("conflict.md"), "local").await;
    let generation = async || {
        store
            .load_stored_page(&input.local.key)
            .await
            .unwrap()
            .unwrap()
            .source_generation
    };
    let before = generation().await;
    let mut conflict = ConflictRecord {
        id: uuid::Uuid::new_v4(),
        key: input.local.key.clone(),
        base: input.base.clone(),
        local: input.local.clone(),
        remote: input.remote.clone(),
    };
    store.save_conflict(conflict.clone()).await.unwrap();
    let captured = generation().await;
    assert!(
        captured > before,
        "new conflict observation must invalidate source token"
    );
    let db = rusqlite::Connection::open(&path).unwrap();
    if let RemoteRead::Present(remote) = &mut conflict.remote {
        remote.body = "new remote observation".into();
    }
    db.execute(
        "UPDATE conflicts SET snapshot=?1 WHERE id=?2",
        rusqlite::params![
            serde_json::to_string(&conflict).unwrap(),
            conflict.id.to_string()
        ],
    )
    .unwrap();
    let changed = generation().await;
    assert!(changed > captured);
    db.execute("DELETE FROM conflicts", []).unwrap();
    assert!(generation().await > changed);
    store.shutdown().await.unwrap();
}

#[tokio::test]
async fn failed_registration_rolls_back_event_attempt_and_recovery_page() {
    let directory = tempfile::tempdir().unwrap();
    let path = database(&directory);
    let store = StoreHandle::open(&path).await.unwrap();
    let input = observation(&store, key("failure.md"), "local").await;
    let db = rusqlite::Connection::open(&path).unwrap();
    db.execute_batch("CREATE TRIGGER fail_recovery BEFORE INSERT ON recovery_uploads BEGIN SELECT RAISE(ABORT,'simulated failure'); END;").unwrap();
    assert!(
        store
            .register_automatic_recovery(input.clone())
            .await
            .is_err()
    );
    assert_eq!(
        store.load_page(&input.local.key).await.unwrap().unwrap(),
        input.local
    );
    for table in ["automatic_events", "recovery_uploads"] {
        let count: i64 = db
            .query_row(&format!("SELECT count(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(count, 0);
    }
    let pages: i64 = db
        .query_row("SELECT count(*) FROM pages", [], |row| row.get(0))
        .unwrap();
    assert_eq!(pages, 1);
    assert_eq!(
        store
            .claim_pending(&input.local.key.connection, 10)
            .await
            .unwrap()[0]
            .kind,
        MutationKind::Put("local".into())
    );
    store.shutdown().await.unwrap();
}

#[test]
fn migration_failure_preserves_legacy_rows_and_backup() {
    use bulletnotes::store::recovery::{Migration, migrate};
    let directory = tempfile::tempdir().unwrap();
    let path = database(&directory);
    let mut db = rusqlite::Connection::open(&path).unwrap();
    for sql in [
        include_str!("../migrations/001_initial.sql"),
        include_str!("../migrations/002_probe_journal.sql"),
        include_str!("../migrations/003_remote_cache.sql"),
        include_str!("../migrations/004_recovery_intents.sql"),
    ] {
        db.execute_batch(sql).unwrap();
    }
    db.execute_batch("PRAGMA user_version=4; INSERT INTO pages(connection,path,title,body) VALUES('fixture','legacy.md','legacy','retained'); INSERT INTO merge_drafts VALUES('fixture','legacy.md','draft');").unwrap();
    let sql = format!(
        "{} invalid SQL;",
        include_str!("../migrations/005_automatic_reconciliation.sql")
    );
    let backup = directory.path().join("backup.sqlite");
    assert!(matches!(
        migrate(
            &mut db,
            &[Migration {
                version: 5,
                sql: &sql
            }],
            Some(&backup)
        ),
        Err(StoreError::MigrationFailed)
    ));
    for db in [&db, &rusqlite::Connection::open(&backup).unwrap()] {
        assert_eq!(
            db.query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))
                .unwrap(),
            4
        );
        assert_eq!(
            db.query_row("SELECT body FROM pages", [], |row| row.get::<_, String>(0))
                .unwrap(),
            "retained"
        );
        assert_eq!(
            db.query_row("SELECT body FROM merge_drafts", [], |row| row
                .get::<_, String>(0))
                .unwrap(),
            "draft"
        );
        assert!(db.prepare("SELECT source_generation FROM pages").is_err());
    }
}

#[tokio::test]
async fn source_generation_tracks_source_base_permission_and_deletion_not_position() {
    let directory = tempfile::tempdir().unwrap();
    let path = database(&directory);
    let store = StoreHandle::open(&path).await.unwrap();
    let page = key("generation.md");
    store.create_draft(page.clone()).await.unwrap();
    let db = rusqlite::Connection::open(&path).unwrap();
    let columns: Vec<String> = db
        .prepare("PRAGMA table_info(pages)")
        .unwrap()
        .query_map([], |row| row.get(1))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert!(
        columns.iter().any(|name| name == "source_generation"),
        "source generation must be durable"
    );
    let generation = || {
        db.query_row("SELECT source_generation FROM pages", [], |row| {
            row.get::<_, i64>(0)
        })
        .unwrap()
    };
    let initial = generation();
    store
        .save_position(&page, EditorPosition::default(), 1)
        .await
        .unwrap();
    assert_eq!(generation(), initial);
    store
        .save_local(page.clone(), "local".into(), LocalRevision(1))
        .await
        .unwrap();
    assert!(generation() > initial);
    for change in [
        "remote_body='base'",
        "remote_revision='etag'",
        "permission=0",
        "remote_deleted=1",
        "trashed=1",
    ] {
        let before = generation();
        db.execute(&format!("UPDATE pages SET {change}"), [])
            .unwrap();
        assert!(generation() > before, "generation missed {change}");
    }
    let before = generation();
    db.execute("UPDATE pages SET body=body,permission=permission", [])
        .unwrap();
    assert_eq!(generation(), before);
    store.shutdown().await.unwrap();
}

#[tokio::test]
async fn migration_initializes_legacy_draft_revision_without_changing_source() {
    let directory = tempfile::tempdir().unwrap();
    let path = database(&directory);
    let db = rusqlite::Connection::open(&path).unwrap();
    for sql in [
        include_str!("../migrations/001_initial.sql"),
        include_str!("../migrations/002_probe_journal.sql"),
        include_str!("../migrations/003_remote_cache.sql"),
        include_str!("../migrations/004_recovery_intents.sql"),
    ] {
        db.execute_batch(sql).unwrap();
    }
    db.execute_batch("PRAGMA user_version=4").unwrap();
    let page = key("legacy.md");
    let body = "---\r\n日本語 👩🏽‍💻\r\nno final newline";
    db.execute(
        "INSERT INTO merge_drafts(connection,path,body) VALUES(?1,?2,?3)",
        rusqlite::params![page.connection.as_str(), page.path.as_str(), body],
    )
    .unwrap();
    let store = StoreHandle::open(&path).await.unwrap();
    assert_eq!(
        store.merge_draft(&page).await.unwrap().as_deref(),
        Some(body)
    );
    let columns: Vec<String> = db
        .prepare("PRAGMA table_info(merge_drafts)")
        .unwrap()
        .query_map([], |row| row.get(1))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert!(
        columns.iter().any(|name| name == "revision"),
        "legacy drafts need a durable revision"
    );
    let revision: i64 = db
        .query_row("SELECT revision FROM merge_drafts", [], |row| row.get(0))
        .unwrap();
    assert!(revision >= 0);
    db.execute("DELETE FROM merge_drafts", []).unwrap();
    db.execute(
        "INSERT INTO merge_drafts(connection,path,body) VALUES(?1,?2,?3)",
        rusqlite::params![page.connection.as_str(), page.path.as_str(), body],
    )
    .unwrap();
    let recreated: i64 = db
        .query_row("SELECT revision FROM merge_drafts", [], |row| row.get(0))
        .unwrap();
    assert!(recreated > revision);
    db.execute("UPDATE merge_drafts SET body=body", []).unwrap();
    let rewritten: i64 = db
        .query_row("SELECT revision FROM merge_drafts", [], |row| row.get(0))
        .unwrap();
    assert!(rewritten > recreated);
    store.shutdown().await.unwrap();
}

#[tokio::test]
async fn same_path_recreation_gets_a_new_generation_and_overflow_rolls_back() {
    let directory = tempfile::tempdir().unwrap();
    let path = database(&directory);
    let store = StoreHandle::open(&path).await.unwrap();
    let page = key("recreated.md");
    store.create_draft(page.clone()).await.unwrap();
    let db = rusqlite::Connection::open(&path).unwrap();
    let generation = || {
        db.query_row("SELECT source_generation FROM pages", [], |row| {
            row.get::<_, i64>(0)
        })
        .unwrap_or(0)
    };
    let before = generation();
    db.execute("DELETE FROM pages", []).unwrap();
    store.create_draft(page.clone()).await.unwrap();
    assert!(
        generation() > before,
        "recreation must invalidate old source tokens"
    );
    db.execute("UPDATE source_clock SET generation=9223372036854775807", [])
        .unwrap();
    assert!(
        store
            .save_local(page.clone(), "must roll back".into(), LocalRevision(1))
            .await
            .is_err()
    );
    assert_eq!(store.load_page(&page).await.unwrap().unwrap().body, "");
    assert_eq!(generation(), before + 1);
    store.shutdown().await.unwrap();
}
