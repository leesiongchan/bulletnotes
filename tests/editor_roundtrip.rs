use bulletnotes::{
    app::{editor::EditorSession, model::EditorPosition},
    domain::{connection::ConnectionConfig, page::*},
    store::StoreHandle,
};
use std::time::Duration;
fn key() -> PageKey {
    PageKey {
        connection: ConnectionConfig::parse("https://notes.example")
            .unwrap()
            .id()
            .clone(),
        path: PagePath::parse("Journal/50% café.md").unwrap(),
    }
}
fn page(body: &str) -> PageSnapshot {
    PageSnapshot {
        key: key(),
        body: body.into(),
        local_revision: LocalRevision(0),
        remote_base: None,
        position: Default::default(),
        permission: Permission::ReadWrite,
    }
}
#[test]
fn unknown_native_selection_is_never_fabricated_from_a_restore_target() {
    let session = EditorSession::restore(page("café"));
    let (body, position) = session.snapshot();
    assert_eq!(body, "café");
    assert_eq!(position, None);
}
#[test]
fn read_only_edit_or_invalid_position_cannot_modify_source() {
    let mut snapshot = page("café");
    snapshot.permission = Permission::ReadOnly;
    let mut session = EditorSession::restore(snapshot);
    assert!(session.edit("new".into(), Duration::ZERO).is_err());
    assert_eq!(session.body(), "café");
    assert!(
        session
            .observe_position(EditorPosition {
                anchor: 4,
                caret: 5,
                scroll_y: 0.0
            })
            .is_err()
    );
}
#[tokio::test]
async fn edit_roundtrip_preserves_markdown_after_restart() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("local.sqlite");
    let source =
        "---\r\ntitle: café\r\n---\r\n\t👩🏽‍💻 日本語\r\n    let x = 1;\r\n[[Unsupported syntax]]\r\n";
    let store = StoreHandle::open(&path).await.unwrap();
    store.create_draft(key()).await.unwrap();
    store
        .save_local(key(), source.into(), LocalRevision(1))
        .await
        .unwrap();
    let saved = store.load_page(&key()).await.unwrap().unwrap();
    let mut editor = EditorSession::restore(saved);
    let changed = source.replacen(
        "    let x = 1;",
        "    let x = 1;\r\n    // inserted café 👩🏽‍💻 日本語",
        1,
    );
    editor.edit(changed.clone(), Duration::ZERO).unwrap();
    let (body, position) = editor.snapshot();
    assert_eq!(position, None);
    store
        .save_local(key(), body, editor.revision())
        .await
        .unwrap();
    store.shutdown().await.unwrap();
    let reopened = StoreHandle::open(&path).await.unwrap();
    let recovered = reopened.load_page(&key()).await.unwrap().unwrap();
    assert_eq!(recovered.body.as_bytes(), changed.as_bytes());
    assert_eq!(recovered.local_revision, LocalRevision(2));
    assert_eq!(
        reopened
            .claim_pending(&key().connection, 10)
            .await
            .unwrap()
            .len(),
        1
    );
    reopened.shutdown().await.unwrap();
}

#[test]
#[ignore = "opens generated-content native UI; build example local_review first"]
fn native_local_callbacks_save_browse_and_recover_exact_text() {
    let executable = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("examples/local_review");
    let output = std::process::Command::new(executable)
        .env("SLINT_BACKEND", "winit-skia")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout)
            .contains("native_local_callbacks_and_restart: true")
    );
}
