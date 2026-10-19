//! Generated scratchpads, a temporary store and owned callbacks only.
use bulletnotes::{
    AppWindow,
    app::{appearance_ui, coordinator::AppAction, local_ui},
    domain::{connection::ConnectionConfig, page::*},
    store::{StoreHandle, settings::AppSettings},
    theme::SystemAppearance,
};
use slint::ComponentHandle;
use std::{cell::Cell, rc::Rc, time::Duration};
pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let runtime = tokio::runtime::Runtime::new()?;
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("scratchpads.sqlite");
    let store = runtime.block_on(StoreHandle::open(&path))?;
    let window = AppWindow::new()?;
    let appearance = appearance_ui::bind(
        &window,
        store.clone(),
        AppSettings::default(),
        SystemAppearance::Light,
    )?;
    let connection = ConnectionConfig::parse("https://generated.example")?
        .id()
        .clone();
    let local = local_ui::bind(&window, store.clone(), connection.clone(), appearance)?;
    let _recovery = bulletnotes::app::recovery_ui::bind(&window, local.clone(), store.clone())?;
    let passed = Rc::new(Cell::new(false));
    let result = passed.clone();
    let weak = window.as_weak();
    slint::Timer::single_shot(Duration::from_millis(100), move || {
        let _ = slint::spawn_local(async move {
            let check=async {
            let window=weak.upgrade().ok_or("window closed")?;
            for _ in 0..3 {window.invoke_new_note();local.flush().await?;}
            local.refresh_scratchpads().await?;
            if window.get_note_counter()!="1 of 3" {return Err(format!("wrong stack position: {}",window.get_note_counter()).into());}
            let newest=local.current_key().ok_or("missing newest")?;
            let entries=store.scratchpad_stack(&connection).await?;let older=entries[1].key.clone();
            window.invoke_note_edited("generated café\r\n🙂".into());local.wait_until_saved().await?;
            let position=bulletnotes::domain::editor::EditorPosition {anchor:0,caret:9,scroll_y:12.0};
            local.dispatch(AppAction::SavePosition(position));
            window.invoke_scratchpad_step(-1);local.wait_until_loaded(&older).await?;
            if window.get_note_counter()!="2 of 3" {return Err("older note counter incorrect".into());}
            window.invoke_jump_to_newest();local.wait_until_loaded(&newest).await.map_err(|error|format!("{error}; status={}",window.get_status_text()))?;
            if window.get_note_text()!="generated café\r\n🙂" || store.load_page(&newest).await?.ok_or("page absent")?.position!=position {return Err("source or known position lost".into());}
            let injector=rusqlite::Connection::open(&path)?;
            injector.execute_batch("CREATE TRIGGER reject_visit BEFORE UPDATE OF last_visited_ms ON pages BEGIN SELECT RAISE(ABORT,'generated metadata failure'); END;")?;
            local.dispatch(AppAction::SavePosition(position));window.invoke_scratchpad_step(-1);
            if local.flush().await.is_ok() || local.current_key()!=Some(newest.clone()) {return Err("metadata failure did not retain note".into());}
            injector.execute_batch("DROP TRIGGER reject_visit")?;
            window.invoke_scratchpad_step(-1);local.wait_until_loaded(&older).await?;
            let ordinary=PageKey {connection:connection.clone(),path:PagePath::parse("ordinary.md")?};store.create_draft(ordinary.clone()).await?;
            local.dispatch(AppAction::OpenPage(ordinary.clone()));local.wait_until_loaded(&ordinary).await?;
            if !window.get_note_counter().is_empty() || window.get_can_go_older() || window.get_can_go_newer() {return Err("ordinary page has stack position".into());}
            local.flush().await?;
            window.set_recovery_busy(true);
            local.dispatch(AppAction::EmptyTrash(vec![]));
            local.dispatch(AppAction::Navigate(bulletnotes::app::model::View::Appearance));
            if window.get_recovery_busy() {return Err("cancelled recovery retained busy admission".into());}
            window.invoke_return_to_note();
            window.invoke_note_edited("editing after cancelled recovery".into());local.flush().await?;
            if window.get_note_read_only() || window.get_note_text()!="editing after cancelled recovery" {return Err("cancelled recovery blocked editing".into());}
            window.invoke_request_hide();local.flush().await?;
            if window.window().is_visible() {return Err("cancelled recovery blocked close admission".into());}
            store.shutdown().await?;Ok::<(),Box<dyn std::error::Error>>(())
        }.await;
            match check {
                Ok(()) => {
                    result.set(true);
                    println!("native_scratchpad_contract_passed: true");
                }
                Err(error) => eprintln!("scratchpad contract: {error}"),
            };
            let _ = slint::quit_event_loop();
        });
    });
    slint::Timer::single_shot(Duration::from_secs(15), || {
        let _ = slint::quit_event_loop();
    });
    window.show()?;
    slint::run_event_loop_until_quit()?;
    if !passed.get() {
        return Err("native scratchpad contract failed".into());
    }
    Ok(())
}
