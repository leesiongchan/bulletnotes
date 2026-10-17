use bulletnotes::{
    app::{
        coordinator::{AppAction, AppEffect, AppEvent, Coordinator},
        model::EditorPosition,
    },
    domain::{
        connection::ConnectionConfig,
        page::*,
        status::{LocalSaveState, SyncState, status_text},
    },
};
fn key(host: &str) -> PageKey {
    PageKey {
        connection: ConnectionConfig::parse(host).unwrap().id().clone(),
        path: PagePath::parse("one.md").unwrap(),
    }
}
fn snapshot(body: &str) -> PageSnapshot {
    let key = key("https://one.example/");
    PageSnapshot {
        key: key.clone(),
        body: body.into(),
        local_revision: LocalRevision(0),
        remote_base: Some(RemoteFile {
            key,
            body: body.into(),
            revision: Some(RemoteRevision("\"a\"".into())),
            modified_ms: None,
            permission: Permission::ReadWrite,
        }),
        position: EditorPosition::default(),
        permission: Permission::ReadWrite,
    }
}
fn app() -> Coordinator {
    let key = key("https://one.example/");
    let mut app = Coordinator::new(key.connection.clone(), move || key.clone());
    let effects = app.dispatch(AppAction::OpenPage(snapshot("base").key));
    let id = effects
        .into_iter()
        .find_map(|effect| match effect {
            AppEffect::LoadPage { request_id, .. } => Some(request_id),
            _ => None,
        })
        .unwrap();
    app.receive(AppEvent::PageLoaded {
        request_id: id,
        key: snapshot("base").key,
        result: Ok(Some(snapshot("base"))),
    });
    app
}
#[test]
fn remote_notifications_do_not_replace_an_unsaved_editor_buffer() {
    let mut app = app();
    app.dispatch(AppAction::EditText("uncommitted".into()));
    app.receive(AppEvent::RemoteApplied(snapshot("remote")));
    assert_eq!(app.editor().unwrap().body(), "uncommitted");
    let effects = app.dispatch(AppAction::Flush);
    let baseline = effects
        .into_iter()
        .find_map(|effect| match effect {
            AppEffect::SaveLocal { baseline, .. } => Some(baseline),
            _ => None,
        })
        .unwrap();
    assert_eq!(baseline.body, "base");
    assert_eq!(baseline.remote_base.unwrap().body, "base");
}
#[test]
fn remote_notifications_update_a_clean_editor_even_when_local_revision_is_unchanged() {
    let mut app = app();
    app.receive(AppEvent::RemoteApplied(snapshot("remote")));
    assert_eq!(app.editor().unwrap().body(), "remote");
    assert_eq!(app.editor().unwrap().revision(), LocalRevision(0));
}
#[test]
fn connection_changes_refuse_to_discard_unsaved_edits() {
    let mut app = app();
    app.dispatch(AppAction::EditText("uncommitted".into()));
    let next = key("https://two.example/");
    let draft = next.clone();
    assert!(!app.change_connection(next.connection, move || draft.clone()));
    assert_eq!(app.editor().unwrap().body(), "uncommitted");
}
#[test]
fn local_durability_and_pending_work_override_a_stale_synced_indication() {
    assert_eq!(
        status_text(
            LocalSaveState::Dirty(LocalRevision(2)),
            SyncState::Synced,
            true
        ),
        "Unsaved changes"
    );
    assert_eq!(
        status_text(
            LocalSaveState::Clean(LocalRevision(2)),
            SyncState::Synced,
            false
        ),
        "Saved locally · sync pending"
    );
    assert_eq!(
        status_text(
            LocalSaveState::Clean(LocalRevision(2)),
            SyncState::Synced,
            true
        ),
        "Synced"
    );
    assert_eq!(
        status_text(
            LocalSaveState::Clean(LocalRevision(2)),
            SyncState::Offline,
            true
        ),
        "Offline · saved locally"
    );
}

