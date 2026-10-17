//! Exact probe identities only; no credentials, note bodies, or wildcard cleanup.
use crate::domain::{
    connection::ConnectionId,
    error::StoreError,
    page::{PageKey, PagePath},
};
use rusqlite::{Connection, params};
pub(super) fn register(db: &Connection, key: &PageKey) -> Result<(), StoreError> {
    db.execute(
        "INSERT INTO probe_journal(connection,path) VALUES(?1,?2)",
        params![key.connection.as_str(), key.path.as_str()],
    )?;
    Ok(())
}
pub(super) fn pending(db: &Connection, id: &ConnectionId) -> Result<Vec<PageKey>, StoreError> {
    let mut statement =
        db.prepare("SELECT path FROM probe_journal WHERE connection=?1 ORDER BY path")?;
    let paths = statement.query_map([id.as_str()], |row| row.get::<_, String>(0))?;
    paths
        .map(|path| {
            Ok(PageKey {
                connection: id.clone(),
                path: PagePath::parse(&path?).map_err(|_| StoreError::InvalidData)?,
            })
        })
        .collect()
}
pub(super) fn forget(db: &Connection, key: &PageKey) -> Result<(), StoreError> {
    db.execute(
        "DELETE FROM probe_journal WHERE connection=?1 AND path=?2",
        params![key.connection.as_str(), key.path.as_str()],
    )?;
    Ok(())
}
