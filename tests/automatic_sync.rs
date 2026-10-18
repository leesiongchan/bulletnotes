#![cfg(feature = "protocol-validation")]
#[path = "support/space_fixture.rs"]
mod space_fixture;
use bulletnotes::{
    domain::{connection::ConnectionConfig, error::SyncError, page::*, reconciliation::*},
    store::StoreHandle,
    sync::{
        SilverBulletClient,
        probe::{new_probe_key, verify_write_guards},
        worker::SyncHandle,
    },
};
use space_fixture::SpaceFixture;
use std::time::Duration;
fn config() -> ConnectionConfig {
    ConnectionConfig::parse("https://generated.example/").unwrap()
}
fn key(path: &str) -> PageKey {
    PageKey {
        connection: config().id().clone(),
        path: PagePath::parse(path).unwrap(),
    }
}
fn client(f: &SpaceFixture) -> SilverBulletClient {
    SilverBulletClient::for_loopback_fixture(
        &config(),
        f.base.clone(),
        None,
        Duration::from_millis(120),
    )
    .unwrap()
}
async fn until(mut predicate: impl FnMut() -> bool) {
    tokio::time::timeout(Duration::from_secs(8), async {
        while !predicate() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("sync must make progress");
}
async fn start(store: &StoreHandle, f: &SpaceFixture) -> SyncHandle {
    let c = client(f);
    let evidence = verify_write_guards(&c, new_probe_key(config().id().clone()))
        .await
        .unwrap();
    SyncHandle::start(store.clone(), c, Some(evidence)).unwrap()
}
async fn recovery(store: &StoreHandle, f: &SpaceFixture) -> RecoveryUploadHead {
    f.put("one.md", "base");
    let c = client(f);
    store
        .apply_remote(c.read(&key("one.md")).await.unwrap())
        .await
        .unwrap();
    store
        .save_local(key("one.md"), "本地\r\n💾".into(), LocalRevision(1))
        .await
        .unwrap();
    f.put("one.md", "remote");
    store
        .apply_remote(c.read(&key("one.md")).await.unwrap())
        .await
        .unwrap();
    store
        .recovery_uploads(config().id(), 100)
        .await
        .unwrap()
        .remove(0)
}
#[tokio::test]
async fn lost_recovery_ack_uses_exact_readback_and_queues_later_edits() {
    let dir = tempfile::tempdir().unwrap();
    let store = StoreHandle::open(&dir.path().join("test.sqlite"))
        .await
        .unwrap();
    let f = SpaceFixture::start().await;
    let head = recovery(&store, &f).await;
    f.data.lock().unwrap().lost_put = Some(head.key.path.as_str().into());
    store
        .save_local(head.key.clone(), "later edit".into(), LocalRevision(2))
        .await
        .unwrap();
    let sync = start(&store, &f).await;
    until(|| f.body(head.key.path.as_str()).as_deref() == Some("later edit")).await;
    until(|| sync.snapshot().pending == 0).await;
    let puts = f.data.lock().unwrap().puts.clone();
    let puts: Vec<_> = puts
        .iter()
        .filter(|p| p.0 == head.key.path.as_str())
        .collect();
    assert_eq!(puts.len(), 2);
    assert_eq!(puts[0].1, "本地\r\n💾");
    assert!(puts[0].2);
    assert!(!puts[1].2);
    assert_eq!(
        store.load_page(&key("one.md")).await.unwrap().unwrap().body,
        "remote"
    );
    sync.shutdown().await.unwrap();
    store.shutdown().await.unwrap();
}
#[tokio::test]
async fn occupied_recovery_destinations_are_acknowledged_or_relocated() {
    for identical in [true, false] {
        let dir = tempfile::tempdir().unwrap();
        let store = StoreHandle::open(&dir.path().join("test.sqlite"))
            .await
            .unwrap();
        let f = SpaceFixture::start().await;
        let head = recovery(&store, &f).await;
        f.put(
            head.key.path.as_str(),
            if identical {
                "本地\r\n💾"
            } else {
                "occupant"
            },
        );
        let sync = start(&store, &f).await;
        until(|| sync.snapshot().complete && sync.snapshot().pending == 0).await;
        assert_eq!(
            f.body(head.key.path.as_str()).as_deref(),
            Some(if identical {
                "本地\r\n💾"
            } else {
                "occupant"
            })
        );
        assert!(
            f.data
                .lock()
                .unwrap()
                .files
                .values()
                .any(|f| f.body == "本地\r\n💾")
        );
        if !identical {
            store
                .save_local(head.key.clone(), "edited occupant".into(), LocalRevision(2))
                .await
                .unwrap();
            until(|| f.body(head.key.path.as_str()).as_deref() == Some("edited occupant")).await;
            assert!(
                f.data
                    .lock()
                    .unwrap()
                    .puts
                    .iter()
                    .any(|p| p.0 == head.key.path.as_str() && !p.2)
            );
        }
        sync.shutdown().await.unwrap();
        store.shutdown().await.unwrap();
    }
}
#[tokio::test]
async fn denied_recovery_is_visible_on_original_and_does_not_block_peer() {
    let dir = tempfile::tempdir().unwrap();
    let store = StoreHandle::open(&dir.path().join("test.sqlite"))
        .await
        .unwrap();
    let f = SpaceFixture::start().await;
    let head = recovery(&store, &f).await;
    f.data.lock().unwrap().denied_put = Some(head.key.path.as_str().into());
    store.create_draft(key("peer.md")).await.unwrap();
    store
        .save_local(key("peer.md"), "peer".into(), LocalRevision(1))
        .await
        .unwrap();
    let sync = start(&store, &f).await;
    until(|| sync.snapshot().page_errors.get(&key("one.md")) == Some(&SyncError::Permission)).await;
    until(|| f.body("peer.md").as_deref() == Some("peer")).await;
    assert!(sync.snapshot().pending > 0);
    assert!(
        store
            .load_sync_page(&key("one.md"))
            .await
            .unwrap()
            .unwrap()
            .1
            .pending
    );
    sync.shutdown().await.unwrap();
    store.shutdown().await.unwrap();
}
#[tokio::test]
async fn delayed_delete_executes_after_retirement_and_restart_then_repairs() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.sqlite");
    let store = StoreHandle::open(&path).await.unwrap();
    let f = SpaceFixture::start().await;
    f.put("one.md", "unchanged\r\n💾");
    store
        .apply_remote(client(&f).read(&key("one.md")).await.unwrap())
        .await
        .unwrap();
    store
        .move_to_trash(key("one.md"), LocalRevision(0))
        .await
        .unwrap();
    f.data.lock().unwrap().delay_delete = Some("one.md".into());
    let sync = start(&store, &f).await;
    until(|| f.data.lock().unwrap().delete_received).await;
    until(|| sync.snapshot().pending == 0).await;
    assert_eq!(
        store
            .load_active_page(&key("one.md"))
            .await
            .unwrap()
            .unwrap()
            .body,
        "unchanged\r\n💾"
    );
    sync.shutdown().await.unwrap();
    store.shutdown().await.unwrap();
    let store = StoreHandle::open(&path).await.unwrap();
    f.data.lock().unwrap().release.notify_one();
    until(|| f.body("one.md").is_none()).await;
    let sync = start(&store, &f).await;
    until(|| f.body("one.md").as_deref() == Some("unchanged\r\n💾")).await;
    until(|| sync.snapshot().pending == 0).await;
    assert_eq!(
        f.data
            .lock()
            .unwrap()
            .deletes
            .iter()
            .filter(|d| d.0 == "one.md")
            .count(),
        1
    );
    assert!(
        f.data
            .lock()
            .unwrap()
            .puts
            .iter()
            .any(|p| p.0 == "one.md" && p.2)
    );
    store
        .move_to_trash(key("one.md"), LocalRevision(0))
        .await
        .unwrap();
    until(|| f.body("one.md").is_none()).await;
    sync.refresh();
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(f.body("one.md").is_none());
    sync.shutdown().await.unwrap();
    store.shutdown().await.unwrap();
}

#[tokio::test]
async fn delayed_recovery_create_and_unknown_restart_keep_exact_attempt() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.sqlite");
    let store = StoreHandle::open(&path).await.unwrap();
    let f = SpaceFixture::start().await;
    let head = recovery(&store, &f).await;
    f.data.lock().unwrap().delay_put = Some(head.key.path.as_str().into());
    let sync = start(&store, &f).await;
    until(|| f.data.lock().unwrap().delay_put.is_none()).await;
    sync.shutdown().await.unwrap();
    store.shutdown().await.unwrap();
    let store = StoreHandle::open(&path).await.unwrap();
    let heads = store.recovery_uploads(config().id(), 100).await.unwrap();
    assert_eq!(heads[0].operation_id, head.operation_id);
    assert_eq!(heads[0].state, RecoveryUploadState::Unknown);
    f.data.lock().unwrap().release.notify_one();
    until(|| f.body(head.key.path.as_str()).as_deref() == Some("本地\r\n💾")).await;
    let sync = start(&store, &f).await;
    until(|| sync.snapshot().complete && sync.snapshot().pending == 0).await;
    assert_eq!(
        f.data
            .lock()
            .unwrap()
            .puts
            .iter()
            .filter(|p| p.0 == head.key.path.as_str())
            .count(),
        1
    );
    sync.shutdown().await.unwrap();
    store.shutdown().await.unwrap();
}

