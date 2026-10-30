#[path = "support/transitions.rs"]
mod transitions;
use bulletnotes::{
    app::{
        model::{EditorPosition, View},
        navigation::NavigationState,
    },
    domain::{
        connection::ConnectionConfig,
        page::{PageKey, PagePath},
    },
};
fn key(path: &str) -> PageKey {
    PageKey {
        connection: ConnectionConfig::parse("https://notes.example")
            .unwrap()
            .id()
            .clone(),
        path: PagePath::parse(path).unwrap(),
    }
}

#[test]
fn history_restores_selection() {
    let text = "café 👩🏽‍💻\nsecond line";
    let pos = EditorPosition {
        anchor: 3,
        caret: 5,
        scroll_y: 92.5,
    };
    assert!(pos.validate(text).is_ok());
    let mut nav = NavigationState::default();
    nav.open(key("a.md"), pos);
    nav.search_query = "café".into();
    nav.set_view(View::AllPages);
    nav.set_view(View::Note);
    assert_eq!(nav.position(), Some(pos));
    assert_eq!(nav.search_query, "café");
    nav.open(key("b.md"), EditorPosition::default());
    assert_eq!(nav.back(), Some(key("a.md")));
    assert_eq!(nav.position(), Some(pos));
    assert_eq!(nav.forward(), Some(key("b.md")));
    assert_eq!(nav.back(), Some(key("a.md")));
    assert_eq!(nav.search_query, "café");
}

#[test]
fn invalid_utf8_offsets_or_scroll_are_rejected() {
    for pos in [
        EditorPosition {
            anchor: 1,
            caret: 2,
            scroll_y: 0.0,
        },
        EditorPosition {
            anchor: 0,
            caret: 3,
            scroll_y: 0.0,
        },
        EditorPosition {
            anchor: 0,
            caret: 0,
            scroll_y: f32::NAN,
        },
        EditorPosition {
            anchor: 0,
            caret: 0,
            scroll_y: -1.0,
        },
    ] {
        assert!(pos.validate("é").is_err());
    }
}

#[test]
fn opening_new_page_after_back_discards_forward_history() {
    let mut nav = NavigationState::default();
    nav.open(key("a.md"), EditorPosition::default());
    nav.open(key("b.md"), EditorPosition::default());
    nav.back();
    nav.open(key("c.md"), EditorPosition::default());
    assert_eq!(nav.forward(), None);
    assert_eq!(nav.back(), Some(key("a.md")));
    assert_eq!(nav.back(), None);
}

#[test]
fn reopening_current_page_does_not_duplicate_history() {
    let mut nav = NavigationState::default();
    nav.open(key("a.md"), EditorPosition::default());
    nav.open(key("a.md"), EditorPosition::default());
    assert_eq!(nav.back(), None);
    assert_eq!(nav.forward(), None);
}

#[test]
fn save_position_follows_page_identity_across_visits() {
    let mut nav = NavigationState::default();
    let updated = EditorPosition {
        anchor: 0,
        caret: 3,
        scroll_y: 10.0,
    };
    nav.open(key("a.md"), EditorPosition::default());
    nav.save_position(updated);
    nav.open(key("b.md"), EditorPosition::default());
    nav.back();
    assert_eq!(nav.position(), Some(updated));
}

