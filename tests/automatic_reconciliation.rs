use bulletnotes::{
    domain::{connection::ConnectionConfig, error::StoreError, page::*},
    store::StoreHandle,
};

fn key() -> PageKey {
    PageKey {
        connection: ConnectionConfig::parse("https://automatic.example/")
            .unwrap()
            .id()
            .clone(),
        path: PagePath::parse("note.md").unwrap(),
    }
}
fn file(body: &str, revision: &str) -> RemoteFile {
    RemoteFile {
        key: key(),
        body: body.into(),
        revision: Some(RemoteRevision(revision.into())),
        modified_ms: None,
        permission: Permission::ReadWrite,
    }
}
async fn setup(base: Option<&str>, local: &str) -> (tempfile::TempDir, StoreHandle) {
    let dir = tempfile::tempdir().unwrap();
    let store = StoreHandle::open(&dir.path().join("notes.sqlite"))
        .await
        .unwrap();
    if let Some(base) = base {
        store
            .apply_remote(RemoteRead::Present(file(base, "a")))
            .await
            .unwrap();
    } else {
        store.create_draft(key()).await.unwrap();
    }
    store
        .save_local(key(), local.into(), LocalRevision(1))
        .await
        .unwrap();
    (dir, store)
}

#[tokio::test]
async fn disjoint_changes_merge_without_fabricating_authored_revision() {
    let (_dir, store) = setup(Some("red fox jumps"), "blue fox jumps").await;
    let before = store.load_stored_page(&key()).await.unwrap().unwrap();
    store
        .apply_remote(RemoteRead::Present(file("red fox runs", "b")))
        .await
        .unwrap();
    let after = store.load_stored_page(&key()).await.unwrap().unwrap();
    assert_eq!(after.page.body, "blue fox runs");
    assert_eq!(after.page.local_revision, LocalRevision(1));
    assert!(after.source_generation > before.source_generation);
    assert!(store.conflicts(&key().connection).await.unwrap().is_empty());
    let op = store
        .claim_pending(&key().connection, 1)
        .await
        .unwrap()
        .remove(0);
    assert_eq!(op.kind, MutationKind::Put("blue fox runs".into()));
    assert_eq!(
        op.condition,
        WriteCondition::Match(RemoteRevision("b".into()))
    );
    assert!(
        store
            .recovery_uploads(&key().connection, 10)
            .await
            .unwrap()
            .is_empty()
    );
    let history = store.recovery_copies(&key().connection).await.unwrap();
    for body in [
        "red fox jumps",
        "blue fox jumps",
        "red fox runs",
        "blue fox runs",
    ] {
        assert!(
            history.iter().any(|copy| copy.page.body == body),
            "missing reversible merge source: {body}"
        );
    }
    store.shutdown().await.unwrap();
}

#[tokio::test]
async fn overlap_keeps_independent_local_text_and_exact_originals() {
    let original = "blue fox jumps\r\n\r\nlocal paragraph 中文🙂";
    let remote = "green fox jumps\r\n\r\nbase paragraph 中文🙂";
    let (_dir, store) = setup(Some("red fox jumps\r\n\r\nbase paragraph 中文🙂"), original).await;
    store
        .apply_remote(RemoteRead::Present(file(remote, "b")))
        .await
        .unwrap();
    let chosen = "green fox jumps\r\n\r\nlocal paragraph 中文🙂";
    assert_eq!(store.load_page(&key()).await.unwrap().unwrap().body, chosen);
    let heads = store.recovery_uploads(&key().connection, 10).await.unwrap();
    assert_eq!(heads.len(), 1);
    let event = store
        .automatic_recovery_event(heads[0].event_id)
        .await
        .unwrap();
    assert_eq!(event.input.local.body, original);
    assert_eq!(event.input.kept_body, chosen);
    let upload = store
        .claim_recovery_upload(heads[0].event_id, 0)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(upload.operation.kind, MutationKind::Put(original.into()));
    let copies = store.recovery_copies(&key().connection).await.unwrap();
    for body in [
        original,
        remote,
        chosen,
        "red fox jumps\r\n\r\nbase paragraph 中文🙂",
    ] {
        assert!(
            copies.iter().any(|copy| copy.page.body == body),
            "missing kept source: {body}"
        );
    }
    store.shutdown().await.unwrap();
}

