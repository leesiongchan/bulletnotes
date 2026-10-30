mod support;
use bulletnotes::{
    app::model::EditorPosition,
    domain::{connection::ConnectionConfig, page::LocalRevision},
    store::{StoreHandle, search::SearchQuery},
};
use support::storage_fixture::{database, key};
#[tokio::test]
async fn substring_search_matches_inside_words_and_unicode_case() {
    let directory = tempfile::tempdir().unwrap();
    let store = StoreHandle::open(&database(&directory)).await.unwrap();
    let page = key("sample.md");
    store.create_draft(page.clone()).await.unwrap();
    store
        .save_local(
            page.clone(),
            "# Header\r\ncafé Σςσ 日本語 .* 50% a_b \"quoted\"".into(),
            LocalRevision(1),
        )
        .await
        .unwrap();
    for query in ["eAdE", "CAFÉ", "σ", "本", ".*", "%", "_", "\"quoted\""] {
        let result = store.search(search_query(query)).await.unwrap();
        assert_eq!(result.total, 1, "query {query}");
        let hit = &result.items[0];
        let excerpt = hit.excerpt.as_ref().unwrap();
        assert!(!hit.highlights.excerpt.is_empty(), "query {query}");
        for range in &hit.highlights.excerpt {
            assert!(excerpt.get(range.clone()).is_some());
        }
    }
    for query in ["cafe", ".+", "not_there"] {
        assert_eq!(store.search(search_query(query)).await.unwrap().total, 0);
    }
    store.shutdown().await.unwrap();
}
#[tokio::test]
async fn search_is_literal_and_scoped() {
    let directory = tempfile::tempdir().unwrap();
    let store = StoreHandle::open(&database(&directory)).await.unwrap();
    let a = key("Projects/50% café.md");
    let b = key("Other/same.md");
    let mut other = a.clone();
    other.connection = ConnectionConfig::parse("https://notes.example/b")
        .unwrap()
        .id()
        .clone();
    for (page, body) in [
        (&a, "# Same\nLiteral OR words and \"quotes\" 日本語"),
        (&b, "# Same\nOrdinary words"),
        (&other, "# Same\nLiteral OR words and \"quotes\" 日本語"),
    ] {
        store.create_draft(page.clone()).await.unwrap();
        store
            .save_local(page.clone(), body.into(), LocalRevision(1))
            .await
            .unwrap();
    }
    for text in ["Literal OR words", "\"quotes\"", "日本語", "50% café"] {
        let results = store
            .search(SearchQuery {
                scope: bulletnotes::store::search::SearchScope::AllPages,
                connection: a.connection.clone(),
                text: text.into(),
                folder: None,
                offset: 0,
                limit: 50,
            })
            .await
            .unwrap();
        assert_eq!(results.items.len(), 1, "query {text}");
        assert_eq!(results.items[0].key, a);
        assert!(results.items[0].cached);
    }
    let all = store
        .search(SearchQuery {
            scope: bulletnotes::store::search::SearchScope::AllPages,
            connection: a.connection.clone(),
            text: "Same".into(),
            folder: None,
            offset: 0,
            limit: 50,
        })
        .await
        .unwrap();
    assert_eq!(all.total, 2);
    assert_eq!(all.indexed, 2);
    assert_eq!(all.catalogue_total, 2);
    let punctuation = store
        .search(SearchQuery {
            scope: bulletnotes::store::search::SearchScope::AllPages,
            connection: a.connection.clone(),
            text: "\" : * OR - ()".into(),
            folder: None,
            offset: 0,
            limit: 50,
        })
        .await;
    assert!(punctuation.is_ok());
    store.shutdown().await.unwrap();
}
#[tokio::test]
async fn empty_query_orders_recent_visits_then_paths_and_respects_folder() {
    let directory = tempfile::tempdir().unwrap();
    let store = StoreHandle::open(&database(&directory)).await.unwrap();
    for path in ["Projects/a.md", "Projects/b.md", "Other/z.md"] {
        let page = key(path);
        store.create_draft(page.clone()).await.unwrap();
        store
            .save_local(page, "note".into(), LocalRevision(1))
            .await
            .unwrap();
    }
    store
        .save_position(&key("Projects/b.md"), EditorPosition::default(), 100)
        .await
        .unwrap();
    let results = store
        .search(SearchQuery {
            scope: bulletnotes::store::search::SearchScope::AllPages,
            connection: key("a.md").connection,
            text: String::new(),
            folder: None,
            offset: 0,
            limit: 20,
        })
        .await
        .unwrap();
    assert_eq!(
        results
            .items
            .iter()
            .map(|item| item.key.path.as_str())
            .collect::<Vec<_>>(),
        vec!["Projects/b.md", "Other/z.md", "Projects/a.md"]
    );
    let results = store
        .search(SearchQuery {
            scope: bulletnotes::store::search::SearchScope::AllPages,
            connection: key("a.md").connection,
            text: String::new(),
            folder: Some(bulletnotes::domain::page::PagePath::parse("Projects").unwrap()),
            offset: 1,
            limit: 1,
        })
        .await
        .unwrap();
    assert_eq!(results.total, 2);
    assert_eq!(results.items[0].key, key("Projects/a.md"));
    store.shutdown().await.unwrap();
}

