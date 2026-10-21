# Editor geometry against AntiNote

Compared the generated editor with genuine native AntiNote Tartan and Sanrio
captures on 21 October 2026. The reference's glyph widths match Menlo 14pt:
rendering Bulletnotes at 14pt produces identical Task and List text x coordinates
and the same 67-pixel Task ink width at 2×. The prior preview used 15pt, which
made the two-em task slot look wider. The existing slot and marker center remain.
The installed binary confirms Menlo as the default family; its font-size enum's
numeric mapping was not established. No note or preference was modified.

## Changes

| Geometry | Before | After |
| --- | --- | --- |
| Rows | 1.5 × natural font height | 1.6 × font size |
| Grid pitch | 18pt | Same as text rows |
| Grid stroke | 1pt | 0.5pt, minimum one physical pixel |
| Left/right text inset | 40pt | 45pt |
| Top inset | 32pt | 33.75pt |
| Checkbox side | 0.8em | 6/7em: 12pt at 14pt text |
| Checkbox corner radius | 1.5pt | 2pt |
| Bullet diameter | 0.36em | 0.4em |

Slint's line-height factor multiplies natural font metrics. The editor normalizes
that factor using Menlo's unrounded ascent/descent, so actual rendered rows and
grid cells share a pitch. The unused second measurement item was removed.
At 14pt, the 22.4pt pitch matches the reference. At 15pt, it is 24pt. Saved font
and paper preferences are preserved. Find retains its additional 60pt clearance.

The grid keeps the measured horizontal phase of about 5.35pt. A trial quarter-point
stroke-centering offset made vertical lines disappear on the 1× renderer and was
removed. Rendered regressions verify grid visibility, pitch, actual text rows,
inset, aligned task/list bodies, and exact source. A 14pt, 1× regression reproduced
missing fractional-position strokes and now requires every grid column. Grid
strokes keep a one-physical-pixel minimum, preserving 0.5pt on Retina displays.
Native rectangle rasterization still differs slightly from AntiNote's subpixel
stroke antialiasing.

## Verification

`scripts/check.sh` passed formatting, strict all-target/all-feature Clippy, and
351 tests with zero failures; 24 existing tests remained ignored. New geometry
tests first reproduced the original row and inset mismatches. The independent
marker test also verifies the actual compact slot width, pointer hit testing,
and undo/redo. All 30 renderer tests, including the seven replacement-undo regressions below,
and their formatting/strict Clippy checks passed. Independent read-only review found no blocking issue.

Generated headless Skia previews of the actual AppWindow:

- [Tartan, 14pt](editor-geometry/tartan.png)
- [Mononoke, 14pt](editor-geometry/mononoke.png)
- [Tartan, minimum window](editor-geometry/compact.png)

The previous session’s final native screenshot run failed before producing a visible frame
(the owned window reported alpha zero at its fixed one-second timer). The
previews above therefore use the same real editor with the headless testing
backend. The earlier native 14pt capture established the matching text metrics
before the final small inset and marker-size adjustments. The disposable capture
source is retained locally at `target/editor-geometry/capture-review.rs.txt`.

The normal native preview command remains:

```sh
target/debug/examples/ui_review --markdown-style --grid --theme antinote-tartan \
  --font 14
target/debug/examples/ui_review --markdown-style --grid --theme antinote-mononoke \
  --font 14
```

The development package under `target/packages/editor-geometry-final-20261021` passed
signature, attribution, ARM64/macOS 14, hardened-runtime, and generated-UI Metal
launch verification. Its binary and archive hashes match the verification report.
The prior app quit normally and the new package was opened; its running bundle
path and visible window were verified.

## Native cursor and IME verification

Native verification resumed on 21 October 2026, macOS 27.0.1 (26A434), ARM64,
Rust 1.99.1. The repeatable fixture is
[`editor_native_input_review`](../../examples/editor_native_input_review.rs),
using the actual AppWindow, generated Unicode/CRLF Markdown, 14pt Menlo, grid
paper, and both Tartan and Mononoke. It opens no store or network connection.
Geometry source files retain their starting SHA-256 hashes. The later atomic-undo
follow-up below changes only editing history in the authoritative Slint patch.

