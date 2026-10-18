# Automatic Sync Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reconcile native, browser and external Markdown edits automatically while preserving incompatible source as synced recovery pages.

**Architecture:** Use Gitoxide's maintained merge engine outside the SQLite owner and UI threads, then apply its decision through a checked transaction. Persist immutable recovery uploads separately from editable page intents. Rebase an open editor through explicit source/input generations and its existing ordered save boundary.

**Tech Stack:** Rust 1.92+, Tokio, SQLite/rusqlite, reqwest, Slint; pinned gix-merge 0.22.0, gix-imara-diff 0.3.1, unicode-segmentation 1.13.3, sha2 0.11.1.

**Spec:** [Automatic sync reconciliation](../specs/2026-10-18-automatic-sync-design.md).
**Library evidence:** [Merge library evaluation](../../validation/merge-library-research.md).

## Global Constraints

- Ordinary Markdown and conditional file writes remain the interoperability boundary; no server/browser changes or CRDT operation store.
- Keep unrelated local edits even if a different passage conflicts.
- For incompatible overlaps, retain the observed server passage and preserve the complete original local version as a synced recovery page.
- Recovery destination: `Bulletnotes Recovery/<original-name>-<event-id>.md`; body is exact original source without generated wrappers.
- Persist originals and recovery obligations before replacing source or eligible queued work; leased/unknown mutation payloads remain immutable.
- Concurrent edits survive deletion; a present file after an unknown DELETE is never destroyed on that old intent.
- Preserve CRLF, Unicode, missing final newline and explicit empty documents. Never normalize source as a merge side effect.
- Local durability and full synchronization are distinct; unresolved recovery uploads prevent a fully synced status.
- Keep Apple Silicon/macOS 14+, explicit Metal rendering, Rust 1.92 minimum, current authentication and validator requirements.
- Tests use generated data; live checks author only uniquely owned pages. Existing IME, VoiceOver, undo and hardware release gates require actual evidence.

## Review Focus

1. Literal conflict-marker lines, YAML frontmatter and code fences remain ordinary source; Task 1 tests exact bytes.
2. Two spaces or pages with identical titles cannot share recovery identity; Task 2 tests connection/path scoping and destination collisions.
3. A recovery copy edited before its first upload must not erase its original bytes; Tasks 2/4 test immutable first-create obligations.
4. A stale save completion after more typing or reopening the same page cannot replace the active buffer; Task 5 tests session/input/source tokens.
5. A saved recovery draft or timed-out network request must survive process exit at each commit boundary; Tasks 3/4/6 inject those failures.

## File and responsibility map

- New `src/sync/text_merge.rs`: bounded source-preserving adapter around the selected library; no persistence or HTTP.
- New `src/sync/merge_executor.rs`: one admitted CPU job, timeout/fallback and late-result ownership.
- New `src/domain/reconciliation.rs`: source generations, reconciliation tokens/results and immutable recovery-job values.
- New `src/store/reconciliation.rs`: context loads and atomic checked commits; new `src/store/recovery_uploads.rs`: immutable upload state and identity.
- Modify existing page/store/worker/recovery modules only at their integration boundaries; retain ordinary outbox semantics.
- Modify existing editor/coordinator/local-UI effects for explicit rebase completion; retain the TextInput instance.
- Add migration 005 and focused integration suites below; extend generated space/native fixtures.
- Floating chrome remains a separate bounded UI change after this sync implementation. Its approved design does not belong in storage or sync state.

## Task 1: Library merge adapter and bounded execution

**Files:** Create `src/sync/text_merge.rs`, `src/sync/merge_executor.rs`, `tests/automatic_text_merge.rs`; modify `src/sync/mod.rs`, `Cargo.toml`, `Cargo.lock`, `src/diagnostics.rs`.

**Interfaces:** `merge_text(base: &str, local: &str, remote: &str) -> TextMergeResult`.
Define `TextMergeResult { body: String, disposition: MergeDisposition, edits: Vec<SourceEdit> }`,
`MergeDisposition::{Clean, PreservedOverlap, BudgetFallback}`, and
`SourceEdit { range: Range<usize>, replacement: String }` (local UTF-8 byte coordinates).
`MergeExecutor::merge(&self, base: String, local: String, remote: String) -> TextMergeResult`
is async; cloned executors share one semaphore. Export both modules from sync/mod.rs
following the existing public reconcile integration-test boundary. Keep dependency
types private to the adapter; its public contract uses Bulletnotes/std types.