#[tokio::test]
async fn current_recovery_ack_settles_obligation_while_old_attempt_stays_unknown() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.sqlite");
    let store = StoreHandle::open(&path).await.unwrap();
    let f = SpaceFixture::start().await;
    let head = recovery(&store, &f).await;
    let old = store
        .claim_recovery_upload(head.event_id, head.ordinal)
        .await
        .unwrap()
        .unwrap();
    store
        .mark_recovery_unknown(head.event_id, head.ordinal, old.operation.id)
        .await
        .unwrap();
    f.put(head.key.path.as_str(), "occupied");
    let RemoteRead::Present(occupied) = client(&f).read(&head.key).await.unwrap() else {
        panic!()
    };
    let replacement = store
        .relocate_recovery_upload(head.event_id, head.ordinal, old.operation.id, occupied)
        .await
        .unwrap();
    let sync = start(&store, &f).await;
    until(|| {
        f.body(replacement.operation.key.path.as_str()).as_deref() == Some("本地\r\n💾")
            && sync.snapshot().pending == 0
    })
    .await;
    assert!(
        !store
            .load_sync_page(&key("one.md"))
            .await
            .unwrap()
            .unwrap()
            .1
            .pending
    );
    let heads = store.recovery_uploads(config().id(), 100).await.unwrap();
    assert!(heads.iter().any(|h| h.operation_id == old.operation.id
        && h.state == RecoveryUploadState::Unknown
        && !h.current));
    store
        .save_local(head.key.clone(), "edited occupied".into(), LocalRevision(2))
        .await
        .unwrap();
    until(|| f.body(head.key.path.as_str()).as_deref() == Some("edited occupied")).await;
    sync.shutdown().await.unwrap();
    store.shutdown().await.unwrap();
    let store = StoreHandle::open(&path).await.unwrap();
    assert_eq!(
        store.cache_progress(config().id()).await.unwrap().pending,
        0
    );
    assert!(
        store
            .recovery_uploads(config().id(), 100)
            .await
            .unwrap()
            .iter()
            .any(|h| h.operation_id == old.operation.id)
    );
    store.shutdown().await.unwrap();
}

