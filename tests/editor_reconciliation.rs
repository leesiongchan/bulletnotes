use bulletnotes::{
    app::{
        coordinator::{AppAction, AppEffect, AppEvent, Coordinator},
        editor::EditorSession,
    },
    domain::{
        connection::ConnectionConfig,
        page::*,
        reconciliation::{LocalSaveOutcome, StoredPage},
    },
    sync::text_merge::merge_text,
};
use std::time::Duration;
fn key() -> PageKey {
    PageKey {
        connection: ConnectionConfig::parse("https://editor.example/")
            .unwrap()
            .id()
            .clone(),
        path: PagePath::parse("owned.md").unwrap(),
    }
}
fn stored(body: &str, revision: u64, generation: u64) -> StoredPage {
    StoredPage {
        page: PageSnapshot {
            key: key(),
            body: body.into(),
            local_revision: LocalRevision(revision),
            remote_base: None,
            position: Default::default(),
            permission: Permission::ReadWrite,
        },
        source_generation: generation,
    }
}
fn app() -> Coordinator {
    let mut app = Coordinator::new(key().connection, key);
    let effects = app.dispatch(AppAction::OpenPage(key()));
    let request_id = effects
        .into_iter()
        .find_map(|e| {
            if let AppEffect::LoadPage { request_id, .. } = e {
                Some(request_id)
            } else {
                None
            }
        })
        .unwrap();
    app.receive(AppEvent::PageLoaded {
        request_id,
        key: key(),
        result: Ok(Some(stored("base", 0, 1).into())),
    });
    app
}
#[test]
fn typing_during_rebase_remains_dirty() {
    let mut editor = EditorSession::restore(stored("red fox jumps", 0, 1));
    editor
        .edit("blue fox jumps".into(), Duration::ZERO)
        .unwrap();
    let expected = editor.rebase_token();
    let merge = merge_text("red fox jumps", editor.body(), "red fox runs");
    editor
        .edit("blue fox jumps\n\nnew typing".into(), Duration::ZERO)
        .unwrap();
    assert!(!editor.apply_rebase(&expected, stored(&merge.body, 1, 2), &merge.edits));
    assert_eq!(editor.body(), "blue fox jumps\n\nnew typing");
    assert!(editor.dirty());
}
#[test]
fn late_save_does_not_acknowledge_newer_text() {
    let mut app = app();
    app.dispatch(AppAction::EditText("submitted".into()));
    let effects = app.dispatch(AppAction::Flush);
    let expected = effects
        .into_iter()
        .find_map(|e| {
            if let AppEffect::SaveLocal { expected, .. } = e {
                Some(expected)
            } else {
                None
            }
        })
        .unwrap();
    app.dispatch(AppAction::EditText("submitted and newer".into()));
    app.receive(AppEvent::LocalSaved {
        expected,
        outcome: LocalSaveOutcome {
            attempt: SavedRevision {
                key: key(),
                local_revision: LocalRevision(1),
            },
            stored: stored("actually stored", 1, 2),
        },
    });
    let editor = app.editor().unwrap();
    assert!(!editor.writing());
    assert!(editor.dirty());
    assert_eq!(editor.body(), "submitted and newer");
    assert_eq!(editor.baseline().body, "submitted");
}
#[test]
fn reopening_same_page_rejects_old_session_result() {
    let old = EditorSession::restore(stored("base", 0, 1));
    let expected = old.rebase_token();
    let mut reopened = EditorSession::restore(stored("base", 0, 1));
    let merge = merge_text("base", "base", "remote");
    assert!(!reopened.apply_rebase(&expected, stored("remote", 0, 2), &merge.edits));
    assert_eq!(reopened.body(), "base");
}
#[test]
fn equal_numeric_revision_does_not_imply_equal_source() {
    let mut editor = EditorSession::restore(stored("base", 0, 1));
    let expected = editor.rebase_token();
    let merge = merge_text("base", "base", "remote");
    assert!(editor.apply_rebase(&expected, stored("remote", 0, 2), &merge.edits));
    assert_eq!(editor.revision(), LocalRevision(0));
    assert_eq!(editor.rebase_token().source_generation, 2);
    assert!(!editor.apply_rebase(&expected, stored("late", 0, 3), &merge.edits));
    assert_eq!(editor.body(), "remote");
}