#[tokio::test]
async fn competing_creates_preserve_the_complete_local_document() {
    let (_dir, store) = setup(None, "local without base").await;
    store
        .apply_remote(RemoteRead::Present(file("occupied remote", "b")))
        .await
        .unwrap();
    assert_eq!(
        store.load_page(&key()).await.unwrap().unwrap().body,
        "occupied remote"
    );
    let heads = store.recovery_uploads(&key().connection, 10).await.unwrap();
    assert_eq!(heads.len(), 1);
    assert_eq!(
        store
            .automatic_recovery_event(heads[0].event_id)
            .await
            .unwrap()
            .input
            .local
            .body,
        "local without base"
    );
    store.shutdown().await.unwrap();
}

#[tokio::test]
async fn recovery_insert_failure_leaves_source_base_conflict_and_work_intact() {
    let (dir, store) = setup(Some("base"), "local").await;
    let before = store.load_stored_page(&key()).await.unwrap().unwrap();
    let pending = store.pending_heads(&key().connection).await.unwrap();
    let db = rusqlite::Connection::open(dir.path().join("notes.sqlite")).unwrap();
    db.execute_batch("CREATE TRIGGER fail_recovery BEFORE INSERT ON recovery_uploads BEGIN SELECT RAISE(ABORT, 'injected recovery failure'); END;").unwrap();
    assert!(
        store
            .apply_remote(RemoteRead::Present(file("remote", "b")))
            .await
            .is_err()
    );
    assert_eq!(
        store.load_stored_page(&key()).await.unwrap().unwrap(),
        before
    );
    assert_eq!(
        store.pending_heads(&key().connection).await.unwrap()[0].id,
        pending[0].id
    );
    assert!(store.conflicts(&key().connection).await.unwrap().is_empty());
    assert!(
        store
            .recovery_copies(&key().connection)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        store
            .recovery_uploads(&key().connection, 10)
            .await
            .unwrap()
            .is_empty()
    );
    store.shutdown().await.unwrap();
}

#[tokio::test]
async fn repeated_unchanged_observation_keeps_generation_and_operation_identity() {
    let (_dir, store) = setup(Some("base"), "local").await;
    let before = store.load_stored_page(&key()).await.unwrap().unwrap();
    let pending = store.pending_heads(&key().connection).await.unwrap()[0].id;
    store
        .apply_remote(RemoteRead::Present(file("base", "a")))
        .await
        .unwrap();
    assert_eq!(
        store.load_stored_page(&key()).await.unwrap().unwrap(),
        before
    );
    assert_eq!(
        store.pending_heads(&key().connection).await.unwrap()[0].id,
        pending
    );
    store.shutdown().await.unwrap();
}

#[tokio::test]
async fn manual_resolution_cannot_delete_unknown_payload() {
    let (_dir, store) = setup(Some("base"), "local").await;
    let op = store
        .claim_pending(&key().connection, 1)
        .await
        .unwrap()
        .remove(0);
    store.mark_unknown(op.id).await.unwrap();
    let local = store.load_page(&key()).await.unwrap().unwrap();
    let id = uuid::Uuid::new_v4();
    store
        .save_conflict(ConflictRecord {
            id,
            key: key(),
            base: local.remote_base.clone(),
            local,
            remote: RemoteRead::Present(file("remote", "b")),
        })
        .await
        .unwrap();
    assert!(matches!(
        store
            .resolve_conflict(ConflictResolution {
                conflict_id: id,
                expected_local: LocalRevision(1),
                choice: ResolutionChoice::AcceptRemote
            })
            .await,
        Err(StoreError::UnknownOperation)
    ));
    assert_eq!(
        store.unknown_mutations(&key().connection).await.unwrap(),
        vec![op]
    );
    store.shutdown().await.unwrap();
}

