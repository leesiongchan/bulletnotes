use bulletnotes::{
    app::{coordinator::*, scratchpads::ScratchpadStack},
    domain::{connection::ConnectionConfig, error::StoreError, page::*},
    store::scratchpads::ScratchpadEntry,
};
fn key(name: &str) -> PageKey {
    PageKey {
        connection: ConnectionConfig::parse("https://stack.example")
            .unwrap()
            .id()
            .clone(),
        path: PagePath::parse(name).unwrap(),
    }
}
fn entries() -> Vec<ScratchpadEntry> {
    ["Quick Notes/c.md", "Quick Notes/b.md", "Quick Notes/a.md"]
        .into_iter()
        .map(|path| ScratchpadEntry {
            key: key(path),
            created_ms: 1,
        })
        .collect()
}
fn checked(app: &mut Coordinator, effects: Vec<AppEffect>) -> Vec<AppEffect> {
    effects
        .into_iter()
        .flat_map(|e| match e {
            AppEffect::CheckTransition {
                request_id,
                connection,
            } => app.receive(AppEvent::TransitionChecked {
                request_id,
                connection,
                result: Ok(()),
            }),
            e => vec![e],
        })
        .collect()
}
fn open(app: &mut Coordinator, path: &str) {
    let effects = app.dispatch(AppAction::OpenPage(key(path)));
    let effects = checked(app, effects);
    let id = effects
        .iter()
        .find_map(|e| match e {
            AppEffect::LoadPage { request_id, .. } => Some(*request_id),
            _ => None,
        })
        .unwrap();
    app.receive(AppEvent::PageLoaded {
        request_id: id,
        key: key(path),
        result: Ok(Some(
            PageSnapshot {
                key: key(path),
                body: "source\r\n🙂".into(),
                local_revision: LocalRevision(0),
                remote_base: None,
                position: Default::default(),
                permission: Permission::ReadWrite,
            }
            .into(),
        )),
    });
}
fn refresh(app: &mut Coordinator) -> (u64, bulletnotes::domain::connection::ConnectionId) {
    app.dispatch(AppAction::RefreshScratchpads)
        .into_iter()
        .find_map(|e| match e {
            AppEffect::LoadScratchpads {
                request_id,
                connection,
            } => Some((request_id, connection)),
            _ => None,
        })
        .unwrap()
}
fn app() -> Coordinator {
    let mut app = Coordinator::new(key("x.md").connection, || key("Quick Notes/new.md"));
    open(&mut app, "Quick Notes/c.md");
    let (request_id, connection) = refresh(&mut app);
    app.receive(AppEvent::ScratchpadsLoaded {
        request_id,
        connection,
        result: Ok(entries()),
    });
    app
}
#[test]
fn older_newer_newest_follow_creation_order() {
    let stack = ScratchpadStack::new(entries());
    assert_eq!(stack.position(&key("Quick Notes/c.md")), Some(1));
    assert_eq!(
        stack.older(&key("Quick Notes/b.md")),
        Some(key("Quick Notes/a.md"))
    );
    assert_eq!(
        stack.newer(&key("Quick Notes/b.md")),
        Some(key("Quick Notes/c.md"))
    );
    assert!(stack.newer(&key("Quick Notes/c.md")).is_none());
    assert_eq!(stack.newest(), Some(key("Quick Notes/c.md")));
}
#[test]
fn ordinary_pages_disable_relative_navigation() {
    let mut app = app();
    open(&mut app, "ordinary.md");
    assert!(!app.can_step_scratchpad(-1));
    assert!(app.dispatch(AppAction::StepScratchpad(-1)).is_empty());
}
#[test]
fn loading_is_not_empty() {
    let mut app = Coordinator::new(key("x.md").connection, || key("Quick Notes/new.md"));
    let effects = app.dispatch(AppAction::JumpToNewest);
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, AppEffect::LoadScratchpads { .. }))
    );
    assert!(app.editor().is_none());
    let (request_id, connection) = effects
        .into_iter()
        .find_map(|e| match e {
            AppEffect::LoadScratchpads {
                request_id,
                connection,
            } => Some((request_id, connection)),
            _ => None,
        })
        .unwrap();
    let effects = app.receive(AppEvent::ScratchpadsLoaded {
        request_id,
        connection,
        result: Ok(vec![]),
    });
    let effects = checked(&mut app, effects);
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, AppEffect::CreateDraft { .. }))
    );
}
#[test]
fn rapid_steps_use_the_last_requested_destination() {
    let mut app = app();
    app.dispatch(AppAction::StepScratchpad(-1));
    let effects = app.dispatch(AppAction::StepScratchpad(-1));
    let effects = checked(&mut app, effects);
    assert!(effects.iter().any(|e|matches!(e,AppEffect::LoadPage {key:destination,..} if destination==&key("Quick Notes/a.md"))));
}
#[test]
fn metadata_failure_blocks_transition_without_replacing_editor() {
    let mut app = app();
    let effects = app.dispatch(AppAction::StepScratchpad(-1));
    let (request_id, connection) = effects
        .into_iter()
        .find_map(|e| match e {
            AppEffect::CheckTransition {
                request_id,
                connection,
            } => Some((request_id, connection)),
            _ => None,
        })
        .unwrap();
    assert!(
        app.receive(AppEvent::TransitionChecked {
            request_id,
            connection,
            result: Err(StoreError::Closed)
        })
        .is_empty()
    );
    assert_eq!(app.editor().unwrap().key(), &key("Quick Notes/c.md"));
    assert_eq!(app.editor().unwrap().body(), "source\r\n🙂");
    assert!(app.notice().is_some());
}
#[test]
fn late_barrier_cannot_open_an_old_destination() {
    let mut app = app();
    let effects = app.dispatch(AppAction::StepScratchpad(-1));
    app.dispatch(AppAction::JumpToNewest);
    assert!(checked(&mut app, effects).is_empty());
    assert_eq!(app.editor().unwrap().key(), &key("Quick Notes/c.md"));
}
#[test]
fn stale_generation_is_rejected_after_connection_roundtrip() {
    let mut app = app();
    let (request_id, connection) = refresh(&mut app);
    let other = ConnectionConfig::parse("https://other.example")
        .unwrap()
        .id()
        .clone();
    app.change_connection(other, || key("Quick Notes/unused.md"));
    app.change_connection(connection.clone(), || key("Quick Notes/new.md"));
    app.receive(AppEvent::ScratchpadsLoaded {
        request_id,
        connection,
        result: Ok(entries()),
    });
    assert!(app.scratchpads().is_none());
}

