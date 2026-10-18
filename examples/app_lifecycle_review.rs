//! Generated-content check of the real local application composition and quit route.
#[cfg(feature = "protocol-validation")]
#[path = "../tests/support/http_fixture.rs"]
mod http_fixture;
use bulletnotes::{
    app::{native_app::LocalApplication, startup},
    domain::{connection::ConnectionConfig, page::*},
    platform::{application::NativeApplication, window_chrome::native_window},
    store::StoreHandle,
};
use slint::ComponentHandle;
use std::{cell::Cell, rc::Rc, time::Duration};
fn probe_shortcut(chord: &str, available: bool) -> Result<(), Box<dyn std::error::Error>> {
    let output = std::process::Command::new(std::env::current_exe()?)
        .arg("--shortcut-probe")
        .arg(chord)
        .arg(if available { "available" } else { "busy" })
        .output()?;
    if !output.status.success() {
        return Err(format!(
            "owned shortcut probe: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    Ok(())
}
struct HeldShortcut(std::process::Child);
impl Drop for HeldShortcut {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn hold_shortcut(chord: &str) -> Result<HeldShortcut, Box<dyn std::error::Error>> {
    use std::io::BufRead;
    let mut child = std::process::Command::new(std::env::current_exe()?)
        .arg("--hold-shortcut")
        .arg(chord)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()?;
    let mut line = String::new();
    std::io::BufReader::new(child.stdout.take().ok_or("owned shortcut output missing")?)
        .read_line(&mut line)?;
    if line.trim() != "owned_shortcut_held: true" {
        let _ = child.kill();
        let _ = child.wait();
        return Err("owned shortcut holder did not start".into());
    }
    Ok(HeldShortcut(child))
}
async fn pause() {
    let (sender, receiver) = tokio::sync::oneshot::channel();
    slint::Timer::single_shot(Duration::from_millis(20), move || {
        let _ = sender.send(());
    });
    let _ = receiver.await;
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().collect();
    if args
        .get(1)
        .is_some_and(|arg| arg == "--shortcut-probe" || arg == "--hold-shortcut")
    {
        use std::{io::Write, str::FromStr};
        let _native = NativeApplication::prepare()?;
        use bulletnotes::platform::hotkey::{HotkeyBackend, NativeHotkeyBackend};
        let manager = NativeHotkeyBackend::new()?;
        let chord = args
            .get(2)
            .ok_or("missing owned shortcut")?
            .to_string_lossy();
        let key = global_hotkey::hotkey::HotKey::from_str(&chord)?;
        let registered = manager.register(key);
        if args[1] == "--hold-shortcut" {
            registered?;
            println!("owned_shortcut_held: true");
            std::io::stdout().flush()?;
            let mut release = String::new();
            std::io::stdin().read_line(&mut release)?;
            manager.unregister(key)?;
        } else {
            let available = args.get(3).is_some_and(|arg| arg == "available");
            if registered.is_ok() {
                manager.unregister(key)?;
            }
            if registered.is_ok() != available {
                return Err(format!(
                    "owned shortcut availability expected={available}, registered={}, error={:?}",
                    registered.is_ok(),
                    registered.err()
                )
                .into());
            }
        }
        return Ok(());
    }
    if args.len() == 4 && args[1] == "--activate-owned-store" {
        let path = std::path::Path::new(&args[2]);
        if !path
            .parent()
            .and_then(std::path::Path::file_name)
            .is_some_and(|name| name.to_string_lossy().starts_with("bulletnotes-lifecycle-"))
            || std::fs::read_to_string(path.with_extension("fixture-marker"))?
                != args[3].to_string_lossy()
        {
            return Err("not an owned lifecycle fixture".into());
        }
        let native = NativeApplication::prepare()?;
        let runtime = tokio::runtime::Runtime::new()?;
        if !matches!(
            runtime.block_on(StoreHandle::open(path)),
            Err(bulletnotes::domain::error::StoreError::AlreadyRunning)
        ) {
            return Err("second launcher did not reject a second writer".into());
        }
        native.activate_store_owner(bulletnotes::store::owner_process_id(path)?)?;
        println!("owned_instance_activation_sent: true");
        return Ok(());
    }
    if args.len() == 3 && args[1] == "--restart-generated" {
        return restart_generated(std::path::Path::new(&args[2]));
    }
    let about_capture = args
        .windows(2)
        .find(|pair| pair[0] == "--about-capture")
        .map(|pair| std::path::PathBuf::from(&pair[1]));
    let native = NativeApplication::prepare()?;
    let runtime = tokio::runtime::Runtime::new()?;
    let directory = tempfile::Builder::new()
        .prefix("bulletnotes-lifecycle-")
        .tempdir()?;
    let path = directory.path().join("lifecycle.sqlite");
    let nonce = uuid::Uuid::new_v4().to_string();
    std::fs::write(path.with_extension("fixture-marker"), &nonce)?;
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
    settings.hotkey = bulletnotes::store::settings::HotkeySettings {
        key: "KeyJ".into(),
        alt: true,
        control: true,
        shift: true,
        command: false,
    };
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
    #[cfg(feature = "protocol-validation")]
    let blocked_fixture = {
        let fixture = runtime.block_on(http_fixture::Fixture::start(Some(
            b"owned blocked response",
        )));
        fixture.data.lock().unwrap().catalogue_override = Some(
            serde_json::json!([{"name":"blocked.md","lastModified":100,"size":22,"perm":"rw"}]),
        );
        fixture.fault(http_fixture::Fault::BlockOneRead);
        let config = ConnectionConfig::parse("https://notes.example")?;
        let client = bulletnotes::sync::SilverBulletClient::for_loopback_fixture(
            &config,
            fixture.base.clone(),
            None,
            Duration::from_secs(60),
        )?;
        let sync = runtime.block_on(async {
            bulletnotes::sync::worker::SyncHandle::start(store.clone(), client, None)
        })?;
        app.local().set_sync(Some(sync));
        Rc::new(fixture)
    };
    #[cfg(feature = "protocol-validation")]
    let blocked_check = blocked_fixture.clone();
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
                #[cfg(feature="protocol-validation")]
                {
                    for _ in 0..200{if blocked_check.data.lock().unwrap().active_reads==1{break;}pause().await;}
                    if blocked_check.data.lock().unwrap().active_reads!=1{return Err("fixture did not hold a remote read".into());}
                }
                eprintln!("lifecycle stage: shortcut contracts; startup_warning={}, status={}",window.get_native_warning(),window.get_native_message());
                {
                    let old="Control+Option+Shift+J";let candidate="Control+Option+Shift+K";
                    probe_shortcut(old,false)?;
                    let holder=hold_shortcut("Control+Option+Shift+L")?;
                    window.set_hotkey_accelerator("Control+Option+Shift+L".into());window.invoke_apply_hotkey();
                    if !window.get_native_warning()||store.load_settings().await?.hotkey.key!="KeyJ"{return Err("busy shortcut was silently committed".into());}
                    drop(holder);
                    let injector=rusqlite::Connection::open(&review_path)?;
                    injector.execute_batch("CREATE TRIGGER fail_shortcut BEFORE UPDATE OF value ON settings BEGIN SELECT RAISE(ABORT,'generated failure'); END;")?;
                    window.set_hotkey_accelerator(candidate.into());window.invoke_apply_hotkey();
                    for _ in 0..200{if !window.get_hotkey_busy(){break;}pause().await;}
                    if store.load_settings().await?.hotkey.key!="KeyJ"{return Err("failed settings save changed the durable shortcut".into());}
                    probe_shortcut(old,false)?;probe_shortcut(candidate,true)?;
                    injector.execute_batch("DROP TRIGGER fail_shortcut")?;
                    // A rejected hotkey must not be inherited by a later window-only save.
                    injector.execute_batch("CREATE TRIGGER reject_hotkey_only BEFORE UPDATE OF value ON settings WHEN json_extract(NEW.value,'$.hotkey.key')='KeyK' AND json_extract(NEW.value,'$.window.x')=json_extract(OLD.value,'$.window.x') BEGIN SELECT RAISE(ABORT,'generated hotkey failure'); END;")?;
                    let mut placement=store.load_settings().await?.window.ok_or("placement absent")?;placement.x+=1.0;
                    injector.execute_batch("BEGIN IMMEDIATE")?;
                    window.invoke_apply_hotkey();pause().await;
                    let expected=placement.clone();let appearance=running.appearance().clone();let (sender,receiver)=tokio::sync::oneshot::channel();
                    slint::spawn_local(async move{let _=sender.send(appearance.save_window(placement).await);})?;
                    pause().await;injector.execute_batch("COMMIT")?;
                    receiver.await??;
                    for _ in 0..200{if !window.get_hotkey_busy(){break;}pause().await;}
                    let after=store.load_settings().await?;
                    if after.hotkey.key!="KeyJ" || after.window.as_ref()!=Some(&expected){return Err("window metadata resurrected a rejected shortcut".into());}
                    probe_shortcut(old,false)?;probe_shortcut(candidate,true)?;
                    injector.execute_batch("DROP TRIGGER reject_hotkey_only")?;
                    // A queued appearance ticket must retain a successfully changed key.
                    injector.execute_batch("BEGIN IMMEDIATE")?;
                    window.invoke_apply_hotkey();pause().await;
                    window.set_font_size_px(20.0);window.invoke_appearance_changed();pause().await;
                    injector.execute_batch("COMMIT")?;
                    running.appearance().flush().await?;
                    if store.load_settings().await?.appearance.font_size!=20.0{return Err("queued font preference was lost".into());}
                    for _ in 0..200{if !window.get_hotkey_busy(){break;}pause().await;}
                    if store.load_settings().await?.hotkey.key!="KeyK"{return Err("valid shortcut was not saved".into());}
                    probe_shortcut(old,true)?;probe_shortcut(candidate,false)?;
                }
                eprintln!("lifecycle stage: appearance observation");
                {
                    use objc2_app_kit::{NSApplication,NSAppearance,NSAppearanceNameAqua,NSAppearanceNameDarkAqua};
                    let app=NSApplication::sharedApplication(objc2::MainThreadMarker::new().ok_or("not main thread")?);
                    for dark in [!window.get_theme().dark,window.get_theme().dark]{
                        // SAFETY: immutable appearance-name constants exported by AppKit.
                        let name=unsafe{if dark{NSAppearanceNameDarkAqua}else{NSAppearanceNameAqua}};
                        let appearance=NSAppearance::appearanceNamed(name).ok_or("appearance unavailable")?;app.setAppearance(Some(&appearance));
                        for _ in 0..200{if window.get_theme().dark==dark{break;}pause().await;}
                        if window.get_theme().dark!=dark||window.get_note_text()!="existing café\r\n👩🏽‍💻 日本語"{return Err("appearance observation did not update the theme safely".into());}
                    }
                    app.setAppearance(None);
                }
                // A pending theme write must not be overwritten by window persistence.
                window.set_font_size_px(21.0);
                window.invoke_appearance_changed();
                let changed = "unsaved before close café\r\n👩🏽‍💻 日本語";
                window.set_note_text(changed.into());
                window.invoke_note_edited(changed.into());
                eprintln!("lifecycle stage: native close");
                native_window(window.window())?.performClose(None);
                for _ in 0..200 { if !window.window().is_visible() { break; } pause().await; }
                if window.window().is_visible() { return Err("native close did not hide".into()); }
                if store.load_page(&review_key).await?.ok_or("page absent")?.body != changed { return Err("native close hid before saving".into()); }
                #[cfg(feature="protocol-validation")]
                if blocked_check.data.lock().unwrap().active_reads!=1{return Err("hide required the held network response".into());}
                eprintln!("lifecycle stage: About while note is hidden");
                window.invoke_show_about();
                let mut about_observed=false;
                for _ in 0..200 {
                    let app=objc2_app_kit::NSApplication::sharedApplication(objc2::MainThreadMarker::new().ok_or("not main thread")?);
                    if let Some(about)=app.windows().iter().find(|native|native.title().to_string()=="About Bulletnotes" && native.isVisible()) {
                        pause().await;
                        if window.window().is_visible(){return Err("About action unexpectedly reopened the hidden note".into());}
                        about_observed=true;
                        if let Some(path)=&about_capture {
                            for _ in 0..100 {if about.occlusionState().contains(objc2_app_kit::NSWindowOcclusionState::Visible){break;}pause().await;}
                            eprintln!("About capture: number={}, visible={}, occlusion={}, app_hidden={}",about.windowNumber(),about.isVisible(),about.occlusionState().bits(),app.isHidden());
                            let status=std::process::Command::new("/usr/sbin/screencapture").args(["-x","-o",&format!("-l{}",about.windowNumber())]).arg(path).status()?;
                            if !status.success(){return Err("About window capture failed".into());}
                        }
                        about.performClose(None);break;
                    }
                    pause().await;
                }
                if !about_observed {return Err("About window was not observed".into());}
                eprintln!("lifecycle stage: owned second launch");
                let executable=std::env::current_exe().map_err(|error|format!("owned executable lookup: {error}"))?;
                eprintln!("lifecycle executable exists: {}",executable.is_file());
                let child=std::process::Command::new(executable).arg("--activate-owned-store").arg(&review_path).arg(&nonce).output().map_err(|error|format!("owned launcher spawn: {error}"))?;
                if !child.status.success(){return Err(format!("owned launcher failed: {}",String::from_utf8_lossy(&child.stderr)).into());}
                for _ in 0..200{if window.window().is_visible(){break;}pause().await;}
                if !window.window().is_visible(){return Err("second launcher activated the app but did not reopen its hidden window".into());}
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
    slint::Timer::single_shot(Duration::from_secs(25), move || {
        timeout.set(true);
        let _ = slint::quit_event_loop();
    });
    app.run()?;
    if !checked.get() || timed_out.get() {
        return Err("native lifecycle check failed".into());
    }
    #[cfg(feature = "protocol-validation")]
    if blocked_fixture.data.lock().unwrap().active_reads != 1 {
        return Err("quit required the held network response".into());
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