#[test]
#[ignore = "opens isolated generated-text native sync app; build sync_review with native-validation,protocol-validation first"]
fn native_two_client_offline_reconnect_and_restart_preserve_source() {
    let executable = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("examples/sync_review");
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
        String::from_utf8_lossy(&output.stdout).contains("native_sync_contract_passed: true"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn an_uncached_open_preserves_its_selected_path_and_the_current_editor() {
    let mut app = app();
    app.dispatch(AppAction::Navigate(bulletnotes::app::model::View::AllPages));
    let selected = PageKey {
        connection: key("https://one.example/").connection,
        path: PagePath::parse("uncached.md").unwrap(),
    };
    let effects = app.dispatch(AppAction::OpenPage(selected.clone()));
    let id = effects
        .into_iter()
        .find_map(|effect| match effect {
            AppEffect::LoadPage { request_id, .. } => Some(request_id),
            _ => None,
        })
        .unwrap();
    app.receive(AppEvent::PageLoaded {
        request_id: id,
        key: selected.clone(),
        result: Ok(None),
    });
    assert_eq!(app.selected_uncached(), Some(&selected));
    assert_eq!(app.editor().unwrap().body(), "base");
    assert!(app.notice().unwrap().contains("Connect"));
    app.dispatch(AppAction::EditText("continue editing".into()));
    assert!(app.selected_uncached().is_none());
}
#[test]
fn catalogue_progress_is_scoped_to_the_active_connection() {
    let mut app = app();
    app.receive(AppEvent::CatalogueProgress {
        connection: key("https://two.example/").connection,
        indexed: 10,
        total: 10,
        complete: true,
    });
    assert_eq!(app.catalogue_progress(), None);
    app.receive(AppEvent::CatalogueProgress {
        connection: key("https://one.example/").connection,
        indexed: 2,
        total: 10,
        complete: false,
    });
    assert_eq!(app.catalogue_progress(), Some((2, 10, false)));
}

#[test]
fn background_search_refresh_does_not_cancel_the_selected_uncached_page() {
    use bulletnotes::{app::coordinator::SearchRequest, store::search::SearchQuery};
    let mut app = app();
    app.dispatch(AppAction::Navigate(bulletnotes::app::model::View::AllPages));
    let selected = PageKey {
        connection: key("https://one.example/").connection,
        path: PagePath::parse("uncached.md").unwrap(),
    };
    let effects = app.dispatch(AppAction::OpenPage(selected.clone()));
    let id = effects
        .into_iter()
        .find_map(|effect| match effect {
            AppEffect::LoadPage { request_id, .. } => Some(request_id),
            _ => None,
        })
        .unwrap();
    app.receive(AppEvent::PageLoaded {
        request_id: id,
        key: selected.clone(),
        result: Ok(None),
    });
    app.dispatch(AppAction::Search(SearchRequest {
        id: 1,
        query: SearchQuery {
            connection: selected.connection.clone(),
            text: String::new(),
            folder: None,
            offset: 0,
            limit: 100,
        },
    }));
    assert_eq!(app.selected_uncached(), Some(&selected));
}
#[test]
fn returning_to_the_note_cancels_an_older_page_load() {
    let mut app = app();
    let selected = PageKey {
        connection: key("https://one.example/").connection,
        path: PagePath::parse("other.md").unwrap(),
    };
    let effects = app.dispatch(AppAction::OpenPage(selected.clone()));
    let id = effects
        .into_iter()
        .find_map(|effect| match effect {
            AppEffect::LoadPage { request_id, .. } => Some(request_id),
            _ => None,
        })
        .unwrap();
    app.dispatch(AppAction::Navigate(bulletnotes::app::model::View::Note));
    let mut late = snapshot("late replacement");
    late.key = selected.clone();
    late.remote_base.as_mut().unwrap().key = selected.clone();
    app.receive(AppEvent::PageLoaded {
        request_id: id,
        key: selected,
        result: Ok(Some(late)),
    });
    assert_eq!(app.editor().unwrap().body(), "base");
}

#[test]
fn background_reindexing_keeps_the_current_page_results_visible() {
    use bulletnotes::{
        app::coordinator::SearchRequest,
        store::search::{SearchQuery, SearchResults},
    };
    let mut app = app();
    let query = || SearchQuery {
        connection: key("https://one.example/").connection,
        text: String::new(),
        folder: None,
        offset: 0,
        limit: 100,
    };
    app.dispatch(AppAction::Search(SearchRequest {
        id: 1,
        query: query(),
    }));
    app.receive(AppEvent::SearchReady(
        1,
        Ok(SearchResults {
            items: vec![],
            total: 12,
            indexed: 3,
            catalogue_total: 12,
            complete: false,
        }),
    ));
    app.dispatch(AppAction::Search(SearchRequest {
        id: 2,
        query: query(),
    }));
    assert_eq!(app.search_generation(), Some(1));
    assert_eq!(app.search_results().unwrap().total, 12);
}

#[test]
fn destructive_recovery_intents_wait_for_the_latest_local_commit() {
    let mut app = app();
    app.dispatch(AppAction::EditText("unsaved before trash".into()));
    let effects = app.dispatch(AppAction::MoveToTrash(
        snapshot("base").key,
        LocalRevision(1),
    ));
    assert!(
        effects
            .iter()
            .any(|effect| matches!(effect, AppEffect::SaveLocal { .. }))
    );
    assert!(
        !effects
            .iter()
            .any(|effect| matches!(effect, AppEffect::Recovery(_)))
    );
    let effects = app.receive(AppEvent::LocalSaved(SavedRevision {
        key: snapshot("base").key,
        local_revision: LocalRevision(1),
    }));
    assert!(
        effects
            .iter()
            .any(|effect| matches!(effect, AppEffect::Recovery(_)))
    );
}