mod coordination {
    use super::key;
    use crate::transitions::ReadyTransitions;
    use bulletnotes::{
        app::{coordinator::*, model::View},
        domain::{error::StoreError, page::*, status::LocalSaveState},
        store::search::{SearchQuery, SearchResults},
    };
    use std::time::Duration;
    fn page(path: &str, body: &str) -> PageSnapshot {
        PageSnapshot {
            key: key(path),
            body: body.into(),
            local_revision: LocalRevision(0),
            remote_base: None,
            position: Default::default(),
            permission: Permission::ReadWrite,
        }
    }
    fn app() -> Coordinator {
        let mut app = Coordinator::new(key("a.md").connection.clone(), || key("draft.md"));
        let effects = app.dispatch_ready(AppAction::OpenPage(key("a.md")));
        let request = load(&effects);
        app.receive_ready(AppEvent::PageLoaded {
            request_id: request,
            key: key("a.md"),
            result: Ok(Some(page("a.md", "original").into())),
        });
        app
    }
    fn load(effects: &[AppEffect]) -> u64 {
        effects
            .iter()
            .find_map(|effect| {
                if let AppEffect::LoadPage { request_id, .. } = effect {
                    Some(*request_id)
                } else {
                    None
                }
            })
            .expect("load request")
    }
    fn save(
        effects: &[AppEffect],
    ) -> (
        PageKey,
        String,
        LocalRevision,
        bulletnotes::domain::reconciliation::EditorToken,
    ) {
        effects
            .iter()
            .find_map(|effect| {
                if let AppEffect::SaveLocal {
                    key,
                    body,
                    revision,
                    expected,
                    ..
                } = effect
                {
                    Some((key.clone(), body.clone(), *revision, expected.clone()))
                } else {
                    None
                }
            })
            .expect("save request")
    }
    fn ack(
        app: &mut Coordinator,
        write: (
            PageKey,
            String,
            LocalRevision,
            bulletnotes::domain::reconciliation::EditorToken,
        ),
    ) -> Vec<AppEffect> {
        let mut page = app.editor().unwrap().rebase_input().1;
        page.body = write.1;
        page.local_revision = write.2;
        app.receive_ready(AppEvent::LocalSaved {
            expected: write.3,
            outcome: bulletnotes::domain::reconciliation::LocalSaveOutcome {
                attempt: SavedRevision {
                    key: write.0,
                    local_revision: write.2,
                },
                stored: page.into(),
            },
        })
    }
    #[test]
    fn save_is_due_by_250ms_and_only_acknowledgement_is_clean() {
        let mut app = app();
        app.dispatch_ready(AppAction::EditText("changed".into()));
        assert!(
            app.dispatch_ready(AppAction::Tick(Duration::from_millis(249)))
                .is_empty()
        );
        assert_eq!(
            app.editor().unwrap().save_state(),
            LocalSaveState::Dirty(LocalRevision(1))
        );
        let effects = app.dispatch_ready(AppAction::Tick(Duration::from_millis(250)));
        let write = save(&effects);
        assert_eq!(
            app.editor().unwrap().save_state(),
            LocalSaveState::Saving(LocalRevision(1))
        );
        assert!(
            app.dispatch_ready(AppAction::Tick(Duration::from_secs(1)))
                .is_empty()
        );
        ack(&mut app, write);
        assert_eq!(
            app.editor().unwrap().save_state(),
            LocalSaveState::Clean(LocalRevision(1))
        );
    }
    #[test]
    fn continuous_typing_does_not_postpone_durability_forever() {
        let mut app = app();
        app.dispatch_ready(AppAction::EditText("first".into()));
        app.dispatch_ready(AppAction::Tick(Duration::from_millis(240)));
        app.dispatch_ready(AppAction::EditText("latest".into()));
        let write = save(&app.dispatch_ready(AppAction::Tick(Duration::from_millis(250))));
        assert_eq!((write.1, write.2), ("latest".into(), LocalRevision(2)));
    }
    #[test]
    fn page_navigation_flushes_and_waits_for_durability() {
        let mut app = app();
        app.dispatch_ready(AppAction::EditText("keep me".into()));
        let effects = app.dispatch_ready(AppAction::OpenPage(key("b.md")));
        let write = save(&effects);
        assert!(
            !effects
                .iter()
                .any(|effect| matches!(effect, AppEffect::LoadPage { .. }))
        );
        assert_eq!(app.editor().unwrap().key(), &key("a.md"));
        let effects = ack(&mut app, write);
        assert!(load(&effects) > 0);
    }
    #[test]
    fn failed_save_preserves_buffer_and_blocks_implicit_discard_or_quit() {
        let mut app = app();
        app.dispatch_ready(AppAction::EditText("unsaved 日本語".into()));
        let write = save(&app.dispatch_ready(AppAction::RequestQuit));
        let effects = app.receive_ready(AppEvent::LocalSaveFailed(
            write.3,
            write.2,
            StoreError::Closed,
        ));
        assert!(
            !effects
                .iter()
                .any(|effect| matches!(effect, AppEffect::QuitReady))
        );
        assert_eq!(app.editor().unwrap().body(), "unsaved 日本語");
        assert_eq!(
            app.editor().unwrap().save_state(),
            LocalSaveState::Failed(LocalRevision(1))
        );
        let retry = save(&app.dispatch_ready(AppAction::Flush));
        assert_eq!(
            (retry.1, retry.2),
            ("unsaved 日本語".into(), LocalRevision(1))
        );
    }
    #[test]
    fn older_acknowledgement_cannot_mark_newer_edits_saved() {
        let mut app = app();
        app.dispatch_ready(AppAction::EditText("one".into()));
        let first = save(&app.dispatch_ready(AppAction::Flush));
        app.dispatch_ready(AppAction::EditText("two".into()));
        ack(&mut app, first);
        assert_eq!(
            app.editor().unwrap().save_state(),
            LocalSaveState::Dirty(LocalRevision(2))
        );
        let second = save(&app.dispatch_ready(AppAction::Tick(Duration::from_millis(250))));
        assert_eq!(second.1, "two");
    }
    #[test]
    fn late_load_cannot_replace_active_note_including_repeated_page_identity() {
        let mut app = Coordinator::new(key("a.md").connection.clone(), || key("draft.md"));
        let old_a = load(&app.dispatch_ready(AppAction::OpenPage(key("a.md"))));
        let b = load(&app.dispatch_ready(AppAction::OpenPage(key("b.md"))));
        let new_a = load(&app.dispatch_ready(AppAction::OpenPage(key("a.md"))));
        assert!(new_a > b && b > old_a);
        app.receive_ready(AppEvent::PageLoaded {
            request_id: new_a,
            key: key("a.md"),
            result: Ok(Some(page("a.md", "new A").into())),
        });
        app.receive_ready(AppEvent::PageLoaded {
            request_id: b,
            key: key("b.md"),
            result: Ok(Some(page("b.md", "B").into())),
        });
        app.receive_ready(AppEvent::PageLoaded {
            request_id: old_a,
            key: key("a.md"),
            result: Ok(Some(page("a.md", "old A").into())),
        });
        assert_eq!(app.editor().unwrap().body(), "new A");
    }
    #[test]
    fn a_late_search_cannot_replace_newer_results() {
        let mut app = app();
        let query = |text: &str| SearchQuery {
            scope: bulletnotes::store::search::SearchScope::AllPages,
            connection: key("a.md").connection,
            text: text.into(),
            folder: None,
            offset: 0,
            limit: 50,
        };
        app.dispatch_ready(AppAction::Search(SearchRequest {
            id: 1,
            query: query("old"),
        }));
        app.dispatch_ready(AppAction::Search(SearchRequest {
            id: 2,
            query: query("new"),
        }));
        let results = |total| SearchResults {
            items: vec![],
            total,
            indexed: 0,
            catalogue_total: 0,
            complete: true,
        };
        app.receive_ready(AppEvent::SearchReady(2, Ok(results(2))));
        app.receive_ready(AppEvent::SearchReady(1, Ok(results(1))));
        assert_eq!(app.search_results().unwrap().total, 2);
    }
    #[test]
    fn appearance_transition_flushes_without_replacing_editor() {
        let mut app = app();
        app.dispatch_ready(AppAction::EditText("unchanged by view".into()));
        let effects = app.dispatch_ready(AppAction::Navigate(View::Appearance));
        assert_eq!(save(&effects).1, "unchanged by view");
        assert_eq!(app.view(), View::Appearance);
        assert_eq!(app.editor().unwrap().body(), "unchanged by view");
    }
    #[test]
    fn an_unacknowledged_empty_draft_is_not_yet_saved() {
        let mut app = Coordinator::new(key("a.md").connection.clone(), || key("draft.md"));
        let effects = app.dispatch_ready(AppAction::NewNote);
        assert!(effects.iter().any(
            |e| matches!(e,AppEffect::CreateDraft { key, .. } if key.path.as_str()=="draft.md")
        ));
        assert_ne!(
            app.editor().unwrap().save_state(),
            LocalSaveState::Clean(LocalRevision(0))
        );
        app.dispatch_ready(AppAction::EditText(
            "typed before create acknowledgement".into(),
        ));
        app.dispatch_ready(AppAction::RequestQuit);
        let expected = effects
            .iter()
            .find_map(|effect| {
                if let AppEffect::CreateDraft { expected, .. } = effect {
                    Some(expected.clone())
                } else {
                    None
                }
            })
            .unwrap();
        let effects = app.receive_ready(AppEvent::LocalSaved {
            expected,
            outcome: bulletnotes::domain::reconciliation::LocalSaveOutcome {
                attempt: SavedRevision {
                    key: key("draft.md"),
                    local_revision: LocalRevision(0),
                },
                stored: page("draft.md", "").into(),
            },
        });
        let write = save(&effects);
        assert_eq!(write.1, "typed before create acknowledgement");
        assert!(
            ack(&mut app, write)
                .iter()
                .any(|e| matches!(e, AppEffect::QuitReady))
        );
    }

