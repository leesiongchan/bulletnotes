use super::recovery::{Migration, migrate};
use crate::domain::error::StoreError;
use rusqlite::Connection;
use std::{
    fs::{File, OpenOptions, TryLockError},
    io::{Read, Write},
    path::{Path, PathBuf},
    time::Duration,
};
use tokio::sync::{mpsc, oneshot};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Durability {
    pub journal_mode: String,
    pub synchronous: i64,
    pub foreign_keys: bool,
    pub fullfsync: bool,
}
pub(super) struct Worker {
    pub db: Connection,
    _guard: InstanceGuard,
    pub durability: Durability,
}
struct InstanceGuard {
    _file: File,
}
pub(super) type Job = Box<dyn FnOnce(&mut Worker) + Send>;
pub(super) enum Command {
    Run(Job),
    Shutdown(oneshot::Sender<()>),
}

impl Worker {
    pub fn open(path: &Path) -> Result<Self, StoreError> {
        let path = absolute_path(path)?;
        let guard = acquire_instance_lock(&path)?;
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        drop(options.open(&path)?);
        let mut db = Connection::open(&path)?;
        db.busy_timeout(Duration::from_secs(5))?;
        let integrity = db.query_row("PRAGMA quick_check", [], |row| row.get::<_, String>(0))?;
        if integrity != "ok" {
            return Err(StoreError::CorruptDatabase);
        }
        db.pragma_update(None, "foreign_keys", true)?;
        let journal_mode =
            db.query_row("PRAGMA journal_mode=WAL", [], |row| row.get::<_, String>(0))?;
        db.pragma_update(None, "synchronous", "FULL")?;
        #[cfg(target_os = "macos")]
        db.pragma_update(None, "fullfsync", true)?;
        let durability = Durability {
            journal_mode,
            synchronous: db.query_row("PRAGMA synchronous", [], |row| row.get(0))?,
            foreign_keys: db.query_row("PRAGMA foreign_keys", [], |row| row.get(0))?,
            fullfsync: db.query_row("PRAGMA fullfsync", [], |row| row.get(0))?,
        };
        if durability.journal_mode != "wal"
            || durability.synchronous != 2
            || !durability.foreign_keys
            || (cfg!(target_os = "macos") && !durability.fullfsync)
        {
            return Err(StoreError::Durability);
        }
        let backup = path.with_extension(format!("pre-migration-{}.sqlite", uuid::Uuid::new_v4()));
        migrate(
            &mut db,
            &[
                Migration {
                    version: 1,
                    sql: include_str!("../../migrations/001_initial.sql"),
                },
                Migration {
                    version: 2,
                    sql: include_str!("../../migrations/002_probe_journal.sql"),
                },
                Migration {
                    version: 3,
                    sql: include_str!("../../migrations/003_remote_cache.sql"),
                },
                Migration {
                    version: 4,
                    sql: include_str!("../../migrations/004_recovery_intents.sql"),
                },
                Migration {
                    version: 5,
                    sql: include_str!("../../migrations/005_automatic_reconciliation.sql"),
                },
                Migration {
                    version: 6,
                    sql: include_str!("../../migrations/006_retired_delete_watches.sql"),
                },
                Migration {
                    version: 7,
                    sql: include_str!("../../migrations/007_editor_handoffs.sql"),
                },
                Migration {
                    version: 8,
                    sql: include_str!("../../migrations/008_scratchpad_stack.sql"),
                },
            ],
            Some(&backup),
        )?;
        super::scratchpads::backfill(&mut db)?;
        db.execute("UPDATE outbox SET state='unknown' WHERE state='leased'", [])?;
        db.execute(
            "UPDATE recovery_uploads SET state='unknown' WHERE state='leased'",
            [],
        )?;
        Ok(Self {
            db,
            _guard: guard,
            durability,
        })
    }
    pub fn run(mut self, mut receiver: mpsc::Receiver<Command>) {
        let mut shutdown = None;
        while let Some(command) = receiver.blocking_recv() {
            match command {
                Command::Run(job) => job(&mut self),
                Command::Shutdown(reply) => {
                    shutdown = Some(reply);
                    break;
                }
            }
        }
        drop(self); // Close SQLite and release the process lock before confirming shutdown.
        if let Some(reply) = shutdown {
            let _ = reply.send(());
        }
    }
}
fn absolute_path(path: &Path) -> Result<PathBuf, StoreError> {
    let path = if path.is_absolute() {
        path.to_owned()
    } else {
        std::env::current_dir()?.join(path)
    };
    let parent = path.parent().ok_or(StoreError::InvalidData)?;
    std::fs::create_dir_all(parent)?;
    if path.exists() {
        return Ok(path.canonicalize()?);
    }
    Ok(parent
        .canonicalize()?
        .join(path.file_name().ok_or(StoreError::InvalidData)?))
}
fn acquire_instance_lock(path: &Path) -> Result<InstanceGuard, StoreError> {
    let mut name = path.as_os_str().to_owned();
    name.push(".lock");
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(PathBuf::from(name))?;
    match file.try_lock() {
        Ok(()) => {
            file.set_len(0)?;
            write!(file, "{}", std::process::id())?;
            file.sync_data()?;
            Ok(InstanceGuard { _file: file })
        }
        Err(TryLockError::WouldBlock) => Err(StoreError::AlreadyRunning),
        Err(TryLockError::Error(error)) => Err(StoreError::Io(error)),
    }
}

/// Read only after AlreadyRunning. The kernel lock remains the writer authority;
/// this metadata is a hint for activating a verified running native owner.
pub fn owner_process_id(path: &Path) -> Result<u32, StoreError> {
    let path = absolute_path(path)?;
    let mut lock = path.as_os_str().to_owned();
    lock.push(".lock");
    let mut text = String::new();
    File::open(PathBuf::from(lock))?
        .take(11)
        .read_to_string(&mut text)?;
    if text.len() > 10 {
        return Err(StoreError::InvalidData);
    }
    let pid = text.parse::<u32>().map_err(|_| StoreError::InvalidData)?;
    if pid == 0 || pid > i32::MAX as u32 {
        return Err(StoreError::InvalidData);
    }
    Ok(pid)
}