- [ ] Add failing tests named `compatible_words_survive_on_one_line`, `overlap_keeps_remote_and_independent_local_words`, `literal_markers_are_not_control_data`, `source_bytes_survive_crlf_unicode_and_missing_newline`, and `budget_fallback_preserves_the_remote_document`. Assert exact output and disposition, including same-position insertions and delete/edit overlaps.
- [ ] Run `rustup run 1.99.1 cargo test --test automatic_text_merge --locked`; require the new contract to fail before implementing it.
- [ ] Pin the four researched dependencies exactly. Disable gix-merge default features, enable `sha256`; do not enable CLI, external merge drivers or Git repository discovery. Add sha2 for event identity in Task 2, not a handwritten digest.
- [ ] Keep one-sided/equal-source fast paths. For three changed inputs, use public `text::Merge::new`/`run` with Histogram and `ResolveWithTheirs` on raw lines first. If it reports an auto-resolved conflict, refine through a shared Unicode `split_word_bounds` dictionary encoded as integer/LF lines, rerun the same engine, and decode only checked known IDs. Choose recovery from the word pass's resolution. Never parse user text as markers or use lossy UTF-8 decoding.
- [ ] Admit at most 1,048,576 bytes per input and 20,000 combined tokens per merge pass, counted before engine allocation. Cap accumulated output at the sum of input byte lengths using checked arithmetic. Exceeding any cap returns whole-remote fallback and a source edit covering the replaced local buffer. Decoder inconsistency or worker failure also preserves via fallback and emits a typed diagnostic.
- [ ] Compute local-to-result `SourceEdit` ranges with gix-imara-diff's public token diff/hunks and byte-offset tables, not a new diff algorithm. Tests assert UTF-8 boundaries and that applying edits reconstructs the result. A whole-buffer replacement is permitted for the resource fallback.
- [ ] Run admitted computation through `spawn_blocking`, outside SQLite and Slint. Acquire a slot before scheduling; if busy, use budget fallback. After 150 ms stop awaiting the result and use fallback, but keep the owned permit inside the running closure until it really finishes. Late results cannot commit. Do not spawn an unbounded queue or claim that aborting a JoinHandle cancels CPU work.
- [ ] Add generated identity/one-sided/concurrent-edit cases and an injected slow worker test proving timeout, single-job capacity, late-result rejection and continued unrelated store saves. Run the focused suite, then commit `Add bounded library-backed text reconciliation`.

## Task 2: Durable events and immutable recovery uploads

**Files:** Create `migrations/005_automatic_reconciliation.sql`, `src/domain/reconciliation.rs`, `src/store/recovery_uploads.rs`, `tests/automatic_recovery_store.rs`; modify `src/domain/mod.rs`, `src/store/{mod,worker,pages,restoration,sync_queue}.rs`.

**Interfaces:** Define `StoredPage { page: PageSnapshot, source_generation: u64 }`;
`RecoveryUpload { event_id: Uuid, ordinal: u32, operation: PendingMutation }`;
`RecoveryUploadState::{Pending, Leased, Unknown, Blocked, Acknowledged}`.
Expose these async StoreHandle methods, each returning an existing StoreError on failure:
`recovery_uploads(connection: &ConnectionId, limit: usize) -> Result<Vec<RecoveryUploadHead>, StoreError>`,
`claim_recovery_upload(event_id: Uuid, ordinal: u32) -> Result<Option<RecoveryUpload>, StoreError>`,
`mark_recovery_unknown(event_id: Uuid, ordinal: u32) -> Result<(), StoreError>`, and
`acknowledge_recovery_upload(event_id: Uuid, ordinal: u32, ack: MutationAck) -> Result<(), StoreError>`.
Define `RecoveryUploadHead { event_id: Uuid, ordinal: u32, key: PageKey, state: RecoveryUploadState }`.
The list returns pending/unknown job metadata; bodies load only when claimed/read back.

