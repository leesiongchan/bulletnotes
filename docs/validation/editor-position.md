# Live editor position persistence

Implemented on `codex/editor-position`, from `c7e9f48`, for the existing private
Slint 1.18.1 editor on Apple Silicon/macOS 27.0.1. Generated notes, isolated
SQLite stores, and owned windows/processes supply the evidence.

## Behavior

LocalUi captures UTF-8 anchor/caret and viewport at action/event boundaries when
the rendered page and source match the active editor. It validates offsets and
scroll, deduplicates observations, and uses the existing metadata journal. Note
switches, scratchpad history, auxiliary views, hide and quit retain directed
selection and scroll. This is boundary capture; a crash before the next boundary
does not promise to save the last cursor movement.

The note editor opts into retaining selection on focus loss. Other TextInputs
keep the previous default. External source is aligned before restoring selection
and viewport, so deferred source processing cannot reveal the caret afterward.
IME teardown and composition guards remain in place. No schema or dependency
change was needed.

Hide/quit wait for position metadata. Failure retains the visible editor and
releases close admission for editing/retry. Successful rebase records mapped
positions in both metadata and navigation history; stale rebase completions do
not rewrite them.

## Automated verification

- `scripts/check.sh`: 396 passed, zero failures, 24 explicit ignores; formatting
  and all-target/all-feature strict Clippy passed.
- `python3 scripts/check-markdown.py`: 45 renderer/history/composition tests
  passed, zero failures/ignores; formatting and strict Clippy passed.
- The authoritative patch reapplied to fresh checksum-verified pinned sources.
- RED/GREEN regressions cover live reverse Unicode/CRLF selection and long-note
  viewport across note/scratchpad switches, auxiliary views, a new window/store,
  metadata rejection/retry, failed-close admission, and mapped rebase metadata
  plus history. They do not inject `SavePosition` into LocalUi.
- Independent review found a mapped-position persistence gap and a native
  watchdog false-pass risk; both were fixed and the focused review found no
  remaining material issue.

Full logs: `target/editor-position/check-accepted.log`, `markdown-final.log`,
`rebase-red.log`, `rebase-green.log`, `rebase-history.log`,
`close-admission-red.log`, `close-admission-green.log`, and
`target/markdown-validation/`.

## Native, package and accessibility evidence

Nine final native contracts passed: live position/process restart, full Pinyin
input in Tartan/Mononoke at top/scrolled positions with single-step undo and
focus-loss guarding, commands/history, floating chrome (30 cases), composition
guarding, search reentry, scratchpad navigation, public AX ranges, and signed
LaunchServices lifecycle. Exact logs and success markers are in the
[structured evidence](editor-position-evidence.json).

The position fixture creates a backward selection through an owned AppKit
Shift+Left event, navigates away/back and through Appearance, then changes the
viewport and quits without an injected `SavePosition` or explicit final flush.
A separately launched reader process restores the exact bytes, anchor/caret and
scroll. Both stages reject timeouts and require current rendered note pixels.
The [before](editor-position/native-before-restart.png) and
[after](editor-position/native-after-restart.png) screenshots are byte-identical:
the same highlight, note lines and scrollbar are visible. Duplicate image
previews were misleading; PNG decoding and hashes confirm the actual files.

The earlier activation-lost position/command runs remain rejected in
`native-roundtrip-final.log` and `accepted-commands.log`; acceptance uses
`position-visible.log` and `commands-rerun.log`. The first input fixture relied on
deferred reveal after restoring viewport zero; it now requests reveal explicitly,
keeping the same candidate visibility assertion. The initial lifecycle fixture
called `performClose` while floating traffic lights were disabled, so no close
callback ran. It now reveals the header and checks the native close control is
enabled before invoking the same AppKit action. No production workaround was
added for either fixture assumption.

A fresh local ad-hoc signed development package was built in
`target/editor-position/package-20261023/`. Verification passed thin arm64,
macOS 14.0 deployment target, hardened runtime/empty entitlements, signature,
and isolated Metal installation launch. Its recorded source is `8cf85a3+dirty`:
the generator refreshed notices for already-locked hashbrown 0.15.5 and foldhash
0.1.5 (plus a notice ordering change). Those generated notices are included in
this checkpoint. No dependency was changed. The later changes affect fixtures
and documentation only. The separate signed lifecycle bundle used a temporary
store and held loopback response; it passed native close, failed-quit retry,
second launch and exact-source process restart. No normal user store was opened,
and nothing was published or notarized.

The public AX range check passed forward/reverse emoji selection and caret
ranges. The bounded VoiceOver availability attempt timed out on AppleScript
without obtaining reader output. Both owned processes were reaped, no VoiceOver
settings changed, and a subsequent process check confirmed the reader was off.
Apple documents [AppleScript control as a VoiceOver Utility setting](https://support.apple.com/en-ca/guide/voiceover/cpvougen/mac);
the timeout does not establish whether that setting or another automation
boundary caused it. VoiceOver speech, navigation and editing acceptance remains
open. Other IMEs, supported hardware/macOS coverage, full-screen/Spaces,
performance, and signed/notarized distribution remain broader release gates.
