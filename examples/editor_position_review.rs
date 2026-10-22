//! Live native position capture across navigation and a supervised process restart.
#[cfg(all(target_os = "macos", feature = "native-validation"))]
#[path = "support/child_process.rs"]
mod child_process;
#[cfg(all(target_os = "macos", feature = "native-validation"))]
mod native {
    use bulletnotes::{
        AppWindow,
        app::{appearance_ui, coordinator::AppAction, local_ui, startup},
        domain::{connection::ConnectionConfig, editor::EditorPosition, page::*},
        platform::{application::NativeApplication, renderer, window_chrome},
        store::{StoreHandle, settings::AppSettings},
        theme::SystemAppearance,
    };
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSApplication, NSEvent, NSEventModifierFlags as Flags, NSEventType};
    use objc2_foundation::{NSPoint, NSProcessInfo, NSString};
    use slint::ComponentHandle;
    use std::{
        cell::RefCell,
        path::Path,
        rc::Rc,
        time::{Duration, Instant},
    };
    type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;
    fn source() -> String {
        format!(
            "café 日本語 👩🏽‍💻\r\n{}",
            "generated café 日本語 long line\r\n".repeat(100)
        )
    }
    async fn pause() -> Result {
        let (tx, rx) = tokio::sync::oneshot::channel();
        slint::Timer::single_shot(Duration::from_millis(150), move || {
            let _ = tx.send(());
        });
        rx.await?;
        Ok(())
    }
    fn observed(ui: &AppWindow) -> EditorPosition {
        EditorPosition {
            anchor: ui.get_editor_anchor() as usize,
            caret: ui.get_editor_caret() as usize,
            scroll_y: ui.get_editor_scroll_y(),
        }
    }
    fn check(ui: &AppWindow, want: EditorPosition) -> Result {
        if observed(ui) != want || ui.get_note_text() != source() {
            return Err(format!(
                "native position/source differs: {:?}, expected {want:?}",
                observed(ui)
            )
            .into());
        }
        Ok(())
    }
    async fn shift_left(ui: &AppWindow) -> Result {
        let window = window_chrome::native_window(ui.window())?;
        let app = NSApplication::sharedApplication(MainThreadMarker::from(&*window));
        if !window.isKeyWindow() || !app.isActive() {
            return Err("owned position window lost activation".into());
        }
        let text = NSString::from_str("\u{f702}");
        for (kind, code, flags) in [
            (NSEventType::FlagsChanged, 56, Flags::Shift),
            (NSEventType::KeyDown, 123, Flags::Shift),
            (NSEventType::KeyUp, 123, Flags::Shift),
            (NSEventType::FlagsChanged, 56, Flags::empty()),
        ] {
            let event = NSEvent::keyEventWithType_location_modifierFlags_timestamp_windowNumber_context_characters_charactersIgnoringModifiers_isARepeat_keyCode(
                kind, NSPoint::new(0.0,0.0), flags, NSProcessInfo::processInfo().systemUptime(), window.windowNumber(), None, &text, &text, false, code
            ).ok_or("owned position event construction failed")?;
            app.postEvent_atStart(&event, false);
        }
        pause().await
    }
    fn child(mode: &str, directory: &Path) -> Result {
        let application = NativeApplication::prepare()?;
        renderer::select()?;
        let runtime = tokio::runtime::Runtime::new()?;
        let _entered = runtime.enter();
        let connection = ConnectionConfig::parse("https://position.example/")?
            .id()
            .clone();
        let database = directory.join("position.sqlite");
        let opened = runtime.block_on(startup::open(&database, connection.clone()))?;
        let store: StoreHandle = opened.store;
        let first = PageKey {
            connection: connection.clone(),
            path: PagePath::parse("first.md")?,
        };
        let second = PageKey {
            connection: connection.clone(),
            path: PagePath::parse("second.md")?,
        };
        if mode == "write" {
            runtime.block_on(async {
                for key in [&first, &second] {
                    store.create_draft(key.clone()).await?;
                    store
                        .save_local(key.clone(), source(), LocalRevision(1))
                        .await?;
                }
                Ok::<_, Box<dyn std::error::Error>>(())
            })?;
        } else if opened.last_page.as_ref() != Some(&first) {
            return Err("restart did not remember the active page".into());
        }
        let ui = AppWindow::new()?;
        let appearance = appearance_ui::bind(
            &ui,
            store.clone(),
            AppSettings::default(),
            SystemAppearance::Light,
        )?;
        let local = local_ui::bind(&ui, store, connection, appearance)?;
        let frames = Rc::new(std::cell::Cell::new(0_u64));
        let rendered = frames.clone();
        ui.window().set_rendering_notifier(move |state, _| {
            if matches!(state, slint::RenderingState::AfterRendering) {
                rendered.set(rendered.get() + 1);
            }
        })?;
        let outcome = Rc::new(RefCell::new(None));
        let completed = outcome.clone();
        let weak = ui.as_weak();
        let expectation = directory.join("position.json");
        let writer = mode == "write";
        slint::spawn_local(async move {
            let result: Result = async {
                let ui = weak.upgrade().ok_or("owned position window closed")?;
                for _ in 0..4 {
                    pause().await?;
                }
                for _ in 0..40 {
                    if frames.get() > 0 {
                        break;
                    }
                    pause().await?;
                }
                if frames.get() == 0 {
                    return Err("owned position window did not render".into());
                }
                window_chrome::configure(ui.window())?;
                let window = window_chrome::native_window(ui.window())?;
                window.makeKeyAndOrderFront(None);
                NSApplication::sharedApplication(MainThreadMarker::from(&*window)).activate();
                local.dispatch(AppAction::OpenPage(first.clone()));
                local.wait_until_loaded(&first).await?;
                ui.invoke_focus_editor();
                let view = window.contentView().ok_or("owned position view missing")?;
                if !window.makeFirstResponder(Some(&view)) {
                    return Err("owned position responder unavailable".into());
                }
                pause().await?;
                let want = if writer {
                    let text = source();
                    let caret = text
                        .match_indices("generated")
                        .nth(15)
                        .ok_or("generated row missing")?
                        .0
                        + 4;
                    let anchor = caret + "generated café 日本語 long line\r\n".len();
                    ui.invoke_restore_editor_position(anchor as i32, caret as i32, 240.0);
                    pause().await?;
                    shift_left(&ui).await?;
                    let want = observed(&ui);
                    if want.anchor != anchor || want.caret != caret - 1 || want.scroll_y <= 0.0 {
                        return Err(format!(
                            "native backward selection was not established: {want:?}"
                        )
                        .into());
                    }
                    local.dispatch(AppAction::OpenPage(second.clone()));
                    local.wait_until_loaded(&second).await?;
                    local.dispatch(AppAction::OpenPage(first.clone()));
                    local.wait_until_loaded(&first).await?;
                    pause().await?;
                    check(&ui, want)?;
                    ui.invoke_show_appearance();
                    pause().await?;
                    ui.invoke_return_to_note();
                    pause().await?;
                    check(&ui, want)?;
                    // The final observation is captured by Quit, without an explicit flush.
                    ui.invoke_restore_editor_position(
                        want.anchor as i32,
                        want.caret as i32,
                        want.scroll_y + 40.0,
                    );
                    pause().await?;
                    let want = observed(&ui);
                    std::fs::write(&expectation, serde_json::to_vec(&want)?)?;
                    want
                } else {
                    serde_json::from_slice(&std::fs::read(&expectation)?)?
                };
                pause().await?;
                check(&ui, want)?;
                let start = Instant::now();
                while !window
                    .occlusionState()
                    .contains(objc2_app_kit::NSWindowOcclusionState::Visible)
                    || window.alphaValue() <= 0.0
                {
                    if start.elapsed() > Duration::from_secs(5) {
                        return Err("owned position capture is not visible".into());
                    }
                    pause().await?;
                }
                let previous = frames.get();
                ui.window().request_redraw();
                while frames.get() <= previous {
                    if start.elapsed() > Duration::from_secs(5) {
                        return Err("owned position capture did not render current content".into());
                    }
                    pause().await?;
                }
                // Allow the fresh content transaction to reach WindowServer before capturing.
                pause().await?;
                let path = capture_path(writer);
                let status = std::process::Command::new("/usr/sbin/screencapture")
                    .args(["-x", "-o", &format!("-l{}", window.windowNumber())])
                    .arg(&path)
                    .status()?;
                if !status.success() {
                    return Err("owned position screenshot failed".into());
                }
                require_note_pixels(&path)?;
                Ok(())
            }
            .await;
            let passed = result.is_ok();
            *completed.borrow_mut() = Some(result);
            if passed {
                local.dispatch(AppAction::RequestQuit);
            } else {
                let _ = slint::quit_event_loop();
            }
        })?;
        let watchdog = slint::Timer::default();
        let timed_out = Rc::new(std::cell::Cell::new(false));
        let expired = timed_out.clone();
        watchdog.start(
            slint::TimerMode::SingleShot,
            Duration::from_secs(30),
            move || {
                expired.set(true);
                let _ = slint::quit_event_loop();
            },
        );
        ui.show()?;
        application.activate();
        slint::run_event_loop_until_quit()?;
        ui.hide()?;
        if timed_out.get() {
            return Err("native position stage or Quit timed out".into());
        }
        outcome
            .borrow_mut()
            .take()
            .ok_or("native position stage timed out")??;
        // Successful Quit closes the store; opening it also checks committed durability.
        let reopened = runtime.block_on(StoreHandle::open(&database))?;
        let saved = runtime
            .block_on(
                reopened.load_page(&PageKey {
                    connection: ConnectionConfig::parse("https://position.example/")?
                        .id()
                        .clone(),
                    path: PagePath::parse("first.md")?,
                }),
            )?
            .ok_or("persisted native page missing")?;
        let want: EditorPosition =
            serde_json::from_slice(&std::fs::read(directory.join("position.json"))?)?;
        if saved.position != want || saved.body != source() {
            return Err("Quit did not persist the final live position".into());
        }
        runtime.block_on(reopened.shutdown())?;
        println!("native_editor_position_{mode}: true");
        Ok(())
    }
    fn require_note_pixels(path: &Path) -> Result {
        let mut reader =
            png::Decoder::new(std::io::BufReader::new(std::fs::File::open(path)?)).read_info()?;
        let mut pixels = vec![
            0;
            reader
                .output_buffer_size()
                .ok_or("capture image too large")?
        ];
        let info = reader.next_frame(&mut pixels)?;
        let channels = match (info.color_type, info.bit_depth) {
            (png::ColorType::Rgb, png::BitDepth::Eight) => 3,
            (png::ColorType::Rgba, png::BitDepth::Eight) => 4,
            _ => return Err("unexpected native capture format".into()),
        };
        let width = info.width as usize;
        let ink = pixels[..info.buffer_size()]
            .chunks_exact(channels)
            .enumerate()
            .filter(|(index, pixel)| {
                let x = index % width;
                let y = index / width;
                x > 40
                    && x < width.saturating_sub(40)
                    && y > 40
                    && y < info.height as usize - 40
                    && pixel[..3].iter().all(|channel| *channel < 160)
                    && (channels == 3 || pixel[3] > 200)
            })
            .count();
        if ink < 1000 {
            return Err("native capture does not contain the generated note".into());
        }
        Ok(())
    }
    fn capture_path(writer: bool) -> std::path::PathBuf {
        std::path::Path::new("target/editor-position").join(if writer {
            "native-before-restart.png"
        } else {
            "native-after-restart.png"
        })
    }
    pub fn run() -> Result {
        let arguments: Vec<_> = std::env::args_os().collect();
        if arguments.len() == 4 && arguments[1] == "--stage" {
            return child(
                arguments[2].to_str().ok_or("invalid position stage")?,
                Path::new(&arguments[3]),
            );
        }
        std::fs::create_dir_all("target/editor-position")?;
        let directory = tempfile::Builder::new()
            .prefix("bulletnotes-position-")
            .tempdir()?;
        for mode in ["write", "read"] {
            let mut child = crate::child_process::ChildGuard::new(
                std::process::Command::new(std::env::current_exe()?)
                    .arg("--stage")
                    .arg(mode)
                    .arg(directory.path())
                    .spawn()?,
            );
            let deadline = Instant::now() + Duration::from_secs(40);
            loop {
                if let Some(status) = child.try_wait()? {
                    if !status.success() {
                        return Err(format!("native position {mode} failed: {status}").into());
                    }
                    break;
                }
                if Instant::now() >= deadline {
                    return Err(format!("native position {mode} timed out").into());
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        }
        println!("native_editor_position_roundtrip_passed: true");
        Ok(())
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(all(target_os = "macos", feature = "native-validation"))]
    return native::run();
    #[cfg(not(all(target_os = "macos", feature = "native-validation")))]
    Err("requires macOS and --features native-validation".into())
}
