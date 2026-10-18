//! Typed recovery intents; remote mutations remain in the guarded durable worker.
use crate::{
    domain::{connection::ConnectionId, error::StoreError, page::*},
    store::StoreHandle,
};
#[derive(Clone, Debug)]
pub enum RecoveryIntent {
    Resolve(ConflictResolution),
    Trash(PageKey, LocalRevision),
    RestoreTrash(TrashId, Option<PagePath>),
    RestoreCopy(uuid::Uuid, Option<PagePath>),
    EmptyTrash(Vec<TrashId>),
}
pub struct RecoveryOutcome {
    pub affected: Option<PageKey>,
    pub page: Option<crate::domain::reconciliation::StoredPage>,
    pub restored: Option<PageKey>,
}
pub async fn execute(
    store: &StoreHandle,
    connection: &ConnectionId,
    intent: RecoveryIntent,
) -> Result<RecoveryOutcome, StoreError> {
    let mut outcome = RecoveryOutcome {
        affected: None,
        page: None,
        restored: None,
    };
    match intent {
        RecoveryIntent::Resolve(request) => {
            let RecoveryItem::Conflict(record) = store
                .recovery_item(connection, RecoveryKind::Conflict, request.conflict_id)
                .await?
            else {
                return Err(StoreError::InvalidData);
            };
            let key = record.key.clone();
            store.resolve_conflict(request).await?;
            outcome.page = store.load_stored_page(&key).await?;
            outcome.affected = Some(key);
        }
        RecoveryIntent::Trash(key, revision) => {
            if key.connection != *connection {
                return Err(StoreError::InvalidData);
            }
            store.move_to_trash(key.clone(), revision).await?;
            outcome.affected = Some(key);
        }
        RecoveryIntent::RestoreTrash(id, path) => {
            store
                .recovery_item(connection, RecoveryKind::Trash, id.0)
                .await?;
            outcome.restored = Some(store.restore_trash(id, path).await?);
        }
        RecoveryIntent::RestoreCopy(id, path) => {
            store
                .recovery_item(connection, RecoveryKind::Copy, id)
                .await?;
            outcome.restored = Some(store.restore_recovery(id, path).await?);
        }
        RecoveryIntent::EmptyTrash(ids) => {
            for id in &ids {
                store
                    .recovery_item(connection, RecoveryKind::Trash, id.0)
                    .await?;
            }
            store.empty_trash(ids).await?;
        }
    }
    Ok(outcome)
}