async fn prepared(
    store: &StoreHandle,
    remote: RemoteRead,
) -> bulletnotes::domain::reconciliation::PreparedResolution {
    let context = store.load_reconcile_context(&key(), remote).await.unwrap();
    bulletnotes::sync::reconcile::prepare_reconciliation(
        context,
        &bulletnotes::sync::merge_executor::MergeExecutor::default(),
    )
    .await
}
async fn legacy(store: &StoreHandle, remote: RemoteRead) -> ConflictRecord {
    let local = store.load_page(&key()).await.unwrap().unwrap();
    let record = ConflictRecord {
        id: uuid::Uuid::new_v4(),
        key: key(),
        base: local.remote_base.clone(),
        local,
        remote,
    };
    store.save_conflict(record.clone()).await.unwrap();
    record
}

#[tokio::test]
async fn newer_local_save_rejects_prepared_result_without_preserving_obsolete_work() {
    let (_dir, store) = setup(Some("base"), "local").await;
    let attempt = prepared(&store, RemoteRead::Present(file("remote", "b"))).await;
    store
        .save_local(key(), "later local".into(), LocalRevision(2))
        .await
        .unwrap();
    let before = store.load_stored_page(&key()).await.unwrap().unwrap();
    let pending = store.pending_heads(&key().connection).await.unwrap()[0].id;
    assert!(matches!(
        store.commit_reconciliation(attempt).await,
        Err(StoreError::StaleRevision)
    ));
    assert_eq!(
        store.load_stored_page(&key()).await.unwrap().unwrap(),
        before
    );
    assert_eq!(
        store.pending_heads(&key().connection).await.unwrap()[0].id,
        pending
    );
    assert!(
        store
            .recovery_uploads(&key().connection, 10)
            .await
            .unwrap()
            .is_empty()
    );
    store.shutdown().await.unwrap();
}

#[tokio::test]
async fn newer_draft_invalidates_resolution_and_keeps_both_original_and_draft_jobs() {
    let (_dir, store) = setup(Some("base"), "local").await;
    legacy(&store, RemoteRead::Present(file("remote", "b"))).await;
    store
        .save_merge_drafts(vec![(key(), "unfinished draft".into())])
        .await
        .unwrap();
    let attempt = prepared(&store, RemoteRead::Present(file("remote", "b"))).await;
    store
        .save_merge_drafts(vec![(key(), "new unfinished draft".into())])
        .await
        .unwrap();
    let current = store
        .load_stored_merge_draft(&key())
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(
        store.commit_reconciliation(attempt).await,
        Err(StoreError::StaleRevision)
    ));
    assert_eq!(
        store
            .load_stored_merge_draft(&key())
            .await
            .unwrap()
            .unwrap(),
        current
    );
    assert_eq!(store.conflicts(&key().connection).await.unwrap().len(), 1);
    assert!(
        store
            .recovery_uploads(&key().connection, 10)
            .await
            .unwrap()
            .is_empty()
    );
    store
        .apply_remote(RemoteRead::Present(file("remote", "b")))
        .await
        .unwrap();
    assert_eq!(
        store.load_page(&key()).await.unwrap().unwrap().body,
        "remote"
    );
    let heads = store.recovery_uploads(&key().connection, 10).await.unwrap();
    assert_eq!(heads.len(), 2);
    let draft = heads.iter().find(|head| head.ordinal == 1).unwrap();
    let job = store
        .read_recovery_upload(draft.event_id, draft.ordinal, draft.operation_id)
        .await
        .unwrap();
    assert_eq!(
        job.operation.kind,
        MutationKind::Put("new unfinished draft".into())
    );
    assert!(
        store
            .recovery_copies(&key().connection)
            .await
            .unwrap()
            .iter()
            .any(|copy| copy.page.body == "new unfinished draft")
    );
    assert!(store.merge_draft(&key()).await.unwrap().is_none());
    store.shutdown().await.unwrap();
}

#[tokio::test]
async fn active_mutation_state_change_rejects_prepared_result_and_defers_fresh_attempt() {
    let (_dir, store) = setup(Some("base"), "local").await;
    let attempt = prepared(&store, RemoteRead::Present(file("remote", "b"))).await;
    let op = store
        .claim_pending(&key().connection, 1)
        .await
        .unwrap()
        .remove(0);
    assert!(matches!(
        store.commit_reconciliation(attempt).await,
        Err(StoreError::StaleRevision)
    ));
    let before = store.load_stored_page(&key()).await.unwrap().unwrap();
    assert!(matches!(
        store
            .apply_remote(RemoteRead::Present(file("remote", "b")))
            .await,
        Err(StoreError::UnknownOperation)
    ));
    assert!(store.operation_is_leased(op.id).await.unwrap());
    store.mark_unknown(op.id).await.unwrap();
    assert!(matches!(
        store
            .apply_remote(RemoteRead::Present(file("remote", "b")))
            .await,
        Err(StoreError::UnknownOperation)
    ));
    assert_eq!(
        store.unknown_mutations(&key().connection).await.unwrap(),
        vec![op]
    );
    assert_eq!(
        store.load_stored_page(&key()).await.unwrap().unwrap(),
        before
    );
    store.shutdown().await.unwrap();
}

