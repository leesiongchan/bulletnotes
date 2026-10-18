//! Compare captured source and work identity before atomically publishing a prepared result.
use super::{StoreHandle, outbox, pages, recovery_uploads, remote, restoration};
use crate::domain::{error::StoreError, page::*, reconciliation::*};
use rusqlite::{Connection, params};

impl StoreHandle {
    /// Commands already admitted to this store are drained before the context is captured.
    /// The editor coordinator must also drain admitted in-memory draft writes before calling.
    pub async fn load_reconcile_context(
        &self,
        key: &PageKey,
        remote: RemoteRead,
    ) -> Result<ReconcileContext, StoreError> {
        let key = key.clone();
        self.call(move |w| load(&w.db, &key, remote)).await
    }

    /// A stale result must be discarded and recomputed from a fresh remote read.
    pub async fn commit_reconciliation(
        &self,
        prepared: PreparedResolution,
    ) -> Result<StoredPage, StoreError> {
        self.call(move |w| commit(&mut w.db, prepared)).await
    }
}

fn load(
    db: &Connection,
    key: &PageKey,
    remote: RemoteRead,
) -> Result<ReconcileContext, StoreError> {
    let remote_key = match &remote {
        RemoteRead::Present(file) => &file.key,
        RemoteRead::Missing(key) => key,
    };
    if remote_key != key {
        return Err(StoreError::InvalidData);
    }
    let stored = pages::load_stored(db, key)?.ok_or(StoreError::PageMissing)?;
    let conflict = remote::read_conflict(db, key)?;
    let draft = restoration::stored_merge_draft(db, key)?;
    let (trashed, remote_deleted, explicit_create) = db.query_row(
        "SELECT trashed,remote_deleted,explicit_create FROM pages WHERE id=?1",
        [pages::id(db, key)?],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    )?;
    let mut query =
        db.prepare("SELECT operation_id,state FROM outbox WHERE page_id=?1 ORDER BY rowid")?;
    let mutations = query
        .query_map([pages::id(db, key)?], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?
        .map(|row| {
            let (id, state) = row?;
            let state = match state.as_str() {
                "pending" => ReconcileMutationState::Pending,
                "blocked" => ReconcileMutationState::Blocked,
                "leased" => ReconcileMutationState::Leased,
                "unknown" => ReconcileMutationState::Unknown,
                _ => return Err(StoreError::InvalidData),
            };
            Ok(ReconcileMutation {
                id: OperationId(uuid::Uuid::parse_str(&id).map_err(|_| StoreError::InvalidData)?),
                state,
            })
        })
        .collect::<Result<Vec<_>, StoreError>>()?;
    Ok(ReconcileContext {
        token: ReconcileToken {
            key: key.clone(),
            source_generation: stored.source_generation,
            expected_local: stored.page.local_revision,
            conflict_id: conflict.as_ref().map(|record| record.id),
            draft_revision: draft.as_ref().map(|draft| draft.revision),
            mutations,
            trashed,
            remote_deleted,
            explicit_create,
        },
        base: conflict.map_or_else(|| stored.page.remote_base.clone(), |record| record.base),
        local: stored.page,
        remote,
        draft,
    })
}

fn commit(db: &mut Connection, prepared: PreparedResolution) -> Result<StoredPage, StoreError> {
    let tx = db.transaction()?;
    let context = &prepared.context;
    let key = &context.token.key;
    let current = load(&tx, key, context.remote.clone()).map_err(|error| match error {
        StoreError::PageMissing => StoreError::StaleRevision,
        error => error,
    })?;
    if current.token != context.token
        || current.local.body != context.local.body
        || current.base != context.base
    {
        return Err(StoreError::StaleRevision);
    }
    if context.token.mutations.iter().any(|op| {
        matches!(
            op.state,
            ReconcileMutationState::Leased | ReconcileMutationState::Unknown
        )
    }) || recovery_uploads::fenced(&tx, key)?
    {
        return Err(StoreError::UnknownOperation);
    }
    let keep_deletion = prepared.disposition == ResolutionDisposition::KeepDeletion;
    if keep_deletion && !prepared.preserve_history {
        return pages::load_stored(&tx, key)?.ok_or(StoreError::PageMissing);
    }
    let base = match &context.remote {
        RemoteRead::Present(file) => Some(file),
        RemoteRead::Missing(_) => None,
    };
    let deleted = prepared.disposition == ResolutionDisposition::RetainDeleted;
    // Preserve operation identity on repeated observations; no new source or work is needed.
    if !prepared.preserve_history
        && context.token.conflict_id.is_none()
        && context.draft.is_none()
        && prepared.body == current.local.body
        && (deleted && current.token.remote_deleted
            || !deleted
                && !current.token.trashed
                && !current.token.remote_deleted
                && crate::sync::reconcile::same_base(current.local.remote_base.as_ref(), base))
    {
        return pages::load_stored(&tx, key)?.ok_or(StoreError::PageMissing);
    }
    if let Some(reason) = prepared.recovery_reason {
        recovery_uploads::register(
            &tx,
            AutomaticRecoveryInput {
                local: context.local.clone(),
                base: context.base.clone(),
                remote: context.remote.clone(),
                draft: context.draft.as_ref().map(|draft| draft.body.clone()),
                kept_body: prepared.body.clone(),
                reason,
            },
        )?;
    }
    if prepared.preserve_history {
        restoration::preserve_conflict(
            &tx,
            &ConflictRecord {
                id: context.token.conflict_id.unwrap_or_else(uuid::Uuid::new_v4),
                key: key.clone(),
                base: context.base.clone(),
                local: context.local.clone(),
                remote: context.remote.clone(),
            },
        )?;
        let mut chosen = context.local.clone();
        chosen.body = prepared.body.clone();
        chosen.position = Default::default();
        restoration::preserve(
            &tx,
            chosen,
            if deleted || base.is_none() {
                RecoveryReason::RemoteDeletion
            } else {
                RecoveryReason::MergedDraft
            },
        )?;
        if let Some(draft) = &context.draft {
            let mut snapshot = context.local.clone();
            snapshot.body = draft.body.clone();
            snapshot.position = Default::default();
            restoration::preserve(&tx, snapshot, RecoveryReason::MergedDraft)?;
        }
    }
    tx.execute(
        "DELETE FROM merge_drafts WHERE connection=?1 AND path=?2",
        [key.connection.as_str(), key.path.as_str()],
    )?;
    tx.execute(
        "DELETE FROM conflicts WHERE connection=?1 AND path=?2",
        [key.connection.as_str(), key.path.as_str()],
    )?;
    tx.execute(
        "DELETE FROM outbox WHERE page_id=?1 AND state IN ('pending','blocked')",
        [pages::id(&tx, key)?],
    )?;
    if deleted {
        tx.execute(
            "UPDATE pages SET remote_deleted=1,catalogued=0 WHERE id=?1",
            [pages::id(&tx, key)?],
        )?;
    } else {
        remote::set_base(&tx, key, base)?;
        let clamp = |offset: usize| {
            let mut offset = offset.min(prepared.body.len());
            while !prepared.body.is_char_boundary(offset) {
                offset -= 1;
            }
            offset
        };
        let explicit_create =
            base.is_none() && (context.token.explicit_create || context.base.is_some());
        tx.execute("UPDATE pages SET body=?1,title=?2,anchor=?3,caret=?4,trashed=?5,remote_deleted=0,explicit_create=?6 WHERE id=?7", params![prepared.body,key.title_from(&prepared.body),clamp(current.local.position.anchor) as i64,clamp(current.local.position.caret) as i64,keep_deletion,explicit_create,pages::id(&tx,key)?])?;
        let snapshot = pages::load(&tx, key)?.ok_or(StoreError::PageMissing)?;
        outbox::queue_put(&tx, pages::id(&tx, key)?, &snapshot)?;
        if snapshot.permission == Permission::ReadOnly {
            tx.execute(
                "UPDATE outbox SET state='blocked' WHERE page_id=?1 AND state='pending'",
                [pages::id(&tx, key)?],
            )?;
        }
    }
    let stored = pages::load_stored(&tx, key)?.ok_or(StoreError::PageMissing)?;
    tx.commit()?;
    Ok(stored)
}