#[tokio::test]
async fn accepted_stale_save_retains_canonical_branch_until_restart_replay() {
    use bulletnotes::store::StoreHandle;
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("owned.sqlite");
    let store = StoreHandle::open(&path).await.unwrap();
    store.create_draft(key()).await.unwrap();
    store
        .save_local(key(), "red fox jumps".into(), LocalRevision(1))
        .await
        .unwrap();
    let baseline = EditorBaseline {
        body: "red fox jumps".into(),
        remote_base: None,
    };
    // Durable automatic source includes an independent change absent from any HTTP base.
    store
        .save_local(key(), "red fox runs".into(), LocalRevision(2))
        .await
        .unwrap();
    let outcome = store
        .save_local_from_base_outcome(key(), "blue fox jumps".into(), LocalRevision(3), baseline)
        .await
        .unwrap();
    assert_eq!(outcome.attempt.local_revision, LocalRevision(3));
    assert_eq!(outcome.stored.page.body, "red fox runs");
    store.shutdown().await.unwrap();
    let reopened = StoreHandle::open(&path).await.unwrap();
    assert_eq!(
        reopened
            .load_editor_page(&key())
            .await
            .unwrap()
            .unwrap()
            .presented()
            .body,
        "blue fox jumps"
    );
    assert!(
        reopened
            .load_sync_page(&key())
            .await
            .unwrap()
            .unwrap()
            .1
            .pending
    );
    reopened.shutdown().await.unwrap();
}

#[tokio::test]
async fn restart_replay_merges_latest_original_branch_with_canonical_without_http_fiction() {
    use bulletnotes::store::StoreHandle;
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("owned.sqlite");
    let store = StoreHandle::open(&path).await.unwrap();
    store.create_draft(key()).await.unwrap();
    store
        .save_local(key(), "red fox jumps\n\nstable\n".into(), LocalRevision(1))
        .await
        .unwrap();
    store
        .save_local(key(), "red fox runs\n\nstable\n".into(), LocalRevision(2))
        .await
        .unwrap();
    for (revision, body, baseline) in [
        (3, "blue fox jumps\n\nstable\n", "red fox jumps\n\nstable\n"),
        (
            4,
            "green fox jumps\n\nstable\n\nmore typing",
            "blue fox jumps\n\nstable\n",
        ),
    ] {
        store
            .save_local_from_base_outcome(
                key(),
                body.into(),
                LocalRevision(revision),
                EditorBaseline {
                    body: baseline.into(),
                    remote_base: None,
                },
            )
            .await
            .unwrap();
    }
    store.shutdown().await.unwrap();
    let store = StoreHandle::open(&path).await.unwrap();
    assert_eq!(
        store
            .load_editor_page(&key())
            .await
            .unwrap()
            .unwrap()
            .presented()
            .body,
        "green fox jumps\n\nstable\n\nmore typing"
    );
    let result = store.replay_editor_handoff(&key()).await.unwrap().unwrap();
    assert_eq!(result.page.body, "green fox runs\n\nstable\n\nmore typing");
    assert_eq!(result.page.local_revision, LocalRevision(4));
    assert_eq!(result.page.remote_base, None);
    assert!(
        store
            .recovery_copies(&key().connection)
            .await
            .unwrap()
            .iter()
            .any(|copy| copy.page.body == "red fox jumps\n\nstable\n"),
        "displayed common baseline must remain reversible after replay"
    );
    assert!(store.replay_editor_handoff(&key()).await.unwrap().is_none());
    store.shutdown().await.unwrap();
}

