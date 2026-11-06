use bulletnotes::{
    AppWindow, LinkPicker,
    app::{
        appearance_ui,
        coordinator::AppAction,
        local_ui::{self, LocalUi},
        model::View,
    },
    domain::{
        connection::{ConnectionConfig, ConnectionId},
        page::*,
    },
    store::{StoreHandle, settings::AppSettings},
    theme::SystemAppearance,
};
use slint::{ComponentHandle, Model, platform::Key, platform::WindowEvent};
use std::{cell::RefCell, rc::Rc, sync::LazyLock, time::Duration};

type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;
/// Both pages are taller than the window, so revealing a heading must scroll.
static SOURCE: LazyLock<String> = LazyLock::new(|| {
    format!(
        "# Top\nRead [[plan#Milestones]], then [[Someday]] or [[report.pdf]].\nAlso [[plan@L62C4]] and [[Notes]].\n{}Back to [[#Top]].\n",
        "filler line\n".repeat(60)
    )
});
static PLAN: LazyLock<String> = LazyLock::new(|| {
    format!(
        "# Plan\n{}## Milestones\nship it\n",
        "filler line\n".repeat(60)
    )
});

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
/// Settle until `done` holds, up to two seconds; resolution and page loads are
/// asynchronous. The assertion that follows reports any timeout.
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
    assert_eq!(window.get_note_text(), SOURCE.as_str());

    follow_at(window, 0).await?;
    assert_eq!(
        local.current_key(),
        Some(today.clone()),
        "Ctrl+Enter outside a link"
    );
    assert_eq!(
        window.get_note_text(),
        SOURCE.as_str(),
        "Ctrl+Enter outside a link must not edit"
    );

    follow_at(window, SOURCE.find("plan").unwrap() + 2).await?;
    let plan = key(connection, "Projects/Plan.md");
    until(window, || local.current_key().as_ref() == Some(&plan)).await?;
    assert_eq!(
        local.current_key(),
        Some(plan.clone()),
        "name lookup is case-insensitive"
    );
    assert_eq!(window.get_note_text(), PLAN.as_str());
    assert_eq!(
        window.get_editor_caret() as usize,
        PLAN.find("## Milestones").unwrap(),
        "the caret moves to the linked heading"
    );
    assert!(
        window.get_editor_scroll_y() > 0.0,
        "the heading is scrolled into view"
    );

    open(local, &today).await?;
    follow_at(window, SOURCE.find("@L62").unwrap()).await?;
    until(window, || local.current_key().as_ref() == Some(&plan)).await?;
    assert_eq!(local.current_key(), Some(plan.clone()));
    assert_eq!(
        window.get_editor_caret() as usize,
        PLAN.find("Milestones").unwrap(),
        "line 62, column 4 is the heading's first word"
    );

    // Two pages are equally good matches, so the user picks one.
    open(local, &today).await?;
    let notes_at = SOURCE.find("[[Notes]]").unwrap() + 3;
    let choices = |window: &AppWindow| {
        window
            .global::<LinkPicker>()
            .get_choices()
            .iter()
            .map(|choice| choice.to_string())
            .collect::<Vec<_>>()
    };
    follow_at(window, notes_at).await?;
    until(window, || !choices(window).is_empty()).await?;
    assert_eq!(choices(window), ["Home/Notes.md", "Work/Notes.md"]);
    assert_eq!(local.current_key(), Some(today.clone()));
    press(window, Key::Escape.into());
    settle(window).await?;
    assert!(choices(window).is_empty(), "Escape dismisses the choices");
    assert_eq!(local.current_key(), Some(today.clone()));
    // Leaving the page drops its choices; they must not reappear on return.
    follow_at(window, notes_at).await?;
    until(window, || !choices(window).is_empty()).await?;
    local.dispatch(AppAction::Navigate(View::AllPages));
    settle(window).await?;
    local.dispatch(AppAction::Navigate(View::Note));
    settle(window).await?;
    assert!(
        choices(window).is_empty(),
        "leaving the note closes the choices"
    );
    open(local, &today).await?;
    follow_at(window, notes_at).await?;
    until(window, || !choices(window).is_empty()).await?;
    press(window, Key::DownArrow.into());
    press(window, Key::Return.into());
    let work = key(connection, "Work/Notes.md");
    until(window, || local.current_key().as_ref() == Some(&work)).await?;
    assert!(choices(window).is_empty());
    assert_eq!(local.current_key(), Some(work.clone()));

    open(local, &today).await?;
    follow_at(window, SOURCE.find("[[#Top]]").unwrap() + 3).await?;
    assert_eq!(local.current_key(), Some(today.clone()));
    assert_eq!(
        window.get_editor_caret(),
        0,
        "[[#Top]] jumps within the page"
    );
    // Line 0 sits below the editor's 33.75 px top clearance, so it is in view up to there.
    assert!(
        window.get_editor_scroll_y() <= 33.75,
        "the page scrolls back to its top"
    );

    follow_at(window, SOURCE.find("report.pdf").unwrap()).await?;
    until(window, || {
        window.get_status_text().starts_with("Only Markdown")
    })
    .await?;
    assert_eq!(local.current_key(), Some(today.clone()));
    assert_eq!(
        window.get_status_text(),
        "Only Markdown pages open in Bulletnotes."
    );

    let someday_at = SOURCE.find("[[Someday]]").unwrap();
    follow_at(window, someday_at).await?;
    until(window, || window.get_status_text().contains("page list")).await?;
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
    until(window, || local.current_key().as_ref() == Some(&someday)).await?;
    assert_eq!(local.current_key(), Some(someday.clone()));
    assert_eq!(window.get_note_text(), "");
    local.flush().await?;
    assert_eq!(
        store.load_page(&someday).await?.map(|page| page.body),
        Some(String::new()),
        "a missing target becomes a real local page"
    );

    // Cmd+Shift+B lists the pages linking here; picking one opens it at the link.
    backlinks_shortcut(window);
    until(window, || !choices(window).is_empty()).await?;
    assert_eq!(choices(window), ["Journal/today.md"]);
    assert_eq!(
        window.global::<LinkPicker>().get_title(),
        "Pages linking here"
    );
    press(window, Key::Return.into());
    until(window, || local.current_key().as_ref() == Some(&today)).await?;
    assert_eq!(local.current_key(), Some(today.clone()));
    until(window, || window.get_editor_caret() as usize == someday_at).await?;
    assert_eq!(
        window.get_editor_caret() as usize,
        someday_at,
        "the source page opens at its link"
    );

    // An ambiguous link may mean either page, so it counts for both.
    open(local, &work).await?;
    backlinks_shortcut(window);
    until(window, || !choices(window).is_empty()).await?;
    assert_eq!(choices(window), ["Journal/today.md"]);
    press(window, Key::Escape.into());
    settle(window).await?;

    open(local, &today).await?;
    backlinks_shortcut(window);
    until(window, || window.get_status_text().starts_with("No other")).await?;
    assert!(choices(window).is_empty());
    assert_eq!(window.get_status_text(), "No other pages link here.");
    Ok(())
}
fn backlinks_shortcut(window: &AppWindow) {
    window.invoke_focus_editor();
    for modifier in [Key::Control, Key::Shift] {
        window.window().dispatch_event(WindowEvent::KeyPressed {
            text: modifier.into(),
        });
    }
    press(window, "B".into());
    for modifier in [Key::Shift, Key::Control] {
        window.window().dispatch_event(WindowEvent::KeyReleased {
            text: modifier.into(),
        });
    }
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
        for (path, body) in [
            ("Journal/today.md", SOURCE.as_str()),
            ("Projects/Plan.md", PLAN.as_str()),
            ("Home/Notes.md", "# Home notes"),
            ("Work/Notes.md", "# Work notes"),
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
