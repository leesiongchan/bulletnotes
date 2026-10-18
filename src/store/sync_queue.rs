//! Bounded worker admission; queued bodies remain in SQLite until claimed.
use super::{outbox, pages};
use crate::domain::{connection::ConnectionId, error::StoreError, page::*};
use rusqlite::{Connection, OptionalExtension, params};
#[derive(Clone, Debug)]
pub struct PendingHead {
    pub id: OperationId,
    pub key: PageKey,
    pub revision: LocalRevision,
}
#[derive(Clone, Copy, Debug, Default)]
pub struct CacheProgress {
    pub indexed: usize,
    pub catalogue_total: usize,
    pub complete: bool,
    pub pending: usize,
    pub conflicts: usize,
}
pub(super) fn heads(
    db: &Connection,
    connection: &ConnectionId,
) -> Result<Vec<PendingHead>, StoreError> {
    let mut query=db.prepare("SELECT o.operation_id,o.page_id,o.local_revision FROM outbox o JOIN pages p ON p.id=o.page_id WHERE p.connection=?1 AND p.permission=1 AND o.state='pending' AND NOT EXISTS(SELECT 1 FROM outbox a WHERE a.page_id=o.page_id AND a.state IN ('leased','unknown')) ORDER BY o.rowid")?;
    query
        .query_map([connection.as_str()], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?
        .map(|row| {
            let (id, page, revision) = row?;
            Ok(PendingHead {
                id: OperationId(uuid::Uuid::parse_str(&id).map_err(|_| StoreError::InvalidData)?),
                key: pages::key_for_id(db, page)?,
                revision: LocalRevision(revision.parse().map_err(|_| StoreError::InvalidData)?),
            })
        })
        .collect()
}
pub(super) fn claim(
    db: &mut Connection,
    id: OperationId,
) -> Result<Option<PendingMutation>, StoreError> {
    let tx = db.transaction()?;
    let eligible=tx.query_row("SELECT EXISTS(SELECT 1 FROM outbox o JOIN pages p ON p.id=o.page_id WHERE o.operation_id=?1 AND o.state='pending' AND p.permission=1 AND NOT EXISTS(SELECT 1 FROM outbox a WHERE a.page_id=o.page_id AND a.state IN ('leased','unknown')))",[id.0.to_string()],|row|row.get::<_,bool>(0))?;
    if !eligible {
        return Ok(None);
    }
    let operation = outbox::read_operation(&tx, &id.0.to_string())?;
    tx.execute(
        "UPDATE outbox SET state='leased' WHERE operation_id=?1",
        [id.0.to_string()],
    )?;
    tx.commit()?;
    Ok(Some(operation))
}
pub(super) fn leased(db: &Connection, id: OperationId) -> Result<bool, StoreError> {
    Ok(db.query_row("SELECT EXISTS(SELECT 1 FROM outbox o JOIN pages p ON p.id=o.page_id WHERE o.operation_id=?1 AND o.state='leased' AND p.permission=1 AND NOT EXISTS(SELECT 1 FROM conflicts c WHERE c.connection=p.connection AND c.path=p.path))",[id.0.to_string()],|row|row.get(0))?)
}
pub(super) fn mark_unknown(db: &Connection, id: OperationId) -> Result<(), StoreError> {
    db.execute(
        "UPDATE outbox SET state='unknown' WHERE operation_id=?1 AND state='leased'",
        [id.0.to_string()],
    )?;
    Ok(())
}
pub(super) fn unknown_batch(
    db: &Connection,
    connection: &ConnectionId,
    limit: usize,
) -> Result<Vec<PendingMutation>, StoreError> {
    let limit = i64::try_from(limit).map_err(|_| StoreError::InvalidQuery)?;
    let mut statement=db.prepare("SELECT o.operation_id FROM outbox o JOIN pages p ON p.id=o.page_id WHERE p.connection=?1 AND o.state='unknown' AND NOT EXISTS(SELECT 1 FROM conflicts c WHERE c.connection=p.connection AND c.path=p.path) ORDER BY o.rowid LIMIT ?2")?;
    let ids = statement
        .query_map(params![connection.as_str(), limit], |row| {
            row.get::<_, String>(0)
        })?
        .collect::<Result<Vec<_>, _>>()?;
    ids.iter()
        .map(|id| outbox::read_operation(db, id))
        .collect()
}
pub(super) fn is_unknown(db: &Connection, id: OperationId) -> Result<bool, StoreError> {
    Ok(db.query_row(
        "SELECT EXISTS(SELECT 1 FROM outbox WHERE operation_id=?1 AND state='unknown')",
        [id.0.to_string()],
        |row| row.get(0),
    )?)
}
pub(super) fn retry(db: &mut Connection, id: OperationId) -> Result<(), StoreError> {
    let tx = db.transaction()?;
    let state = tx
        .query_row(
            "SELECT state FROM outbox WHERE operation_id=?1",
            [id.0.to_string()],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    if state.as_deref() != Some("unknown") {
        return Err(StoreError::UnknownOperation);
    }
    let operation = outbox::read_operation(&tx, &id.0.to_string())?;
    let page = pages::id(&tx, &operation.key)?;
    tx.execute(
        "DELETE FROM outbox WHERE operation_id=?1",
        [id.0.to_string()],
    )?;
    let current = pages::load(&tx, &operation.key)?.ok_or(StoreError::PageMissing)?;
    if operation.kind == MutationKind::Delete && operation.local_revision == current.local_revision
    {
        tx.execute(
            "DELETE FROM outbox WHERE page_id=?1 AND state IN ('pending','blocked')",
            [page],
        )?;
        let WriteCondition::Match(revision) = operation.condition else {
            return Err(StoreError::InvalidData);
        };
        tx.execute("INSERT INTO outbox(operation_id,page_id,local_revision,kind,condition_kind,condition_revision,state) VALUES(?1,?2,?3,'delete','match',?4,'pending')",params![OperationId::new().0.to_string(),page,current.local_revision.0.to_string(),revision.0])?;
    } else {
        outbox::queue_put(&tx, page, &current)?;
    }
    tx.commit()?;
    Ok(())
}
pub(super) fn block(db: &Connection, key: &PageKey) -> Result<(), StoreError> {
    db.execute(
        "UPDATE pages SET permission=0 WHERE connection=?1 AND path=?2",
        [key.connection.as_str(), key.path.as_str()],
    )?;
    db.execute("UPDATE outbox SET state=CASE state WHEN 'leased' THEN 'unknown' WHEN 'pending' THEN 'blocked' ELSE state END WHERE page_id=?1",[pages::id(db,key)?])?;
    Ok(())
}
pub(super) fn release_connection(
    db: &Connection,
    connection: &ConnectionId,
) -> Result<(), StoreError> {
    db.execute("UPDATE outbox SET state='unknown' WHERE state='leased' AND page_id IN (SELECT id FROM pages WHERE connection=?1)",[connection.as_str()])?;
    Ok(())
}
pub(super) fn keys(db: &Connection, connection: &ConnectionId) -> Result<Vec<PageKey>, StoreError> {
    let mut query=db.prepare("SELECT id FROM pages WHERE connection=?1 AND (catalogued=1 OR (remote_deleted=0 AND (remote_body IS NOT NULL OR body IS NULL))) ORDER BY path")?;
    let ids = query
        .query_map([connection.as_str()], |row| row.get::<_, i64>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    ids.into_iter()
        .map(|id| pages::key_for_id(db, id))
        .collect()
}
pub(super) fn progress(
    db: &Connection,
    connection: &ConnectionId,
) -> Result<CacheProgress, StoreError> {
    let (total, indexed) = db.query_row(
        "SELECT count(*),count(body) FROM pages WHERE connection=?1 AND remote_deleted=0 AND trashed=0",
        [connection.as_str()],
        |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
    )?;
    let pending = db.query_row(
        "SELECT (SELECT count(*) FROM outbox o JOIN pages p ON p.id=o.page_id WHERE p.connection=?1) + (SELECT count(*) FROM recovery_uploads u JOIN automatic_events e ON e.id=u.event_id WHERE e.connection=?1 AND ((u.current=1 AND u.state!='acknowledged') OR u.state IN ('leased','unknown')))",
        [connection.as_str()],
        |row| row.get::<_, i64>(0),
    )?;
    let conflicts = db.query_row(
        "SELECT count(*) FROM conflicts WHERE connection=?1",
        [connection.as_str()],
        |row| row.get::<_, i64>(0),
    )?;
    let total = usize::try_from(total).map_err(|_| StoreError::InvalidData)?;
    let indexed = usize::try_from(indexed).map_err(|_| StoreError::InvalidData)?;
    let pending = usize::try_from(pending).map_err(|_| StoreError::InvalidData)?;
    let conflicts = usize::try_from(conflicts).map_err(|_| StoreError::InvalidData)?;
    let complete = total == indexed
        && db.query_row(
            "SELECT EXISTS(SELECT 1 FROM catalogue_state WHERE connection=?1)",
            [connection.as_str()],
            |row| row.get::<_, bool>(0),
        )?;
    Ok(CacheProgress {
        catalogue_total: total,
        indexed,
        complete,
        pending,
        conflicts,
    })
}

#[derive(Clone, Copy, Debug, Default)]
pub struct PageSyncFacts {
    pub pending: bool,
    pub conflict: bool,
    pub deleted: bool,
    pub conflict_id: Option<uuid::Uuid>,
}
pub(super) fn page_facts(
    db: &Connection,
    key: &PageKey,
) -> Result<Option<(PageSnapshot, PageSyncFacts)>, StoreError> {
    let Some(page) = pages::load(db, key)? else {
        return Ok(None);
    };
    let (pending,conflict,deleted,id)=db.query_row("SELECT (EXISTS(SELECT 1 FROM outbox o WHERE o.page_id=p.id) OR EXISTS(SELECT 1 FROM recovery_uploads u JOIN automatic_events e ON e.id=u.event_id WHERE e.connection=p.connection AND (e.path=p.path OR u.path=p.path) AND ((u.current=1 AND u.state!='acknowledged') OR u.state IN ('leased','unknown')))),EXISTS(SELECT 1 FROM conflicts c WHERE c.connection=p.connection AND c.path=p.path),remote_deleted,(SELECT id FROM conflicts c WHERE c.connection=p.connection AND c.path=p.path) FROM pages p WHERE connection=?1 AND path=?2",[key.connection.as_str(),key.path.as_str()],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get::<_,Option<String>>(3)?)))?;
    let conflict_id = id
        .map(|id| uuid::Uuid::parse_str(&id).map_err(|_| StoreError::InvalidData))
        .transpose()?;
    Ok(Some((
        page,
        PageSyncFacts {
            pending,
            conflict,
            deleted,
            conflict_id,
        },
    )))
}
