#[path = "support/http_fixture.rs"]
mod http_fixture;
use bulletnotes::{
    app::connection::ConnectionController,
    domain::{
        connection::{ConnectionConfig, ConnectionId},
        error::CredentialError,
        page::*,
    },
    platform::credentials::{CredentialBackend, CredentialStore},
    store::StoreHandle,
    sync::{SecretToken, SilverBulletClient},
};
use http_fixture::{Fault, Fixture};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
#[derive(Default)]
struct Denied(Mutex<Vec<ConnectionId>>);
impl CredentialBackend for Denied {
    fn load(&self, id: &ConnectionId) -> Result<Option<SecretToken>, CredentialError> {
        self.0.lock().unwrap().push(id.clone());
        Err(CredentialError::Denied)
    }
    fn save(&self, id: &ConnectionId, _token: &SecretToken) -> Result<(), CredentialError> {
        self.0.lock().unwrap().push(id.clone());
        Err(CredentialError::Denied)
    }
    fn delete(&self, id: &ConnectionId) -> Result<(), CredentialError> {
        self.0.lock().unwrap().push(id.clone());
        Err(CredentialError::Denied)
    }
}
fn config() -> ConnectionConfig {
    ConnectionConfig::parse("https://notes.example/prefix/").unwrap()
}
fn client(f: &Fixture, token: Option<SecretToken>, timeout: Duration) -> SilverBulletClient {
    SilverBulletClient::for_loopback_fixture(&config(), f.base.clone(), token, timeout).unwrap()
}
#[tokio::test]
async fn keychain_denial_keeps_the_authenticated_session_usable() {
    let dir = tempfile::tempdir().unwrap();
    let store = StoreHandle::open(&dir.path().join("test.sqlite"))
        .await
        .unwrap();
    let denied = Arc::new(Denied::default());
    let controller = ConnectionController::new(store.clone(), CredentialStore::new(denied.clone()));
    let f = Fixture::start(None).await;
    let token = SecretToken::new("generated-sensitive-token".into()).unwrap();
    let c = client(&f, Some(token.clone()), Duration::from_secs(2));
    let result = controller
        .connect_fixture(c, Some(token), true)
        .await
        .unwrap();
    assert!(result.readable);
    assert!(result.write_capability.is_ok());
    assert_eq!(result.credential_warning, Some(CredentialError::Denied));
    assert!(
        f.seen()
            .iter()
            .all(|r| r.headers["authorization"] == "Bearer generated-sensitive-token")
    );
    assert_eq!(*denied.0.lock().unwrap(), vec![config().id().clone()]);
    assert!(
        store
            .pending_probes(config().id())
            .await
            .unwrap()
            .is_empty()
    );
    store.shutdown().await.unwrap();
    let bytes = std::fs::read(dir.path().join("test.sqlite")).unwrap();
    assert!(
        !bytes
            .windows(b"generated-sensitive-token".len())
            .any(|w| w == b"generated-sensitive-token")
    );
}
#[tokio::test]
async fn timed_out_probe_survives_restart_and_cleanup_is_exact_and_guarded() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.sqlite");
    let store = StoreHandle::open(&path).await.unwrap();
    let controller = ConnectionController::new(
        store.clone(),
        CredentialStore::new(Arc::new(Denied::default())),
    );
    let f = Fixture::start(None).await;
    f.fault(Fault::TimeoutAfterWrite);
    let c = client(&f, None, Duration::from_millis(50));
    let outcome = controller.connect_fixture(c, None, false).await.unwrap();
    assert!(outcome.write_capability.is_err());
    let pending = store.pending_probes(config().id()).await.unwrap();
    assert_eq!(pending.len(), 1);
    assert!(f.body().is_some());
    store.shutdown().await.unwrap();
    drop(controller);
    let store = StoreHandle::open(&path).await.unwrap();
    let controller = ConnectionController::new(
        store.clone(),
        CredentialStore::new(Arc::new(Denied::default())),
    );
    assert_eq!(store.pending_probes(config().id()).await.unwrap(), pending);
    f.fault(Fault::None);
    let c = client(&f, None, Duration::from_secs(2));
    controller.cleanup_pending(&c).await.unwrap();
    assert!(f.body().is_none());
    assert!(
        store
            .pending_probes(config().id())
            .await
            .unwrap()
            .is_empty()
    );
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
            .ends_with(pending[0].path.as_str().rsplit('/').next().unwrap())
    );
    store.shutdown().await.unwrap();
}
#[tokio::test]
async fn switched_connection_does_not_claim_another_spaces_outbox_or_cleanup() {
    let dir = tempfile::tempdir().unwrap();
    let store = StoreHandle::open(&dir.path().join("test.sqlite"))
        .await
        .unwrap();
    let a = config().id().clone();
    let b = ConnectionConfig::parse("https://other.example/").unwrap();
    let key = PageKey {
        connection: a.clone(),
        path: PagePath::parse("local.md").unwrap(),
    };
    store.create_draft(key.clone()).await.unwrap();
    store
        .save_local(key.clone(), "retain locally".into(), LocalRevision(1))
        .await
        .unwrap();
    assert!(store.claim_pending(b.id(), 8).await.unwrap().is_empty());
    assert_eq!(store.claim_pending(&a, 8).await.unwrap()[0].key, key);
    store.shutdown().await.unwrap();
}

