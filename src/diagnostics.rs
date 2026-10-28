//! Local, typed, non-blocking telemetry. No global subscriber or raw error logging.
//! Questions: did local durability succeed, why is sync paused, and where did time go?
use crate::domain::error::{StoreError, SyncError};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File},
    io::{BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex, Weak},
    time::Instant,
};
use tracing_subscriber::{Layer, filter::filter_fn, layer::SubscriberExt};
#[derive(Debug, thiserror::Error)]
pub enum DiagnosticError {
    #[error("diagnostics are unavailable")]
    Unavailable,
    #[error("diagnostics are already initialized")]
    AlreadyActive,
    #[error("diagnostic data failed validation")]
    InvalidData,
    #[error("diagnostic export exceeds its size limit")]
    TooLarge,
    #[error("diagnostic files could not be accessed")]
    Io(#[from] std::io::Error),
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Operation {
    LocalSave,
    TextMerge,
    AutomaticRecovery,
    EditorReplay,
    Storage,
    CachedSearch,
    ScratchpadLoad,
    ScratchpadNavigation,
    CachedOpen,
    EditorPresentation,
    EditorSourceUpdate,
    EditorFocus,
    SelectionRestoration,
    RemoteCatalogue,
    RemoteRead,
    RemoteWrite,
    RemoteDelete,
    Activation,
    Shutdown,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Success,
    Storage,
    Authentication,
    Permission,
    Conflict,
    Missing,
    Incompatible,
    InvalidResponse,
    Network,
    UnknownMutation,
    ResourceLimit,
    Cancelled,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MergeFallbackReason {
    InputLimit,
    TokenLimit,
    OutputLimit,
    InvalidEncoding,
    WorkerBusy,
    WorkerTimeout,
    WorkerFailure,
}
impl Outcome {
    pub fn from_store_error(_: &StoreError) -> Self {
        Self::Storage
    }
    pub fn from_sync_error(error: &SyncError) -> Self {
        match error {
            SyncError::LocalStorage => Self::Storage,
            SyncError::Authentication => Self::Authentication,
            SyncError::Permission => Self::Permission,
            SyncError::Conflict => Self::Conflict,
            SyncError::Missing => Self::Missing,
            SyncError::Incompatible => Self::Incompatible,
            SyncError::InvalidResponse => Self::InvalidResponse,
            SyncError::Network => Self::Network,
            SyncError::UnknownMutationOutcome => Self::UnknownMutation,
            SyncError::ResponseTooLarge => Self::ResourceLimit,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DesktopCapability {
    GlobalShortcut,
    Tray,
}

/// Allowlist by construction: no string, path, error, URL, or clipboard fields.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case", deny_unknown_fields)]
pub enum DiagnosticEvent {
    DesktopCapabilityUnavailable {
        capability: DesktopCapability,
    },
    MergeFallback {
        id: uuid::Uuid,
        reason: MergeFallbackReason,
    },
    Finished {
        id: uuid::Uuid,
        operation: Operation,
        outcome: Outcome,
        elapsed_micros: u64,
    },
    Queue {
        id: uuid::Uuid,
        pending: u64,
        conflicts: u64,
    },
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    utc_ms: i64,
    event: DiagnosticEvent,
}
struct Sink {
    dispatch: tracing::Dispatch,
    dropped: tracing_appender::non_blocking::ErrorCounter,
}
struct Active {
    directory: PathBuf,
    sink: Weak<Sink>,
}
static ACTIVE: Mutex<Option<Active>> = Mutex::new(None);
type Maintenance = (std::sync::mpsc::Sender<()>, std::thread::JoinHandle<()>);
pub struct DiagnosticGuard {
    maintenance: Option<Maintenance>,
    sink: Arc<Sink>,
    _worker: tracing_appender::non_blocking::WorkerGuard,
}
impl DiagnosticGuard {
    pub fn dropped_events(&self) -> usize {
        self.sink.dropped.dropped_lines()
    }
}
fn day(name: &str) -> Option<i64> {
    let value = name.strip_prefix("bulletnotes.")?.strip_suffix(".log")?;
    if value.len() != 10 {
        return None;
    }
    let timestamp = format!("{value}T00:00:00Z")
        .parse::<jiff::Timestamp>()
        .ok()?;
    Some(timestamp.as_second().div_euclid(86400))
}
fn today() -> i64 {
    jiff::Timestamp::now().as_second().div_euclid(86400)
}
fn prune(directory: &Path, current_day: i64) -> Result<(), DiagnosticError> {
    let cutoff = current_day - 6;
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        if entry.file_type()?.is_file()
            && entry
                .file_name()
                .to_str()
                .and_then(day)
                .is_some_and(|day| day < cutoff)
        {
            fs::remove_file(entry.path())?;
        }
    }
    Ok(())
}
impl Drop for DiagnosticGuard {
    fn drop(&mut self) {
        if let Some((stop, worker)) = self.maintenance.take() {
            let _ = stop.send(());
            let _ = worker.join();
        }
    }
}
pub fn init(log_dir: &Path) -> Result<DiagnosticGuard, DiagnosticError> {
    let mut active = ACTIVE.lock().map_err(|_| DiagnosticError::Unavailable)?;
    if active
        .as_ref()
        .is_some_and(|active| active.sink.strong_count() > 0)
    {
        return Err(DiagnosticError::AlreadyActive);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(log_dir)?;
        fs::set_permissions(log_dir, fs::Permissions::from_mode(0o700))?;
    }
    #[cfg(not(unix))]
    fs::create_dir_all(log_dir)?;
    let directory = log_dir.canonicalize()?;
    prune(&directory, today())?;
    let appender = tracing_appender::rolling::RollingFileAppender::builder()
        .rotation(tracing_appender::rolling::Rotation::DAILY)
        .filename_prefix("bulletnotes")
        .filename_suffix("log")
        .max_log_files(7)
        .build(&directory)
        .map_err(|_| DiagnosticError::Unavailable)?;
    let (writer, worker) = tracing_appender::non_blocking::NonBlockingBuilder::default()
        .buffered_lines_limit(1024)
        .lossy(true)
        .thread_name("bulletnotes-diagnostics")
        .finish(appender);
    let dropped = writer.error_counter();
    let subscriber = tracing_subscriber::registry().with(
        tracing_subscriber::fmt::layer()
            .json()
            .with_writer(writer)
            .with_ansi(false)
            .with_target(true)
            .with_filter(filter_fn(|metadata| {
                metadata.target() == "bulletnotes::diagnostics"
            })),
    );
    let sink = Arc::new(Sink {
        dispatch: tracing::Dispatch::new(subscriber),
        dropped,
    });
    *active = Some(Active {
        directory,
        sink: Arc::downgrade(&sink),
    });
    let (stop, receive) = std::sync::mpsc::channel();
    let directory = log_dir.canonicalize()?;
    let maintenance = std::thread::Builder::new()
        .name("bulletnotes-log-retention".into())
        .spawn(move || {
            loop {
                let now = jiff::Timestamp::now().as_second();
                let until = std::time::Duration::from_secs((86400 - now.rem_euclid(86400)) as u64);
                match receive.recv_timeout(until) {
                    Ok(()) | Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                        if prune(&directory, today()).is_err() {
                            record(DiagnosticEvent::Finished {
                                id: uuid::Uuid::new_v4(),
                                operation: Operation::Storage,
                                outcome: Outcome::Storage,
                                elapsed_micros: 0,
                            });
                        }
                    }
                }
            }
        })?;
    Ok(DiagnosticGuard {
        maintenance: Some((stop, maintenance)),
        sink,
        _worker: worker,
    })
}
pub fn record(event: DiagnosticEvent) {
    let sink = ACTIVE
        .lock()
        .ok()
        .and_then(|active| active.as_ref().and_then(|active| active.sink.upgrade()));
    if let Some(sink) = sink
        && let Ok(record) = serde_json::to_string(&Record {
            utc_ms: jiff::Timestamp::now().as_millisecond(),
            event,
        })
    {
        tracing::dispatcher::with_default(
            &sink.dispatch,
            || tracing::info!(target:"bulletnotes::diagnostics",record=%record),
        );
    }
}
/// Export complete typed records already on disk. In-flight diagnostic events may
/// not yet have reached disk. Local note durability is independent of this queue.
/// Atomic replacement preserves an existing destination on validation/I/O failure.
pub fn export(destination: &Path) -> Result<(), DiagnosticError> {
    let directory = ACTIVE
        .lock()
        .map_err(|_| DiagnosticError::Unavailable)?
        .as_ref()
        .map(|active| active.directory.clone())
        .ok_or(DiagnosticError::Unavailable)?;
    let parent = destination
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let parent = parent.canonicalize()?;
    if parent == directory
        && destination
            .file_name()
            .and_then(|name| name.to_str())
            .and_then(day)
            .is_some()
    {
        return Err(DiagnosticError::InvalidData);
    }
    let mut files = Vec::new();
    for entry in fs::read_dir(&directory)? {
        let entry = entry?;
        if entry
            .file_name()
            .to_str()
            .and_then(day)
            .is_some_and(|day| day >= today() - 6)
        {
            if !entry.file_type()?.is_file() {
                return Err(DiagnosticError::InvalidData);
            }
            files.push(entry.path());
        }
    }
    files.sort();
    let mut output = tempfile::NamedTempFile::new_in(&parent)?;
    let mut remaining = 64 * 1024 * 1024;
    for path in files {
        let file = File::open(path)?;
        let length =
            usize::try_from(file.metadata()?.len()).map_err(|_| DiagnosticError::TooLarge)?;
        if length > remaining {
            return Err(DiagnosticError::TooLarge);
        }
        remaining -= length;
        let mut reader = BufReader::new(file.take(length as u64));
        let mut line = String::new();
        while reader.read_line(&mut line)? > 0 {
            // Snapshot may end inside a concurrent final write. Never export fragments.
            if !line.ends_with('\n') {
                break;
            }
            let wrapper: serde_json::Value =
                serde_json::from_str(&line).map_err(|_| DiagnosticError::InvalidData)?;
            if let Some(encoded) = wrapper
                .get("fields")
                .and_then(|v| v.get("record"))
                .and_then(|v| v.as_str())
            {
                let record: Record =
                    serde_json::from_str(encoded).map_err(|_| DiagnosticError::InvalidData)?;
                serde_json::to_writer(&mut output, &record)
                    .map_err(|_| DiagnosticError::InvalidData)?;
                output.write_all(b"\n")?;
            }
            line.clear();
        }
    }
    output.as_file().sync_all()?;
    output
        .persist(destination)
        .map_err(|error| DiagnosticError::Io(error.error))?;
    // The atomic rename is the commit point. A later directory sync failure must
    // not report export failure after replacing an existing destination successfully.
    let _ = File::open(parent).and_then(|file| file.sync_all());
    Ok(())
}
/// Cancellation is a classified outcome as well; spans carry no application data.
pub struct OperationTimer {
    id: uuid::Uuid,
    operation: Operation,
    started: Instant,
    finished: bool,
}
impl OperationTimer {
    pub fn start(operation: Operation) -> Self {
        Self::with_id(operation, uuid::Uuid::new_v4())
    }
    pub fn with_id(operation: Operation, id: uuid::Uuid) -> Self {
        Self {
            id,
            operation,
            started: Instant::now(),
            finished: false,
        }
    }
    pub fn success(self) {
        self.complete(Outcome::Success);
    }
    pub fn complete(mut self, outcome: Outcome) {
        self.finish(outcome);
    }
    fn finish(&mut self, outcome: Outcome) {
        self.finished = true;
        record(DiagnosticEvent::Finished {
            id: self.id,
            operation: self.operation,
            outcome,
            elapsed_micros: u64::try_from(self.started.elapsed().as_micros()).unwrap_or(u64::MAX),
        });
    }
    pub fn store<T>(mut self, result: Result<T, StoreError>) -> Result<T, StoreError> {
        self.finish(
            result
                .as_ref()
                .err()
                .map_or(Outcome::Success, Outcome::from_store_error),
        );
        result
    }
    pub fn sync<T>(mut self, result: Result<T, SyncError>) -> Result<T, SyncError> {
        self.finish(
            result
                .as_ref()
                .err()
                .map_or(Outcome::Success, Outcome::from_sync_error),
        );
        result
    }
}
impl Drop for OperationTimer {
    fn drop(&mut self) {
        if !self.finished {
            self.finish(Outcome::Cancelled);
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn aged_logs_expire_even_when_no_new_log_is_rotated() {
        let dir = tempfile::tempdir().unwrap();
        let current = jiff::Timestamp::now().to_string()[..10].to_owned();
        let log = dir.path().join(format!("bulletnotes.{current}.log"));
        std::fs::write(&log, b"generated log").unwrap();
        let other = dir.path().join("unrelated.md");
        std::fs::write(&other, b"preserve").unwrap();
        super::prune(dir.path(), super::today() + 8).unwrap();
        assert!(!log.exists());
        assert!(other.exists());
    }
}