```sh
rustup run 1.99.1 cargo run --locked --features native-validation \
  --example editor_native_input_review -- --pinyin
# Also require single-step selection replacement undo:
rustup run 1.99.1 cargo run --locked --features native-validation \
  --example editor_native_input_review -- --pinyin --require-single-undo
```

Run native checks with WindowServer/Metal access. `--pinyin` requires the enabled
Apple Pinyin source (`com.apple.inputmethod.SCIM.ITABC`) and existing event-posting
permission. The fixture checks permission without requesting it, temporarily
selects Pinyin, restores the previous source after success or a check error, and
verifies restoration. Omitting this option tests native key/protocol handling
without exercising an actual conversion engine. Events target only the fixture
window or PID; captures select only its own AppKit windows.

Verified in both themes:

- Native Right-arrow movement and typing at the task body’s exact UTF-8 offset,
  including raw-prefix reveal and the compact slot transition.
- Movement across accented text, CJK, and the complete emoji grapheme; native
  Shift+Right selection and replacement; Command+Z / Shift+Command+Z source
  roundtrips, including exact original CRLF bytes.
- Public NSTextInputClient marked-text updates, selected composition ranges,
  cancellation, commit, and undo. Preedit changes the rendered text without
  changing the source buffer.
- Real Apple Pinyin `nihao` composition, visible conversion candidates, Space
  selection/commit of `你好`, undo, and Escape cancellation. Each runs at the
  first task and at a task after a long, wrapped/scrolled paragraph: four real
  conversion cases, each with verified input-source restoration.
- Candidate anchors remain inside the visible editor and candidate panels stay
  beside them. At the first row the native anchor is at logical `(95.57, 33.75)`;
  at the scrolled row it is `(95.57, 529.60)`. Both have 22.4pt height. Screenshots
  show candidate lists containing the expected leading `你好` conversion.

Fresh `scripts/check.sh` verification passed formatting, strict all-target/
all-feature Clippy, and 351 tests with zero failures; 24 existing tests remain
ignored. The initial strict native run completed all four Pinyin cases and then
exited with the reported one-step-undo failure. The follow-up below corrects that
failure. Independent read-only fixture review found no blocking issue.

Native evidence, visually inspected:

- [Tartan before composition](editor-geometry/native-tartan-before.png)
- [Tartan Pinyin preedit](editor-geometry/native-tartan-pinyin-preedit.png)
- [Tartan committed source](editor-geometry/native-tartan-pinyin-committed.png)
- [Mononoke Pinyin preedit](editor-geometry/native-mononoke-pinyin-preedit.png)
- [Scrolled Tartan preedit](editor-geometry/native-tartan-scrolled-preedit.png)
- [First-row candidates](editor-geometry/native-candidates.png) and
  [scrolled-row candidates](editor-geometry/native-candidates-scrolled.png)

### Why the earlier fixture stalled

