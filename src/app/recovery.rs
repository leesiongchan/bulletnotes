//! Typed recovery intents; remote mutations remain in the guarded durable worker.
use crate::{
    domain::{connection::ConnectionId, error::StoreError, page::*},
    store::StoreHandle,
};
#[derive(Clone, Debug)]
pub enum RecoveryIntent {
    Resolve(ConflictResolution),
    Trash(PageKey, LocalRevision),
    PrepareTrash(Vec<PageKey>),
    TrashMany(Vec<(PageKey, LocalRevision)>),
    MoveScratchpads {
        expected: Vec<PageKey>,
        selected: Vec<PageKey>,
        before: Option<PageKey>,
    },
    RestoreTrash(TrashId, Option<PagePath>),
    RestoreCopy(uuid::Uuid, Option<PagePath>),
    EmptyTrash(Vec<TrashId>),
}
pub struct RecoveryOutcome {
    pub affected: Vec<PageKey>,
    pub prepared_trash: Option<Vec<(PageKey, LocalRevision)>>,
    pub stack: Option<Vec<crate::store::scratchpads::ScratchpadEntry>>,
    pub page: Option<crate::domain::reconciliation::StoredPage>,
    pub restored: Option<PageKey>,
}
pub async fn execute(
    store: &StoreHandle,
    connection: &ConnectionId,
    intent: RecoveryIntent,
) -> Result<RecoveryOutcome, StoreError> {
    let mut outcome = RecoveryOutcome {
        affected: vec![],
        prepared_trash: None,
        stack: None,
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
            outcome.affected.push(key);
        }
        RecoveryIntent::Trash(key, revision) => {
            if key.connection != *connection {
                return Err(StoreError::InvalidData);
            }
            store.move_to_trash(key.clone(), revision).await?;
            outcome.affected.push(key);
        }
        RecoveryIntent::PrepareTrash(keys) => {
            outcome.prepared_trash = Some(store.prepare_trash(connection, keys).await?);
        }
        RecoveryIntent::TrashMany(pages) => {
            outcome.affected = pages.iter().map(|(key, _)| key.clone()).collect();
            store.move_many_to_trash(connection, pages).await?;
        }
        RecoveryIntent::MoveScratchpads {
            expected,
            selected,
            before,
        } => {
            outcome.stack = Some(
                store
                    .move_scratchpads(connection, expected, selected, before)
                    .await?,
            );
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
