use crate::{
    app::model::EditorPosition,
    domain::{connection::ConnectionId, error::StoreError, page::*},
};
use rusqlite::{Connection, OptionalExtension, Row};

struct RawPage {
    body: Option<String>,
    revision: String,
    remote_body: Option<String>,
    remote_revision: Option<String>,
    remote_modified: Option<i64>,
    permission: i64,
    anchor: i64,
    caret: i64,
    scroll_y: f32,
}
fn raw(row: &Row<'_>) -> rusqlite::Result<RawPage> {
    Ok(RawPage {
        body: row.get(0)?,
        revision: row.get(1)?,
        remote_body: row.get(2)?,
        remote_revision: row.get(3)?,
        remote_modified: row.get(4)?,
        permission: row.get(5)?,
        anchor: row.get(6)?,
        caret: row.get(7)?,
        scroll_y: row.get(8)?,
    })
}
pub(super) fn load(db: &Connection, key: &PageKey) -> Result<Option<PageSnapshot>, StoreError> {
    let row=db.query_row("SELECT body,local_revision,remote_body,remote_revision,remote_modified_ms,permission,anchor,caret,scroll_y FROM pages WHERE connection=?1 AND path=?2",[key.connection.as_str(),key.path.as_str()],raw).optional()?;
    let Some(row) = row else {
        return Ok(None);
    };
    let Some(body) = row.body else {
        return Ok(None);
    };
    let permission = match row.permission {
        0 => Permission::ReadOnly,
        1 => Permission::ReadWrite,
        _ => return Err(StoreError::InvalidData),
    };
    let position = EditorPosition {
        anchor: row.anchor.try_into().map_err(|_| StoreError::InvalidData)?,
        caret: row.caret.try_into().map_err(|_| StoreError::InvalidData)?,
        scroll_y: row.scroll_y,
    }
    .validate(&body)
    .map_err(|_| StoreError::InvalidData)?;
    let remote_modified = row
        .remote_modified
        .map(u64::try_from)
        .transpose()
        .map_err(|_| StoreError::InvalidData)?;
    let remote_base = row.remote_body.map(|body| RemoteFile {
        key: key.clone(),
        body,
        revision: row.remote_revision.map(RemoteRevision),
        modified_ms: remote_modified,
        permission,
    });
    Ok(Some(PageSnapshot {
        key: key.clone(),
        body,
        local_revision: LocalRevision(row.revision.parse().map_err(|_| StoreError::InvalidData)?),
        remote_base,
        position,
        permission,
    }))
}
pub(super) fn id(db: &Connection, key: &PageKey) -> Result<i64, StoreError> {
    db.query_row(
        "SELECT id FROM pages WHERE connection=?1 AND path=?2",
        [key.connection.as_str(), key.path.as_str()],
        |row| row.get(0),
    )
    .optional()?
    .ok_or(StoreError::PageMissing)
}
pub(super) fn key_for_id(db: &Connection, id: i64) -> Result<PageKey, StoreError> {
    let (connection, path) = db.query_row(
        "SELECT connection,path FROM pages WHERE id=?1",
        [id],
        |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
    )?;
    Ok(PageKey {
        connection: ConnectionId::try_from(connection).map_err(|_| StoreError::InvalidData)?,
        path: PagePath::parse(&path).map_err(|_| StoreError::InvalidData)?,
    })
}
pub(super) fn create(db: &mut Connection, key: PageKey) -> Result<SavedRevision, StoreError> {
    if let Some(existing) = load(db, &key)? {
        return Ok(SavedRevision {
            key,
            local_revision: existing.local_revision,
        });
    }
    let tx = db.transaction()?;
    tx.execute("INSERT INTO pages(connection,path,title,body,local_revision,modified_ms) VALUES(?1,?2,?3,'','0',?4)",rusqlite::params![key.connection.as_str(),key.path.as_str(),key.title_from(""),now_ms()?])?;
    tx.commit()?;
    Ok(SavedRevision {
        key,
        local_revision: LocalRevision(0),
    })
}
pub(super) fn save(
    db: &mut Connection,
    key: PageKey,
    body: String,
    revision: LocalRevision,
) -> Result<SavedRevision, StoreError> {
    let tx = db.transaction()?;
    let saved = save_transaction(&tx, key, body, revision)?;
    tx.commit()?;
    Ok(saved)
}
pub(super) fn save_transaction(
    db: &Connection,
    key: PageKey,
    body: String,
    revision: LocalRevision,
) -> Result<SavedRevision, StoreError> {
    let mut snapshot = load(db, &key)?.ok_or(StoreError::PageMissing)?;
    if snapshot.permission == Permission::ReadOnly {
        return Err(StoreError::ReadOnly);
    }
    if revision <= snapshot.local_revision {
        if revision == snapshot.local_revision && body == snapshot.body {
            return Ok(SavedRevision {
                key,
                local_revision: revision,
            });
        }
        return Err(StoreError::StaleRevision);
    }
    let clamp = |offset: usize| {
        let mut offset = offset.min(body.len());
        while !body.is_char_boundary(offset) {
            offset -= 1;
        }
        offset
    };
    snapshot.position.anchor = clamp(snapshot.position.anchor);
    snapshot.position.caret = clamp(snapshot.position.caret);
    snapshot.body = body;
    snapshot.local_revision = revision;
    let page_id = id(db, &key)?;
    db.execute("UPDATE pages SET body=?1,title=?2,local_revision=?3,anchor=?4,caret=?5,modified_ms=?6 WHERE id=?7",rusqlite::params![snapshot.body,key.title_from(&snapshot.body),revision.0.to_string(),snapshot.position.anchor as i64,snapshot.position.caret as i64,now_ms()?,page_id])?;
    super::outbox::queue_put(db, page_id, &snapshot)?;
    Ok(SavedRevision {
        key,
        local_revision: revision,
    })
}
pub(super) fn save_position(
    db: &mut Connection,
    key: &PageKey,
    position: EditorPosition,
    visited_ms: u64,
) -> Result<(), StoreError> {
    let snapshot = load(db, key)?.ok_or(StoreError::PageMissing)?;
    position
        .validate(&snapshot.body)
        .map_err(|_| StoreError::InvalidData)?;
    let visited_ms = visit_time(db, &key.connection, visited_ms)?;
    db.execute("UPDATE pages SET anchor=?1,caret=?2,scroll_y=?3,last_visited_ms=?4 WHERE connection=?5 AND path=?6",rusqlite::params![position.anchor as i64,position.caret as i64,position.scroll_y,visited_ms,key.connection.as_str(),key.path.as_str()])?;
    Ok(())
}
pub(super) fn record_visit(
    db: &Connection,
    key: &PageKey,
    visited_ms: u64,
) -> Result<(), StoreError> {
    let visited_ms = visit_time(db, &key.connection, visited_ms)?;
    let changed = db.execute(
        "UPDATE pages SET last_visited_ms=?1 WHERE connection=?2 AND path=?3",
        rusqlite::params![visited_ms, key.connection.as_str(), key.path.as_str()],
    )?;
    if changed != 1 {
        return Err(StoreError::PageMissing);
    }
    Ok(())
}
fn now_ms() -> Result<i64, StoreError> {
    let duration = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| StoreError::InvalidData)?;
    i64::try_from(duration.as_millis()).map_err(|_| StoreError::InvalidData)
}