#[test]
fn edit_during_barrier_reenters_local_save() {
    let mut app = app();
    let effects = app.dispatch(AppAction::StepScratchpad(-1));
    let (request_id, connection) = effects
        .into_iter()
        .find_map(|e| match e {
            AppEffect::CheckTransition {
                request_id,
                connection,
            } => Some((request_id, connection)),
            _ => None,
        })
        .unwrap();
    app.dispatch(AppAction::EditText("new source".into()));
    let effects = app.receive(AppEvent::TransitionChecked {
        request_id,
        connection,
        result: Ok(()),
    });
    let expected = effects
        .into_iter()
        .find_map(|e| match e {
            AppEffect::SaveLocal { expected, .. } => Some(expected),
            _ => None,
        })
        .unwrap();
    let mut page = app.editor().unwrap().rebase_input().1;
    page.body = "new source".into();
    page.local_revision = LocalRevision(1);
    let effects = app.receive(AppEvent::LocalSaved {
        expected,
        outcome: bulletnotes::domain::reconciliation::LocalSaveOutcome {
            attempt: SavedRevision {
                key: page.key.clone(),
                local_revision: page.local_revision,
            },
            stored: page.into(),
        },
    });
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, AppEffect::CheckTransition { .. }))
    );
    assert!(
        !effects
            .iter()
            .any(|e| matches!(e, AppEffect::LoadPage { .. }))
    );
}
fn deleted(app: &mut Coordinator) -> Vec<AppEffect> {
    let effects = app.dispatch(AppAction::MoveToTrash(
        key("Quick Notes/c.md"),
        LocalRevision(0),
    ));
    let effects = checked(app, effects);
    let (request_id, connection) = effects
        .into_iter()
        .find_map(|e| match e {
            AppEffect::Recovery {
                request_id,
                connection,
                ..
            } => Some((request_id, connection)),
            _ => None,
        })
        .unwrap();
    app.receive(AppEvent::RecoveryFinished {
        request_id,
        connection,
        result: Ok(bulletnotes::app::recovery::RecoveryOutcome {
            affected: Some(key("Quick Notes/c.md")),
            page: None,
            restored: None,
        }),
    })
}
#[test]
fn deletion_chooses_older_and_a_later_intent_cancels_its_followup() {
    for navigate_away in [false, true] {
        let mut app = app();
        let effects = deleted(&mut app);
        let (request_id, connection) = effects
            .into_iter()
            .find_map(|e| match e {
                AppEffect::LoadScratchpads {
                    request_id,
                    connection,
                } => Some((request_id, connection)),
                _ => None,
            })
            .unwrap();
        if navigate_away {
            app.dispatch(AppAction::Navigate(bulletnotes::app::model::View::AllPages));
        }
        let effects = app.receive(AppEvent::ScratchpadsLoaded {
            request_id,
            connection,
            result: Ok(entries().into_iter().skip(1).collect()),
        });
        if navigate_away {
            assert!(effects.iter().all(|e| !matches!(
                e,
                AppEffect::CheckTransition { .. } | AppEffect::LoadPage { .. }
            )));
        } else {
            let effects = checked(&mut app, effects);
            assert!(effects.iter().any(|e|matches!(e,AppEffect::LoadPage {key:target,..} if target==&key("Quick Notes/b.md"))));
        }
    }
}
#[test]
fn uncached_stack_destination_keeps_editor_until_download_succeeds() {
    let mut app = app();
    let effects = app.dispatch(AppAction::StepScratchpad(-1));
    let effects = checked(&mut app, effects);
    let (request_id, target) = effects
        .into_iter()
        .find_map(|e| match e {
            AppEffect::LoadPage { request_id, key } => Some((request_id, key)),
            _ => None,
        })
        .unwrap();
    app.receive(AppEvent::PageLoaded {
        request_id,
        key: target.clone(),
        result: Ok(None),
    });
    assert_eq!(app.editor().unwrap().key(), &key("Quick Notes/c.md"));
    assert_eq!(app.selected_uncached(), Some(&target));
}

