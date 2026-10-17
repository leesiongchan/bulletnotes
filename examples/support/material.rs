//! Throwaway public-AppKit composition experiment, excluded from normal builds.
use objc2::{MainThreadMarker, MainThreadOnly, rc::Retained};
use objc2_app_kit::{
    NSAutoresizingMaskOptions, NSColor, NSView, NSVisualEffectBlendingMode, NSVisualEffectMaterial,
    NSVisualEffectState, NSVisualEffectView, NSWindow, NSWindowOrderingMode,
};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use slint::ComponentHandle;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Duration,
};

struct MaterialGuard {
    window: Retained<NSWindow>,
    original: Retained<NSView>,
    background: Retained<NSColor>,
    opaque: bool,
    effect: Retained<NSVisualEffectView>,
}

impl MaterialGuard {
    fn attach(window: &slint::Window) -> Result<Self, Box<dyn std::error::Error>> {
        let mtm = MainThreadMarker::new().ok_or("material adapter requires main thread")?;
        let provider = window.window_handle();
        let handle = provider.window_handle()?;
        let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
            return Err("AppKit window required".into());
        };
        // SAFETY: the public AppKit raw handle names an NSView owned by Slint/winit.
        // The handle and live Slint window outlive this borrow. Main-thread access
        // is checked above; contentView() retains the view before any reparenting.
        let view = unsafe { handle.ns_view.cast::<NSView>().as_ref() };
        let native = view.window().ok_or("native window missing")?;
        let original = native.contentView().ok_or("native content view missing")?;
        let effect =
            NSVisualEffectView::initWithFrame(NSVisualEffectView::alloc(mtm), original.bounds());
        effect.setMaterial(NSVisualEffectMaterial::UnderWindowBackground);
        effect.setBlendingMode(NSVisualEffectBlendingMode::BehindWindow);
        effect.setState(NSVisualEffectState::FollowsWindowActiveState);
        effect.setAutoresizingMask(
            NSAutoresizingMaskOptions::ViewWidthSizable
                | NSAutoresizingMaskOptions::ViewHeightSizable,
        );
        let guard = Self {
            background: native.backgroundColor(),
            opaque: native.isOpaque(),
            window: native,
            original,
            effect,
        };
        guard.window.setOpaque(false);
        guard
            .window
            .setBackgroundColor(Some(&NSColor::clearColor()));
        // Keep winit's contentView identity intact: its delegates assume that exact
        // view class. Add the native material below existing subviews instead.
        guard.original.addSubview_positioned_relativeTo(
            &guard.effect,
            NSWindowOrderingMode::Below,
            None,
        );
        Ok(guard)
    }

    fn report(&self, window: &slint::Window) -> Result<(), Box<dyn std::error::Error>> {
        let snapshot = window.take_snapshot()?;
        let mut translucent_paper_pixels = 0;
        let mut opaque_ink_pixels = 0;
        for pixel in snapshot.as_slice() {
            if (180..230).contains(&pixel.a) {
                translucent_paper_pixels += 1;
            }
            if pixel.a == 255 && pixel.r == 53 && pixel.g == 59 && pixel.b == 50 {
                opaque_ink_pixels += 1;
            }
        }
        let installed = self.effect.isDescendantOf(&self.original);
        println!("material_view_installed: {installed}");
        println!("window_alpha: {:?}", self.window.alphaValue());
        println!("translucent_paper_pixels: {translucent_paper_pixels}");
        println!("opaque_ink_pixels: {opaque_ink_pixels}");
        if !installed
            || self.window.alphaValue() != 1.0
            || translucent_paper_pixels < 100
            || opaque_ink_pixels < 100
        {
            return Err("material composition postconditions failed".into());
        }
        Ok(())
    }
}

impl Drop for MaterialGuard {
    fn drop(&mut self) {
        // All retained objects are main-thread-only, as is the guard's owner.
        self.effect.removeFromSuperview();
        self.window.setBackgroundColor(Some(&self.background));
        self.window.setOpaque(self.opaque);
    }
}

pub fn run(auto_close: bool) -> Result<(), Box<dyn std::error::Error>> {
    let window = bulletnotes::NativeMaterialWindow::new()?;
    let guard = Rc::new(RefCell::new(None::<MaterialGuard>));
    let failed = Rc::new(Cell::new(false));
    let weak = window.as_weak();
    let state = guard.clone();
    let error_state = failed.clone();
    slint::Timer::single_shot(Duration::from_millis(300), move || {
        if let Some(window) = weak.upgrade() {
            match MaterialGuard::attach(window.window()) {
                Ok(material) => {
                    *state.borrow_mut() = Some(material);
                    window.window().request_redraw();
                    let weak = window.as_weak();
                    let state = state.clone();
                    let error_state = error_state.clone();
                    slint::Timer::single_shot(Duration::from_millis(300), move || {
                        if let (Some(window), Some(material)) =
                            (weak.upgrade(), state.borrow().as_ref())
                        {
                            let readback = if std::env::args().any(|arg| arg == "--no-readback") {
                                Ok(())
                            } else {
                                material.report(window.window())
                            };
                            let result = readback.and_then(|()| {
                                println!(
                                    "{:#?}",
                                    bulletnotes::platform::macos::probe_window(window.window())?
                                );
                                Ok(())
                            });
                            if let Err(error) = result {
                                error_state.set(true);
                                eprintln!("material probe: {error}");
                                let _ = slint::quit_event_loop();
                            }
                        }
                    });
                }
                Err(error) => {
                    error_state.set(true);
                    eprintln!("material probe: {error}");
                    let _ = slint::quit_event_loop();
                }
            }
        }
    });
    if auto_close {
        slint::Timer::single_shot(Duration::from_secs(15), || {
            let _ = slint::quit_event_loop();
        });
    }
    window.run()?;
    // Restore the original content hierarchy while the Slint component is alive.
    guard.borrow_mut().take();
    if failed.get() {
        return Err("material native probe failed".into());
    }
    Ok(())
}
