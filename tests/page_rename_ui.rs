//! Renaming the current note from the rename bar moves it and updates links to it.
use bulletnotes::{
    AppWindow, PageRename,
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
const TODAY: &str = "See [[plan]] and [[plan#Plan|the plan]].";

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
    let plan = key(connection, "Projects/Plan.md");
    let rename = window.global::<PageRename>();
    open(local, &plan).await?;
    settle(window).await?;

    // Escape closes the bar without renaming.
    window.invoke_rename_current_note();
    until(window, || rename.get_open()).await?;
    assert_eq!(rename.get_name(), "Projects/Plan");
    press(window, Key::Escape.into());
    settle(window).await?;
    assert!(!rename.get_open());
    assert_eq!(local.current_key(), Some(plan.clone()));

    window.invoke_rename_current_note();
    until(window, || rename.get_link_pages() >= 0).await?;
    assert_eq!(rename.get_link_pages(), 1, "Journal/today links here");
    assert!(rename.get_update_links());
    let toggle = || {
        let _ = window.window().take_snapshot();
        i_slint_backend_testing::ElementHandle::find_by_accessible_label(
            window,
            "Update links in 1 page",
        )
        .next()
        .expect("the link switch is shown")
    };
    toggle().invoke_accessible_default_action();
    assert!(!rename.get_update_links());
    toggle().invoke_accessible_default_action();
    assert!(rename.get_update_links());
    window.invoke_rename_current_note();
    settle(window).await?;
    // The name is selected, so typing replaces it.
    type_text(window, "Other");
    assert_eq!(rename.get_name(), "Other");
    press(window, Key::Return.into());
    settle(window).await?;
    assert!(rename.get_open(), "a taken name keeps the bar open");
    assert_eq!(
        rename.get_message(),
        "A page with that name already exists."
    );

    // Leaving the note closes the bar.
    let today = key(connection, "Journal/today.md");
    open(local, &today).await?;
    settle(window).await?;
    assert!(
        !rename.get_open(),
        "the bar belongs to the note it was opened on"
    );
    open(local, &plan).await?;

    // The caret stays where it was.
    window.invoke_focus_editor();
    window.invoke_select_find_match(3, 3);
    settle(window).await?;
    window.invoke_rename_current_note();
    until(window, || rename.get_open()).await?;
    rename.set_name("Archive/Roadmap".into());
    press(window, Key::Return.into());
    let roadmap = key(connection, "Archive/Roadmap.md");
    until(window, || local.current_key().as_ref() == Some(&roadmap)).await?;
    assert_eq!(local.current_key(), Some(roadmap.clone()));
    assert!(!rename.get_open());
    assert_eq!(window.get_note_text(), "# Plan");
    until(window, || window.get_editor_caret() == 3).await?;
    assert_eq!(window.get_editor_caret(), 3);
    assert_eq!(
        store
            .load_page(&key(connection, "Journal/today.md"))
            .await?
            .map(|page| page.body),
        Some("See [[Archive/Roadmap]] and [[Archive/Roadmap#Plan|the plan]].".into()),
        "links to the old name are rewritten"
    );
    assert_eq!(store.trash(connection).await?.len(), 1);
    Ok(())
}

#[test]
fn renaming_the_note_moves_it_and_updates_links() -> Result {
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
    let store = runtime.block_on(StoreHandle::open(&directory.path().join("rename.sqlite")))?;
    let connection = ConnectionConfig::parse("https://links.example/")?
        .id()
        .clone();
    runtime.block_on(async {
        for (path, body) in [
            ("Journal/today.md", TODAY),
            ("Projects/Plan.md", "# Plan"),
            ("Other.md", "x"),
        ] {
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