#[tokio::test]
async fn editor_overlap_preserves_exact_original_with_honest_local_provenance() {
    use bulletnotes::store::StoreHandle;
    let directory = tempfile::tempdir().unwrap();
    let store = StoreHandle::open(&directory.path().join("owned.sqlite"))
        .await
        .unwrap();
    store.create_draft(key()).await.unwrap();
    store
        .save_local(key(), "red fox".into(), LocalRevision(1))
        .await
        .unwrap();
    store
        .save_local(key(), "green fox".into(), LocalRevision(2))
        .await
        .unwrap();
    store
        .save_local_from_base_outcome(
            key(),
            "blue fox".into(),
            LocalRevision(3),
            EditorBaseline {
                body: "red fox".into(),
                remote_base: None,
            },
        )
        .await
        .unwrap();
    let result = store.replay_editor_handoff(&key()).await.unwrap().unwrap();
    assert_eq!(result.page.body, "green fox");
    let uploads = store.recovery_uploads(&key().connection, 10).await.unwrap();
    assert_eq!(uploads.len(), 1);
    let event = store
        .automatic_recovery_event(uploads[0].event_id)
        .await
        .unwrap();
    assert_eq!(event.input.local.body, "blue fox");
    assert_eq!(event.input.remote, None);
    assert_eq!(event.input.editor.unwrap().durable.page.body, "green fox");
    store.shutdown().await.unwrap();
}

#[test]
fn admitted_save_defers_rebase_and_failed_old_session_cannot_clear_current_attempt() {
    let mut app = app();
    app.dispatch(AppAction::EditText("submitted".into()));
    let effects = app.dispatch(AppAction::Flush);
    let expected = effects
        .into_iter()
        .find_map(|effect| {
            if let AppEffect::SaveLocal { expected, .. } = effect {
                Some(expected)
            } else {
                None
            }
        })
        .unwrap();
    let edits = merge_text("submitted", "submitted", "remote").edits;
    app.receive(AppEvent::RebaseReady {
        expected: expected.clone(),
        stored: stored("remote", 1, 2),
        edits,
    });
    assert_eq!(app.editor().unwrap().body(), "submitted");
    assert!(app.editor().unwrap().writing());
    let mut old = expected;
    old.session_id = uuid::Uuid::new_v4();
    app.receive(AppEvent::LocalSaveFailed(
        old,
        LocalRevision(1),
        bulletnotes::domain::error::StoreError::Closed,
    ));
    assert!(app.editor().unwrap().writing());
}
#[test]
fn rebase_maps_only_dependable_directional_utf8_observations() {
    use bulletnotes::domain::editor::EditorPosition;
    let mut editor = EditorSession::restore(stored("café 日本語", 0, 1));
    editor
        .observe_position(EditorPosition {
            anchor: 9,
            caret: 5,
            scroll_y: 42.0,
        })
        .unwrap();
    let expected = editor.rebase_token();
    let merge = merge_text(editor.body(), editor.body(), "prefix café 日本語");
    assert!(editor.apply_rebase(&expected, stored(&merge.body, 0, 2), &merge.edits));
    assert_eq!(
        editor.observed_position(),
        Some(EditorPosition {
            anchor: 16,
            caret: 12,
            scroll_y: 42.0
        })
    );
    let mut unknown = EditorSession::restore(stored("café 日本語", 0, 1));
    let expected = unknown.rebase_token();
    unknown.apply_rebase(&expected, stored(&merge.body, 0, 2), &merge.edits);
    assert_eq!(unknown.observed_position(), None);
}
#[tokio::test]
async fn replay_failure_rolls_back_and_explicit_trash_supersedes_pending_branch() {
    use bulletnotes::store::StoreHandle;
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("owned.sqlite");
    let store = StoreHandle::open(&path).await.unwrap();
    store.create_draft(key()).await.unwrap();
    store
        .save_local(key(), "red fox".into(), LocalRevision(1))
        .await
        .unwrap();
    store
        .save_local(key(), "green fox".into(), LocalRevision(2))
        .await
        .unwrap();
    store
        .save_local_from_base_outcome(
            key(),
            "blue fox".into(),
            LocalRevision(3),
            EditorBaseline {
                body: "red fox".into(),
                remote_base: None,
            },
        )
        .await
        .unwrap();
    let before = store.load_stored_page(&key()).await.unwrap().unwrap();
    let db = rusqlite::Connection::open(&path).unwrap();
    db.execute_batch("CREATE TRIGGER fail_editor_event BEFORE INSERT ON automatic_events BEGIN SELECT RAISE(ABORT,'generated replay failure'); END;").unwrap();
    assert!(store.replay_editor_handoff(&key()).await.is_err());
    assert_eq!(
        store.load_stored_page(&key()).await.unwrap().unwrap(),
        before
    );
    assert_eq!(
        store
            .load_editor_page(&key())
            .await
            .unwrap()
            .unwrap()
            .presented()
            .body,
        "blue fox"
    );
    db.execute_batch("DROP TRIGGER fail_editor_event").unwrap();
    store.move_to_trash(key(), LocalRevision(3)).await.unwrap();
    assert_eq!(
        store.trash(&key().connection).await.unwrap()[0].page.body,
        "blue fox"
    );
    assert!(store.replay_editor_handoff(&key()).await.unwrap().is_none());
    assert!(store.load_active_page(&key()).await.is_err());
    store.shutdown().await.unwrap();
}
#[tokio::test]
async fn editor_recovery_identity_excludes_revision_generation_position_and_timestamp() {
    use bulletnotes::{
        domain::reconciliation::{
            AutomaticRecoveryInput, AutomaticRecoveryReason, EditorRecoveryProvenance,
        },
        store::StoreHandle,
    };
    let directory = tempfile::tempdir().unwrap();
    let store = StoreHandle::open(&directory.path().join("owned.sqlite"))
        .await
        .unwrap();
    let mut input = AutomaticRecoveryInput {
        local: stored("original", 3, 1).page,
        base: None,
        remote: None,
        editor: Some(EditorRecoveryProvenance {
            remote_missing: false,
            baseline: "base".into(),
            durable: stored("canonical", 3, 1),
        }),
        draft: None,
        kept_body: "chosen".into(),
        reason: AutomaticRecoveryReason::Overlap,
    };
    let first = store
        .register_automatic_recovery(input.clone())
        .await
        .unwrap();
    input.local.local_revision = LocalRevision(99);
    input.local.position.scroll_y = 14.0;
    let durable = &mut input.editor.as_mut().unwrap().durable;
    durable.source_generation = 92;
    durable.page.local_revision = LocalRevision(99);
    durable.page.position.scroll_y = 19.0;
    let repeated = store.register_automatic_recovery(input).await.unwrap();
    assert_eq!(first.id, repeated.id);
    assert_eq!(
        store
            .recovery_uploads(&key().connection, 10)
            .await
            .unwrap()
            .len(),
        1
    );
    store.shutdown().await.unwrap();
}

