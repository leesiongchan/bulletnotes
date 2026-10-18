#[path = "../examples/support/acceptance_relay.rs"]
mod acceptance_relay;
#[path = "support/space_fixture.rs"]
mod space_fixture;
#[tokio::test]
async fn relay_rejects_unregistered_paths_and_sources_before_upstream_and_filters_catalogue() {
    let server = space_fixture::SpaceFixture::start().await;
    server.put("owned.md", "generated");
    server.put("unrelated.md", "UNRELATED_BODY_SENTINEL");
    let directory = tempfile::tempdir().unwrap();
    let (relay, base, task) = acceptance_relay::Relay::start(
        server.base.clone(),
        directory.path().join("journal.json"),
        false,
    )
    .await
    .unwrap();
    relay.register("owned.md", &["generated"]).unwrap();
    let client = reqwest::Client::new();
    let entries: serde_json::Value = client
        .get(base.join(".fs").unwrap())
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(entries.as_array().unwrap().len(), 1);
    assert_eq!(entries[0]["name"], "owned.md");
    for path in ["unrelated.md", "owned.md/extra", "%6fwned.md"] {
        assert_eq!(
            client
                .get(base.join(&format!(".fs/{path}")).unwrap())
                .send()
                .await
                .unwrap()
                .status(),
            403
        );
    }
    assert_eq!(
        client
            .put(base.join(".fs/owned.md").unwrap())
            .header("if-none-match", "*")
            .body("UNREGISTERED")
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    assert!(server.data.lock().unwrap().puts.is_empty());
    relay.offline(true);
    assert_eq!(
        client
            .get(base.join(".fs/owned.md").unwrap())
            .send()
            .await
            .unwrap()
            .status(),
        503
    );
    relay.offline(false);
    relay.arm("owned.md");
    let reading = tokio::spawn(async move {
        client
            .get(base.join(".fs/owned.md").unwrap())
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap()
    });
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        while !relay.waiting() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert!(!reading.is_finished());
    relay.release();
    assert_eq!(reading.await.unwrap(), "generated");
    assert_eq!(relay.rejected(), 4);
    task.abort();
}
