use bulletnotes::{
    domain::{connection::ConnectionConfig, error::StoreError, page::*},
    store::{StoreHandle, search::SearchQuery},
};
fn key(path: &str) -> PageKey {
    PageKey {
        connection: ConnectionConfig::parse("https://recovery.example/")
            .unwrap()
            .id()
            .clone(),
        path: PagePath::parse(path).unwrap(),
    }
}
fn file(body: &str, revision: &str) -> RemoteFile {
    RemoteFile {
        key: key("one.md"),
        body: body.into(),
        revision: Some(RemoteRevision(format!("\"{revision}\""))),
        permission: Permission::ReadWrite,
        modified_ms: None,
    }
}
async fn conflict(store: &StoreHandle, remote: RemoteRead) -> ConflictRecord {
    store
        .apply_remote(RemoteRead::Present(file("base", "a")))
        .await
        .unwrap();
    store
        .save_local(key("one.md"), "local\r\n中文🙂".into(), LocalRevision(1))
        .await
        .unwrap();
    let local = store.load_page(&key("one.md")).await.unwrap().unwrap();
    let record = ConflictRecord {
        id: uuid::Uuid::new_v4(),
        key: key("one.md"),
        base: local.remote_base.clone(),
        local,
        remote,
    };
    store.save_conflict(record.clone()).await.unwrap();
    record
}
fn resolution(record: &ConflictRecord, choice: ResolutionChoice) -> ConflictResolution {
    ConflictResolution {
        conflict_id: record.id,
        expected_local: record.local.local_revision,
        choice,
    }
}
#[tokio::test]
async fn resolution_races_back_to_review_and_keeps_all_three_versions() {
    let dir = tempfile::tempdir().unwrap();
    let store = StoreHandle::open(&dir.path().join("notes.sqlite"))
        .await
        .unwrap();
    let first = conflict(&store, RemoteRead::Present(file("remote", "b"))).await;
    store
        .save_conflict(ConflictRecord {
            remote: RemoteRead::Present(file("newer remote", "c")),
            ..first.clone()
        })
        .await
        .unwrap();
    assert!(matches!(
        store
            .resolve_conflict(resolution(
                &first,
                ResolutionChoice::Merge("reviewed merge".into())
            ))
            .await,
        Err(StoreError::StaleRevision)
    ));
    let latest = store
        .conflicts(&key("one.md").connection)
        .await
        .unwrap()
        .pop()
        .unwrap();
    assert_ne!(latest.id, first.id);
    store
        .resolve_conflict(resolution(
            &latest,
            ResolutionChoice::Merge("merged\r\n中文🙂".into()),
        ))
        .await
        .unwrap();
    let operation = store
        .claim_pending(&key("one.md").connection, 1)
        .await
        .unwrap()
        .pop()
        .unwrap();
    assert_eq!(
        operation.condition,
        WriteCondition::Match(RemoteRevision("\"c\"".into()))
    );
    assert_eq!(operation.kind, MutationKind::Put("merged\r\n中文🙂".into()));
    let copies = store
        .recovery_copies(&key("one.md").connection)
        .await
        .unwrap();
    for expected in ["base", "local\r\n中文🙂", "remote", "newer remote"] {
        assert!(
            copies.iter().any(|copy| copy.page.body == expected),
            "missing {expected}"
        );
    }
    store.shutdown().await.unwrap();
}
#[tokio::test]
async fn remote_delete_is_not_empty_text_and_recreation_is_explicit_create_only() {
    for missing in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let store = StoreHandle::open(&dir.path().join("notes.sqlite"))
            .await
            .unwrap();
        let remote = if missing {
            RemoteRead::Missing(key("one.md"))
        } else {
            RemoteRead::Present(file("", "b"))
        };
        let record = conflict(&store, remote).await;
        store
            .resolve_conflict(resolution(&record, ResolutionChoice::AcceptRemote))
            .await
            .unwrap();
        let current = store.load_page(&key("one.md")).await.unwrap();
        if missing {
            assert!(current.is_none());
        } else {
            assert_eq!(current.unwrap().body, "");
        }
        assert!(
            store
                .claim_pending(&key("one.md").connection, 1)
                .await
                .unwrap()
                .is_empty()
        );
        store.shutdown().await.unwrap();
    }
    let dir = tempfile::tempdir().unwrap();
    let store = StoreHandle::open(&dir.path().join("notes.sqlite"))
        .await
        .unwrap();
    let record = conflict(&store, RemoteRead::Missing(key("one.md"))).await;
    store
        .resolve_conflict(resolution(&record, ResolutionChoice::Merge(String::new())))
        .await
        .unwrap();
    let operation = store
        .claim_pending(&key("one.md").connection, 1)
        .await
        .unwrap()
        .pop()
        .unwrap();
    assert_eq!(operation.condition, WriteCondition::CreateOnly);
    assert_eq!(operation.kind, MutationKind::Put(String::new()));
    store.shutdown().await.unwrap();
}
#[tokio::test]
async fn trash_survives_restart_with_source_base_and_delete_intent() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("notes.sqlite");
    let store = StoreHandle::open(&path).await.unwrap();
    store
        .apply_remote(RemoteRead::Present(file("original\r\n中文🙂", "a")))
        .await
        .unwrap();
    let id = store
        .move_to_trash(key("one.md"), LocalRevision(0))
        .await
        .unwrap();
    assert!(
        store
            .search(SearchQuery {
                scope: bulletnotes::store::search::SearchScope::AllPages,
                connection: key("one.md").connection,
                text: String::new(),
                folder: None,
                offset: 0,
                limit: 100
            })
            .await
            .unwrap()
            .items
            .is_empty()
    );
    let pending = store
        .claim_pending(&key("one.md").connection, 1)
        .await
        .unwrap()
        .pop()
        .unwrap();
    assert_eq!(pending.kind, MutationKind::Delete);
    assert_eq!(
        pending.condition,
        WriteCondition::Match(RemoteRevision("\"a\"".into()))
    );
    store.shutdown().await.unwrap();
    let store = StoreHandle::open(&path).await.unwrap();
    let trash = store.trash(&key("one.md").connection).await.unwrap();
    assert_eq!(trash.len(), 1);
    assert_eq!(trash[0].id, id);
    assert_eq!(trash[0].page.body, "original\r\n中文🙂");
    assert_eq!(
        trash[0].page.remote_base.as_ref().unwrap().revision,
        Some(RemoteRevision("\"a\"".into()))
    );
    assert_eq!(
        store
            .unknown_mutations(&key("one.md").connection)
            .await
            .unwrap()[0],
        pending
    );
    assert!(
        store.restore_trash(id, None).await.is_err(),
        "in-flight deletion must be resolved before reusing its path"
    );
    let restored = store
        .restore_trash(id, Some(PagePath::parse("recovered.md").unwrap()))
        .await
        .unwrap();
    let operation = store
        .claim_pending(&restored.connection, 1)
        .await
        .unwrap()
        .pop()
        .unwrap();
    assert_eq!(operation.key, restored);
    assert_eq!(operation.condition, WriteCondition::CreateOnly);
    assert_eq!(
        store.trash(&restored.connection).await.unwrap().len(),
        1,
        "restoration retains the original recovery copy until explicit purge"
    );
    store.empty_trash(vec![id]).await.unwrap();
    assert!(store.trash(&restored.connection).await.unwrap().is_empty());
    store.shutdown().await.unwrap();
}
#[tokio::test]
async fn trash_restore_never_replaces_an_existing_local_page() {
    let dir = tempfile::tempdir().unwrap();
    let store = StoreHandle::open(&dir.path().join("notes.sqlite"))
        .await
        .unwrap();
    store
        .apply_remote(RemoteRead::Present(file("original", "a")))
        .await
        .unwrap();
    let id = store
        .move_to_trash(key("one.md"), LocalRevision(0))
        .await
        .unwrap();
    let operation = store
        .claim_pending(&key("one.md").connection, 1)
        .await
        .unwrap()
        .pop()
        .unwrap();
    store
        .acknowledge(
            operation.id,
            MutationAck {
                remote_revision: None,
            },
        )
        .await
        .unwrap();
    store
        .apply_remote(RemoteRead::Present(file("replacement", "b")))
        .await
        .unwrap();
    assert!(store.restore_trash(id, None).await.is_err());
    assert_eq!(
        store.load_page(&key("one.md")).await.unwrap().unwrap().body,
        "replacement"
    );
    let restored = store
        .restore_trash(id, Some(PagePath::parse("explicit-recovery.md").unwrap()))
        .await
        .unwrap();
    assert_eq!(
        store.load_page(&restored).await.unwrap().unwrap().body,
        "original"
    );
    assert_eq!(
        store.claim_pending(&restored.connection, 1).await.unwrap()[0].condition,
        WriteCondition::CreateOnly
    );
    store.shutdown().await.unwrap();
}
#[tokio::test]
async fn empty_existing_remote_page_is_retained_without_delete_intent() {
    let dir = tempfile::tempdir().unwrap();
    let store = StoreHandle::open(&dir.path().join("notes.sqlite"))
        .await
        .unwrap();
    store
        .apply_remote(RemoteRead::Present(file("", "a")))
        .await
        .unwrap();
    store
        .save_local(key("one.md"), String::new(), LocalRevision(1))
        .await
        .unwrap();
    assert!(store.load_page(&key("one.md")).await.unwrap().is_some());
    assert!(
        store
            .claim_pending(&key("one.md").connection, 1)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        store
            .trash(&key("one.md").connection)
            .await
            .unwrap()
            .is_empty()
    );
    store.shutdown().await.unwrap();
}
#[tokio::test]
async fn deletion_waiting_on_a_put_uses_its_acknowledged_revision() {
    let dir = tempfile::tempdir().unwrap();
    let store = StoreHandle::open(&dir.path().join("notes.sqlite"))
        .await
        .unwrap();
    store.create_draft(key("one.md")).await.unwrap();
    store
        .save_local(key("one.md"), "uploading".into(), LocalRevision(1))
        .await
        .unwrap();
    let put = store
        .claim_pending(&key("one.md").connection, 1)
        .await
        .unwrap()
        .pop()
        .unwrap();
    store
        .move_to_trash(key("one.md"), LocalRevision(1))
        .await
        .unwrap();
    store
        .acknowledge(
            put.id,
            MutationAck {
                remote_revision: Some(RemoteRevision("\"uploaded\"".into())),
            },
        )
        .await
        .unwrap();
    let delete = store
        .claim_pending(&key("one.md").connection, 1)
        .await
        .unwrap()
        .pop()
        .unwrap();
    assert_eq!(delete.kind, MutationKind::Delete);
    assert_eq!(
        delete.condition,
        WriteCondition::Match(RemoteRevision("\"uploaded\"".into()))
    );
    store.shutdown().await.unwrap();
}

