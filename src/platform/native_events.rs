//! Public AppKit notifications and KVO, retained and removed on the main thread.
use crate::AppWindow;
use block2::RcBlock;
use objc2::{
    DeclaredClass, MainThreadMarker, MainThreadOnly, define_class, msg_send,
    rc::Retained,
    runtime::{AnyObject, ProtocolObject},
    sel,
};
use objc2_app_kit::{
    NSApplication, NSApplicationDidBecomeActiveNotification,
    NSApplicationDidChangeScreenParametersNotification, NSWindow, NSWindowDidMoveNotification,
    NSWindowDidResizeNotification, NSWorkspace,
    NSWorkspaceAccessibilityDisplayOptionsDidChangeNotification,
    NSWorkspaceDidActivateApplicationNotification,
};
use objc2_foundation::{
    NSDistributedNotificationCenter, NSKeyValueObservingOptions, NSNotification,
    NSNotificationCenter, NSNotificationName, NSNotificationSuspensionBehavior, NSObject,
    NSObjectNSKeyValueObserverRegistration, NSObjectProtocol, NSOperationQueue, NSString,
    ns_string,
};
use slint::ComponentHandle;
use std::{ffi::c_void, ptr::NonNull};
static APPEARANCE_CONTEXT: u8 = 0;
struct ObserverIvars {
    window: slint::Weak<AppWindow>,
}
define_class!(
    // SAFETY: NSObject has no subclass invariants. The observer and its AppKit
    // source are main-thread-only; callbacks only queue work for Slint's UI loop.
    #[unsafe(super=NSObject)]
    #[thread_kind=objc2::MainThreadOnly]
    #[ivars=ObserverIvars]
    #[name="BulletnotesAppearanceObserver"]
    struct AppearanceObserver;
    unsafe impl NSObjectProtocol for AppearanceObserver{}
    impl AppearanceObserver{
        // SAFETY: documented NSNotification observer signature. No payload is read;
        // an untrusted local sender can only request the same action as menu Reopen.
        #[unsafe(method(reopen:))]
        fn reopen(&self, _notification: &NSNotification) {
            let _=self.ivars().window.upgrade_in_event_loop(|window|window.invoke_native_reopen_requested());
        }
        // SAFETY: matches NSObject's documented KVO selector and object encodings.
        #[unsafe(method(observeValueForKeyPath:ofObject:change:context:))]
        fn changed(&self,key:Option<&NSString>,object:Option<&AnyObject>,change:Option<&AnyObject>,context:*mut c_void){
            if context==std::ptr::addr_of!(APPEARANCE_CONTEXT).cast_mut().cast(){
                let _=self.ivars().window.upgrade_in_event_loop(|window|window.invoke_native_preferences_changed());
            }else{
                // SAFETY: forward observations owned by other code unchanged.
                unsafe{let _:()=msg_send![super(self),observeValueForKeyPath:key,ofObject:object,change:change,context:context];}
            }
        }
    }
);
#[derive(Clone, Copy)]
enum Event {
    Preferences,
    Activation,
    Geometry,
    Displays,
}
pub(super) fn reopen_name() -> &'static NSString {
    ns_string!("lol.harflix.bulletnotes.reopen")
}
type ObserverToken = (
    Retained<NSNotificationCenter>,
    Retained<ProtocolObject<dyn NSObjectProtocol>>,
);
pub struct NativeEvents {
    app: Retained<NSApplication>,
    observer: Retained<AppearanceObserver>,
    tokens: Vec<ObserverToken>,
    distributed: Retained<NSDistributedNotificationCenter>,
    _main: MainThreadMarker,
}
impl NativeEvents {
    pub fn new(window: &AppWindow, native: &NSWindow) -> Result<Self, &'static str> {
        let main = MainThreadMarker::new().ok_or("native observers require the main thread")?;
        let app = NSApplication::sharedApplication(main);
        let allocated = AppearanceObserver::alloc(main).set_ivars(ObserverIvars {
            window: window.as_weak(),
        });
        // SAFETY: NSObject init initializes the allocated subclass after its Rust ivars.
        let observer: Retained<AppearanceObserver> = unsafe { msg_send![super(allocated), init] };
        let observed: &NSObject = &app;
        // SAFETY: effectiveAppearance is AppKit's public KVO property. This guard
        // retains both source and observer, uses a unique static context address,
        // and removes that exact registration before either retained object drops.
        unsafe {
            observed.addObserver_forKeyPath_options_context(
                &observer,
                ns_string!("effectiveAppearance"),
                NSKeyValueObservingOptions::New,
                std::ptr::addr_of!(APPEARANCE_CONTEXT).cast_mut().cast(),
            );
        }
        let distributed = NSDistributedNotificationCenter::defaultCenter();
        let destination = NSString::from_str(&std::process::id().to_string());
        // SAFETY: owned retained observer implements reopen: with the documented
        // selector signature. Filter is this PID, no userInfo or note/store payload.
        // Remove the observer from this center before its retained storage drops.
        unsafe {
            distributed.addObserver_selector_name_object_suspensionBehavior(
                &observer,
                sel!(reopen:),
                Some(reopen_name()),
                Some(&destination),
                NSNotificationSuspensionBehavior::DeliverImmediately,
            );
        }
        let mut events = Self {
            app,
            observer,
            tokens: Vec::new(),
            distributed,
            _main: main,
        };
        let workspace = NSWorkspace::sharedWorkspace().notificationCenter();
        let default = NSNotificationCenter::defaultCenter();
        // SAFETY: these immutable notification-name constants are exported by the
        // initialized AppKit framework and are never mutated or manually released.
        unsafe {
            events.observe(
                window,
                &workspace,
                NSWorkspaceAccessibilityDisplayOptionsDidChangeNotification,
                None,
                Event::Preferences,
            );
            events.observe(
                window,
                &workspace,
                NSWorkspaceDidActivateApplicationNotification,
                None,
                Event::Activation,
            );
            events.observe(
                window,
                &default,
                NSApplicationDidBecomeActiveNotification,
                None,
                Event::Activation,
            );
            events.observe(
                window,
                &default,
                NSApplicationDidChangeScreenParametersNotification,
                None,
                Event::Displays,
            );
            events.observe(
                window,
                &default,
                NSWindowDidMoveNotification,
                Some(native),
                Event::Geometry,
            );
            events.observe(
                window,
                &default,
                NSWindowDidResizeNotification,
                Some(native),
                Event::Geometry,
            );
        }
        Ok(events)
    }
    fn observe(
        &mut self,
        window: &AppWindow,
        center: &Retained<NSNotificationCenter>,
        name: &NSNotificationName,
        object: Option<&AnyObject>,
        event: Event,
    ) {
        let weak = window.as_weak();
        let block = RcBlock::new(move |_notification: NonNull<NSNotification>| {
            let _ = weak.upgrade_in_event_loop(move |window| match event {
                Event::Preferences => window.invoke_native_preferences_changed(),
                Event::Activation => window.invoke_native_activation_changed(),
                Event::Geometry => window.invoke_native_geometry_changed(),
                Event::Displays => window.invoke_native_displays_changed(),
            });
        });
        // SAFETY: object is either this live NSWindow or absent. The block captures
        // only a Send+Sync Slint weak handle and a Copy enum, runs via mainQueue,
        // and queues UI access instead of reentering Slint. The returned token is
        // retained here and removed from its originating center during Drop.
        let token = unsafe {
            center.addObserverForName_object_queue_usingBlock(
                Some(name),
                object,
                Some(&NSOperationQueue::mainQueue()),
                &block,
            )
        };
        self.tokens.push((center.clone(), token));
    }
}
impl Drop for NativeEvents {
    fn drop(&mut self) {
        // SAFETY: this exact retained selector observer belongs to this center.
        unsafe {
            self.distributed.removeObserver(&self.observer);
        }
        let observed: &NSObject = &self.app;
        // SAFETY: this exact source/observer/key/context registration is owned by
        // this main-thread guard and remains retained until after removal.
        unsafe {
            observed.removeObserver_forKeyPath_context(
                &self.observer,
                ns_string!("effectiveAppearance"),
                std::ptr::addr_of!(APPEARANCE_CONTEXT).cast_mut().cast(),
            );
        }
        for (center, token) in self.tokens.drain(..) {
            // SAFETY: token is the retained block-observer returned by this center.
            unsafe {
                let object: &AnyObject = (*token).as_ref();
                center.removeObserver(object);
            }
        }
    }
}
