//! Durable source identity and immutable automatic-recovery obligations.
use super::page::*;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct StoredPage {
    pub page: PageSnapshot,
    pub source_generation: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredMergeDraft {
    pub body: String,
    pub revision: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum AutomaticRecoveryReason {
    Overlap,
    NoCommonBase,
    BudgetFallback,
    AuthoredDraft,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AutomaticRecoveryInput {
    pub local: PageSnapshot,
    pub base: Option<RemoteFile>,
    pub remote: Option<RemoteRead>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub editor: Option<EditorRecoveryProvenance>,
    pub draft: Option<String>,
    pub kept_body: String,
    pub reason: AutomaticRecoveryReason,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AutomaticRecoveryEvent {
    pub id: Uuid,
    pub input: AutomaticRecoveryInput,
    pub created_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecoveryUpload {
    pub event_id: Uuid,
    pub ordinal: u32,
    pub operation: PendingMutation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecoveryUploadState {
    Pending,
    Leased,
    Unknown,
    Blocked,
    Acknowledged,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecoveryUploadHead {
    pub current: bool,
    pub event_id: Uuid,
    pub ordinal: u32,
    pub operation_id: OperationId,
    pub key: PageKey,
    pub state: RecoveryUploadState,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReconcileMutationState {
    Pending,
    Blocked,
    Leased,
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReconcileMutation {
    pub id: OperationId,
    pub state: ReconcileMutationState,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReconcileToken {
    pub key: PageKey,
    pub source_generation: u64,
    pub expected_local: LocalRevision,
    pub conflict_id: Option<Uuid>,
    pub draft_revision: Option<u64>,
    pub mutations: Vec<ReconcileMutation>,
    pub trashed: bool,
    pub remote_deleted: bool,
    pub explicit_create: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReconcileContext {
    pub token: ReconcileToken,
    pub local: PageSnapshot,
    /// A legacy conflict can retain an older common base than the current remote base.
    pub base: Option<RemoteFile>,
    pub remote: RemoteRead,
    pub draft: Option<StoredMergeDraft>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResolutionDisposition {
    Install,
    RetainDeleted,
    KeepDeletion,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PreparedResolution {
    pub context: ReconcileContext,
    pub body: String,
    pub disposition: ResolutionDisposition,
    pub recovery_reason: Option<AutomaticRecoveryReason>,
    pub preserve_history: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LocalSaveOutcome {
    pub attempt: SavedRevision,
    pub stored: StoredPage,
}

/// Identity of the buffer that admitted asynchronous work; revisions alone are not identity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditorToken {
    pub key: PageKey,
    pub session_id: Uuid,
    pub input_generation: u64,
    pub source_generation: u64,
}

impl From<PageSnapshot> for StoredPage {
    fn from(page: PageSnapshot) -> Self {
        Self {
            page,
            source_generation: 0,
        }
    }
}

/// Canonical source is local provenance, never an invented HTTP observation.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct EditorRecoveryProvenance {
    #[serde(default)]
    pub remote_missing: bool,
    pub baseline: String,
    pub durable: StoredPage,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct EditorHandoff {
    pub id: Uuid,
    pub baseline: String,
    pub input: PageSnapshot,
    pub displaced: StoredPage,
}

/// Durable canonical snapshot plus an accepted authored branch awaiting background replay.
#[derive(Clone, Debug, PartialEq)]
pub struct EditorRestore {
    pub stored: StoredPage,
    pub authored: Option<PageSnapshot>,
}
impl EditorRestore {
    pub fn presented(&self) -> &PageSnapshot {
        self.authored.as_ref().unwrap_or(&self.stored.page)
    }
}
impl From<StoredPage> for EditorRestore {
    fn from(stored: StoredPage) -> Self {
        Self {
            stored,
            authored: None,
        }
    }
}
impl From<PageSnapshot> for EditorRestore {
    fn from(page: PageSnapshot) -> Self {
        StoredPage::from(page).into()
    }
}
