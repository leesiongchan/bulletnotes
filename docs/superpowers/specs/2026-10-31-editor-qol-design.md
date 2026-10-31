# Editor and chrome quality of life

Status: approved by the user on 2026-10-31, including the native-editor approach
and all seven requested changes with language highlighting.

## Follow-up after testing

The user questioned the persistent dot and recovery toolbar entry, and reported
moving native window controls and an ambiguous new-note glyph. The current
implementation removes the dot, retains recovery data and safeguards, moves
recovery access to Settings → General → Recovery, and uses a simple plus for new
notes. The normal delete action remains in the footer. This supersedes the
indicator and recovery-toolbar placement described below. The recommended UI
cleanup was stated while awaiting optional preference feedback; no destructive
backend removal is authorized.

Native traffic lights retain AppKit-owned geometry and remain in layout while
alpha/enabled state changes on reveal. The 32 px note-header row aligns custom
controls with the standard native controls. A reusable `chrome-check` example
measures native frames through repeated reveals without opening notes or sync.

## Intent and approach

Make Bulletnotes clean, calm and comfortable, using the installed AntiNote 2.1.3
as a reference while preserving Bulletnotes' eggshell, persimmon and silver
identity. Keep Rust/Slint, the source editor, existing Markdown semantics and
the approved application icon. See the [reference study](../../antinote-editor-chrome-study.md)
for observed evidence and inspection limits.

Three approaches were considered: native-editor polish with language highlighting
(selected), the same visual work deferring language highlighting (smaller but
incomplete), and replacing the editor (unnecessary selection, undo and input risk).
This work includes all seven QOL requests, necessary settings/documentation updates,
and verification. It does not add AntiNote's keyword modes, timers, extensions or
other unrelated product features.

Specific dimensions, typography and interaction choices below are proposed builder
decisions, not claims that the user specified them.

## Menu bar

Remove the `BN` title, retaining the small monochrome handwritten b and existing
menu actions and tooltip. Follow the tray's system `color-scheme`, independently
of the note's selected palette: warm black in light appearance, white in dark.
Keep the icon legible when system and note appearances differ. Prefer a native
template image if supported by the pinned backend; otherwise use explicit light
and dark SVG variants with the existing tray appearance property. Do not modify
the external Slint checkout or add uncommitted vendor patches.

## Auto-hiding scrollbar

Apply one shared behavior to editor, settings, search and recovery scrollbars,
including standalone `ThemeScrollbar` instances. Use a 5 px rounded thumb within
the existing 12 px pointer target. Do not reserve new horizontal content space.
Hide when there is no overflow. Reveal immediately on content-offset changes or
pointer entry into the scrollbar target; retain visibility while hovered or
dragging. Fade out over 180 ms after 900 ms without activity. Dragging, track
clicking, wheel input and accessibility increment/decrement remain functional.
Under reduced motion, visibility switches immediately after the same idle delay.
Hidden paint must not disable the edge target or interrupt an active drag.

## SilverBullet indicator

Replace the newest-note dot with a persistent, small sync indicator while a note
is visible, independent of whether the footer is revealed. Its scope is the current
note's SilverBullet state, not a claim that every cached page is up to date.
Use a 6 px mark in a 28 px keyboard-accessible target, positioned at the bottom
right with a 14 px inset. Keep that space clear of footer navigation controls.
Tooltip and accessibility text say “SilverBullet” and the actual state; show the
full state text in the revealed footer. Activating it opens the existing
connection panel; conflicts retain access to the existing recovery action.

| Evidence | Appearance and label |
| --- | --- |
| Local save failed | Red, “Local save failed”; preserve the retry notice |
| Conflict, authentication, failed sync, offline or uncertain remote outcome | Red, specific reason; never imply data loss |
| No configured sync for this note | Gray, “Saved locally · not syncing” |
| Read-only connection | Gray, “Read-only connection” |
| Current revision unsaved, saving locally, queued, or remote facts not yet known | Steady orange, specific pending/saving label |
| Work actively executing for the current note | Pulsing orange, “Syncing with SilverBullet” |
| Local revision saved, matching confirmed remote base, no pending or active work and no error | Green, “Synced with SilverBullet” |

Errors take precedence over activity; a newer dirty local revision takes precedence
over an older revision's activity/success. Expose actual executing page keys from
the sync worker's operation lifecycle in `SyncSnapshot`; admitted/queued work is
insufficient evidence for blinking. Clear activity on success, error, cancellation,
shutdown and connection replacement. Do not blink for periodic unrelated catalogue
polls. Keep network scheduling and guarded write behavior unchanged.

Use a gentle 1.2 s opacity pulse; under reduced motion use a steady mark with a
“Syncing” label. Reinforce red attention states with the existing visible error
notice so status is not communicated only through color. Choose light/dark colors
with at least 3:1 non-text contrast against the resolved paper, using a contrasting
outline when a custom theme needs it. Do not parse human-readable status strings
to determine the indicator state.

Remove the latest-note-dot preference from the visible settings UI. Continue to
deserialize the old `show_newest_indicator` field for existing settings without
repurposing its value to hide sync. Preserve the newest-note navigation command
and footer button. Update documentation to explain the replacement.

## Color, typography and controls

