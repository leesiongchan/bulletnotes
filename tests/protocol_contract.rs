#[path = "support/http_fixture.rs"]
mod http_fixture;
use bulletnotes::{
    domain::{connection::ConnectionConfig, error::SyncError, page::*},
    sync::{SecretToken, SilverBulletClient},
};
use http_fixture::{Fault, Fixture};
use std::time::Duration;
fn config() -> ConnectionConfig {
    ConnectionConfig::parse("https://notes.example/prefix/").unwrap()
}
fn key(path: &str) -> PageKey {
    PageKey {
        connection: config().id().clone(),
        path: PagePath::parse(path).unwrap(),
    }
}
fn client(fixture: &Fixture) -> SilverBulletClient {
    SilverBulletClient::for_loopback_fixture(
        &config(),
        fixture.base.clone(),
        None,
        Duration::from_secs(2),
    )
    .unwrap()
}
fn mutation(path: &str, body: &str, condition: WriteCondition) -> PendingMutation {
    PendingMutation {
        id: OperationId::new(),
        key: key(path),
        local_revision: LocalRevision(1),
        kind: MutationKind::Put(body.into()),
        condition,
    }
}
#[tokio::test]
async fn create_update_delete_are_guarded_and_preserve_source_and_encoded_path() {
    let fixture = Fixture::start(None).await;
    let client = client(&fixture);
    let source = "---\r\ntitle: café\r\n---\r\n\t👩🏽‍💻 日本語\r\n";
    let mut operation = mutation(
        "Projects/50% café #1.md",
        source,
        WriteCondition::CreateOnly,
    );
    let ack = client.put(&operation).await.unwrap();
    assert_eq!(
        ack.remote_revision,
        Some(RemoteRevision("\"fixture-2\"".into()))
    );
    let RemoteRead::Present(remote) = client.read(&operation.key).await.unwrap() else {
        panic!("missing created file")
    };
    assert_eq!(remote.body.as_bytes(), source.as_bytes());
    assert_eq!(remote.permission, Permission::ReadWrite);
    assert_eq!(
        fixture.seen()[0].uri,
        "/prefix/.fs/Projects/50%25%20caf%C3%A9%20%231.md"
    );
    assert_eq!(fixture.seen()[0].headers["if-none-match"], "*");
    assert_eq!(fixture.seen()[0].method, reqwest::Method::PUT);
    operation.condition = WriteCondition::Match(ack.remote_revision.unwrap());
    operation.kind = MutationKind::Put(format!("{source}next\r\n"));
    let ack = client.put(&operation).await.unwrap();
    let requests = fixture.seen();
    assert_eq!(requests[2].headers["if-match"], "\"fixture-2\"");
    assert!(requests.iter().all(|r| r.headers["x-sync-mode"] == "true"));
    assert_eq!(requests[0].body, source.as_bytes());
    operation.kind = MutationKind::Delete;
    operation.condition = WriteCondition::Match(ack.remote_revision.unwrap());
    client.delete(&operation).await.unwrap();
    assert_eq!(
        client.read(&operation.key).await.unwrap(),
        RemoteRead::Missing(operation.key)
    );
}
#[tokio::test]
async fn stale_write_and_missing_auth_or_permission_are_classified() {
    let fixture = Fixture::start(Some(b"remote")).await;
    let client = client(&fixture);
    let operation = mutation(
        "sample.md",
        "local",
        WriteCondition::Match(RemoteRevision("\"stale\"".into())),
    );
    assert_eq!(
        client.put(&operation).await.unwrap_err(),
        SyncError::Conflict
    );
    assert_eq!(fixture.body().unwrap(), b"remote");
    for (status, expected) in [
        (401, SyncError::Authentication),
        (403, SyncError::Permission),
        (404, SyncError::Missing),
        (500, SyncError::UnknownMutationOutcome),
    ] {
        fixture.fault(Fault::Status(status));
        assert_eq!(client.put(&operation).await.unwrap_err(), expected);
    }
}
#[tokio::test]
async fn successful_looking_html_and_invalid_utf8_are_never_notes() {
    let fixture = Fixture::start(Some(b"content")).await;
    let client = client(&fixture);
    for fault in [Fault::Html, Fault::InvalidUtf8] {
        fixture.fault(fault);
        assert_eq!(
            client.read(&key("sample.md")).await.unwrap_err(),
            SyncError::InvalidResponse
        );
    }
}
#[tokio::test]
async fn missing_permission_or_revision_is_read_only_and_proxy_etags_do_not_replace_content_revisions()
 {
    let fixture = Fixture::start(Some(b"content")).await;
    let client = client(&fixture);
    for fault in [
        Fault::MissingPermission,
        Fault::MissingRevision,
        Fault::InvalidDedicated,
    ] {
        fixture.fault(fault);
        let RemoteRead::Present(file) = client.read(&key("sample.md")).await.unwrap() else {
            panic!("missing")
        };
        assert_eq!(file.permission, Permission::ReadOnly);
    }
    fixture.fault(Fault::ProxyRevision);
    let RemoteRead::Present(file) = client.read(&key("sample.md")).await.unwrap() else {
        panic!("missing")
    };
    assert_eq!(file.permission, Permission::ReadWrite);
    assert_eq!(file.revision, Some(RemoteRevision("\"fixture-1\"".into())));
}
#[tokio::test]
async fn catalogue_validates_paths_metadata_and_duplicate_identity() {
    let fixture = Fixture::start(Some(b"content")).await;
    let client = client(&fixture);
    let list = client.list_files().await.unwrap();
    assert_eq!(list[0].key, key("sample.md"));
    assert_eq!(list[0].content_length, Some(7));
    for value in [
        serde_json::json!([{"name":"../escape.md"}]),
        serde_json::json!([{"name":"a.md","size":-1}]),
        serde_json::json!([{"name":"a.md"},{"name":"a.md"}]),
        serde_json::json!({"login":true}),
    ] {
        fixture.fault(Fault::Catalogue(value));
        assert_eq!(
            client.list_files().await.unwrap_err(),
            SyncError::InvalidResponse
        );
    }
    fixture.fault(Fault::Catalogue(
        serde_json::json!([{"name":"Projects/50% café #1.md"}]),
    ));
    assert_eq!(
        client.list_files().await.unwrap()[0].permission,
        Permission::ReadOnly
    );
}
#[tokio::test]
async fn redirect_never_reaches_the_other_origin_or_discloses_a_token() {
    let fixture = Fixture::start(Some(b"content")).await;
    let other = Fixture::start(None).await;
    fixture.fault(Fault::Redirect(other.base.to_string()));
    let token = SecretToken::new("generated-sensitive-token".into()).unwrap();
    assert!(!format!("{token:?}").contains("generated-sensitive-token"));
    let client = SilverBulletClient::for_loopback_fixture(
        &config(),
        fixture.base.clone(),
        Some(token),
        Duration::from_secs(2),
    )
    .unwrap();
    let error = client.read(&key("sample.md")).await.unwrap_err();
    assert_eq!(error, SyncError::Incompatible);
    assert!(other.seen().is_empty());
    assert!(!format!("{error:?} {error}").contains("sample.md"));
}
#[tokio::test]
async fn timeout_after_a_write_is_unknown_and_transport_does_not_retry() {
    let fixture = Fixture::start(None).await;
    fixture.fault(Fault::TimeoutAfterWrite);
    let client = SilverBulletClient::for_loopback_fixture(
        &config(),
        fixture.base.clone(),
        None,
        Duration::from_millis(50),
    )
    .unwrap();
    let operation = mutation(
        "sample.md",
        "generated pending body",
        WriteCondition::CreateOnly,
    );
    assert_eq!(
        client.put(&operation).await.unwrap_err(),
        SyncError::UnknownMutationOutcome
    );
    assert_eq!(fixture.body().unwrap(), b"generated pending body");
    assert_eq!(fixture.seen().len(), 1);
}
#[tokio::test]
async fn invalid_write_conditions_and_other_spaces_never_send_requests() {
    let fixture = Fixture::start(None).await;
    let client = client(&fixture);
    let mut operation = mutation(
        "sample.md",
        "body",
        WriteCondition::Match(RemoteRevision("*".into())),
    );
    assert_eq!(
        client.put(&operation).await.unwrap_err(),
        SyncError::Incompatible
    );
    operation.condition = WriteCondition::CreateOnly;
    operation.kind = MutationKind::Delete;
    assert_eq!(
        client.delete(&operation).await.unwrap_err(),
        SyncError::Incompatible
    );
    let mut foreign = key("sample.md");
    foreign.connection = ConnectionConfig::parse("https://other.example")
        .unwrap()
        .id()
        .clone();
    assert_eq!(
        client.read(&foreign).await.unwrap_err(),
        SyncError::Incompatible
    );
    assert!(fixture.seen().is_empty());
}
#[test]
fn insecure_non_loopback_transports_and_header_injection_are_rejected() {
    assert!(ConnectionConfig::parse("http://notes.example").is_err());
    assert!(SecretToken::new("token\r\nHeader: value".into()).is_err());
    assert!(
        SilverBulletClient::for_loopback_fixture(
            &config(),
            url::Url::parse("http://example.com").unwrap(),
            None,
            Duration::from_secs(2)
        )
        .is_err()
    );
    for base in [
        "https://notes.example:443/prefix/",
        "https://notes.example:8443/prefix/",
    ] {
        assert!(SilverBulletClient::new(&ConnectionConfig::parse(base).unwrap(), None).is_ok());
    }
}

