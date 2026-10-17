//! Stable page identity independent of display text.
use super::{
    connection::ConnectionId,
    error::{PathError, RevisionError},
};
use uuid::Uuid;

#[derive(
    Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
#[serde(try_from = "String", into = "String")]
pub struct PagePath(String);
impl PagePath {
    pub fn parse(input: &str) -> Result<Self, PathError> {
        if input.is_empty() {
            return Err(PathError::Empty);
        }
        if input.starts_with('/') || input.as_bytes().get(1) == Some(&b':') {
            return Err(PathError::Absolute);
        }
        if input.contains(['\\', '\0']) {
            return Err(PathError::ForbiddenCharacter);
        }
        if input
            .split('/')
            .any(|s| s.is_empty() || s == "." || s == "..")
        {
            return Err(PathError::InvalidSegment);
        }
        Ok(Self(input.to_owned()))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(
    Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub struct PageKey {
    pub connection: ConnectionId,
    pub path: PagePath,
}
impl PageKey {
    pub fn title_from<'a>(&'a self, body: &'a str) -> &'a str {
        body.lines()
            .find_map(|line| {
                let title = line.trim().trim_start_matches('#').trim();
                (!title.is_empty()).then_some(title)
            })
            .unwrap_or_else(|| {
                self.path
                    .as_str()
                    .rsplit('/')
                    .next()
                    .unwrap_or(self.path.as_str())
                    .trim_end_matches(".md")
            })
    }
}

#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    serde::Serialize,
    serde::Deserialize,
)]
pub struct LocalRevision(pub u64);
impl LocalRevision {
    pub fn next(self) -> Result<Self, RevisionError> {
        self.0
            .checked_add(1)
            .map(Self)
            .ok_or(RevisionError::Exhausted)
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct RemoteRevision(pub String);
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct OperationId(pub Uuid);
impl OperationId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}
impl Default for OperationId {
    fn default() -> Self {
        Self::new()
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct TrashId(pub Uuid);
impl TrashId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}
impl Default for TrashId {
    fn default() -> Self {
        Self::new()
    }
}

/// Call once per draft and persist the resulting path; retries reuse it.
pub fn new_note_path(now: &jiff::Zoned, id: Uuid) -> PagePath {
    PagePath(format!(
        "Quick Notes/{}/{}-{id}.md",
        now.strftime("%Y-%m-%d"),
        now.strftime("%H%M%S")
    ))
}

impl TryFrom<String> for PagePath {
    type Error = PathError;
    fn try_from(input: String) -> Result<Self, Self::Error> {
        Self::parse(&input)
    }
}
impl From<PagePath> for String {
    fn from(path: PagePath) -> Self {
        path.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Permission {
    ReadOnly,
    ReadWrite,
}
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RemoteFile {
    pub key: PageKey,
    pub body: String,
    pub revision: Option<RemoteRevision>,
    pub modified_ms: Option<u64>,
    pub permission: Permission,
}
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PageSnapshot {
    pub key: PageKey,
    pub body: String,
    pub local_revision: LocalRevision,
    pub remote_base: Option<RemoteFile>,
    pub position: crate::app::model::EditorPosition,
    pub permission: Permission,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SavedRevision {
    pub key: PageKey,
    pub local_revision: LocalRevision,
}
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum MutationKind {
    Put(String),
    Delete,
}
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum WriteCondition {
    CreateOnly,
    Match(RemoteRevision),
}
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PendingMutation {
    pub id: OperationId,
    pub key: PageKey,
    pub local_revision: LocalRevision,
    pub kind: MutationKind,
    pub condition: WriteCondition,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MutationAck {
    pub remote_revision: Option<RemoteRevision>,
}
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum RemoteRead {
    Present(RemoteFile),
    Missing(PageKey),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemoteMetadata {
    pub key: PageKey,
    pub modified_ms: Option<u64>,
    pub content_length: Option<u64>,
    pub permission: Permission,
    pub revision: Option<RemoteRevision>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ConflictRecord {
    pub id: Uuid,
    pub key: PageKey,
    pub base: Option<RemoteFile>,
    pub local: PageSnapshot,
    pub remote: RemoteRead,
}
#[derive(Clone, Debug, PartialEq)]
pub enum ReconcileDecision {
    Unchanged,
    InstallRemote(RemoteFile),
    KeepPending,
    Conflict(Box<ConflictRecord>),
    RetainDeletedSnapshot,
}

/// What the editor last committed, plus the remote base from which it edited.
/// The committed body distinguishes an acknowledged local write from a remote
/// replacement that arrived before the editor could commit its next change.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditorBaseline {
    pub body: String,
    pub remote_base: Option<RemoteFile>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResolutionChoice {
    AcceptRemote,
    KeepLocal,
    Merge(String),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConflictResolution {
    pub conflict_id: Uuid,
    pub expected_local: LocalRevision,
    pub choice: ResolutionChoice,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum RecoveryReason {
    CommonBase,
    LocalVersion,
    RemoteVersion,
    RemoteDeletion,
    MergedDraft,
}
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RecoveryCopy {
    pub id: Uuid,
    pub page: PageSnapshot,
    pub reason: RecoveryReason,
}
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TrashRecord {
    pub id: TrashId,
    pub page: PageSnapshot,
    pub pending: Vec<PendingMutation>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecoveryKind {
    Conflict,
    Trash,
    Copy,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecoverySummary {
    pub label: String,
    pub id: Uuid,
    pub key: PageKey,
    pub kind: RecoveryKind,
}
#[derive(Clone, Debug, PartialEq)]
pub enum RecoveryItem {
    Conflict(Box<ConflictRecord>),
    Trash(Box<TrashRecord>),
    Copy(Box<RecoveryCopy>),
}
