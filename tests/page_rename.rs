//! Renaming commits the new page, the old page's move to Trash and the link
//! rewrites in one transaction, and each change is queued for sync.
mod support;
use bulletnotes::{
    domain::{error::StoreError, page::*},
    store::{LinkRewrite, StoreHandle},
};
use support::storage_fixture::{database, key};

fn rewrite() -> LinkRewrite {
    Box::new(
        |_pages: &[(PagePath, bool)], _source: &PagePath, body: &str| {
            body.contains("[[Plan")
                .then(|| body.replace("[[Plan", "[[Roadmap"))
        },
    )
}

#[tokio::test]
async fn renaming_moves_the_page_to_trash_and_updates_links_atomically() {
    let directory = tempfile::tempdir().unwrap();
    let path = database(&directory);
    let store = StoreHandle::open(&path).await.unwrap();
    let plan = key("Projects/Plan.md");
    let connection = plan.connection.clone();
    for (page, body) in [
        ("Projects/Plan.md", "# Plan\nBack to [[Plan#Plan]]"),
        ("Journal/today.md", "See [[Plan]] and [[Other]]."),
        ("Other.md", "No links to the plan."),
    ] {
        store.create_draft(key(page)).await.unwrap();
        store
            .save_local(key(page), body.into(), LocalRevision(1))
            .await
            .unwrap();
    }
    let body = |page: &'static str| {
        let store = store.clone();
        async move {
            store
                .load_page(&key(page))
                .await
                .unwrap()
                .map(|page| (page.body, page.local_revision))
        }
    };

    // Failures leave every page as it was.
    assert!(matches!(
        store
            .rename_page(
                plan.clone(),
                LocalRevision(1),
                PagePath::parse("Other.md").unwrap(),
                Some(rewrite())
            )
            .await,
        Err(StoreError::PathCollision)
    ));
    assert!(matches!(
        store
            .rename_page(
                plan.clone(),
                LocalRevision(0),
                PagePath::parse("Roadmap.md").unwrap(),
                Some(rewrite())
            )
            .await,
        Err(StoreError::StaleRevision)
    ));
    assert!(store.trash(&connection).await.unwrap().is_empty());
    assert_eq!(
        body("Journal/today.md").await,
        Some(("See [[Plan]] and [[Other]].".into(), LocalRevision(1)))
    );

    let (renamed, updated) = store
        .rename_page(
            plan.clone(),
            LocalRevision(1),
            PagePath::parse("Roadmap.md").unwrap(),
            Some(rewrite()),
        )
        .await
        .unwrap();
    assert_eq!(renamed, key("Roadmap.md"));
    assert_eq!(updated, [key("Journal/today.md")]);
    assert_eq!(
        body("Roadmap.md").await.map(|(body, _)| body),
        Some("# Plan\nBack to [[Roadmap#Plan]]".into()),
        "links inside the renamed page are updated too"
    );
    assert_eq!(
        body("Journal/today.md").await,
        Some(("See [[Roadmap]] and [[Other]].".into(), LocalRevision(2)))
    );
    assert_eq!(
        body("Other.md").await,
        Some(("No links to the plan.".into(), LocalRevision(1)))
    );
    let trash = store.trash(&connection).await.unwrap();
    assert_eq!(trash.len(), 1, "the old page stays recoverable");

    let db = rusqlite::Connection::open(&path).unwrap();
    let queued = |page: &str, kind: &str| -> i64 {
        db.query_row(
            "SELECT count(*) FROM outbox o JOIN pages p ON p.id=o.page_id WHERE p.path=?1 AND o.kind=?2",
            [page, kind],
            |row| row.get(0),
        )
        .unwrap()
    };
    assert!(queued("Roadmap.md", "put") > 0, "the new page uploads");
    assert!(
        queued("Journal/today.md", "put") > 0,
        "the rewritten link uploads"
    );
    // The old page never reached the server, so Trash withdraws its upload rather
    // than queueing a remote delete.
    assert_eq!(queued("Projects/Plan.md", "put"), 0);
    let create: String = db
        .query_row(
            "SELECT o.condition_kind FROM outbox o JOIN pages p ON p.id=o.page_id WHERE p.path='Roadmap.md' AND o.kind='put'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(
        create, "create",
        "the new page never overwrites a remote page"
    );

    // Without link updates only the page moves.
    let (_, updated) = store
        .rename_page(
            renamed,
            LocalRevision(1),
            PagePath::parse("Later.md").unwrap(),
            None,
        )
        .await
        .unwrap();
    assert!(updated.is_empty());
    assert_eq!(
        body("Journal/today.md").await,
        Some(("See [[Roadmap]] and [[Other]].".into(), LocalRevision(2)))
    );
    store.shutdown().await.unwrap();
}

