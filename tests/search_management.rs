mod support;
use bulletnotes::{
    app::{
        coordinator::*,
        model::View,
        recovery::{RecoveryIntent, RecoveryOutcome},
    },
    domain::page::LocalRevision,
};
use support::storage_fixture::key;

fn checked(app: &mut Coordinator, effects: Vec<AppEffect>) -> Vec<AppEffect> {
    effects
        .into_iter()
        .flat_map(|effect| match effect {
            AppEffect::CheckTransition {
                request_id,
                connection,
            } => app.receive(AppEvent::TransitionChecked {
                request_id,
                connection,
                result: Ok(()),
            }),
            effect => vec![effect],
        })
        .collect()
}

#[test]
fn search_trash_confirmation_pins_revisions_and_requires_matching_id() {
    let mut app = Coordinator::new(key("a.md").connection, || key("Quick Notes/new.md"));
    app.dispatch(AppAction::Navigate(View::AllPages));
    let effects = app.dispatch(AppAction::PrepareSearchTrash(vec![key("a.md")]));
    let effects = checked(&mut app, effects);
    let (id, connection) = effects
        .into_iter()
        .find_map(|e| match e {
            AppEffect::Recovery {
                request_id,
                connection,
                intent: RecoveryIntent::PrepareTrash(keys),
            } => {
                assert_eq!(keys, vec![key("a.md")]);
                Some((request_id, connection))
            }
            _ => None,
        })
        .unwrap();
    app.receive(AppEvent::RecoveryFinished {
        request_id: id,
        connection,
        result: Ok(RecoveryOutcome {
            affected: vec![],
            page: None,
            restored: None,
            prepared_trash: Some(vec![(key("a.md"), LocalRevision(7))]),
            stack: None,
        }),
    });
    assert_eq!(app.search_management.confirmation(), Some((id, 1)));
    assert!(
        app.dispatch(AppAction::ConfirmSearchTrash(id + 1))
            .is_empty()
    );
    let effects = app.dispatch(AppAction::ConfirmSearchTrash(id));
    let effects = checked(&mut app, effects);
    assert!(effects.iter().any(|e| matches!(e,AppEffect::Recovery {intent:RecoveryIntent::TrashMany(pins),..} if pins==&vec![(key("a.md"),LocalRevision(7))])));
    assert_eq!(app.view(), View::AllPages);
}

#[test]
fn selection_is_key_based_and_refresh_prunes_only_unavailable_keys() {
    let mut state = bulletnotes::app::search_management::SearchManagement::default();
    state.toggle(key("a.md"));
    state.toggle(key("b.md"));
    state.retain_loaded(&[key("a.md"), key("b.md"), key("c.md")]);
    assert_eq!(state.selected().len(), 2);
    state.retain_loaded(&[key("b.md")]);
    assert!(state.selected().contains(&key("b.md")));
    assert_eq!(state.selected().len(), 1);
    state.clear();
    assert!(state.selected().is_empty());
}

#[test]
fn deleting_mounted_note_and_all_neighbors_keeps_search_and_replaces_buffer() {
    use bulletnotes::{
        domain::page::{PageSnapshot, Permission},
        store::scratchpads::ScratchpadEntry,
    };
    let current = key("Quick Notes/current.md");
    let other = key("Quick Notes/other.md");
    let mut app = Coordinator::new(current.connection.clone(), || key("Quick Notes/new.md"));
    let effects = app.dispatch(AppAction::OpenPage(current.clone()));
    let effects = checked(&mut app, effects);
    let id = effects
        .iter()
        .find_map(|e| match e {
            AppEffect::LoadPage { request_id, .. } => Some(*request_id),
            _ => None,
        })
        .unwrap();
    app.receive(AppEvent::PageLoaded {
        request_id: id,
        key: current.clone(),
        result: Ok(Some(
            PageSnapshot {
                key: current.clone(),
                body: "original buffer".into(),
                local_revision: LocalRevision(0),
                remote_base: None,
                permission: Permission::ReadWrite,
                position: Default::default(),
            }
            .into(),
        )),
    });
    let effects = app.dispatch(AppAction::RefreshScratchpads);
    let (id, connection) = effects
        .into_iter()
        .find_map(|e| match e {
            AppEffect::LoadScratchpads {
                request_id,
                connection,
            } => Some((request_id, connection)),
            _ => None,
        })
        .unwrap();
    app.receive(AppEvent::ScratchpadsLoaded {
        request_id: id,
        connection,
        result: Ok(vec![
            ScratchpadEntry {
                key: current.clone(),
                created_ms: 2,
            },
            ScratchpadEntry {
                key: other.clone(),
                created_ms: 1,
            },
        ]),
    });
    app.dispatch(AppAction::Navigate(View::AllPages));
    let effects = app.dispatch(AppAction::PrepareSearchTrash(vec![
        current.clone(),
        other.clone(),
    ]));
    let effects = checked(&mut app, effects);
    let (id, connection) = effects
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
        request_id: id,
        connection: connection.clone(),
        result: Ok(RecoveryOutcome {
            affected: vec![],
            page: None,
            restored: None,
            prepared_trash: Some(vec![
                (current.clone(), LocalRevision(0)),
                (other.clone(), LocalRevision(0)),
            ]),
            stack: None,
        }),
    });
    let effects = app.dispatch(AppAction::ConfirmSearchTrash(id));
    let effects = checked(&mut app, effects);
    let id = effects
        .iter()
        .find_map(|e| match e {
            AppEffect::Recovery { request_id, .. } => Some(*request_id),
            _ => None,
        })
        .unwrap();
    let effects = app.receive(AppEvent::RecoveryFinished {
        request_id: id,
        connection: connection.clone(),
        result: Ok(RecoveryOutcome {
            affected: vec![current, other],
            page: None,
            restored: None,
            prepared_trash: None,
            stack: None,
        }),
    });
    assert!(app.editor().is_none());
    let id = effects
        .iter()
        .find_map(|e| match e {
            AppEffect::LoadScratchpads { request_id, .. } => Some(*request_id),
            _ => None,
        })
        .unwrap();
    let effects = app.receive(AppEvent::ScratchpadsLoaded {
        request_id: id,
        connection,
        result: Ok(vec![]),
    });
    let _ = checked(&mut app, effects);
    assert_eq!(app.view(), View::AllPages);
    assert_eq!(app.editor().unwrap().body(), "");
    assert_eq!(app.editor().unwrap().key(), &key("Quick Notes/new.md"));
}

#[test]
fn create_from_search_seeds_new_scratchpad_and_rejects_blank_query() {
    let mut app = Coordinator::new(key("a.md").connection, || key("Quick Notes/new.md"));
    assert!(
        app.dispatch(AppAction::NewNoteFromSearch(" \n".into()))
            .is_empty()
    );
    let effects = app.dispatch(AppAction::NewNoteFromSearch("# literal\r\n日本語".into()));
    let _ = checked(&mut app, effects);
    assert_eq!(app.editor().unwrap().body(), "# literal\r\n日本語");
    assert_eq!(app.editor().unwrap().key(), &key("Quick Notes/new.md"));
}
