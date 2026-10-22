//! Owned-window composition lifecycle proof; generated text and PID-targeted events only.
#[cfg(all(target_os = "macos", feature = "native-validation"))]
mod native {
    use bulletnotes::platform::{application::NativeApplication, renderer, window_chrome};
    use objc2::{MainThreadMarker, msg_send, rc::Retained, runtime::AnyObject};
    use objc2_app_kit::NSApplication;
    use objc2_foundation::{NSRange, NSString};
    use slint::ComponentHandle;
    use std::{cell::RefCell, rc::Rc, time::Duration};
    type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
    slint::slint! {
        export component CompositionPeer inherits Window {
            width: 200px; height: 100px;
            forward-focus: focus;
            focus := FocusScope {}
        }
        export component CompositionProbe inherits Window {
            width: 505px; height: 552px; background: #fcfaf4;
            forward-focus: input;
            in-out property <string> source: "Generated composition proof";
            out property <bool> allowed: input.source-edits-allowed;
            out property <string> preedit: input.preedit-text;
            out property <int> edits: 0;
            input := TextInput {
                x: 45px; y: 34px; width: parent.width - 90px;
                height: parent.height - 68px; font-family: "Menlo"; font-size: 14px;
                single-line: false; text <=> root.source;
                edited => { root.edits += 1; }
            }
        }
    }
    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGEventCreateKeyboardEvent(
            source: *const std::ffi::c_void,
            code: u16,
            down: bool,
        ) -> *const std::ffi::c_void;
        fn CGEventPostToPid(pid: i32, event: *const std::ffi::c_void);
        fn CGPreflightPostEventAccess() -> bool;
    }
    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFRelease(object: *const std::ffi::c_void);
    }
    async fn pause() -> Result<()> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        slint::Timer::single_shot(Duration::from_millis(150), move || {
            let _ = tx.send(());
        });
        rx.await?;
        Ok(())
    }
    async fn key(ui: &CompositionProbe, code: u16) -> Result<()> {
        let native = window_chrome::native_window(ui.window())?;
        let app = NSApplication::sharedApplication(MainThreadMarker::from(&*native));
        if !app.isActive() || !native.isKeyWindow() {
            return Err(format!(
                "owned input window is not active: application={} key={}",
                app.isActive(),
                native.isKeyWindow()
            )
            .into());
        }
        for down in [true, false] {
            // SAFETY: documented Quartz API, posting solely to this fixture's PID.
            unsafe {
                let event = CGEventCreateKeyboardEvent(std::ptr::null(), code, down);
                if event.is_null() {
                    return Err("could not create native key event".into());
                }
                CGEventPostToPid(std::process::id() as i32, event);
                CFRelease(event);
            }
            pause().await?;
        }
        Ok(())
    }
    fn check(condition: bool, message: &'static str) -> Result<()> {
        if condition {
            Ok(())
        } else {
            Err(message.into())
        }
    }
    pub fn run() -> Result<()> {
        if !std::env::args().any(|arg| arg == "--pinyin") {
            return Err("actual --pinyin proof is required".into());
        }
        // SAFETY: read-only preflight; never requests permissions.
        check(
            unsafe { CGPreflightPostEventAccess() },
            "native event-post access unavailable",
        )?;
        use tracing_subscriber::{Layer, layer::SubscriberExt, util::SubscriberInitExt};
        tracing_subscriber::registry()
            .with(
                tracing_subscriber::fmt::layer()
                    .with_ansi(false)
                    .with_filter(tracing_subscriber::filter::filter_fn(|metadata| {
                        metadata.target() == "bulletnotes::composition"
                    })),
            )
            .try_init()?;
        let application = NativeApplication::prepare()?;
        renderer::select()?;
        let ui = CompositionProbe::new()?;
        let outcome = Rc::new(RefCell::new(None));
        let completed = outcome.clone();
        let weak = ui.as_weak();
        slint::spawn_local(async move {
            let result: Result<()> = async {
                let ui = weak.upgrade().ok_or("owned window closed")?;
                for _ in 0..4 { pause().await?; }
                let native = window_chrome::native_window(ui.window())?;
                native.makeKeyAndOrderFront(None);
                let view = native.contentView().ok_or("missing native input view")?;
                check(native.makeFirstResponder(Some(&view)), "input view did not become first responder")?;
                pause().await?;
                check(ui.get_allowed(), "new native context did not establish Idle")?;
                // SAFETY: public NSTextInputClient/context selectors on our main-thread view.
                let context: Retained<AnyObject> = unsafe { msg_send![&*view, inputContext] };
                let previous: Option<Retained<NSString>> = unsafe { msg_send![&*context, selectedKeyboardInputSource] };
                let method = NSString::from_str("com.apple.inputmethod.SCIM.ITABC");
                unsafe { let _: () = msg_send![&*context, setSelectedKeyboardInputSource: &*method]; }
                let proof: Result<()> = async {
                    pause().await?;
                    let selected: Option<Retained<NSString>> = unsafe { msg_send![&*context, selectedKeyboardInputSource] };
                    check(selected.as_deref() == Some(&*method), "Pinyin did not activate")?;
                    native.makeKeyAndOrderFront(None);
                    NSApplication::sharedApplication(MainThreadMarker::from(&*native)).activate();
                    for _ in 0..3 { pause().await?; }
                    let original = ui.get_source();
                    for code in [45, 34, 4, 0, 31] { key(&ui, code).await?; }
                    check(!ui.get_preedit().is_empty() && !ui.get_allowed(), "native candidate session was not guarded")?;
                    check(ui.get_source() == original && ui.get_edits() == 0, "preedit changed committed source")?;
                    println!("native_composition_phase: candidate active=true guarded=true window={}", native.windowNumber());
                    // Empty marked updates are ambiguous; they must remain guarded until explicit end.
                    let empty = NSString::from_str("");
                    unsafe { let _: () = msg_send![&*view, setMarkedText: &*empty, selectedRange: NSRange::new(0, 0), replacementRange: NSRange::new(usize::MAX, 0)]; }
                    pause().await?;
                    check(!ui.get_allowed(), "empty native update admitted commands")?;
                    // Recreate an explicit context, then exercise the real conversion commit.
                    let peer = CompositionPeer::new()?;
                    peer.show()?;
                    pause().await?;
                    let peer_native = window_chrome::native_window(peer.window())?;
                    peer_native.makeKeyAndOrderFront(None);
                    for _ in 0..3 { pause().await?; }
                    check(peer_native.isKeyWindow() && !native.isKeyWindow(), "owned peer did not acquire focus")?;
                    check(!ui.get_allowed(), "focus loss did not guard commands")?;
                    native.makeKeyWindow();
                    NSApplication::sharedApplication(MainThreadMarker::from(&*native)).activate();
                    for _ in 0..3 { pause().await?; }
                    check(native.isKeyWindow(), "owned editor did not regain focus")?;
                    peer.hide()?;
                    check(ui.get_allowed(), "context recreation did not establish Idle")?;
                    for code in [45, 34, 4, 0, 31] { key(&ui, code).await?; }
                    check(!ui.get_allowed(), "composition after context recreation was not guarded")?;
                    key(&ui, 49).await?;
                    check(ui.get_source().contains("你好") && ui.get_edits() == 1, "native Pinyin commit failed")?;
                    check(ui.get_allowed(), "actual native commit did not establish Idle")?;
                    for code in [45, 34] { key(&ui, code).await?; }
                    check(!ui.get_allowed(), "second candidate was not guarded")?;
                    let committed = ui.get_source();
                    key(&ui, 53).await?;
                    check(ui.get_source() == committed && ui.get_preedit().is_empty(), "Escape did not cancel without editing")?;
                    check(ui.get_allowed(), "Escape cancellation did not recover Idle")?;
                    println!("native_composition_phase: commit_and_cancel recovered=true edits={}", ui.get_edits());
                    Ok(())
                }.await;
                unsafe { let _: () = msg_send![&*context, setSelectedKeyboardInputSource: previous.as_deref()]; }
                pause().await?;
                let restored: Option<Retained<NSString>> = unsafe { msg_send![&*context, selectedKeyboardInputSource] };
                check(restored == previous, "input source was not restored")?;
                proof
            }.await;
            *completed.borrow_mut() = Some(result);
            let _ = slint::quit_event_loop();
        })?;
        ui.show()?;
        application.activate();
        slint::run_event_loop()?;
        ui.hide()?;
        outcome
            .borrow_mut()
            .take()
            .ok_or("native proof did not finish")??;
        println!("native_composition_guard_passed: true");
        Ok(())
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(all(target_os = "macos", feature = "native-validation"))]
    return native::run();
    #[cfg(not(all(target_os = "macos", feature = "native-validation")))]
    Err("requires macOS and --features native-validation".into())
}