#[tokio::test]
async fn newer_native_source_survives_canonical_upload_in_flight() {
    let dir = tempfile::tempdir().unwrap();
    let store = StoreHandle::open(&dir.path().join("test.sqlite"))
        .await
        .unwrap();
    let f = SpaceFixture::start().await;
    f.put("one.md", "red fox jumps\n\nlast\n");
    let c = client(&f);
    store
        .apply_remote(c.read(&key("one.md")).await.unwrap())
        .await
        .unwrap();
    store
        .save_local(
            key("one.md"),
            "blue fox jumps\n\nlast\n".into(),
            LocalRevision(1),
        )
        .await
        .unwrap();
    f.put("one.md", "red fox runs\n\nlast\n");
    store
        .apply_remote(c.read(&key("one.md")).await.unwrap())
        .await
        .unwrap();
    f.data.lock().unwrap().delay_put = Some("one.md".into());
    let sync = start(&store, &f).await;
    until(|| f.data.lock().unwrap().delay_put.is_none()).await;
    store
        .save_local(
            key("one.md"),
            "blue fox runs\n\nnew native source\n".into(),
            LocalRevision(2),
        )
        .await
        .unwrap();
    f.data.lock().unwrap().release.notify_one();
    until(|| {
        f.body("one.md").as_deref() == Some("blue fox runs\n\nnew native source\n")
            && sync.snapshot().pending == 0
    })
    .await;
    assert_eq!(
        store.load_page(&key("one.md")).await.unwrap().unwrap().body,
        "blue fox runs\n\nnew native source\n"
    );
    sync.shutdown().await.unwrap();
    store.shutdown().await.unwrap();
}

