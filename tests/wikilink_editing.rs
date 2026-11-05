//! Wikilinks while writing: links to missing pages look different, and typing
//! `[[` offers page names that are inserted into the saved note.
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
use i_slint_backend_testing::ElementHandle;
use slint::{ComponentHandle, Model, platform::Key, platform::WindowEvent};
use std::{cell::RefCell, rc::Rc, time::Duration};

type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;
const TODAY: &str = "Linked: [[plan]] and [[Someday]].\n";

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
/// Settle until `done` holds, up to two seconds; page reads are asynchronous.
async fn until(window: &AppWindow, done: impl Fn() -> bool) -> Result {
    for _ in 0..100 {
        if done() {
            break;
        }
        settle(window).await?;
    }
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
fn type_text(window: &AppWindow, text: &str) {
    for character in text.chars() {
        press(window, character.to_string().into());
    }
}
fn missing(window: &AppWindow) -> Vec<String> {
    window
        .get_missing_links()
        .iter()
        .map(|target| target.to_string())
        .collect()
}
fn suggestions(window: &AppWindow) -> Vec<String> {
    let _ = window.window().take_snapshot();
    ElementHandle::find_by_accessible_label(window, "Page suggestions")
        .next()
        .map(|list| {
            list.query_descendants()
                .match_accessible_role(i_slint_backend_testing::AccessibleRole::ListItem)
                .find_all()
                .into_iter()
                .filter_map(|item| item.accessible_label().map(|label| label.to_string()))
                .collect()
        })
        .unwrap_or_default()
}
async fn open(local: &LocalUi, page: &PageKey) -> Result {
    local.dispatch(AppAction::OpenPage(page.clone()));
    local.wait_until_loaded(page).await?;
    Ok(())
}
/// Put the caret at the end of the note.
async fn caret_at_end(window: &AppWindow) -> Result {
    window.invoke_focus_editor();
    let end = window.get_note_text().len() as i32;
    window.invoke_select_find_match(end, end);
    settle(window).await
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
    assert!(
        missing(window).is_empty(),
        "before a complete listing, any page may exist remotely"
    );

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
    // Loading a page rereads the page list, as a sync update does in the app.
    open(local, &key(connection, "Projects/Plan.md")).await?;
    open(local, &today).await?;
    until(window, || !missing(window).is_empty()).await?;
    assert_eq!(missing(window), ["Someday"]);
    let spans = window.invoke_highlight_markdown(
        window.get_note_text(),
        window.get_theme(),
        false,
        window.get_missing_links(),
    );
    let underlined: Vec<_> = spans
        .iter()
        .filter(|span| span.underline)
        .map(|span| TODAY[span.start as usize..span.end as usize].to_owned())
        .collect();
    assert_eq!(
        underlined,
        ["plan"],
        "only the existing page stays underlined"
    );

    // Typing a link offers matching pages; Return inserts the full name.
    caret_at_end(window).await?;
    type_text(window, "See [[pl");
    until(window, || !suggestions(window).is_empty()).await?;
    assert_eq!(suggestions(window), ["Projects/Plan"]);
    press(window, Key::Return.into());
    settle(window).await?;
    let linked = format!("{TODAY}See [[Projects/Plan]]");
    assert_eq!(window.get_note_text(), linked.as_str());
    assert_eq!(window.get_editor_caret() as usize, linked.len());
    assert!(
        suggestions(window).is_empty(),
        "a finished link has no suggestions"
    );
    local.flush().await?;
    assert_eq!(
        store.load_page(&today).await?.map(|page| page.body),
        Some(linked.clone()),
        "the inserted name is saved"
    );

    // Escape hides the list, so Return goes back to starting a new line.
    type_text(window, " [[pl");
    until(window, || !suggestions(window).is_empty()).await?;
    press(window, Key::Escape.into());
    settle(window).await?;
    assert!(suggestions(window).is_empty());
    press(window, Key::Return.into());
    settle(window).await?;
    assert_eq!(window.get_note_text(), format!("{linked} [[pl\n").as_str());

    // A new link to a page that does not exist is marked as it is finished.
    type_text(window, "[[Nowhere]]");
    until(window, || missing(window).len() == 2).await?;
    assert_eq!(missing(window), ["Someday", "Nowhere"]);
    Ok(())
}

#[test]
fn links_are_suggested_while_typing_and_missing_pages_are_marked() -> Result {
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
        for (path, body) in [("Journal/today.md", TODAY), ("Projects/Plan.md", "# Plan")] {
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
    window
        .window()
        .set_size(slint::LogicalSize::new(600.0, 500.0));
    window.show()?;
    window
        .window()
        .dispatch_event(WindowEvent::WindowActiveChanged(true));
    // The native backend reports this once the input method is idle.
    window.window().dispatch_event(WindowEvent::internal(
        i_slint_core::platform::InternalEvent::CompositionStatusChanged {
            context_id: 1,
            status: i_slint_core::input::CompositionStatus::Idle,
        },
    ));
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
