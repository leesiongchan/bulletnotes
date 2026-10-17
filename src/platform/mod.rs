//! Supported native integration boundaries.
#[cfg(target_os = "macos")]
pub mod application;
#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(target_os = "macos")]
pub mod window_chrome;

pub mod credentials;