#[tokio::test]
async fn retirement_rolls_back_evidence_and_intent_if_preservation_fails() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.sqlite");
    let store = StoreHandle::open(&path).await.unwrap();
    let f = SpaceFixture::start().await;
    f.put("one.md", "original");
    let c = client(&f);
    store
        .apply_remote(c.read(&key("one.md")).await.unwrap())
        .await
        .unwrap();
    store
        .move_to_trash(key("one.md"), LocalRevision(0))
        .await
        .unwrap();
    let operation = store
        .claim_pending(config().id(), 1)
        .await
        .unwrap()
        .remove(0);
    store.mark_unknown(operation.id).await.unwrap();
    let db = rusqlite::Connection::open(&path).unwrap();
    db.execute_batch("CREATE TRIGGER reject_watch BEFORE INSERT ON retired_delete_watches BEGIN SELECT RAISE(ABORT,'injected'); END;").unwrap();
    let RemoteRead::Present(observed) = c.read(&key("one.md")).await.unwrap() else {
        panic!()
    };
    assert!(
        store
            .retire_unknown_delete(operation.id, observed.clone())
            .await
            .is_err()
    );
    assert_eq!(
        store.unknown_mutations(config().id()).await.unwrap(),
        vec![operation.clone()]
    );
    assert!(
        store
            .retired_delete_keys(config().id())
            .await
            .unwrap()
            .is_empty()
    );
    assert!(matches!(
        store.load_active_page(&key("one.md")).await,
        Err(bulletnotes::domain::error::StoreError::PageMissing)
    ));
    db.execute_batch("DROP TRIGGER reject_watch").unwrap();
    store
        .retire_unknown_delete(operation.id, observed)
        .await
        .unwrap();
    assert!(store.retry_after_readback(operation.id).await.is_err());
    assert!(
        store
            .acknowledge(
                operation.id,
                MutationAck {
                    remote_revision: None
                }
            )
            .await
            .is_err()
    );
    assert_eq!(
        store
            .load_active_page(&key("one.md"))
            .await
            .unwrap()
            .unwrap()
            .body,
        "original"
    );
    store.shutdown().await.unwrap();
}

#[tokio::test]
async fn stale_editor_save_is_automatically_reconsidered_after_worker_is_idle() {
    let dir = tempfile::tempdir().unwrap();
    let store = StoreHandle::open(&dir.path().join("test.sqlite"))
        .await
        .unwrap();
    let f = SpaceFixture::start().await;
    f.put("one.md", "base");
    let sync = start(&store, &f).await;
    until(|| sync.snapshot().complete).await;
    let baseline = store.load_page(&key("one.md")).await.unwrap().unwrap();
    f.put("one.md", "remote");
    store
        .apply_remote(client(&f).read(&key("one.md")).await.unwrap())
        .await
        .unwrap();
    store
        .save_local_from_base(
            key("one.md"),
            "late native".into(),
            LocalRevision(1),
            EditorBaseline {
                body: baseline.body,
                remote_base: baseline.remote_base,
            },
        )
        .await
        .unwrap();
    until(|| {
        f.data
            .lock()
            .unwrap()
            .files
            .values()
            .any(|f| f.body == "late native")
            && sync.snapshot().pending == 0
    })
    .await;
    assert_eq!(f.body("one.md").as_deref(), Some("remote"));
    assert!(store.conflicts(config().id()).await.unwrap().is_empty());
    sync.shutdown().await.unwrap();
    store.shutdown().await.unwrap();
}