#[tokio::test]
async fn unexpected_content_encoding_cannot_be_installed_as_literal_markdown() {
    let fixture = Fixture::start(Some(b"ASCII bytes pretending to be compressed")).await;
    fixture.fault(Fault::Encoded);
    assert_eq!(
        client(&fixture).read(&key("sample.md")).await.unwrap_err(),
        SyncError::InvalidResponse
    );
}

#[tokio::test]
async fn html_login_cannot_acknowledge_a_delete() {
    let fixture = Fixture::start(Some(b"preserve this page")).await;
    fixture.fault(Fault::Html);
    let mut operation = mutation(
        "sample.md",
        "",
        WriteCondition::Match(RemoteRevision("\"fixture-1\"".into())),
    );
    operation.kind = MutationKind::Delete;
    assert_eq!(
        client(&fixture).delete(&operation).await.unwrap_err(),
        SyncError::UnknownMutationOutcome
    );
    assert_eq!(fixture.body().unwrap(), b"preserve this page");
}

#[tokio::test]
async fn cached_reads_reuse_only_the_matching_validated_content_revision() {
    let fixture = Fixture::start(Some(b"cached source")).await;
    let client = client(&fixture);
    let RemoteRead::Present(known) = client.read(&key("sample.md")).await.unwrap() else {
        panic!("missing")
    };
    let unchanged = client.read_if_changed(&known).await.unwrap();
    assert_eq!(unchanged, RemoteRead::Present(known.clone()));
    assert_eq!(fixture.seen()[1].method, reqwest::Method::HEAD);
    {
        let mut data = fixture.data.lock().unwrap();
        data.body = Some(b"new source".to_vec());
        data.revision += 1;
    }
    let RemoteRead::Present(changed) = client.read_if_changed(&known).await.unwrap() else {
        panic!("missing")
    };
    assert_eq!(changed.body, "new source");
    fixture.fault(Fault::Status(304));
    assert_eq!(
        client.read_if_changed(&known).await.unwrap_err(),
        SyncError::InvalidResponse
    );
    assert_eq!(
        client.read(&key("sample.md")).await.unwrap_err(),
        SyncError::InvalidResponse
    );
}

#[tokio::test]
async fn status_200_proxy_bodies_never_acknowledge_a_write() {
    for fault in [Fault::HtmlCaps, Fault::FakeAck] {
        let fixture = Fixture::start(None).await;
        fixture.fault(fault);
        let operation = mutation("sample.md", "preserve locally", WriteCondition::CreateOnly);
        assert_eq!(
            client(&fixture).put(&operation).await.unwrap_err(),
            SyncError::UnknownMutationOutcome
        );
        assert!(fixture.body().is_none());
    }
}