- [ ] Write failing tests for immutable originals after recovery-page edits, same-name pages across connections/folders, restart while leased, duplicate observations, occupied destinations and event survival after deleting the original page. Run `rustup run 1.99.1 cargo test --test automatic_recovery_store --locked` and confirm failure.
- [ ] Add nonnegative checked `source_generation` to pages and revision to merge_drafts; increment source generation on source/base/permission/deletion-state changes. Add immutable automatic-event snapshots and recovery-upload jobs with persisted source, destination, operation ID, ordinal and constrained state. Preserve destination-attempt rows on relocation so uncertain old requests remain watched. Events/jobs must not cascade away with a source page. Preserve all existing rows and migration backup/rollback behavior.
- [ ] Fingerprint a versioned, length-unambiguous serde tuple of connection, original path, base/body/revision, local source, remote presence/body/revision/permission and optional draft source using sha2. Compare the stored original tuple on reuse; do not equate different tuples solely from their digest. Enforce unique event identity transactionally and persist IDs once, including destination retries.
- [ ] Generate the filename from the original final stem: retain ASCII letters/digits/hyphen/underscore, replace other runs with one hyphen, truncate to 64 ASCII bytes, use `note` if empty, then append the event UUID and `.md`. Validate through PagePath. Ordinal 1 preserves a distinct authored draft using its own persisted UUID destination. Do not modify recovery source to add metadata.
- [ ] Fence ordinary mutations of a recovery destination behind its exact immutable first-create job. Edits remain locally durable; after first-create acknowledgement queue their ordinary guarded update against that acknowledged base. No coalescing, trash action or same-path edit may replace the original first-create source. Keep the event's kept version even if the user later edits the recovery page.
- [ ] Include linked jobs in both original-page `PageSyncFacts` and connection `CacheProgress`; blocked/unknown jobs remain obligations. Test migration failure rollback, legacy draft initialization, and exact-source preservation. Run the focused suite and existing store recovery suite; commit `Persist automatic recovery events and upload jobs`.

## Task 3: Checked transactional reconciliation and local durability

**Files:** Create `src/store/reconciliation.rs`, `tests/automatic_reconciliation.rs`; modify `src/store/{mod,remote,pages,restoration}.rs`, `src/domain/{page,reconciliation}.rs`, `src/sync/reconcile.rs`.

**Interfaces:** Define `ReconcileContext { token: ReconcileToken, local: PageSnapshot, remote: RemoteRead, draft: Option<MergeDraft> }` and `MergeDraft { body: String, revision: u64 }`.
`ReconcileToken` carries key, source generation, expected local revision, conflict ID,
draft revision and the active mutation's ID/state. `PreparedResolution` carries that
context, chosen text/disposition and preservation obligations. Provide async
`StoreHandle::load_reconcile_context(key: &PageKey, remote: RemoteRead) -> Result<ReconcileContext, StoreError>` and
`StoreHandle::commit_reconciliation(prepared: PreparedResolution) -> Result<StoredPage, StoreError>`.
For pages not yet cached, retain existing direct remote-install semantics.
Define `LocalSaveOutcome { attempt: SavedRevision, stored: StoredPage }` and return
it from `save_local_from_base`; preserve the simpler save_local API for other callers.

- [ ] Write failing tests for disjoint edits, overlap plus independent local text, no common base, stale context after a local save, stale draft, and a recovery insert failure rolling back source/base/outbox changes. Run `rustup run 1.99.1 cargo test --test automatic_reconciliation --locked` and confirm failure.
- [ ] Load context on the store owner, compute with Task 1 outside the owner, then compare every token field in one commit transaction. On mismatch return StaleRevision with no changes. Do not relax existing source checks or merge inside an open transaction. Preserve original snapshots/chosen result and Task 2 obligations before installing source, rebasing its remote base and replacing only eligible pending/blocked work. Automatic resolution increments source_generation while retaining local_revision as the last accepted authored revision; it must not fabricate an editor-authored revision.
- [ ] Retain equal/one-sided/clean-deletion paths. Implement missing/base-less create and incompatible overlap policy from the spec. Leased/unknown operations defer replacement until readback; never delete them through manual resolve_conflict. A stale network observation requires a new remote read before another commit attempt.
- [ ] Retain the existing local-save transaction's source/base/conflict capture as the durable handoff when an editor baseline is stale. Return its actual StoredPage with the authored attempt immediately after persistence; do not wait for a merge to acknowledge local durability. Automatically reconsider the persisted conflict through the new pipeline, including after restart. Capture the pre-save stored source in kept history when it differs, rather than dropping a pending resolved version. Later edits and changes to stored source invalidate obsolete prepared results.
- [ ] Persist draft revisions and expose them through existing draft APIs; drain admitted writes before automatic processing of legacy conflicts. Preserve a distinct draft as a kept version and immutable ordinal-1 recovery job; do not apply unfinished draft source to canonical text. Leave prior records intact on failed checks.
- [ ] Add tests for empty-versus-missing pages, read-only observations, identical-body/new-revision updates, leased/unknown immutability and repeated unchanged observations. Run the focused suite plus `--test sync_reconcile --test conflict_recovery`; update old manual-conflict expectations only where the new policy intentionally changes them. Commit `Apply automatic reconciliation with source guards`.