#[tokio::test]
async fn legacy_unknown_delete_is_discovered_and_preserves_authored_draft() {
    let dir = tempfile::tempdir().unwrap();
    let store = StoreHandle::open(&dir.path().join("test.sqlite"))
        .await
        .unwrap();
    let f = SpaceFixture::start().await;
    f.put("one.md", "base");
    let c = client(&f);
    store
        .apply_remote(c.read(&key("one.md")).await.unwrap())
        .await
        .unwrap();
    store
        .move_to_trash(key("one.md"), LocalRevision(0))
        .await
        .unwrap();
    let operation = store
        .claim_pending(config().id(), 1)
        .await
        .unwrap()
        .remove(0);
    store.mark_unknown(operation.id).await.unwrap();
    let local = store.load_page(&key("one.md")).await.unwrap().unwrap();
    f.put("one.md", "concurrent edit");
    store
        .save_conflict(ConflictRecord {
            id: uuid::Uuid::new_v4(),
            key: key("one.md"),
            base: local.remote_base.clone(),
            local,
            remote: c.read(&key("one.md")).await.unwrap(),
        })
        .await
        .unwrap();
    store
        .save_merge_drafts(vec![(key("one.md"), "unfinished\r\n草稿".into())])
        .await
        .unwrap();
    let sync = start(&store, &f).await;
    until(|| {
        sync.snapshot().complete && sync.snapshot().pending == 0 && sync.snapshot().conflicts == 0
    })
    .await;
    assert_eq!(f.body("one.md").as_deref(), Some("concurrent edit"));
    assert!(
        f.data
            .lock()
            .unwrap()
            .files
            .values()
            .any(|file| file.body == "unfinished\r\n草稿")
    );
    assert!(
        f.data
            .lock()
            .unwrap()
            .files
            .values()
            .any(|file| file.body == "base")
    );
    assert!(
        !f.data
            .lock()
            .unwrap()
            .deletes
            .iter()
            .any(|d| d.0 == "one.md")
    );
    assert!(
        store
            .unknown_mutations(config().id())
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        store.retired_delete_keys(config().id()).await.unwrap(),
        vec![key("one.md")]
    );
    sync.shutdown().await.unwrap();
    store.shutdown().await.unwrap();
}

#[tokio::test]
async fn watched_missing_uses_current_source_and_survives_denied_repair_restart() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.sqlite");
    let store = StoreHandle::open(&path).await.unwrap();
    let f = SpaceFixture::start().await;
    f.put("one.md", "original");
    let c = client(&f);
    store
        .apply_remote(c.read(&key("one.md")).await.unwrap())
        .await
        .unwrap();
    store
        .move_to_trash(key("one.md"), LocalRevision(0))
        .await
        .unwrap();
    let op = store
        .claim_pending(config().id(), 1)
        .await
        .unwrap()
        .remove(0);
    store.mark_unknown(op.id).await.unwrap();
    let RemoteRead::Present(observed) = c.read(&key("one.md")).await.unwrap() else {
        panic!()
    };
    store.retire_unknown_delete(op.id, observed).await.unwrap();
    store
        .save_local(key("one.md"), "newest\r\n原稿".into(), LocalRevision(1))
        .await
        .unwrap();
    {
        let mut data = f.data.lock().unwrap();
        data.files.remove("one.md");
        data.denied_put = Some("one.md".into());
    }
    let sync = start(&store, &f).await;
    until(|| sync.snapshot().page_errors.get(&key("one.md")) == Some(&SyncError::Permission)).await;
    assert!(sync.snapshot().pending > 0);
    assert_eq!(
        store.load_page(&key("one.md")).await.unwrap().unwrap().body,
        "newest\r\n原稿"
    );
    assert!(
        f.data
            .lock()
            .unwrap()
            .puts
            .iter()
            .any(|p| p.0 == "one.md" && p.1 == "newest\r\n原稿" && p.2)
    );
    sync.shutdown().await.unwrap();
    store.shutdown().await.unwrap();
    let store = StoreHandle::open(&path).await.unwrap();
    assert!(
        store
            .load_sync_page(&key("one.md"))
            .await
            .unwrap()
            .unwrap()
            .1
            .pending
    );
    assert_eq!(
        store.retired_delete_keys(config().id()).await.unwrap(),
        vec![key("one.md")]
    );
    assert_eq!(
        store.load_page(&key("one.md")).await.unwrap().unwrap().body,
        "newest\r\n原稿"
    );
    store.shutdown().await.unwrap();
}

