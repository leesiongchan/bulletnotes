#[path = "support/http_fixture.rs"]
mod http_fixture;
use bulletnotes::{
    domain::{connection::ConnectionConfig, page::*},
    store::StoreHandle,
    sync::{
        SilverBulletClient,
        probe::{new_probe_key, verify_write_guards},
        worker::SyncHandle,
    },
};
use http_fixture::{Fault, Fixture};
use std::time::Duration;
fn config() -> ConnectionConfig {
    ConnectionConfig::parse("https://notes.example/prefix/").unwrap()
}
fn key() -> PageKey {
    PageKey {
        connection: config().id().clone(),
        path: PagePath::parse("sample.md").unwrap(),
    }
}
fn client(f: &Fixture, timeout: Duration) -> SilverBulletClient {
    SilverBulletClient::for_loopback_fixture(&config(), f.base.clone(), None, timeout).unwrap()
}
async fn until(mut predicate: impl FnMut() -> bool) {
    tokio::time::timeout(Duration::from_secs(5), async {
        while !predicate() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
}
#[tokio::test]
async fn write_waits_for_local_commit_and_250ms_then_preserves_newer_edits() {
    let dir = tempfile::tempdir().unwrap();
    let store = StoreHandle::open(&dir.path().join("test.sqlite"))
        .await
        .unwrap();
    let f = Fixture::start(None).await;
    let client = client(&f, Duration::from_secs(2));
    let evidence = verify_write_guards(&client, new_probe_key(config().id().clone()))
        .await
        .unwrap();
    f.data.lock().unwrap().seen.clear();
    let sync = SyncHandle::start(store.clone(), client, Some(evidence)).unwrap();
    store.create_draft(key()).await.unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(!f.seen().iter().any(|r| r.method == reqwest::Method::PUT));
    store
        .save_local(key(), "revision one".into(), LocalRevision(1))
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(180)).await;
    assert!(!f.seen().iter().any(|r| r.method == reqwest::Method::PUT));
    f.fault(Fault::TimeoutAfterWrite);
    tokio::time::timeout(Duration::from_millis(400), async {
        until(|| f.body().as_deref() == Some(b"revision one")).await;
    })
    .await
    .expect("a quiet committed edit must upload after 250ms");
    store
        .save_local(key(), "revision two".into(), LocalRevision(2))
        .await
        .unwrap();
    until(|| f.body().as_deref() == Some(b"revision two")).await;
    until(|| sync.snapshot().pending == 0).await;
    let current = store.load_page(&key()).await.unwrap().unwrap();
    assert_eq!(current.body, "revision two");
    assert_eq!(current.remote_base.unwrap().body, "revision two");
    assert_eq!(
        f.seen()
            .iter()
            .filter(|r| r.method == reqwest::Method::PUT)
            .count(),
        2
    );
    sync.shutdown().await.unwrap();
    store.shutdown().await.unwrap();
}

#[tokio::test]
async fn continuous_local_commits_upload_within_one_second_without_waiting_for_quiet() {
    let dir = tempfile::tempdir().unwrap();
    let store = StoreHandle::open(&dir.path().join("test.sqlite"))
        .await
        .unwrap();
    let f = Fixture::start(None).await;
    let c = client(&f, Duration::from_secs(2));
    let evidence = verify_write_guards(&c, new_probe_key(config().id().clone()))
        .await
        .unwrap();
    f.data.lock().unwrap().seen.clear();
    store.create_draft(key()).await.unwrap();
    let sync = SyncHandle::start(store.clone(), c, Some(evidence)).unwrap();
    let started = tokio::time::Instant::now();
    for revision in 1..=8 {
        store
            .save_local(
                key(),
                format!("revision {revision}"),
                LocalRevision(revision),
            )
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_millis(150)).await;
        if f.body().is_some() {
            break;
        }
    }
    assert!(
        f.body().is_some(),
        "continuous saves starved the first upload"
    );
    assert!(started.elapsed() < Duration::from_millis(1250));
    assert_eq!(
        f.seen()
            .iter()
            .filter(|request| request.method == reqwest::Method::PUT)
            .count(),
        1
    );
    sync.shutdown().await.unwrap();
    store.shutdown().await.unwrap();
}

