use bulletnotes::{
    app::model::EditorPosition,
    domain::{connection::ConnectionConfig, page::*},
    store::{StoreHandle, search::SearchQuery},
    sync::reconcile::reconcile,
};
fn key(path: &str) -> PageKey {
    PageKey {
        connection: ConnectionConfig::parse("https://notes.example/")
            .unwrap()
            .id()
            .clone(),
        path: PagePath::parse(path).unwrap(),
    }
}
fn remote(path: &str, body: &str, revision: &str) -> RemoteFile {
    RemoteFile {
        key: key(path),
        body: body.into(),
        revision: Some(RemoteRevision(format!("\"{revision}\""))),
        modified_ms: Some(1),
        permission: Permission::ReadWrite,
    }
}
fn local(body: &str, base: Option<RemoteFile>) -> PageSnapshot {
    PageSnapshot {
        key: key("one.md"),
        body: body.into(),
        local_revision: LocalRevision(1),
        remote_base: base,
        position: EditorPosition::default(),
        permission: Permission::ReadWrite,
    }
}
fn query() -> SearchQuery {
    SearchQuery {
        connection: key("one.md").connection,
        text: String::new(),
        folder: None,
        offset: 0,
        limit: 100,
    }
}
#[test]
fn reconciliation_keeps_a_common_base_and_distinguishes_deletion_from_empty_text() {
    let a = remote("one.md", "base", "a");
    assert!(matches!(
        reconcile(
            &local("base", Some(a.clone())),
            &RemoteRead::Present(remote("one.md", "new", "b"))
        ),
        ReconcileDecision::InstallRemote(_)
    ));
    let dirty = local("local edit", Some(a.clone()));
    let observation = RemoteRead::Present(remote("one.md", "remote edit", "b"));
    let ReconcileDecision::Conflict(conflict) = reconcile(&dirty, &observation) else {
        panic!("lost concurrent edit")
    };
    assert_eq!(conflict.base, Some(a.clone()));
    assert_eq!(conflict.local, dirty);
    assert_eq!(conflict.remote, observation);
    assert!(matches!(
        reconcile(
            &local("base", Some(a.clone())),
            &RemoteRead::Missing(key("one.md"))
        ),
        ReconcileDecision::RetainDeletedSnapshot
    ));
    assert!(matches!(
        reconcile(&dirty, &RemoteRead::Missing(key("one.md"))),
        ReconcileDecision::Conflict(_)
    ));
    assert!(
        matches!(reconcile(&local("base",Some(a)),&RemoteRead::Present(remote("one.md","","b"))),ReconcileDecision::InstallRemote(file) if file.body.is_empty())
    );
    assert!(matches!(
        reconcile(
            &local("new local", None),
            &RemoteRead::Missing(key("one.md"))
        ),
        ReconcileDecision::KeepPending
    ));
}
#[tokio::test]
async fn catalogue_failure_does_not_delete_pages_and_partial_index_is_visible() {
    let dir = tempfile::tempdir().unwrap();
    let store = StoreHandle::open(&dir.path().join("test.sqlite"))
        .await
        .unwrap();
    let entries = vec!["one.md", "two.md"]
        .into_iter()
        .map(|path| RemoteMetadata {
            key: key(path),
            modified_ms: Some(1),
            content_length: Some(4),
            permission: Permission::ReadOnly,
            revision: None,
        })
        .collect();
    store
        .record_catalogue(&key("one.md").connection, entries, true)
        .await
        .unwrap();
    let result = store.search(query()).await.unwrap();
    assert_eq!(result.total, 2);
    assert_eq!(result.indexed, 0);
    assert!(!result.complete);
    store
        .apply_remote(RemoteRead::Present(remote("one.md", "first", "a")))
        .await
        .unwrap();
    store
        .record_catalogue(&key("one.md").connection, vec![], false)
        .await
        .unwrap();
    let result = store.search(query()).await.unwrap();
    assert_eq!(result.total, 2);
    assert_eq!(result.indexed, 1);
    assert!(!result.complete);
    assert_eq!(
        store.load_page(&key("one.md")).await.unwrap().unwrap().body,
        "first"
    );
    store.shutdown().await.unwrap();
}
#[tokio::test]
async fn a_remote_overlap_is_installed_with_a_durable_local_recovery() {
    let dir = tempfile::tempdir().unwrap();
    let store = StoreHandle::open(&dir.path().join("test.sqlite"))
        .await
        .unwrap();
    store
        .apply_remote(RemoteRead::Present(remote("one.md", "base", "a")))
        .await
        .unwrap();
    store
        .save_local(key("one.md"), "local".into(), LocalRevision(1))
        .await
        .unwrap();
    assert!(matches!(
        store
            .apply_remote(RemoteRead::Present(remote("one.md", "remote", "b")))
            .await
            .unwrap(),
        ReconcileDecision::InstallRemote(_)
    ));
    assert_eq!(
        store.load_page(&key("one.md")).await.unwrap().unwrap().body,
        "remote"
    );
    assert!(
        store
            .claim_pending(&key("one.md").connection, 8)
            .await
            .unwrap()
            .is_empty()
    );
    let conflicts = store.conflicts(&key("one.md").connection).await.unwrap();
    assert!(conflicts.is_empty());
    let jobs = store
        .recovery_uploads(&key("one.md").connection, 10)
        .await
        .unwrap();
    assert_eq!(jobs.len(), 1);
    assert_eq!(
        store
            .automatic_recovery_event(jobs[0].event_id)
            .await
            .unwrap()
            .input
            .local
            .body,
        "local"
    );
    store.shutdown().await.unwrap();
}
#[tokio::test]
async fn an_uncommitted_editor_keeps_its_original_base_when_remote_install_wins_the_race() {
    let dir = tempfile::tempdir().unwrap();
    let store = StoreHandle::open(&dir.path().join("test.sqlite"))
        .await
        .unwrap();
    let original = remote("one.md", "base", "a");
    store
        .apply_remote(RemoteRead::Present(original.clone()))
        .await
        .unwrap();
    store
        .apply_remote(RemoteRead::Present(remote("one.md", "remote", "b")))
        .await
        .unwrap();
    let saved = store
        .save_local_from_base(
            key("one.md"),
            "unsaved editor text".into(),
            LocalRevision(1),
            EditorBaseline {
                body: "base".into(),
                remote_base: Some(original.clone()),
            },
        )
        .await
        .unwrap();
    assert_eq!(saved.local_revision, LocalRevision(1));
    let restored = store
        .load_editor_page(&key("one.md"))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        restored.authored.as_ref().unwrap().remote_base,
        Some(original)
    );
    assert_eq!(restored.presented().body, "unsaved editor text");
    assert_eq!(restored.stored.page.body, "remote");
    assert_eq!(
        store
            .pending_reconciliation_keys(&key("one.md").connection)
            .await
            .unwrap(),
        vec![key("one.md")]
    );
    assert!(
        store
            .claim_pending(&key("one.md").connection, 8)
            .await
            .unwrap()
            .is_empty()
    );
    store.shutdown().await.unwrap();
}