#[tokio::test]
async fn stale_authoring_after_remote_install_and_delete_remains_restorable_and_recreates() {
    use bulletnotes::store::StoreHandle;
    let directory = tempfile::tempdir().unwrap();
    let store = StoreHandle::open(&directory.path().join("owned.sqlite"))
        .await
        .unwrap();
    let file = |body: &str, revision: &str| RemoteFile {
        key: key(),
        body: body.into(),
        revision: Some(RemoteRevision(revision.into())),
        modified_ms: None,
        permission: Permission::ReadWrite,
    };
    let base = file("red fox jumps", "a");
    store
        .apply_remote(RemoteRead::Present(base.clone()))
        .await
        .unwrap();
    store
        .apply_remote(RemoteRead::Present(file("red fox runs", "b")))
        .await
        .unwrap();
    store
        .apply_remote(RemoteRead::Missing(key()))
        .await
        .unwrap();
    store
        .save_local_from_base_outcome(
            key(),
            "blue fox jumps".into(),
            LocalRevision(1),
            EditorBaseline {
                body: base.body.clone(),
                remote_base: Some(base),
            },
        )
        .await
        .unwrap();
    assert_eq!(
        store
            .load_editor_page(&key())
            .await
            .unwrap()
            .unwrap()
            .presented()
            .body,
        "blue fox jumps"
    );
    let stored = store.replay_editor_handoff(&key()).await.unwrap().unwrap();
    assert_eq!(stored.page.body, "blue fox runs");
    assert_eq!(stored.page.remote_base, None);
    assert_eq!(
        store.claim_pending(&key().connection, 1).await.unwrap()[0].condition,
        WriteCondition::CreateOnly
    );
    store.shutdown().await.unwrap();
}

