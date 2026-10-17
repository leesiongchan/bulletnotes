//! Source and opaque revisions drive reconciliation, never modification timestamps.
use crate::domain::page::*;
pub fn same_base(a: Option<&RemoteFile>, b: Option<&RemoteFile>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => {
            a.key == b.key
                && a.body == b.body
                && a.revision == b.revision
                && a.permission == b.permission
        }
        _ => false,
    }
}
pub fn reconcile(local: &PageSnapshot, remote: &RemoteRead) -> ReconcileDecision {
    let remote_key = match remote {
        RemoteRead::Present(file) => &file.key,
        RemoteRead::Missing(key) => key,
    };
    if &local.key != remote_key {
        return ReconcileDecision::Unchanged;
    }
    match remote {
        RemoteRead::Missing(_) if local.remote_base.is_none() => {
            if local.body.is_empty() {
                ReconcileDecision::Unchanged
            } else {
                ReconcileDecision::KeepPending
            }
        }
        RemoteRead::Missing(_)
            if local
                .remote_base
                .as_ref()
                .is_some_and(|base| base.body == local.body) =>
        {
            ReconcileDecision::RetainDeletedSnapshot
        }
        RemoteRead::Present(file) if local.body == file.body => {
            if same_base(local.remote_base.as_ref(), Some(file)) {
                ReconcileDecision::Unchanged
            } else {
                ReconcileDecision::InstallRemote(file.clone())
            }
        }
        RemoteRead::Present(file)
            if local
                .remote_base
                .as_ref()
                .is_some_and(|base| base.body == local.body) =>
        {
            ReconcileDecision::InstallRemote(file.clone())
        }
        RemoteRead::Present(file)
            if local
                .remote_base
                .as_ref()
                .is_some_and(|base| base.body == file.body) =>
        {
            ReconcileDecision::KeepPending
        }
        _ => ReconcileDecision::Conflict(Box::new(ConflictRecord {
            id: uuid::Uuid::new_v4(),
            key: local.key.clone(),
            base: local.remote_base.clone(),
            local: local.clone(),
            remote: remote.clone(),
        })),
    }
}
