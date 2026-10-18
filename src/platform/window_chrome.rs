//! Public AppKit frame/hover adapter; retains Slint ownership of its content view.
use block2::RcBlock;
use objc2::{
    MainThreadMarker,
    rc::Retained,
    runtime::{AnyObject, ProtocolObject},
};
use objc2_app_kit::{
    NSAnimatablePropertyContainer, NSAnimationContext, NSApplicationDidHideNotification, NSEvent,
    NSEventMask, NSEventType, NSView, NSWindow, NSWindowButton, NSWindowDidBecomeKeyNotification,
    NSWindowDidResignKeyNotification, NSWindowStyleMask, NSWindowTitleVisibility,
};
use objc2_foundation::{NSNotification, NSNotificationCenter, NSObjectProtocol, NSOperationQueue};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::{cell::Cell, ptr::NonNull, rc::Rc};
type ProbeResult<T> = Result<T, Box<dyn std::error::Error>>;

pub trait ChromeWindow: slint::ComponentHandle {
    const FLOATING: bool;
    fn hover_edges(&self, header: bool, footer: bool);
    fn visible_edges(&self) -> (bool, bool);
    fn reset_chrome(&self);
    fn observe_header(&self, callback: Box<dyn Fn(bool, bool)>);
}
impl ChromeWindow for crate::AppWindow {
    const FLOATING: bool = true;
    fn hover_edges(&self, header: bool, footer: bool) {
        self.set_chrome_active(true);
        self.set_header_hovered(header);
        self.set_footer_hovered(footer);
    }
    fn visible_edges(&self) -> (bool, bool) {
        (
            self.get_header_opacity() > 0.0,
            self.get_footer_opacity() > 0.0,
        )
    }
    fn reset_chrome(&self) {
        self.invoke_reset_chrome();
    }
    fn observe_header(&self, callback: Box<dyn Fn(bool, bool)>) {
        self.on_header_visibility_changed(callback);
    }
}
impl ChromeWindow for crate::NativePreviewWindow {
    const FLOATING: bool = false;
    fn hover_edges(&self, header: bool, _footer: bool) {
        self.set_chrome_revealed(header);
    }
    fn visible_edges(&self) -> (bool, bool) {
        (self.get_chrome_revealed(), false)
    }
    fn reset_chrome(&self) {
        self.set_chrome_revealed(false);
    }
    fn observe_header(&self, _callback: Box<dyn Fn(bool, bool)>) {}
}

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
    for button in standard_buttons(window) {
        button.setEnabled(visible);
        button.setAlphaValue(if visible { 1.0 } else { 0.0 });
        button.setHidden(!visible);
    }
}
fn standard_buttons(
    window: &NSWindow,
) -> impl Iterator<Item = Retained<objc2_app_kit::NSButton>> + '_ {
    [
        NSWindowButton::CloseButton,
        NSWindowButton::MiniaturizeButton,
        NSWindowButton::ZoomButton,
    ]
    .into_iter()
    .filter_map(|kind| window.standardWindowButton(kind))
}
fn align_buttons(window: &NSWindow) {
    for button in standard_buttons(window) {
        // SAFETY: the live window and retained standard buttons are accessed on the main thread.
        if let Some(parent) = unsafe { button.superview() } {
            let frame = button.frame();
            let bounds = parent.convertRect_toView(frame, None);
            let delta =
                window.frame().size.height - 22.0 - bounds.origin.y - bounds.size.height / 2.0;
            button.setFrameOrigin(objc2_foundation::NSPoint::new(
                frame.origin.x,
                frame.origin.y + if parent.isFlipped() { -delta } else { delta },
            ));
        }
    }
}
fn animate_buttons(
    window: &Retained<NSWindow>,
    visible: bool,
    immediate: bool,
    generation: &Rc<Cell<u64>>,
) {
    let current = generation.get().wrapping_add(1);
    generation.set(current);
    if immediate {
        show_buttons(window, visible);
        if visible {
            align_buttons(window);
        }
        return;
    }
    let buttons: Vec<_> = standard_buttons(window).collect();
    for button in &buttons {
        button.setEnabled(visible);
        if visible {
            button.setHidden(false);
        }
    }
    if visible {
        align_buttons(window);
    }
    let targets = buttons.clone();
    let changes = RcBlock::new(move |context: NonNull<NSAnimationContext>| {
        // SAFETY: AppKit supplies the live animation context for this synchronous block.
        unsafe { context.as_ref() }.setDuration(if visible { 0.160 } else { 0.200 });
        for button in &targets {
            button
                .animator()
                .setAlphaValue(if visible { 1.0 } else { 0.0 });
        }
    });
    let generation = generation.clone();
    let native = window.clone();
    let completion = RcBlock::new(move || {
        if visible && generation.get() == current {
            align_buttons(&native);
        }
        if !visible && generation.get() == current {
            for button in &buttons {
                button.setHidden(true);
            }
        }
    });
    NSAnimationContext::runAnimationGroup_completionHandler(&changes, Some(&completion));
}

