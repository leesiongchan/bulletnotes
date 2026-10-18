# Structural hardening design

The user's active goal is to fix all code-structure/design weaknesses identified
in the assessment and cleanup sequence. This is an implementation-quality pass
on the existing Bulletnotes architecture. Preserve product behavior, durable
source bytes, database schema, sync protocol, theme format, Apple Silicon/macOS14+
and explicit Metal selection. Existing manual/hardware/distribution gates remain
recorded separately; this pass cannot establish those observations.

## Decisions

1. Move the persisted EditorPosition value and its validation to domain/editor.rs.
   The domain and store modules must not depend on app modules. Preserve serialized
   fields and UTF-8/directional-selection validation. Existing app::model callers
   may use a re-export while internal dependencies use the domain definition.
2. One UI-independent lifecycle model owns admission and progress for editing,
   connection changes, close preparation, body flush and final quit. Hide and quit
   carry explicit intents. Native callbacks request preparation through LocalUi,
   flush recovery and appearance, then commit the admitted intent. UI properties
   reflect state, and are not a second source of truth. Reject overlapping
   transitions. Errors restore admission, and shutdown freezes editing before the
   first wait. Independent background sync refresh state stays independent.
3. Split local_ui by responsibility: controller/action/event orchestration;
   effect queues and workers; presentation; search/find; sync adaptation; lifecycle
   workflows. Private child modules share controller state; do not add traits,
   dependency containers, locks or public abstractions solely for this extraction.
   Each module owns its behavior, not just arbitrary blocks of the old file.
4. Extract dense native lifecycle and connection callbacks into named workflows.
   Retain existing typed errors and user messages; no broad error redesign.

## Alternatives

A whole-app framework rewrite adds risk with no evidence of need. A file-only
split leaves the lifecycle ownership problem intact. Use narrow domain movement,
explicit lifecycle transitions and cohesive private modules, in that order.

## Acceptance

- No app imports from src/domain or src/store; position serialization unchanged.
- No independent quitting/changing_connection/lifecycle_dispatched/native pending
  flags controlling the same lifecycle. One model determines admission and UI lock.
- Model tests cover repeated requests, conflicting intents, connection changes,
  failures and retry. Native fixtures retain save/merge-before-close and failure
  recovery, plus generated connection-change checks.
- LocalUi no longer owns worker loops, projection, search/find and sync adaptation
  in one large source file; callbacks delegate substantial async workflows.
- Existing strict checks, minimum Rust check, and relevant generated-data native
  lifecycle/editor/theme/sync fixtures pass. No real user data, Keychain or endpoint.
- One fresh-context review covers this complete cleanup delta, findings fixed and
  verification recorded before completion. No merge/push/publication implied.
