use bulletnotes::{
    app::coordinator::{AppAction, AppEvent, Coordinator, SearchRequest},
    domain::{connection::ConnectionConfig, page::*},
    store::search::{PageSummary, SearchQuery, SearchResults},
};
fn key(n: usize) -> PageKey {
    PageKey {
        connection: ConnectionConfig::parse("https://pagination.example")
            .unwrap()
            .id()
            .clone(),
        path: PagePath::parse(&format!("{n}.md")).unwrap(),
    }
}
fn request(id: u64, offset: usize, text: &str) -> AppAction {
    AppAction::Search(SearchRequest {
        id,
        query: SearchQuery {
            scope: bulletnotes::store::search::SearchScope::AllPages,
            connection: key(0).connection,
            text: text.into(),
            folder: None,
            offset,
            limit: 2,
        },
    })
}
fn result(first: usize, last: usize) -> SearchResults {
    SearchResults {
        items: (first..last)
            .map(|n| PageSummary {
                conflicted: false,
                highlights: Default::default(),
                key: key(n),
                title: format!("Generated {n}"),
                excerpt: Some("generated".into()),
                modified_ms: None,
                cached: true,
                permission: Permission::ReadWrite,
                last_visited_ms: None,
            })
            .collect(),
        total: 8,
        indexed: 8,
        catalogue_total: 8,
        complete: true,
    }
}
#[test]
fn continuation_extends_results_and_an_old_page_cannot_append_to_a_new_query() {
    let mut app = Coordinator::new(key(0).connection, || key(99));
    app.dispatch(request(1, 0, "old"));
    app.receive(AppEvent::SearchReady(1, Ok(result(0, 2))));
    app.dispatch(request(2, 2, "old"));
    app.receive(AppEvent::SearchReady(2, Ok(result(2, 4))));
    assert_eq!(
        app.search_results()
            .unwrap()
            .items
            .iter()
            .map(|page| page.key.clone())
            .collect::<Vec<_>>(),
        (0..4).map(key).collect::<Vec<_>>()
    );
    app.dispatch(request(3, 4, "old"));
    app.dispatch(request(4, 0, "new"));
    app.receive(AppEvent::SearchReady(3, Ok(result(4, 6))));
    assert!(app.search_results().is_none());
    app.receive(AppEvent::SearchReady(4, Ok(result(6, 8))));
    assert_eq!(app.search_results().unwrap().items[0].key, key(6));
}
#[test]
fn failed_continuation_preserves_the_loaded_pages_for_retry() {
    let mut app = Coordinator::new(key(0).connection, || key(99));
    app.dispatch(request(1, 0, ""));
    app.receive(AppEvent::SearchReady(1, Ok(result(0, 2))));
    app.dispatch(request(2, 2, ""));
    app.receive(AppEvent::SearchReady(
        2,
        Err(bulletnotes::domain::error::StoreError::Closed),
    ));
    assert_eq!(app.search_results().unwrap().items.len(), 2);
    app.dispatch(request(3, 2, ""));
    app.receive(AppEvent::SearchReady(3, Ok(result(2, 4))));
    assert_eq!(app.search_results().unwrap().items.len(), 4);
}
