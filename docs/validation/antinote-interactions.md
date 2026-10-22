# Antinote interaction validation

This branch implements the approved [interaction design](../superpowers/specs/2026-10-22-antinote-interaction-fidelity-design.md)
on pinned Slint 1.18.1, with Rust 1.99.1 on Apple Silicon/macOS 27.0.1.
All evidence uses generated notes, temporary stores, owned windows and controlled
loopback synchronization. No user store, deployed space, credentials or clipboard
contents were used.

## Implemented behavior

Markdown commands compute a source-preserving UTF-8 replacement and apply it
synchronously through the native input history. Each command has one undo/redo
step with directed selection. Task boxes use the same rendered hit geometry for
painting and activation. Layout/source changes cancel pending activation; pointer
toggling preserves an offscreen caret and viewport. Read-only, recovery, lifecycle
and native composition guards remain enforced. See [usage](../usage.md#source-editing-shortcuts).

The header uses theme paper, circular idle controls have no outline, keyboard
focus uses an accent outline, and increased contrast adds an ink outline. The
capsule footer retains its surface with a softer border. Independent hover/F6,
editor geometry, native controls and persistent failure visibility are retained.

Search uses FTS5 snippets/highlights, literal ordinary Text, explicit validated
UTF-8 match ranges and two-line 112px result rows. Matches stay visible with CJK,
emoji and literal markup. Theme/contrast changes refresh emphasis. Online-only
rows do not fabricate a body. Query ordering, folder isolation, generation checks
and pagination remain unchanged.

## Search reentry diagnosis

The retained selected index allowed newly instantiated ResultRows to reclaim
focus after reopening search and replacing its results. Native virtualization
could then remove the focused row, leaving Escape without an event target.
A failing headless regression established query focus before replacement and row
focus afterward. Clearing selected-index in focus-query fixes the cause. The
regression now verifies focus survives replacement and accepts typing. The native
fixture also types/deletes after reopening; it accepts the retained query caret
rather than assuming insertion at the end. Temporary focus/event diagnostics were
removed. Idle Escape capture leaves active composition to the input method.

## Fresh verification

- `scripts/check.sh`: passed; formatting, all-target/all-feature strict Clippy,
  393 tests passed, zero failures, 24 explicit ignores.
- `python3 scripts/check-markdown.py`: passed; prototype formatting/strict Clippy,
  43 renderer/history/composition/marker tests passed, zero failures/ignores.
- Authoritative patch reapplied successfully to fresh checksum-verified pinned
  sources before the integrated checks; generated dependency changes are retained
  solely in the authoritative patch.
- Nine native contracts passed: search, real Pinyin composition guarding,
  commands/history, floating chrome (30 cases), shortcuts, UI/source preservation,
  scrollbars, scratchpad navigation, and controlled sync/restart after an actual
  command. First desktop-interrupted attempts remain recorded as failures;
  acceptance uses the later successful runs.
- Full `editor_native_input_review --pinyin --require-single-undo --trace-input`:
  **passed twice in fresh complete runs**. Each passed all four Pinyin cases,
  candidate placement, source restoration and one-step undo in Tartan/Mononoke at
  top and scrolled positions. This closes the F1 input-fixture gate; the historical
  failed attempts remain recorded below.
- Independent whole-branch review: no Critical issues, two Important defects fixed
  with observed RED→GREEN regressions, and one deferred Minor. Final full checks
  and native command/sync fixtures passed after the fixes.

Complete logs remain in `target/antinote-interactions/f1-*` and `f2-*` and
`target/markdown-validation/`. Ignored tests include explicit native/release/manual
wrappers; only individually executed fixtures count as native evidence.

## Native evidence and visual inspection

| Contract | Accepted log | Success marker |
| --- | --- | --- |
| Search reentry/previews/pagination | `f1-rerun-search.log` | `native_search_preview_passed` |
| Real Pinyin composition guard | `f1-rerun-composition.log` | `native_composition_guard_passed` |
| Commands/history/pointer/local restart | `f1-idle-commands.log` | `native_editor_commands_passed` |
| Floating chrome/input/geometry | `f1-idle-floating.log` | `floating_chrome_contract_passed` |
| Shortcuts | `f1-native-shortcuts.log` | `native_shortcut_contract_passed` |
| UI/source | `f1-native-ui.log` | `native_ui_contract_passed` |
| Scrollbars | `f1-native-scrollbar.log` | `native_scrollbar_contract_passed` |
| Scratchpad navigation | `f1-native-scratchpad.log` | `native_scratchpad_contract_passed` |
| Actual command, controlled sync/restart | `f1-idle-sync.log` | `native_sync_contract_passed` |
| Full native input, four Pinyin cases and single-step undo | `f2-input-baseline.log`, `f2-input-repeat-1.log` | `native_editor_input_roundtrips_passed` |

Each accepted log records its marker as true and exited zero. The failing full
input logs are `f1-idle-pinyin.log`, `f1-pinyin-confirm.log`, `f1-pinyin-trace.log`
and `f1-pinyin-final-*.log`; do not count them as passes. The later supplementary
Tartan/Mononoke/attention captures also timed out without a first frame. This
resembles the previously documented [occlusion state](editor-geometry.md#why-the-earlier-fixture-stalled),
but that does not identify this run's occluder or explain its earlier cancellation
failure. No alpha override, renderer workaround, or native-input change was made.

Inspected generated evidence:

- [Before formatting](antinote-interactions/editor-commands-before.png) /
  [after](antinote-interactions/editor-commands-after.png): exact source delimiters,
  unchanged editor placement and selected word.
- [Before task click](antinote-interactions/editor-checkbox-before.png) /
  [after](antinote-interactions/editor-checkbox-after.png): first task tick changes;
  second task, offscreen caret and viewport stay in place.
- Fresh floating fixture: [hidden](antinote-interactions/chrome-hidden.png),
  [header](antinote-interactions/chrome-header.png),
  [footer](antinote-interactions/chrome-footer.png),
  [both](antinote-interactions/chrome-both.png), and
  [400×440 maximum-size text](antinote-interactions/chrome-clearance-400-last.png).
  Reveal preserves editor layout and keeps required actions inside the window.
- Fresh search: [505×552 Tartan](antinote-interactions/search-tartan.png),
  [400×440 Mononoke](antinote-interactions/search-mononoke-min.png),
  [increased contrast](antinote-interactions/search-contrast-min.png), and
  [long token](antinote-interactions/search-long-token.png). Matches are visible;
  literal markup, two-line previews and date/cache labels do not overlap.
- Retained **C1 checkpoint** style captures: [Tartan](antinote-interactions/chrome-tartan.png),
  [minimum Mononoke](antinote-interactions/chrome-mononoke-min.png), and
  [persistent save failure](antinote-interactions/chrome-attention.png).
  These establish that checkpoint's style inspection; they are not fresh final
  captures. Header/footer style has not changed since C1.

## Independent review and fixes

A fresh reviewer inspected the complete branch, including the native patch and
uncommitted S2 work. It found no Critical defects. The two Important findings
were reproduced in the actual command module before correction:

1. Selecting `- # heading`, `- > quote`, or nested equivalents formatted structural
   prefixes as ordinary words. Prefix resolution now composes list, quote and
   parser-recognized heading boundaries. Bold/italic/strike tests assert exact
   source and both directed selections; cuts through nested prefixes decline.
2. Heading protection intersected an entire line, rejecting a caret outside inline
   code while allowing the empty EOF line inside an unfinished fence. The guard
   now checks caret context and probes the empty EOF line through the same parser.
   Tests distinguish unfinished/closed fences and a caret inside/outside inline code.

The author verified both fixes via tests and the full suite; no second independent
review of the fixes is claimed. The reviewer also ran a temporary 174,630-case
command/selection sweep without observing a panic or invalid resulting selection.

Deferred Minor: the earlier E1 lockfile change resolves gix-imara-diff's hashbrown
edge to 0.15.5 instead of 0.17.1. No functional failure was found and the checkpoint
has no rationale for that unrelated edge. It was left outside the material fix
pass; this session did not modify the lockfile.

## Input and release limits

The E1/E4/E5 checkpoint established real Simplified Chinese Pinyin conversion,
commit, Escape cancellation, empty-marked-text guarding, native focus handoff and
candidate placement. Fresh final native checks have their own status above.
Unknown or active composition declines custom source edits; empty preedit alone
never establishes Idle. Non-macOS backends remain fail-closed unless they supply
an authoritative Idle status. Linux/Windows runtime command activation is not
accepted by this macOS pass.

Physical human input methods beyond the exercised Pinyin path, VoiceOver,
supported macOS/hardware coverage, selection capture across page changes/relaunch,
performance and signed/notarized distribution remain separate release gates.
Earlier selection and lifecycle records remain historical and are not promoted
to fresh acceptance. The controlled command/save/restart/sync checks do not waive
live-server or packaged-app gates.

## Execution decisions

The ledger records these choices and their limits:

1. Keep plan task IDs E1–E5/C1/S1–S2/F1 in the ledger; numeric helper tooling does
   not match these headings. Risk: bookkeeping only.
2. Establish Idle after ambiguous Pinyin Escape only following an explicit native
   discard. Risk: an untested candidate submode could handle cancellation differently.
3. Add a direct dev reference to already pinned i-slint-core for lifecycle injection.
   Risk: one removable test-only manifest/lock reference; no new version/package.
4. Editor-to-chrome focus discards composition and establishes fresh Idle only
   while the window is key. Deactivation remains Unknown. Risk: an untested input
   method could retain conversion despite explicit discard.
5. Invalidate cached search presentation on theme/contrast changes through the
   existing local bindings. Risk: redundant model projection per appearance change.
6. Keep a first paragraph at the box edge eligible despite fallback-glyph top
   overhang; shifted lines still clip. A red/green emoji pixel regression and full
   renderer suite cover this. Risk: an untested vertical-alignment edge in Text.

7. Keep non-macOS activation, other IMEs, VoiceOver, hardware/OS, distribution,
   live-server and performance acceptance separate. Risk: those paths remain
   unverified, not implicitly approved by this branch review.
8. Do not compile a C++ consumer of the shared generator changes: the application
   builds through Rust. Risk: the C++ generation path lacks runtime verification.

F1 interaction acceptance is complete on this host. Broader release gates above
remain open; this development branch does not claim general IME/VoiceOver,
performance, hardware/OS or distribution acceptance.

## Native input and startup follow-up — 2026-10-23

The source and maintained patch are unchanged from `7911aa0`; no fixture retry,
forced alpha, activation workaround or native-input correction was introduced.
[Structured evidence](antinote-interactions-evidence.json) records the source
hashes, accepted runs and interruption separately. The fresh full application
check passed 393 tests with 24 explicit ignores, formatting and strict Clippy.
The unchanged renderer suite's 43 passes above remain applicable.

`f2-input-baseline.log` and `f2-input-repeat-1.log` both exited zero, each with
four `native_pinyin_conversion_commit_cancel_passed` markers and the final input
success marker. Their traces show the Pinyin input source, active key window,
first responder and input context intact after undo, followed by a new marked
composition and source-preserving Escape cancellation. Both themes also passed
exact one-step selection undo/redo, cursor/preedit geometry, candidate placement
and input-source restoration. The acceptance is bounded to these observed cases.

A third attempt, `f2-input-repeat-2.log`, was interrupted during the first key of
Mononoke's scrolled case. Its notification trace explicitly records the owned
window resigning key and the application resigning active; the next input was
rejected. This run did not reach its optional deliberate-focus-loss probe and is
not counted as a full pass. That observed interruption does not establish the
cause of the earlier untraced Pinyin-after-undo failure.

The startup controls also passed without implementation changes:

| Control | Observed first-frame behavior |
| --- | --- |
| Ordinary startup | First frame at 149 ms; verified visible/alpha 1 at 251 ms |
| Owned opaque covers, delayed show | Zero frames/alpha 0/occlusion 8192 while covered; first frame 45 ms after uncovering |
| Same cover with chrome configured before the first frame | First frame 38 ms after uncovering; source and Metal layer verified |

The covered controls reproduce and recover from the stalled state through normal
visibility events. Neither early chrome nor a permanent rendering deadlock was
established. The original historical occluder remains unidentified; these results
do not attribute every earlier no-frame observation to the same external cause.

Fresh supplementary captures now pass and were inspected: [Tartan with hidden
chrome](antinote-interactions/final-chrome-tartan.png), [minimum Mononoke with
controls](antinote-interactions/final-chrome-mononoke.png), and [persistent save
failure in increased contrast](antinote-interactions/final-chrome-attention.png).
They supplement, rather than relabel, the C1 checkpoint captures above.
[Scrolled Tartan preedit](antinote-interactions/native-antinote-tartan-scrolled-pinyin-preedit.png),
[Mononoke preedit](antinote-interactions/native-antinote-mononoke-pinyin-preedit.png)
and [Mononoke committed text](antinote-interactions/native-antinote-mononoke-pinyin-committed.png)
show the generated input phases. No personal note content was used.
