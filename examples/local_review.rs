//! Generated-content local milestone check; no HTTP client or real server access.
#[path = "support/scratchpad_contract.rs"]
mod scratchpad_contract;
use bulletnotes::{
    AppWindow,
    app::{appearance_ui, coordinator::AppAction, local_ui},
    domain::{connection::ConnectionConfig, page::LocalRevision},
    store::{StoreHandle, settings::AppSettings},
    theme::SystemAppearance,
};
use slint::ComponentHandle;
use std::{cell::Cell, rc::Rc, time::Duration};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::args().any(|arg| arg == "--scratchpad-contract") {
        return scratchpad_contract::run();
    }
    let runtime = tokio::runtime::Runtime::new()?;
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("review.sqlite");
    let store = runtime.block_on(StoreHandle::open(&path))?;
    let window = AppWindow::new()?;
    let appearance = appearance_ui::bind(
        &window,
        store.clone(),
        AppSettings::default(),
        SystemAppearance::Light,
    )?;
    let connection = ConnectionConfig::parse("https://notes.example/")?
        .id()
        .clone();
    let local = local_ui::bind(&window, store.clone(), connection, appearance)?;
    let passed = Rc::new(Cell::new(false));
    let result = passed.clone();
    let weak = window.as_weak();
    slint::Timer::single_shot(Duration::from_millis(300), move || {
        let _ = slint::spawn_local(async move {
            let check = async {
                let window = weak.upgrade().ok_or("review window closed")?;
                local.dispatch(AppAction::NewNote);
                local.flush().await?;
                let original_key = local.current_key().ok_or("draft absent")?;
                let source = "---\r\ntitle: café\r\n---\r\n\t👩🏽‍💻 日本語\r\n    indented source\r\n- [ ] source command\r\n";
                window.set_note_text(source.into());
                window.invoke_note_edited(source.into());
                // Observe automatic durability; do not request a manual flush here.
                local.wait_until_saved().await?;
                if store
                    .load_page(&original_key)
                    .await?
                    .ok_or("saved page absent")?
                    .body
                    .as_bytes()
                    != source.as_bytes()
                {
                    return Err("UI callback source was not durably saved".into());
                }
                window.invoke_show_find();
                window.set_find_query("café".into());
                window.invoke_find_query_changed("café".into());
                if !window.get_find_visible() || window.get_find_status() != "1 of 1" {
                    return Err("native find callback did not select the literal match".into());
                }
                window.invoke_find_step(-1);
                window.invoke_close_find();
                if window.get_find_visible() || window.get_note_text().as_bytes() != source.as_bytes()
                    || store.load_page(&original_key).await?.ok_or("find page absent")?.local_revision != LocalRevision(1) {
                    return Err("find modified source or left its bar open".into());
                }
                let injector = rusqlite::Connection::open(&path)?;
                injector.execute_batch(
                    "CREATE TRIGGER fail_note BEFORE UPDATE OF body ON pages BEGIN SELECT RAISE(ABORT,'generated failure'); END;",
                )?;
                let failed_text = format!("{source}# keep this failed-save buffer\r\n");
                window.set_note_text(failed_text.as_str().into());
                window.invoke_note_edited(failed_text.as_str().into());
                let task = failed_text.find("- [ ] source command").ok_or("generated task absent")? + "- [ ] source command".len();
                window.invoke_focus_editor();
                window.invoke_restore_editor_position(task as i32, task as i32, 0.0);
                use slint::platform::{Key, WindowEvent};
                for (text, down) in [(Key::Control.into(), true), (Key::Shift.into(), true), ("k".into(), true), ("k".into(), false), (Key::Shift.into(), false), (Key::Control.into(), false)] {
                    window.window().dispatch_event(if down { WindowEvent::KeyPressed { text } } else { WindowEvent::KeyReleased { text } });
                }
                let failed_text = failed_text.replacen("- [ ] source command", "- [x] source command", 1);
                if local.flush().await.is_ok() || window.get_note_text().as_str() != failed_text {
                    return Err("failed save lost or acknowledged the buffer".into());
                }
                if store.load_page(&original_key).await?.ok_or("original page absent")?.body != source {
                    return Err("failed transaction modified durable source".into());
                }
                injector.execute_batch("DROP TRIGGER fail_note")?;
                window.invoke_save_now();
                local.wait_until_saved().await?;
                if store.load_page(&original_key).await?.ok_or("retry page absent")?.body != failed_text {
                    return Err("retry callback did not save retained text".into());
                }
                let source = failed_text.as_str();
                window.invoke_new_note();
                local.flush().await?;
                if local.current_key() == Some(original_key.clone()) {
                    return Err("new note reused old identity".into());
                }
                window.invoke_browse_pages();
                local.search_current().await?;
                if window.get_active_view() != "pages" {
                    return Err("browse callback did not show pages".into());
                }
                use slint::Model;
                if window.get_page_results().row_count() != 2 {
                    return Err("browser did not show both local pages".into());
                }
                window.invoke_open_page(serde_json::to_string(&original_key)?.into());
                local.wait_until_loaded(&original_key).await?;
                if window.get_note_text().as_bytes() != source.as_bytes() {
                    return Err("opening page changed source bytes".into());
                }
                window.invoke_show_appearance();
                window.invoke_return_to_note();
                if window.get_active_view() != "note"
                    || window.get_note_text().as_bytes() != source.as_bytes()
                {
                    return Err("view callbacks lost note".into());
                }
                local.flush().await?;
                store.shutdown().await?;
                let reopened = StoreHandle::open(&path).await?;
                if reopened
                    .load_page(&original_key)
                    .await?
                    .ok_or("restart page absent")?
                    .body
                    .as_bytes()
                    != source.as_bytes()
                {
                    return Err("restart lost source".into());
                }
                reopened.shutdown().await?;
                Ok::<(), Box<dyn std::error::Error>>(())
            }
            .await;
            match check {
                Ok(()) => {
                    result.set(true);
                    println!("native_local_callbacks_and_restart: true");
                }
                Err(error) => eprintln!("local review: {error}"),
            }
            let _ = slint::quit_event_loop();
        });
    });
    slint::Timer::single_shot(Duration::from_secs(8), || {
        let _ = slint::quit_event_loop();
    });
    window.run()?;
    if !passed.get() {
        return Err("local native review failed".into());
    }
    Ok(())
}
