//! Generated-content check of the real local application composition and quit route.
use bulletnotes::{
    app::{native_app::LocalApplication, startup},
    domain::{connection::ConnectionConfig, page::*},
    platform::{application::NativeApplication, window_chrome::native_window},
    store::StoreHandle,
};
use slint::ComponentHandle;
use std::{cell::Cell, rc::Rc, time::Duration};
async fn pause() {
    let (sender, receiver) = tokio::sync::oneshot::channel();
    slint::Timer::single_shot(Duration::from_millis(20), move || {
        let _ = sender.send(());
    });
    let _ = receiver.await;
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().collect();
    if args.len() == 3 && args[1] == "--restart-generated" {
        return restart_generated(std::path::Path::new(&args[2]));
    }
    let native = NativeApplication::prepare()?;
    let runtime = tokio::runtime::Runtime::new()?;
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("lifecycle.sqlite");
    let connection = ConnectionConfig::parse("https://notes.example")?
        .id()
        .clone();
    let key = PageKey {
        connection: connection.clone(),
        path: PagePath::parse("existing.md")?,
    };
    let store = runtime.block_on(StoreHandle::open(&path))?;
    runtime.block_on(store.create_draft(key.clone()))?;
    runtime.block_on(store.save_local(
        key.clone(),
        "existing café\r\n👩🏽‍💻 日本語".into(),
        LocalRevision(1),
    ))?;
    runtime.block_on(store.record_visit(&key, 100))?;
    let mut settings = runtime.block_on(store.load_settings())?;
    settings.active_connection = Some(connection.clone());
    settings.appearance.font_size = 19.0;
    settings.window = Some(bulletnotes::store::settings::WindowPlacement {
        x: 90000.0,
        y: -90000.0,
        width: 430.0,
        height: 470.0,
        pinned: false,
    });
    runtime.block_on(store.save_settings(settings))?;
    runtime.block_on(store.shutdown())?;
    let startup = runtime.block_on(startup::open(&path, connection))?;
    let store = startup.store.clone();
    let app = Rc::new(LocalApplication::new(startup, native)?);
    let checked = Rc::new(Cell::new(false));
    let passed = checked.clone();
    let running = app.clone();
    let review_path = path.clone();
    let review_key = key.clone();
    slint::Timer::single_shot(Duration::from_millis(300), move || {
        let _ = slint::spawn_local(async move {
            let result = async {
                running.local().wait_until_loaded(&review_key).await?;
                let window = running.window();
                if window.get_font_size_px() != 19.0 || window.get_note_text() != "existing café\r\n👩🏽‍💻 日本語" { return Err("startup did not restore note and appearance".into()); }
                // A pending theme write must not be overwritten by window persistence.
                window.set_font_size_px(21.0);
                window.invoke_appearance_changed();
                let changed = "unsaved before close café\r\n👩🏽‍💻 日本語";
                window.set_note_text(changed.into());
                window.invoke_note_edited(changed.into());
                native_window(window.window())?.performClose(None);
                for _ in 0..200 { if !window.window().is_visible() { break; } pause().await; }
                if window.window().is_visible() { return Err("native close did not hide".into()); }
                if store.load_page(&review_key).await?.ok_or("page absent")?.body != changed { return Err("native close hid before saving".into()); }
                running.show()?;
                let injector = rusqlite::Connection::open(&review_path)?;
                injector.execute_batch("CREATE TRIGGER fail_note BEFORE UPDATE OF body ON pages BEGIN SELECT RAISE(ABORT,'generated failure'); END;")?;
                let failed = "retain failed quit café\r\n👩🏽‍💻 日本語";
                window.set_note_text(failed.into());
                window.invoke_note_edited(failed.into());
                running.request_native_termination();
                if running.local().wait_until_saved().await.is_ok() { return Err("failed quit was acknowledged".into()); }
                if window.get_note_text() != failed || !window.window().is_visible() || window.get_note_read_only() { return Err("failed quit did not retain an editable window".into()); }
                injector.execute_batch("DROP TRIGGER fail_note")?;
                passed.set(true);
                running.request_native_termination();
                Ok::<(), Box<dyn std::error::Error>>(())
            }.await;
            if let Err(error) = result {
                eprintln!("lifecycle check: {error}");
                let _ = slint::quit_event_loop();
            }
        });
    });
    let timed_out = Rc::new(Cell::new(false));
    let timeout = timed_out.clone();
    slint::Timer::single_shot(Duration::from_secs(10), move || {
        timeout.set(true);
        let _ = slint::quit_event_loop();
    });
    app.run()?;
    if !checked.get() || timed_out.get() {
        return Err("native lifecycle check failed".into());
    }
    drop(app);
    let reopened = runtime.block_on(StoreHandle::open(&path))?;
    let saved = runtime
        .block_on(reopened.load_page(&key))?
        .ok_or("reopened page absent")?;
    if saved.body != "retain failed quit café\r\n👩🏽‍💻 日本語" {
        return Err("native quit did not save the latest buffer".into());
    }
    let settings = runtime.block_on(reopened.load_settings())?;
    let placement = settings.window.ok_or("window placement absent")?;
    if placement.x > 80000.0 || placement.y < -80000.0 || settings.appearance.font_size != 21.0 {
        return Err("window or appearance settings did not persist".into());
    }
    runtime.block_on(reopened.shutdown())?;
    let child = std::process::Command::new(std::env::current_exe()?)
        .arg("--restart-generated")
        .arg(&path)
        .output()?;
    if !child.status.success()
        || !String::from_utf8_lossy(&child.stdout).contains("generated_process_restart: true")
    {
        return Err(format!(
            "generated process restart failed: {}",
            String::from_utf8_lossy(&child.stderr)
        )
        .into());
    }
    println!("native_startup_close_and_quit: true");
    Ok(())
}

fn restart_generated(path: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    let native = NativeApplication::prepare()?;
    let runtime = tokio::runtime::Runtime::new()?;
    let connection = ConnectionConfig::parse("https://notes.example")?
        .id()
        .clone();
    let startup = runtime.block_on(startup::open(path, connection))?;
    let key = startup
        .last_page
        .clone()
        .ok_or("restart last page absent")?;
    let app = Rc::new(LocalApplication::new(startup, native)?);
    let checked = Rc::new(Cell::new(false));
    let passed = checked.clone();
    let running = app.clone();
    slint::Timer::single_shot(Duration::from_millis(300), move || {
        let _ = slint::spawn_local(async move {
            let result = async {
                running.local().wait_until_loaded(&key).await?;
                if running.window().get_note_text() != "retain failed quit café\r\n👩🏽‍💻 日本語"
                    || running.window().get_font_size_px() != 21.0
                {
                    return Err("process restart changed note or appearance".into());
                }
                passed.set(true);
                running.request_native_termination();
                Ok::<(), Box<dyn std::error::Error>>(())
            }
            .await;
            if let Err(error) = result {
                eprintln!("restart check: {error}");
                let _ = slint::quit_event_loop();
            }
        });
    });
    let timed_out = Rc::new(Cell::new(false));
    let timeout = timed_out.clone();
    slint::Timer::single_shot(Duration::from_secs(5), move || {
        timeout.set(true);
        let _ = slint::quit_event_loop();
    });
    app.run()?;
    if !checked.get() || timed_out.get() {
        return Err("generated restart check failed".into());
    }
    println!("generated_process_restart: true");
    Ok(())
}