#[tokio::test]
async fn missing_watch_repair_with_occupied_destination_uses_automatic_policy() {
    let dir = tempfile::tempdir().unwrap();
    let store = StoreHandle::open(&dir.path().join("test.sqlite"))
        .await
        .unwrap();
    let f = SpaceFixture::start().await;
    f.put("one.md", "");
    let c = client(&f);
    store
        .apply_remote(c.read(&key("one.md")).await.unwrap())
        .await
        .unwrap();
    store
        .move_to_trash(key("one.md"), LocalRevision(0))
        .await
        .unwrap();
    let op = store
        .claim_pending(config().id(), 1)
        .await
        .unwrap()
        .remove(0);
    store.mark_unknown(op.id).await.unwrap();
    let RemoteRead::Present(observed) = c.read(&key("one.md")).await.unwrap() else {
        panic!()
    };
    store.retire_unknown_delete(op.id, observed).await.unwrap();
    f.data.lock().unwrap().files.remove("one.md");
    store
        .apply_remote(RemoteRead::Missing(key("one.md")))
        .await
        .unwrap();
    let pending = store
        .claim_pending(config().id(), 1)
        .await
        .unwrap()
        .remove(0);
    assert_eq!(pending.kind, MutationKind::Put(String::new()));
    assert_eq!(pending.condition, WriteCondition::CreateOnly);
    store.mark_unknown(pending.id).await.unwrap();
    f.put("one.md", "new occupant");
    let sync = start(&store, &f).await;
    until(|| sync.snapshot().complete && sync.snapshot().pending == 0).await;
    assert_eq!(f.body("one.md").as_deref(), Some("new occupant"));
    assert!(
        f.data
            .lock()
            .unwrap()
            .files
            .iter()
            .any(|(path, file)| path.starts_with("Bulletnotes Recovery/") && file.body.is_empty())
    );
    sync.shutdown().await.unwrap();
    store.shutdown().await.unwrap();
}

#[tokio::test]
async fn watch_evidence_survives_page_removal_and_repairs_from_exact_snapshot() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.sqlite");
    let store = StoreHandle::open(&path).await.unwrap();
    let f = SpaceFixture::start().await;
    f.put("one.md", "saved\r\n💾");
    let c = client(&f);
    store
        .apply_remote(c.read(&key("one.md")).await.unwrap())
        .await
        .unwrap();
    store
        .move_to_trash(key("one.md"), LocalRevision(0))
        .await
        .unwrap();
    let op = store
        .claim_pending(config().id(), 1)
        .await
        .unwrap()
        .remove(0);
    store.mark_unknown(op.id).await.unwrap();
    let RemoteRead::Present(observed) = c.read(&key("one.md")).await.unwrap() else {
        panic!()
    };
    store.retire_unknown_delete(op.id, observed).await.unwrap();
    let db = rusqlite::Connection::open(&path).unwrap();
    db.execute("DELETE FROM pages WHERE path='one.md'", [])
        .unwrap();
    // Catalogue metadata can recreate an uncached row before the watch is read.
    db.execute("INSERT INTO pages(connection,path,title,body,local_revision) VALUES(?1,'one.md','one',NULL,'0')",[config().id().as_str()]).unwrap();
    f.data.lock().unwrap().files.remove("one.md");
    store
        .apply_remote(RemoteRead::Missing(key("one.md")))
        .await
        .unwrap();
    assert_eq!(
        store.load_page(&key("one.md")).await.unwrap().unwrap().body,
        "saved\r\n💾"
    );
    assert!(store.cache_progress(config().id()).await.unwrap().pending > 0);
    let sync = start(&store, &f).await;
    until(|| f.body("one.md").as_deref() == Some("saved\r\n💾") && sync.snapshot().pending == 0)
        .await;
    sync.shutdown().await.unwrap();
    store.shutdown().await.unwrap();
}

#[tokio::test]
async fn readonly_occupied_recovery_is_relocated_without_writing_that_path() {
    let dir = tempfile::tempdir().unwrap();
    let store = StoreHandle::open(&dir.path().join("test.sqlite"))
        .await
        .unwrap();
    let f = SpaceFixture::start().await;
    let head = recovery(&store, &f).await;
    f.put(head.key.path.as_str(), "protected occupant");
    f.data.lock().unwrap().read_only = Some(head.key.path.as_str().into());
    store
        .record_catalogue(config().id(), client(&f).list_files().await.unwrap(), true)
        .await
        .unwrap();
    store.block_page(&head.key).await.unwrap();
    let sync = start(&store, &f).await;
    until(|| sync.snapshot().complete && sync.snapshot().pending == 0).await;
    assert_eq!(
        f.body(head.key.path.as_str()).as_deref(),
        Some("protected occupant")
    );
    assert!(
        !f.data
            .lock()
            .unwrap()
            .puts
            .iter()
            .any(|p| p.0 == head.key.path.as_str())
    );
    assert!(
        f.data
            .lock()
            .unwrap()
            .files
            .values()
            .any(|f| f.body == "本地\r\n💾")
    );
    sync.shutdown().await.unwrap();
    store.shutdown().await.unwrap();
}

