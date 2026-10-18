//! Admission and lifetime ownership for blocking text reconciliation.
use std::{sync::Arc, time::Duration};

use tokio::sync::Semaphore;

use super::text_merge::{MergeDisposition, TextMergeResult, fallback, merge_text, whole_edit};
use crate::diagnostics::{self, DiagnosticEvent, MergeFallbackReason};

const WAIT_LIMIT: Duration = Duration::from_millis(150);

#[derive(Clone)]
pub struct MergeExecutor {
    slots: Arc<Semaphore>,
}

impl Default for MergeExecutor {
    fn default() -> Self {
        Self {
            slots: Arc::new(Semaphore::new(1)),
        }
    }
}

impl MergeExecutor {
    pub async fn merge(&self, base: String, local: String, remote: String) -> TextMergeResult {
        self.merge_with(base, local, remote, merge_text).await
    }

    async fn merge_with(
        &self,
        base: String,
        local: String,
        remote: String,
        worker: impl FnOnce(&str, &str, &str) -> TextMergeResult + Send + 'static,
    ) -> TextMergeResult {
        if local == remote || base == remote {
            return TextMergeResult {
                body: local,
                disposition: MergeDisposition::Clean,
                edits: Vec::new(),
            };
        }
        let remote_only = base == local;
        let without_worker = |reason| {
            if remote_only {
                diagnostics::record(DiagnosticEvent::MergeFallback {
                    id: uuid::Uuid::new_v4(),
                    reason,
                });
                TextMergeResult {
                    body: remote.clone(),
                    disposition: MergeDisposition::Clean,
                    edits: whole_edit(&local, &remote),
                }
            } else {
                fallback(&local, &remote, reason)
            }
        };
        let Ok(permit) = self.slots.clone().try_acquire_owned() else {
            return without_worker(MergeFallbackReason::WorkerBusy);
        };
        let worker_local = local.clone();
        let worker_remote = remote.clone();
        let job = tokio::task::spawn_blocking(move || {
            // A dropped JoinHandle cannot stop CPU work; its permit lives until the closure exits.
            let _permit = permit;
            worker(&base, &worker_local, &worker_remote)
        });
        match tokio::time::timeout(WAIT_LIMIT, job).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => without_worker(MergeFallbackReason::WorkerFailure),
            Err(_) => without_worker(MergeFallbackReason::WorkerTimeout),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        domain::{
            connection::ConnectionConfig,
            page::{LocalRevision, PageKey, PagePath},
        },
        store::StoreHandle,
        sync::text_merge::{MergeDisposition, SourceEdit},
    };
    use std::{
        sync::{
            atomic::{AtomicBool, Ordering},
            mpsc,
        },
        time::Duration,
    };

    #[tokio::test]
    async fn timeout_retains_capacity_rejects_late_results_and_allows_unrelated_saves() {
        let executor = MergeExecutor::default();
        let directory = tempfile::tempdir().unwrap();
        let store = StoreHandle::open(&directory.path().join("test.sqlite"))
            .await
            .unwrap();
        let key = PageKey {
            connection: ConnectionConfig::parse("https://notes.example/")
                .unwrap()
                .id()
                .clone(),
            path: PagePath::parse("independent.md").unwrap(),
        };
        store.create_draft(key.clone()).await.unwrap();
        let (started, ready) = tokio::sync::oneshot::channel();
        let (release, blocked) = mpsc::channel();
        let finished = Arc::new(AtomicBool::new(false));
        let worker_finished = finished.clone();
        let running = executor.clone();
        let job = tokio::spawn(async move {
            running
                .merge_with(
                    "base".into(),
                    "local".into(),
                    "remote".into(),
                    move |_, _, _| {
                        started.send(()).unwrap();
                        blocked.recv_timeout(Duration::from_secs(3)).unwrap();
                        worker_finished.store(true, Ordering::SeqCst);
                        TextMergeResult {
                            body: "late worker output".into(),
                            disposition: MergeDisposition::Clean,
                            edits: Vec::new(),
                        }
                    },
                )
                .await
        });
        ready.await.unwrap();
        tokio::time::timeout(
            Duration::from_secs(1),
            store.save_local(key.clone(), "durable during merge".into(), LocalRevision(1)),
        )
        .await
        .unwrap()
        .unwrap();
        assert!(!finished.load(Ordering::SeqCst));
        let timed_out = tokio::time::timeout(Duration::from_secs(1), job)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(timed_out.body, "remote");
        assert_eq!(timed_out.disposition, MergeDisposition::BudgetFallback);
        assert_eq!(
            timed_out.edits,
            vec![SourceEdit {
                range: 0..5,
                replacement: "remote".into()
            }]
        );
        assert_eq!(executor.slots.available_permits(), 0);
        let clone = executor.clone();
        let busy = tokio::time::timeout(
            Duration::from_millis(100),
            clone.merge(
                "red fox jumps".into(),
                "blue fox jumps".into(),
                "red fox runs".into(),
            ),
        )
        .await
        .unwrap();
        assert_eq!(busy.body, "red fox runs");
        assert_eq!(busy.disposition, MergeDisposition::BudgetFallback);
        let local_only = clone
            .merge(
                "base".into(),
                "unambiguous local edit".into(),
                "base".into(),
            )
            .await;
        assert_eq!(local_only.body, "unambiguous local edit");
        assert_eq!(local_only.disposition, MergeDisposition::Clean);
        assert!(local_only.edits.is_empty());
        let remote_only = clone
            .merge(
                "base".into(),
                "base".into(),
                "unambiguous remote edit".into(),
            )
            .await;
        assert_eq!(remote_only.body, "unambiguous remote edit");
        assert_eq!(remote_only.disposition, MergeDisposition::Clean);
        release.send(()).unwrap();
        tokio::time::timeout(Duration::from_secs(1), async {
            while executor.slots.available_permits() == 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        assert!(finished.load(Ordering::SeqCst));
        assert_eq!(timed_out.body, "remote");
        assert_eq!(
            store.load_page(&key).await.unwrap().unwrap().body,
            "durable during merge"
        );
        let next = executor
            .merge(
                "red fox jumps".into(),
                "blue fox jumps".into(),
                "red fox runs".into(),
            )
            .await;
        assert_eq!(next.body, "blue fox runs");
        assert_eq!(next.disposition, MergeDisposition::Clean);
        store.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn worker_failure_preserves_remote_and_releases_capacity() {
        let executor = MergeExecutor::default();
        let logs = tempfile::tempdir().unwrap();
        let guard = diagnostics::init(logs.path()).unwrap();
        let result = executor
            .merge_with(
                "base".into(),
                "LOCAL_SOURCE_SENTINEL".into(),
                "REMOTE_SOURCE_SENTINEL".into(),
                |_, _, _| panic!("injected worker failure"),
            )
            .await;
        assert_eq!(result.body, "REMOTE_SOURCE_SENTINEL");
        assert_eq!(result.disposition, MergeDisposition::BudgetFallback);
        assert_eq!(executor.slots.available_permits(), 1);
        drop(guard);
        let destination = logs.path().join("export.jsonl");
        diagnostics::export(&destination).unwrap();
        let exported = std::fs::read_to_string(destination).unwrap();
        assert!(exported.contains("worker_failure"));
        assert!(!exported.contains("SOURCE_SENTINEL"));
    }
}
