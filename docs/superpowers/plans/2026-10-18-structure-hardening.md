# Structural hardening implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Fix the identified code/design weaknesses while preserving the existing app contracts.

**Architecture:** Retain domain/store/sync/theme/platform separation and the action/event/effect coordinator. Centralize lifecycle admission, then extract private cohesive LocalUi responsibilities.

**Tech Stack:** Rust1.99.1 (minimum1.92), Slint1.18.1, existing Tokio/SQLite/macOS adapters; no dependencies added.

**Spec:** docs/superpowers/specs/2026-10-18-structure-hardening-design.md

## Global Constraints

- Apple Silicon only/macOS14+, explicit Metal, current editor retained.
- Preserve UI behavior, persisted schema/serialization, sync protocol and theme format.
- Use generated temporary stores; do not access user notes, credentials or deployed endpoint.
- No merge, push, notarization upload or publication. Native execution method retained.

## Review Focus

- Close/hide plus asynchronous connection replacement must not overlap or lose accepted edits.
- Shutdown failure must restore authoring and permit retry, including merged drafts.
- Stale load/search/remote callbacks must retain revision and connection checks.
- Bounded effect queues and durability barriers must preserve ordering and report failure.
- Position bytes/selection direction and custom theme persistence must survive restart.

### Task 1: Domain dependency direction

Files: domain/editor.rs, domain/mod.rs, domain/page.rs, app/model.rs, store imports, tests/domain.rs.
Interfaces: domain::editor::EditorPosition retains fields anchor:usize, caret:usize, scroll_y:f32 and validate(self, &str)->Result<Self,PositionError>; app::model re-exports compatibility name.
- [x] Add neutral-path position serde/direction/invalid-boundary tests; observe missing-module RED.
- [x] Move existing value/validation without changing serialized data; change domain/store imports.
- [x] Run domain/navigation/editor/storage regressions; verify no inward app import; commit.

### Task 2: Lifecycle ownership

Files: app/lifecycle.rs, app/local_ui.rs, app/native_app.rs, native lifecycle/recovery examples.
Interfaces: private lifecycle model carries explicit Hide/Quit intent and Editing/ChangingConnection/Preparing/Flushing/Quitting state; LocalUi owns prepare/commit/cancel and UI projection.
- [ ] Add model tests for duplicate/conflicting requests, connection overlap, rollback/retry; observe RED.
- [ ] Implement model, replace duplicate flags, and route native preparation through LocalUi before awaits.
- [ ] Add or extend generated native admission assertions; run failure/restart and close durability fixtures; commit.

### Task 3: Cohesive LocalUi decomposition

Files: app/local_ui.rs and private child modules effects.rs, presentation.rs, search.rs, synchronization.rs, lifecycle.rs, bindings.rs.
Interfaces: public LocalUi preserved; child functions/methods private to controller; store worker effects return existing AppEvent.
- [ ] Preserve existing behavioral regression baseline; add coverage only for an uncovered boundary discovered during extraction.
- [ ] Extract worker creation/execution, projection, search/find, sync adaptation, lifecycle and binding functions by responsibility; preserve queue limits, weak references and error propagation.
- [ ] Run strict all-feature checks and generated local/editor/sync/theme/recovery/lifecycle fixtures; commit.

### Task 4: Callback readability

Files: native_app.rs and connection_ui.rs; affected child helpers.
Interfaces: callbacks invoke named workflows; existing controller contracts and user messages retained.
- [ ] Characterize existing failures with relevant current regressions, then extract dense async callback bodies into named functions.
- [ ] Run connection security/probe regressions and native lifecycle; inspect for equivalent ordering and rollback; commit.

### Task 5: Review and completion audit

Files: docs/validation/structure-hardening.md and this plan's execution ledger.
- [ ] Run scripts/check.sh, Rust1.92 locked production check and native generated-data matrix; inspect outputs.
- [ ] Dispatch one read-only fresh-context review of this cleanup range; fix material findings with regressions and retain explicit dispositions for every finding.
- [ ] Audit every spec acceptance against current source and observed tests; record remaining broader release gates separately; commit evidence.