#[tokio::test]
async fn empty_present_is_a_document_and_missing_preserves_concurrent_edit_create_only() {
    for missing in [false, true] {
        let (_dir, store) = setup(Some("base"), "edited").await;
        let observation = if missing {
            RemoteRead::Missing(key())
        } else {
            RemoteRead::Present(file("", "b"))
        };
        store.apply_remote(observation).await.unwrap();
        let current = store.load_page(&key()).await.unwrap().unwrap();
        if missing {
            assert_eq!(current.body, "edited");
            assert!(current.remote_base.is_none());
            let op = store
                .claim_pending(&key().connection, 1)
                .await
                .unwrap()
                .remove(0);
            assert_eq!(op.condition, WriteCondition::CreateOnly);
            assert_eq!(op.kind, MutationKind::Put("edited".into()));
            assert!(
                store
                    .recovery_copies(&key().connection)
                    .await
                    .unwrap()
                    .iter()
                    .any(|copy| copy.reason == RecoveryReason::RemoteDeletion)
            );
        } else {
            assert_eq!(current.body, "");
            assert_eq!(current.remote_base.unwrap().body, "");
            assert_eq!(
                store
                    .recovery_uploads(&key().connection, 10)
                    .await
                    .unwrap()
                    .len(),
                1
            );
        }
        store.shutdown().await.unwrap();
    }
}

#[tokio::test]
async fn identical_body_new_validator_updates_base_without_fabricating_authored_revision() {
    let (_dir, store) = setup(Some("base"), "local").await;
    store
        .apply_remote(RemoteRead::Present(file("local", "b")))
        .await
        .unwrap();
    let before = store.load_stored_page(&key()).await.unwrap().unwrap();
    assert_eq!(before.page.local_revision, LocalRevision(1));
    assert_eq!(
        before.page.remote_base.as_ref().unwrap().revision,
        Some(RemoteRevision("b".into()))
    );
    assert!(
        store
            .pending_heads(&key().connection)
            .await
            .unwrap()
            .is_empty()
    );
    store
        .apply_remote(RemoteRead::Present(file("local", "b")))
        .await
        .unwrap();
    assert_eq!(
        store.load_stored_page(&key()).await.unwrap().unwrap(),
        before
    );
    store.shutdown().await.unwrap();
}

#[tokio::test]
async fn read_only_observation_keeps_merged_local_work_but_never_claims_its_put() {
    let (_dir, store) = setup(Some("red fox jumps"), "blue fox jumps").await;
    let mut observed = file("red fox runs", "b");
    observed.permission = Permission::ReadOnly;
    store
        .apply_remote(RemoteRead::Present(observed))
        .await
        .unwrap();
    let current = store.load_page(&key()).await.unwrap().unwrap();
    assert_eq!(current.body, "blue fox runs");
    assert_eq!(current.permission, Permission::ReadOnly);
    assert!(
        store
            .claim_pending(&key().connection, 10)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        store
            .pending_heads(&key().connection)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        store
            .load_sync_page(&key())
            .await
            .unwrap()
            .unwrap()
            .1
            .pending
    );
    store.shutdown().await.unwrap();
}

