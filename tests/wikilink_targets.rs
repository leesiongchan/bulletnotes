mod support;
use bulletnotes::{
    domain::{
        link::{self, Resolution},
        page::{LocalRevision, PagePath, Permission, RemoteMetadata},
    },
    store::StoreHandle,
};
use support::storage_fixture::{database, key};

fn path(value: &str) -> PagePath {
    PagePath::parse(value).unwrap()
}

#[tokio::test]
async fn catalogued_trashed_and_unknown_targets_resolve_against_the_real_store() {
    let directory = tempfile::tempdir().unwrap();
    let store = StoreHandle::open(&database(&directory)).await.unwrap();
    let source = key("Journal/today.md");
    let connection = source.connection.clone();
    let resolve = |catalogue: &bulletnotes::store::LinkCatalogue, raw: &str| {
        link::resolve(
            &link::parse(raw).unwrap().unwrap(),
            &source.path,
            &catalogue.pages,
            catalogue.listed,
        )
    };

    let catalogue = store.link_catalogue(&connection).await.unwrap();
    assert!(!catalogue.listed, "no remote listing has been recorded yet");
    assert_eq!(
        resolve(&catalogue, "Someday"),
        Err(link::LinkError::Unlisted)
    );

    // A remote-only page has a row without a cached body; it must open, not be recreated.
    store
        .record_catalogue(
            &connection,
            vec![RemoteMetadata {
                key: key("Projects/Plan.md"),
                modified_ms: Some(1),
                content_length: None,
                permission: Permission::ReadWrite,
                revision: None,
            }],
            true,
        )
        .await
        .unwrap();
    assert!(
        store
            .load_page(&key("Projects/Plan.md"))
            .await
            .unwrap()
            .is_none()
    );

    let trashed = key("Old.md");
    store.create_draft(trashed.clone()).await.unwrap();
    store
        .save_local(trashed.clone(), "# Old".into(), LocalRevision(1))
        .await
        .unwrap();
    store
        .move_to_trash(trashed.clone(), LocalRevision(1))
        .await
        .unwrap();

    let catalogue = store.link_catalogue(&connection).await.unwrap();
    assert!(catalogue.listed);
    assert_eq!(
        resolve(&catalogue, "Plan"),
        Ok(Resolution::Open(path("Projects/Plan.md")))
    );
    assert_eq!(
        resolve(&catalogue, "Old"),
        Ok(Resolution::Open(path("Old.md"))),
        "an exact trashed page keeps its unavailable handling"
    );
    assert_eq!(
        resolve(&catalogue, "Projects/old"),
        Ok(Resolution::Create(path("Projects/old.md")))
    );
    assert_eq!(
        resolve(&catalogue, "Someday"),
        Ok(Resolution::Create(path("Someday.md")))
    );

    let other = bulletnotes::domain::connection::ConnectionConfig::parse("https://other.example")
        .unwrap()
        .id()
        .clone();
    let elsewhere = store.link_catalogue(&other).await.unwrap();
    assert!(elsewhere.pages.is_empty() && !elsewhere.listed);
    store.shutdown().await.unwrap();
}

#[tokio::test]
async fn link_sources_are_visible_local_bodies_containing_wikilinks() {
    let directory = tempfile::tempdir().unwrap();
    let store = StoreHandle::open(&database(&directory)).await.unwrap();
    let connection = key("a.md").connection.clone();
    for (path, body) in [
        ("Linker.md", "See [[Plan]]."),
        ("Plain.md", "No links [here]."),
        ("Gone.md", "Was [[Plan]]."),
    ] {
        store.create_draft(key(path)).await.unwrap();
        store
            .save_local(key(path), body.into(), LocalRevision(1))
            .await
            .unwrap();
    }
    store
        .move_to_trash(key("Gone.md"), LocalRevision(1))
        .await
        .unwrap();
    // A remote-only page has no local body to read.
    store
        .record_catalogue(
            &connection,
            vec![RemoteMetadata {
                key: key("Remote.md"),
                modified_ms: Some(1),
                content_length: None,
                permission: Permission::ReadWrite,
                revision: None,
            }],
            false,
        )
        .await
        .unwrap();
    assert_eq!(
        store.link_sources(&connection).await.unwrap(),
        [(path("Linker.md"), "See [[Plan]].".to_owned())]
    );
    store.shutdown().await.unwrap();
}