#[tokio::test]
async fn cleanup_leaves_replacement_content_at_a_probe_path_intact() {
    let dir = tempfile::tempdir().unwrap();
    let store = StoreHandle::open(&dir.path().join("test.sqlite"))
        .await
        .unwrap();
    let controller = ConnectionController::new(
        store.clone(),
        CredentialStore::new(Arc::new(Denied::default())),
    );
    let f = Fixture::start(None).await;
    f.fault(Fault::TimeoutAfterWrite);
    controller
        .connect_fixture(client(&f, None, Duration::from_millis(50)), None, false)
        .await
        .unwrap();
    f.fault(Fault::None);
    f.data.lock().unwrap().body = Some(b"replacement belongs to the user".to_vec());
    assert!(
        controller
            .cleanup_pending(&client(&f, None, Duration::from_secs(2)))
            .await
            .is_err()
    );
    assert_eq!(f.body().unwrap(), b"replacement belongs to the user");
    assert!(!f.seen().iter().any(|r| r.method == reqwest::Method::DELETE));
    assert_eq!(store.pending_probes(config().id()).await.unwrap().len(), 1);
    store.shutdown().await.unwrap();
}
#[tokio::test]
async fn redirected_connection_never_probes_or_discloses_a_token() {
    let dir = tempfile::tempdir().unwrap();
    let store = StoreHandle::open(&dir.path().join("test.sqlite"))
        .await
        .unwrap();
    let controller = ConnectionController::new(
        store.clone(),
        CredentialStore::new(Arc::new(Denied::default())),
    );
    let f = Fixture::start(None).await;
    let other = Fixture::start(None).await;
    f.fault(Fault::Redirect(other.base.to_string()));
    let token = SecretToken::new("generated-sensitive-token".into()).unwrap();
    assert!(
        controller
            .connect_fixture(
                client(&f, Some(token.clone()), Duration::from_secs(2)),
                Some(token),
                false
            )
            .await
            .is_err()
    );
    assert!(other.seen().is_empty());
    assert!(
        store
            .pending_probes(config().id())
            .await
            .unwrap()
            .is_empty()
    );
    store.shutdown().await.unwrap();
}
#[tokio::test]
#[ignore = "explicit external server validation; requires BULLETNOTES_TEST_ENDPOINT and BULLETNOTES_TEST_JOURNAL; generated probe only"]
async fn official_or_deployed_server_verifies_and_cleans_an_owned_page() {
    let endpoint = std::env::var("BULLETNOTES_TEST_ENDPOINT").unwrap();
    let path = std::env::var("BULLETNOTES_TEST_JOURNAL").unwrap();
    let url = url::Url::parse(&endpoint).unwrap();
    let config = if url.scheme() == "http" {
        ConnectionConfig::parse("https://isolated-server.example/").unwrap()
    } else {
        ConnectionConfig::parse(&endpoint).unwrap()
    };
    let store = StoreHandle::open(std::path::Path::new(&path))
        .await
        .unwrap();
    let controller = ConnectionController::new(
        store.clone(),
        CredentialStore::new(Arc::new(Denied::default())),
    );
    let result = if url.scheme() == "http" {
        let c =
            SilverBulletClient::for_loopback_fixture(&config, url, None, Duration::from_secs(30))
                .unwrap();
        controller.connect_fixture(c, None, false).await
    } else {
        controller.connect(config.clone(), None, false).await
    };
    let result = result.unwrap();
    assert!(result.readable);
    assert!(
        result.write_capability.is_ok(),
        "{:?}",
        result.write_capability.err()
    );
    assert!(!result.cleanup_pending);
    assert!(store.pending_probes(config.id()).await.unwrap().is_empty());
    store.shutdown().await.unwrap();
}
#[cfg(target_os = "macos")]
#[tokio::test]
#[ignore = "requires explicit developer approval to access the real Keychain"]
async fn real_keychain_round_trip_for_an_isolated_account() {
    let id = ConnectionConfig::parse(&format!("https://{}.invalid/", uuid::Uuid::new_v4()))
        .unwrap()
        .id()
        .clone();
    let credentials = CredentialStore::keychain();
    assert!(credentials.load(&id).await.unwrap().is_none());
    let token = SecretToken::new(uuid::Uuid::new_v4().to_string()).unwrap();
    credentials.save(&id, &token).await.unwrap();
    let loaded = credentials.load(&id).await.unwrap();
    credentials.delete(&id).await.unwrap();
    assert!(loaded.is_some());
    assert!(credentials.load(&id).await.unwrap().is_none());
}