const CRASH_BASE: &str = "red fox jumps\n\nstable\n";
const CRASH_CANONICAL: &str = "red fox runs\n\nstable\n";
const CRASH_U1: &str = "blue fox jumps\n\nstable\n";
const CRASH_U2: &str = "green fox jumps\n\nstable\n\nmore typing";
const CRASH_MERGED: &str = "green fox runs\n\nstable\n\nmore typing";

#[tokio::test]
async fn killed_editor_owner_retains_exact_admitted_branch_and_real_http_base() {
    use bulletnotes::{domain::error::StoreError, store::StoreHandle};
    use std::{
        process::{Command, Stdio},
        time::Instant,
    };
    for stage in [
        "before-admission",
        "after-admission",
        "coalesced",
        "before-replay",
        "after-replay",
        "leased",
        "unknown",
    ] {
        let directory = tempfile::Builder::new()
            .prefix("bulletnotes-editor-crash-")
            .tempdir()
            .unwrap();
        let path = directory.path().join("owned.sqlite");
        let marker = directory.path().join("ready");
        let nonce = uuid::Uuid::new_v4().to_string();
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args(["--ignored", "--exact", "editor_process_boundary_child"])
            .env("BULLETNOTES_CRASH_PATH", &path)
            .env("BULLETNOTES_CRASH_STAGE", stage)
            .env("BULLETNOTES_CRASH_NONCE", &nonce)
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(8);
        while !marker.exists() {
            if child.try_wait().unwrap().is_some() || Instant::now() >= deadline {
                let _ = child.kill();
                panic!(
                    "{stage}: child did not reach boundary: {}",
                    String::from_utf8_lossy(&child.wait_with_output().unwrap().stderr)
                );
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(
            std::fs::read_to_string(&marker).unwrap(),
            format!("{}:{nonce}", child.id())
        );
        child.kill().unwrap();
        assert!(!child.wait().unwrap().success());
        let store = StoreHandle::open(&path).await.unwrap();
        let restored = store.load_editor_page(&key()).await.unwrap().unwrap();
        let expected = match stage {
            "before-admission" => CRASH_CANONICAL,
            "after-admission" => CRASH_U1,
            "after-replay" => CRASH_MERGED,
            _ => CRASH_U2,
        };
        assert_eq!(restored.presented().body, expected, "{stage}");
        assert_eq!(
            restored.stored.page.remote_base.as_ref().unwrap().body,
            CRASH_BASE,
            "{stage}"
        );
        assert_eq!(
            restored.stored.page.remote_base.as_ref().unwrap().revision,
            Some(RemoteRevision("\"actual-http-base\"".into()))
        );
        assert!(
            store
                .load_sync_page(&key())
                .await
                .unwrap()
                .unwrap()
                .1
                .pending
        );
        if stage == "before-admission" {
            assert!(restored.authored.is_none());
        } else {
            if stage != "after-replay" {
                assert!(restored.authored.is_some(), "{stage}");
            }
            let copies = store.recovery_copies(&key().connection).await.unwrap();
            assert!(
                copies.iter().any(|copy| copy.page.body == CRASH_U1),
                "{stage}"
            );
            assert!(
                copies.iter().any(|copy| copy.page.body == CRASH_CANONICAL),
                "{stage}"
            );
            if matches!(stage, "leased" | "unknown") {
                let old = store.unknown_mutations(&key().connection).await.unwrap();
                assert_eq!(old.len(), 1);
                assert_eq!(old[0].kind, MutationKind::Put(CRASH_CANONICAL.into()));
                assert!(matches!(
                    store.replay_editor_handoff(&key()).await,
                    Err(StoreError::UnknownOperation)
                ));
                store
                    .acknowledge(
                        old[0].id,
                        MutationAck {
                            remote_revision: Some(RemoteRevision("\"late-old-put\"".into())),
                        },
                    )
                    .await
                    .unwrap();
                assert_eq!(
                    store
                        .load_editor_page(&key())
                        .await
                        .unwrap()
                        .unwrap()
                        .presented()
                        .body,
                    CRASH_U2
                );
                assert!(
                    store
                        .load_sync_page(&key())
                        .await
                        .unwrap()
                        .unwrap()
                        .1
                        .pending,
                    "old completion falsely acknowledged U2"
                );
            }
            if stage != "after-replay" {
                let chosen = store.replay_editor_handoff(&key()).await.unwrap().unwrap();
                assert_eq!(
                    chosen.page.body,
                    if stage == "after-admission" {
                        "blue fox runs\n\nstable\n"
                    } else {
                        CRASH_MERGED
                    }
                );
            }
            assert!(store.replay_editor_handoff(&key()).await.unwrap().is_none());
            let chosen = store.load_page(&key()).await.unwrap().unwrap();
            let operation = store
                .claim_pending(&key().connection, 100)
                .await
                .unwrap()
                .into_iter()
                .find(|operation| operation.key == key())
                .unwrap();
            assert_eq!(operation.kind, MutationKind::Put(chosen.body.clone()));
            store
                .acknowledge(
                    operation.id,
                    MutationAck {
                        remote_revision: Some(RemoteRevision("\"exact-chosen-ack\"".into())),
                    },
                )
                .await
                .unwrap();
            assert!(
                !store
                    .load_sync_page(&key())
                    .await
                    .unwrap()
                    .unwrap()
                    .1
                    .pending
            );
        }
        store.shutdown().await.unwrap();
    }
}

#[test]
#[ignore = "finite crash helper; parent validates nonce/PID and kills only this owned process"]
fn editor_process_boundary_child() {
    use bulletnotes::store::StoreHandle;
    let path = std::path::PathBuf::from(std::env::var_os("BULLETNOTES_CRASH_PATH").unwrap());
    assert!(
        path.parent()
            .unwrap()
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("bulletnotes-editor-crash-")
    );
    let stage = std::env::var("BULLETNOTES_CRASH_STAGE").unwrap();
    let nonce = std::env::var("BULLETNOTES_CRASH_NONCE").unwrap();
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let store = StoreHandle::open(&path).await.unwrap();
        store
            .apply_remote(RemoteRead::Present(RemoteFile {
                key: key(),
                body: CRASH_BASE.into(),
                revision: Some(RemoteRevision("\"actual-http-base\"".into())),
                modified_ms: None,
                permission: Permission::ReadWrite,
            }))
            .await
            .unwrap();
        store
            .save_local(key(), CRASH_CANONICAL.into(), LocalRevision(1))
            .await
            .unwrap();
        if matches!(stage.as_str(), "leased" | "unknown") {
            let old = store
                .claim_pending(&key().connection, 1)
                .await
                .unwrap()
                .remove(0);
            if stage == "unknown" {
                store.mark_unknown(old.id).await.unwrap();
            }
        }
        let admission = |store: StoreHandle| async move {
            store
                .save_local_from_base_outcome(
                    key(),
                    CRASH_U1.into(),
                    LocalRevision(2),
                    EditorBaseline {
                        body: CRASH_BASE.into(),
                        remote_base: store.load_page(&key()).await.unwrap().unwrap().remote_base,
                    },
                )
                .await
                .unwrap();
        };
        let blocker = if stage == "before-admission" {
            let db = rusqlite::Connection::open(&path).unwrap();
            db.execute_batch("BEGIN IMMEDIATE").unwrap();
            tokio::spawn(admission(store.clone()));
            Some(db)
        } else {
            admission(store.clone()).await;
            if stage != "after-admission" {
                store
                    .save_local_from_base_outcome(
                        key(),
                        CRASH_U2.into(),
                        LocalRevision(3),
                        EditorBaseline {
                            body: CRASH_U1.into(),
                            remote_base: store
                                .load_page(&key())
                                .await
                                .unwrap()
                                .unwrap()
                                .remote_base,
                        },
                    )
                    .await
                    .unwrap();
            }
            if stage == "after-replay" {
                store.replay_editor_handoff(&key()).await.unwrap().unwrap();
            }
            if stage == "before-replay" {
                let db = rusqlite::Connection::open(&path).unwrap();
                db.execute_batch("BEGIN IMMEDIATE").unwrap();
                let replay = store.clone();
                tokio::spawn(async move {
                    let _ = replay.replay_editor_handoff(&key()).await;
                });
                Some(db)
            } else {
                None
            }
        };
        // The lock proves these started operations cannot commit before the parent kills us.
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
