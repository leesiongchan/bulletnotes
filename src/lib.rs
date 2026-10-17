//! Native feasibility harness for the approved Bulletnotes design.
slint::include_modules!();
#[cfg(target_os = "macos")]
pub mod domain;
pub mod platform;
