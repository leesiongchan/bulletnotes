#[path = "support/transitions.rs"]
mod transitions;
use bulletnotes::{
    app::coordinator::{AppAction, AppEffect, AppEvent, Coordinator},
    domain::{connection::ConnectionConfig, error::StoreError, page::*},
};
use transitions::ReadyTransitions;
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
    app.dispatch_ready(AppAction::NewNote);
    app.receive_ready(AppEvent::LocalSaved {
        expected: app.editor().unwrap().rebase_token(),
        outcome: {
            let attempt = SavedRevision {
                key: key(),
                local_revision: LocalRevision(0),
            };
            let mut page = app.editor().unwrap().rebase_input().1;
            page.local_revision = attempt.local_revision;
            bulletnotes::domain::reconciliation::LocalSaveOutcome {
                attempt,
                stored: page.into(),
            }
        },
    });
    app
}
#[test]
fn close_flushes_locally_before_hiding_without_a_network_dependency() {
    let mut app = app();
    app.dispatch_ready(AppAction::EditText("keep me".into()));
    let effects = app.dispatch_ready(AppAction::RequestHide);
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
    let effects = app.receive_ready(AppEvent::LocalSaved {
        expected: app.editor().unwrap().rebase_token(),
        outcome: {
            let attempt = SavedRevision {
                key: key(),
                local_revision: LocalRevision(1),
            };
            let mut page = app.editor().unwrap().rebase_input().1;
            page.local_revision = attempt.local_revision;
            bulletnotes::domain::reconciliation::LocalSaveOutcome {
                attempt,
                stored: page.into(),
            }
        },
    });
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
        app.dispatch_ready(AppAction::EditText("copy this unsaved text".into()));
        app.dispatch_ready(if hide {
            AppAction::RequestHide
        } else {
            AppAction::RequestQuit
        });
        let effects = app.receive_ready(AppEvent::LocalSaveFailed(
            app.editor().unwrap().rebase_token(),
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
        let effects = app.dispatch_ready(AppAction::Flush);
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
    let effects = app.dispatch_ready(AppAction::NewNote);
    assert!(
        !effects
            .iter()
            .any(|effect| matches!(effect, AppEffect::RecordVisit(_)))
    );
    let effects = app.receive_ready(AppEvent::LocalSaved {
        expected: app.editor().unwrap().rebase_token(),
        outcome: {
            let attempt = SavedRevision {
                key: key(),
                local_revision: LocalRevision(0),
            };
            let mut page = app.editor().unwrap().rebase_input().1;
            page.local_revision = attempt.local_revision;
            bulletnotes::domain::reconciliation::LocalSaveOutcome {
                attempt,
                stored: page.into(),
            }
        },
    });
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
        .env("SLINT_BACKEND", "winit-skia")
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

#[cfg(target_os = "macos")]
#[test]
fn hotkey_conflict_preserves_the_previous_registration() {
    use bulletnotes::platform::hotkey::{HotkeyBackend, HotkeyError, HotkeyRegistration};
    use global_hotkey::hotkey::{Code, HotKey, Modifiers};
    use std::{cell::RefCell, rc::Rc};
    #[derive(Default)]
    struct Fake {
        registered: RefCell<Vec<HotKey>>,
        busy: RefCell<Option<HotKey>>,
    }
    impl HotkeyBackend for Fake {
        fn register(&self, key: HotKey) -> Result<(), HotkeyError> {
            if self.busy.borrow().as_ref() == Some(&key) {
                return Err(HotkeyError::Unavailable);
            }
            self.registered.borrow_mut().push(key);
            Ok(())
        }
        fn unregister(&self, key: HotKey) -> Result<(), HotkeyError> {
            self.registered
                .borrow_mut()
                .retain(|registered| *registered != key);
            Ok(())
        }
    }
    let backend = Rc::new(Fake::default());
    let old = HotKey::new(Some(Modifiers::ALT), Code::KeyA);
    let busy = HotKey::new(Some(Modifiers::ALT), Code::KeyB);
    *backend.busy.borrow_mut() = Some(busy);
    let mut binding = HotkeyRegistration::new(backend.clone());
    binding.commit(binding.prepare(old).unwrap()).unwrap();
    assert!(binding.prepare(busy).is_err());
    assert_eq!(binding.current(), Some(old));
    assert_eq!(*backend.registered.borrow(), vec![old]);
    let candidate = HotKey::new(Some(Modifiers::ALT), Code::KeyC);
    {
        let _abandoned = binding.prepare(candidate).unwrap();
        assert_eq!(binding.current(), Some(old));
    }
    assert_eq!(
        *backend.registered.borrow(),
        vec![old],
        "abandoned settings commit must release only its reserved candidate"
    );
    let stale = binding.prepare(old).unwrap();
    binding.commit(binding.prepare(candidate).unwrap()).unwrap();
    assert!(
        binding.commit(stale).is_err(),
        "a stale reservation cannot undo a later committed shortcut"
    );
    assert_eq!(binding.current(), Some(candidate));
    assert_eq!(*backend.registered.borrow(), vec![candidate]);
    drop(binding);
    assert!(backend.registered.borrow().is_empty());
}
#[tokio::test]
async fn a_contested_store_identifies_its_owner_without_opening_a_second_writer() {
    use bulletnotes::store::{StoreHandle, owner_process_id};
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("notes.sqlite");
    let owner = StoreHandle::open(&path).await.unwrap();
    assert!(matches!(
        StoreHandle::open(&path).await,
        Err(StoreError::AlreadyRunning)
    ));
    assert_eq!(owner_process_id(&path).unwrap(), std::process::id());
    owner.shutdown().await.unwrap();
}
