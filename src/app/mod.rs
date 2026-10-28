pub mod appearance;
pub mod appearance_ui;
pub mod coordinator;
pub mod desktop_ui;
pub mod editor;
pub mod find;
pub mod local_ui;
pub mod model;
pub mod native_app;
pub mod navigation;
pub mod scratchpads;
pub mod startup;
pub mod theme_files;
pub mod ui;
pub mod window_placement;

pub mod connection;

pub mod connection_ui;

pub mod sync_ui;

pub mod recovery;
pub mod recovery_ui;

#[cfg(target_os = "macos")]
pub mod native_ui;
#[cfg(not(target_os = "macos"))]
pub use desktop_ui as native_ui;

pub mod diagnostics_ui;

mod lifecycle;

mod markdown;

mod source_commands;