pub struct ChromeMonitor {
    token: Retained<AnyObject>,
    notifications: Vec<Retained<ProtocolObject<dyn NSObjectProtocol>>>,
    _generation: Rc<Cell<u64>>,
}
impl ChromeMonitor {
    pub fn install<W: ChromeWindow + 'static>(
        window: &W,
        observer: Option<Box<dyn Fn(bool, isize)>>,
    ) -> ProbeResult<Self> {
        configure(window.window())?;
        let native = native_window(window.window())?;
        if W::FLOATING {
            align_buttons(&native);
        }
        let generation = Rc::new(Cell::new(0));
        show_buttons(&native, window.visible_edges().0);
        let weak_generation = Rc::downgrade(&generation);
        let buttons_window = native.clone();
        window.observe_header(Box::new(move |visible, immediate| {
            if let Some(generation) = weak_generation.upgrade() {
                animate_buttons(&buttons_window, visible, immediate, &generation);
            }
        }));
        let weak = window.as_weak();
        let selecting = Cell::new(false);
        let block = {
            let native = native.clone();
            RcBlock::new(move |event: NonNull<NSEvent>| -> *mut NSEvent {
                if let Some(mtm) = MainThreadMarker::new() {
                    // SAFETY: AppKit supplies a live event. Returning its exact pointer
                    // leaves selection, scrolling, context menus and window gestures intact.
                    let event_ref = unsafe { event.as_ref() };
                    let inside = event_ref.window(mtm).is_some_and(|target| target == native);
                    let point = event_ref.locationInWindow();
                    let size = native.frame().size;
                    if let Some(window) = weak.upgrade() {
                        if W::FLOATING && (!native.isKeyWindow() || !native.isVisible()) {
                            selecting.set(false);
                            window.reset_chrome();
                            return event.as_ptr();
                        }
                        let (shown_header, shown_footer) = window.visible_edges();
                        let in_window = inside
                            && point.x >= 0.0
                            && point.x <= size.width
                            && point.y >= 0.0
                            && point.y <= size.height;
                        let header_depth = if W::FLOATING {
                            if shown_header { 52.0 } else { 16.0 }
                        } else {
                            36.0
                        };
                        let footer_depth = if shown_footer { 62.0 } else { 16.0 };
                        let header = in_window && point.y >= size.height - header_depth;
                        let footer = W::FLOATING && in_window && point.y <= footer_depth;
                        if event_ref.r#type() == NSEventType::LeftMouseDown {
                            selecting.set(
                                in_window && !(shown_header && header || shown_footer && footer),
                            );
                        }
                        if event_ref.r#type() == NSEventType::LeftMouseUp
                            || (event_ref.r#type() == NSEventType::MouseMoved
                                && NSEvent::pressedMouseButtons() & 1 == 0)
                        {
                            selecting.set(false);
                        }
                        if event_ref.r#type() == NSEventType::LeftMouseDragged
                            && NSEvent::pressedMouseButtons() & 1 != 0
                            && !shown_header
                            && !shown_footer
                        {
                            selecting.set(true);
                        }
                        let header = header && (!selecting.get() || shown_header);
                        let footer = footer && (!selecting.get() || shown_footer);
                        window.hover_edges(header, footer);
                        if !W::FLOATING {
                            show_buttons(&native, header);
                        }
                        if let Some(observer) = &observer {
                            observer(header, event_ref.eventNumber());
                        }
                    }
                }
                event.as_ptr()
            })
        };
        // SAFETY: local passive monitor returns the original event unchanged.
        let token = unsafe {
            NSEvent::addLocalMonitorForEventsMatchingMask_handler(
                NSEventMask::MouseMoved
                    | NSEventMask::MouseEntered
                    | NSEventMask::MouseExited
                    | NSEventMask::LeftMouseDown
                    | NSEventMask::LeftMouseUp
                    | NSEventMask::LeftMouseDragged,
                &block,
            )
        }
        .ok_or("native hover monitor unavailable")?;
        let mut notifications = Vec::new();
        let center = NSNotificationCenter::defaultCenter();
        // SAFETY: AppKit notification constants are immutable; blocks are copied by
        // Foundation and removed by this main-thread guard before the window drops.
        for (name, object) in unsafe {
            [
                (NSWindowDidResignKeyNotification, Some(&*native)),
                (NSWindowDidBecomeKeyNotification, Some(&*native)),
                (NSApplicationDidHideNotification, None),
            ]
        } {
            let weak = window.as_weak();
            let native = native.clone();
            let generation = generation.clone();
            let block = RcBlock::new(move |_notification: NonNull<NSNotification>| {
                let _ = weak.upgrade_in_event_loop(|window| window.reset_chrome());
                generation.set(generation.get().wrapping_add(1));
                show_buttons(&native, false);
            });
            notifications.push(unsafe {
                center.addObserverForName_object_queue_usingBlock(
                    Some(name),
                    object.map(|window| -> &AnyObject { window }),
                    Some(&NSOperationQueue::mainQueue()),
                    &block,
                )
            });
        }
        Ok(Self {
            token,
            notifications,
            _generation: generation,
        })
    }
}
impl Drop for ChromeMonitor {
    fn drop(&mut self) {
        // SAFETY: these tokens are owned by this main-thread guard and removed once.
        unsafe {
            NSEvent::removeMonitor(&self.token);
        }
        let center = NSNotificationCenter::defaultCenter();
        for token in &self.notifications {
            unsafe {
                let object: &AnyObject = (**token).as_ref();
                center.removeObserver(object);
            }
        }
    }
}
