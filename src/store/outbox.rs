//! A leased payload is immutable; newer edits become a separate unsent intent.
use super::pages;
use crate::domain::{connection::ConnectionId, error::StoreError, page::*};
use rusqlite::{Connection, OptionalExtension};

pub(super) fn queue_put(
    db: &Connection,
    page_id: i64,
    snapshot: &PageSnapshot,
) -> Result<(), StoreError> {
    db.execute(
        "DELETE FROM outbox WHERE page_id=?1 AND state IN ('pending','blocked')",
        [page_id],
    )?;
    let inflight = db.query_row(
        "SELECT EXISTS(SELECT 1 FROM outbox WHERE page_id=?1 AND state IN ('leased','unknown'))",
        [page_id],
        |row| row.get::<_, bool>(0),
    )?;
    if !inflight
        && ((snapshot.remote_base.is_none() && snapshot.body.is_empty())
            || snapshot
                .remote_base
                .as_ref()
                .is_some_and(|base| base.body == snapshot.body))
    {
        return Ok(());
    }
    let (condition, revision, state) = match &snapshot.remote_base {
        None => ("create", None, "pending"),
        Some(base) => match &base.revision {
            Some(revision) => ("match", Some(revision.0.as_str()), "pending"),
            None => ("match", None, "blocked"),
        },
    };
    db.execute("INSERT INTO outbox(operation_id,page_id,local_revision,kind,body,condition_kind,condition_revision,state) VALUES(?1,?2,?3,'put',?4,?5,?6,?7)",rusqlite::params![OperationId::new().0.to_string(),page_id,snapshot.local_revision.0.to_string(),snapshot.body,condition,revision,state])?;
    Ok(())
}
fn read_operation(db: &Connection, id: &str) -> Result<PendingMutation, StoreError> {
    let (page_id,revision,kind,body,condition,etag)=db.query_row("SELECT page_id,local_revision,kind,body,condition_kind,condition_revision FROM outbox WHERE operation_id=?1",[id],|row|Ok((row.get::<_,i64>(0)?,row.get::<_,String>(1)?,row.get::<_,String>(2)?,row.get::<_,Option<String>>(3)?,row.get::<_,String>(4)?,row.get::<_,Option<String>>(5)?)))?;
    let kind = match kind.as_str() {
        "put" => MutationKind::Put(body.ok_or(StoreError::InvalidData)?),
        "delete" => MutationKind::Delete,
        _ => return Err(StoreError::InvalidData),
    };
    let condition = match condition.as_str() {
        "create" => WriteCondition::CreateOnly,
        "match" => WriteCondition::Match(RemoteRevision(etag.ok_or(StoreError::InvalidData)?)),
        _ => return Err(StoreError::InvalidData),
    };
    Ok(PendingMutation {
        id: OperationId(uuid::Uuid::parse_str(id).map_err(|_| StoreError::InvalidData)?),
        key: pages::key_for_id(db, page_id)?,
        local_revision: LocalRevision(revision.parse().map_err(|_| StoreError::InvalidData)?),
        kind,
        condition,
    })
}
pub(super) fn claim(
    db: &mut Connection,
    connection: &ConnectionId,
    limit: usize,
) -> Result<Vec<PendingMutation>, StoreError> {
    let limit = i64::try_from(limit).map_err(|_| StoreError::InvalidQuery)?;
    let tx = db.transaction()?;
    let ids = {
        let mut query=tx.prepare("SELECT o.operation_id FROM outbox o JOIN pages p ON p.id=o.page_id WHERE p.connection=?1 AND o.state='pending' AND NOT EXISTS(SELECT 1 FROM outbox active WHERE active.page_id=o.page_id AND active.state IN ('leased','unknown')) ORDER BY o.rowid LIMIT ?2")?;
        query
            .query_map(rusqlite::params![connection.as_str(), limit], |row| {
                row.get::<_, String>(0)
            })?
            .collect::<Result<Vec<_>, _>>()?
    };
    let mut operations = Vec::with_capacity(ids.len());
    for id in ids {
        operations.push(read_operation(&tx, &id)?);
        tx.execute(
            "UPDATE outbox SET state='leased' WHERE operation_id=?1",
            [id],
        )?;
    }
    tx.commit()?;
    Ok(operations)
}
pub(super) fn unknown(
    db: &Connection,
    connection: &ConnectionId,
) -> Result<Vec<PendingMutation>, StoreError> {
    let mut query=db.prepare("SELECT o.operation_id FROM outbox o JOIN pages p ON p.id=o.page_id WHERE p.connection=?1 AND o.state='unknown' ORDER BY o.rowid")?;
    let ids = query
        .query_map([connection.as_str()], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    ids.iter().map(|id| read_operation(db, id)).collect()
}
pub(super) fn acknowledge(
    db: &mut Connection,
    id: OperationId,
    ack: MutationAck,
) -> Result<Option<PageSnapshot>, StoreError> {
    let id = id.0.to_string();
    let tx = db.transaction()?;
    let state = tx
        .query_row(
            "SELECT state FROM outbox WHERE operation_id=?1",
            [&id],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .ok_or(StoreError::UnknownOperation)?;
    if !matches!(state.as_str(), "leased" | "unknown") {
        return Err(StoreError::UnknownOperation);
    }
    let sent = read_operation(&tx, &id)?;
    let page_id = pages::id(&tx, &sent.key)?;
    let current = pages::load(&tx, &sent.key)?.ok_or(StoreError::PageMissing)?;
    if let MutationKind::Put(body) = &sent.kind {
        tx.execute("UPDATE pages SET remote_body=?1,remote_revision=?2,remote_modified_ms=NULL WHERE id=?3",rusqlite::params![body,ack.remote_revision.map(|rev|rev.0),page_id])?;
    } else if current.local_revision == sent.local_revision {
        tx.execute("DELETE FROM pages WHERE id=?1", [page_id])?;
        tx.commit()?;
        return Ok(None);
    } else {
        tx.execute("UPDATE pages SET remote_body=NULL,remote_revision=NULL,remote_modified_ms=NULL WHERE id=?1",[page_id])?;
    }
    tx.execute("DELETE FROM outbox WHERE operation_id=?1", [&id])?;
    let snapshot = pages::load(&tx, &sent.key)?.ok_or(StoreError::PageMissing)?;
    queue_put(&tx, page_id, &snapshot)?;
    tx.commit()?;
    Ok(Some(snapshot))
}
