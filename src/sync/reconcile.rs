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

/// Prepare source outside the SQLite owner; the commit must recheck the captured context.
pub async fn prepare_reconciliation(
    context: crate::domain::reconciliation::ReconcileContext,
    executor: &super::merge_executor::MergeExecutor,
) -> crate::domain::reconciliation::PreparedResolution {
    use super::text_merge::MergeDisposition;
    use crate::domain::reconciliation::{
        AutomaticRecoveryReason as Reason, PreparedResolution, ResolutionDisposition as Disposition,
    };

    let mut disposition = Disposition::Install;
    let mut reason = None;
    let mut preserve_history = context.token.conflict_id.is_some();
    let body = match &context.remote {
        RemoteRead::Present(file) if context.token.trashed => {
            if same_base(context.base.as_ref(), Some(file)) && !context.token.explicit_create {
                disposition = Disposition::KeepDeletion;
                context.local.body.clone()
            } else {
                preserve_history = true;
                file.body.clone()
            }
        }
        RemoteRead::Present(file) if context.local.body == file.body => file.body.clone(),
        RemoteRead::Present(file) => {
            if let Some(base) = &context.base {
                preserve_history |= base.body != context.local.body && base.body != file.body;
                let merged = executor
                    .merge(
                        base.body.clone(),
                        context.local.body.clone(),
                        file.body.clone(),
                    )
                    .await;
                reason = match merged.disposition {
                    MergeDisposition::Clean => None,
                    MergeDisposition::PreservedOverlap => Some(Reason::Overlap),
                    MergeDisposition::BudgetFallback => Some(Reason::BudgetFallback),
                };
                merged.body
            } else {
                reason = Some(Reason::NoCommonBase);
                file.body.clone()
            }
        }
        RemoteRead::Missing(_) => {
            if context.token.trashed
                || context
                    .base
                    .as_ref()
                    .is_some_and(|base| base.body == context.local.body)
            {
                disposition = Disposition::RetainDeleted;
            }
            preserve_history |= context.base.is_some() && !context.token.remote_deleted;
            context.local.body.clone()
        }
    };
    if context.draft.is_some() {
        reason = reason.or(Some(Reason::AuthoredDraft));
    }
    preserve_history |= reason.is_some();
    PreparedResolution {
        context,
        body,
        disposition,
        recovery_reason: reason,
        preserve_history,
    }
}
