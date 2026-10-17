//! Migrations back up through SQLite and roll back as one transaction.
use crate::domain::error::StoreError;
use rusqlite::Connection;
use std::{fs::OpenOptions, path::Path};
pub struct Migration<'a> {
    pub version: u32,
    pub sql: &'a str,
}

pub fn migrate(
    db: &mut Connection,
    steps: &[Migration<'_>],
    backup: Option<&Path>,
) -> Result<(), StoreError> {
    let current = db.query_row("PRAGMA user_version", [], |row| row.get::<_, u32>(0))?;
    let target = steps.last().map_or(current, |step| step.version);
    if current > target {
        return Err(StoreError::UnsupportedSchema);
    }
    let pending = steps
        .iter()
        .filter(|step| step.version > current)
        .collect::<Vec<_>>();
    if pending.is_empty() {
        return Ok(());
    }
    if current > 0 {
        let path = backup.ok_or(StoreError::MigrationFailed)?;
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        drop(options.open(path)?);
        // The backup API includes committed WAL content; copying the main file would not.
        db.backup(rusqlite::MAIN_DB, path, None)?;
    }
    let tx = db.transaction()?;
    let mut version = current;
    for step in pending {
        if step.version != version + 1 {
            return Err(StoreError::MigrationFailed);
        }
        tx.execute_batch(step.sql)
            .map_err(|_| StoreError::MigrationFailed)?;
        tx.pragma_update(None, "user_version", step.version)
            .map_err(|_| StoreError::MigrationFailed)?;
        version = step.version;
    }
    tx.commit().map_err(|_| StoreError::MigrationFailed)?;
    Ok(())
}
