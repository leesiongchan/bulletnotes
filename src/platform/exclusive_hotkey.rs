//! Exclusive global shortcuts through Apple's public Carbon event API.
//!
//! ABI/constants follow macOS SDK CarbonEvents{,Core}.h and Events.h.
//! `global-hotkey` 0.9.0 registers non-exclusive shortcuts on macOS; its public
//! parser/event types remain useful, but cannot enforce our conflict contract.
use super::hotkey::{HotkeyBackend, HotkeyError};
use global_hotkey::{
    GlobalHotKeyEvent, HotKeyState,
    hotkey::{Code, HotKey, Modifiers},
};
use objc2::MainThreadMarker;
use std::{
    cell::RefCell,
    collections::BTreeMap,
    ffi::{c_ulong, c_void},
    ptr,
};
type NativeRef = *mut c_void;
type EventCallback = unsafe extern "C" fn(NativeRef, NativeRef, NativeRef) -> i32;
#[repr(C)]
#[derive(Default)]
struct KeyId {
    signature: u32,
    id: u32,
}
#[repr(C)]
struct EventType {
    class: u32,
    kind: u32,
}
const SIGNATURE: u32 = u32::from_be_bytes(*b"BNot");
const NOT_HANDLED: i32 = -9874;
#[link(name = "Carbon", kind = "framework")]
unsafe extern "C" {
    fn GetApplicationEventTarget() -> NativeRef;
    fn InstallEventHandler(
        target: NativeRef,
        callback: Option<EventCallback>,
        count: c_ulong,
        events: *const EventType,
        data: NativeRef,
        handler: *mut NativeRef,
    ) -> i32;
    fn RemoveEventHandler(handler: NativeRef) -> i32;
    fn RegisterEventHotKey(
        code: u32,
        modifiers: u32,
        id: KeyId,
        target: NativeRef,
        options: u32,
        reference: *mut NativeRef,
    ) -> i32;
    fn UnregisterEventHotKey(reference: NativeRef) -> i32;
    fn GetEventKind(event: NativeRef) -> u32;
    fn GetEventParameter(
        event: NativeRef,
        name: u32,
        desired: u32,
        actual: *mut u32,
        size: c_ulong,
        actual_size: *mut c_ulong,
        result: NativeRef,
    ) -> i32;
}
type Callback = Box<dyn Fn(GlobalHotKeyEvent)>;
#[derive(Default)]
struct Context {
    callback: RefCell<Option<Callback>>,
}
/// Main-thread-only owner of Carbon's handler, callback context, and registrations.
/// The marker prevents moving this object or its callback state to another thread.
pub struct NativeHotkeyBackend {
    handler: NativeRef,
    context: Option<Box<Context>>,
    keys: RefCell<BTreeMap<u32, NativeRef>>,
    _main: MainThreadMarker,
}
impl NativeHotkeyBackend {
    pub fn new() -> Result<Self, HotkeyError> {
        let main = MainThreadMarker::new().ok_or(HotkeyError::Unavailable)?;
        let mut context = Box::<Context>::default();
        let events = [
            EventType {
                class: u32::from_be_bytes(*b"keyb"),
                kind: 5,
            },
            EventType {
                class: u32::from_be_bytes(*b"keyb"),
                kind: 6,
            },
        ];
        let mut handler = ptr::null_mut();
        // SAFETY: main thread; repr(C) descriptors; boxed context stays at a stable
        // address until RemoveEventHandler succeeds. Carbon copies event descriptors.
        let status = unsafe {
            InstallEventHandler(
                GetApplicationEventTarget(),
                Some(on_event),
                events.len() as c_ulong,
                events.as_ptr(),
                (&mut *context as *mut Context).cast(),
                &mut handler,
            )
        };
        if status != 0 || handler.is_null() {
            return Err(HotkeyError::Unavailable);
        }
        Ok(Self {
            handler,
            context: Some(context),
            keys: RefCell::new(BTreeMap::new()),
            _main: main,
        })
    }
    pub fn set_event_handler(&self, handler: impl Fn(GlobalHotKeyEvent) + 'static) {
        if let Some(context) = &self.context {
            *context.callback.borrow_mut() = Some(Box::new(handler));
        }
    }
}
impl HotkeyBackend for NativeHotkeyBackend {
    fn register(&self, key: HotKey) -> Result<(), HotkeyError> {
        let code = key_code(key.key).ok_or(HotkeyError::Invalid)?;
        if key
            .mods
            .difference(Modifiers::SHIFT | Modifiers::ALT | Modifiers::CONTROL | Modifiers::META)
            .bits()
            != 0
        {
            return Err(HotkeyError::Invalid);
        }
        let mut keys = self.keys.borrow_mut();
        if keys.contains_key(&key.id()) {
            return Err(HotkeyError::Unavailable);
        }
        let mut modifiers = 0;
        for (flag, mask) in [
            (Modifiers::META, 1 << 8),
            (Modifiers::SHIFT, 1 << 9),
            (Modifiers::ALT, 1 << 11),
            (Modifiers::CONTROL, 1 << 12),
        ] {
            if key.mods.contains(flag) {
                modifiers |= mask;
            }
        }
        let mut reference = ptr::null_mut();
        // SAFETY: main-thread owner; initialized out pointer; validated physical key
        // and modifiers. kEventHotKeyExclusive=1 atomically rejects existing owners.
        let status = unsafe {
            RegisterEventHotKey(
                code,
                modifiers,
                KeyId {
                    signature: SIGNATURE,
                    id: key.id(),
                },
                GetApplicationEventTarget(),
                1,
                &mut reference,
            )
        };
        if status != 0 || reference.is_null() {
            return Err(HotkeyError::Unavailable);
        }
        keys.insert(key.id(), reference);
        Ok(())
    }
    fn unregister(&self, key: HotKey) -> Result<(), HotkeyError> {
        let mut keys = self.keys.borrow_mut();
        if let Some(reference) = keys.get(&key.id()) {
            // SAFETY: live registration owned by this main-thread adapter. Keep its
            // reference on failure so teardown can still release it.
            if unsafe { UnregisterEventHotKey(*reference) } != 0 {
                return Err(HotkeyError::Unavailable);
            }
            keys.remove(&key.id());
        }
        Ok(())
    }
}
impl Drop for NativeHotkeyBackend {
    fn drop(&mut self) {
        for reference in self.keys.get_mut().values() {
            // SAFETY: every reference was returned by our successful registration.
            unsafe { UnregisterEventHotKey(*reference) };
        }
        // SAFETY: own handler, called on the main thread before releasing its context.
        if unsafe { RemoveEventHandler(self.handler) } != 0 {
            // Carbon might still reference user data. Disable delivery and retain its
            // allocation rather than leave a dangling callback pointer on OS failure.
            if let Some(context) = self.context.take() {
                context.callback.borrow_mut().take();
                let _ = Box::leak(context);
            }
        }
    }
}
unsafe extern "C" fn on_event(_: NativeRef, event: NativeRef, data: NativeRef) -> i32 {
    // No unwind may cross the C ABI. Foreign or malformed events remain unhandled.
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if event.is_null() || data.is_null() {
            return NOT_HANDLED;
        }
        let mut key = KeyId::default();
        let mut size = 0;
        // SAFETY: Carbon supplies a live event; the repr(C) result buffer is exactly
        // EventHotKeyID and writable for the supplied length. Optional outputs null.
        let status = unsafe {
            GetEventParameter(
                event,
                u32::from_be_bytes(*b"----"),
                u32::from_be_bytes(*b"hkid"),
                ptr::null_mut(),
                size_of::<KeyId>() as c_ulong,
                &mut size,
                (&mut key as *mut KeyId).cast(),
            )
        };
        if status != 0 || size != size_of::<KeyId>() as c_ulong || key.signature != SIGNATURE {
            return NOT_HANDLED;
        }
        // SAFETY: event remains live throughout the callback.
        let state = match unsafe { GetEventKind(event) } {
            5 => HotKeyState::Pressed,
            6 => HotKeyState::Released,
            _ => return NOT_HANDLED,
        };
        // SAFETY: data is our boxed Context, retained until handler removal succeeds.
        // Carbon dispatch and all mutations occur on the main thread.
        let context = unsafe { &*data.cast::<Context>() };
        if let Ok(callback) = context.callback.try_borrow()
            && let Some(callback) = callback.as_ref()
        {
            callback(GlobalHotKeyEvent { id: key.id, state });
        }
        0
    }))
    .unwrap_or(NOT_HANDLED)
}
// Physical positions defined by public HIToolbox Events.h kVK_ANSI_*, kVK_F*, kVK_Space.
fn key_code(code: Code) -> Option<u32> {
    Some(match code {
        Code::KeyA => 0x00,
        Code::KeyS => 0x01,
        Code::KeyD => 0x02,
        Code::KeyF => 0x03,
        Code::KeyH => 0x04,
        Code::KeyG => 0x05,
        Code::KeyZ => 0x06,
        Code::KeyX => 0x07,
        Code::KeyC => 0x08,
        Code::KeyV => 0x09,
        Code::KeyB => 0x0b,
        Code::KeyQ => 0x0c,
        Code::KeyW => 0x0d,
        Code::KeyE => 0x0e,
        Code::KeyR => 0x0f,
        Code::KeyY => 0x10,
        Code::KeyT => 0x11,
        Code::KeyO => 0x1f,
        Code::KeyU => 0x20,
        Code::KeyI => 0x22,
        Code::KeyP => 0x23,
        Code::KeyL => 0x25,
        Code::KeyJ => 0x26,
        Code::KeyK => 0x28,
        Code::KeyN => 0x2d,
        Code::KeyM => 0x2e,
        Code::Digit1 => 0x12,
        Code::Digit2 => 0x13,
        Code::Digit3 => 0x14,
        Code::Digit4 => 0x15,
        Code::Digit6 => 0x16,
        Code::Digit5 => 0x17,
        Code::Digit9 => 0x19,
        Code::Digit7 => 0x1a,
        Code::Digit8 => 0x1c,
        Code::Digit0 => 0x1d,
        Code::Space => 0x31,
        Code::F1 => 0x7a,
        Code::F2 => 0x78,
        Code::F3 => 0x63,
        Code::F4 => 0x76,
        Code::F5 => 0x60,
        Code::F6 => 0x61,
        Code::F7 => 0x62,
        Code::F8 => 0x64,
        Code::F9 => 0x65,
        Code::F10 => 0x6d,
        Code::F11 => 0x67,
        Code::F12 => 0x6f,
        Code::F13 => 0x69,
        Code::F14 => 0x6b,
        Code::F15 => 0x71,
        Code::F16 => 0x6a,
        Code::F17 => 0x40,
        Code::F18 => 0x4f,
        Code::F19 => 0x50,
        Code::F20 => 0x5a,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn callback_counts_match_the_public_mactypes_sdk_abi() {
        // MacTypes.h: ByteCount and ItemCount are unsigned long, including LP64.
        let _: unsafe extern "C" fn(
            NativeRef,
            u32,
            u32,
            *mut u32,
            std::ffi::c_ulong,
            *mut std::ffi::c_ulong,
            NativeRef,
        ) -> i32 = GetEventParameter;
        let _: unsafe extern "C" fn(
            NativeRef,
            Option<EventCallback>,
            std::ffi::c_ulong,
            *const EventType,
            NativeRef,
            *mut NativeRef,
        ) -> i32 = InstallEventHandler;
    }
}