#[tokio::test]
async fn stale_editor_attempt_is_durable_before_reconciliation_and_survives_restart() {
    let (dir, store) = setup(Some("red fox jumps\n\nend"), "blue fox jumps\n\nend").await;
    let baseline = store.load_page(&key()).await.unwrap().unwrap();
    store
        .apply_remote(RemoteRead::Present(file("red fox runs\n\nend", "b")))
        .await
        .unwrap();
    let result = store
        .save_local_from_base_outcome(
            key(),
            "blue fox jumps\n\nend higher".into(),
            LocalRevision(2),
            EditorBaseline {
                body: baseline.body,
                remote_base: baseline.remote_base,
            },
        )
        .await
        .unwrap();
    assert_eq!(result.attempt.local_revision, LocalRevision(2));
    assert_eq!(result.stored.page.body, "blue fox runs\n\nend");
    assert_eq!(
        store
            .load_editor_page(&key())
            .await
            .unwrap()
            .unwrap()
            .presented()
            .body,
        "blue fox jumps\n\nend higher"
    );
    assert_eq!(
        result.stored,
        store.load_stored_page(&key()).await.unwrap().unwrap()
    );
    assert_eq!(
        store
            .pending_reconciliation_keys(&key().connection)
            .await
            .unwrap(),
        vec![key()]
    );
    assert!(
        store
            .recovery_copies(&key().connection)
            .await
            .unwrap()
            .iter()
            .any(|copy| copy.page.body == "blue fox runs\n\nend")
    );
    store.shutdown().await.unwrap();
    let store = StoreHandle::open(&dir.path().join("notes.sqlite"))
        .await
        .unwrap();
    assert_eq!(
        store.load_stored_page(&key()).await.unwrap().unwrap(),
        result.stored
    );
    store
        .apply_remote(RemoteRead::Present(file("red fox runs\n\nend", "b")))
        .await
        .unwrap();
    assert_eq!(
        store.load_page(&key()).await.unwrap().unwrap().body,
        "blue fox runs\n\nend higher"
    );
    assert!(store.conflicts(&key().connection).await.unwrap().is_empty());
    store.shutdown().await.unwrap();
}