#[cfg(feature = "protocol-validation")]
#[path = "support/space_fixture.rs"]
mod space_fixture;
#[cfg(feature = "protocol-validation")]
mod network {
    use super::*;
    use bulletnotes::sync::{
        SilverBulletClient,
        probe::{new_probe_key, verify_write_guards},
        worker::SyncHandle,
    };
    use std::time::Duration;
    async fn until(mut condition: impl FnMut() -> bool) {
        tokio::time::timeout(Duration::from_secs(5), async {
            while !condition() {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
    }
    fn client(f: &space_fixture::SpaceFixture) -> SilverBulletClient {
        SilverBulletClient::for_loopback_fixture(
            &ConnectionConfig::parse(key("one.md").connection.as_str()).unwrap(),
            f.base.clone(),
            None,
            Duration::from_millis(100),
        )
        .unwrap()
    }
    #[tokio::test]
    async fn recreated_path_survives_delete_retry() {
        let dir = tempfile::tempdir().unwrap();
        let store = StoreHandle::open(&dir.path().join("notes.sqlite"))
            .await
            .unwrap();
        let f = space_fixture::SpaceFixture::start().await;
        let c = client(&f);
        let evidence = verify_write_guards(&c, new_probe_key(key("one.md").connection))
            .await
            .unwrap();
        f.put("one.md", "original");
        store
            .apply_remote(c.read(&key("one.md")).await.unwrap())
            .await
            .unwrap();
        store
            .move_to_trash(key("one.md"), LocalRevision(0))
            .await
            .unwrap();
        f.data.lock().unwrap().lost_delete = Some("one.md".into());
        let sync = SyncHandle::start(store.clone(), c, Some(evidence)).unwrap();
        until(|| f.data.lock().unwrap().deleted).await;
        f.put("one.md", "replacement");
        f.data.lock().unwrap().release.notify_one();
        until(|| sync.snapshot().complete && sync.snapshot().pending == 0).await;
        assert_eq!(
            store
                .load_active_page(&key("one.md"))
                .await
                .unwrap()
                .unwrap()
                .page
                .body,
            "replacement"
        );
        assert_eq!(sync.snapshot().conflicts, 0);
        assert_eq!(f.body("one.md").as_deref(), Some("replacement"));
        assert_eq!(
            f.data
                .lock()
                .unwrap()
                .deletes
                .iter()
                .filter(|(path, _)| path == "one.md")
                .count(),
            1
        );
        assert_eq!(
            store.trash(&key("one.md").connection).await.unwrap()[0]
                .page
                .body,
            "original"
        );
        sync.shutdown().await.unwrap();
        store.shutdown().await.unwrap();
    }
    async fn identical_recreation(restart: bool) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("notes.sqlite");
        let mut store = StoreHandle::open(&path).await.unwrap();
        let f = space_fixture::SpaceFixture::start().await;
        f.data.lock().unwrap().content_addressed = true;
        let c = client(&f);
        let evidence = verify_write_guards(&c, new_probe_key(key("one.md").connection))
            .await
            .unwrap();
        f.put("one.md", "identical generated content");
        let original = c.read(&key("one.md")).await.unwrap();
        store.apply_remote(original.clone()).await.unwrap();
        store
            .move_to_trash(key("one.md"), LocalRevision(0))
            .await
            .unwrap();
        let sync = if restart {
            let sent = store
                .claim_pending(&key("one.md").connection, 1)
                .await
                .unwrap()
                .pop()
                .unwrap();
            store.mark_unknown(sent.id).await.unwrap();
            f.data.lock().unwrap().files.remove("one.md");
            f.put("one.md", "identical generated content");
            store.shutdown().await.unwrap();
            store = StoreHandle::open(&path).await.unwrap();
            SyncHandle::start(store.clone(), c.clone(), Some(evidence)).unwrap()
        } else {
            f.data.lock().unwrap().lost_delete = Some("one.md".into());
            let sync = SyncHandle::start(store.clone(), c.clone(), Some(evidence)).unwrap();
            until(|| f.data.lock().unwrap().deleted).await;
            f.put("one.md", "identical generated content");
            f.data.lock().unwrap().release.notify_one();
            sync
        };
        assert_eq!(c.read(&key("one.md")).await.unwrap(), original);
        until(|| sync.snapshot().complete && sync.snapshot().pending == 0).await;
        assert_eq!(
            f.body("one.md").as_deref(),
            Some("identical generated content"),
            "unknown delete removed a same-content recreation"
        );
        assert_eq!(sync.snapshot().conflicts, 0);
        assert_eq!(
            store
                .load_active_page(&key("one.md"))
                .await
                .unwrap()
                .unwrap()
                .page
                .body,
            "identical generated content"
        );
        assert_eq!(
            store.trash(&key("one.md").connection).await.unwrap()[0]
                .page
                .body,
            "identical generated content"
        );
        let deletes = f
            .data
            .lock()
            .unwrap()
            .deletes
            .iter()
            .filter(|(path, _)| path == "one.md")
            .count();
        assert_eq!(deletes, if restart { 0 } else { 1 });
        sync.shutdown().await.unwrap();
        store.shutdown().await.unwrap();
    }
    #[tokio::test]
    async fn identical_content_recreation_survives_unknown_delete() {
        identical_recreation(false).await;
    }
    #[tokio::test]
    async fn identical_content_recreation_survives_unknown_delete_after_restart() {
        identical_recreation(true).await;
    }
    #[tokio::test]
    async fn automatic_recovery_uploads_while_unrelated_pages_keep_syncing() {
        let dir = tempfile::tempdir().unwrap();
        let store = StoreHandle::open(&dir.path().join("notes.sqlite"))
            .await
            .unwrap();
        let f = space_fixture::SpaceFixture::start().await;
        let c = client(&f);
        let evidence = verify_write_guards(&c, new_probe_key(key("one.md").connection))
            .await
            .unwrap();
        f.put("one.md", "base");
        store
            .apply_remote(c.read(&key("one.md")).await.unwrap())
            .await
            .unwrap();
        store
            .save_local(key("one.md"), "local".into(), LocalRevision(1))
            .await
            .unwrap();
        f.put("one.md", "remote");
        store
            .apply_remote(c.read(&key("one.md")).await.unwrap())
            .await
            .unwrap();
        let jobs = store
            .recovery_uploads(&key("one.md").connection, 10)
            .await
            .unwrap();
        assert_eq!(jobs.len(), 1);
        let sync = SyncHandle::start(store.clone(), c, Some(evidence)).unwrap();
        store.create_draft(key("two.md")).await.unwrap();
        store
            .save_local(key("two.md"), "unrelated".into(), LocalRevision(1))
            .await
            .unwrap();
        until(|| f.body("two.md").as_deref() == Some("unrelated")).await;
        assert_eq!(
            store.load_page(&key("one.md")).await.unwrap().unwrap().body,
            "remote"
        );
        assert_eq!(f.body("one.md").as_deref(), Some("remote"));
        assert!(
            store
                .conflicts(&key("one.md").connection)
                .await
                .unwrap()
                .is_empty()
        );
        let event = store
            .automatic_recovery_event(jobs[0].event_id)
            .await
            .unwrap();
        assert_eq!(event.input.local.body, "local");
        assert_eq!(event.input.kept_body, "remote");
        let pending = store
            .read_recovery_upload(jobs[0].event_id, 0, jobs[0].operation_id)
            .await
            .unwrap();
        assert_eq!(pending.operation.kind, MutationKind::Put("local".into()));
        until(|| sync.snapshot().pending == 0 && sync.snapshot().conflicts == 0).await;
        assert_eq!(f.body(jobs[0].key.path.as_str()).as_deref(), Some("local"));
        sync.shutdown().await.unwrap();
        store.shutdown().await.unwrap();
    }
    #[tokio::test]
    async fn another_server_change_after_resolution_preserves_the_chosen_local_source() {
        let dir = tempfile::tempdir().unwrap();
        let store = StoreHandle::open(&dir.path().join("notes.sqlite"))
            .await
            .unwrap();
        let f = space_fixture::SpaceFixture::start().await;
        let c = client(&f);
        f.put("one.md", "base");
        store
            .apply_remote(c.read(&key("one.md")).await.unwrap())
            .await
            .unwrap();
        store
            .save_local(key("one.md"), "local".into(), LocalRevision(1))
            .await
            .unwrap();
        f.put("one.md", "reviewed remote");
        let local = store.load_page(&key("one.md")).await.unwrap().unwrap();
        let record = ConflictRecord {
            id: uuid::Uuid::new_v4(),
            key: key("one.md"),
            base: local.remote_base.clone(),
            local,
            remote: c.read(&key("one.md")).await.unwrap(),
        };
        store.save_conflict(record.clone()).await.unwrap();
        store
            .resolve_conflict(resolution(
                &record,
                ResolutionChoice::Merge("merged".into()),
            ))
            .await
            .unwrap();
        f.put("one.md", "newer remote");
        store
            .apply_remote(c.read(&key("one.md")).await.unwrap())
            .await
            .unwrap();
        assert_eq!(f.body("one.md").as_deref(), Some("newer remote"));
        assert_eq!(
            store.load_page(&key("one.md")).await.unwrap().unwrap().body,
            "newer remote"
        );
        assert!(
            store
                .conflicts(&key("one.md").connection)
                .await
                .unwrap()
                .is_empty()
        );
        let jobs = store
            .recovery_uploads(&key("one.md").connection, 10)
            .await
            .unwrap();
        assert_eq!(jobs.len(), 1);
        let event = store
            .automatic_recovery_event(jobs[0].event_id)
            .await
            .unwrap();
        assert_eq!(event.input.local.body, "merged");
        assert_eq!(event.input.kept_body, "newer remote");
        let pending = store
            .read_recovery_upload(jobs[0].event_id, 0, jobs[0].operation_id)
            .await
            .unwrap();
        assert_eq!(pending.operation.kind, MutationKind::Put("merged".into()));
        store.shutdown().await.unwrap();
    }
}

#[tokio::test]
async fn a_late_unsynced_create_meets_a_retained_trash_tombstone() {
    let dir = tempfile::tempdir().unwrap();
    let store = StoreHandle::open(&dir.path().join("notes.sqlite"))
        .await
        .unwrap();
    store.create_draft(key("one.md")).await.unwrap();
    store
        .save_local(key("one.md"), "uncertain source".into(), LocalRevision(1))
        .await
        .unwrap();
    let sent = store
        .claim_pending(&key("one.md").connection, 1)
        .await
        .unwrap()
        .pop()
        .unwrap();
    store
        .move_to_trash(key("one.md"), LocalRevision(1))
        .await
        .unwrap();
    store.mark_unknown(sent.id).await.unwrap();
    store.retry_after_readback(sent.id).await.unwrap();
    assert!(
        store.load_page(&key("one.md")).await.unwrap().is_some(),
        "retain the tombstone for a create that might arrive late"
    );
    assert!(matches!(
        store
            .apply_remote(RemoteRead::Present(file("uncertain source", "late")))
            .await
            .unwrap(),
        ReconcileDecision::InstallRemote(_)
    ));
    assert_eq!(
        store
            .load_active_page(&key("one.md"))
            .await
            .unwrap()
            .unwrap()
            .page
            .body,
        "uncertain source"
    );
    assert!(
        store
            .claim_pending(&key("one.md").connection, 1)
            .await
            .unwrap()
            .is_empty()
    );
    store.shutdown().await.unwrap();
}
#[tokio::test]
async fn clean_remote_deletion_can_be_restored_to_its_original_path_create_only() {
    let dir = tempfile::tempdir().unwrap();
    let store = StoreHandle::open(&dir.path().join("notes.sqlite"))
        .await
        .unwrap();
    store
        .apply_remote(RemoteRead::Present(file("retained source", "a")))
        .await
        .unwrap();
    store
        .apply_remote(RemoteRead::Missing(key("one.md")))
        .await
        .unwrap();
    let copy = store
        .recovery_copies(&key("one.md").connection)
        .await
        .unwrap()
        .pop()
        .unwrap();
    let restored = store.restore_recovery(copy.id, None).await.unwrap();
    assert_eq!(restored, key("one.md"));
    assert_eq!(
        store.claim_pending(&restored.connection, 1).await.unwrap()[0].condition,
        WriteCondition::CreateOnly
    );
    store.shutdown().await.unwrap();
}

#[tokio::test]
async fn unapplied_merged_drafts_survive_restart_and_resolution_keeps_their_source() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("notes.sqlite");
    let store = StoreHandle::open(&path).await.unwrap();
    let record = conflict(&store, RemoteRead::Present(file("remote", "b"))).await;
    store
        .save_merge_drafts(vec![(key("one.md"), "unapplied merge\r\n🙂".into())])
        .await
        .unwrap();
    store.shutdown().await.unwrap();
    let store = StoreHandle::open(&path).await.unwrap();
    assert_eq!(
        store.merge_draft(&key("one.md")).await.unwrap().as_deref(),
        Some("unapplied merge\r\n🙂")
    );
    store
        .resolve_conflict(resolution(&record, ResolutionChoice::AcceptRemote))
        .await
        .unwrap();
    assert!(store.merge_draft(&key("one.md")).await.unwrap().is_none());
    assert!(
        store
            .recovery_copies(&key("one.md").connection)
            .await
            .unwrap()
            .iter()
            .any(|copy| copy.page.body == "unapplied merge\r\n🙂")
    );
    store.shutdown().await.unwrap();
}

#[tokio::test]
async fn identical_content_at_a_reused_restore_path_adopts_the_current_validator() {
    let dir = tempfile::tempdir().unwrap();
    let store = StoreHandle::open(&dir.path().join("notes.sqlite"))
        .await
        .unwrap();
    store
        .apply_remote(RemoteRead::Present(file("original", "a")))
        .await
        .unwrap();
    let id = store
        .move_to_trash(key("one.md"), LocalRevision(0))
        .await
        .unwrap();
    let sent = store
        .claim_pending(&key("one.md").connection, 1)
        .await
        .unwrap()
        .pop()
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
    store.restore_trash(id, None).await.unwrap();
    assert!(matches!(
        store
            .apply_remote(RemoteRead::Present(file("original", "replacement")))
            .await
            .unwrap(),
        ReconcileDecision::InstallRemote(_)
    ));
    assert_eq!(
        store
            .load_page(&key("one.md"))
            .await
            .unwrap()
            .unwrap()
            .remote_base
            .unwrap()
            .revision,
        Some(RemoteRevision("\"replacement\"".into()))
    );
    assert!(
        store
            .claim_pending(&key("one.md").connection, 1)
            .await
            .unwrap()
            .is_empty()
    );
    store.shutdown().await.unwrap();
}

#[tokio::test]
async fn an_already_editing_buffer_remains_durable_after_remote_deletion() {
    let dir = tempfile::tempdir().unwrap();
    let store = StoreHandle::open(&dir.path().join("notes.sqlite"))
        .await
        .unwrap();
    let base = file("base", "a");
    store
        .apply_remote(RemoteRead::Present(base.clone()))
        .await
        .unwrap();
    store
        .apply_remote(RemoteRead::Missing(key("one.md")))
        .await
        .unwrap();
    store
        .save_local_from_base(
            key("one.md"),
            "typed before the deletion arrived".into(),
            LocalRevision(1),
            EditorBaseline {
                body: "base".into(),
                remote_base: Some(base),
            },
        )
        .await
        .unwrap();
    let record = store
        .conflicts(&key("one.md").connection)
        .await
        .unwrap()
        .pop()
        .unwrap();
    assert_eq!(record.local.body, "typed before the deletion arrived");
    assert!(matches!(record.remote, RemoteRead::Missing(_)));
    store.shutdown().await.unwrap();
}
