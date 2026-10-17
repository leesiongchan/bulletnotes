# Durable local coordination checkpoint

Recorded 2026-10-17 on macOS arm64, Rust 1.99.1, Slint 1.18.1, winit/FemtoVG. The user asked to skip AppKit text-editor prototyping; the existing Slint editor remains selected. This checkpoint advances the independent local portion of Task 6. It does not close the native feasibility gate or complete the first milestone.

## Implemented behavior

- EditorSession owns exact Markdown source, monotonic local revisions, and acknowledged durability. A new draft is unsaved until its creation transaction acknowledges. A pending write never marks newer text saved.
- Coordinator is synchronous and performs no I/O. Effects carry revision and request identities; stale search/load results cannot replace a newer request, including A→B→A page loads. New edits cancel a pending page replacement.
- Autosave schedules a local write at 250 ms after the first unsaved edit; continuous typing does not postpone it indefinitely. This is a scheduling target, not a measured end-to-end disk latency guarantee. Navigation and explicit save/quit request immediate flushing. Disk failures preserve the editable/copyable buffer and cancel implicit discard or quit.
- LocalUi binds the actual AppWindow callbacks to bounded workers and one single-shot save timer. Body writes serialize; loads/searches retain only the latest queued request. UI status changes do not repeatedly assign the text buffer. Command+S retries a failed save.
- Page browsing, literal local search, folder filters, history, new notes, and Appearance navigation share the local coordinator. AppearanceUi remains the sole settings writer.
- Recency writes preserve saved selection metadata. EditorSession distinguishes a persisted restoration target from a freshly observed position; no observation is fabricated when Slint cannot expose one through a supported API.

## Evidence

The generated-content `local_review` example uses real AppWindow callbacks and a temporary SQLite database. It checks automatic save without a manual flush, injects an actual SQLite transaction failure, checks the unsaved buffer and old durable source, retries through the save callback, browses two notes, opens a saved note, traverses Appearance, closes/reopens SQLite, and compares exact source bytes. The fixture uses only a fake connection identity and performs no HTTP requests. It is callback integration evidence, not physical typing, IME, undo, accessibility, or caret evidence.

Verification commands (all prefixed by `rustup run 1.99.1`):

```sh
cargo test --all-targets --all-features --locked
cargo check --all-targets --locked
cargo build --example local_review --locked
cargo test --test editor_roundtrip --locked native_local_callbacks_save_browse_and_recover_exact_text -- --ignored
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo fmt --all -- --check
git diff --check
```

Standard suite: 71 passed, 0 failed, 11 ignored. The separate native callback check passed 1/1. Ignored native acceptance cases remain outside that count and are not assumed successful. Default build, strict all-feature Clippy, formatting, and whitespace checks passed. Red→green evidence covers missing coordinator/editor/binding contracts, retry clearing the failed-save notice, and a repeated flush draining edits made while an older write was pending.

## Remaining before a usable app milestone

`src/main.rs` is still the UI shell. Persistent app startup, restoring the last note and window settings, and safe native quit integration remain to be wired. The local binding currently handles the Slint window-close callback by requesting a saved quit; the final macOS close-to-hide behavior belongs to the lifecycle integration. Do not present this fixture as a production installation or a completed offline app.

Command+F current-note find is outstanding. Exact cross-page/relaunch selection capture is unresolved as described in [the selection investigation](selection-api.md); the user has not waived it. Physical IME, undo, selection/paste, VoiceOver, focus/full-screen behavior, and the remaining native visual observations are still required. Existing Preview 2 and its temporary user text were not replaced or terminated. Automatic SilverBullet sync and remote-page browsing follow in later tasks; this checkpoint has no remote transport.