#[tokio::test]
async fn punctuation_only_searches_do_not_become_fts_syntax() {
    let directory = tempfile::tempdir().unwrap();
    let store = StoreHandle::open(&database(&directory)).await.unwrap();
    let page = key("a.md");
    store.create_draft(page.clone()).await.unwrap();
    for text in ["\"", "*", "()", ":", "-", "\"\"\""] {
        let result = store
            .search(SearchQuery {
                scope: bulletnotes::store::search::SearchScope::AllPages,
                connection: page.connection.clone(),
                text: text.into(),
                folder: None,
                offset: 0,
                limit: 20,
            })
            .await;
        assert!(result.is_ok(), "literal query failed: {text}");
    }
    store.shutdown().await.unwrap();
}

#[tokio::test]
async fn recording_a_visit_does_not_invent_or_replace_selection() {
    let directory = tempfile::tempdir().unwrap();
    let store = StoreHandle::open(&database(&directory)).await.unwrap();
    let page = key("a.md");
    store.create_draft(page.clone()).await.unwrap();
    store
        .save_local(page.clone(), "café\ntext".into(), LocalRevision(1))
        .await
        .unwrap();
    let position = EditorPosition {
        anchor: 3,
        caret: 5,
        scroll_y: 42.0,
    };
    store.save_position(&page, position, 10).await.unwrap();
    store.record_visit(&page, 20).await.unwrap();
    assert_eq!(
        store.load_page(&page).await.unwrap().unwrap().position,
        position
    );
    let results = store
        .search(SearchQuery {
            scope: bulletnotes::store::search::SearchScope::AllPages,
            connection: page.connection.clone(),
            text: String::new(),
            folder: None,
            offset: 0,
            limit: 50,
        })
        .await
        .unwrap();
    assert_eq!(results.items[0].last_visited_ms, Some(20));
    store.shutdown().await.unwrap();
}