    #[test]
    fn editing_while_another_page_loads_preserves_current_buffer() {
        let mut app = app();
        let request = load(&app.dispatch_ready(AppAction::OpenPage(key("b.md"))));
        app.dispatch_ready(AppAction::EditText("late typing in A".into()));
        app.receive_ready(AppEvent::PageLoaded {
            request_id: request,
            key: key("b.md"),
            result: Ok(Some(page("b.md", "B").into())),
        });
        assert_eq!(app.editor().unwrap().key(), &key("a.md"));
        assert_eq!(app.editor().unwrap().body(), "late typing in A");
    }
    #[test]
    fn successful_retry_clears_the_failed_save_notice() {
        let mut app = app();
        app.dispatch_ready(AppAction::EditText("keep".into()));
        let first = save(&app.dispatch_ready(AppAction::Flush));
        app.receive_ready(AppEvent::LocalSaveFailed(
            first.3,
            first.2,
            StoreError::Closed,
        ));
        assert!(app.notice().is_some());
        let retry = save(&app.dispatch_ready(AppAction::Flush));
        ack(&mut app, retry);
        assert!(app.notice().is_none());
    }

    #[test]
    fn explicit_flush_drains_edits_that_arrived_during_a_write() {
        let mut app = app();
        app.dispatch_ready(AppAction::EditText("first".into()));
        let first = save(&app.dispatch_ready(AppAction::Flush));
        app.dispatch_ready(AppAction::EditText("second".into()));
        app.dispatch_ready(AppAction::Flush);
        let effects = ack(&mut app, first);
        let second = save(&effects);
        assert_eq!(second.1, "second");
    }
}
