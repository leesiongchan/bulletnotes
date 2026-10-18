//! Generated owned-data checks for the real mounted editor and RecoveryUi admission bridge.
use bulletnotes::{
    app::{native_app::LocalApplication, startup::Startup},
    domain::{connection::ConnectionConfig, page::*},
    platform::{application::NativeApplication, window_chrome::native_window},
    store::{StoreHandle, settings::AppSettings},
    sync::worker::SyncSnapshot,
};
use slint::ComponentHandle;
use std::{
    cell::Cell,
    rc::Rc,
    time::{Duration, Instant},
};
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
const BASE: &str = "red fox jumps\n\nstable café 日本語\n";
const DRAFT: &str = "admitted draft\r\n日本語 👩🏽‍💻";
fn key() -> PageKey {
    PageKey {
        connection: ConnectionConfig::parse("https://editor-fixture.example/")
            .unwrap()
            .id()
            .clone(),
        path: PagePath::parse("owned.md").unwrap(),
    }
}
fn remote(body: &str, revision: &str) -> RemoteRead {
    RemoteRead::Present(RemoteFile {
        key: key(),
        body: body.into(),
        revision: Some(RemoteRevision(revision.into())),
        modified_ms: None,
        permission: Permission::ReadWrite,
    })
}
async fn pause() -> Result<()> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    slint::Timer::single_shot(Duration::from_millis(35), move || {
        let _ = tx.send(());
    });
    rx.await?;
    Ok(())
}
async fn until(mut test: impl FnMut() -> bool) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(8);
    while !test() {
        if Instant::now() >= deadline {
            return Err("generated native fixture timed out".into());
        }
        pause().await?;
    }
    Ok(())
}
fn key_event(window: &bulletnotes::AppWindow, text: slint::SharedString, pressed: bool) {
    window.window().dispatch_event(if pressed {
        slint::platform::WindowEvent::KeyPressed { text }
    } else {
        slint::platform::WindowEvent::KeyReleased { text }
    });
}
fn type_text(window: &bulletnotes::AppWindow, text: &str) {
    key_event(window, text.into(), true);
    key_event(window, text.into(), false);
}
fn undo(window: &bulletnotes::AppWindow, redo: bool) {
    key_event(window, slint::platform::Key::Meta.into(), true);
    if redo {
        key_event(window, slint::platform::Key::Shift.into(), true);
    }
    type_text(window, "z");
    if redo {
        key_event(window, slint::platform::Key::Shift.into(), false);
    }
    key_event(window, slint::platform::Key::Meta.into(), false);
}
fn capture(window: &bulletnotes::AppWindow, name: &str) -> Result<()> {
    let native = native_window(window.window())?;
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(".superpowers/sdd/2026-10-18-automatic-sync")
        .join(name);
    if !std::process::Command::new("/usr/sbin/screencapture")
        .args(["-x", "-o", &format!("-l{}", native.windowNumber())])
        .arg(path)
        .status()?
        .success()
    {
        return Err("owned fixture capture failed".into());
    }
    Ok(())
}
fn snapshot() -> SyncSnapshot {
    SyncSnapshot {
        connection: key().connection,
        writable: true,
        indexed: 1,
        catalogue_total: 1,
        complete: true,
        pending: 0,
        conflicts: 0,
        error: None,
        page_errors: Default::default(),
        generation: 1,
    }
}
fn main() -> Result<()> {
    let native = NativeApplication::prepare()?;
    let runtime = tokio::runtime::Runtime::new()?;
    let directory = tempfile::Builder::new()
        .prefix("bulletnotes-editor-rebase-")
        .tempdir()?;
    let path = directory.path().join("owned.sqlite");
    let store = runtime.block_on(StoreHandle::open(&path))?;
    runtime.block_on(store.apply_remote(remote(BASE, "a")))?;
    let app = LocalApplication::new(
        Startup {
            store: store.clone(),
            settings: AppSettings {
                active_connection: Some(key().connection.clone()),
                ..Default::default()
            },
            connection: key().connection,
            last_page: Some(key()),
        },
        native,
    )?;
    let local = app.local().clone();
    let weak = app.window().as_weak();
    let saved = store.clone();
    let passed = Rc::new(Cell::new(false));
    let observed = passed.clone();
    slint::spawn_local(async move {
        let result: Result<()> = async {
            until(|| local.current_key() == Some(key())).await?;
            local.wait_until_loaded(&key()).await?;
            let window = weak.upgrade().ok_or("closed fixture")?;
            until(|| window.get_note_text() == BASE).await?;
            eprintln!("fixture phase: loaded");
            window.invoke_focus_editor();
            window.invoke_restore_editor_position(0, 0, 0.0);
            type_text(&window, "Q");
            pause().await?;
            assert_eq!(window.get_note_text(), format!("Q{BASE}"));
            undo(&window, false);
            pause().await?;
            assert_eq!(window.get_note_text(), BASE);
            undo(&window, true);
            pause().await?;
            assert_eq!(window.get_note_text(), format!("Q{BASE}"));
            local.flush().await?;
            pause().await?;
            // A same-text status render must leave genuine keyboard undo intact.
            local.receive_sync(snapshot());
            pause().await?;
            undo(&window, false);
            pause().await?;
            assert_eq!(window.get_note_text(), BASE);
            undo(&window, true);
            pause().await?;
            local.flush().await?;
            pause().await?;
            eprintln!("fixture phase: keyboard controls passed");
            capture(&window, "task5-editor-before.png")?;
            let remote_body = BASE.replace("jumps", "runs");
            let background = saved.clone();
            let observation = remote(&remote_body, "b");
            tokio::spawn(async move { background.apply_remote(observation).await }).await??;
            eprintln!("fixture phase: remote stored");
            local.receive_sync(snapshot());
            until(|| window.get_note_text() == format!("Q{remote_body}")).await?;
            pause().await?;
            let installed = window.get_note_text();
            undo(&window, false);
            pause().await?;
            // Pinned Slint clears existing undo on external source assignment; record the limit.
            assert_eq!(window.get_note_text(), installed);
            capture(&window, "task5-editor-after.png")?;
            println!("native_keyboard_undo_redo_control_and_noop_guard: true");
            println!("native_external_replacement_clears_existing_undo_observed: true");
            let current = saved
                .load_page(&key())
                .await?
                .ok_or("missing fixture source")?;
            saved
                .save_conflict(ConflictRecord {
                    id: uuid::Uuid::new_v4(),
                    key: key(),
                    base: current.remote_base.clone(),
                    local: current,
                    remote: remote("later server source", "c"),
                })
                .await?;
            window.invoke_show_recovery();
            until(|| window.get_recovery_selected()).await?;
            window.set_recovery_merge(DRAFT.into());
            window.invoke_recovery_merge_edited();
            let injector = rusqlite::Connection::open(path)?;
            injector.execute_batch("BEGIN IMMEDIATE")?;
            let background = saved.clone();
            let job = tokio::spawn(async move {
                background
                    .apply_remote(remote("later server source", "c"))
                    .await
            });
            eprintln!("fixture phase: draft admitted and worker started");
            until(|| window.get_recovery_draft_closed()).await?;
            window.invoke_focus_recovery_merge();
            type_text(&window, "Z");
            assert_eq!(window.get_recovery_merge(), DRAFT);
            assert!(!window.get_recovery_busy());
            assert!(!window.get_note_read_only());
            injector.execute_batch("ROLLBACK")?;
            job.await??;
            until(|| window.get_recovery_stale()).await?;
            assert_eq!(window.get_active_view(), "recovery");
            assert_eq!(window.get_recovery_merge(), DRAFT);
            let heads = saved.recovery_uploads(&key().connection, 20).await?;
            let mut found = false;
            for head in heads {
                let event = saved.automatic_recovery_event(head.event_id).await?;
                found |= event.input.draft.as_deref() == Some(DRAFT);
            }
            assert!(found);
            assert!(
                saved
                    .recovery_copies(&key().connection)
                    .await?
                    .iter()
                    .any(|copy| copy.page.body == DRAFT)
            );
            println!("native_worker_origin_closes_and_drains_admitted_draft: true");
            println!("native_selection_endpoint_IME_VoiceOver_fidelity: unverified");
            observed.set(true);
            window.invoke_request_quit();
            Ok(())
        }
        .await;
        if let Err(error) = result {
            if let Some(window) = weak.upgrade() {
                eprintln!(
                    "fixture wait evidence: status={} native={} view={} key_loaded={}",
                    window.get_status_text(),
                    window.get_native_message(),
                    window.get_active_view(),
                    local.current_key().is_some()
                );
            }
            eprintln!("editor reconciliation fixture: {error}");
            let _ = slint::quit_event_loop();
        }
    })?;
    slint::Timer::single_shot(Duration::from_secs(25), || {
        let _ = slint::quit_event_loop();
    });
    let _entered = runtime.enter();
    app.run()?;
    eprintln!("fixture frames: {}", app.rendered_frame_count());
    if !passed.get() {
        return Err("generated native editor reconciliation fixture failed".into());
    }
    Ok(())
}