The retained failure reports `visible=true`, `alpha=0`, and occlusion state 8192;
working captures report alpha 1 and occlusion state 8194. The missing bit `2`
is AppKit’s occlusion-visible flag. The pinned wgpu-hal 29.0.4 Metal acquisition
path returns `SurfaceError::Occluded` when it is absent, before Slint reaches
its rendering notification. Slint’s
[RevealOnFirstFrame guard](https://docs.rs/crate/i-slint-backend-winit/1.18.1/source/macos.rs)
sets alpha to zero after the unsuccessful pre-show frame and retains it while
acquisition remains occluded. This explains how a running timer can coexist
with zero rendered frames and an invisible owned window. It is consistent with
a startup occlusion wait; the fixed one-second checkpoint did not establish a
persistent deadlock. The controlled follow-up below reproduces and recovers from
that state, though the original environmental trigger remains unconfirmed.
Fresh native runs rendered normally, without changing alpha or patching the
backend. The restricted sandbox itself exposes no usable Metal adapter, so its
launch failure cannot validate native rendering.

Three independent fixture details also mattered:

- Direct `insertText` calls bypass Winit’s normal `keyDown` end-of-commit cleanup;
  explicit `unmarkText` prevents leftover marked state consuming the next key.
- Native modifier checks need physical `FlagsChanged` events. Modifier flags on
  an ordinary letter event alone do not update Slint’s modifier-key state.
- Queued `NSEvent` keys inserted literal `nihao` in both this fixture and an
  independent standard AppKit NSTextView. Process-targeted CoreGraphics events
  reached the real Pinyin conversion engine. An InputMethodKit mach-port warning
  occurred in both controls and in successful conversion runs, so that warning
  alone does not identify the failure.

An intermediate scrolled fixture incorrectly restored the caret with scroll
position zero, moving composition offscreen. Passing the current viewport position
to the existing restoration function kept both the composition and its candidate
panel beside the visible caret. No editor geometry change was needed.

### Atomic selection replacement undo follow-up

The maintained Slint patch now records a selection replacement as one history
entry containing both removed and inserted source. One undo restores the original
bytes and directed selection; one redo reapplies the replacement and collapses the
caret at its end. This covers typing, paste, and IME commits. Replacement history
stays separate from preceding deletions and following typing. A new edit discards
obsolete redo entries so their old source ranges cannot be replayed.

Seven [undo regressions](../../prototypes/slint-source-highlighting/tests/undo.rs)
first failed against the original patch and then passed. They cover shorter and
longer Unicode replacements, forward/backward selections, emoji, exact CRLF bytes,
IME commit, paste, edit callback counts, neighboring history, and branching after
undo. Fresh-source preparation reapplied the authoritative patch successfully.
The full renderer check passed all 30 tests plus formatting and strict Clippy;
`scripts/check.sh` passed 351 tests, with 24 existing tests ignored, formatting,
and strict all-target/all-feature Clippy. Independent read-only review found no
blocking issue. Geometry source hashes and every other Slint patch section are
unchanged.

The native `--pinyin --require-single-undo` command now exits successfully: both
themes restore the selected emoji in one Command+Z and reapply its replacement
in one Shift+Command+Z. All four real Pinyin conversion/commit/cancel cases,
candidate placement, and input-source restoration passed in the completed run.

The first rerun passed one-step selection undo in both themes but stopped in the
third Pinyin case when Space inserted a literal space instead of committing
`你好`. The same binary and unchanged fixture subsequently passed all four cases.
That run had no focus trace; its failure evidence
is retained alongside the successful run under `target/editor-undo/`:
`native-strict-first-failure.log` and `native-strict.log`. The regression red/green
logs, full checks, and isolated core source diff are in the same directory.

### Intermittent Pinyin investigation

The fixture's optional `--trace-input` mode records generated Quartz key codes,
flags and uptime; selected input source; application/key-window state; responder
and input-context identity; marked-text length; source length; and edit callbacks.
Winit's existing native callback traces go to a buffered, per-process file at
`target/editor-ime/callbacks-<pid>.log`. A failed Pinyin check also captures the
owned editor before restoring the previous input source. No retry or focus
override is added to the Pinyin cases. The focus-loss probe below deliberately
changes key status after the normal checks finish.

```sh
rustup run 1.99.1 cargo run --locked --features native-validation \
  --example editor_native_input_review -- \
  --pinyin --require-single-undo --trace-input
```

`target/editor-ime/diagnostic-6.log` reproduced the literal-Space failure with
native callback evidence in `callbacks-73643.log`. After capturing the first-row
Tartan candidates, the editor was key and had six UTF-16 units of marked text.
Before Space, it was no longer key and had no marked text. The selected Pinyin
source, application activity, responder identity and input-context identity were
unchanged. Repeated `windowDidResignKey:` / `windowDidBecomeKey:` callbacks occurred
between those checkpoints. Space reached `keyDown:` and `keyUp:` after composition
was gone and inserted one ordinary space.

The pinned dependency code explains this sequence: Slint sends focus-out to the
text input on native window deactivation; that disables IME, and Winit clears
marked text directly. The reproduced failure therefore starts before candidate
commit processing. The trigger for the key-window changes is still unconfirmed;
the failing callback trace lacks window identity. Later checkpoint logs include
the owned and current key-window numbers. This does not establish the cause of
every earlier intermittent failure.

Lighter traces also reproduced missing composition at startup and before Escape,
including one failure before any Pinyin candidate capture. Four complete runs
with callback tracing passed all four conversion/commit/cancel cases, including
single-step selection undo and input-source restoration. A disposable standard
AppKit NSTextView control also passed four equivalent Pinyin sequences. These
passes do not establish reliability or rule out a fixture timing problem.
The diagnostic runs and control source/log remain under `target/editor-ime/`.
Geometry, the maintained Slint patch and the undo regressions retain their
starting hashes from this investigation.

The diagnostic changes pass formatting and strict all-target/all-feature Clippy;
independent read-only review found no blocking issue. Two fresh `scripts/check.sh`
runs stopped on different assertions in the unchanged `automatic_sync` tests:
`current_recovery_ack_settles_obligation_while_old_attempt_stays_unknown` and
`blocked_handoffs_bound_reads_and_replay_latest_after_settlement`. The first
passed in isolation. Both failures remain in `full-check.log` and
`full-check-recheck.log` under `target/editor-ime/`; no sync code or assertion
was changed. A diagnostic full-suite run with `-- --test-threads=1` then passed
351 tests with zero failures and 24 existing tests ignored, recorded in
`full-tests-serial.log`. The normal parallel check remained unresolved at that
point; the test synchronization follow-up below resolves those failures.

### Focus interruption follow-up

The user confirmed switching or clicking apps during earlier Pinyin checks.
That provides a plausible trigger for the recorded native focus loss; the old
trace cannot identify the application receiving focus. Three subsequent strict
native runs completed all four Pinyin cases without focus transitions during
the input checks (`target/editor-focus/focus-1.log` through `focus-3.log`).

`--trace-input` now also observes this process's key-window and application
activation notifications. Each record identifies the notified and current key
windows by number/class, alongside the owned editor number and uptime. It reads
no external window metadata. Observer tokens are removed when the check exits.

Before creating or posting each Quartz key pair, the fixture now requires its
editor to be the active key window. If that precondition fails, the run stops with
`native input interrupted: owned editor is not the active key window`, and the
existing Pinyin cleanup restores the previous input source. It does not retry or
reclaim focus during conversion. This boundary check cannot detect focus lost
and regained between checks or prevent a change after an event is posted.

The optional `--verify-focus-loss` regression runs after the normal checks. It
deliberately resigns the owned window's key status, attempts Space, restores key
status, and requires rejection with exact source bytes unchanged. Before the
guard, this probe failed: Space was posted and changed the generated source.
With the guard it passes without creating a Quartz event. Both focus notifications
identify the owned Winit window, verifying observer delivery as well as the input
precondition. Red/green logs remain at `target/editor-focus/focus-loss-red.log`
and `focus-loss-green.log`.

```sh
rustup run 1.99.1 cargo run --locked --features native-validation \
  --example editor_native_input_review -- \
  --pinyin --require-single-undo --trace-input --verify-focus-loss
```

The combined command exited successfully with all four Pinyin cases, candidate
placement, single-step selection undo, input-source restoration and the deliberate
focus-loss probe passing (`target/editor-focus/native-final.log`). Formatting and
strict all-target/all-feature Clippy passed. Geometry sources, the maintained
Slint patch and undo regressions retain their starting hashes. The earlier
parallel sync test failures were addressed separately below. Independent
read-only review found no blocking issue.

### Parallel sync verification follow-up

Both intermittent failures were reproduced with controlled interleavings in
[`automatic_sync.rs`](../../tests/automatic_sync.rs). The recovery test now forces
a lost acknowledgment and waits for durable pending work to reach zero before
shutdown; observing the remote body alone could previously leave an unknown
operation after restart. The handoff test forces a writable readback/replay and
checks the exact source presented to the editor, whether still authored or
already canonical. It continues to require an authored branch while an immutable
recovery upload is unacknowledged. Preservation-copy and final remote assertions
remain intact.

Only the tests changed. The final normal parallel `scripts/check.sh` run passed
formatting, strict all-target/all-feature Clippy and 351 tests with zero failures;
24 existing tests remain ignored. Independent read-only review found no remaining
issue. Editor/geometry sources, the native input fixture and the maintained Slint
patch retain their starting hashes. Red/green logs and full checks remain under
`target/sync-races/`, including `handoff-red.log`, `recovery-red.log`,
`affected-green.log` and `full-check-final.log`.

### Startup capture follow-up

The generated [`startup_occlusion_review`](../../examples/startup_occlusion_review.rs)
reproduces the alpha-zero state with opaque owned cover windows already displayed
before showing the editor. While covered it reports `visible=true`, alpha zero,
occlusion 8192 and zero rendered frames. Removing the covers after 1.5 seconds
allows the first frame and alpha restoration within about 100ms. Both the normal
and pre-frame native chrome variants recovered. This proves initial frame
recovery; the probe's later chrome setup does not itself verify another frame.

```sh
rustup run 1.99.1 cargo run --locked --features native-validation \
  --example startup_occlusion_review -- --covered --delayed-show
# Compare native chrome configuration before the occluded first frame:
rustup run 1.99.1 cargo run --locked --features native-validation \
  --example startup_occlusion_review -- \
  --covered --delayed-show --chrome-before-frame
```

The covers briefly obscure the displays and are removed by an owned-window guard.
The probe uses generated Unicode/CRLF text, opens no store or network, and records
frame counts and native window state. Run it with WindowServer/Metal access.

The old `ui_review --capture` timer attempted capture after one second even when
the editor was still transparent. A controlled child launch reproduced that
attempt; its screenshot could return success with a fully transparent PNG.
Capture now keeps the one-second settling period for chrome, then requires a
rendered, visible, nontransparent and unoccluded frame. It yields one further
timer turn before the blocking screenshot command, allowing the reveal's Core
Animation transaction to commit. Apple's
[transaction documentation](https://developer.apple.com/documentation/quartzcore/catransaction)
describes this run-loop commit boundary. The eight-second readiness timeout reports
the native state instead of attempting an unready capture.

The controlled capture regression passed with actual opaque PNG pixels;
[the recovered native capture](editor-geometry/native-startup-recovered.png) was
also visually inspected. Holding the covers beyond eight seconds correctly failed
with zero frames and created no PNG. The existing generated-data production
renderer acceptance passed Metal startup, four themes and durable edit/reopen.
No production renderer or dependency change was needed. This does not identify
the historical occluder or prove a persistent production startup defect.

Final verification completed on 22 October 2026: the normal `scripts/check.sh`
passed formatting, strict all-target/all-feature Clippy and 351 tests, with zero
failures and 24 existing tests ignored. Independent read-only review found no
blocking issue. Geometry, undo, native input and sync-regression source hashes
retain their starting values for this follow-up.

Detailed probe logs, the disposable child-capture control source and red/green
evidence remain under `target/startup-occlusion/`, including `settled-cover.log`,
`settled-cover-chrome.log`, `capture-control-red.log`, `capture-control-presented.log`,
`capture-timeout.log`, `production-final.log` and `full-check.log`.

### Reviewed development package — 22 October 2026

Independent read-only reviews of the accumulated theme and editor changes found
no blocking issues. The reviewed sources were preserved while building the fresh
ARM64 development package at `target/packages/editor-geometry-20261022/`:

- `Bulletnotes-0.1.0-arm64-development.app`
- `Bulletnotes-0.1.0-arm64-development.zip`
- `Bulletnotes-0.1.0-arm64-development.verification.json`

The locked release build passed signature, hardened-runtime, empty-entitlement,
ARM64/macOS 14 and attribution checks. Its generated-UI installation launch
acknowledged Metal without opening a store, credentials or network connection.
Binary/archive hashes match the verification report; the archived binary and
attribution resources match the app bundle. This is an ad-hoc-signed development
artifact from the current dirty worktree. No installation or publication occurred.
The reviewed diff, source hash manifest and build log remain in
`target/package-review-20261022/`.

### Remaining limits

These checks exercise native AppKit/Winit and the installed Apple Pinyin engine
with generated events. Physical-keyboard observations, other input methods,
composition inside wrapped visual rows, VoiceOver, full-screen/Spaces behavior,
and the broader [native checklist](native-checklist.md) retain their own acceptance
requirements. The intermittent startup trigger remains unconfirmed. No platform
acceptance gate was relaxed.

Detailed logs and disposable AppKit controls remain under
`target/editor-geometry/`, including `native-input-final.log`,
`native-input-strict.log`, `appkit-ime-control.log`, and
`appkit-ime-cg-control.log`.
