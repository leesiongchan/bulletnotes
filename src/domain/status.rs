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
}
