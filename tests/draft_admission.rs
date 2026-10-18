use bulletnotes::{
    domain::{connection::ConnectionConfig, error::StoreError, page::*},
    store::{DraftAdmissionRequest, StoreHandle},
    sync::{merge_executor::MergeExecutor, reconcile::prepare_reconciliation},
};
use std::time::Duration;
fn key(path: &str) -> PageKey {
    PageKey {
        connection: ConnectionConfig::parse("https://admission.example/")
            .unwrap()
            .id()
            .clone(),
        path: PagePath::parse(path).unwrap(),
    }
}
fn observed() -> RemoteRead {
    RemoteRead::Present(RemoteFile {
        key: key("owned.md"),
        body: "remote".into(),
        revision: Some(RemoteRevision("r".into())),
        modified_ms: None,
        permission: Permission::ReadWrite,
    })
}
async fn seed(store: &StoreHandle) {
    store.create_draft(key("owned.md")).await.unwrap();
    store
        .save_local(key("owned.md"), "local".into(), LocalRevision(1))
        .await
        .unwrap();
    store
        .save_conflict(ConflictRecord {
            id: uuid::Uuid::new_v4(),
            key: key("owned.md"),
            base: None,
            local: store.load_page(&key("owned.md")).await.unwrap().unwrap(),
            remote: observed(),
        })
        .await
        .unwrap();
}
#[tokio::test]
async fn worker_origin_drains_latest_draft_while_local_durability_and_peer_progress() {
    let directory = tempfile::tempdir().unwrap();
    let store = StoreHandle::open(&directory.path().join("owned.sqlite"))
        .await
        .unwrap();
    seed(&store).await;
    let mut owner = store.install_draft_admission().unwrap();
    let running = store.clone();
    let job = tokio::spawn(async move { running.apply_remote(observed()).await });
    let DraftAdmissionRequest::Close {
        key: target, reply, ..
    } = owner.recv().await.unwrap()
    else {
        panic!("expected close")
    };
    assert_eq!(target, key("owned.md"));
    assert!(!job.is_finished());
    tokio::time::timeout(Duration::from_secs(1), async {
        store.create_draft(key("peer.md")).await.unwrap();
        store
            .save_local(key("peer.md"), "independent".into(), LocalRevision(1))
            .await
            .unwrap();
        store
            .save_local_from_base_outcome(
                key("owned.md"),
                "local newest".into(),
                LocalRevision(2),
                EditorBaseline {
                    body: "local".into(),
                    remote_base: None,
                },
            )
            .await
            .unwrap();
        store
            .save_merge_drafts(vec![(target, "admitted draft\r\n日本語".into())])
            .await
            .unwrap();
    })
    .await
    .unwrap();
    reply.send(Ok(())).unwrap();
    job.await.unwrap().unwrap();
    let DraftAdmissionRequest::Release { committed, .. } = owner.recv().await.unwrap() else {
        panic!("expected release")
    };
    assert!(committed);
    let heads = store
        .recovery_uploads(&key("owned.md").connection, 10)
        .await
        .unwrap();
    let event = store
        .automatic_recovery_event(heads[0].event_id)
        .await
        .unwrap();
    assert_eq!(
        event.input.draft.as_deref(),
        Some("admitted draft\r\n日本語")
    );
    store.shutdown().await.unwrap();
}
#[tokio::test]
async fn public_commit_cannot_bypass_owner_and_rejects_pre_drain_snapshot() {
    let directory = tempfile::tempdir().unwrap();
    let store = StoreHandle::open(&directory.path().join("owned.sqlite"))
        .await
        .unwrap();
    seed(&store).await;
    let context = store
        .load_reconcile_context(&key("owned.md"), observed())
        .await
        .unwrap();
    let prepared = prepare_reconciliation(context, &MergeExecutor::default()).await;
    let mut owner = store.install_draft_admission().unwrap();
    let running = store.clone();
    let job = tokio::spawn(async move { running.commit_reconciliation(prepared).await });
    let DraftAdmissionRequest::Close {
        key: target, reply, ..
    } = owner.recv().await.unwrap()
    else {
        panic!("expected close")
    };
    store
        .save_merge_drafts(vec![(target, "new admitted".into())])
        .await
        .unwrap();
    reply.send(Ok(())).unwrap();
    assert!(matches!(job.await.unwrap(), Err(StoreError::StaleRevision)));
    let DraftAdmissionRequest::Release { committed, .. } = owner.recv().await.unwrap() else {
        panic!("expected release")
    };
    assert!(!committed);
    assert_eq!(
        store
            .conflicts(&key("owned.md").connection)
            .await
            .unwrap()
            .len(),
        1
    );
    store.shutdown().await.unwrap();
}
#[tokio::test]
async fn cancellation_releases_exact_attempt_and_disconnected_owner_fails_closed() {
    let directory = tempfile::tempdir().unwrap();
    let store = StoreHandle::open(&directory.path().join("owned.sqlite"))
        .await
        .unwrap();
    seed(&store).await;
    let mut owner = store.install_draft_admission().unwrap();
    let running = store.clone();
    let job = tokio::spawn(async move { running.apply_remote(observed()).await });
    let DraftAdmissionRequest::Close {
        lease_id, reply, ..
    } = owner.recv().await.unwrap()
    else {
        panic!("expected close")
    };
    job.abort();
    let _ = job.await;
    drop(reply);
    let DraftAdmissionRequest::Release {
        lease_id: released,
        committed,
        ..
    } = owner.recv().await.unwrap()
    else {
        panic!("expected release")
    };
    assert_eq!(released, lease_id);
    assert!(!committed);
    drop(owner);
    assert!(matches!(
        store.apply_remote(observed()).await,
        Err(StoreError::Closed)
    ));
    assert_eq!(
        store
            .conflicts(&key("owned.md").connection)
            .await
            .unwrap()
            .len(),
        1
    );
    store.create_draft(key("peer.md")).await.unwrap();
    store.shutdown().await.unwrap();
}

