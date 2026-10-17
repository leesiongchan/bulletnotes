use bulletnotes::{
    app::coordinator::{AppAction, AppEffect, AppEvent, Coordinator},
    domain::{connection::ConnectionConfig, error::StoreError, page::*},
};
fn key() -> PageKey {
    PageKey {
        connection: ConnectionConfig::parse("https://notes.example")
            .unwrap()
            .id()
            .clone(),
        path: PagePath::parse("a.md").unwrap(),
    }
}
fn app() -> Coordinator {
    let mut app = Coordinator::new(key().connection, key);
    app.dispatch(AppAction::NewNote);
    app.receive(AppEvent::LocalSaved(SavedRevision {
        key: key(),
        local_revision: LocalRevision(0),
    }));
    app
}
#[test]
fn close_flushes_locally_before_hiding_without_a_network_dependency() {
    let mut app = app();
    app.dispatch(AppAction::EditText("keep me".into()));
    let effects = app.dispatch(AppAction::RequestHide);
    assert!(
        effects
            .iter()
            .any(|effect| matches!(effect, AppEffect::SaveLocal { .. }))
    );
    assert!(
        !effects
            .iter()
            .any(|effect| matches!(effect, AppEffect::HideReady))
    );
    let effects = app.receive(AppEvent::LocalSaved(SavedRevision {
        key: key(),
        local_revision: LocalRevision(1),
    }));
    assert!(
        effects
            .iter()
            .any(|effect| matches!(effect, AppEffect::HideReady))
    );
    assert!(
        !effects
            .iter()
            .any(|effect| matches!(effect, AppEffect::QuitReady))
    );
}
#[test]
fn failed_save_cancels_hide_and_quit_and_retains_the_buffer() {
    for hide in [true, false] {
        let mut app = app();
        app.dispatch(AppAction::EditText("copy this unsaved text".into()));
        app.dispatch(if hide {
            AppAction::RequestHide
        } else {
            AppAction::RequestQuit
        });
        let effects = app.receive(AppEvent::LocalSaveFailed(
            key(),
            LocalRevision(1),
            StoreError::Closed,
        ));
        assert!(
            !effects
                .iter()
                .any(|effect| matches!(effect, AppEffect::HideReady | AppEffect::QuitReady))
        );
        assert_eq!(app.editor().unwrap().body(), "copy this unsaved text");
        assert!(app.editor().unwrap().dirty());
        let effects = app.dispatch(AppAction::Flush);
        assert!(
            effects
                .iter()
                .any(|effect| matches!(effect, AppEffect::SaveLocal { .. }))
        );
    }
}
#[test]
fn new_drafts_are_recorded_as_the_last_visited_note_after_creation_acknowledges() {
    let mut app = Coordinator::new(key().connection, key);
    let effects = app.dispatch(AppAction::NewNote);
    assert!(
        !effects
            .iter()
            .any(|effect| matches!(effect, AppEffect::RecordVisit(_)))
    );
    let effects = app.receive(AppEvent::LocalSaved(SavedRevision {
        key: key(),
        local_revision: LocalRevision(0),
    }));
    assert!(
        effects
            .iter()
            .any(|effect| matches!(effect, AppEffect::RecordVisit(page) if page==&key()))
    );
}
#[test]
#[ignore = "opens generated local application; build app_lifecycle_review first"]
fn native_startup_close_and_termination_preserve_local_data() {
    let executable = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("examples/app_lifecycle_review");
    let output = std::process::Command::new(executable)
        .env("SLINT_BACKEND", "winit-femtovg")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("native_startup_close_and_quit: true")
    );
}
