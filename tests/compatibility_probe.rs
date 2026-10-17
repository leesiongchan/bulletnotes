#[path = "support/http_fixture.rs"]
mod http_fixture;
use bulletnotes::{
    domain::{connection::ConnectionConfig, error::SyncError, page::*},
    sync::{
        SilverBulletClient,
        probe::{new_probe_key, verify_write_guards},
    },
};
use http_fixture::{Fault, Fixture};
use std::time::Duration;
fn config() -> ConnectionConfig {
    ConnectionConfig::parse("https://notes.example/prefix/").unwrap()
}
fn client(f: &Fixture) -> SilverBulletClient {
    SilverBulletClient::for_loopback_fixture(
        &config(),
        f.base.clone(),
        None,
        Duration::from_millis(100),
    )
    .unwrap()
}
#[tokio::test]
async fn complete_probe_checks_guards_preserves_its_identity_and_cleans_up() {
    let f = Fixture::start(None).await;
    let c = client(&f);
    let key = new_probe_key(config().id().clone());
    let evidence = verify_write_guards(&c, key.clone()).await.unwrap();
    assert!(evidence.permits(&c));
    assert!(
        !evidence.permits(&client(&f)),
        "reconnection requires fresh evidence even at the same URL"
    );
    assert_eq!(evidence.checks().len(), 6);
    assert!(f.body().is_none());
    let requests = f.seen();
    assert!(requests.iter().all(|r| r.uri.ends_with(
        &format!("/{}.md", key.path.as_str().rsplit('/').next().unwrap().trim_end_matches(".md"))
    )));
    assert_eq!(
        requests
            .iter()
            .filter(|r| r.method == reqwest::Method::PUT)
            .count(),
        4
    );
    assert!(
        requests
            .iter()
            .filter(|r| r.method == reqwest::Method::PUT || r.method == reqwest::Method::DELETE)
            .all(|r| r.headers.contains_key("if-match")
                || r.headers.get("if-none-match").is_some_and(|v| v == "*"))
    );
    assert!(requests.iter().any(|r| !r.body.is_empty()));
}
#[tokio::test]
async fn ignored_preconditions_disable_mutations() {
    let f = Fixture::start(None).await;
    f.fault(Fault::IgnorePreconditions);
    assert!(
        verify_write_guards(&client(&f), new_probe_key(config().id().clone()))
            .await
            .is_err()
    );
}
#[tokio::test]
async fn probe_cannot_be_aimed_at_an_existing_normal_page() {
    let f = Fixture::start(Some(b"normal note")).await;
    let key = PageKey {
        connection: config().id().clone(),
        path: PagePath::parse("sample.md").unwrap(),
    };
    assert_eq!(
        verify_write_guards(&client(&f), key).await.unwrap_err(),
        SyncError::Incompatible
    );
    assert!(f.seen().is_empty());
    assert_eq!(f.body().unwrap(), b"normal note");
}
