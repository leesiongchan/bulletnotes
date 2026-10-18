//! Durable original editor branch and checked local-source merge, independent of HTTP observations.
use super::{StoreHandle, delete_watches, outbox, pages, recovery_uploads, restoration};
use crate::domain::{error::StoreError, page::*, reconciliation::*};
use crate::sync::text_merge::{MergeDisposition, SourceEdit};
use rusqlite::{Connection, OptionalExtension, params};

pub(super) fn handoff(db: &Connection, key: &PageKey) -> Result<Option<EditorHandoff>, StoreError> {
    let value: Option<String> = db
        .query_row(
            "SELECT snapshot FROM editor_handoffs WHERE connection=?1 AND path=?2",
            [key.connection.as_str(), key.path.as_str()],
            |row| row.get(0),
        )
        .optional()?;
    value
        .map(|value| {
            let value: EditorHandoff =
                serde_json::from_str(&value).map_err(|_| StoreError::InvalidData)?;
            restoration::validate_page(&value.input)?;
            restoration::validate_page(&value.displaced.page)?;
            if value.input.key != *key || value.displaced.page.key != *key {
                return Err(StoreError::InvalidData);
            }
            Ok(value)
        })
        .transpose()
}

/// This transaction admits authored source without waiting for merge, network or UI admission.
pub(super) fn admit(
    db: &Connection,
    before: StoredPage,
    body: String,
    revision: LocalRevision,
    baseline: EditorBaseline,
) -> Result<LocalSaveOutcome, StoreError> {
    let key = &before.page.key;
    let trashed: bool = db.query_row(
        "SELECT trashed FROM pages WHERE connection=?1 AND path=?2",
        [key.connection.as_str(), key.path.as_str()],
        |row| row.get(0),
    )?;
    if trashed {
        return Err(StoreError::PageMissing);
    }
    let previous = handoff(db, key)?;
    if revision <= before.page.local_revision {
        if previous.as_ref().is_some_and(|previous| {
            previous.input.local_revision == revision && previous.input.body == body
        }) {
            return Ok(LocalSaveOutcome {
                attempt: SavedRevision {
                    key: key.clone(),
                    local_revision: revision,
                },
                stored: before,
            });
        }
        return Err(StoreError::StaleRevision);
    }
    let mut input = before.page.clone();
    input.body = body;
    input.local_revision = revision;
    input.position = Default::default();
    input.remote_base = baseline.remote_base;
    let branch = if let Some(mut previous) = previous {
        restoration::preserve(db, previous.input, RecoveryReason::LocalVersion)?;
        previous.input = input;
        previous.id = uuid::Uuid::new_v4();
        previous
    } else {
        EditorHandoff {
            id: uuid::Uuid::new_v4(),
            baseline: baseline.body,
            input,
            displaced: before.clone(),
        }
    };
    // Exact authored input and displaced canonical source survive even if replay never runs.
    restoration::preserve(db, branch.input.clone(), RecoveryReason::LocalVersion)?;
    restoration::preserve(db, before.page.clone(), RecoveryReason::LocalVersion)?;
    let value = serde_json::to_string(&branch).map_err(|_| StoreError::InvalidData)?;
    db.execute("INSERT INTO editor_handoffs(connection,path,snapshot) VALUES(?1,?2,?3) ON CONFLICT(connection,path) DO UPDATE SET snapshot=excluded.snapshot",params![key.connection.as_str(),key.path.as_str(),value])?;
    db.execute(
        "DELETE FROM outbox WHERE page_id=?1 AND state IN ('pending','blocked')",
        [pages::id(db, key)?],
    )?;
    db.execute(
        "UPDATE pages SET local_revision=?1,modified_ms=?2 WHERE connection=?3 AND path=?4",
        params![
            revision.0.to_string(),
            pages::now_ms()?,
            key.connection.as_str(),
            key.path.as_str()
        ],
    )?;
    Ok(LocalSaveOutcome {
        attempt: SavedRevision {
            key: key.clone(),
            local_revision: revision,
        },
        stored: pages::load_stored(db, key)?.ok_or(StoreError::PageMissing)?,
    })
}

