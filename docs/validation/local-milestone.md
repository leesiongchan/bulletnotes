# Durable local application checkpoint

Recorded 2026-10-17 on macOS 27.0.1 arm64, Rust 1.99.1, Slint 1.18.1, winit/FemtoVG. The existing Slint editor remains selected; AppKit text-editor prototyping was skipped. This advances Task 6 and the local lifecycle portion of Task 11. The native feasibility gate and the first milestone are not yet complete.

## Implemented behavior

`src/main.rs` now composes the real local application. Foundation resolves the user's Application Support directory; Bulletnotes creates its own directory with mode 0700, and the existing store creates database/lock files with mode 0600. Startup validates settings and opens the last cached page in the selected connection, or creates a fresh local draft when none exists. Invalid data is preserved and reported, never reset. Visit recency increases within each space even if the wall clock repeats or moves backwards. Settings, theme assignments, window geometry, and pin state restore at startup; disconnected-monitor positions are clamped to a current visible display.

EditorSession owns exact Markdown source, monotonic revisions, and acknowledged durability. Autosave schedules a local write 250 ms after the first unsaved edit; continuous typing does not postpone it indefinitely. This is a scheduling target, not a measured end-to-end disk latency guarantee. Newer edits cannot be acknowledged by an older write. Errors preserve the editable/copyable buffer, and Command+S retries. New notes and page changes wait for required local acknowledgements. The coordinator performs no I/O; bounded workers serialize writes and retain only the latest queued load/search. Request identities reject stale results, including A→B→A navigation races.

Close and Command+W flush local changes and hide the window. The BN menu reopens it. Command+Q, the menu's Quit action, and macOS `NSApplication.terminate:` route through the same local save path. An application subclass overrides that public action without replacing Winit's delegate or modifying its class. No network response is involved. A failed body or window-settings save keeps the process/window available. Successful quit drains the existing serialized appearance writer, local metadata writes, and the SQLite worker before leaving the event loop.

Native window setup waits for Slint's first `AfterRendering` notification, then queues main-thread work using the supported weak-handle event-loop bridge. A zero-delay startup timer was tested and failed because no native window handle existed yet. The rendering notification fixes that demonstrated ordering error without a fixed startup sleep or an unstable backend extension. [Slint's rendering notifier](https://docs.slint.dev/latest/docs/rust/slint/struct.Window#method.set_rendering_notifier), [Apple's terminate action](https://developer.apple.com/documentation/appkit/nsapplication/terminate(_:)), and the [pinned Winit integration](https://github.com/rust-windowing/winit/blob/v0.30.13/src/platform_impl/macos/app.rs) describe the public boundaries involved. Winit's source explicitly accommodates custom NSApplication subclasses; its delegate remains owned by Winit.

Command+F opens current-note find, with literal, case-sensitive queries as indicated in the field. Enter/Shift+Enter and Command+G/Shift+Command+G navigate and wrap; Escape closes the bar. Find uses UTF-8 source offsets and Slint's supported selection setter, with constant auxiliary memory rather than retaining every match. Editing invalidates the current find match without moving the caret. Finding never edits the source or creates a saved revision. All-pages search, folder filters, history, and Appearance remain integrated. Window metadata uses AppearanceUi's existing serialized settings owner, preventing an older window snapshot from overwriting a newer theme.

A persisted editor restoration target is distinct from a freshly observed native position. Visits preserve existing selection metadata instead of writing invented zero offsets. Exact cross-page/relaunch capture remains unresolved in [the selection investigation](selection-api.md).

## Evidence

- Standard all-target/all-feature suite: **84 passed, 0 failed, 12 ignored**.
- Two separate native integration checks passed. `local_review` covers automatic saving without manual flush, real SQLite failure/retry, browsing, find callbacks, Appearance, and exact CRLF/tab/emoji/CJK bytes after reopening SQLite.
- `app_lifecycle_review` uses the actual application composition and a temporary database: startup note/font restoration, a native window close action, save-before-hide, reopening, a real failed quit transaction with retained editable text, native terminate retry, window/theme persistence, and a **separate-process restart** that restores the exact final text and font setting.
- The lifecycle fixture also passed when copied into an ad-hoc signed development bundle and launched through LaunchServices with `open -W -n`. This is local development signing, not notarization or distribution evidence.
- Existing native `ui_contract` checks passed **9/9**, including settings failure rollback and maximum-font footer captures. Find-bar layout was inspected at 400×440 in [Paper](native-ui/find-paper-minimum.png) and [Graphite](native-ui/find-graphite-minimum.png). Those two captures are presentation fixtures, not selection-reading tests.
- Default all-target build check, strict all-feature Clippy, formatting, whitespace, and shell syntax checks passed. Red→green evidence includes missing startup/find/lifecycle contracts, native-handle ordering, and repeated/backwards wall-clock visits restoring the wrong last note.

Reproduce:

```sh
rustup run 1.99.1 cargo test --all-targets --all-features --locked
rustup run 1.99.1 cargo check --all-targets --locked
rustup run 1.99.1 cargo build --example local_review --example app_lifecycle_review --example ui_review --features native-validation --locked
rustup run 1.99.1 cargo test --test editor_roundtrip --test macos_lifecycle --features native-validation --locked -- --ignored
rustup run 1.99.1 cargo test --test ui_contract --features native-validation --locked -- --include-ignored
rustup run 1.99.1 cargo clippy --all-targets --all-features --locked -- -D warnings
rustup run 1.99.1 cargo fmt --all -- --check
bash -n scripts/local-preview.sh
git diff --check
```

All automated native checks use generated content and temporary stores. No live SilverBullet page was read or changed. The old Preview 2 bundle and temporary user text were not replaced or terminated.

## Persistent local preview

`scripts/local-preview.sh` builds the main binary into a new, uniquely located, ad-hoc signed **Bulletnotes Local.app**. It never overwrites a running preview bundle. `--build-only` omits launching. The BN menu provides Show/Quit; local notes live under `~/Library/Application Support/lol.harflix.bulletnotes/bulletnotes.sqlite`. This preview now persists user edits, unlike the earlier disposable previews. Its footer explicitly says it is not syncing. The configured SilverBullet URL is only a connection identity at this stage; no remote protocol is connected.

## Remaining gates

Manual native typing, IME, undo/redo, selected paste, VoiceOver, focus/full-screen/Spaces behavior, and the outstanding visual observations remain required. Exact cross-page/relaunch selection capture has not been waived. Live OS appearance notifications, previous-app focus restoration, second-launch activation of the owner, global shortcut integration, and the rest of Task 11 remain open. The local preview does not register Option+A while Preview 2 owns it. The saved window position is captured on hide/quit; continuously tracking geometry and display changes belongs to the remaining lifecycle work.

Automatic SilverBullet synchronization, full remote-page catalogue/cache integration, conflicts/recovery UI, diagnostics/performance acceptance, and signed/notarized release validation are later tasks. No claim that these gates passed follows from the local checks above.