#[tokio::test]
async fn selected_cached_page_detects_remote_edits_without_a_catalogue_refresh() {
    let dir = tempfile::tempdir().unwrap();
    let store = StoreHandle::open(&dir.path().join("test.sqlite"))
        .await
        .unwrap();
    let f = Fixture::start(Some(b"original source")).await;
    let sync = SyncHandle::start(store.clone(), client(&f, Duration::from_secs(2)), None).unwrap();
    until(|| sync.snapshot().complete).await;
    sync.set_selected_page(Some(key()));
    tokio::time::sleep(Duration::from_millis(100)).await;
    {
        let mut data = f.data.lock().unwrap();
        data.seen.clear();
        data.body = Some(b"remote edit".to_vec());
        data.revision += 1;
    }
    let fresh = tokio::time::timeout(Duration::from_millis(1500), async {
        loop {
            if store.load_page(&key()).await.unwrap().unwrap().body == "remote edit" {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await;
    sync.shutdown().await.unwrap();
    store.shutdown().await.unwrap();
    assert!(
        fresh.is_ok(),
        "the selected cached page waited for the 30-second catalogue poll"
    );
    let requests = f.seen();
    assert!(
        requests
            .iter()
            .all(|request| request.uri.ends_with("/sample.md")),
        "foreground refresh listed the whole catalogue"
    );
    assert_eq!(
        requests
            .iter()
            .filter(|request| request.method == reqwest::Method::GET)
            .count(),
        1
    );
}

#[tokio::test]
async fn selected_page_idle_checks_use_head_and_stop_when_hidden_or_cleared() {
    let dir = tempfile::tempdir().unwrap();
    let store = StoreHandle::open(&dir.path().join("test.sqlite"))
        .await
        .unwrap();
    let f = Fixture::start(Some(b"unchanged source")).await;
    let sync = SyncHandle::start(store.clone(), client(&f, Duration::from_secs(2)), None).unwrap();
    until(|| sync.snapshot().complete).await;
    f.data.lock().unwrap().seen.clear();
    sync.set_selected_page(Some(key()));
    until(|| !f.seen().is_empty()).await;
    tokio::time::sleep(Duration::from_millis(2200)).await;
    let requests = f.seen();
    assert!(
        (2..=3).contains(&requests.len()),
        "idle checks were not bounded to approximately once per second: {}",
        requests.len()
    );
    assert!(
        requests
            .iter()
            .all(|request| request.method == reqwest::Method::HEAD
                && request.uri.ends_with("/sample.md"))
    );

    sync.set_active(false);
    tokio::time::sleep(Duration::from_millis(100)).await;
    f.data.lock().unwrap().seen.clear();
    tokio::time::sleep(Duration::from_millis(1100)).await;
    assert!(
        f.seen().is_empty(),
        "a hidden note kept its foreground heartbeat"
    );

    sync.set_active(true);
    until(|| !f.seen().is_empty()).await;
    tokio::time::sleep(Duration::from_millis(100)).await;
    sync.set_selected_page(None);
    tokio::time::sleep(Duration::from_millis(100)).await;
    f.data.lock().unwrap().seen.clear();
    tokio::time::sleep(Duration::from_millis(1100)).await;
    assert!(
        f.seen().is_empty(),
        "clearing selection kept the old heartbeat"
    );
    sync.shutdown().await.unwrap();
    store.shutdown().await.unwrap();
}

#[tokio::test]
async fn changing_selected_cached_page_immediately_checks_the_new_key() {
    let dir = tempfile::tempdir().unwrap();
    let store = StoreHandle::open(&dir.path().join("test.sqlite"))
        .await
        .unwrap();
    let f = Fixture::start(Some(b"cached source")).await;
    f.data.lock().unwrap().catalogue_override = Some(serde_json::json!([
        {"name":"sample.md"}, {"name":"other.md"}
    ]));
    let sync = SyncHandle::start(store.clone(), client(&f, Duration::from_secs(2)), None).unwrap();
    until(|| sync.snapshot().complete).await;
    sync.set_selected_page(Some(key()));
    tokio::time::sleep(Duration::from_millis(100)).await;
    f.data.lock().unwrap().seen.clear();
    let other = PageKey {
        connection: config().id().clone(),
        path: PagePath::parse("other.md").unwrap(),
    };
    sync.set_selected_page(Some(other));
    tokio::time::timeout(Duration::from_millis(300), until(|| !f.seen().is_empty()))
        .await
        .expect("switching a cached note must check it immediately");
    tokio::time::sleep(Duration::from_millis(1100)).await;
    let requests = f.seen();
    assert!((2..=3).contains(&requests.len()));
    assert!(
        requests
            .iter()
            .all(|request| request.method == reqwest::Method::HEAD
                && request.uri.ends_with("/other.md")),
        "selection switch retained the previous heartbeat or listed the catalogue"
    );
    sync.shutdown().await.unwrap();
    store.shutdown().await.unwrap();
}

#[tokio::test]
async fn selected_page_permission_failure_does_not_retry_on_each_heartbeat() {
    let dir = tempfile::tempdir().unwrap();
    let store = StoreHandle::open(&dir.path().join("test.sqlite"))
        .await
        .unwrap();
    let f = Fixture::start(Some(b"cached source")).await;
    let sync = SyncHandle::start(store.clone(), client(&f, Duration::from_secs(2)), None).unwrap();
    until(|| sync.snapshot().complete).await;
    f.data.lock().unwrap().seen.clear();
    f.fault(Fault::Status(403));
    sync.set_selected_page(Some(key()));
    until(|| sync.snapshot().page_errors.contains_key(&key())).await;
    tokio::time::sleep(Duration::from_millis(2200)).await;
    assert_eq!(
        f.seen().len(),
        1,
        "foreground checks bypassed the page error gate"
    );
    sync.shutdown().await.unwrap();
    store.shutdown().await.unwrap();
}

#[tokio::test]
async fn changing_selection_preserves_network_backoff() {
    let dir = tempfile::tempdir().unwrap();
    let store = StoreHandle::open(&dir.path().join("test.sqlite"))
        .await
        .unwrap();
    let f = Fixture::start(Some(b"cached source")).await;
    let sync = SyncHandle::start(store.clone(), client(&f, Duration::from_secs(2)), None).unwrap();
    until(|| sync.snapshot().complete).await;
    f.data.lock().unwrap().seen.clear();
    f.fault(Fault::Status(500));
    sync.set_selected_page(Some(key()));
    until(|| sync.snapshot().error.is_some()).await;
    for _ in 0..20 {
        sync.set_selected_page(None);
        sync.set_selected_page(Some(key()));
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    tokio::time::sleep(Duration::from_millis(1200)).await;
    assert!(
        (1..=3).contains(&f.seen().len()),
        "selection changes bypassed network backoff: {} requests",
        f.seen().len()
    );
    sync.shutdown().await.unwrap();
    store.shutdown().await.unwrap();
}

#[tokio::test]
async fn selected_cached_page_bypasses_the_remaining_background_downloads() {
    let dir = tempfile::tempdir().unwrap();
    let store = StoreHandle::open(&dir.path().join("test.sqlite"))
        .await
        .unwrap();
    let f = Fixture::start(Some(b"cached source")).await;
    let c = client(&f, Duration::from_secs(2));
    let selected = PageKey {
        connection: config().id().clone(),
        path: PagePath::parse("page-099.md").unwrap(),
    };
    store
        .apply_remote(c.read(&selected).await.unwrap())
        .await
        .unwrap();
    f.data.lock().unwrap().catalogue_override = Some(serde_json::Value::Array(
        (0..100)
            .map(|n| serde_json::json!({"name":format!("page-{n:03}.md")}))
            .collect(),
    ));
    f.fault(Fault::SlowReads);
    let sync = SyncHandle::start(store.clone(), c, None).unwrap();
    until(|| sync.snapshot().catalogue_total == 100).await;
    {
        let mut data = f.data.lock().unwrap();
        data.body = Some(b"new selected source".to_vec());
        data.revision += 1;
    }
    sync.set_selected_page(Some(selected.clone()));
    let fresh = tokio::time::timeout(Duration::from_millis(1500), async {
        loop {
            if store.load_page(&selected).await.unwrap().unwrap().body == "new selected source" {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await;
    assert!(
        fresh.is_ok(),
        "a selected cached page waited behind the entire background download queue"
    );
    assert!(!sync.snapshot().complete);
    sync.shutdown().await.unwrap();
    store.shutdown().await.unwrap();
}

#[tokio::test]
async fn selecting_a_local_only_draft_does_not_observe_a_remote_deletion() {
    let dir = tempfile::tempdir().unwrap();
    let store = StoreHandle::open(&dir.path().join("test.sqlite"))
        .await
        .unwrap();
    let f = Fixture::start(None).await;
    store.create_draft(key()).await.unwrap();
    store
        .save_local(key(), "unsent draft".into(), LocalRevision(1))
        .await
        .unwrap();
    let sync = SyncHandle::start(store.clone(), client(&f, Duration::from_secs(2)), None).unwrap();
    until(|| sync.snapshot().generation > 1).await;
    tokio::time::sleep(Duration::from_millis(100)).await;
    f.data.lock().unwrap().seen.clear();
    sync.set_selected_page(Some(key()));
    tokio::time::sleep(Duration::from_millis(1200)).await;
    assert!(
        f.seen().is_empty(),
        "an unsent local draft was read as a remote page"
    );
    assert_eq!(
        store.load_page(&key()).await.unwrap().unwrap().body,
        "unsent draft"
    );
    assert!(
        store
            .recovery_uploads(config().id(), 10)
            .await
            .unwrap()
            .is_empty()
    );
    sync.shutdown().await.unwrap();
    store.shutdown().await.unwrap();
}
#[tokio::test]
async fn timeout_after_success_reads_back_before_retrying_and_never_duplicates_create() {
    let dir = tempfile::tempdir().unwrap();
    let store = StoreHandle::open(&dir.path().join("test.sqlite"))
        .await
        .unwrap();
    let f = Fixture::start(None).await;
    let c = client(&f, Duration::from_millis(50));
    let evidence = verify_write_guards(&c, new_probe_key(config().id().clone()))
        .await
        .unwrap();
    f.data.lock().unwrap().seen.clear();
    f.fault(Fault::TimeoutAfterWrite);
    store.create_draft(key()).await.unwrap();
    store
        .save_local(key(), "applied despite timeout".into(), LocalRevision(1))
        .await
        .unwrap();
    let sync = SyncHandle::start(store.clone(), c, Some(evidence)).unwrap();
    until(|| f.body().is_some()).await;
    until(|| sync.snapshot().pending == 0).await;
    let requests = f.seen();
    let write = requests
        .iter()
        .position(|r| r.method == reqwest::Method::PUT)
        .unwrap();
    assert!(
        requests[write + 1..]
            .iter()
            .any(|r| r.method == reqwest::Method::GET)
    );
    assert_eq!(
        requests
            .iter()
            .filter(|r| r.method == reqwest::Method::PUT)
            .count(),
        1
    );
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
#[tokio::test]
async fn absent_evidence_caches_pages_but_never_mutates() {
    let dir = tempfile::tempdir().unwrap();
    let store = StoreHandle::open(&dir.path().join("test.sqlite"))
        .await
        .unwrap();
    let f = Fixture::start(Some(b"remote source")).await;
    let sync = SyncHandle::start(store.clone(), client(&f, Duration::from_secs(2)), None).unwrap();
    until(|| sync.snapshot().indexed == 1).await;
    assert_eq!(
        store.load_page(&key()).await.unwrap().unwrap().body,
        "remote source"
    );
    store
        .save_local(key(), "retained local edit".into(), LocalRevision(1))
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(900)).await;
    assert!(
        !f.seen()
            .iter()
            .any(|r| r.method == reqwest::Method::PUT || r.method == reqwest::Method::DELETE)
    );
    assert!(!sync.snapshot().writable);
    sync.shutdown().await.unwrap();
    store.shutdown().await.unwrap();
}

#[tokio::test]
async fn shutdown_cancels_network_without_losing_the_unknown_payload() {
    let dir = tempfile::tempdir().unwrap();
    let store = StoreHandle::open(&dir.path().join("test.sqlite"))
        .await
        .unwrap();
    let f = Fixture::start(None).await;
    let c = client(&f, Duration::from_secs(5));
    let evidence = verify_write_guards(&c, new_probe_key(config().id().clone()))
        .await
        .unwrap();
    f.data.lock().unwrap().seen.clear();
    f.fault(Fault::TimeoutAfterWrite);
    store.create_draft(key()).await.unwrap();
    store
        .save_local(key(), "saved before shutdown".into(), LocalRevision(1))
        .await
        .unwrap();
    let sync = SyncHandle::start(store.clone(), c.clone(), Some(evidence.clone())).unwrap();
    until(|| f.body().is_some()).await;
    tokio::time::timeout(Duration::from_millis(150), sync.shutdown())
        .await
        .unwrap()
        .unwrap();
    let unknown = store.unknown_mutations(config().id()).await.unwrap();
    assert_eq!(unknown.len(), 1);
    assert_eq!(
        unknown[0].kind,
        MutationKind::Put("saved before shutdown".into())
    );
    f.fault(Fault::None);
    let restarted = SyncHandle::start(store.clone(), c, Some(evidence)).unwrap();
    until(|| restarted.snapshot().indexed == 1 && restarted.snapshot().pending == 0).await;
    assert_eq!(
        f.seen()
            .iter()
            .filter(|r| r.method == reqwest::Method::PUT)
            .count(),
        1
    );
    restarted.shutdown().await.unwrap();
    store.shutdown().await.unwrap();
}

#[tokio::test]
async fn full_catalogue_downloads_are_bounded_and_a_failed_refresh_is_not_deletion() {
    use bulletnotes::store::search::SearchQuery;
    let dir = tempfile::tempdir().unwrap();
    let store = StoreHandle::open(&dir.path().join("test.sqlite"))
        .await
        .unwrap();
    let f = Fixture::start(Some(b"generated source")).await;
    let entries: Vec<_> = (0..12)
        .map(|n| serde_json::json!({"name":format!("page-{n}.md"),"size":16,"lastModified":100}))
        .collect();
    f.data.lock().unwrap().catalogue_override = Some(serde_json::Value::Array(entries));
    f.fault(Fault::SlowReads);
    let sync = SyncHandle::start(store.clone(), client(&f, Duration::from_secs(2)), None).unwrap();
    until(|| sync.snapshot().catalogue_total == 12).await;
    assert!(!sync.snapshot().complete);
    until(|| sync.snapshot().indexed == 12).await;
    let maximum = f.data.lock().unwrap().max_active_reads;
    assert!(maximum <= 4);
    assert!(maximum >= 2);
    f.fault(Fault::Status(500));
    sync.refresh();
    until(|| sync.snapshot().error.is_some()).await;
    let results = store
        .search(SearchQuery {
            scope: bulletnotes::store::search::SearchScope::AllPages,
            connection: config().id().clone(),
            text: String::new(),
            folder: None,
            offset: 0,
            limit: 100,
        })
        .await
        .unwrap();
    assert_eq!(results.total, 12);
    assert_eq!(results.indexed, 12);
    sync.shutdown().await.unwrap();
    store.shutdown().await.unwrap();
}
#[tokio::test]
async fn an_unrelated_page_can_use_a_free_lane_while_another_read_is_stalled() {
    use std::hash::{Hash, Hasher};
    let bucket = |key: &PageKey| {
        let mut hash = std::hash::DefaultHasher::new();
        key.hash(&mut hash);
        hash.finish() % 4
    };
    let blocked = PageKey {
        connection: config().id().clone(),
        path: PagePath::parse("blocked.md").unwrap(),
    };
    // Adversarial names expose a fixed hash partition's unnecessary head-of-line blocking.
    let peer = (0..100)
        .map(|n| PageKey {
            connection: config().id().clone(),
            path: PagePath::parse(&format!("peer-{n}.md")).unwrap(),
        })
        .find(|key| bucket(key) == bucket(&blocked))
        .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let store = StoreHandle::open(&dir.path().join("test.sqlite"))
        .await
        .unwrap();
    let f = Fixture::start(Some(b"generated source")).await;
    f.data.lock().unwrap().catalogue_override =
        Some(serde_json::json!([{"name":blocked.path.as_str()},{"name":peer.path.as_str()}]));
    f.fault(Fault::BlockOneRead);
    let sync = SyncHandle::start(store.clone(), client(&f, Duration::from_secs(5)), None).unwrap();
    let ready = tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            if store.load_page(&peer).await.unwrap().is_some() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await;
    f.data.lock().unwrap().release.notify_one();
    sync.shutdown().await.unwrap();
    store.shutdown().await.unwrap();
    assert!(
        ready.is_ok(),
        "an idle lane must serve the unrelated page without waiting for the stalled read"
    );
}

#[tokio::test]
async fn opening_an_uncached_page_prioritizes_it_over_background_downloads() {
    let dir = tempfile::tempdir().unwrap();
    let store = StoreHandle::open(&dir.path().join("test.sqlite"))
        .await
        .unwrap();
    let f = Fixture::start(Some(b"generated source")).await;
    f.data.lock().unwrap().catalogue_override = Some(serde_json::Value::Array(
        (0..100)
            .map(|n| serde_json::json!({"name":format!("page-{n:03}.md")}))
            .collect(),
    ));
    f.fault(Fault::SlowReads);
    let sync = SyncHandle::start(store.clone(), client(&f, Duration::from_secs(2)), None).unwrap();
    until(|| sync.snapshot().catalogue_total == 100).await;
    let selected = PageKey {
        connection: config().id().clone(),
        path: PagePath::parse("page-099.md").unwrap(),
    };
    sync.request_page(selected.clone());
    let result = tokio::time::timeout(Duration::from_millis(1500), async {
        loop {
            if store.load_page(&selected).await.unwrap().is_some() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await;
    sync.shutdown().await.unwrap();
    store.shutdown().await.unwrap();
    assert!(
        result.is_ok(),
        "selected page waited behind the entire space"
    );
}

#[tokio::test]
async fn a_reconnected_client_cannot_reuse_old_write_evidence() {
    let dir = tempfile::tempdir().unwrap();
    let store = StoreHandle::open(&dir.path().join("test.sqlite"))
        .await
        .unwrap();
    let f = Fixture::start(None).await;
    let previous = client(&f, Duration::from_secs(2));
    let evidence = verify_write_guards(&previous, new_probe_key(config().id().clone()))
        .await
        .unwrap();
    {
        let mut data = f.data.lock().unwrap();
        data.seen.clear();
        data.body = Some(b"remote source".to_vec());
    }
    let sync = SyncHandle::start(
        store.clone(),
        client(&f, Duration::from_secs(2)),
        Some(evidence),
    )
    .unwrap();
    until(|| sync.snapshot().indexed == 1).await;
    store
        .save_local(key(), "local capture".into(), LocalRevision(1))
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(900)).await;
    assert!(!sync.snapshot().writable);
    assert!(!f.seen().iter().any(|r| r.method == reqwest::Method::PUT));
    sync.shutdown().await.unwrap();
    store.shutdown().await.unwrap();
}

#[tokio::test]
async fn permission_and_resource_failures_pause_catalogue_retries() {
    use bulletnotes::domain::error::SyncError;
    for oversized in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let store = StoreHandle::open(&dir.path().join("test.sqlite"))
            .await
            .unwrap();
        let f = Fixture::start(None).await;
        if oversized {
            // Build the payload outside the request deadline: this test checks
            // resource-limit pausing, not debug JSON serialization throughput.
            f.fault(Fault::RawCatalogue(vec![b' '; 16 * 1024 * 1024 + 1].into()));
        } else {
            f.fault(Fault::Status(403));
        }
        let sync =
            SyncHandle::start(store.clone(), client(&f, Duration::from_secs(2)), None).unwrap();
        let expected = if oversized {
            SyncError::ResponseTooLarge
        } else {
            SyncError::Permission
        };
        let reached = tokio::time::timeout(Duration::from_secs(5), async {
            while sync.snapshot().error != Some(expected) {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await;
        assert!(
            reached.is_ok(),
            "oversized={oversized}, expected={expected:?}, actual={:?}, requests={}",
            sync.snapshot().error,
            f.seen().len()
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
        let requests = f.seen().len();
        sync.shutdown().await.unwrap();
        store.shutdown().await.unwrap();
        assert_eq!(
            requests, 1,
            "a non-transient catalogue failure must wait for explicit resolution"
        );
    }
}

#[tokio::test]
async fn verified_worker_rechecks_historical_probe_watches_but_read_only_worker_never_deletes() {
    let dir = tempfile::tempdir().unwrap();
    let store = StoreHandle::open(&dir.path().join("test.sqlite"))
        .await
        .unwrap();
    let f = Fixture::start(None).await;
    let c = client(&f, Duration::from_secs(2));
    let watched = new_probe_key(config().id().clone());
    let evidence = verify_write_guards(&c, watched.clone()).await.unwrap();
    let late_body = f
        .seen()
        .into_iter()
        .find(|r| r.method == reqwest::Method::PUT)
        .unwrap()
        .body;
    store.register_probe(&watched).await.unwrap();
    {
        let mut data = f.data.lock().unwrap();
        data.body = Some(late_body);
        data.revision += 1;
        data.seen.clear();
    }
    let readonly = SyncHandle::start(store.clone(), c.clone(), None).unwrap();
    until(|| readonly.snapshot().complete).await;
    assert!(!f.seen().iter().any(|r| r.method == reqwest::Method::DELETE));
    assert!(f.body().is_some());
    readonly.shutdown().await.unwrap();
    let writable = SyncHandle::start(store.clone(), c, Some(evidence)).unwrap();
    until(|| f.body().is_none()).await;
    let deletes: Vec<_> = f
        .seen()
        .into_iter()
        .filter(|r| r.method == reqwest::Method::DELETE)
        .collect();
    assert_eq!(deletes.len(), 1);
    assert!(deletes[0].headers.contains_key("if-match"));
    assert!(
        deletes[0]
            .uri
            .ends_with(watched.path.as_str().rsplit('/').next().unwrap())
    );
    assert_eq!(
        store.pending_probes(config().id()).await.unwrap(),
        vec![watched]
    );
    writable.shutdown().await.unwrap();
    store.shutdown().await.unwrap();
}
