//! Supported native integration boundaries.
#[cfg(target_os = "macos")]
pub mod application;
pub mod desktop;
#[cfg(not(target_os = "macos"))]
pub use desktop as application;
#[cfg(target_os = "macos")]
pub mod window_chrome;

pub mod credentials;

pub mod hotkey;

#[cfg(target_os = "macos")]
pub mod integration;
#[cfg(target_os = "macos")]
mod native_events;

#[cfg(target_os = "macos")]
mod exclusive_hotkey;

pub mod installation;

#[cfg(target_os = "macos")]
pub mod renderer;
#[cfg(not(target_os = "macos"))]
pub use desktop as renderer;
