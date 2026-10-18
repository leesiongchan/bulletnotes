//! Atomic remote observations, editor-base checks, and durable conflict capture.
use super::pages;
use crate::{
    domain::{connection::ConnectionId, error::StoreError, page::*},
    sync::reconcile::same_base,
};
use rusqlite::{Connection, OptionalExtension, params};
use std::collections::HashSet;
pub(super) fn read_conflict(
    db: &Connection,
    key: &PageKey,
) -> Result<Option<ConflictRecord>, StoreError> {
    let row = db
        .query_row(
            "SELECT id,snapshot FROM conflicts WHERE connection=?1 AND path=?2",
            [key.connection.as_str(), key.path.as_str()],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()?;
    row.map(|(id, value)| {
        let record: ConflictRecord =
            serde_json::from_str(&value).map_err(|_| StoreError::InvalidData)?;
        validate_record(&record)?;
        if record.key != *key || record.id.to_string() != id {
            return Err(StoreError::InvalidData);
        }
        Ok(record)
    })
    .transpose()
}
fn validate_record(record: &ConflictRecord) -> Result<(), StoreError> {
    super::restoration::validate_page(&record.local)?;
    let remote_key = match &record.remote {
        RemoteRead::Present(file) => &file.key,
        RemoteRead::Missing(key) => key,
    };
    if record.key != record.local.key
        || &record.key != remote_key
        || record
            .base
            .as_ref()
            .is_some_and(|base| base.key != record.key)
    {
        return Err(StoreError::InvalidData);
    }
    Ok(())
}

fn capture(db: &Connection, mut record: ConflictRecord) -> Result<ConflictRecord, StoreError> {
    if let Some(previous) = read_conflict(db, &record.key)? {
        let changed = match (&previous.remote, &record.remote) {
            (RemoteRead::Present(a), RemoteRead::Present(b)) => !same_base(Some(a), Some(b)),
            (RemoteRead::Missing(a), RemoteRead::Missing(b)) => a != b,
            _ => true,
        };
        if changed || previous.local.body != record.local.body {
            super::restoration::preserve_conflict(db, &previous)?;
        }
        record.id = if !changed {
            previous.id
        } else {
            uuid::Uuid::new_v4()
        };
        record.base = previous.base;
    }
    db.execute(
        "UPDATE pages SET remote_deleted=0 WHERE connection=?1 AND path=?2",
        [record.key.connection.as_str(), record.key.path.as_str()],
    )?;
    validate_record(&record)?;
    let value = serde_json::to_string(&record).map_err(|_| StoreError::InvalidData)?;
    db.execute("INSERT INTO conflicts(connection,path,snapshot,id) VALUES(?1,?2,?3,?4) ON CONFLICT(connection,path) DO UPDATE SET snapshot=excluded.snapshot,id=excluded.id",params![record.key.connection.as_str(),record.key.path.as_str(),value,record.id.to_string()])?;
    db.execute("UPDATE outbox SET state=CASE state WHEN 'pending' THEN 'blocked' WHEN 'leased' THEN 'unknown' ELSE state END WHERE page_id=?1",[pages::id(db,&record.key)?])?;
    Ok(record)
}
pub(super) fn conflicts(
    db: &Connection,
    connection: &ConnectionId,
) -> Result<Vec<ConflictRecord>, StoreError> {
    let mut statement =
        db.prepare("SELECT path FROM conflicts WHERE connection=?1 ORDER BY path")?;
    let paths = statement
        .query_map([connection.as_str()], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    paths
        .into_iter()
        .map(|path| {
            let key = PageKey {
                connection: connection.clone(),
                path: PagePath::parse(&path).map_err(|_| StoreError::InvalidData)?,
            };
            read_conflict(db, &key)?.ok_or(StoreError::InvalidData)
        })
        .collect()
}

pub(super) fn catalogue(
    db: &mut Connection,
    connection: &ConnectionId,
    entries: Vec<RemoteMetadata>,
    complete: bool,
) -> Result<(), StoreError> {
    if !complete {
        return Ok(());
    }
    let mut seen = HashSet::new();
    for entry in &entries {
        if &entry.key.connection != connection || !seen.insert(entry.key.path.clone()) {
            return Err(StoreError::InvalidData);
        }
    }
    let tx = db.transaction()?;
    tx.execute(
        "UPDATE pages SET catalogued=0 WHERE connection=?1",
        [connection.as_str()],
    )?;
    for entry in entries
        .into_iter()
        .filter(|entry| entry.key.path.as_str().ends_with(".md"))
    {
        let modified = entry
            .modified_ms
            .map(i64::try_from)
            .transpose()
            .map_err(|_| StoreError::InvalidData)?;
        tx.execute("INSERT INTO pages(connection,path,title,permission,modified_ms,catalogued) VALUES(?1,?2,?3,0,?4,1) ON CONFLICT(connection,path) DO UPDATE SET catalogued=1,remote_deleted=0",params![connection.as_str(),entry.key.path.as_str(),entry.key.title_from(""),modified])?;
    }
    tx.execute(
        "INSERT INTO catalogue_state(connection) VALUES(?1) ON CONFLICT DO NOTHING",
        [connection.as_str()],
    )?;
    tx.commit()?;
    Ok(())
}
fn permission(value: Permission) -> i64 {
    i64::from(value == Permission::ReadWrite)
}
pub(super) fn set_base(
    db: &Connection,
    key: &PageKey,
    base: Option<&RemoteFile>,
) -> Result<(), StoreError> {
    let modified = base
        .and_then(|file| file.modified_ms)
        .map(i64::try_from)
        .transpose()
        .map_err(|_| StoreError::InvalidData)?;
    db.execute("UPDATE pages SET remote_body=?1,remote_revision=?2,remote_modified_ms=?3,permission=?4 WHERE connection=?5 AND path=?6",params![base.map(|file|&file.body),base.and_then(|file|file.revision.as_ref()).map(|rev|&rev.0),modified,permission(base.map_or(Permission::ReadWrite,|file|file.permission)),key.connection.as_str(),key.path.as_str()])?;
    Ok(())
}
fn install(db: &Connection, file: &RemoteFile) -> Result<(), StoreError> {
    let mut position = pages::load(db, &file.key)?
        .map(|page| page.position)
        .unwrap_or_default();
    let clamp = |offset: usize| {
        let mut offset = offset.min(file.body.len());
        while !file.body.is_char_boundary(offset) {
            offset -= 1;
        }
        offset
    };
    position.anchor = clamp(position.anchor);
    position.caret = clamp(position.caret);
    db.execute("INSERT INTO pages(connection,path,title,body,local_revision) VALUES(?1,?2,?3,?4,'0') ON CONFLICT(connection,path) DO UPDATE SET title=excluded.title,body=excluded.body,anchor=?5,caret=?6,scroll_y=?7,remote_deleted=0",params![file.key.connection.as_str(),file.key.path.as_str(),file.key.title_from(&file.body),file.body,i64::try_from(position.anchor).map_err(|_|StoreError::InvalidData)?,i64::try_from(position.caret).map_err(|_|StoreError::InvalidData)?,position.scroll_y])?;
    set_base(db, &file.key, Some(file))?;
    // The scheduler serializes remote observations and mutations for each page.
    // Leased/unknown payloads remain immutable until their outcomes are resolved.
    db.execute(
        "DELETE FROM outbox WHERE page_id=?1 AND state IN ('pending','blocked')",
        [pages::id(db, &file.key)?],
    )?;
    Ok(())
}
pub(super) fn apply_uncached(
    db: &mut Connection,
    remote: RemoteRead,
) -> Result<ReconcileDecision, StoreError> {
    let tx = db.transaction()?;
    let key = match &remote {
        RemoteRead::Present(file) => &file.key,
        RemoteRead::Missing(key) => key,
    };
    // A save or another observation may have filled the cache after the context lookup.
    if pages::load(&tx, key)?.is_some() {
        return Err(StoreError::StaleRevision);
    }
    let result = match remote {
        RemoteRead::Present(file) => {
            install(&tx, &file)?;
            ReconcileDecision::InstallRemote(file)
        }
        RemoteRead::Missing(key) => {
            tx.execute(
                "UPDATE pages SET remote_deleted=1,catalogued=0 WHERE connection=?1 AND path=?2",
                [key.connection.as_str(), key.path.as_str()],
            )?;
            ReconcileDecision::RetainDeletedSnapshot
        }
    };
    tx.commit()?;
    Ok(result)
}
pub(super) fn save_from_base(
    db: &mut Connection,
    key: PageKey,
    body: String,
    revision: LocalRevision,
    expected: EditorBaseline,
) -> Result<crate::domain::reconciliation::LocalSaveOutcome, StoreError> {
    if expected
        .remote_base
        .as_ref()
        .is_some_and(|file| file.key != key)
    {
        return Err(StoreError::InvalidData);
    }
    let tx = db.transaction()?;
    let before = pages::load(&tx, &key)?.ok_or(StoreError::PageMissing)?;
    let deleted = tx.query_row(
        "SELECT remote_deleted FROM pages WHERE connection=?1 AND path=?2",
        [key.connection.as_str(), key.path.as_str()],
        |row| row.get::<_, bool>(0),
    )?;
    let changed = revision != before.local_revision
        && (deleted && expected.remote_base.is_some()
            || before.permission == Permission::ReadOnly
                && expected
                    .remote_base
                    .as_ref()
                    .is_none_or(|base| base.permission == Permission::ReadWrite)
            || expected.body != before.body);
    if changed {
        if before.body != body {
            super::restoration::preserve(&tx, before.clone(), RecoveryReason::LocalVersion)?;
        }
        set_base(&tx, &key, expected.remote_base.as_ref())?;
        if deleted {
            tx.execute(
                "UPDATE pages SET remote_deleted=0 WHERE id=?1",
                [pages::id(&tx, &key)?],
            )?;
        }
    }
    let saved = pages::save_transaction(&tx, key.clone(), body, revision)?;
    let local = pages::load(&tx, &key)?.ok_or(StoreError::PageMissing)?;
    if changed {
        let remote = if deleted {
            RemoteRead::Missing(key.clone())
        } else {
            match before.remote_base {
                Some(file) => RemoteRead::Present(file),
                None => RemoteRead::Missing(key.clone()),
            }
        };
        capture(
            &tx,
            ConflictRecord {
                id: uuid::Uuid::new_v4(),
                key,
                base: expected.remote_base,
                local,
                remote,
            },
        )?;
    } else if let Some(mut conflict) = read_conflict(&tx, &key)? {
        conflict.local = local;
        capture(&tx, conflict)?;
    }
    let stored = pages::load_stored(&tx, &saved.key)?.ok_or(StoreError::PageMissing)?;
    tx.commit()?;
    Ok(crate::domain::reconciliation::LocalSaveOutcome {
        attempt: saved,
        stored,
    })
}

pub(super) fn save_conflict(
    db: &mut Connection,
    mut record: ConflictRecord,
) -> Result<(), StoreError> {
    let tx = db.transaction()?;
    let current = pages::load(&tx, &record.key)?.ok_or(StoreError::PageMissing)?;
    record.local = current;
    capture(&tx, record)?;
    tx.commit()?;
    Ok(())
}
