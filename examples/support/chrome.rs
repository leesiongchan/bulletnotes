//! Disposable public-AppKit frame/hover experiment; retains Slint's content view.
use block2::RcBlock;
pub trait ChromeWindow: slint::ComponentHandle {
    fn reveal_chrome(&self, revealed: bool);
}
impl ChromeWindow for bulletnotes::AppWindow {
    fn reveal_chrome(&self, revealed: bool) {
        self.set_chrome_revealed(revealed);
    }
}
impl ChromeWindow for bulletnotes::NativePreviewWindow {
    fn reveal_chrome(&self, revealed: bool) {
        self.set_chrome_revealed(revealed);
    }
}
use objc2::{MainThreadMarker, rc::Retained, runtime::AnyObject};
use objc2_app_kit::{
    NSEvent, NSEventMask, NSView, NSWindow, NSWindowButton, NSWindowStyleMask,
    NSWindowTitleVisibility,
};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::ptr::NonNull;
type ProbeResult<T> = Result<T, Box<dyn std::error::Error>>;

pub fn native_window(window: &slint::Window) -> ProbeResult<Retained<NSWindow>> {
    MainThreadMarker::new().ok_or("native chrome requires main thread")?;
    let provider = window.window_handle();
    let handle = provider.window_handle()?;
    let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
        return Err("AppKit window required".into());
    };
    // SAFETY: a supported AppKit handle points to NSView; provider and live Slint window
    // outlive this main-thread borrow. window() retains the resulting native object.
    let view = unsafe { handle.ns_view.cast::<NSView>().as_ref() };
    view.window()
        .ok_or_else(|| "native window unavailable".into())
}
pub fn configure(window: &slint::Window) -> ProbeResult<()> {
    let native = native_window(window)?;
    native.setStyleMask(
        native.styleMask()
            | NSWindowStyleMask::Titled
            | NSWindowStyleMask::Resizable
            | NSWindowStyleMask::Closable
            | NSWindowStyleMask::Miniaturizable
            | NSWindowStyleMask::FullSizeContentView,
    );
    native.setTitleVisibility(NSWindowTitleVisibility::Hidden);
    native.setTitlebarAppearsTransparent(true);
    native.setAcceptsMouseMovedEvents(true);
    show_buttons(&native, false);
    Ok(())
}
fn show_buttons(window: &NSWindow, visible: bool) {
    for button in [
        NSWindowButton::CloseButton,
        NSWindowButton::MiniaturizeButton,
        NSWindowButton::ZoomButton,
    ] {
        if let Some(button) = window.standardWindowButton(button) {
            button.setHidden(!visible);
        }
    }
}

pub struct ChromeMonitor {
    token: Retained<AnyObject>,
}
impl ChromeMonitor {
    pub fn install<W: ChromeWindow + 'static>(
        window: &W,
        observer: Option<Box<dyn Fn(bool, isize)>>,
    ) -> ProbeResult<Self> {
        configure(window.window())?;
        let native = native_window(window.window())?;
        let weak = window.as_weak();
        let block = RcBlock::new(move |event: NonNull<NSEvent>| -> *mut NSEvent {
            if let Some(mtm) = MainThreadMarker::new() {
                // SAFETY: AppKit supplies a live NSEvent for this callback; never retained
                // or mutated. Returning the exact input pointer preserves event dispatch.
                let event_ref = unsafe { event.as_ref() };
                let inside = event_ref.window(mtm).is_some_and(|target| target == native);
                let point = event_ref.locationInWindow();
                let frame = native.frame();
                let hover = inside
                    && point.x >= 0.0
                    && point.x <= frame.size.width
                    && point.y >= frame.size.height - 36.0
                    && point.y <= frame.size.height;
                if let Some(window) = weak.upgrade() {
                    window.reveal_chrome(hover);
                    show_buttons(&native, hover);
                    if let Some(observer) = &observer {
                        observer(hover, event_ref.eventNumber());
                    }
                }
            }
            event.as_ptr()
        });
        // SAFETY: callback returns its valid original event unchanged; this is a local
        // application monitor only. It requires no global input/accessibility permission.
        let token = unsafe {
            NSEvent::addLocalMonitorForEventsMatchingMask_handler(
                NSEventMask::MouseMoved
                    | NSEventMask::MouseEntered
                    | NSEventMask::MouseExited
                    | NSEventMask::LeftMouseDragged,
                &block,
            )
        }
        .ok_or("native hover monitor unavailable")?;
        Ok(Self { token })
    }
}
impl Drop for ChromeMonitor {
    fn drop(&mut self) {
        // SAFETY: token came from addLocalMonitor and is removed exactly once while
        // the preview is still on its main thread.
        unsafe {
            NSEvent::removeMonitor(&self.token);
        }
    }
}
