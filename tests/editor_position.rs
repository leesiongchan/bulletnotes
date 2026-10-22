use bulletnotes::{
    AppWindow,
    app::{
        appearance_ui,
        coordinator::AppAction,
        local_ui::{self, LocalUi},
        startup,
    },
    domain::{
        connection::{ConnectionConfig, ConnectionId},
        editor::EditorPosition,
        page::*,
    },
    store::{StoreHandle, settings::AppSettings},
    theme::SystemAppearance,
};
use slint::ComponentHandle;
use std::{cell::RefCell, path::PathBuf, rc::Rc, time::Duration};

type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;
struct Fixture {
    window: AppWindow,
    local: LocalUi,
    store: StoreHandle,
    path: PathBuf,
    connection: ConnectionId,
}
fn bind(store: StoreHandle, connection: ConnectionId) -> Result<(AppWindow, LocalUi)> {
    let window = AppWindow::new()?;
    let appearance = appearance_ui::bind(
        &window,
        store.clone(),
        AppSettings::default(),
        SystemAppearance::Light,
    )?;
    let local = local_ui::bind(&window, store, connection, appearance)?;
    window.show()?;
    Ok((window, local))
}
fn run(check: impl AsyncFnOnce(Fixture) -> Result + 'static) -> Result {
    let runtime = tokio::runtime::Runtime::new()?;
    let _entered = runtime.enter();
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("position.sqlite");
    let store = runtime.block_on(StoreHandle::open(&path))?;
    let connection = ConnectionConfig::parse("https://position.example/")?
        .id()
        .clone();
    runtime.block_on(async {
        for name in ["first.md", "second.md"] {
            let key = PageKey {
                connection: connection.clone(),
                path: PagePath::parse(name)?,
            };
            store.create_draft(key.clone()).await?;
            store.save_local(key, source(), LocalRevision(1)).await?;
        }
        Ok::<_, Box<dyn std::error::Error>>(())
    })?;
    let (window, local) = bind(store.clone(), connection.clone())?;
    let outcome = Rc::new(RefCell::new(None));
    let finished = outcome.clone();
    slint::spawn_local(async move {
        *finished.borrow_mut() = Some(
            check(Fixture {
                window,
                local,
                store,
                path,
                connection,
            })
            .await,
        );
        let _ = slint::quit_event_loop();
    })?;
    slint::run_event_loop_until_quit()?;
    outcome
        .borrow_mut()
        .take()
        .ok_or("position test did not finish")?
}
fn source() -> String {
    format!(
        "café 日本語 👩🏽‍💻\r\n{}",
        "generated long line café 日本語\r\n".repeat(80)
    )
}
fn page(f: &Fixture, name: &str) -> PageKey {
    PageKey {
        connection: f.connection.clone(),
        path: PagePath::parse(name).unwrap(),
    }
}
async fn settle(window: &AppWindow) -> Result {
    let (tx, rx) = tokio::sync::oneshot::channel();
    slint::Timer::single_shot(Duration::from_millis(20), move || {
        let _ = tx.send(());
    });
    rx.await?;
    window.window().take_snapshot()?;
    Ok(())
}
fn observed(window: &AppWindow) -> EditorPosition {
    EditorPosition {
        anchor: window.get_editor_anchor() as usize,
        caret: window.get_editor_caret() as usize,
        scroll_y: window.get_editor_scroll_y(),
    }
}
async fn select(window: &AppWindow) -> Result<EditorPosition> {
    let position = EditorPosition {
        anchor: source().find('\r').unwrap(),
        caret: 0,
        scroll_y: 240.0,
    };
    window.invoke_restore_editor_position(
        position.anchor as i32,
        position.caret as i32,
        position.scroll_y,
    );
    settle(window).await?;
    assert_eq!(
        observed(window),
        position,
        "fixture must create a real backward selection and viewport"
    );
    Ok(position)
}
fn live_position_survives_note_switch_and_reopen() -> Result {
    run(async |f| {
        let first = page(&f, "first.md");
        let second = page(&f, "second.md");
        f.local.dispatch(AppAction::OpenPage(first.clone()));
        f.local.wait_until_loaded(&first).await?;
        settle(&f.window).await?;
        let want = select(&f.window).await?;
        f.local.dispatch(AppAction::OpenPage(second.clone()));
        f.local.wait_until_loaded(&second).await?;
        f.local.dispatch(AppAction::OpenPage(first.clone()));
        f.local.wait_until_loaded(&first).await?;
        settle(&f.window).await?;
        assert_eq!(
            observed(&f.window),
            want,
            "navigation must restore live position without SavePosition injection"
        );
        assert_eq!(f.store.load_page(&first).await?.unwrap().position, want);
        f.local.flush().await?;
        f.store.shutdown().await?;
        let reopened = startup::open(&f.path, f.connection.clone()).await?;
        assert_eq!(reopened.last_page, Some(first.clone()));
        let (window, local) = bind(reopened.store.clone(), f.connection)?;
        local.dispatch(AppAction::OpenPage(first.clone()));
        local.wait_until_loaded(&first).await?;
        settle(&window).await?;
        assert_eq!(
            observed(&window),
            want,
            "new window/store must restore direction and viewport"
        );
        assert_eq!(window.get_note_text(), source());
        reopened.store.shutdown().await?;
        Ok(())
    })
}
fn auxiliary_views_and_scratchpad_navigation_preserve_live_position() -> Result {
    run(async |f| {
        f.window.invoke_new_note();
        f.local.flush().await?;
        let first = f.local.current_key().unwrap();
        f.window.set_note_text(source().into());
        f.window.invoke_note_edited(source().into());
        f.local.wait_until_saved().await?;
        let want = select(&f.window).await?;
        f.window.invoke_show_appearance();
        settle(&f.window).await?;
        f.local.flush().await?;
        f.window.invoke_return_to_note();
        settle(&f.window).await?;
        assert_eq!(observed(&f.window), want);
        f.window.invoke_new_note();
        f.local.flush().await?;
        let second = f.local.current_key().unwrap();
        f.local.refresh_scratchpads().await?;
        f.window.invoke_scratchpad_step(-1);
        f.local.wait_until_loaded(&first).await?;
        settle(&f.window).await?;
        assert_eq!(observed(&f.window), want);
        assert_eq!(f.window.get_note_text(), source());
        f.window.invoke_jump_to_newest();
        f.local.wait_until_loaded(&second).await?;
        f.store.shutdown().await?;
        Ok(())
    })
}
fn metadata_failure_prevents_hide_and_retry_persists_live_position() -> Result {
    run(async |f| {
        let first = page(&f, "first.md");
        f.local.dispatch(AppAction::OpenPage(first.clone()));
        f.local.wait_until_loaded(&first).await?;
        f.local.flush().await?;
        let want = select(&f.window).await?;
        let injector = rusqlite::Connection::open(&f.path)?;
        injector.execute_batch("CREATE TRIGGER reject_position BEFORE UPDATE OF anchor,caret,scroll_y ON pages BEGIN SELECT RAISE(ABORT,'generated position failure'); END;")?;
        f.local.dispatch(AppAction::RequestHide);
        assert!(
            f.local.flush().await.is_err(),
            "hide must wait for the position metadata failure"
        );
        assert!(
            f.window.window().is_visible(),
            "failed position save must retain the visible editor"
        );
        assert_eq!(observed(&f.window), want);
        assert_eq!(f.window.get_note_text(), source());
        settle(&f.window).await?;
        assert!(
            !f.window.get_lifecycle_pending(),
            "failed close must release editor admission"
        );
        injector.execute_batch("DROP TRIGGER reject_position")?;
        f.local.flush().await?;
        assert_eq!(f.store.load_page(&first).await?.unwrap().position, want);
        f.store.shutdown().await?;
        Ok(())
    })
}

#[test]
fn live_editor_position_contracts() -> Result {
    slint::platform::set_platform(Box::new(i_slint_backend_testing::TestingBackend::new(
        i_slint_backend_testing::TestingBackendOptions {
            mock_time: false,
            threading: true,
            renderer_name: Some("skia".into()),
        },
    )))?;
    live_position_survives_note_switch_and_reopen()?;
    auxiliary_views_and_scratchpad_navigation_preserve_live_position()?;
    metadata_failure_prevents_hide_and_retry_persists_live_position()
}
