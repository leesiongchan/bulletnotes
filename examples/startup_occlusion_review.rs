//! Generated-text startup probe; covers only with owned windows and opens no store.
#[cfg(all(target_os = "macos", feature = "native-validation"))]
mod native {
    use bulletnotes::{
        AppWindow,
        platform::{application::NativeApplication, renderer, window_chrome},
    };
    use objc2::{MainThreadMarker, MainThreadOnly, rc::Retained};
    use objc2_app_kit::{
        NSBackingStoreType, NSColor, NSFloatingWindowLevel, NSScreen, NSWindow, NSWindowStyleMask,
    };
    use slint::ComponentHandle;
    use std::{
        cell::Cell,
        rc::Rc,
        time::{Duration, Instant},
    };

    type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
    const SOURCE: &str = "Generated startup fixture\r\n- [ ] Task café 日本語\r\n";

    struct Cover(Retained<NSWindow>);
    impl Drop for Cover {
        fn drop(&mut self) {
            self.0.orderOut(None);
        }
    }

    async fn pause() -> Result<()> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        slint::Timer::single_shot(Duration::from_millis(50), move || {
            let _ = tx.send(());
        });
        rx.await?;
        Ok(())
    }

    pub fn run() -> Result<()> {
        let application = NativeApplication::prepare()?;
        renderer::select()?;
        let start = Instant::now();
        let covered = std::env::args().any(|arg| arg == "--covered");
        let delayed_show = std::env::args().any(|arg| arg == "--delayed-show");
        let early_chrome = std::env::args().any(|arg| arg == "--chrome-before-frame");
        let mut covers = Vec::new();
        if covered {
            let mtm = MainThreadMarker::new().ok_or("main thread required")?;
            for screen in NSScreen::screens(mtm) {
                // SAFETY: public AppKit initializer on the main thread; disable
                // release-on-close so this Retained owns the window's lifetime.
                let cover = unsafe {
                    let cover = NSWindow::initWithContentRect_styleMask_backing_defer(
                        NSWindow::alloc(mtm),
                        screen.frame(),
                        NSWindowStyleMask::Borderless,
                        NSBackingStoreType::Buffered,
                        false,
                    );
                    cover.setReleasedWhenClosed(false);
                    cover
                };
                cover.setOpaque(true);
                cover.setBackgroundColor(Some(&NSColor::grayColor()));
                cover.setLevel(NSFloatingWindowLevel);
                cover.orderFrontRegardless();
                covers.push(Cover(cover));
            }
        }
        let window = AppWindow::new()?;
        window.set_note_text(SOURCE.into());
        let frames = Rc::new(Cell::new(0));
        let rendered = frames.clone();
        window.window().set_rendering_notifier(move |state, _| {
            if rendered.get() == 0 {
                println!(
                    "startup_render: elapsed_ms={} state={state:?}",
                    start.elapsed().as_millis()
                );
            }
            if matches!(state, slint::RenderingState::AfterRendering) {
                rendered.set(rendered.get() + 1);
            }
        })?;
        let passed = Rc::new(Cell::new(false));
        let result = passed.clone();
        let weak = window.as_weak();
        slint::spawn_local(async move {
            let check: Result<()> = async {
                let window = weak.upgrade().ok_or("fixture closed")?;
                let mut covers = covers;
                let mut configured = false;
                let mut next_sample = 0;
                let mut shown = !delayed_show;
                loop {
                    pause().await?;
                    let elapsed = start.elapsed().as_millis();
                    if !shown {
                        if elapsed < 350 { continue; }
                        window.show()?;
                        shown = true;
                    }
                    let native = window_chrome::native_window(window.window())?;
                    if early_chrome && !configured && elapsed >= 220 {
                        window_chrome::configure(window.window())?;
                        configured = true;
                    }
                    if elapsed >= next_sample {
                        println!("startup_sample: elapsed_ms={elapsed} frames={} visible={} key={} alpha={} occlusion={:?} frame={:?}",
                            frames.get(), native.isVisible(), native.isKeyWindow(), native.alphaValue(), native.occlusionState(), native.frame());
                        next_sample = elapsed + 250;
                    }
                    if covered && !covers.is_empty() && elapsed >= 1500 {
                        println!("startup_uncover: elapsed_ms={elapsed}");
                        covers.clear();
                    }
                    if frames.get() > 0 && native.alphaValue() == 1.0 && covers.is_empty()
                        && native.occlusionState().contains(objc2_app_kit::NSWindowOcclusionState::Visible) {
                        if !configured { window_chrome::configure(window.window())?; }
                        if window.get_note_text() != SOURCE { return Err("startup changed generated source".into()); }
                        if !renderer::uses_metal(window.window())? { return Err("startup has no Metal layer".into()); }
                        println!("startup_verified: elapsed_ms={elapsed} covered={covered} early_chrome={early_chrome}");
                        return Ok(());
                    }
                    if elapsed >= 8000 { return Err("startup did not deliver a visible frame".into()); }
                }
            }.await;
            match check {
                Ok(()) => result.set(true),
                Err(error) => eprintln!("startup: {error}"),
            }
            let _ = slint::quit_event_loop();
        })?;
        if !delayed_show {
            window.show()?;
        }
        application.activate();
        slint::run_event_loop()?;
        window.hide()?;
        if !passed.get() {
            return Err("startup probe failed".into());
        }
        Ok(())
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(all(target_os = "macos", feature = "native-validation"))]
    return native::run();
    #[cfg(not(all(target_os = "macos", feature = "native-validation")))]
    Err("requires macOS and --features native-validation".into())
}
