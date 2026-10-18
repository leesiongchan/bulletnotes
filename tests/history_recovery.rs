use bulletnotes::{
    app::coordinator::*,
    domain::{connection::ConnectionConfig, error::StoreError, page::*},
};
fn key(path: &str) -> PageKey {
    PageKey {
        connection: ConnectionConfig::parse("https://history.example/")
            .unwrap()
            .id()
            .clone(),
        path: PagePath::parse(path).unwrap(),
    }
}
fn load(effects: Vec<AppEffect>) -> (u64, PageKey) {
    effects
        .into_iter()
        .find_map(|effect| match effect {
            AppEffect::LoadPage { request_id, key } => Some((request_id, key)),
            _ => None,
        })
        .expect("history should request the next available candidate")
}
fn complete(app: &mut Coordinator, request: (u64, PageKey)) {
    let (request_id, key) = request;
    let page = PageSnapshot {
        key: key.clone(),
        body: key.path.as_str().into(),
        local_revision: LocalRevision(0),
        remote_base: None,
        position: Default::default(),
        permission: Permission::ReadWrite,
    };
    app.receive(AppEvent::PageLoaded {
        request_id,
        key,
        result: Ok(Some(page)),
    });
}
fn visited() -> Coordinator {
    let mut app = Coordinator::new(key("a.md").connection, || key("draft.md"));
    for path in ["a.md", "b.md", "c.md", "d.md"] {
        let request = load(app.dispatch(AppAction::OpenPage(key(path))));
        complete(&mut app, request);
    }
    app
}
#[test]
fn deleted_history_entries_are_skipped_in_both_directions() {
    for step in [-1, 1] {
        let mut app = visited();
        if step > 0 {
            for _ in 0..3 {
                let request = load(app.dispatch(AppAction::StepHistory(-1)));
                complete(&mut app, request);
            }
        }
        let original = app.editor().unwrap().key().clone();
        let (mut request_id, mut missing) = load(app.dispatch(AppAction::StepHistory(step)));
        for expected in if step < 0 {
            ["c.md", "b.md"]
        } else {
            ["b.md", "c.md"]
        } {
            assert_eq!(missing, key(expected));
            let next = load(app.receive(AppEvent::PageLoaded {
                request_id,
                key: missing,
                result: Err(StoreError::PageMissing),
            }));
            assert_eq!(app.editor().unwrap().key(), &original);
            (request_id, missing) = next;
        }
        assert_eq!(missing, key(if step < 0 { "a.md" } else { "d.md" }));
        complete(&mut app, (request_id, missing));
        assert_eq!(
            app.editor().unwrap().key(),
            &key(if step < 0 { "a.md" } else { "d.md" })
        );
    }
}
#[test]
fn transient_history_load_failures_leave_the_entry_available_for_retry() {
    let mut app = visited();
    let (request_id, key) = load(app.dispatch(AppAction::StepHistory(-1)));
    app.receive(AppEvent::PageLoaded {
        request_id,
        key: key.clone(),
        result: Err(StoreError::Closed),
    });
    assert_eq!(load(app.dispatch(AppAction::StepHistory(-1))).1, key);
}
