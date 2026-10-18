//! Retained request evidence is independent of the authority to repair its delayed effects.
use super::{StoreHandle, outbox, pages};
use crate::domain::{connection::ConnectionId, error::StoreError, page::*, reconciliation::*};
use rusqlite::{Connection, OptionalExtension, params};

impl StoreHandle {
    pub async fn retired_delete_keys(
        &self,
        connection: &ConnectionId,
    ) -> Result<Vec<PageKey>, StoreError> {
        let connection = connection.clone();
        self.call(move |w| {
            let mut query=w.db.prepare("SELECT DISTINCT path FROM retired_delete_watches WHERE connection=?1 AND active=1 ORDER BY path")?;
            query.query_map([connection.as_str()],|row|row.get::<_,String>(0))?.map(|row|Ok(PageKey {connection:connection.clone(),path:PagePath::parse(&row?).map_err(|_|StoreError::InvalidData)?})).collect()
        }).await
    }
}
pub(super) fn authority(db: &Connection, key: &PageKey) -> Result<Vec<String>, StoreError> {
    let mut query=db.prepare("SELECT operation_id FROM retired_delete_watches WHERE connection=?1 AND path=?2 AND active=1 ORDER BY operation_id")?;
    Ok(query
        .query_map([key.connection.as_str(), key.path.as_str()], |row| {
            row.get(0)
        })?
        .collect::<Result<Vec<_>, _>>()?)
}
pub(super) fn retire(
    db: &Connection,
    operation: &PendingMutation,
    context: &ReconcileContext,
) -> Result<(), StoreError> {
    let original = serde_json::to_string(&(
        context.local.clone(),
        context.base.clone(),
        context.remote.clone(),
        context.draft.as_ref().map(|d| (&d.body, d.revision)),
    ))
    .map_err(|_| StoreError::InvalidData)?;
    let operation_json = serde_json::to_string(operation).map_err(|_| StoreError::InvalidData)?;
    let protected = serde_json::to_string(&context.local).map_err(|_| StoreError::InvalidData)?;
    let rejected: bool = db.query_row(
        "SELECT delete_rejected FROM outbox WHERE operation_id=?1",
        [operation.id.0.to_string()],
        |row| row.get(0),
    )?;
    db.execute("INSERT INTO retired_delete_watches(operation_id,connection,path,operation,original,protected,terminal_rejected,active,created_ms) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",params![operation.id.0.to_string(),operation.key.connection.as_str(),operation.key.path.as_str(),operation_json,original,protected,rejected,!rejected,pages::now_ms()?])?;
    Ok(())
}
pub(super) fn protect(db: &Connection, page: &PageSnapshot) -> Result<(), StoreError> {
    let value = serde_json::to_string(page).map_err(|_| StoreError::InvalidData)?;
    db.execute("UPDATE retired_delete_watches SET protected=?1 WHERE connection=?2 AND path=?3 AND active=1",params![value,page.key.connection.as_str(),page.key.path.as_str()])?;
    Ok(())
}
pub(super) fn supersede(db: &Connection, key: &PageKey) -> Result<(), StoreError> {
    db.execute(
        "UPDATE retired_delete_watches SET active=0 WHERE connection=?1 AND path=?2 AND active=1",
        [key.connection.as_str(), key.path.as_str()],
    )?;
    Ok(())
}
/// Only a missing page may be restored from the durable authorized snapshot.
/// Existing source always enters the checked reconciliation facade instead.
pub(super) fn restore_missing(db: &mut Connection, key: &PageKey) -> Result<(), StoreError> {
    let tx = db.transaction()?;
    if pages::load(&tx, key)?.is_some() {
        return Ok(());
    }
    let value:Option<String>=tx.query_row("SELECT protected FROM retired_delete_watches WHERE connection=?1 AND path=?2 AND active=1 ORDER BY rowid DESC LIMIT 1",[key.connection.as_str(),key.path.as_str()],|row|row.get(0)).optional()?;
    if let Some(value) = value {
        let page: PageSnapshot =
            serde_json::from_str(&value).map_err(|_| StoreError::InvalidData)?;
        if page.key != *key {
            return Err(StoreError::InvalidData);
        }
        tx.execute("INSERT INTO pages(connection,path,title,body,local_revision,explicit_create,modified_ms) VALUES(?1,?2,?3,?4,?5,1,?6) ON CONFLICT(connection,path) DO UPDATE SET title=excluded.title,body=excluded.body,local_revision=excluded.local_revision,explicit_create=1 WHERE pages.body IS NULL",params![key.connection.as_str(),key.path.as_str(),key.title_from(&page.body),page.body,page.local_revision.0.to_string(),pages::now_ms()?])?;
        let restored = pages::load(&tx, key)?.ok_or(StoreError::PageMissing)?;
        outbox::queue_put(&tx, pages::id(&tx, key)?, &restored)?;
    }
    tx.commit()?;
    Ok(())
}