fn search_query(text: &str) -> SearchQuery {
    SearchQuery {
        scope: bulletnotes::store::search::SearchScope::AllPages,
        connection: key("generated.md").connection,
        text: text.into(),
        folder: None,
        offset: 0,
        limit: 50,
    }
}
#[tokio::test]
async fn late_body_match_is_visible_in_excerpt() {
    let directory = tempfile::tempdir().unwrap();
    let store = StoreHandle::open(&database(&directory)).await.unwrap();
    let page = key("generated.md");
    let body = format!("{}NeEdLe café 日本語 👩🏽‍💻 needle", "unrelated ".repeat(80));
    store.create_draft(page.clone()).await.unwrap();
    store
        .save_local(page.clone(), body.clone(), LocalRevision(1))
        .await
        .unwrap();
    let results = store.search(search_query("needle")).await.unwrap();
    let item = &results.items[0];
    let excerpt = item.excerpt.as_ref().unwrap();
    assert!(!excerpt.starts_with("unrelated "));
    assert_eq!(&excerpt[item.highlights.excerpt[0].clone()], "NeEdLe");
    assert!(item.highlights.excerpt[0].start <= "…".len());
    assert_eq!(item.highlights.excerpt.len(), 2);
    for phrase in ["café", "日本語", "needle café"] {
        let results = store.search(search_query(phrase)).await.unwrap();
        let item = &results.items[0];
        let text = item.excerpt.as_ref().unwrap();
        assert!(!item.highlights.excerpt.is_empty());
        for range in &item.highlights.excerpt {
            assert!(text.is_char_boundary(range.start) && text.is_char_boundary(range.end));
        }
    }
    let empty = store.search(search_query("")).await.unwrap();
    assert_eq!(
        empty.items[0].excerpt.as_ref().unwrap(),
        &body.chars().take(320).collect::<String>()
    );
    assert_eq!(empty.items[0].highlights, Default::default());
    assert_eq!(store.load_page(&page).await.unwrap().unwrap().body, body);
    store.shutdown().await.unwrap();
}
#[tokio::test]
async fn title_only_hit_keeps_body_beginning() {
    let directory = tempfile::tempdir().unwrap();
    let path = database(&directory);
    let store = StoreHandle::open(&path).await.unwrap();
    let page = key("plain.md");
    store.create_draft(page.clone()).await.unwrap();
    store
        .save_local(
            page,
            "Beginning body remains literal".into(),
            LocalRevision(1),
        )
        .await
        .unwrap();
    let db = rusqlite::Connection::open(&path).unwrap();
    db.execute("UPDATE pages SET title=?1", ["Title Needle"])
        .unwrap();
    let results = store.search(search_query("needle")).await.unwrap();
    let item = &results.items[0];
    assert_eq!(item.title, "Title Needle");
    assert_eq!(
        item.excerpt.as_deref(),
        Some("Beginning body remains literal")
    );
    assert!(item.highlights.excerpt.is_empty());
    store.shutdown().await.unwrap();
}
#[tokio::test]
async fn literal_marker_text_survives() {
    let directory = tempfile::tempdir().unwrap();
    let store = StoreHandle::open(&database(&directory)).await.unwrap();
    let page = key("literal.md");
    let body =
        "<o>needle<c> <b>needle</b> **needle** [bn-00000000-0000-0000-0000-000000000000-open]";
    store.create_draft(page.clone()).await.unwrap();
    store
        .save_local(page, body.into(), LocalRevision(1))
        .await
        .unwrap();
    let results = store.search(search_query("needle")).await.unwrap();
    let item = &results.items[0];
    assert!(
        item.excerpt
            .as_ref()
            .unwrap()
            .contains("<b>needle</b> **needle**")
    );
    for range in &item.highlights.excerpt {
        assert_eq!(&item.excerpt.as_ref().unwrap()[range.clone()], "needle");
    }
    store.shutdown().await.unwrap();
}
#[tokio::test]
async fn online_only_hit_has_no_body_excerpt() {
    use bulletnotes::domain::page::{Permission, RemoteMetadata};
    let directory = tempfile::tempdir().unwrap();
    let store = StoreHandle::open(&database(&directory)).await.unwrap();
    let page = key("folder/Needle.md");
    store
        .record_catalogue(
            &page.connection,
            vec![RemoteMetadata {
                key: page.clone(),
                modified_ms: Some(1),
                content_length: None,
                permission: Permission::ReadOnly,
                revision: None,
            }],
            true,
        )
        .await
        .unwrap();
    let results = store.search(search_query("needle")).await.unwrap();
    let item = &results.items[0];
    assert!(!item.cached);
    assert!(item.excerpt.is_none());
    assert!(item.highlights.excerpt.is_empty());
    assert!(!item.highlights.path.is_empty());
    assert!(store.load_page(&page).await.unwrap().is_none());
    store.shutdown().await.unwrap();
}

#[tokio::test]
async fn scratchpad_scope_uses_stack_order_and_ignores_all_pages_folder() {
    use bulletnotes::store::search::SearchScope;
    let directory = tempfile::tempdir().unwrap();
    let store = StoreHandle::open(&database(&directory)).await.unwrap();
    for path in ["Quick Notes/a.md", "Quick Notes/b.md", "Other/c.md"] {
        let page = key(path);
        store.create_draft(page.clone()).await.unwrap();
        store
            .save_local(page, "Header".into(), LocalRevision(1))
            .await
            .unwrap();
    }
    let mut query = search_query("eade");
    query.scope = SearchScope::Scratchpads;
    query.folder = Some(bulletnotes::domain::page::PagePath::parse("Other").unwrap());
    query.limit = 1;
    let result = store.search(query.clone()).await.unwrap();
    assert_eq!(result.total, 2);
    assert_eq!(result.items[0].key, key("Quick Notes/b.md"));
    query.offset = 1;
    let result = store.search(query).await.unwrap();
    assert_eq!(result.items[0].key, key("Quick Notes/a.md"));
    assert!(!result.items[0].conflicted);
    store.shutdown().await.unwrap();
}