#[tokio::test]
async fn definitive_delete_rejection_survives_restart_without_resurrection_authority() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.sqlite");
    let store = StoreHandle::open(&path).await.unwrap();
    let f = SpaceFixture::start().await;
    f.put("one.md", "base");
    let c = client(&f);
    store
        .apply_remote(c.read(&key("one.md")).await.unwrap())
        .await
        .unwrap();
    store
        .move_to_trash(key("one.md"), LocalRevision(0))
        .await
        .unwrap();
    let op = store
        .claim_pending(config().id(), 1)
        .await
        .unwrap()
        .remove(0);
    f.put("one.md", "concurrent edit");
    assert_eq!(c.delete(&op).await.unwrap_err(), SyncError::Conflict);
    store.record_delete_rejection(op.id).await.unwrap();
    store.shutdown().await.unwrap();
    let store = StoreHandle::open(&path).await.unwrap();
    let sync = start(&store, &f).await;
    until(|| sync.snapshot().complete && sync.snapshot().pending == 0).await;
    assert_eq!(
        store.load_page(&key("one.md")).await.unwrap().unwrap().body,
        "concurrent edit"
    );
    assert!(
        store
            .retired_delete_keys(config().id())
            .await
            .unwrap()
            .is_empty(),
        "terminal rejection must not grant repair authority"
    );
    f.data.lock().unwrap().files.remove("one.md");
    sync.refresh();
    tokio::time::timeout(Duration::from_secs(5), async {
        while !store
            .load_sync_page(&key("one.md"))
            .await
            .unwrap()
            .unwrap()
            .1
            .deleted
        {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    assert!(f.body("one.md").is_none());
    assert!(!f.data.lock().unwrap().puts.iter().any(|p| p.0 == "one.md"));
    assert_eq!(
        f.data
            .lock()
            .unwrap()
            .delete_requests
            .iter()
            .filter(|path| *path == "one.md")
            .count(),
        1
    );
    let db = rusqlite::Connection::open(&path).unwrap();
    let evidence: (bool, bool) = db
        .query_row(
            "SELECT terminal_rejected,active FROM retired_delete_watches WHERE operation_id=?1",
            [op.id.0.to_string()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(evidence, (true, false));
    sync.shutdown().await.unwrap();
    store.shutdown().await.unwrap();
}

#[tokio::test]
async fn worker_persists_guard_rejected_delete_without_retry_or_resurrection() {
    let dir = tempfile::tempdir().unwrap();
    let store = StoreHandle::open(&dir.path().join("test.sqlite"))
        .await
        .unwrap();
    let f = SpaceFixture::start().await;
    f.put("one.md", "base");
    store
        .apply_remote(client(&f).read(&key("one.md")).await.unwrap())
        .await
        .unwrap();
    store
        .move_to_trash(key("one.md"), LocalRevision(0))
        .await
        .unwrap();
    f.data.lock().unwrap().replace_before_delete =
        Some(("one.md".into(), "concurrent edit".into()));
    let sync = start(&store, &f).await;
    until(|| sync.snapshot().complete && sync.snapshot().pending == 0).await;
    assert_eq!(
        store.load_page(&key("one.md")).await.unwrap().unwrap().body,
        "concurrent edit"
    );
    assert!(
        store
            .retired_delete_keys(config().id())
            .await
            .unwrap()
            .is_empty()
    );
    f.data.lock().unwrap().files.remove("one.md");
    sync.refresh();
    tokio::time::timeout(Duration::from_secs(5), async {
        while !store
            .load_sync_page(&key("one.md"))
            .await
            .unwrap()
            .unwrap()
            .1
            .deleted
        {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    assert!(f.body("one.md").is_none());
    assert_eq!(
        f.data
            .lock()
            .unwrap()
            .delete_requests
            .iter()
            .filter(|path| *path == "one.md")
            .count(),
        1
    );
    assert!(!f.data.lock().unwrap().puts.iter().any(|p| p.0 == "one.md"));
    sync.shutdown().await.unwrap();
    store.shutdown().await.unwrap();
}