## Task 4: Network worker, readback and truthful completion

**Files:** Modify `src/sync/worker.rs`, `src/store/{sync_queue,recovery_uploads,mod}.rs`, `tests/{sync_lifecycle,conflict_recovery,sync_presentation}.rs`, `tests/support/space_fixture.rs`; create `tests/automatic_sync.rs`.

**Interfaces:** Reuse SilverBulletClient::put(&PendingMutation), existing read/guard
validation and Task 2 jobs. Add async `StoreHandle::retire_unknown_delete(id: OperationId, observed: RemoteFile) -> Result<(), StoreError>` and
`StoreHandle::relocate_recovery_upload(event_id: Uuid, ordinal: u32) -> Result<(), StoreError>`.
Retirement preserves operation/source evidence and retains the observed file;
relocation persists a new unused UUID destination before another create attempt.

- [ ] Add failing generated-space tests for lost recovery PUT acknowledgements, differing/identical occupied destinations, delayed creates, denied recovery uploads, and native source changing during canonical upload. Run `rustup run 1.99.1 cargo test --test automatic_sync --features protocol-validation --locked` and confirm failure.
- [ ] Feed ordinary observations, stale guarded writes and divergent unknown-PUT readback through Task 3. Preserve four bounded network lanes and per-key serialization; recovery first-create jobs participate in destination-key serialization. Limit stale reconciliation retries to three fresh-read attempts before existing backoff, without blocking unrelated pages.
- [ ] For recovery jobs use create-only writes and exact readback. Matching bytes acknowledge; different bytes relocate durably; missing readback retries only the same immutable job. Record each destination attempt and retain readback watches on uncertain old destinations, so a late arrival cannot be forgotten when a destination changes. Job recovery must survive process restart.
- [ ] Admit legacy unknown operations even when a conflict record exists, then reconsider that record. Unknown DELETE plus any present document retires the old destructive intent; it must never enter retry_after_readback. Missing readback acknowledges deletion. Preserve concurrent edits with create-only recreation and retry on occupied paths through current observations. Retain evidence/watch state for already-sent uncertain requests: retiring a local intent cannot cancel server work already accepted. If a delayed delete removes a preserved concurrent edit, the edit-wins recreation obligation remains durable.
- [ ] Propagate recovery-job failures to original-page and global status. Canonical acknowledgement alone cannot report Synced. Preserve auth/permission/validator handling and uncertain-outcome rules. Test identical-content reincarnation with zero extra DELETE requests and recovery edits queued behind their first-create payload.
- [ ] Run the focused suites and store/recovery regressions, including restart with unknown/blocked work; commit `Sync immutable recovery copies and resolve races automatically`.

## Task 5: Open-editor rebasing and recovery-draft admission

**Files:** Modify `src/app/{editor,coordinator,recovery_ui}.rs`, `src/app/local_ui/{effects,synchronization,presentation}.rs`, `src/domain/reconciliation.rs`, relevant existing editor/coordinator tests; create `tests/editor_reconciliation.rs`.

**Interfaces:** Define `EditorToken { key: PageKey, session_id: Uuid, input_generation: u64, source_generation: u64 }`.
Every restored/draft session gets a unique session ID and checked input generation.
Add `EditorSession::rebase_token() -> EditorToken` and
`EditorSession::apply_rebase(expected: &EditorToken, stored: StoredPage, edits: &[SourceEdit]) -> bool`.
Add `AppEvent::RebaseReady { expected: EditorToken, stored: StoredPage, edits: Vec<SourceEdit> }`
and change LocalSaved to `{ expected: EditorToken, outcome: LocalSaveOutcome }`.
Carry the expected token through SaveLocal effects and the ordered write queue;
keep it alongside the in-flight authored revision. Tokens accompany work, not its
completion. Carry StoredPage through load_active_page/load_sync_page, PageLoaded
and EditorSession::restore so source_generation is known at initial load.

