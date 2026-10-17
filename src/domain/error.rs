//! Errors from the small native adapter.
use std::fmt;

#[derive(Debug)]
pub enum PlatformError {
    WrongThread,
    WindowUnavailable,
    #[cfg(target_os = "macos")]
    Handle(raw_window_handle::HandleError),
}

impl fmt::Display for PlatformError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongThread => f.write_str("native probe requires the main thread"),
            Self::WindowUnavailable => f.write_str("native macOS window is not available"),
            #[cfg(target_os = "macos")]
            Self::Handle(error) => write!(f, "native window handle: {error}"),
        }
    }
}
impl std::error::Error for PlatformError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum PathError {
    #[error("page path is empty")]
    Empty,
    #[error("page path must be relative")]
    Absolute,
    #[error("page path contains a forbidden character")]
    ForbiddenCharacter,
    #[error("page path contains an empty or dot segment")]
    InvalidSegment,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ConnectionError {
    #[error("invalid server URL")]
    InvalidUrl,
    #[error("server URL must be an absolute HTTPS address")]
    HttpsRequired,
    #[error("credentials must be supplied separately from the server URL")]
    CredentialsInUrl,
    #[error("server URL cannot contain a query or fragment")]
    QueryOrFragment,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum PositionError {
    #[error("selection is outside UTF-8 boundaries")]
    InvalidBoundary,
    #[error("scroll position must be finite and nonnegative")]
    InvalidScroll,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum RevisionError {
    #[error("local revision counter is exhausted")]
    Exhausted,
}

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("Bulletnotes is already using this local store")]
    AlreadyRunning,
    #[error("local store worker is unavailable")]
    Closed,
    #[error("local database operation failed")]
    Database(#[from] rusqlite::Error),
    #[error("local file operation failed")]
    Io(#[from] std::io::Error),
    #[error("local database is corrupt; preserve it for recovery")]
    CorruptDatabase,
    #[error("local database requires a newer app version")]
    UnsupportedSchema,
    #[error("database migration failed; original data was preserved")]
    MigrationFailed,
    #[error("local database durability configuration could not be verified")]
    Durability,
    #[error("page is unavailable locally")]
    PageMissing,
    #[error("page is read only")]
    ReadOnly,
    #[error("a newer local revision is already saved")]
    StaleRevision,
    #[error("operation is unavailable or already acknowledged")]
    UnknownOperation,
    #[error("stored data failed validation")]
    InvalidData,
    #[error("appearance or application settings failed validation")]
    InvalidSettings,
    #[error("search bounds are invalid")]
    InvalidQuery,
}