#[test]
fn missing_stack_entry_is_skipped_without_losing_current_note() {
    let mut app = app();
    let effects = app.dispatch(AppAction::StepScratchpad(-1));
    let effects = checked(&mut app, effects);
    let (request_id, key) = effects
        .into_iter()
        .find_map(|e| match e {
            AppEffect::LoadPage { request_id, key } => Some((request_id, key)),
            _ => None,
        })
        .unwrap();
    let effects = app.receive(AppEvent::PageLoaded {
        request_id,
        key,
        result: Err(StoreError::PageMissing),
    });
    let effects = checked(&mut app, effects);
    assert!(effects.iter().any(|e|matches!(e,AppEffect::LoadPage {key:target,..} if target==&crate::key("Quick Notes/a.md"))));
    assert_eq!(app.editor().unwrap().key(), &crate::key("Quick Notes/c.md"));
}
#[test]
fn failed_stack_read_never_becomes_an_empty_stack() {
    let mut app = Coordinator::new(key("x.md").connection, || key("Quick Notes/new.md"));
    let (request_id, connection) = refresh(&mut app);
    let effects = app.receive(AppEvent::ScratchpadsLoaded {
        request_id,
        connection,
        result: Err(StoreError::Closed),
    });
    assert!(app.scratchpads().is_none());
    assert!(app.editor().is_none());
    assert!(
        effects
            .iter()
            .all(|e| !matches!(e, AppEffect::CreateDraft { .. }))
    );
}