pub(super) fn restore(db: &Connection, stored: StoredPage) -> Result<EditorRestore, StoreError> {
    let authored = handoff(db, &stored.page.key)?.map(|branch| branch.input);
    Ok(EditorRestore { stored, authored })
}

pub(super) fn load_editor(
    db: &Connection,
    key: &PageKey,
) -> Result<Option<EditorRestore>, StoreError> {
    let Some(stored) = pages::load_stored(db, key)? else {
        return Ok(None);
    };
    let (trashed, deleted): (bool, bool) = db.query_row(
        "SELECT trashed,remote_deleted FROM pages WHERE connection=?1 AND path=?2",
        [key.connection.as_str(), key.path.as_str()],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    let restored = restore(db, stored)?;
    if trashed || deleted && restored.authored.is_none() {
        return Err(StoreError::PageMissing);
    }
    Ok(Some(restored))
}

struct Context {
    stored: StoredPage,
    token: ReconcileToken,
    branch: Option<EditorHandoff>,
    watch: Vec<String>,
}
fn context(db: &Connection, key: &PageKey) -> Result<Context, StoreError> {
    let (stored, token, _, _) = super::reconciliation::load_inputs(db, key)?;
    Ok(Context {
        stored,
        token,
        branch: handoff(db, key)?,
        watch: delete_watches::authority(db, key)?,
    })
}
impl StoreHandle {
    /// Replay the latest accepted original branch before observing or publishing canonical source.
    pub async fn replay_editor_handoff(
        &self,
        key: &PageKey,
    ) -> Result<Option<StoredPage>, StoreError> {
        let timer =
            crate::diagnostics::OperationTimer::start(crate::diagnostics::Operation::EditorReplay);
        let result = async {
            let key = key.clone();
            let captured = self.call(move |w| context(&w.db, &key)).await?;
            let Some(branch) = &captured.branch else {
                return Ok(None);
            };
            let result = self
                .merge_executor
                .merge(
                    branch.baseline.clone(),
                    branch.input.body.clone(),
                    captured.stored.page.body.clone(),
                )
                .await;
            let input = branch.input.clone();
            let baseline = branch.baseline.clone();
            let stored = self
                .call(move |w| commit(&mut w.db, captured, baseline, input, result))
                .await?;
            self.local_changes.send_replace(());
            Ok(Some(stored))
        }
        .await;
        timer.store(result)
    }
    /// The UI admits/acknowledges authored source before calling; this path installs durable source only.
    pub async fn rebase_editor(
        &self,
        baseline: String,
        input: PageSnapshot,
        expected: StoredPage,
    ) -> Result<(StoredPage, Vec<SourceEdit>), StoreError> {
        let key = input.key.clone();
        let captured = self.call(move |w| context(&w.db, &key)).await?;
        if captured.branch.is_some()
            || captured.stored.source_generation != expected.source_generation
            || captured.stored.page.body != expected.page.body
            || input.local_revision != captured.stored.page.local_revision
        {
            return Err(StoreError::StaleRevision);
        }
        let result = self
            .merge_executor
            .merge(
                baseline.clone(),
                input.body.clone(),
                captured.stored.page.body.clone(),
            )
            .await;
        let edits = result.edits.clone();
        // A clean displayed buffer normally selects the already durable canonical snapshot.
        let stored = if result.body == captured.stored.page.body {
            let key = input.key;
            let current = self.call(move |w| context(&w.db, &key)).await?;
            if current.token != captured.token {
                return Err(StoreError::StaleRevision);
            }
            current.stored
        } else {
            self.call(move |w| commit(&mut w.db, captured, baseline, input, result))
                .await?
        };
        Ok((stored, edits))
    }
}
fn commit(
    db: &mut Connection,
    captured: Context,
    baseline: String,
    input: PageSnapshot,
    result: crate::sync::text_merge::TextMergeResult,
) -> Result<StoredPage, StoreError> {
    let timer = (result.disposition != MergeDisposition::Clean).then(|| {
        crate::diagnostics::OperationTimer::start(crate::diagnostics::Operation::AutomaticRecovery)
    });
    let result = commit_transaction(db, captured, baseline, input, result);
    match timer {
        Some(timer) => timer.store(result),
        None => result,
    }
}
fn commit_transaction(
    db: &mut Connection,
    captured: Context,
    baseline: String,
    input: PageSnapshot,
    result: crate::sync::text_merge::TextMergeResult,
) -> Result<StoredPage, StoreError> {
    let tx = db.transaction()?;
    let key = &captured.token.key;
    let current = context(&tx, key)?;
    if current.token != captured.token
        || current.branch != captured.branch
        || current.stored.page.body != captured.stored.page.body
        || current.stored.page.remote_base != captured.stored.page.remote_base
        || current.watch != captured.watch
    {
        return Err(StoreError::StaleRevision);
    }
    if current.token.trashed || current.token.remote_deleted && captured.branch.is_none() {
        return Err(StoreError::PageMissing);
    }
    if current.token.mutations.iter().any(|operation| {
        matches!(
            operation.state,
            ReconcileMutationState::Leased | ReconcileMutationState::Unknown
        )
    }) || recovery_uploads::fenced(&tx, key)?
    {
        return Err(StoreError::UnknownOperation);
    }
    if input.local_revision != current.stored.page.local_revision {
        return Err(StoreError::StaleRevision);
    }
    let mut presented_base = input.clone();
    presented_base.body = baseline.clone();
    presented_base.position = Default::default();
    restoration::preserve(&tx, presented_base, RecoveryReason::CommonBase)?;
    restoration::preserve(&tx, input.clone(), RecoveryReason::LocalVersion)?;
    restoration::preserve(
        &tx,
        current.stored.page.clone(),
        RecoveryReason::LocalVersion,
    )?;
    if let Some(branch) = &captured.branch {
        restoration::preserve(
            &tx,
            branch.displaced.page.clone(),
            RecoveryReason::LocalVersion,
        )?;
    }
    if result.disposition != MergeDisposition::Clean {
        recovery_uploads::register(
            &tx,
            AutomaticRecoveryInput {
                local: input.clone(),
                base: input.remote_base.clone(),
                remote: None,
                editor: Some(EditorRecoveryProvenance {
                    remote_missing: current.token.remote_deleted,
                    baseline,
                    durable: captured.stored.clone(),
                }),
                draft: None,
                kept_body: result.body.clone(),
                reason: if result.disposition == MergeDisposition::BudgetFallback {
                    AutomaticRecoveryReason::BudgetFallback
                } else {
                    AutomaticRecoveryReason::Overlap
                },
            },
        )?;
    }
    tx.execute(
        "DELETE FROM editor_handoffs WHERE connection=?1 AND path=?2",
        [key.connection.as_str(), key.path.as_str()],
    )?;
    let mut chosen = current.stored.page.clone();
    if current.token.remote_deleted {
        super::remote::set_base(&tx, key, None)?;
        chosen.remote_base = None;
        chosen.permission = Permission::ReadWrite;
    }

    chosen.body = result.body;
    let clamp = |offset: usize| {
        let mut offset = offset.min(chosen.body.len());
        while !chosen.body.is_char_boundary(offset) {
            offset -= 1;
        }
        offset
    };
    chosen.position.anchor = clamp(chosen.position.anchor);
    chosen.position.caret = clamp(chosen.position.caret);
    restoration::preserve(&tx, chosen.clone(), RecoveryReason::MergedDraft)?;
    tx.execute(
        "UPDATE pages SET body=?1,title=?2,anchor=?3,caret=?4,remote_deleted=0,explicit_create=?7 WHERE connection=?5 AND path=?6",
        params![
            chosen.body,
            key.title_from(&chosen.body),
            chosen.position.anchor as i64,
            chosen.position.caret as i64,
            key.connection.as_str(),
            key.path.as_str(),
            current.token.explicit_create || current.token.remote_deleted
        ],
    )?;
    outbox::queue_put(&tx, pages::id(&tx, key)?, &chosen)?;
    let stored = pages::load_stored(&tx, key)?.ok_or(StoreError::PageMissing)?;
    delete_watches::protect(&tx, &stored.page)?;
    tx.commit()?;
    Ok(stored)
}
