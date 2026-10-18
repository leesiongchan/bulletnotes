//! Isolated generated-text native recovery and separate-process restart fixture.
#[path = "../tests/support/space_fixture.rs"]
mod space_fixture;
use bulletnotes::{
    app::{
        native_app::LocalApplication,
        startup::{self, Startup},
    },
    domain::{connection::ConnectionConfig, error::StoreError, page::*},
    platform::{application::NativeApplication, window_chrome::native_window},
    store::{StoreHandle, settings::AppSettings},
    sync::{
        SilverBulletClient,
        probe::{new_probe_key, verify_write_guards},
        worker::SyncHandle,
    },
};
use slint::ComponentHandle;
use std::{
    cell::{Cell, RefCell},
    path::Path,
    rc::Rc,
    time::{Duration, Instant},
};
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
const LOCAL: &str = "Local recovery source\r\n日本語 🙂\r\n";
const MERGE: &str = "Merged recovery source\r\n日本語 🙂\r\n";
fn config() -> ConnectionConfig {
    ConnectionConfig::parse("https://recovery.example/").unwrap()
}
fn key(path: &str) -> PageKey {
    PageKey {
        connection: config().id().clone(),
        path: PagePath::parse(path).unwrap(),
    }
}
async fn pause(duration: Duration) -> Result<()> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    slint::Timer::single_shot(duration, move || {
        let _ = tx.send(());
    });
    rx.await?;
    Ok(())
}
async fn until(mut condition: impl FnMut() -> bool) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(8);
    while !condition() {
        if Instant::now() >= deadline {
            return Err("native recovery condition timed out".into());
        }
        pause(Duration::from_millis(15)).await?;
    }
    Ok(())
}
fn capture(window: &bulletnotes::AppWindow, name: &str) -> Result<()> {
    let native = native_window(window.window())?;
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(".superpowers/sdd/2026-10-17-bulletnotes")
        .join(name);
    let status = std::process::Command::new("/usr/sbin/screencapture")
        .args(["-x", "-o", &format!("-l{}", native.windowNumber())])
        .arg(path)
        .status()?;
    if !status.success() {
        return Err("owned recovery window capture failed".into());
    }
    Ok(())
}
fn restart(path: &Path, nonce: &str, draft: bool) -> Result<()> {
    if !path
        .parent()
        .and_then(Path::file_name)
        .is_some_and(|name| name.to_string_lossy().starts_with("bulletnotes-recovery-"))
        || std::fs::read_to_string(path.with_extension("fixture-marker"))? != nonce
    {
        return Err("not an owned recovery fixture".into());
    }
    let native = NativeApplication::prepare()?;
    let runtime = tokio::runtime::Runtime::new()?;
    let startup = runtime.block_on(startup::open(path, config().id().clone()))?;
    let store = startup.store.clone();
    let app = LocalApplication::new(startup, native)?;
    let local = app.local().clone();
    let weak = app.window().as_weak();
    let saved = store.clone();
    let passed = Rc::new(Cell::new(false));
    let observed = passed.clone();
    slint::spawn_local(async move {
        let result: Result<()> = async {
            until(|| local.current_key().is_some()).await?;
            local.flush().await?;
            let window = weak.upgrade().ok_or("closed recovery fixture")?;
            if draft {
                window.invoke_show_recovery();
                until(|| window.get_recovery_selected()).await?;
                if window.get_recovery_merge() != MERGE {
                    return Err("unapplied merged draft was not restored into review".into());
                }
                observed.set(true);
                window.invoke_request_quit();
                return Ok(());
            }
            window.set_recovery_section("trash".into());
            window.invoke_show_recovery();
            until(|| window.get_recovery_selected()).await?;
            if window.get_recovery_local() != MERGE {
                return Err("Trash source changed across restart".into());
            }
            window.set_recovery_destination("restored.md".into());
            window.invoke_recovery_restore();
            until(|| local.current_key() == Some(key("restored.md"))).await?;
            local.wait_until_loaded(&key("restored.md")).await?;
            if window.get_note_text() != MERGE {
                return Err("restored editor source changed".into());
            }
            let operation = saved
                .claim_pending(config().id(), 1)
                .await?
                .pop()
                .ok_or("no restore intent")?;
            if operation.condition != WriteCondition::CreateOnly
                || operation.kind != MutationKind::Put(MERGE.into())
            {
                return Err("restore was not exact create-only intent".into());
            }
            if saved.trash(config().id()).await?.len() != 1 {
                return Err("restoration discarded its Trash copy".into());
            }
            observed.set(true);
            window.invoke_request_quit();
            Ok(())
        }
        .await;
        if let Err(error) = result {
            eprintln!("native recovery restart: {error}");
            let _ = slint::quit_event_loop();
        }
    })?;
    app.run()?;
    if !passed.get()
        || !matches!(
            runtime.block_on(store.durability()),
            Err(StoreError::Closed)
        )
    {
        return Err("restart did not close its owned store durably".into());
    }
    println!("native_recovery_restart_passed: true");
    Ok(())
}
fn draft_contract() -> Result<()> {
    let native = NativeApplication::prepare()?;
    let runtime = tokio::runtime::Runtime::new()?;
    let dir = tempfile::Builder::new()
        .prefix("bulletnotes-recovery-")
        .tempdir()?;
    let path = dir.path().join("draft.sqlite");
    let nonce = uuid::Uuid::new_v4().to_string();
    std::fs::write(path.with_extension("fixture-marker"), &nonce)?;
    let store = runtime.block_on(StoreHandle::open(&path))?;
    let remote = |body: &str, revision: &str| {
        RemoteRead::Present(RemoteFile {
            key: key("one.md"),
            body: body.into(),
            revision: Some(RemoteRevision(format!("\"{revision}\""))),
            modified_ms: None,
            permission: Permission::ReadWrite,
        })
    };
    runtime.block_on(store.apply_remote(remote("base", "a")))?;
    runtime.block_on(store.save_local(key("one.md"), LOCAL.into(), LocalRevision(1)))?;
    runtime.block_on(store.apply_remote(remote("remote", "b")))?;
    let app = LocalApplication::new(
        Startup {
            store: store.clone(),
            settings: AppSettings {
                active_connection: Some(config().id().clone()),
                ..Default::default()
            },
            connection: config().id().clone(),
            last_page: Some(key("one.md")),
        },
        native,
    )?;
    let failure_path = path.clone();
    let retained_store = store.clone();
    let local = app.local().clone();
    let weak = app.window().as_weak();
    let passed = Rc::new(Cell::new(false));
    let observed = passed.clone();
    let expected_draft = Rc::new(RefCell::new(MERGE.to_owned()));
    let expected_after_typing = expected_draft.clone();
    slint::spawn_local(async move {
        let result: Result<()> = async {
            until(|| local.current_key() == Some(key("one.md"))).await?;
            local.wait_until_loaded(&key("one.md")).await?;
            let window = weak.upgrade().ok_or("closed draft fixture")?;
            window.invoke_show_recovery();
            until(|| window.get_recovery_selected()).await?;
            window.set_recovery_merge(MERGE.into());
            window.invoke_recovery_merge_edited();
            let injector=rusqlite::Connection::open(&failure_path)?;
            injector.execute_batch("CREATE TRIGGER fail_merge BEFORE INSERT ON merge_drafts BEGIN SELECT RAISE(ABORT,'generated failure'); END;")?;
            // Quit in this very event, before a 250ms autosave timer can run.
            window.invoke_request_quit();
            until(||window.get_status_text().contains("Merged draft could not be saved")).await?;
            if window.get_recovery_merge()!=MERGE || retained_store.durability().await.is_err(){return Err("failed draft save lost the buffer or exited".into());}
            injector.execute_batch("DROP TRIGGER fail_merge")?;
            // Reach the merge TextInput through normal keyboard focus, and prove
            // authoring resumes after a failed close before testing the race.
            let press = |text: slint::SharedString| {
                window.window().dispatch_event(slint::platform::WindowEvent::KeyPressed { text: text.clone() });
                window.window().dispatch_event(slint::platform::WindowEvent::KeyReleased { text });
            };
            window.invoke_focus_recovery_merge();
            press("Q".into());
            if window.get_recovery_merge() == MERGE { return Err("merge did not accept keyboard input after failed close".into()); }
            window.set_recovery_merge(MERGE.into()); window.invoke_recovery_merge_edited();
            pause(Duration::from_millis(350)).await?;
            if retained_store.merge_draft(&key("one.md")).await?.as_deref() != Some(MERGE) { return Err("merge baseline was not durable before metadata contention".into()); }
            injector.execute_batch("BEGIN IMMEDIATE")?;
            window.invoke_request_quit();
            pause(Duration::from_millis(40)).await?;
            press("Z".into());
            *expected_after_typing.borrow_mut() = window.get_recovery_merge().to_string();
            injector.execute_batch("ROLLBACK")?;
            observed.set(true);
            Ok(())
        }
        .await;
        if let Err(error) = result {
            eprintln!("draft fixture: {error}");
            let _ = slint::quit_event_loop();
        }
    })?;
    slint::Timer::single_shot(Duration::from_secs(15), || {
        let _ = slint::quit_event_loop();
    });
    app.run()?;
    if !passed.get()
        || !matches!(
            runtime.block_on(store.durability()),
            Err(StoreError::Closed)
        )
    {
        return Err("draft fixture did not close durably".into());
    }
    drop(app);
    let reopened = runtime.block_on(StoreHandle::open(&path))?;
    let retained = runtime.block_on(reopened.merge_draft(&key("one.md")))?;
    runtime.block_on(reopened.shutdown())?;
    if retained.as_deref() != Some(expected_draft.borrow().as_str()) {
        return Err("quit lost merge input accepted while settings were pending".into());
    }
    let child = std::process::Command::new(std::env::current_exe()?)
        .arg("--draft-restart")
        .arg(path)
        .arg(nonce)
        .output()?;
    if !child.status.success() {
        return Err(format!(
            "draft restart failed: {}",
            String::from_utf8_lossy(&child.stderr)
        )
        .into());
    }
    Ok(())
}
fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    if args.get(1).is_some_and(|arg| arg == "--draft-contract") {
        return draft_contract();
    }
    if args
        .get(1)
        .is_some_and(|arg| arg == "--restart" || arg == "--draft-restart")
    {
        return restart(
            Path::new(args.get(2).ok_or("missing owned path")?),
            args.get(3).ok_or("missing nonce")?,
            args.get(1).is_some_and(|arg| arg == "--draft-restart"),
        );
    }
    let draft = std::process::Command::new(std::env::current_exe()?)
        .arg("--draft-contract")
        .output()?;
    if !draft.status.success() {
        return Err(format!(
            "unapplied draft contract failed: {}",
            String::from_utf8_lossy(&draft.stderr)
        )
        .into());
    }
    let native = NativeApplication::prepare()?;
    let runtime = tokio::runtime::Runtime::new()?;
    let dir = tempfile::Builder::new()
        .prefix("bulletnotes-recovery-")
        .tempdir()?;
    let path = dir.path().join("notes.sqlite");
    let nonce = uuid::Uuid::new_v4().to_string();
    std::fs::write(path.with_extension("fixture-marker"), &nonce)?;
    let store = runtime.block_on(StoreHandle::open(&path))?;
    let fixture = runtime.block_on(space_fixture::SpaceFixture::start());
    let client = SilverBulletClient::for_loopback_fixture(
        &config(),
        fixture.base.clone(),
        None,
        Duration::from_secs(2),
    )?;
    let evidence = runtime.block_on(verify_write_guards(
        &client,
        new_probe_key(config().id().clone()),
    ))?;
    fixture.put("one.md", "base");
    runtime.block_on(store.apply_remote(runtime.block_on(client.read(&key("one.md")))?))?;
    runtime.block_on(store.save_local(key("one.md"), LOCAL.into(), LocalRevision(1)))?;
    fixture.put("one.md", "remote");
    runtime.block_on(store.apply_remote(runtime.block_on(client.read(&key("one.md")))?))?;
    let app = LocalApplication::new(
        Startup {
            store: store.clone(),
            settings: AppSettings {
                active_connection: Some(config().id().clone()),
                ..Default::default()
            },
            connection: config().id().clone(),
            last_page: Some(key("one.md")),
        },
        native,
    )?;
    let sync = {
        let _entered = runtime.enter();
        SyncHandle::start(store.clone(), client, Some(evidence))?
    };
    app.local().set_sync(Some(sync.clone()));
    let mut snapshots = sync.subscribe();
    let forwarding = app.local().clone();
    slint::spawn_local(async move {
        loop {
            forwarding.receive_sync(snapshots.borrow_and_update().clone());
            if snapshots.changed().await.is_err() {
                break;
            }
        }
    })?;
    let local = app.local().clone();
    let weak = app.window().as_weak();
    let saved = store.clone();
    let passed = Rc::new(Cell::new(false));
    let observed = passed.clone();
    slint::spawn_local(async move {
        let result: Result<()> = async {
            until(|| local.current_key() == Some(key("one.md"))).await?;
            local.wait_until_loaded(&key("one.md")).await?;
            let window = weak.upgrade().ok_or("closed recovery fixture")?;
            window
                .window()
                .set_size(slint::LogicalSize::new(400., 440.));
            window.invoke_theme_selected("paper".into());
            window.invoke_show_recovery();
            until(|| window.get_recovery_selected()).await?;
            if window.get_recovery_local() != LOCAL
                || window.get_recovery_remote() != "remote"
                || window.get_recovery_base() != "base"
            {
                return Err("native review omitted a retained version".into());
            }
            pause(Duration::from_millis(200)).await?;
            capture(&window, "task-10-recovery-paper.png")?;
            window.invoke_theme_selected("graphite".into());
            pause(Duration::from_millis(200)).await?;
            capture(&window, "task-10-recovery-graphite.png")?;
            window.set_recovery_merge(MERGE.into());
            window.invoke_recovery_merge_edited();
            window.invoke_return_to_note();
            if window.get_note_text() != LOCAL {
                return Err("leaving review replaced note text".into());
            }
            window.invoke_show_recovery();
            until(|| window.get_recovery_selected()).await?;
            if window.get_recovery_merge() != MERGE {
                return Err("leaving review lost the merged draft".into());
            }
            fixture.put("one.md", "newer remote");
            sync.refresh();
            until(|| window.get_recovery_stale()).await?;
            if window.get_recovery_merge() != MERGE {
                return Err("incoming update discarded merged draft".into());
            }
            window.invoke_recovery_reload();
            until(|| {
                !window.get_recovery_stale() && window.get_recovery_remote() == "newer remote"
            })
            .await?;
            window.invoke_recovery_apply("merge".into());
            if !window.get_recovery_busy() {
                return Err("recovery action did not enter its preparation phase".into());
            }
            window.invoke_request_hide();
            if window.get_lifecycle_pending() {
                return Err(
                    "close admitted while accepted recovery awaited its durability flush".into(),
                );
            }
            until(|| !window.get_recovery_busy()).await?;
            until(|| fixture.body("one.md").as_deref() == Some(MERGE)).await?;
            until(|| sync.snapshot().pending == 0 && sync.snapshot().conflicts == 0).await?;
            window.invoke_return_to_note();
            if window.get_note_text() != MERGE {
                return Err("merge did not install in the retained editor".into());
            }
            window.invoke_request_hide();
            until(|| !window.window().is_visible()).await?;
            window.invoke_native_reopen_requested();
            until(|| window.window().is_visible()).await?;
            window.invoke_trash_current_note();
            until(|| {
                window.get_active_view() == "recovery"
                    && window.get_recovery_section() == "trash"
                    && window.get_recovery_selected()
                    && !window.get_recovery_busy()
            })
            .await?;
            until(|| fixture.body("one.md").is_none()).await?;
            if saved.trash(config().id()).await?.len() != 1 {
                return Err("native delete has no durable recovery copy".into());
            }
            observed.set(true);
            window.invoke_request_quit();
            Ok(())
        }
        .await;
        if let Err(error) = result {
            eprintln!("native recovery contract: {error}");
            let _ = slint::quit_event_loop();
        }
    })?;
    slint::Timer::single_shot(Duration::from_secs(35), || {
        let _ = slint::quit_event_loop();
    });
    app.run()?;
    if !passed.get()
        || !matches!(
            runtime.block_on(store.durability()),
            Err(StoreError::Closed)
        )
    {
        return Err("native recovery contract did not finish durably".into());
    }
    drop(app);
    let child = std::process::Command::new(std::env::current_exe()?)
        .arg("--restart")
        .arg(&path)
        .arg(nonce)
        .output()?;
    if !child.status.success()
        || !String::from_utf8_lossy(&child.stdout).contains("native_recovery_restart_passed: true")
    {
        return Err(format!(
            "owned restart failed: {}",
            String::from_utf8_lossy(&child.stderr)
        )
        .into());
    }
    println!("native_recovery_contract_passed: true");
    Ok(())
}