- [ ] Write failing tests `typing_during_rebase_remains_dirty`, `late_save_does_not_acknowledge_newer_text`, `reopening_same_page_rejects_old_session_result`, and `equal_numeric_revision_does_not_imply_equal_source`. Run `rustup run 1.99.1 cargo test --test editor_reconciliation --locked` and confirm failure.
- [ ] Keep the ordered local-write barrier. Defer editor rebasing while its admitted save is in flight; acknowledge that exact session/attempt first using its actual stored snapshot. If submitted and stored source differ, retain both the presented editing baseline and durable snapshot until rebasing succeeds; do not pretend newer typing was authored against an unseen merged buffer. Capture current buffer/baseline/token afterward, compute outside UI/SQLite, persist through Task 3 and apply only if the complete token still matches. Recompute/coalesce after later typing rather than replacing it or marking it durable. An unmatched completion cannot reset the current baseline.
- [ ] Replace the exact-local-revision-only refresh gate with this explicit event path. Do not globally loosen apply_remote_snapshot. Preserve the TextInput instance and available selection/scroll state; use Task 1 edits to map valid UTF-8 observations. If observation is unavailable, retain the documented native gate rather than inventing a captured caret.
- [ ] Coordinate automatic legacy conflict processing with RecoveryUi::flush: close draft admission for that record, drain the current generation, then CAS its persisted draft revision. On failed commit reopen admission; on success keep preserved drafts reachable and show an optional resolved/recovery notice. No modal choice or forced view change.
- [ ] Verify keyboard editing, newer unsent changes, hide/quit barriers and failed local persistence. Add native fixture assertions for selection/undo only where observed; record remaining IME/VoiceOver gaps explicitly. Run focused and existing editor/coordinator/lifecycle suites; commit `Rebase active editors without discarding newer typing`.

## Task 6: Production acceptance, observability and evidence

**Files:** Extend `examples/{sync_review,recovery_review}.rs`, native integration tests and `benches/interaction.rs`; update `docs/validation/{sync-lifecycle,recovery,protocol-contract,release-matrix,merge-library-research}.md` and generated-data evidence.

**Interfaces:** Use existing typed DiagnosticEvent/OperationTimer instrumentation.
Add only typed merge/recovery operation categories and finite outcomes; never log
source, titles, paths, URLs, credentials or hashes that identify note content.

- [ ] Add boundary fault tests for process exit before/after resolution commit, recovery create/readback, canonical upload and draft drain. Assert every captured original remains local or remotely acknowledged and that no completion falsely acknowledges newer text. Test output-size/byte/token caps, saturation and late results under repeated-text load.
- [ ] Run `scripts/check.sh`, `git diff --check`, and `rustup run 1.92.0 cargo check --all-targets --locked`; the script runs formatting, strict all-feature Clippy and the complete lib/bin/test/example suite. Execute relevant ignored native fixtures explicitly. Read results; do not count ignored tests as passing evidence or rerun the same full checks without new changes.
- [ ] Run `, cargo-audit audit --file Cargo.lock --json` after integration. Require zero known vulnerabilities; classify existing maintenance warnings by active macOS runtime/build graph and record upstream ownership. Do not mask them or change unrelated dependencies within this feature.
- [ ] Capture generated-data native observations: remote edit during typing, overlap recovery without a prompt, source/selection/scroll after rebase, restart while offline, and resumed upload. Repeated unchanged polling must produce no new recovery files. Test only the owned fixture window and data.
- [ ] Execute an owned-page browser/native acceptance check against the configured server: independent edits, offline overlap, automatic recovery copy, delete/edit, restart, exact readback and guarded cleanup. Keep this distinct from the loopback contract and never inspect unrelated page bodies.
- [ ] Measure repeated-text merges in release mode within the caps and verify local-save responsiveness while a CPU job is deliberately stalled. Record distributions and resource limits, not a production guarantee inferred from the research timings. Obtain one fresh whole-branch review, fix findings, rerun affected checks, and commit validation evidence before claiming completion. Existing distribution/hardware/native release gaps remain explicit.

## Handoff

The six tasks share state and must execute in order. Recommend subagent-driven
execution with fresh task reviews because errors at the store/editor/network
boundaries could lose source. Native execution is also available with a final
independent branch review. Review this plan and select the execution method before
product code or application dependencies change.
