//! Errors from the small native adapter.
use std::fmt;

#[derive(Debug)]
pub enum PlatformError {
    WrongThread,
    WindowUnavailable,
    Handle(raw_window_handle::HandleError),
}

impl fmt::Display for PlatformError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongThread => f.write_str("native probe requires the main thread"),
            Self::WindowUnavailable => f.write_str("native macOS window is not available"),
            Self::Handle(error) => write!(f, "native window handle: {error}"),
        }
    }
}
impl std::error::Error for PlatformError {}
