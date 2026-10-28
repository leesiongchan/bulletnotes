//! Reserve a candidate before committing settings; failed changes retain the old key.
#[cfg(target_os = "macos")]
pub use super::exclusive_hotkey::NativeHotkeyBackend;
use global_hotkey::hotkey::HotKey;
use std::rc::Rc;
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum HotkeyError {
    #[error("shortcut is unavailable; choose another shortcut or keep the current one")]
    Unavailable,
    #[error("shortcut is invalid")]
    Invalid,
}
pub trait HotkeyBackend {
    fn register(&self, key: HotKey) -> Result<(), HotkeyError>;
    fn unregister(&self, key: HotKey) -> Result<(), HotkeyError>;
}
pub struct HotkeyRegistration<B: HotkeyBackend> {
    backend: Rc<B>,
    current: Option<HotKey>,
}
pub struct PendingHotkey<B: HotkeyBackend> {
    backend: Rc<B>,
    key: HotKey,
    expected: Option<HotKey>,
    reserved: bool,
}
impl<B: HotkeyBackend> HotkeyRegistration<B> {
    pub fn new(backend: Rc<B>) -> Self {
        Self {
            backend,
            current: None,
        }
    }
    #[cfg(target_os = "macos")]
    pub(crate) fn backend(&self) -> &B {
        &self.backend
    }
    pub fn current(&self) -> Option<HotKey> {
        self.current
    }
    pub fn prepare(&self, key: HotKey) -> Result<PendingHotkey<B>, HotkeyError> {
        let reserved = self.current != Some(key);
        if reserved {
            self.backend.register(key)?;
        }
        Ok(PendingHotkey {
            backend: self.backend.clone(),
            key,
            expected: self.current,
            reserved,
        })
    }
    pub fn commit(&mut self, mut pending: PendingHotkey<B>) -> Result<(), HotkeyError> {
        if !Rc::ptr_eq(&self.backend, &pending.backend) || self.current != pending.expected {
            return Err(HotkeyError::Invalid);
        }
        if self.current == Some(pending.key) {
            return Ok(());
        }
        if let Some(old) = self.current {
            self.backend.unregister(old)?;
        }
        self.current = Some(pending.key);
        pending.reserved = false;
        Ok(())
    }
}
impl<B: HotkeyBackend> Drop for HotkeyRegistration<B> {
    fn drop(&mut self) {
        if let Some(key) = self.current {
            let _ = self.backend.unregister(key);
        }
    }
}
impl<B: HotkeyBackend> Drop for PendingHotkey<B> {
    fn drop(&mut self) {
        if self.reserved {
            let _ = self.backend.unregister(self.key);
        }
    }
}