Retain warm eggshell and warm graphite base palettes with readable warm ink.
Keep persimmon for selected controls and meaningful emphasis; make decorative
edges quieter by reducing their component opacity, rather than lowering text
contrast. Preserve imported palettes, independent system/light/dark assignments,
paper types/styles and saved text sizes. No surprise resetting of preferences.

Use the already licensed, locally available DM Mono for writing and note previews,
and DM Sans for app controls and metadata. Bundle/register the existing project
fonts and their licenses; do not load fonts over the network. Preserve Unicode
fallback and emphasis rendering. Default writing size remains 15 px with the
existing paper-derived line spacing (normally 24 px); ruler/grid alignment must
use the same measured font metrics. Adjust the shared measurement element too.
Use 12 px metadata with readable contrast; avoid squeezed double-line footer text.

Resting toolbar controls have transparent fill, faint optional outlines and no
drop shadows. Hover/press use a restrained selection tint; selected pin remains
distinct. Preserve visible focus rings, disabled states and accessible labels.
Keep the editor's stable writing margins and 32 px control targets; reveal/hide
chrome never moves text or changes the editor viewport.

## Top and bottom bars

Keep the existing hover-reveal and keyboard F6 behavior. Top: traffic-light space
on macOS, Settings/Search at the leading edge, quiet elided title in the available
center, New Note/Pin trailing. Other platforms retain their native window frame.
Use the same horizontal rhythm and icon sizes without adding a permanent divider
or heavy background. Revealed controls sit on the resolved paper.

Bottom: one aligned row, Trash at the leading edge, optional date/count metadata,
older/newer/newest navigation and Recovery trailing, followed by the reserved sync
target. Give status a separate tooltip/accessibility description, and show its
text in the footer when width permits; at 400 px prioritize actions and elide
metadata/status rather than overlap or clip controls. Preserve persistent-bottom-
bar and note-count/date preferences. No unlabeled icon changes meaning silently.

The reference footer was not reliably captured in the initial study. Inspect it
before finalizing bottom-bar implementation and record any material differences;
the layout above is a Bulletnotes proposal, not an asserted AntiNote replica.

## Code blocks

Continue editing literal Markdown, including visible, muted fence delimiters and
language labels. Fenced and indented code receive a quiet continuous block
background across the writing width, covering wrapped and empty lines. Inline
code retains its compact treatment. Preserve CRLF, Unicode, selection, undo,
copy/paste, source bytes and Markdown-command isolation inside code.

Use a maintained tokenizer instead of custom language regular expressions.
Syntect is the proposed dependency: its official [documentation](https://docs.rs/syntect/5.3.0/syntect/)
identifies 5.3.0 as the current release when checked on 2026-10-31. Verify the
current stable version and toolchain compatibility again during implementation,
pin it exactly and commit the lockfile. Prefer its Rust regex backend. Use bundled
grammars and local processing only. Unknown/omitted languages and incomplete
fences remain readable plain code; no guessing from prose or source rewriting.

Produce sorted, disjoint UTF-8 byte ranges through the existing decoration
callback. Token colors adapt to the resolved palette, falling back to ink where
contrast is insufficient; increased contrast suppresses decorative token colors.
Cache grammar initialization and results by source/palette, bound tokenization
to 32 KiB per block and 128 KiB per note, and fall back to plain code beyond those
budgets. Verify typing latency on a representative 100 KiB note.

The pinned renderer currently paints code capsules around glyph runs, not full
block panels. The builder must validate an app-owned background overlay using
the actual wrapped text layout/geometry before committing to that mechanism.
Do not approximate block positions with character counts or line counts under
word wrap. If the pinned public API cannot expose accurate geometry, report this
constraint before changing the pinned Slint dependency; no silent substitution
of token capsules for the requested block treatment.

## Interface icons

Redraw all existing interface SVGs on a shared 24 × 24 grid, 1.5 px rounded
strokes, balanced 18 px visible bounds, and no baked color or shadow. Display
toolbar glyphs at 17–18 px, with optical corrections where needed. Use consistent,
recognizable shapes: sliders for settings, magnifier for search, outlined note
and pen for new, upright pin, trash, directional chevrons, newest-stack marker,
archive/recovery box, import/export arrows, and coherent appearance/paper/view
symbols. Tint through existing theme roles. Preserve app/tray brand silhouettes.

## Acceptance and verification

- Preserve the unrelated in-progress sync changes visible at start of this task.
  Do not commit or revert them as part of the UI work.
- Verify menu bar icon-only appearance with system light/dark and opposite note
  palettes. Record unavailable platform checks explicitly.
- Test scrollbar overflow, idle fade, hover, drag, accessibility actions and
  reduced-motion behavior against actual rendered UI.
- Test revision-aware indicator mapping, active versus queued work, errors,
  cancellation, reconnect, local-only and read-only cases. Exercise real HTTP
  fixture boundaries; live SilverBullet behavior requires an available server.
- Test fenced/indented/unknown-language/unclosed/Unicode/CRLF code, empty and
  wrapped lines, undo, selection preservation and source bytes after recoloring.
- Compare native before/after captures in light/dark at 400 × 440 and default/larger
  sizes. Exercise F6, Tab, tooltips, focus restoration, high contrast, reduced
  motion, paper alignment and existing persistent-footer settings.
- Run `scripts/check.sh`, review the final diff and report actual outcomes and
  unverified behavior. No completion claim based only on test doubles.