#[test]
fn connection_form_does_not_embed_credentials_or_accept_insecure_urls() {
    use bulletnotes::app::connection::ConnectionInput;
    assert!(ConnectionInput::parse("http://notes.example/", "", false).is_err());
    assert!(ConnectionInput::parse("https://notes.example/", "bad\r\nheader", true).is_err());
    let input = ConnectionInput::parse(
        "https://notes.example/prefix",
        "generated-sensitive-token",
        true,
    )
    .unwrap();
    assert_eq!(input.config.id().as_str(), "https://notes.example/prefix/");
    assert!(!format!("{input:?}").contains("generated-sensitive-token"));
}

#[tokio::test]
async fn probe_journal_migration_preserves_existing_source_and_pending_writes() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("previous.sqlite");
    let id = config().id().clone();
    let operation = uuid::Uuid::new_v4();
    let source = "original source\r\n日本語 👩🏽‍💻\r\n";
    {
        let db = rusqlite::Connection::open(&path).unwrap();
        db.execute_batch(include_str!("../migrations/001_initial.sql"))
            .unwrap();
        db.pragma_update(None, "user_version", 1).unwrap();
        db.execute("INSERT INTO pages(id,connection,path,title,body,local_revision) VALUES(1,?1,'retained.md','Retained',?2,'1')",rusqlite::params![id.as_str(),source]).unwrap();
        db.execute("INSERT INTO outbox(operation_id,page_id,local_revision,kind,body,condition_kind,state) VALUES(?1,1,'1','put',?2,'create','pending')",rusqlite::params![operation.to_string(),source]).unwrap();
    }
    let store = StoreHandle::open(&path).await.unwrap();
    let key = PageKey {
        connection: id.clone(),
        path: PagePath::parse("retained.md").unwrap(),
    };
    assert_eq!(store.load_page(&key).await.unwrap().unwrap().body, source);
    let queued = store.claim_pending(&id, 1).await.unwrap();
    assert_eq!(queued[0].id, OperationId(operation));
    assert_eq!(queued[0].kind, MutationKind::Put(source.into()));
    let probe = bulletnotes::sync::probe::new_probe_key(id.clone());
    store.register_probe(&probe).await.unwrap();
    assert_eq!(store.pending_probes(&id).await.unwrap(), vec![probe]);
    store.shutdown().await.unwrap();
    assert!(std::fs::read_dir(dir.path()).unwrap().any(|entry| {
        entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .contains("pre-migration-")
    }));
}