#[tokio::test]
async fn scratchpads_keep_their_place_only_while_named_inside_quick_notes() {
    let directory = tempfile::tempdir().unwrap();
    let store = StoreHandle::open(&database(&directory)).await.unwrap();
    let connection = key("a.md").connection.clone();
    let pads = [
        "Quick Notes/2026-11-01/090000-00000000-0000-4000-8000-000000000001.md",
        "Quick Notes/2026-11-02/090000-00000000-0000-4000-8000-000000000002.md",
        "Quick Notes/2026-11-03/090000-00000000-0000-4000-8000-000000000003.md",
    ];
    for page in pads.iter().copied().chain(["Projects/Plan.md"]) {
        store.create_draft(key(page)).await.unwrap();
        store
            .save_local(key(page), "text".into(), LocalRevision(1))
            .await
            .unwrap();
    }
    let stack = || {
        let store = store.clone();
        let connection = connection.clone();
        async move {
            store
                .scratchpad_stack(&connection)
                .await
                .unwrap()
                .into_iter()
                .map(|entry| entry.key.path.as_str().to_owned())
                .collect::<Vec<_>>()
        }
    };
    let before = stack().await;
    assert_eq!(before.len(), 3);
    let middle = before[1].clone();
    let rename = |from: &str, to: &str| {
        let store = store.clone();
        let (from, to) = (key(from), PagePath::parse(to).unwrap());
        async move { store.rename_page(from, LocalRevision(1), to, None).await }
    };

    rename(&middle, "Quick Notes/Renamed.md").await.unwrap();
    let mut expected = before.clone();
    expected[1] = "Quick Notes/Renamed.md".into();
    assert_eq!(stack().await, expected, "same place, old name gone");

    rename("Quick Notes/Renamed.md", "Ideas/Renamed.md")
        .await
        .unwrap();
    assert_eq!(stack().await, [before[0].clone(), before[2].clone()]);

    rename("Projects/Plan.md", "Projects/Roadmap.md")
        .await
        .unwrap();
    assert_eq!(
        stack().await.len(),
        2,
        "an ordinary page stays out of the stack"
    );

    // A page that never reached the server leaves no remote delete behind, so its
    // name is free again at once.
    assert_eq!(
        rename("Projects/Roadmap.md", "Projects/Plan.md")
            .await
            .unwrap()
            .0,
        key("Projects/Plan.md")
    );
    store.shutdown().await.unwrap();
}

#[tokio::test]
async fn a_synced_page_keeps_its_old_name_until_the_remote_delete_syncs() {
    let directory = tempfile::tempdir().unwrap();
    let store = StoreHandle::open(&database(&directory)).await.unwrap();
    store
        .apply_remote(RemoteRead::Present(RemoteFile {
            key: key("Synced.md"),
            body: "from the server".into(),
            revision: Some(RemoteRevision("a".into())),
            modified_ms: None,
            permission: Permission::ReadWrite,
        }))
        .await
        .unwrap();
    let revision = store
        .load_page(&key("Synced.md"))
        .await
        .unwrap()
        .unwrap()
        .local_revision;
    let (moved, _) = store
        .rename_page(
            key("Synced.md"),
            revision,
            PagePath::parse("Moved.md").unwrap(),
            None,
        )
        .await
        .unwrap();
    assert!(matches!(
        store
            .rename_page(
                moved,
                LocalRevision(1),
                PagePath::parse("Synced.md").unwrap(),
                None
            )
            .await,
        Err(StoreError::PathCollision)
    ));
    store.shutdown().await.unwrap();
}
