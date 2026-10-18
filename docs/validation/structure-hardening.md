# Code structure and design hardening — 2026-10-18

This work addresses the four code/design weaknesses identified in the assessment
and cleanup sequence, plus concrete lifecycle defects exposed while validating
those changes. It preserves the existing app architecture and product contracts.
The source range starts at `77c8f1d`; no dependencies, database schema, protocol,
theme format, editor or macOS/architecture requirements changed.

## Requirement-by-requirement evidence

| Weakness | Current implementation | Evidence |
| --- | --- | --- |
| Storage/domain depend on application position type | `domain/editor.rs` owns the persisted value; `app/model.rs` re-exports compatibility name | No `app::` references under domain/store; neutral-path serde/directional/invalid-offset tests and existing storage/navigation regressions pass |
| Scattered foreground lifecycle flags | `app/lifecycle.rs` owns Editing, ChangingConnection, Preparing, Flushing and Quitting; close carries Hide/Quit intent | Five model tests; native generated SQLite contention rejects overlapping close/replacement; save-failure and merge-draft retry/restart pass |
| LocalUi mixes unrelated responsibilities | Controller dispatch/receive/composition remains in `local_ui.rs`; private effects, presentation, search, synchronization, lifecycle and bindings modules group related state and behavior | Controller reduced from 1,143 to 234 lines; unchanged queue capacities, request/revision checks and weak ownership inspected; full suite and native matrix pass |
| Dense asynchronous callbacks | Named native close preparation, connection completion/success, connection replacement and shortcut persistence workflows | Strict Clippy/fmt; connection form validation and controller/probe/security regressions; native shortcut commit/failure/rollback and lifecycle checks pass |

The split uses private child modules and concrete owned state; it does not add a
framework, trait hierarchy or new public extension interfaces. The actor still
owns SQLite on its dedicated thread. The ordered 16-slot local write queue and
latest-request read/search channels retain their original semantics. Barriers are
encapsulated by the effect owner and used for flush and quit.

## Defects exposed and fixed

Connection replacement is an owned UI operation once admitted. Dropping the
caller's future cannot abandon a queued settings write. The fallible last-page
lookup occurs before committing settings/coordinator state, and no asynchronous
gap follows coordinator replacement. The scoped operation guard releases admission
on completion/failure. Native regressions cover early caller cancellation,
cancellation while a generated SQLite write is held, read failure retaining the
original editor and settings, successful retry and restoration of the original note.

The UI's blocked property projects every foreground transition, including
connection replacement. Existing recovery authoring/action guards therefore reject
new recovery during a pending connection commit. A held-write native regression
reproduced this symmetric overlap before the projection fix.

Close admission rejects an already accepted recovery action until it finishes its
pre-dispatch durability flush. This prevents close preparation and recovery from
waiting on each other. A native regression starts recovery, immediately requests
close, verifies rejection, then proves recovery completion and subsequent close.

Position/history obligations are retained in an ordered in-memory metadata journal
until SQLite acknowledges them. Repeated updates coalesce by page and field;
queue saturation cannot drop the latest obligation. A barrier retries unsettled
writes and returns an error while storage still rejects them. Errors cannot be
converted into successful quit/flush acknowledgements. Tests cover a rejected
write followed by successful retry, 40 synchronous updates against the 16-slot
queue, preserved visit order and newer updates arriving during a write. A
confirmed removed page or offset into replaced source is treated as obsolete;
valid retained position/history is not discarded. The stale-offset regression was
observed failing before its fix. The journal itself is not durable until the
SQLite write succeeds.

A native run also exposed an intermittent About/activation race: activating About
could reveal the hidden note through the generic activation callback. Source
inspection identified passive activation being treated as a reopen request.
Passive activation now records previous-app focus only. Explicit owner-reopen,
menu and hotkey actions retain show behavior. The native fixture checks About
while hidden, sends the activation callback and then verifies second-instance
reopening through the owned notification route. It passes with the final source.

An ordinary same-instance LaunchServices reopen was also reproduced failing after
passive activation stopped revealing notes. The native events adapter now owns a
retained public Foundation handler for the reopen event, removed on teardown.
It does not replace Winit's delegate. The signed fixture proves ordinary reopening
and then verifies About still leaves the note hidden. This is the Finder-equivalent
LaunchServices route, not a claim of manual Finder interaction. Apple documents
[registering event handlers](https://developer.apple.com/documentation/foundation/nsappleeventmanager/seteventhandler%28_%3Aandselector%3Aforeventclass%3Aandeventid%3A%29)
and [NSApplication event handling](https://developer.apple.com/documentation/appkit/nsapplication).
Only existing Foundation binding features were enabled; Cargo.lock is unchanged.

## Verification

- `scripts/check.sh`: **184 passed, zero failed, 19 explicit ignored gates**;
  strict all-target/all-feature Clippy and formatting passed.
- `rustup run 1.92.0 cargo check --locked --bin bulletnotes`: passed.
- `cargo build --locked --all-features --examples` on pinned Rust1.99.1: passed.
- Native generated-data matrix: full lifecycle, accepted recovery actions, merge-draft failure/shutdown/restart,
  local callbacks/save/reopen, automatic sync, Metal/four themes/exact source,
  appearance persistence/rollback, connection form validation, installation smoke:
  **all nine passed**. No real notes, credentials or deployed endpoint were used.

`scripts/native-lifecycle-check.sh` also passed through an ad-hoc signed
LaunchServices bundle, including the owner-reopen route and shutdown recovery.

The appearance stress fixture emits Slint's long callback/instantiation-chain debug
diagnostic. The identical diagnostic is present in the pre-cleanup Metal evidence;
this change neither introduces nor suppresses it. Its contractual checks pass.

## Independent review and dispositions

The fresh read-only review covered `77c8f1d..45867fc` and found zero Critical,
three Important and one Minor issue. All four were addressed in one fix pass:
accepted-recovery close admission, late connection commit/cancellation boundaries,
metadata barrier failure/retry and ordinary same-instance reopen coverage. Each
behavioral reproduction was observed failing and then passing. The new metadata
edge regression also caught and fixed obsolete offsets after source replacement.
The final fixes were verified by tests and source inspection; no second independent
review was performed. [Structured evidence](structure-hardening-evidence.json)
records the final commands and observed outputs.

The reviewer declined manual input/visual/older-OS/selection/performance and
signed distribution judgment: those broader release gates remain open. It also
declined real server/credentials/user-data judgment: isolated generated fixtures
remain the evidence boundary. It ran no GUI/builds itself; the executor performed
the reported checks. The preexisting appearance diagnostic has no evidence of
being a regression in this delta and was retained without suppression. Attribution
and dependency versions were unchanged; no new legal-clearance claim is made.

## Release scope

These results establish the exercised structure and behavior, not complete product
release acceptance. The [release matrix](release-matrix.md) retains native/manual
input/accessibility, exact selection capture, older supported macOS execution,
large-note performance and Developer ID/notarization gates. The most recent
large-note measurement remains 132.203 ms p95 against the unchanged 100 ms target;
this structural pass does not claim a new performance measurement.