pub(super) fn last_cached(
    db: &Connection,
    connection: &ConnectionId,
) -> Result<Option<PageKey>, StoreError> {
    let path: Option<String> = db.query_row(
        "SELECT path FROM pages WHERE connection=?1 AND body IS NOT NULL ORDER BY last_visited_ms IS NULL,last_visited_ms DESC,path COLLATE BINARY LIMIT 1",
        [connection.as_str()], |row| row.get(0),
    ).optional()?;
    path.map(|path| {
        Ok(PageKey {
            connection: connection.clone(),
            path: PagePath::parse(&path).map_err(|_| StoreError::InvalidData)?,
        })
    })
    .transpose()
}

// Recency is strictly increasing within a space even when the wall clock stalls
// or moves backwards. The worker serializes visits; these values order history.
fn visit_time(
    db: &Connection,
    connection: &ConnectionId,
    requested: u64,
) -> Result<i64, StoreError> {
    let requested = i64::try_from(requested).map_err(|_| StoreError::InvalidData)?;
    let previous: Option<i64> = db.query_row(
        "SELECT max(last_visited_ms) FROM pages WHERE connection=?1",
        [connection.as_str()],
        |row| row.get(0),
    )?;
    match previous {
        Some(previous) if previous >= requested => {
            previous.checked_add(1).ok_or(StoreError::InvalidData)
        }
        _ => Ok(requested),
    }
}
