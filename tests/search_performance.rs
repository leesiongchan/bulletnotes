//! Explicitly run release-only query-to-render measurements on synthetic stores.
mod support;
use bulletnotes::{
    AppWindow,
    app::{appearance_ui, local_ui},
    store::StoreHandle,
    theme::SystemAppearance,
};
use slint::{ComponentHandle, Model};
use std::{
    cell::Cell,
    rc::Rc,
    time::{Duration, Instant},
};
use support::storage_fixture::{database, key};

#[test]
#[ignore = "release benchmark: 10k and 50k synthetic notes"]
#[expect(
    clippy::assertions_on_constants,
    reason = "The ignored benchmark must reject accidental debug runs"
)]
fn substring_query_to_presentation_latency() {
    assert!(!cfg!(debug_assertions), "run with --release");
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
        for count in [10_000, 50_000] {
            let directory = tempfile::tempdir().unwrap();
            let path = database(&directory);
            let store = StoreHandle::open(&path).await.unwrap();
            store.shutdown().await.unwrap();
            {
                let mut db = rusqlite::Connection::open(&path).unwrap();
                let tx = db.transaction().unwrap();
                {
                    let mut insert = tx.prepare("INSERT INTO pages(connection,path,title,body) VALUES(?1,?2,?3,?4)").unwrap();
                    for n in 0..count {
                        let header = format!("# Header {n}\nCAFÉ Σςσ 日本語 👩🏽‍💻\n");
                        let body = format!("{header}{} rare-{n}", "ordinary words ".repeat((4096-header.len())/15));
                        insert.execute(rusqlite::params![key("a.md").connection.as_str(),format!("Notes/{n:05}.md"),format!("Header {n}"),body]).unwrap();
                    }
                }
                tx.commit().unwrap();
            }
            let store = StoreHandle::open(&path).await.unwrap();
            let window = AppWindow::new().unwrap();
            let appearance = appearance_ui::bind(&window, store.clone(), Default::default(), SystemAppearance::Light).unwrap();
            let local = local_ui::bind(&window, store.clone(), key("a.md").connection, appearance).unwrap();
            window.show().unwrap();
            window.window().set_size(slint::LogicalSize::new(505.0,552.0));
            local.dispatch(bulletnotes::app::coordinator::AppAction::Navigate(bulletnotes::app::model::View::AllPages));
            let mut samples = Vec::new();
            for n in 0..110 {
                let query = match n % 6 { 0 => "eAdE".to_owned(), 1 => "CAFÉ".to_owned(), 2 => "σ".to_owned(), 3 => "x".to_owned(), 4 => format!("rare-{}", count-1), _ => "ordinary words ordinary".to_owned() };
                window.set_search_query(query.into());
                let start = Instant::now();
                local.search_current().await.unwrap();
                let _image = window.window().take_snapshot().unwrap();
                if n >= 10 { samples.push(start.elapsed()); }
            }
            samples.sort();
            let p95 = samples[94];
            println!("notes={count}, approximate_body_bytes=4096, samples=100, p95_ms={:.2}, renderer=skia, includes=count+highlights+snapshot",p95.as_secs_f64()*1000.0);
            if count == 10_000 { assert!(p95 <= Duration::from_millis(150), "p95={p95:?}"); }
            let page = key("Notes/00000.md");
            local.dispatch(bulletnotes::app::coordinator::AppAction::OpenPage(page.clone()));
            local.wait_until_loaded(&page).await.unwrap();
            local.dispatch(bulletnotes::app::coordinator::AppAction::EditText("saved while searching".into()));
            window.invoke_browse_pages();
            let burst = Instant::now();
            for n in 0..250 {
                window.set_search_query(format!("superseded-{n}").into());
                window.invoke_query_changed(window.get_search_query());
                if n % 16 == 0 { tokio::task::yield_now().await; }
            }
            local.wait_until_saved().await.unwrap();
            window.set_search_query("saved while searching".into());
            local.search_current().await.unwrap();
            assert_eq!(store.load_page(&page).await.unwrap().unwrap().body,"saved while searching");
            assert_eq!(window.get_page_results().row_count(),1);
            assert_eq!(window.get_page_results().row_data(0).unwrap().path,"Notes/00000.md");
            assert!(burst.elapsed() < Duration::from_secs(5),"obsolete query backlog delayed the save/latest result");
            println!("notes={count}, superseded_requests=250, save_and_latest_result_ms={:.2}",burst.elapsed().as_secs_f64()*1000.0);
            window.hide().unwrap();
            store.shutdown().await.unwrap();
        }
        finished.set(true);
        slint::quit_event_loop().unwrap();
    }).unwrap();
    slint::run_event_loop_until_quit().unwrap();
    assert!(done.get());
}