#[tokio::test]
async fn acknowledgement_of_our_own_write_does_not_create_a_false_editor_conflict() {
    let dir = tempfile::tempdir().unwrap();
    let store = StoreHandle::open(&dir.path().join("test.sqlite"))
        .await
        .unwrap();
    let original = remote("one.md", "base", "a");
    store
        .apply_remote(RemoteRead::Present(original.clone()))
        .await
        .unwrap();
    store
        .save_local(key("one.md"), "first local edit".into(), LocalRevision(1))
        .await
        .unwrap();
    let sent = store
        .claim_pending(&key("one.md").connection, 1)
        .await
        .unwrap()
        .remove(0);
    store
        .acknowledge(
            sent.id,
            MutationAck {
                remote_revision: Some(RemoteRevision("\"b\"".into())),
            },
        )
        .await
        .unwrap();
    store
        .save_local_from_base(
            key("one.md"),
            "second local edit".into(),
            LocalRevision(2),
            EditorBaseline {
                body: "first local edit".into(),
                remote_base: Some(original),
            },
        )
        .await
        .unwrap();
    assert!(
        store
            .conflicts(&key("one.md").connection)
            .await
            .unwrap()
            .is_empty()
    );
    let pending = store
        .claim_pending(&key("one.md").connection, 1)
        .await
        .unwrap();
    assert_eq!(
        pending[0].condition,
        WriteCondition::Match(RemoteRevision("\"b\"".into()))
    );
    store.shutdown().await.unwrap();
}

#[tokio::test]
async fn metadata_only_remote_updates_preserve_saved_selection() {
    let dir = tempfile::tempdir().unwrap();
    let store = StoreHandle::open(&dir.path().join("test.sqlite"))
        .await
        .unwrap();
    store
        .apply_remote(RemoteRead::Present(remote("one.md", "text café", "a")))
        .await
        .unwrap();
    let position = EditorPosition {
        anchor: 2,
        caret: 4,
        scroll_y: 23.0,
    };
    store
        .save_position(&key("one.md"), position, 1)
        .await
        .unwrap();
    store
        .apply_remote(RemoteRead::Present(remote("one.md", "text café", "b")))
        .await
        .unwrap();
    assert_eq!(
        store
            .load_page(&key("one.md"))
            .await
            .unwrap()
            .unwrap()
            .position,
        position
    );
    store.shutdown().await.unwrap();
}
#[tokio::test]
async fn a_permission_change_does_not_destroy_an_already_editing_local_buffer() {
    let dir = tempfile::tempdir().unwrap();
    let store = StoreHandle::open(&dir.path().join("test.sqlite"))
        .await
        .unwrap();
    let original = remote("one.md", "base", "a");
    store
        .apply_remote(RemoteRead::Present(original.clone()))
        .await
        .unwrap();
    let mut readonly = original.clone();
    readonly.permission = Permission::ReadOnly;
    store
        .apply_remote(RemoteRead::Present(readonly))
        .await
        .unwrap();
    store
        .save_local_from_base(
            key("one.md"),
            "already editing".into(),
            LocalRevision(1),
            EditorBaseline {
                body: "base".into(),
                remote_base: Some(original),
            },
        )
        .await
        .unwrap();
    assert_eq!(
        store.load_page(&key("one.md")).await.unwrap().unwrap().body,
        "already editing"
    );
    assert_eq!(
        store
            .conflicts(&key("one.md").connection)
            .await
            .unwrap()
            .len(),
        1
    );
    assert!(
        store
            .claim_pending(&key("one.md").connection, 4)
            .await
            .unwrap()
            .is_empty()
    );
    store.shutdown().await.unwrap();
}
