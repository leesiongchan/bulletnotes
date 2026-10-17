//! A public NSApplication subclass routes termination through acknowledged local saves.
//! Winit retains its own delegate. No delegate replacement, swizzling, or private API.
use crate::theme::SystemAppearance;
use objc2::{
    ClassType, MainThreadMarker, define_class, msg_send, rc::Retained, runtime::AnyObject,
};
use objc2_app_kit::{NSAppearanceNameAqua, NSAppearanceNameDarkAqua, NSApplication};
use objc2_foundation::{
    NSArray, NSFileManager, NSObjectProtocol, NSSearchPathDirectory, NSSearchPathDomainMask,
};
use std::{
    cell::RefCell,
    path::PathBuf,
    rc::{Rc, Weak},
};

type QuitHandler = dyn Fn();
thread_local! { static QUIT: RefCell<Option<Weak<QuitHandler>>> = RefCell::new(None); }
define_class!(
    // SAFETY: NSApplication has no additional subclass invariants. This class and
    // the singleton are main-thread-only. Winit may override sendEvent: normally.
    #[unsafe(super = NSApplication)]
    #[thread_kind = objc2::MainThreadOnly]
    #[name = "BulletnotesApplication"]
    struct BulletnotesApplication;
    impl BulletnotesApplication {
        // SAFETY: matches NSApplication's documented terminate: action signature.
        #[unsafe(method(terminate:))]
        fn request_termination(&self, _sender: Option<&AnyObject>) {
            let handler = QUIT.with(|slot| slot.borrow().as_ref().and_then(Weak::upgrade));
            if let Some(handler) = handler { handler(); }
            // Before UI binding or after teardown, cancellation is safer than exiting
            // without a durability acknowledgement. Never invoke super's immediate exit.
        }
    }
);

pub struct NativeApplication {
    app: Retained<NSApplication>,
    _handler: Option<Rc<QuitHandler>>,
}
impl NativeApplication {
    /// Must be the first NSApplication creation, before creating any Slint components.
    pub fn prepare() -> Result<Self, &'static str> {
        let _mtm = MainThreadMarker::new().ok_or("application requires the main thread")?;
        // SAFETY: sharedApplication is the documented singleton method. objc2 retains
        // its result. Sending it to our registered subclass creates it if absent; the
        // result is checked before any editable window is allowed to open.
        let app: Retained<NSApplication> =
            unsafe { msg_send![BulletnotesApplication::class(), sharedApplication] };
        if !app.isKindOfClass(BulletnotesApplication::class()) {
            return Err("application was initialized before the safe quit handler");
        }
        Ok(Self {
            app,
            _handler: None,
        })
    }
    /// The handler schedules UI work instead of reentering Slint from an AppKit action.
    pub fn bind_quit(&mut self, window: &crate::AppWindow) {
        use slint::ComponentHandle;
        let weak = window.as_weak();
        let handler: Rc<QuitHandler> = Rc::new(move || {
            let _ = weak.upgrade_in_event_loop(|window| window.invoke_request_quit());
        });
        QUIT.with(|slot| slot.replace(Some(Rc::downgrade(&handler))));
        self._handler = Some(handler);
    }
    pub fn request_termination(&self) {
        self.app.terminate(None);
    }
    pub fn activate(&self) {
        self.app.activate();
    }
    pub fn system_appearance(&self) -> SystemAppearance {
        // SAFETY: these immutable NSString constants are exported by AppKit, which
        // is linked and initialized. They are not retained or mutated through a raw pointer.
        let (aqua, dark) = unsafe { (NSAppearanceNameAqua, NSAppearanceNameDarkAqua) };
        let candidates = NSArray::from_slice(&[aqua, dark]);
        let best = self
            .app
            .effectiveAppearance()
            .bestMatchFromAppearancesWithNames(&candidates);
        if best.as_deref() == Some(dark) {
            SystemAppearance::Dark
        } else {
            SystemAppearance::Light
        }
    }
}
pub fn database_path() -> Result<PathBuf, &'static str> {
    let directory = NSFileManager::defaultManager()
        .URLForDirectory_inDomain_appropriateForURL_create_error(
            NSSearchPathDirectory::ApplicationSupportDirectory,
            NSSearchPathDomainMask::UserDomainMask,
            None,
            true,
        )
        .map_err(|_| "Application Support directory is unavailable")?;
    let directory = directory
        .path()
        .ok_or("Application Support path is unavailable")?;
    let directory = PathBuf::from(directory.to_string()).join("lol.harflix.bulletnotes");
    use std::os::unix::fs::DirBuilderExt;
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(&directory)
        .map_err(|_| "Bulletnotes data directory is unavailable")?;
    Ok(directory.join("bulletnotes.sqlite"))
}
