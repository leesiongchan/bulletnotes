//! Supported native integration boundaries.
#[cfg(target_os = "macos")]
pub mod application;
#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(target_os = "macos")]
pub mod window_chrome;

pub mod credentials;

#[cfg(target_os = "macos")]
pub mod hotkey;

#[cfg(target_os = "macos")]
pub mod integration;
#[cfg(target_os = "macos")]
mod native_events;

#[cfg(target_os = "macos")]
mod exclusive_hotkey;
