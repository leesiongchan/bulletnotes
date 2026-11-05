use bulletnotes::{
    AppWindow,
    app::{
        appearance_ui,
        coordinator::AppAction,
        local_ui::{self, LocalUi},
    },
    domain::{
        connection::{ConnectionConfig, ConnectionId},
        page::*,
    },
    store::{StoreHandle, settings::AppSettings},
    theme::SystemAppearance,
};
use slint::{ComponentHandle, platform::Key, platform::WindowEvent};
use std::{cell::RefCell, rc::Rc, time::Duration};

type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;
const SOURCE: &str = "Read [[plan]], then [[Someday]] or [[report.pdf]].";

fn key(connection: &ConnectionId, path: &str) -> PageKey {
    PageKey {
        connection: connection.clone(),
        path: PagePath::parse(path).unwrap(),
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
fn press(window: &AppWindow, text: slint::SharedString) {
    window
        .window()
        .dispatch_event(WindowEvent::KeyPressed { text: text.clone() });
    window
        .window()
        .dispatch_event(WindowEvent::KeyReleased { text });
}
/// Put the caret at `offset` and press Ctrl+Enter (Cmd+Enter on macOS).
async fn follow_at(window: &AppWindow, offset: usize) -> Result {
    window.invoke_focus_editor();
    window.invoke_select_find_match(offset as i32, offset as i32);
    window.window().dispatch_event(WindowEvent::KeyPressed {
        text: Key::Control.into(),
    });
    press(window, Key::Return.into());
    window.window().dispatch_event(WindowEvent::KeyReleased {
        text: Key::Control.into(),
    });
    for _ in 0..3 {
        settle(window).await?;
    }
    Ok(())
}
async fn open(local: &LocalUi, page: &PageKey) -> Result {
    local.dispatch(AppAction::OpenPage(page.clone()));
    local.wait_until_loaded(page).await?;
    Ok(())
}

async fn check(
    window: &AppWindow,
    local: &LocalUi,
    store: &StoreHandle,
    connection: &ConnectionId,
) -> Result {
    let today = key(connection, "Journal/today.md");
    open(local, &today).await?;
    settle(window).await?;
    assert_eq!(window.get_note_text(), SOURCE);

    follow_at(window, 0).await?;
    assert_eq!(
        local.current_key(),
        Some(today.clone()),
        "Ctrl+Enter outside a link"
    );
    assert_eq!(
        window.get_note_text(),
        SOURCE,
        "Ctrl+Enter outside a link must not edit"
    );

    follow_at(window, SOURCE.find("plan").unwrap() + 2).await?;
    let plan = key(connection, "Projects/Plan.md");
    assert_eq!(
        local.current_key(),
        Some(plan.clone()),
        "name lookup is case-insensitive"
    );
    assert_eq!(window.get_note_text(), "# Plan");

    open(local, &today).await?;
    follow_at(window, SOURCE.find("report.pdf").unwrap()).await?;
    assert_eq!(local.current_key(), Some(today.clone()));
    assert_eq!(
        window.get_status_text(),
        "Only Markdown pages open in Bulletnotes."
    );

    let someday_at = SOURCE.find("[[Someday]]").unwrap();
    follow_at(window, someday_at).await?;
    assert_eq!(
        local.current_key(),
        Some(today.clone()),
        "without a complete listing the page may exist remotely"
    );
    assert!(window.get_status_text().contains("page list"));

    let listing = [&today, &key(connection, "Projects/Plan.md")].map(|page| RemoteMetadata {
        key: page.clone(),
        modified_ms: Some(1),
        content_length: None,
        permission: Permission::ReadWrite,
        revision: None,
    });
    store
        .record_catalogue(connection, listing.to_vec(), true)
        .await?;
    follow_at(window, someday_at).await?;
    let someday = key(connection, "Someday.md");
    assert_eq!(local.current_key(), Some(someday.clone()));
    assert_eq!(window.get_note_text(), "");
    local.flush().await?;
    assert_eq!(
        store.load_page(&someday).await?.map(|page| page.body),
        Some(String::new()),
        "a missing target becomes a real local page"
    );
    Ok(())
}

#[test]
fn ctrl_enter_follows_the_wikilink_at_the_caret() -> Result {
    slint::platform::set_platform(Box::new(i_slint_backend_testing::TestingBackend::new(
        i_slint_backend_testing::TestingBackendOptions {
            mock_time: false,
            threading: true,
            renderer_name: Some("skia".into()),
        },
    )))?;
    let runtime = tokio::runtime::Runtime::new()?;
    let _entered = runtime.enter();
    let directory = tempfile::tempdir()?;
    let store = runtime.block_on(StoreHandle::open(&directory.path().join("links.sqlite")))?;
    let connection = ConnectionConfig::parse("https://links.example/")?
        .id()
        .clone();
    runtime.block_on(async {
        for (path, body) in [("Journal/today.md", SOURCE), ("Projects/Plan.md", "# Plan")] {
            let page = key(&connection, path);
            store.create_draft(page.clone()).await?;
            store
                .save_local(page, body.into(), LocalRevision(1))
                .await?;
        }
        Ok::<_, Box<dyn std::error::Error>>(())
    })?;
    let window = AppWindow::new()?;
    let appearance = appearance_ui::bind(
        &window,
        store.clone(),
        AppSettings::default(),
        SystemAppearance::Light,
    )?;
    let local = local_ui::bind(&window, store.clone(), connection.clone(), appearance)?;
    window.show()?;
    window
        .window()
        .dispatch_event(WindowEvent::WindowActiveChanged(true));
    let outcome = Rc::new(RefCell::new(None));
    let finished = outcome.clone();
    slint::spawn_local(async move {
        let result = check(&window, &local, &store, &connection).await;
        let _ = store.shutdown().await;
        *finished.borrow_mut() = Some(result);
        let _ = slint::quit_event_loop();
    })?;
    slint::run_event_loop_until_quit()?;
    outcome
        .borrow_mut()
        .take()
        .ok_or("link test did not finish")?
}