#[tokio::test]
async fn concurrent_remote_edit_cancels_only_pending_local_deletion() {
    let (_dir, store) = setup(Some("base"), "base").await;
    let trash = store.move_to_trash(key(), LocalRevision(1)).await.unwrap();
    store
        .apply_remote(RemoteRead::Present(file("remote edit", "b")))
        .await
        .unwrap();
    assert_eq!(
        store
            .load_active_page(&key())
            .await
            .unwrap()
            .unwrap()
            .page
            .body,
        "remote edit"
    );
    assert!(
        store
            .claim_pending(&key().connection, 10)
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(store.trash(&key().connection).await.unwrap()[0].id, trash);
    store.shutdown().await.unwrap();
}

#[tokio::test]
async fn stale_source_with_same_remote_base_preserves_the_pre_save_version() {
    let (_dir, store) = setup(Some("base"), "pending resolved version").await;
    let before = store.load_page(&key()).await.unwrap().unwrap();
    let outcome = store
        .save_local_from_base_outcome(
            key(),
            "late editor version".into(),
            LocalRevision(2),
            EditorBaseline {
                body: "older presented source".into(),
                remote_base: before.remote_base,
            },
        )
        .await
        .unwrap();
    assert_eq!(outcome.stored.page.body, "pending resolved version");
    assert_eq!(
        store
            .load_editor_page(&key())
            .await
            .unwrap()
            .unwrap()
            .presented()
            .body,
        "late editor version"
    );
    assert_eq!(
        store
            .pending_reconciliation_keys(&key().connection)
            .await
            .unwrap(),
        vec![key()]
    );
    assert!(
        store
            .recovery_copies(&key().connection)
            .await
            .unwrap()
            .iter()
            .any(|copy| copy.page.body == "pending resolved version")
    );
    store.shutdown().await.unwrap();
}

#[tokio::test]
async fn repeated_durable_save_does_not_turn_its_original_baseline_into_a_conflict() {
    let (_dir, store) = setup(Some("base"), "first").await;
    let baseline = EditorBaseline {
        body: "first".into(),
        remote_base: Some(file("base", "a")),
    };
    let first = store
        .save_local_from_base_outcome(key(), "second".into(), LocalRevision(2), baseline.clone())
        .await
        .unwrap();
    let pending = store.pending_heads(&key().connection).await.unwrap()[0].id;
    let repeated = store
        .save_local_from_base_outcome(key(), "second".into(), LocalRevision(2), baseline)
        .await
        .unwrap();
    assert_eq!(repeated, first);
    assert!(store.conflicts(&key().connection).await.unwrap().is_empty());
    assert_eq!(
        store.pending_heads(&key().connection).await.unwrap()[0].id,
        pending
    );
    store.shutdown().await.unwrap();
}

#[tokio::test]
async fn legacy_conflict_on_unchanged_trash_clears_after_preserving_authored_draft() {
    let (_dir, store) = setup(Some("base"), "base").await;
    store.move_to_trash(key(), LocalRevision(1)).await.unwrap();
    legacy(&store, RemoteRead::Present(file("base", "a"))).await;
    store
        .save_merge_drafts(vec![(key(), "unfinished".into())])
        .await
        .unwrap();
    store
        .apply_remote(RemoteRead::Present(file("base", "a")))
        .await
        .unwrap();
    assert!(store.conflicts(&key().connection).await.unwrap().is_empty());
    assert!(store.merge_draft(&key()).await.unwrap().is_none());
    assert!(matches!(
        store.load_active_page(&key()).await,
        Err(StoreError::PageMissing)
    ));
    let op = store
        .claim_pending(&key().connection, 1)
        .await
        .unwrap()
        .remove(0);
    assert_eq!(op.kind, MutationKind::Delete);
    assert_eq!(
        store
            .recovery_uploads(&key().connection, 10)
            .await
            .unwrap()
            .len(),
        2
    );
    store.shutdown().await.unwrap();
}

#[tokio::test]
async fn repeated_overlap_does_not_duplicate_kept_history_or_recovery_jobs() {
    let (_dir, store) = setup(Some("base"), "local").await;
    store
        .apply_remote(RemoteRead::Present(file("remote", "b")))
        .await
        .unwrap();
    let before = store.load_stored_page(&key()).await.unwrap().unwrap();
    let history = store.recovery_copies(&key().connection).await.unwrap();
    let jobs = store.recovery_uploads(&key().connection, 10).await.unwrap();
    store
        .apply_remote(RemoteRead::Present(file("remote", "b")))
        .await
        .unwrap();
    assert_eq!(
        store.load_stored_page(&key()).await.unwrap().unwrap(),
        before
    );
    assert_eq!(
        store.recovery_copies(&key().connection).await.unwrap(),
        history
    );
    assert_eq!(
        store.recovery_uploads(&key().connection, 10).await.unwrap(),
        jobs
    );
    store.shutdown().await.unwrap();
}

#[tokio::test]
async fn automatic_source_update_invalidates_old_result_at_same_authored_revision() {
    let (_dir, store) = setup(Some("base"), "local").await;
    let attempt = prepared(&store, RemoteRead::Present(file("remote", "b"))).await;
    store
        .apply_remote(RemoteRead::Present(file("new remote", "c")))
        .await
        .unwrap();
    let current = store.load_stored_page(&key()).await.unwrap().unwrap();
    assert_eq!(current.page.local_revision, LocalRevision(1));
    assert!(matches!(
        store.commit_reconciliation(attempt).await,
        Err(StoreError::StaleRevision)
    ));
    assert_eq!(
        store.load_stored_page(&key()).await.unwrap().unwrap(),
        current
    );
    store.shutdown().await.unwrap();
}

#[tokio::test]
async fn failed_preservation_keeps_legacy_conflict_and_latest_draft() {
    let (dir, store) = setup(Some("base"), "local").await;
    let record = legacy(&store, RemoteRead::Present(file("remote", "b"))).await;
    store
        .save_merge_drafts(vec![(key(), "draft".into())])
        .await
        .unwrap();
    let before = store.load_stored_page(&key()).await.unwrap().unwrap();
    let draft = store.load_stored_merge_draft(&key()).await.unwrap();
    let db = rusqlite::Connection::open(dir.path().join("notes.sqlite")).unwrap();
    db.execute_batch("CREATE TRIGGER fail_kept BEFORE INSERT ON recovery BEGIN SELECT RAISE(ABORT, 'injected history failure'); END;").unwrap();
    assert!(
        store
            .apply_remote(RemoteRead::Present(file("remote", "b")))
            .await
            .is_err()
    );
    assert_eq!(
        store.load_stored_page(&key()).await.unwrap().unwrap(),
        before
    );
    assert_eq!(
        store.conflicts(&key().connection).await.unwrap(),
        vec![record]
    );
    assert_eq!(store.load_stored_merge_draft(&key()).await.unwrap(), draft);
    assert!(
        store
            .recovery_uploads(&key().connection, 10)
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        db.query_row("SELECT count(*) FROM automatic_events", [], |row| row
            .get::<_, i64>(0))
            .unwrap(),
        0
    );
    store.shutdown().await.unwrap();
}

#[tokio::test]
async fn explicit_empty_recreation_stays_create_only_when_remote_is_missing() {
    let (_dir, store) = setup(Some(""), "").await;
    let trash = store.move_to_trash(key(), LocalRevision(1)).await.unwrap();
    let deletion = store
        .claim_pending(&key().connection, 1)
        .await
        .unwrap()
        .remove(0);
    store
        .acknowledge(
            deletion.id,
            MutationAck {
                remote_revision: None,
            },
        )
        .await
        .unwrap();
    store.restore_trash(trash, None).await.unwrap();
    store
        .apply_remote(RemoteRead::Missing(key()))
        .await
        .unwrap();
    let op = store
        .claim_pending(&key().connection, 1)
        .await
        .unwrap()
        .remove(0);
    assert_eq!(op.kind, MutationKind::Put(String::new()));
    assert_eq!(op.condition, WriteCondition::CreateOnly);
    store.shutdown().await.unwrap();
}

#[tokio::test]
async fn missing_validator_blocks_merged_canonical_upload_without_losing_source() {
    let (_dir, store) = setup(Some("red fox jumps"), "blue fox jumps").await;
    let mut observed = file("red fox runs", "b");
    observed.revision = None;
    store
        .apply_remote(RemoteRead::Present(observed))
        .await
        .unwrap();
    assert_eq!(
        store.load_page(&key()).await.unwrap().unwrap().body,
        "blue fox runs"
    );
    assert!(
        store
            .claim_pending(&key().connection, 1)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        store
            .load_sync_page(&key())
            .await
            .unwrap()
            .unwrap()
            .1
            .pending
    );
    store.shutdown().await.unwrap();
}

#[tokio::test]
async fn budget_fallback_preserves_exact_full_original_as_immutable_upload() {
    let source = "x".repeat(1_048_577);
    let (_dir, store) = setup(Some("base"), &source).await;
    store
        .apply_remote(RemoteRead::Present(file("remote", "b")))
        .await
        .unwrap();
    assert_eq!(
        store.load_page(&key()).await.unwrap().unwrap().body,
        "remote"
    );
    let jobs = store.recovery_uploads(&key().connection, 10).await.unwrap();
    assert_eq!(jobs.len(), 1);
    let event = store
        .automatic_recovery_event(jobs[0].event_id)
        .await
        .unwrap();
    assert_eq!(
        event.input.reason,
        bulletnotes::domain::reconciliation::AutomaticRecoveryReason::BudgetFallback
    );
    let upload = store
        .read_recovery_upload(jobs[0].event_id, 0, jobs[0].operation_id)
        .await
        .unwrap();
    assert_eq!(upload.operation.kind, MutationKind::Put(source));
    store.shutdown().await.unwrap();
}

#[tokio::test]
async fn missing_uncached_catalogue_entry_is_hidden_without_creating_source() {
    let dir = tempfile::tempdir().unwrap();
    let store = StoreHandle::open(&dir.path().join("notes.sqlite"))
        .await
        .unwrap();
    store
        .record_catalogue(
            &key().connection,
            vec![RemoteMetadata {
                key: key(),
                modified_ms: None,
                content_length: None,
                permission: Permission::ReadWrite,
                revision: None,
            }],
            true,
        )
        .await
        .unwrap();
    store
        .apply_remote(RemoteRead::Missing(key()))
        .await
        .unwrap();
    let result = store
        .search(bulletnotes::store::search::SearchQuery {
            scope: bulletnotes::store::search::SearchScope::AllPages,
            connection: key().connection,
            text: String::new(),
            folder: None,
            offset: 0,
            limit: 10,
        })
        .await
        .unwrap();
    assert_eq!(result.total, 0);
    assert!(store.load_page(&key()).await.unwrap().is_none());
    store.shutdown().await.unwrap();
}
