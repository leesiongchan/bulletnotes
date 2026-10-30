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
    deleted_buffer_is_replaced(false);
}
#[test]
fn late_batch_completion_cannot_leave_a_trashed_buffer_after_navigation() {
    deleted_buffer_is_replaced(true);
}
fn deleted_buffer_is_replaced(navigate_away: bool) {
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
    if navigate_away {
        app.dispatch(AppAction::Navigate(View::Appearance));
    }
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
    if navigate_away {
        let _ = checked(&mut app, effects);
        assert_eq!(app.view(), View::Appearance);
        assert_ne!(app.editor().map(|e| e.body()), Some("original buffer"));
        return;
    }
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

#[test]
fn bound_browser_selects_moves_trashes_and_remembers_card_view() {
    use bulletnotes::{
        AppWindow,
        app::{appearance_ui, local_ui},
        store::{StoreHandle, settings::SearchView},
        theme::SystemAppearance,
    };
    use slint::{ComponentHandle, Model};
    use std::{cell::Cell, rc::Rc, time::Duration};
    slint::platform::set_platform(Box::new(i_slint_backend_testing::TestingBackend::new(
        i_slint_backend_testing::TestingBackendOptions {
            mock_time: false,
            threading: true,
            renderer_name: Some("skia".into()),
        },
    )))
    .unwrap();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let _entered = runtime.enter();
    let done = Rc::new(Cell::new(false));
    let finished = done.clone();
    slint::spawn_local(async move {
        async fn until(predicate: impl Fn() -> bool) {
            for _ in 0..500 {
                if predicate() {
                    return;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            panic!("search UI did not settle");
        }
        let directory = tempfile::tempdir().unwrap();
        let store = StoreHandle::open(&support::storage_fixture::database(&directory))
            .await
            .unwrap();
        for name in ["a", "b", "c"] {
            let page = key(&format!("Quick Notes/{name}.md"));
            store.create_draft(page.clone()).await.unwrap();
            store
                .save_local(page, format!("# Header {name}\nbody"), LocalRevision(1))
                .await
                .unwrap();
        }
        let window = AppWindow::new().unwrap();
        let appearance = appearance_ui::bind(
            &window,
            store.clone(),
            Default::default(),
            SystemAppearance::Light,
        )
        .unwrap();
        let local = local_ui::bind(
            &window,
            store.clone(),
            key("a.md").connection,
            appearance.clone(),
        )
        .unwrap();
        window.show().unwrap();
        window.set_search_scratchpads(true);
        window.invoke_browse_pages();
        local.refresh_scratchpads().await.unwrap();
        local.search_current().await.unwrap();
        assert_eq!(window.get_page_results().row_count(), 3);
        let initial = window.get_page_results();
        let first = initial.row_data(0).unwrap().key;
        let second = initial.row_data(1).unwrap().key;
        let last = initial.row_data(2).unwrap().key;
        window.set_search_selecting(true);
        window.invoke_search_select_toggled();
        window.invoke_search_selection_toggled(first.clone());
        window.invoke_search_selection_toggled(second);
        assert_eq!(window.get_search_selected_count(), 2);
        assert!(window.get_page_results().row_data(0).unwrap().checked);
        window.invoke_search_drag_started(first);
        window.invoke_search_reordered(last, true);
        until(|| !window.get_search_busy()).await;
        local.search_current().await.unwrap();
        let stack = store
            .scratchpad_stack(&key("a.md").connection)
            .await
            .unwrap();
        assert_eq!(
            stack
                .iter()
                .map(|e| e.key.path.as_str())
                .collect::<Vec<_>>(),
            vec!["Quick Notes/a.md", "Quick Notes/c.md", "Quick Notes/b.md"]
        );
        window.invoke_search_promote_selected();
        until(|| !window.get_search_busy()).await;
        local.search_current().await.unwrap();
        let promoted = store
            .scratchpad_stack(&key("a.md").connection)
            .await
            .unwrap();
        assert_eq!(
            promoted
                .iter()
                .map(|e| e.key.path.as_str())
                .collect::<Vec<_>>(),
            vec!["Quick Notes/c.md", "Quick Notes/b.md", "Quick Notes/a.md"]
        );
        window.invoke_search_trash_selected();
        until(|| window.get_search_confirm_count() == 2).await;
        window.invoke_search_trash_confirmed(window.get_search_confirm_id());
        until(|| !window.get_search_busy()).await;
        local.search_current().await.unwrap();
        assert_eq!(window.get_page_results().row_count(), 1);
        assert_eq!(window.get_search_selected_count(), 0);
        assert_eq!(store.trash(&key("a.md").connection).await.unwrap().len(), 2);
        window.set_search_cards(true);
        window.invoke_search_view_changed(true);
        for _ in 0..100 {
            if store.load_settings().await.unwrap().search_view == SearchView::Cards {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        appearance.flush().await.unwrap();
        assert_eq!(
            store.load_settings().await.unwrap().search_view,
            SearchView::Cards
        );
        window.hide().unwrap();
        store.shutdown().await.unwrap();
        finished.set(true);
        slint::quit_event_loop().unwrap();
    })
    .unwrap();
    slint::run_event_loop_until_quit().unwrap();
    assert!(done.get());
}
