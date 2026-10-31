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
async fn executing_activity_tracks_a_real_held_put_and_clears_on_shutdown() {
    let dir = tempfile::tempdir().unwrap();
    let store = StoreHandle::open(&dir.path().join("activity.sqlite"))
        .await
        .unwrap();
    let f = SpaceFixture::start().await;
    f.put("one.md", "base");
    store
        .apply_remote(client(&f).read(&key("one.md")).await.unwrap())
        .await
        .unwrap();
    f.data.lock().unwrap().delay_put = Some("one.md".into());
    let release = f.data.lock().unwrap().release.clone();
    store
        .save_local(key("one.md"), "edited".into(), LocalRevision(1))
        .await
        .unwrap();
    let sync = start(&store, &f).await;
    until(|| f.data.lock().unwrap().delay_put.is_none()).await;
    assert!(sync.snapshot().executing.contains(&key("one.md")));
    release.notify_one();
    until(|| sync.snapshot().pending == 0).await;
    until(|| !sync.snapshot().executing.contains(&key("one.md"))).await;
    f.data.lock().unwrap().delay_put = Some("one.md".into());
    store
        .save_local(key("one.md"), "second".into(), LocalRevision(2))
        .await
        .unwrap();
    sync.refresh();
    until(|| f.data.lock().unwrap().delay_put.is_none()).await;
    assert!(sync.snapshot().executing.contains(&key("one.md")));
    sync.shutdown().await.unwrap();
    assert!(
        sync.snapshot().executing.is_empty(),
        "cancelled work must not leave the indicator blinking"
    );
}
#[tokio::test]
async fn denied_canonical_put_recovers_after_fresh_permission_without_restart() {
    for refresh in [true, false] {
        let dir = tempfile::tempdir().unwrap();
        let store = StoreHandle::open(&dir.path().join("test.sqlite"))
            .await
            .unwrap();
        let f = SpaceFixture::start().await;
        f.put("one.md", "red fox jumps");
        store
            .apply_remote(client(&f).read(&key("one.md")).await.unwrap())
            .await
            .unwrap();
        store
            .save_local(key("one.md"), "red fox runs".into(), LocalRevision(1))
            .await
            .unwrap();
        f.data.lock().unwrap().denied_put = Some("one.md".into());
        let sync = start(&store, &f).await;
        until(|| sync.snapshot().page_errors.get(&key("one.md")) == Some(&SyncError::Permission))
            .await;
        let unknown = store
            .unknown_mutations(config().id())
            .await
            .unwrap()
            .remove(0);
        f.data.lock().unwrap().read_only = Some("one.md".into());
        let before = *f.data.lock().unwrap().reads.get("one.md").unwrap();
        sync.refresh();
        tokio::time::sleep(Duration::from_millis(1300)).await;
        let reads = *f.data.lock().unwrap().reads.get("one.md").unwrap() - before;
        assert!(
            (1..=4).contains(&reads),
            "read-only readback must be bounded: {reads}"
        );
        assert_eq!(
            store.unknown_mutations(config().id()).await.unwrap()[0],
            unknown
        );
        assert_eq!(
            f.data
                .lock()
                .unwrap()
                .puts
                .iter()
                .filter(|p| p.0 == "one.md")
                .count(),
            1
        );
        assert!(sync.snapshot().pending > 0);
        assert_eq!(f.body("one.md").as_deref(), Some("red fox jumps"));
        {
            let mut data = f.data.lock().unwrap();
            data.denied_put = None;
            data.read_only = None;
        }
        if refresh {
            sync.refresh();
        }
        until(|| {
            f.body("one.md").as_deref() == Some("red fox runs") && sync.snapshot().pending == 0
        })
        .await;
        assert!(!sync.snapshot().page_errors.contains_key(&key("one.md")));
        assert!(
            store
                .unknown_mutations(config().id())
                .await
                .unwrap()
                .is_empty()
        );
        sync.shutdown().await.unwrap();
        store.shutdown().await.unwrap();
    }
}
#[tokio::test]
async fn denied_unknown_readback_is_bounded_and_recovers_without_restart() {
    for refresh in [true, false] {
        let dir = tempfile::tempdir().unwrap();
        let store = StoreHandle::open(&dir.path().join("test.sqlite"))
            .await
            .unwrap();
        let f = SpaceFixture::start().await;
        f.put("one.md", "red fox jumps");
        store
            .apply_remote(client(&f).read(&key("one.md")).await.unwrap())
            .await
            .unwrap();
        store
            .save_local(key("one.md"), "red fox runs".into(), LocalRevision(1))
            .await
            .unwrap();
        f.data.lock().unwrap().denied_put = Some("one.md".into());
        let sync = start(&store, &f).await;
        until(|| sync.snapshot().page_errors.get(&key("one.md")) == Some(&SyncError::Permission))
            .await;
        let unknown = store
            .unknown_mutations(config().id())
            .await
            .unwrap()
            .remove(0);
        f.data.lock().unwrap().denied_read = Some("one.md".into());
        let before = *f.data.lock().unwrap().reads.get("one.md").unwrap();
        sync.refresh();
        store.create_draft(key("peer.md")).await.unwrap();
        store
            .save_local(key("peer.md"), "unrelated peer".into(), LocalRevision(1))
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_millis(1300)).await;
        let reads = *f.data.lock().unwrap().reads.get("one.md").unwrap() - before;
        assert!(
            (1..=4).contains(&reads),
            "GET403 readback must be bounded: {reads}"
        );
        until(|| f.body("peer.md").as_deref() == Some("unrelated peer")).await;
        assert_eq!(
            store.unknown_mutations(config().id()).await.unwrap(),
            vec![unknown]
        );
        assert_eq!(
            f.data
                .lock()
                .unwrap()
                .puts
                .iter()
                .filter(|p| p.0 == "one.md")
                .count(),
            1
        );
        assert!(sync.snapshot().pending > 0);
        assert_eq!(f.body("one.md").as_deref(), Some("red fox jumps"));
        {
            let mut data = f.data.lock().unwrap();
            data.denied_put = None;
            data.denied_read = None;
        }
        if refresh {
            sync.refresh();
        }
        until(|| {
            f.body("one.md").as_deref() == Some("red fox runs") && sync.snapshot().pending == 0
        })
        .await;
        assert!(!sync.snapshot().page_errors.contains_key(&key("one.md")));
        assert!(
            store
                .unknown_mutations(config().id())
                .await
                .unwrap()
                .is_empty()
        );
        sync.shutdown().await.unwrap();
        store.shutdown().await.unwrap();
    }
}
#[tokio::test]
async fn blocked_handoffs_bound_reads_and_replay_latest_after_settlement() {
    for (fenced, read_only) in [(false, false), (false, true), (true, false)] {
        let dir = tempfile::tempdir().unwrap();
        let store = StoreHandle::open(&dir.path().join("test.sqlite"))
            .await
            .unwrap();
        let f = SpaceFixture::start().await;
        let recovery_head = if fenced {
            Some(recovery(&store, &f).await)
        } else {
            f.put("one.md", "red fox jumps");
            store
                .apply_remote(client(&f).read(&key("one.md")).await.unwrap())
                .await
                .unwrap();
            store
                .save_local(key("one.md"), "red fox runs".into(), LocalRevision(1))
                .await
                .unwrap();
            None
        };
        let target = recovery_head
            .as_ref()
            .map_or_else(|| key("one.md"), |head| head.key.clone());
        let stored = store.load_page(&target).await.unwrap().unwrap();
        let baseline = EditorBaseline {
            body: if fenced {
                "本地\r\nold"
            } else {
                "red fox jumps"
            }
            .into(),
            remote_base: stored.remote_base.clone(),
        };
        let earlier = if fenced {
            "new 本地\r\n💾"
        } else {
            "blue fox runs"
        };
        let latest = if fenced {
            "latest 本地\r\n💾"
        } else {
            "purple fox runs"
        };
        {
            let mut data = f.data.lock().unwrap();
            data.denied_put = Some(target.path.as_str().into());
            data.delay_put = Some(target.path.as_str().into());
        }
        let sync = start(&store, &f).await;
        until(|| f.data.lock().unwrap().delay_put.is_none()).await;
        for (revision, body) in [(2, earlier), (3, latest)] {
            let saved = store
                .save_local_from_base_outcome(
                    target.clone(),
                    body.into(),
                    LocalRevision(revision),
                    baseline.clone(),
                )
                .await
                .unwrap();
            assert_eq!(saved.attempt.local_revision, LocalRevision(revision));
            assert_eq!(
                saved.stored.page.body, stored.body,
                "blocked replay cannot acknowledge the authored source as canonical"
            );
        }
        {
            let mut data = f.data.lock().unwrap();
            if read_only {
                data.read_only = Some(target.path.as_str().into());
            }
            data.release.notify_one();
        }
        until(|| sync.snapshot().page_errors.get(&target) == Some(&SyncError::Permission)).await;
        let before = *f
            .data
            .lock()
            .unwrap()
            .reads
            .get(target.path.as_str())
            .unwrap_or(&0);
        store.create_draft(key("peer.md")).await.unwrap();
        store
            .save_local(key("peer.md"), "unrelated peer".into(), LocalRevision(1))
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_millis(500)).await;
        let reads = *f
            .data
            .lock()
            .unwrap()
            .reads
            .get(target.path.as_str())
            .unwrap_or(&0)
            - before;
        assert!(
            reads <= 3,
            "blocked handoff hot-looped: {reads} reads/500ms"
        );
        if !fenced && !read_only {
            // A writable readback can replay the handoff before the next denied PUT.
            sync.refresh();
            tokio::time::timeout(Duration::from_secs(8), async {
                while store
                    .load_editor_page(&target)
                    .await
                    .unwrap()
                    .unwrap()
                    .authored
                    .is_some()
                {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            })
            .await
            .expect("writable readback must replay the latest authored source");
        }
        let restored = store.load_editor_page(&target).await.unwrap().unwrap();
        assert_eq!(restored.presented().body, latest);
        if fenced {
            assert!(
                restored.authored.is_some(),
                "unacknowledged recovery must retain the authored branch"
            );
        }
        until(|| f.body("peer.md").as_deref() == Some("unrelated peer")).await;
        assert!(sync.snapshot().pending > 0);
        if let Some(head) = &recovery_head {
            assert_eq!(
                store
                    .read_recovery_upload(head.event_id, head.ordinal, head.operation_id)
                    .await
                    .unwrap()
                    .operation
                    .kind,
                MutationKind::Put("本地\r\n💾".into())
            );
        }
        {
            let mut data = f.data.lock().unwrap();
            data.denied_put = None;
            data.read_only = None;
        }
        sync.refresh();
        let settled = tokio::time::timeout(Duration::from_secs(8), async {
            while f.body(target.path.as_str()).as_deref() != Some(latest)
                || sync.snapshot().pending != 0
            {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await;
        assert!(
            settled.is_ok(),
            "fenced={fenced}: pending={}, error={:?}, page_errors={:?}, unknown={}, latest_remote={}, latest_stored={}, authored_pending={}",
            sync.snapshot().pending,
            sync.snapshot().error,
            sync.snapshot().page_errors.values().collect::<Vec<_>>(),
            store.unknown_mutations(config().id()).await.unwrap().len(),
            f.body(target.path.as_str()).as_deref() == Some(latest),
            store.load_page(&target).await.unwrap().unwrap().body == latest,
            store
                .load_editor_page(&target)
                .await
                .unwrap()
                .unwrap()
                .authored
                .is_some()
        );
        assert_eq!(
            store.load_page(&target).await.unwrap().unwrap().body,
            latest
        );
        assert!(
            store
                .load_editor_page(&target)
                .await
                .unwrap()
                .unwrap()
                .authored
                .is_none()
        );
        let copies = store.recovery_copies(config().id()).await.unwrap();
        for body in [earlier, latest, stored.body.as_str()] {
            assert!(
                copies.iter().any(|copy| copy.page.body == body),
                "lost admitted source"
            );
        }
        if fenced {
            let data = f.data.lock().unwrap();
            let puts: Vec<_> = data
                .puts
                .iter()
                .filter(|put| put.0 == target.path.as_str())
                .collect();
            assert!(
                puts.iter()
                    .filter(|put| put.2)
                    .all(|put| put.1 == "本地\r\n💾")
            );
            assert_eq!(puts.last().unwrap().1, latest);
            assert!(!puts.last().unwrap().2);
        }
        sync.shutdown().await.unwrap();
        store.shutdown().await.unwrap();
    }
}
#[tokio::test]
async fn unknown_recovery_readback_bypasses_fenced_observation_backoff() {
    let dir = tempfile::tempdir().unwrap();
    let store = StoreHandle::open(&dir.path().join("test.sqlite"))
        .await
        .unwrap();
    let f = SpaceFixture::start().await;
    let head = recovery(&store, &f).await;
    let upload = store
        .claim_recovery_upload(head.event_id, head.ordinal)
        .await
        .unwrap()
        .unwrap();
    let latest = "latest 本地\r\n💾";
    store
        .save_local_from_base_outcome(
            head.key.clone(),
            latest.into(),
            LocalRevision(2),
            EditorBaseline {
                body: "本地\r\nold".into(),
                remote_base: None,
            },
        )
        .await
        .unwrap();
    let sync = start(&store, &f).await;
    until(|| {
        f.data
            .lock()
            .unwrap()
            .reads
            .get(head.key.path.as_str())
            .copied()
            .unwrap_or(0)
            > 0
    })
    .await;
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(
        store
            .load_editor_page(&head.key)
            .await
            .unwrap()
            .unwrap()
            .authored
            .is_some()
    );
    // Model the leased transport becoming uncertain after the fenced observation has backed off.
    client(&f).put(&upload.operation).await.unwrap();
    store
        .mark_recovery_unknown(head.event_id, head.ordinal, head.operation_id)
        .await
        .unwrap();
    store.create_draft(key("peer.md")).await.unwrap();
    tokio::time::timeout(Duration::from_millis(500), async {
        while !store
            .recovery_uploads(config().id(), 10)
            .await
            .unwrap()
            .is_empty()
        {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("unknown recovery readback must not wait for the observation backoff");
    assert_eq!(
        f.body(head.key.path.as_str()).as_deref(),
        Some("本地\r\n💾")
    );
    assert!(
        store
            .load_editor_page(&head.key)
            .await
            .unwrap()
            .unwrap()
            .authored
            .is_some()
    );
    until(|| {
        f.body(head.key.path.as_str()).as_deref() == Some(latest) && sync.snapshot().pending == 0
    })
    .await;
    assert!(
        store
            .load_editor_page(&head.key)
            .await
            .unwrap()
            .unwrap()
            .authored
            .is_none()
    );
    sync.shutdown().await.unwrap();
    store.shutdown().await.unwrap();
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
    let denied_puts = || {
        f.data
            .lock()
            .unwrap()
            .puts
            .iter()
            .filter(|p| p.0 == head.key.path.as_str())
            .count()
    };
    assert_eq!(denied_puts(), 1);
    tokio::time::sleep(Duration::from_millis(1300)).await;
    assert_eq!(denied_puts(), 1, "denial must retain bounded retry cadence");
    sync.refresh();
    until(|| denied_puts() == 2).await;
    tokio::time::sleep(Duration::from_millis(1300)).await;
    assert_eq!(denied_puts(), 2, "repeated denial must back off again");
    f.data.lock().unwrap().denied_put = None;
    sync.refresh();
    until(|| {
        sync.snapshot().pending == 0
            && !sync.snapshot().page_errors.contains_key(&key("one.md"))
            && !sync.snapshot().page_errors.contains_key(&head.key)
    })
    .await;
    assert_eq!(
        f.body(head.key.path.as_str()).as_deref(),
        Some("本地\r\n💾")
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
            .page
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
    // Make remote visibility precede acknowledgment even without scheduler pressure.
    f.data.lock().unwrap().lost_put = Some(head.key.path.as_str().into());
    store
        .save_local(head.key.clone(), "edited occupied".into(), LocalRevision(2))
        .await
        .unwrap();
    until(|| f.body(head.key.path.as_str()).as_deref() == Some("edited occupied")).await;
    // A remote body or stale zero-valued snapshot does not prove a durable acknowledgment.
    tokio::time::timeout(Duration::from_secs(8), async {
        while store.cache_progress(config().id()).await.unwrap().pending != 0 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("final upload must settle durably before shutdown");
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
            .page
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
    tokio::time::sleep(Duration::from_millis(100)).await;
    let reads = f
        .data
        .lock()
        .unwrap()
        .reads
        .get("one.md")
        .copied()
        .unwrap_or(0);
    sync.set_selected_page(Some(key("one.md")));
    tokio::time::sleep(Duration::from_millis(1200)).await;
    assert_eq!(
        f.data
            .lock()
            .unwrap()
            .reads
            .get("one.md")
            .copied()
            .unwrap_or(0),
        reads,
        "foreground selection must preserve the retired-delete watch cadence"
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
    // Settle the ordinary upload so its denial cannot be mistaken for a repair denial.
    let sync = start(&store, &f).await;
    until(|| f.body("one.md").as_deref() == Some("newest\r\n原稿") && sync.snapshot().pending == 0)
        .await;
    sync.shutdown().await.unwrap();
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

#[tokio::test]
async fn blocked_recovery_restarts_and_settles_after_refresh_or_exact_readback() {
    for matching in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.sqlite");
        let store = StoreHandle::open(&path).await.unwrap();
        let f = SpaceFixture::start().await;
        let head = recovery(&store, &f).await;
        f.data.lock().unwrap().denied_put = Some(head.key.path.as_str().into());
        store.create_draft(key("peer.md")).await.unwrap();
        store
            .save_local(key("peer.md"), "peer".into(), LocalRevision(1))
            .await
            .unwrap();
        let sync = start(&store, &f).await;
        until(|| sync.snapshot().page_errors.get(&key("one.md")) == Some(&SyncError::Permission))
            .await;
        until(|| f.body("peer.md").as_deref() == Some("peer")).await;
        assert!(sync.snapshot().pending > 0);
        sync.shutdown().await.unwrap();
        store.shutdown().await.unwrap();
        let store = StoreHandle::open(&path).await.unwrap();
        let db = rusqlite::Connection::open(&path).unwrap();
        let state: String = db
            .query_row(
                "SELECT state FROM recovery_uploads WHERE operation_id=?1",
                [head.operation_id.0.to_string()],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(state, "blocked");
        let before = f
            .data
            .lock()
            .unwrap()
            .puts
            .iter()
            .filter(|p| p.0 == head.key.path.as_str())
            .count();
        if matching {
            f.put(head.key.path.as_str(), "本地\r\n💾");
        }
        f.data.lock().unwrap().denied_put = None;
        let sync = start(&store, &f).await;
        sync.refresh();
        until(|| {
            f.body(head.key.path.as_str()).as_deref() == Some("本地\r\n💾")
                && sync.snapshot().complete
                && sync.snapshot().pending == 0
                && !sync.snapshot().page_errors.contains_key(&key("one.md"))
                && !sync.snapshot().page_errors.contains_key(&head.key)
        })
        .await;
        assert!(sync.snapshot().error.is_none());
        assert!(
            !store
                .load_sync_page(&key("one.md"))
                .await
                .unwrap()
                .unwrap()
                .1
                .pending
        );
        let after = f
            .data
            .lock()
            .unwrap()
            .puts
            .iter()
            .filter(|p| p.0 == head.key.path.as_str())
            .count();
        assert_eq!(after - before, usize::from(!matching));
        assert_eq!(
            store
                .read_recovery_upload(head.event_id, head.ordinal, head.operation_id)
                .await
                .unwrap()
                .operation
                .kind,
            MutationKind::Put("本地\r\n💾".into())
        );
        sync.shutdown().await.unwrap();
        store.shutdown().await.unwrap();
    }
}

#[tokio::test]
async fn uncached_readonly_recovery_occupant_relocates_after_initial_put_denial() {
    let dir = tempfile::tempdir().unwrap();
    let store = StoreHandle::open(&dir.path().join("test.sqlite"))
        .await
        .unwrap();
    let f = SpaceFixture::start().await;
    let head = recovery(&store, &f).await;
    f.put(head.key.path.as_str(), "protected occupant");
    {
        let mut data = f.data.lock().unwrap();
        data.denied_put = Some(head.key.path.as_str().into());
        data.delay_put = Some(head.key.path.as_str().into());
    }
    assert_eq!(
        store
            .load_page(&head.key)
            .await
            .unwrap()
            .unwrap()
            .permission,
        Permission::ReadWrite
    );
    let sync = start(&store, &f).await;
    until(|| f.data.lock().unwrap().delay_put.is_none()).await;
    {
        let mut data = f.data.lock().unwrap();
        data.read_only = Some(head.key.path.as_str().into());
        data.release.notify_one();
    }
    until(|| sync.snapshot().complete && sync.snapshot().pending == 0).await;
    assert_eq!(
        f.body(head.key.path.as_str()).as_deref(),
        Some("protected occupant")
    );
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
    assert!(
        f.data
            .lock()
            .unwrap()
            .files
            .values()
            .any(|f| f.body == "本地\r\n💾")
    );
    assert!(!sync.snapshot().page_errors.contains_key(&key("one.md")));
    assert!(!sync.snapshot().page_errors.contains_key(&head.key));
    sync.shutdown().await.unwrap();
    store.shutdown().await.unwrap();
}

#[tokio::test]
async fn killed_sync_owner_recovers_resolution_draft_and_exact_http_boundaries() {
    use std::process::{Command, Stdio};
    for stage in [
        "before-resolution",
        "after-resolution",
        "recovery-create",
        "recovery-readback",
        "recovery-ack",
        "canonical-upload",
        "before-draft-drain",
        "after-draft-drain",
    ] {
        let directory = tempfile::Builder::new()
            .prefix("bulletnotes-sync-crash-")
            .tempdir()
            .unwrap();
        let path = directory.path().join("owned.sqlite");
        let marker = directory.path().join("ready");
        let nonce = uuid::Uuid::new_v4().to_string();
        let server = SpaceFixture::start().await;
        server.put("one.md", "red fox jumps\n");
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args(["--ignored", "--exact", "sync_process_boundary_child"])
            .env("BULLETNOTES_SYNC_CRASH_PATH", &path)
            .env("BULLETNOTES_SYNC_CRASH_STAGE", stage)
            .env("BULLETNOTES_SYNC_CRASH_NONCE", &nonce)
            .env("BULLETNOTES_SYNC_CRASH_SERVER", server.base.as_str())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(8);
        while !marker.exists() {
            if child.try_wait().unwrap().is_some() || std::time::Instant::now() >= deadline {
                let _ = child.kill();
                panic!(
                    "{stage}: boundary not reached: {}",
                    String::from_utf8_lossy(&child.wait_with_output().unwrap().stderr)
                );
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        assert_eq!(
            std::fs::read_to_string(marker).unwrap(),
            format!("{}:{nonce}", child.id())
        );
        child.kill().unwrap();
        child.wait().unwrap();
        let store = StoreHandle::open(&path).await.unwrap();
        if stage.ends_with("draft-drain") {
            assert_eq!(
                store.merge_draft(&key("one.md")).await.unwrap().as_deref(),
                if stage == "after-draft-drain" {
                    Some("draft drained before owner loss\r\n日本語")
                } else {
                    None
                }
            );
            assert_eq!(store.conflicts(config().id()).await.unwrap().len(), 1);
        }
        let recovered = store.recovery_copies(config().id()).await.unwrap();
        assert!(
            store.load_page(&key("one.md")).await.unwrap().unwrap().body
                == "blue fox jumps\n\nlocal tail"
                || recovered
                    .iter()
                    .any(|copy| copy.page.body == "blue fox jumps\n\nlocal tail"),
            "{stage}: original lost"
        );
        if matches!(stage, "recovery-create" | "recovery-readback") {
            let heads = store.recovery_uploads(config().id(), 10).await.unwrap();
            assert_eq!(heads.len(), 1);
            assert_eq!(heads[0].state, RecoveryUploadState::Unknown);
            let upload = store
                .read_recovery_upload(heads[0].event_id, heads[0].ordinal, heads[0].operation_id)
                .await
                .unwrap();
            assert_eq!(
                upload.operation.kind,
                MutationKind::Put("blue fox jumps\n\nlocal tail".into())
            );
            assert_eq!(
                server.body(heads[0].key.path.as_str()).as_deref(),
                Some("blue fox jumps\n\nlocal tail")
            );
        }
        if stage == "canonical-upload" {
            let unknown = store.unknown_mutations(config().id()).await.unwrap();
            assert!(unknown.iter().any(|op| op.key == key("one.md")));
            assert!(
                store
                    .load_sync_page(&key("one.md"))
                    .await
                    .unwrap()
                    .unwrap()
                    .1
                    .pending
            );
        }
        let sync = start(&store, &server).await;
        until(|| sync.snapshot().complete && sync.snapshot().pending == 0).await;
        let expected = if stage == "canonical-upload" {
            "green fox jumps\n\nlocal tail\nnewer"
        } else {
            "green fox jumps\n\nlocal tail"
        };
        assert_eq!(server.body("one.md").as_deref(), Some(expected), "{stage}");
        assert_eq!(
            store.load_page(&key("one.md")).await.unwrap().unwrap().body,
            expected
        );
        let copies = store.recovery_copies(config().id()).await.unwrap();
        assert!(
            copies
                .iter()
                .any(|copy| copy.page.body == "blue fox jumps\n\nlocal tail"),
            "{stage}"
        );
        if stage == "after-draft-drain" {
            assert!(
                copies
                    .iter()
                    .any(|copy| copy.page.body == "draft drained before owner loss\r\n日本語")
            );
        }
        sync.shutdown().await.unwrap();
        store.shutdown().await.unwrap();
    }
}

#[test]
#[ignore = "owned parent-supervised crash fixture only"]
fn sync_process_boundary_child() {
    let path = std::path::PathBuf::from(std::env::var_os("BULLETNOTES_SYNC_CRASH_PATH").unwrap());
    assert!(
        path.parent()
            .unwrap()
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("bulletnotes-sync-crash-")
    );
    let stage = std::env::var("BULLETNOTES_SYNC_CRASH_STAGE").unwrap();
    let nonce = std::env::var("BULLETNOTES_SYNC_CRASH_NONCE").unwrap();
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let store = StoreHandle::open(&path).await.unwrap();
        let client = SilverBulletClient::for_loopback_fixture(
            &config(),
            url::Url::parse(&std::env::var("BULLETNOTES_SYNC_CRASH_SERVER").unwrap()).unwrap(),
            None,
            Duration::from_secs(2),
        )
        .unwrap();
        let base = client.read(&key("one.md")).await.unwrap();
        store.apply_remote(base).await.unwrap();
        store
            .save_local(
                key("one.md"),
                "blue fox jumps\n\nlocal tail".into(),
                LocalRevision(1),
            )
            .await
            .unwrap();
        let snapshot = store.load_page(&key("one.md")).await.unwrap().unwrap();
        client
            .put(&PendingMutation {
                id: OperationId::new(),
                key: key("one.md"),
                local_revision: LocalRevision(0),
                kind: MutationKind::Put("green fox jumps\n".into()),
                condition: WriteCondition::Match(
                    snapshot
                        .remote_base
                        .as_ref()
                        .unwrap()
                        .revision
                        .clone()
                        .unwrap(),
                ),
            })
            .await
            .unwrap();
        let observed = client.read(&key("one.md")).await.unwrap();
        let mut blocker = None;
        if stage.ends_with("draft-drain") {
            store
                .save_conflict(ConflictRecord {
                    id: uuid::Uuid::new_v4(),
                    key: key("one.md"),
                    base: snapshot.remote_base.clone(),
                    local: snapshot,
                    remote: observed.clone(),
                })
                .await
                .unwrap();
            let mut owner = store.install_draft_admission().unwrap();
            let background = store.clone();
            tokio::spawn(async move {
                let _ = background.apply_remote(observed).await;
            });
            let bulletnotes::store::DraftAdmissionRequest::Close {
                key: target, reply, ..
            } = owner.recv().await.unwrap()
            else {
                panic!("missing draft close boundary");
            };
            if stage == "after-draft-drain" {
                store
                    .save_merge_drafts(vec![(
                        target,
                        "draft drained before owner loss\r\n日本語".into(),
                    )])
                    .await
                    .unwrap();
            }
            // Do not deliver the close acknowledgement or drop its owner before process loss.
            std::mem::forget(reply);
            std::mem::forget(owner);
        } else if stage == "before-resolution" {
            let db = rusqlite::Connection::open(&path).unwrap();
            db.execute_batch("BEGIN IMMEDIATE").unwrap();
            let background = store.clone();
            tokio::spawn(async move {
                let _ = background.apply_remote(observed).await;
            });
            blocker = Some(db);
        } else {
            store.apply_remote(observed).await.unwrap();
            if stage != "after-resolution" {
                let head = store
                    .recovery_uploads(config().id(), 10)
                    .await
                    .unwrap()
                    .remove(0);
                let upload = store
                    .claim_recovery_upload(head.event_id, head.ordinal)
                    .await
                    .unwrap()
                    .unwrap();
                let ack = client.put(&upload.operation).await.unwrap();
                if stage != "recovery-create" {
                    let RemoteRead::Present(file) =
                        client.read(&upload.operation.key).await.unwrap()
                    else {
                        panic!("missing recovery readback");
                    };
                    assert_eq!(file.body, "blue fox jumps\n\nlocal tail");
                    if stage != "recovery-readback" {
                        store
                            .acknowledge_recovery_upload(
                                head.event_id,
                                head.ordinal,
                                upload.operation.id,
                                ack,
                            )
                            .await
                            .unwrap();
                    }
                }
                if stage == "canonical-upload" {
                    let sent = store
                        .claim_pending(config().id(), 100)
                        .await
                        .unwrap()
                        .into_iter()
                        .find(|operation| operation.key == key("one.md"))
                        .unwrap();
                    client.put(&sent).await.unwrap();
                    store
                        .save_local(
                            key("one.md"),
                            "green fox jumps\n\nlocal tail\nnewer".into(),
                            LocalRevision(2),
                        )
                        .await
                        .unwrap();
                }
            }
        }
        tokio::time::sleep(Duration::from_millis(30)).await;
        std::fs::write(
            path.parent().unwrap().join("ready.tmp"),
            format!("{}:{nonce}", std::process::id()),
        )
        .unwrap();
        std::fs::rename(
            path.parent().unwrap().join("ready.tmp"),
            path.parent().unwrap().join("ready"),
        )
        .unwrap();
        std::thread::park();
        drop(blocker);
    });
}

#[tokio::test]
async fn offline_scratchpad_creation_restarts_then_syncs_in_same_stack_order() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("stack.sqlite");
    let store = StoreHandle::open(&path).await.unwrap();
    let first = key("Quick Notes/first.md");
    let empty = key("Quick Notes/empty.md");
    store.create_draft(first.clone()).await.unwrap();
    store
        .save_local(first.clone(), "café\r\n日本語 💾".into(), LocalRevision(1))
        .await
        .unwrap();
    store.create_draft(empty.clone()).await.unwrap();
    let before = store.scratchpad_stack(config().id()).await.unwrap();
    store.shutdown().await.unwrap();
    let reopened = StoreHandle::open(&path).await.unwrap();
    assert_eq!(
        reopened.scratchpad_stack(config().id()).await.unwrap(),
        before
    );
    let fixture = SpaceFixture::start().await;
    let sync = start(&reopened, &fixture).await;
    until(|| fixture.body(first.path.as_str()).as_deref() == Some("café\r\n日本語 💾")).await;
    until(|| sync.snapshot().pending == 0).await;
    assert!(fixture.body(empty.path.as_str()).is_none());
    assert_eq!(
        reopened.scratchpad_stack(config().id()).await.unwrap(),
        before
    );
    sync.shutdown().await.unwrap();
    reopened.shutdown().await.unwrap();
}
