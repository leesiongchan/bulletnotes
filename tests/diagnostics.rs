use bulletnotes::diagnostics::{self, DiagnosticEvent, Operation, Outcome};
use std::{path::Path, sync::Mutex};
static SERIAL: Mutex<()> = Mutex::new(());
fn date(days_ago: i64) -> String {
    jiff::Timestamp::from_second(jiff::Timestamp::now().as_second() - days_ago * 86400)
        .unwrap()
        .to_string()[..10]
        .to_owned()
}
fn log(dir: &Path, days_ago: i64) -> std::path::PathBuf {
    dir.join(format!("bulletnotes.{}.log", date(days_ago)))
}
#[test]
fn typed_diagnostics_exclude_sentinel_payloads_and_preserve_sources_on_export_failure() {
    let _serial = SERIAL.lock().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let guard = diagnostics::init(dir.path()).unwrap();
    tracing::warn!(target:"reqwest",note="NOTE_BODY_SENTINEL",token="TOKEN_SENTINEL",path="PATH_SENTINEL");
    let failure = bulletnotes::domain::error::StoreError::Io(std::io::Error::other(
        "TOKEN_SENTINEL NOTE_BODY_SENTINEL PATH_SENTINEL",
    ));
    diagnostics::record(DiagnosticEvent::Finished {
        id: uuid::Uuid::new_v4(),
        operation: Operation::LocalSave,
        outcome: Outcome::from_store_error(&failure),
        elapsed_micros: 12,
    });
    diagnostics::record(DiagnosticEvent::Queue {
        id: uuid::Uuid::new_v4(),
        pending: 2,
        conflicts: 1,
    });
    drop(guard);
    let destination = dir.path().join("review.jsonl");
    diagnostics::export(&destination).unwrap();
    let exported = std::fs::read_to_string(&destination).unwrap();
    assert!(exported.contains("local_save"));
    assert!(exported.contains("storage"));
    for sentinel in ["TOKEN_SENTINEL", "NOTE_BODY_SENTINEL", "PATH_SENTINEL"] {
        assert!(!exported.contains(sentinel));
    }
    let failed_destination = dir.path().join("existing-directory");
    std::fs::create_dir(&failed_destination).unwrap();
    std::fs::write(failed_destination.join("preserved"), b"existing data").unwrap();
    assert!(diagnostics::export(&failed_destination).is_err());
    assert_eq!(
        std::fs::read(failed_destination.join("preserved")).unwrap(),
        b"existing data"
    );
    let own_log = log(dir.path(), 0);
    let contents = std::fs::read(&own_log).unwrap();
    std::fs::write(
        log(dir.path(), 1),
        b"{\"fields\":{\"record\":\"TOKEN_SENTINEL NOTE_BODY_SENTINEL PATH_SENTINEL\"}}\n",
    )
    .unwrap();
    assert!(diagnostics::export(&destination).is_err());
    assert_eq!(std::fs::read_to_string(&destination).unwrap(), exported);
    assert_eq!(std::fs::read(own_log).unwrap(), contents);
}
#[test]
fn retention_is_seven_utc_days_and_unrelated_files_are_preserved() {
    let _serial = SERIAL.lock().unwrap();
    let dir = tempfile::tempdir().unwrap();
    for ago in 1..=10 {
        std::fs::write(log(dir.path(), ago), b"{}\n").unwrap();
    }
    let unrelated = dir.path().join("personal.md");
    std::fs::write(&unrelated, b"NOTE_BODY_SENTINEL").unwrap();
    let guard = diagnostics::init(dir.path()).unwrap();
    drop(guard);
    for ago in 1..=6 {
        assert!(log(dir.path(), ago).exists());
    }
    for ago in 7..=10 {
        assert!(!log(dir.path(), ago).exists());
    }
    assert_eq!(std::fs::read(unrelated).unwrap(), b"NOTE_BODY_SENTINEL");
}
#[test]
fn export_rejects_a_log_as_its_destination_without_overwriting_it() {
    let _serial = SERIAL.lock().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let guard = diagnostics::init(dir.path()).unwrap();
    diagnostics::record(DiagnosticEvent::Queue {
        id: uuid::Uuid::new_v4(),
        pending: 0,
        conflicts: 0,
    });
    drop(guard);
    let own_log = log(dir.path(), 0);
    let before = std::fs::read(&own_log).unwrap();
    assert!(diagnostics::export(&own_log).is_err());
    assert_eq!(std::fs::read(own_log).unwrap(), before);
}
