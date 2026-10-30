//! Connection-scoped scratchpad order survives page-row replacement.
use crate::domain::{
    connection::ConnectionId,
    error::StoreError,
    page::{PageKey, PagePath},
};
use jiff::tz::TimeZone;
use rusqlite::{Connection, params};

pub(super) fn move_selected(
    db: &mut Connection,
    connection: &ConnectionId,
    expected: Vec<PageKey>,
    selected: Vec<PageKey>,
    before: Option<PageKey>,
) -> Result<Vec<ScratchpadEntry>, StoreError> {
    use std::collections::HashSet;
    let tx = db.transaction()?;
    let entries = list(&tx, connection)?;
    let current: Vec<_> = entries.iter().map(|e| e.key.clone()).collect();
    let selection_len = selected.len();
    let selected: HashSet<_> = selected.iter().collect();
    if current != expected {
        return Err(StoreError::StaleRevision);
    }
    if selected.len() != selection_len
        || selected.is_empty()
        || selected
            .iter()
            .any(|key| key.connection != *connection || !current.contains(key))
        || before
            .as_ref()
            .is_some_and(|key| selected.contains(key) || !current.contains(key))
    {
        return Err(StoreError::InvalidData);
    }
    let mut rest: Vec<_> = entries
        .iter()
        .filter(|e| !selected.contains(&e.key))
        .cloned()
        .collect();
    let target = before
        .as_ref()
        .and_then(|key| rest.iter().position(|e| &e.key == key))
        .unwrap_or(rest.len());
    rest.splice(
        target..target,
        entries
            .iter()
            .filter(|e| selected.contains(&e.key))
            .cloned(),
    );
    for (index, entry) in rest.iter().enumerate() {
        let rank = i64::try_from(rest.len() - index).map_err(|_| StoreError::InvalidData)?;
        tx.execute(
            "UPDATE scratchpad_order SET order_ms=?1 WHERE connection=?2 AND path=?3",
            params![rank, connection.as_str(), entry.key.path.as_str()],
        )?;
    }
    tx.commit()?;
    tracing::debug!(count = selected.len(), "Scratchpad order saved");
    Ok(rest)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScratchpadEntry {
    pub key: PageKey,
    pub created_ms: i64,
}
fn member(path: &PagePath) -> bool {
    path.as_str().starts_with("Quick Notes/") && path.as_str().ends_with(".md")
}
fn generated_ms(path: &PagePath, zone: TimeZone) -> Option<i64> {
    let rest = path.as_str().strip_prefix("Quick Notes/")?;
    let (date, file) = rest.split_once('/')?;
    let stem = file.strip_suffix(".md")?;
    let (time, id) = stem.split_once('-')?;
    if time.len() != 6
        || !time.bytes().all(|b| b.is_ascii_digit())
        || uuid::Uuid::parse_str(id).is_err()
    {
        return None;
    }
    let civil = jiff::civil::DateTime::strptime("%F %H%M%S", format!("{date} {time}")).ok()?;
    Some(civil.to_zoned(zone).ok()?.timestamp().as_millisecond())
}
fn admit_at(
    db: &Connection,
    key: &PageKey,
    modified: Option<u64>,
    local_new: bool,
    now: i64,
    zone: TimeZone,
) -> Result<(), StoreError> {
    if !member(&key.path) {
        return Ok(());
    }
    let exists: bool = db.query_row(
        "SELECT EXISTS(SELECT 1 FROM scratchpad_order WHERE connection=?1 AND path=?2)",
        [key.connection.as_str(), key.path.as_str()],
        |r| r.get(0),
    )?;
    if exists {
        return Ok(());
    }
    let modified = modified
        .map(i64::try_from)
        .transpose()
        .map_err(|_| StoreError::InvalidData)?;
    let created = if local_new {
        now
    } else {
        generated_ms(&key.path, zone).or(modified).unwrap_or(now)
    };
    let order = if local_new {
        let newest: Option<i64> = db.query_row(
            "SELECT max(order_ms) FROM scratchpad_order WHERE connection=?1",
            [key.connection.as_str()],
            |r| r.get(0),
        )?;
        newest
            .map(|value| value.checked_add(1).ok_or(StoreError::InvalidData))
            .transpose()?
            .map_or(created, |value| value.max(created))
    } else {
        created
    };
    db.execute(
        "INSERT INTO scratchpad_order(connection,path,created_ms,order_ms) VALUES(?1,?2,?3,?4)",
        params![key.connection.as_str(), key.path.as_str(), created, order],
    )?;
    Ok(())
}
pub(super) fn admit(
    db: &Connection,
    key: &PageKey,
    modified_ms: Option<u64>,
    local_new: bool,
) -> Result<(), StoreError> {
    admit_at(
        db,
        key,
        modified_ms,
        local_new,
        super::pages::now_ms()?,
        TimeZone::system(),
    )
}
pub(super) fn carry_order(
    db: &Connection,
    source: &PageKey,
    destination: &PageKey,
) -> Result<(), StoreError> {
    if !member(&destination.path) {
        return Ok(());
    }
    db.execute("INSERT INTO scratchpad_order(connection,path,created_ms,order_ms) SELECT ?1,?2,created_ms,order_ms FROM scratchpad_order WHERE connection=?3 AND path=?4 ON CONFLICT(connection,path) DO UPDATE SET created_ms=excluded.created_ms,order_ms=excluded.order_ms",params![destination.connection.as_str(),destination.path.as_str(),source.connection.as_str(),source.path.as_str()])?;
    Ok(())
}
pub(super) fn backfill(db: &mut Connection) -> Result<(), StoreError> {
    let tx = db.transaction()?;
    let rows = {
        let mut q=tx.prepare("SELECT p.connection,p.path,p.modified_ms FROM pages p LEFT JOIN scratchpad_order s ON s.connection=p.connection AND s.path=p.path WHERE s.path IS NULL AND substr(p.path,1,12)='Quick Notes/' AND substr(p.path,-3)='.md'")?;
        q.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<i64>>(2)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?
    };
    for (connection, path, modified) in rows {
        let key = PageKey {
            connection: ConnectionId::try_from(connection).map_err(|_| StoreError::InvalidData)?,
            path: PagePath::parse(&path).map_err(|_| StoreError::InvalidData)?,
        };
        admit(
            &tx,
            &key,
            modified
                .map(u64::try_from)
                .transpose()
                .map_err(|_| StoreError::InvalidData)?,
            false,
        )?;
    }
    tx.commit()?;
    Ok(())
}
pub(super) fn list(
    db: &Connection,
    connection: &ConnectionId,
) -> Result<Vec<ScratchpadEntry>, StoreError> {
    let mut q=db.prepare("SELECT s.path,s.created_ms FROM scratchpad_order s JOIN pages p ON p.connection=s.connection AND p.path=s.path WHERE s.connection=?1 AND p.trashed=0 AND p.remote_deleted=0 ORDER BY s.order_ms DESC,s.path COLLATE BINARY ASC")?;
    q.query_map([connection.as_str()], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
    })?
    .map(|row| {
        let (path, created_ms) = row?;
        Ok(ScratchpadEntry {
            key: PageKey {
                connection: connection.clone(),
                path: PagePath::parse(&path).map_err(|_| StoreError::InvalidData)?,
            },
            created_ms,
        })
    })
    .collect()
}
#[cfg(test)]
mod tests {
    use super::*;
    fn db() -> Connection {
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch(include_str!("../../migrations/008_scratchpad_stack.sql"))
            .unwrap();
        db
    }
    fn key(path: &str) -> PageKey {
        PageKey {
            connection: crate::domain::connection::ConnectionConfig::parse("https://stack.example")
                .unwrap()
                .id()
                .clone(),
            path: PagePath::parse(path).unwrap(),
        }
    }
    #[test]
    fn clock_rollback_still_inserts_local_note_first() {
        let db = db();
        let a = key("Quick Notes/a.md");
        let b = key("Quick Notes/b.md");
        admit_at(&db, &a, None, true, 100, TimeZone::UTC).unwrap();
        admit_at(&db, &b, None, true, 10, TimeZone::UTC).unwrap();
        assert_eq!(
            db.query_row(
                "SELECT order_ms FROM scratchpad_order WHERE path=?1",
                [b.path.as_str()],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
            101
        );
    }
    #[test]
    fn daylight_saving_and_later_timezone_changes_do_not_reparse_admitted_dates() {
        let db = db();
        for path in [
            "Quick Notes/2024-03-10/023000-00000000-0000-4000-8000-000000000001.md",
            "Quick Notes/2024-11-03/013000-00000000-0000-4000-8000-000000000002.md",
        ] {
            let key = key(path);
            let expected =
                generated_ms(&key.path, TimeZone::get("America/New_York").unwrap()).unwrap();
            admit_at(
                &db,
                &key,
                None,
                false,
                0,
                TimeZone::get("America/New_York").unwrap(),
            )
            .unwrap();
            admit_at(&db, &key, None, false, 99, TimeZone::UTC).unwrap();
            assert_eq!(
                db.query_row(
                    "SELECT created_ms FROM scratchpad_order WHERE path=?1",
                    [path],
                    |r| r.get::<_, i64>(0)
                )
                .unwrap(),
                expected
            );
        }
    }
}
