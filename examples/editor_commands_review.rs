//! Generated native command/history/persistence proof; temporary store, no network.
#[cfg(all(target_os = "macos", feature = "native-validation"))]
mod native {
    use bulletnotes::{
        AppWindow,
        app::{appearance_ui, coordinator::AppAction, local_ui},
        domain::connection::ConnectionConfig,
        platform::{application::NativeApplication, renderer, window_chrome},
        store::{StoreHandle, settings::AppSettings},
        theme::SystemAppearance,
    };
    use objc2::{MainThreadMarker, rc::Retained};
    use objc2_app_kit::{NSApplication, NSEvent, NSEventModifierFlags as Flags, NSEventType};
    use objc2_foundation::{NSPoint, NSProcessInfo, NSString};
    use slint::ComponentHandle;
    use std::{cell::RefCell, rc::Rc, time::Duration};
    type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

    async fn pause() -> Result<()> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        slint::Timer::single_shot(Duration::from_millis(150), move || {
            let _ = tx.send(());
        });
        rx.await?;
        Ok(())
    }
    async fn key(ui: &AppWindow, text: &str, code: u16, flags: Flags) -> Result<()> {
        let window = window_chrome::native_window(ui.window())?;
        let app = NSApplication::sharedApplication(MainThreadMarker::from(&*window));
        if !window.isKeyWindow() || !app.isActive() {
            return Err("owned command window lost activation".into());
        }
        let modifiers = [
            (Flags::Command, 55),
            (Flags::Shift, 56),
            (Flags::Option, 58),
        ];
        let mut held = Flags::empty();
        let mut events = Vec::new();
        for (flag, code) in modifiers {
            if flags.contains(flag) {
                held.insert(flag);
                events.push((NSEventType::FlagsChanged, code, held));
            }
        }
        events.extend([
            (NSEventType::KeyDown, code, flags),
            (NSEventType::KeyUp, code, flags),
        ]);
        for (flag, code) in modifiers.into_iter().rev() {
            if flags.contains(flag) {
                held.remove(flag);
                events.push((NSEventType::FlagsChanged, code, held));
            }
        }
        let characters = if flags.contains(Flags::Shift) {
            text.to_uppercase()
        } else {
            text.to_owned()
        };
        let text = NSString::from_str(&characters);
        for (kind, code, flags) in events {
            let event = NSEvent::keyEventWithType_location_modifierFlags_timestamp_windowNumber_context_characters_charactersIgnoringModifiers_isARepeat_keyCode(
                kind,NSPoint::new(0.0,0.0),flags,NSProcessInfo::processInfo().systemUptime(),window.windowNumber(),None,&text,&text,false,code
            ).ok_or("native command event construction failed")?;
            app.postEvent_atStart(&event, false);
        }
        pause().await
    }
    fn check(condition: bool, message: &'static str) -> Result<()> {
        if condition {
            Ok(())
        } else {
            Err(message.into())
        }
    }
    fn capture(ui: &AppWindow, name: &str) -> Result<()> {
        let window = window_chrome::native_window(ui.window())?;
        std::fs::create_dir_all("target/antinote-interactions")?;
        let path = format!("target/antinote-interactions/{name}.png");
        let status = std::process::Command::new("/usr/sbin/screencapture")
            .args(["-x", "-o", &format!("-l{}", window.windowNumber()), &path])
            .status()?;
        check(status.success(), "owned command capture failed")
    }
    pub fn run() -> Result<()> {
        let application = NativeApplication::prepare()?;
        renderer::select()?;
        let runtime = tokio::runtime::Runtime::new()?;
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("commands.sqlite");
        let store = runtime.block_on(StoreHandle::open(&path))?;
        let ui = AppWindow::new()?;
        let appearance = appearance_ui::bind(
            &ui,
            store.clone(),
            AppSettings::default(),
            SystemAppearance::Light,
        )?;
        let connection = ConnectionConfig::parse("https://notes.example/")?
            .id()
            .clone();
        let local = local_ui::bind(&ui, store.clone(), connection, appearance)?;
        let completed = Rc::new(RefCell::new(None));
        let outcome = completed.clone();
        let weak = ui.as_weak();
        slint::spawn_local(async move {
            let result: Result<()> = async {
                let ui = weak.upgrade().ok_or("owned command window closed")?;
                for _ in 0..4 {
                    pause().await?;
                }
                window_chrome::configure(ui.window())?;
                let window: Retained<objc2_app_kit::NSWindow> =
                    window_chrome::native_window(ui.window())?;
                window.makeKeyAndOrderFront(None);
                NSApplication::sharedApplication(MainThreadMarker::from(&*window)).activate();
                pause().await?;
                local.dispatch(AppAction::NewNote);
                local.flush().await?;
                let page = local.current_key().ok_or("generated command page absent")?;
                ui.set_note_text("word".into());
                ui.invoke_note_edited("word".into());
                local.wait_until_saved().await?;
                ui.invoke_focus_editor();
                ui.invoke_restore_editor_position(4, 0, 0.0);
                let view = window.contentView().ok_or("owned view absent")?;
                check(
                    window.makeFirstResponder(Some(&view)),
                    "owned native view did not become first responder",
                )?;

                capture(&ui, "editor-commands-before")?;
                key(&ui, "b", 11, Flags::Command).await?;
                check(
                    ui.get_note_text() == "**word**"
                        && (ui.get_editor_anchor(), ui.get_editor_caret()) == (6, 2),
                    "native bold source or directed selection differs",
                )?;
                capture(&ui, "editor-commands-after")?;
                key(&ui, "z", 6, Flags::Command).await?;
                check(
                    ui.get_note_text() == "word"
                        && (ui.get_editor_anchor(), ui.get_editor_caret()) == (4, 0),
                    "native undo did not restore directed selection once",
                )?;
                key(&ui, "z", 6, Flags::Command | Flags::Shift).await?;
                check(
                    ui.get_note_text() == "**word**"
                        && (ui.get_editor_anchor(), ui.get_editor_caret()) == (6, 2),
                    "native redo did not restore directed selection once",
                )?;
                local.wait_until_saved().await?;
                for (source, caret, text, code, flags, want) in [
                    ("word", 2, "i", 34, Flags::Command, "*word*"),
                    ("word", 2, "x", 7, Flags::Command | Flags::Shift, "~~word~~"),
                    ("word", 4, "h", 4, Flags::Command | Flags::Shift, "# word"),
                    (
                        "- [ ] one",
                        9,
                        "k",
                        40,
                        Flags::Command | Flags::Shift,
                        "- [x] one",
                    ),
                    (
                        "- [x] one",
                        9,
                        "\r",
                        36,
                        Flags::empty(),
                        "- [x] one\n- [ ] ",
                    ),
                    ("- a", 3, "\t", 48, Flags::empty(), "  - a"),
                    ("  - a", 5, "\t", 48, Flags::Shift, "- a"),
                    ("a\nb", 0, "\u{f701}", 125, Flags::Option, "b\na"),
                    ("a\nb", 2, "\u{f700}", 126, Flags::Option, "b\na"),
                ] {
                    ui.set_note_text(source.into());
                    ui.invoke_note_edited(source.into());
                    local.wait_until_saved().await?;
                    ui.invoke_focus_editor();
                    ui.invoke_restore_editor_position(caret, caret, 0.0);
                    key(&ui, text, code, flags).await?;
                    check(
                        ui.get_note_text() == want,
                        "native source command result differs",
                    )?;
                    local.wait_until_saved().await?;
                    check(
                        store
                            .load_page(&page)
                            .await?
                            .ok_or("saved command page absent")?
                            .body
                            == want,
                        "native command did not reach local persistence",
                    )?;
                }
                ui.set_note_read_only(true);
                key(&ui, "b", 11, Flags::Command).await?;
                check(
                    ui.get_note_text() == "b\na",
                    "read-only native command changed source",
                )?;
                store.shutdown().await?;
                let reopened = StoreHandle::open(&path).await?;
                check(
                    reopened
                        .load_page(&page)
                        .await?
                        .ok_or("restart command page absent")?
                        .body
                        == "b\na",
                    "restart lost command-derived source",
                )?;
                reopened.shutdown().await?;
                Ok(())
            }
            .await;
            *outcome.borrow_mut() = Some(result);
            let _ = slint::quit_event_loop();
        })?;
        ui.show()?;
        application.activate();
        slint::run_event_loop()?;
        ui.hide()?;
        completed
            .borrow_mut()
            .take()
            .ok_or("native command proof did not finish")??;
        println!("native_editor_commands_passed: true");
        Ok(())
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(all(target_os = "macos", feature = "native-validation"))]
    return native::run();
    #[cfg(not(all(target_os = "macos", feature = "native-validation")))]
    Err("requires macOS and --features native-validation".into())
}