#[tokio::test]
async fn unknown_put_delete_retirement_and_watch_repair_share_record_admission() {
    for delete in [false, true] {
        let directory = tempfile::tempdir().unwrap();
        let store = StoreHandle::open(&directory.path().join("owned.sqlite"))
            .await
            .unwrap();
        let RemoteRead::Present(mut base) = observed() else {
            unreachable!()
        };
        base.body = "base".into();
        base.revision = Some(RemoteRevision("base".into()));
        store
            .apply_remote(RemoteRead::Present(base.clone()))
            .await
            .unwrap();
        if delete {
            store
                .move_to_trash(key("owned.md"), LocalRevision(0))
                .await
                .unwrap();
        } else {
            store
                .save_local(key("owned.md"), "local".into(), LocalRevision(1))
                .await
                .unwrap();
        }
        let operation = store
            .claim_pending(&key("owned.md").connection, 1)
            .await
            .unwrap()
            .remove(0);
        store.mark_unknown(operation.id).await.unwrap();
        let current = store.load_page(&key("owned.md")).await.unwrap().unwrap();
        store
            .save_conflict(ConflictRecord {
                id: uuid::Uuid::new_v4(),
                key: key("owned.md"),
                base: Some(base),
                local: current,
                remote: observed(),
            })
            .await
            .unwrap();
        let mut owner = store.install_draft_admission().unwrap();
        let background = store.clone();
        let job = tokio::spawn(async move {
            if delete {
                let RemoteRead::Present(file) = observed() else {
                    unreachable!()
                };
                background.retire_unknown_delete(operation.id, file).await
            } else {
                background
                    .reconcile_unknown_put(operation.id, observed())
                    .await
            }
        });
        let DraftAdmissionRequest::Close {
            key: target, reply, ..
        } = owner.recv().await.unwrap()
        else {
            panic!("expected close")
        };
        store
            .save_merge_drafts(vec![(target, "latest before uncertain retirement".into())])
            .await
            .unwrap();
        reply.send(Ok(())).unwrap();
        job.await.unwrap().unwrap();
        let DraftAdmissionRequest::Release { committed, .. } = owner.recv().await.unwrap() else {
            panic!("expected release")
        };
        assert!(committed);
        if delete {
            let local = store.load_page(&key("owned.md")).await.unwrap().unwrap();
            store
                .save_conflict(ConflictRecord {
                    id: uuid::Uuid::new_v4(),
                    key: key("owned.md"),
                    base: local.remote_base.clone(),
                    local,
                    remote: RemoteRead::Missing(key("owned.md")),
                })
                .await
                .unwrap();
            let background = store.clone();
            let job = tokio::spawn(async move {
                background
                    .apply_remote(RemoteRead::Missing(key("owned.md")))
                    .await
            });
            let DraftAdmissionRequest::Close {
                key: target, reply, ..
            } = owner.recv().await.unwrap()
            else {
                panic!("expected close")
            };
            store
                .save_merge_drafts(vec![(target, "watch repair admitted draft".into())])
                .await
                .unwrap();
            reply.send(Ok(())).unwrap();
            job.await.unwrap().unwrap();
            let DraftAdmissionRequest::Release { committed, .. } = owner.recv().await.unwrap()
            else {
                panic!("expected release")
            };
            assert!(committed);
        }
        store.shutdown().await.unwrap();
    }
}
