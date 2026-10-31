//! Local durability and remote synchronization are independent facts.
use super::page::LocalRevision;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LocalSaveState {
    Clean(LocalRevision),
    Dirty(LocalRevision),
    Saving(LocalRevision),
    Failed(LocalRevision),
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SyncState {
    #[default]
    LocalOnly,
    Pending,
    Syncing,
    Synced,
    Offline,
    ReadOnly,
    Conflict,
    Failed,
    Authentication,
}

/// Local acknowledgement always takes precedence over remote status projection.
pub fn status_text(local: LocalSaveState, remote: SyncState, matches_base: bool) -> &'static str {
    match local {
        LocalSaveState::Dirty(_) => "Unsaved changes",
        LocalSaveState::Saving(_) => "Saving locally…",
        LocalSaveState::Failed(_) => "Local save failed · ⌘S to retry",
        LocalSaveState::Clean(_) => match remote {
            SyncState::LocalOnly => "Saved locally · not syncing",
            SyncState::Offline => "Offline · saved locally",
            SyncState::Authentication => "Sign in · saved locally",
            SyncState::ReadOnly => "Saved locally · read-only connection",
            SyncState::Conflict => "Conflict · saved locally",
            SyncState::Failed => "Cannot sync · saved locally",
            SyncState::Synced if matches_base => "Synced",
            SyncState::Syncing => "Syncing with SilverBullet…",
            SyncState::Pending | SyncState::Synced => "Saved locally · sync pending",
        },
    }
}
