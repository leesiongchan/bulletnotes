//! Durable source identity and immutable automatic-recovery obligations.
use super::page::*;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq)]
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
    pub remote: RemoteRead,
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
    pub event_id: Uuid,
    pub ordinal: u32,
    pub operation_id: OperationId,
    pub key: PageKey,
    pub state: RecoveryUploadState,
}
