//! Recovery copies and deletion intent commit in the same SQLite transaction.
use super::{outbox, pages, remote};
use crate::{
    domain::editor::EditorPosition,
    domain::{connection::ConnectionId, error::StoreError, page::*},
};
use rusqlite::{Connection, OptionalExtension, params};

pub(super) fn validate_page(page: &PageSnapshot) -> Result<(), StoreError> {
    if page.position.validate(&page.body).is_err()
        || page
            .remote_base
            .as_ref()
            .is_some_and(|base| base.key != page.key)
    {
        return Err(StoreError::InvalidData);
    }
    Ok(())
}
pub(super) fn preserve(
    db: &Connection,
    page: PageSnapshot,
    reason: RecoveryReason,
) -> Result<(), StoreError> {
    validate_page(&page)?;
    let copy = RecoveryCopy {
        id: uuid::Uuid::new_v4(),
        page,
        reason,
    };
    let value = serde_json::to_string(&copy).map_err(|_| StoreError::InvalidData)?;
    let label = match reason {
        RecoveryReason::CommonBase => "Common base",
        RecoveryReason::LocalVersion => "Local version",
        RecoveryReason::RemoteVersion => "Remote version",
        RecoveryReason::RemoteDeletion => "Remote deletion",
        RecoveryReason::MergedDraft => "Merged draft",
    };
    db.execute(
        "INSERT INTO recovery(id,connection,path,snapshot,created_ms,label) VALUES(?1,?2,?3,?4,?5,?6)",
        params![
            copy.id.to_string(),
            copy.page.key.connection.as_str(),
            copy.page.key.path.as_str(),
            value,
            pages::now_ms()?, label
        ],
    )?;
    Ok(())
}
pub(super) fn preserve_conflict(
    db: &Connection,
    record: &ConflictRecord,
) -> Result<(), StoreError> {
    preserve(db, record.local.clone(), RecoveryReason::LocalVersion)?;
    if let Some(base) = &record.base {
        preserve(db, snapshot(base), RecoveryReason::CommonBase)?;
    }
    if let RemoteRead::Present(file) = &record.remote {
        preserve(db, snapshot(file), RecoveryReason::RemoteVersion)?;
    }
    Ok(())
}
fn snapshot(file: &RemoteFile) -> PageSnapshot {
    PageSnapshot {
        key: file.key.clone(),
        body: file.body.clone(),
        local_revision: LocalRevision(0),
        remote_base: Some(file.clone()),
        position: EditorPosition::default(),
        permission: file.permission,
    }
}
pub(super) fn copies(
    db: &Connection,
    connection: &ConnectionId,
) -> Result<Vec<RecoveryCopy>, StoreError> {
    let mut query=db.prepare("SELECT id,path,snapshot FROM recovery WHERE connection=?1 ORDER BY created_ms DESC,rowid DESC")?;
    query
        .query_map([connection.as_str()], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?
        .map(|row| {
            let (id, path, value) = row?;
            let copy: RecoveryCopy =
                serde_json::from_str(&value).map_err(|_| StoreError::InvalidData)?;
            validate_page(&copy.page)?;
            if copy.id.to_string() != id
                || copy.page.key.connection != *connection
                || copy.page.key.path.as_str() != path
            {
                return Err(StoreError::InvalidData);
            }
            Ok(copy)
        })
        .collect()
}
pub(super) fn resolve(
    db: &mut Connection,
    request: ConflictResolution,
) -> Result<Option<SavedRevision>, StoreError> {
    let tx = db.transaction()?;
    let (connection, path) = tx
        .query_row(
            "SELECT connection,path FROM conflicts WHERE id=?1",
            [request.conflict_id.to_string()],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()?
        .ok_or(StoreError::StaleRevision)?;
    let key = PageKey {
        connection: ConnectionId::try_from(connection).map_err(|_| StoreError::InvalidData)?,
        path: PagePath::parse(&path).map_err(|_| StoreError::InvalidData)?,
    };
    let record = remote::read_conflict(&tx, &key)?.ok_or(StoreError::StaleRevision)?;
    let current = pages::load(&tx, &key)?.ok_or(StoreError::PageMissing)?;
    if current.local_revision != request.expected_local {
        return Err(StoreError::StaleRevision);
    }
    let leased: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM outbox WHERE page_id=?1 AND state='leased')",
        [pages::id(&tx, &key)?],
        |row| row.get(0),
    )?;
    if leased {
        return Err(StoreError::UnknownOperation);
    }
    preserve_conflict(&tx, &record)?;
    if let Some(body) = merge_draft(&tx, &key)? {
        let mut draft = current.clone();
        draft.body = body;
        draft.position = EditorPosition::default();
        preserve(&tx, draft, RecoveryReason::MergedDraft)?;
        tx.execute(
            "DELETE FROM merge_drafts WHERE connection=?1 AND path=?2",
            [key.connection.as_str(), key.path.as_str()],
        )?;
    }
    tx.execute(
        "DELETE FROM outbox WHERE page_id=?1",
        [pages::id(&tx, &key)?],
    )?;
    tx.execute(
        "DELETE FROM conflicts WHERE id=?1",
        [request.conflict_id.to_string()],
    )?;
    let next = current
        .local_revision
        .next()
        .map_err(|_| StoreError::InvalidData)?;
    let result = match (request.choice, &record.remote) {
        (ResolutionChoice::AcceptRemote, RemoteRead::Missing(_)) => {
            tx.execute("DELETE FROM pages WHERE id=?1", [pages::id(&tx, &key)?])?;
            None
        }
        (choice, observation) => {
            let accepting = matches!(choice, ResolutionChoice::AcceptRemote);
            let body = match choice {
                ResolutionChoice::AcceptRemote => match observation {
                    RemoteRead::Present(file) => file.body.clone(),
                    RemoteRead::Missing(_) => return Err(StoreError::InvalidData),
                },
                ResolutionChoice::KeepLocal => current.body,
                ResolutionChoice::Merge(body) => body,
            };
            let base = match observation {
                RemoteRead::Present(file) => Some(file),
                RemoteRead::Missing(_) => None,
            };
            if !accepting
                && base.is_some_and(|file| {
                    file.permission != Permission::ReadWrite || file.revision.is_none()
                })
            {
                return Err(StoreError::ReadOnly);
            }
            remote::set_base(&tx, &key, base)?;
            tx.execute("UPDATE pages SET body=?1,title=?2,local_revision=?3,anchor=0,caret=0,scroll_y=0,trashed=0,remote_deleted=0,explicit_create=?4 WHERE id=?5",params![body,key.title_from(&body),next.0.to_string(),i64::from(base.is_none()),pages::id(&tx,&key)?])?;
            if !accepting {
                let saved = pages::load(&tx, &key)?.ok_or(StoreError::PageMissing)?;
                outbox::queue_put(&tx, pages::id(&tx, &key)?, &saved)?;
            }
            Some(SavedRevision {
                key,
                local_revision: next,
            })
        }
    };
    tx.commit()?;
    Ok(result)
}
pub(super) fn move_to_trash(
    db: &mut Connection,
    key: PageKey,
    expected: LocalRevision,
) -> Result<TrashId, StoreError> {
    let tx = db.transaction()?;
    let page = pages::load(&tx, &key)?.ok_or(StoreError::PageMissing)?;
    if page.local_revision != expected {
        return Err(StoreError::StaleRevision);
    }
    if page.permission != Permission::ReadWrite {
        return Err(StoreError::ReadOnly);
    }
    let page_id = pages::id(&tx, &key)?;
    let already: bool =
        tx.query_row("SELECT trashed FROM pages WHERE id=?1", [page_id], |row| {
            row.get(0)
        })?;
    if already || remote::read_conflict(&tx, &key)?.is_some() {
        return Err(StoreError::UnknownOperation);
    }
    let pending = {
        let mut query =
            tx.prepare("SELECT operation_id FROM outbox WHERE page_id=?1 ORDER BY rowid")?;
        let ids = query
            .query_map([page_id], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        ids.iter()
            .map(|id| outbox::read_operation(&tx, id))
            .collect::<Result<Vec<_>, _>>()?
    };
    let record = TrashRecord {
        id: TrashId::new(),
        page,
        pending,
    };
    let value = serde_json::to_string(&record).map_err(|_| StoreError::InvalidData)?;
    tx.execute(
        "INSERT INTO trash(id,connection,path,snapshot,created_ms) VALUES(?1,?2,?3,?4,?5)",
        params![
            record.id.0.to_string(),
            key.connection.as_str(),
            key.path.as_str(),
            value,
            pages::now_ms()?
        ],
    )?;
    tx.execute("UPDATE pages SET trashed=1 WHERE id=?1", [page_id])?;
    outbox::queue_put(&tx, page_id, &record.page)?;
    tx.commit()?;
    Ok(record.id)
}
pub(super) fn trash(
    db: &Connection,
    connection: &ConnectionId,
) -> Result<Vec<TrashRecord>, StoreError> {
    let mut query=db.prepare("SELECT id,path,snapshot FROM trash WHERE connection=?1 ORDER BY created_ms DESC,rowid DESC")?;
    query
        .query_map([connection.as_str()], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?
        .map(|row| {
            let (id, path, value) = row?;
            let record: TrashRecord =
                serde_json::from_str(&value).map_err(|_| StoreError::InvalidData)?;
            validate_page(&record.page)?;
            if record.id.0.to_string() != id
                || record.page.key.connection != *connection
                || record.page.key.path.as_str() != path
                || record.pending.iter().any(|op| op.key != record.page.key)
            {
                return Err(StoreError::InvalidData);
            }
            Ok(record)
        })
        .collect()
}
pub(super) fn restore_page(
    db: &Connection,
    page: &PageSnapshot,
    destination: Option<PagePath>,
) -> Result<PageKey, StoreError> {
    validate_page(page)?;
    let key = PageKey {
        connection: page.key.connection.clone(),
        path: destination.unwrap_or_else(|| page.key.path.clone()),
    };
    let exists: bool = db.query_row(
        "SELECT EXISTS(SELECT 1 FROM pages WHERE connection=?1 AND path=?2)",
        [key.connection.as_str(), key.path.as_str()],
        |row| row.get(0),
    )?;
    if exists {
        let reusable:bool=db.query_row("SELECT remote_deleted=1 AND NOT EXISTS(SELECT 1 FROM outbox WHERE page_id=p.id) AND NOT EXISTS(SELECT 1 FROM conflicts WHERE connection=p.connection AND path=p.path) FROM pages p WHERE connection=?1 AND path=?2",[key.connection.as_str(),key.path.as_str()],|row|row.get(0))?;
        if !reusable {
            return Err(StoreError::PathCollision);
        }
        db.execute("DELETE FROM pages WHERE id=?1", [pages::id(db, &key)?])?;
    }
    if !key.path.as_str().ends_with(".md")
        || key
            .path
            .as_str()
            .split('/')
            .any(|segment| segment.starts_with('.'))
    {
        return Err(StoreError::InvalidData);
    }
    db.execute("INSERT INTO pages(connection,path,title,body,local_revision,explicit_create,modified_ms) VALUES(?1,?2,?3,?4,'1',1,?5)",params![key.connection.as_str(),key.path.as_str(),key.title_from(&page.body),page.body,pages::now_ms()?])?;
    let restored = pages::load(db, &key)?.ok_or(StoreError::PageMissing)?;
    outbox::queue_put(db, pages::id(db, &key)?, &restored)?;
    Ok(key)
}
pub(super) fn restore_trash(
    db: &mut Connection,
    id: TrashId,
    destination: Option<PagePath>,
) -> Result<PageKey, StoreError> {
    let tx = db.transaction()?;
    let connection: String = tx
        .query_row(
            "SELECT connection FROM trash WHERE id=?1",
            [id.0.to_string()],
            |row| row.get(0),
        )
        .optional()?
        .ok_or(StoreError::PageMissing)?;
    let connection = ConnectionId::try_from(connection).map_err(|_| StoreError::InvalidData)?;
    let RecoveryItem::Trash(record) = item(&tx, &connection, RecoveryKind::Trash, id.0)? else {
        return Err(StoreError::InvalidData);
    };
    let key = restore_page(&tx, &record.page, destination)?;
    tx.commit()?;
    Ok(key)
}
pub(super) fn restore_copy(
    db: &mut Connection,
    id: uuid::Uuid,
    destination: Option<PagePath>,
) -> Result<PageKey, StoreError> {
    let tx = db.transaction()?;
    let connection: String = tx
        .query_row(
            "SELECT connection FROM recovery WHERE id=?1",
            [id.to_string()],
            |row| row.get(0),
        )
        .optional()?
        .ok_or(StoreError::PageMissing)?;
    let connection = ConnectionId::try_from(connection).map_err(|_| StoreError::InvalidData)?;
    let RecoveryItem::Copy(record) = item(&tx, &connection, RecoveryKind::Copy, id)? else {
        return Err(StoreError::InvalidData);
    };
    let key = restore_page(&tx, &record.page, destination)?;
    tx.commit()?;
    Ok(key)
}
pub(super) fn empty_trash(db: &mut Connection, ids: Vec<TrashId>) -> Result<(), StoreError> {
    let tx = db.transaction()?;
    for id in ids {
        tx.execute("DELETE FROM trash WHERE id=?1", [id.0.to_string()])?;
    }
    tx.commit()?;
    Ok(())
}

pub(super) fn index(
    db: &Connection,
    connection: &ConnectionId,
) -> Result<Vec<RecoverySummary>, StoreError> {
    let mut rows = Vec::new();
    for (table, kind) in [
        ("conflicts", RecoveryKind::Conflict),
        ("trash", RecoveryKind::Trash),
        ("recovery", RecoveryKind::Copy),
    ] {
        let fields = match kind {
            RecoveryKind::Conflict => "id,path,'Changes',0",
            RecoveryKind::Trash => "id,path,'Trash',created_ms",
            RecoveryKind::Copy => "id,path,label,created_ms",
        };
        let mut query = db.prepare(&format!(
            "SELECT {fields} FROM {table} WHERE connection=?1 ORDER BY rowid DESC"
        ))?;
        for row in query.query_map([connection.as_str()], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
            ))
        })? {
            let (id, path, mut label, created) = row?;
            if created != 0 {
                let date = jiff::Timestamp::from_millisecond(created)
                    .map_err(|_| StoreError::InvalidData)?
                    .strftime("%Y-%m-%d %H:%M UTC")
                    .to_string();
                label.push_str(&format!(" · {date}"));
            }
            rows.push(RecoverySummary {
                label,
                id: uuid::Uuid::parse_str(&id).map_err(|_| StoreError::InvalidData)?,
                key: PageKey {
                    connection: connection.clone(),
                    path: PagePath::parse(&path).map_err(|_| StoreError::InvalidData)?,
                },
                kind,
            });
        }
    }
    Ok(rows)
}
pub(super) fn item(
    db: &Connection,
    connection: &ConnectionId,
    kind: RecoveryKind,
    id: uuid::Uuid,
) -> Result<RecoveryItem, StoreError> {
    let table = match kind {
        RecoveryKind::Conflict => "conflicts",
        RecoveryKind::Trash => "trash",
        RecoveryKind::Copy => "recovery",
    };
    let (path, value) = db
        .query_row(
            &format!("SELECT path,snapshot FROM {table} WHERE connection=?1 AND id=?2"),
            params![connection.as_str(), id.to_string()],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()?
        .ok_or(StoreError::StaleRevision)?;
    let key = PageKey {
        connection: connection.clone(),
        path: PagePath::parse(&path).map_err(|_| StoreError::InvalidData)?,
    };
    match kind {
        RecoveryKind::Conflict => Ok(RecoveryItem::Conflict(Box::new(
            remote::read_conflict(db, &key)?.ok_or(StoreError::StaleRevision)?,
        ))),
        RecoveryKind::Trash => {
            let record: TrashRecord =
                serde_json::from_str(&value).map_err(|_| StoreError::InvalidData)?;
            validate_page(&record.page)?;
            if record.id.0 != id
                || record.page.key != key
                || record.pending.iter().any(|op| op.key != key)
            {
                return Err(StoreError::InvalidData);
            }
            Ok(RecoveryItem::Trash(Box::new(record)))
        }
        RecoveryKind::Copy => {
            let record: RecoveryCopy =
                serde_json::from_str(&value).map_err(|_| StoreError::InvalidData)?;
            validate_page(&record.page)?;
            if record.id != id || record.page.key != key {
                return Err(StoreError::InvalidData);
            }
            Ok(RecoveryItem::Copy(Box::new(record)))
        }
    }
}

pub(super) fn merge_draft(db: &Connection, key: &PageKey) -> Result<Option<String>, StoreError> {
    Ok(db
        .query_row(
            "SELECT body FROM merge_drafts WHERE connection=?1 AND path=?2",
            [key.connection.as_str(), key.path.as_str()],
            |row| row.get(0),
        )
        .optional()?)
}
pub(super) fn save_merge_drafts(
    db: &mut Connection,
    drafts: Vec<(PageKey, String)>,
) -> Result<(), StoreError> {
    let tx = db.transaction()?;
    for (key, body) in drafts {
        if remote::read_conflict(&tx, &key)?.is_none() {
            return Err(StoreError::StaleRevision);
        }
        tx.execute("INSERT INTO merge_drafts(connection,path,body) VALUES(?1,?2,?3) ON CONFLICT(connection,path) DO UPDATE SET body=excluded.body",params![key.connection.as_str(),key.path.as_str(),body])?;
    }
    tx.commit()?;
    Ok(())
}
