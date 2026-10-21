# AntiNote-inspired editing, chrome, and search

Date: 22 October 2026.

Status: interaction-first approach and conversational design approved with
"ok". Written specification awaiting user review. Implementation planning has
not started.

## Intent and success criteria

Bring AntiNote's everyday editing interactions to Bulletnotes: functional
checkboxes, list continuation, formatting shortcuts, line movement, quieter
chrome, and useful search previews. The user requested direct study of the
installed app; the [research record](../../validation/antinote-research-2026-10-22.md)
contains observations, screenshots, coverage limits, and the alternatives.

Success means these actions work in the current source editor, preserve the
user's text and selection, undo and redo in one step, and use the existing local
save and guarded synchronization flow. Native screenshots demonstrate the
chrome and search changes; behavioral checks demonstrate editing correctness.

This extends the [19 October design](2026-10-19-antinote-ui-ux-design.md).
Automatic SilverBullet sync, manual recoverable deletion, creation-ordered
scratchpads, navigation shortcuts, and full-space browsing retain their
existing contracts. This specification changes editing behavior and visual
treatment, not the note lifecycle or storage schema.

## Scope and approach

The selected approach is interaction fidelity first. A visual-only pass would
leave the drawn task boxes noninteractive. Broader feature parity would require
independent decisions about slots, split view, utility modes, and persistence.

Included:

- Pointer and keyboard task toggling.
- List continuation and exit with Return; list indentation with Tab/Shift+Tab.
- Bold, italic, strikethrough, and heading shortcuts.
- Moving the current line or selected lines up and down.
- Paper-colored header and softer circular button treatment.
- Match-centered search excerpts with highlighted matches.
- Acceptance fixtures for these changes, including updating the historical
  hover fixture to the already implemented one-step replacement undo contract.

Slots, split view, archive, locking, expiry, export, calculators, timers, OCR,
extensions, Vim, new paper styles, shortcut rebinding, search cards, search
reordering, and create-from-query are outside this pass. Existing clipboard
behavior and the meaning of Command+K and Command+F remain intact.

## Architecture and data flow

Keep the current Rust/Slint editor and maintained Slint 1.18.1 patch. Reuse
`pulldown-cmark` for Markdown context and `unicode-segmentation` for Unicode
boundaries. Search uses the existing SQLite FTS5 index. No dependency upgrade,
new package, or database migration is required by this design.

Three responsibilities remain distinct:

1. A focused Rust command module interprets committed source and computes a
   source edit. It receives the exact text, directed anchor/caret offsets, and
   a command; it returns no change or a replacement with the resulting directed
   selection. It does not serialize a Markdown tree or perform persistence.
2. `NoteEditor` routes editor commands. A small, explicit extension to the
   maintained `TextInput` patch applies atomic edits and reports checkbox
   activation. Native text state, pointer routing, composition, and history stay
   inside the text control. Where current composition events are insufficient,
   this scope also includes explicit composition-status propagation from the
   pinned native backend to core input handling. Global application shortcuts
   remain in `AppWindow`.
3. One successful text change emits the existing `edited` notification, which
   reaches `note-edited`, `AppAction::EditText`, local persistence, and the sync
   outbox through their existing bindings. Commands must not implement editing
   by assigning the app's `note-text` property.

The last constraint matters because external text assignment clears native undo
history. Existing typing/paste replacement already has atomic undo, but its redo
collapses the caret. Formatting and line movement also require replaying their
chosen post-command selection.

## Atomic edit contract

The following is a new internal contract to add, not an API already present in
Slint. It consists of one contiguous source range, replacement text, and the
resulting anchor/caret. Multiline commands may replace the smallest contiguous
span containing their changes while copying all intervening unchanged bytes.

- All offsets refer to UTF-8 source bytes. Selection direction is retained
  separately from sorted range bounds. Replacements must respect Unicode
  grapheme boundaries and cannot split CRLF pairs.
- The text control validates the range, resulting selection, enabled/read-only
  state, and composition guard before changing anything. Invalid commands leave
  text, selection, history, and composition unchanged.
- The command executes synchronously against the current committed text. It
  does not wait on a worker or apply an edit calculated for an older buffer.
- Each command creates one nonmerging history entry, including pure insertion
  or deletion. It records source and directed selection before and after the
  operation. One undo restores the former; one redo restores the latter.
- A new command clears obsolete redo history. Typing immediately before or
  after it remains a separate history operation.
- Exactly one `edited` notification is emitted for a successful source change.
  No-op or rejected commands emit none. Undo and redo each emit their normal
  single change notification.
- Keep the current viewport unless the resulting caret needs to be brought
  into view. Checkbox toggles preserve the viewport. Existing cursor rectangle,
  accessibility, and IME anchor notifications must reflect the resulting state.

Decline custom editing while IME composition is active, including composition
lifecycle transitions with empty preedit text. Native Return, Tab, and candidate
handling must still reach the input method. Do not commit or cancel composition
to force a formatting command through. A nonempty-preedit check alone cannot
establish this contract: current core events expose updates and commits without
an explicit complete lifecycle. Add the necessary backend/core signal for
composition beginning, ending, or cancellation, using actual native composition
state rather than equating IME enablement with composition. While that state is
unknown, decline custom commands without disturbing native input. Materialize
any additional existing Slint backend crate through the same pinned,
checksum-verified source preparation; do not modify Cargo's shared cache or
introduce a second Slint version. A host lacking verified status propagation
does not satisfy the composition acceptance gate.

Check edit permission in the text-control operation as well as the UI. The app's
combined read-only state includes note permission, recovery busy state, and
lifecycle barriers. Plain text selection, navigation, and copy remain available.

## Checkbox behavior

Recognize tasks accepted by the current Markdown parser and rendered by the
existing marker system, including ordered, unordered, nested, and quoted tasks.
Bare `[]` is not introduced as a separate Markdown dialect.

A primary-button press and release on the same visible checkbox toggles the
state byte: space becomes lowercase `x`; `x` or `X` becomes space. Revalidate
the actual task prefix before applying the edit. Do not assume a fixed offset
from marker start: ordered and unordered task marker ranges differ.

Use the same validated layout geometry for painting and hit testing. A checkbox
can be activated only where its actual rendered box is visible. Prefix padding,
bullet markers, task text, and raw-source-revealed prefixes retain ordinary text
selection behavior. Do not calculate hit rectangles from line numbers or fixed
font widths.

Modifier clicks, drags, pointer cancellation, leaving the box, loss of focus, or
a change to the source/layout/marker visibility cancel a pending activation.
Scrolling, resize, DPI changes, wrapping, and bidirectional layout must use
current geometry. A canceled checkbox gesture must not toggle a neighboring
task or turn into window dragging. Normal selection gestures outside a checkbox
continue through the native input path.

Clicking preserves the current directed selection and scroll position; the
one-byte state replacement does not shift offsets. Focus may return to the
editor without moving that selection. Source reveal rules remain intact.

Command+Shift+K toggles a task on the caret's current logical line and provides
the keyboard equivalent. It changes one task even with a selection, using the
caret's line. A line without a valid task produces no change. This also makes
the action available when the task prefix is revealed as raw source.

## List editing

List context comes from the existing Markdown parser. List commands do not
interpret escaped markers, YAML metadata, fenced code, or indented code as
lists. Preserve existing quote/container prefixes and marker spelling.

### Return

With a collapsed caret in a recognized item body, Return splits the item at the
caret and inserts the same container/indentation prefix and a new list marker:

- Unordered lists retain `-`, `*`, or `+`.
- Ordered lists increment the current item's decimal number and retain `.` or
  `)`. Other items are not renumbered. If the increment cannot form a valid
  CommonMark ordered marker, retain native Return behavior.
- Tasks insert an unchecked task using the same surrounding list style,
  regardless of the previous task's checked state.
- Existing body text after the caret follows the new marker. Padding after a
  newly created marker is one space; existing source elsewhere is unchanged.
- New line separators follow the current line's separator. At an unterminated
  final line, use the nearest preceding separator, or LF if none exists.

On an item whose body is empty or only horizontal whitespace, Return removes
that item's marker and its following padding, without inserting another line.
Retain its quote/container prefix and leading indentation. For example, `- `
becomes an empty line and `  - ` becomes two spaces.

Return with a nonempty selection, a caret inside the raw marker, or outside list
context keeps the existing native behavior. Shift+Return also keeps native
behavior. List continuation never runs during composition.

### Tab and Shift+Tab

Inside a list, Tab indents and Shift+Tab outdents the current logical line or the
selected logical lines. A selection ending exactly at the next line's start
does not include that next line. Selected lines must belong to the same list
container and cannot include code or metadata; otherwise the command declines
and the key follows existing native behavior.

Indentation is added after the existing block-quote prefix and before list
content. Add one tab when that line already uses tabs for indentation;
otherwise add two spaces. Outdent removes one leading tab or up to two leading
spaces at that position. Never rewrite the remaining indentation or renumber
items. An outdent with nothing removable is handled without editing.

Preserve directed selection over the same content as its offsets shift. These
actions change only explicitly targeted lines; they do not automatically move
unselected child items. Outside list context, Tab retains native behavior.

## Formatting shortcuts

| Shortcut | Action |
| --- | --- |
| Command+B | Toggle bold; add `**` delimiters. |
| Command+I | Toggle italic; add `*` delimiters. |
| Command+Shift+X | Toggle strikethrough; add `~~` delimiters. |
| Command+Shift+H | Cycle the caret line through plain → H1 → H2 → H3 → plain. |

Inline formatting targets the selection, or the Unicode word at the collapsed
caret. At a word end, use that preceding word; a caret in whitespace with no
adjacent word at that offset produces no change. Do not format list prefixes,
heading prefixes, code spans, code blocks, or metadata as ordinary words.

For a multiline selection, apply the same action independently to each nonempty
logical line's selected text, excluding leading/trailing horizontal whitespace
and line separators. Fully covered quote, list, task, and heading prefixes stay
outside the formatting target: selecting `- one\n- two` and applying bold yields
`- **one**\n- **two**`. A selection cutting through any such prefix rejects the
whole command. Treat the batch as one edit. A selection crossing a code or
metadata region, splitting an existing delimiter pair, or covering only part of
an already formatted span of the requested style produces no change as a whole;
do not partially apply a batch or guess at rewriting nested syntax.

When every target already has the requested style around its complete content,
remove that style's matched delimiter pairs. Otherwise add it only to targets
without that style and retain already styled targets. Use parsed Markdown
boundaries so removing bold from bold+italic text preserves italic, and retain
unrelated delimiter spelling. Targets must correspond to complete styled
content before an existing pair can be removed. A selection containing exactly
one complete delimiter pair and its content is treated as that inner content
for toggling; a selection that cuts a delimiter is rejected.

A nonempty selection continues to select its content and preserves direction;
for multiple lines the continuous selection necessarily includes intervening
markers. A collapsed caret maps to the same position within the word. Repeating
the command can therefore toggle the same target. Undo/redo restore the exact
pre/post-command source and selection.

Heading cycling changes only an ATX heading prefix on the caret's logical line,
after existing quote/container prefixes. Plain text gets `# `; H1 becomes H2;
H2 becomes H3; H3 becomes plain text. H4–H6 become plain text, after which the
normal cycle starts. Preserve optional closing hashes and the line body as
literal source; remove only the opening prefix and its separator when cycling
to plain. Blank lines may become an empty heading. A caret in code, metadata,
or a list item declines the heading command. A selection does not expand its
scope beyond the caret line.

## Line movement

Option+Up/Down swaps the current logical line, or a contiguous selection of
logical lines, with its immediately adjacent line. Selection ending at the next
line's start excludes that next line. Preserve selection direction and the
caret's position within the moved content.

Line content moves verbatim, including Markdown markers and indentation.
Existing line separators remain at their document boundary positions: mixed
LF/CRLF separators are not normalized and the presence of a final newline stays
the same. For example, moving the first line down in `a\r\nb` yields `b\r\na`.
Treat a final separator as a terminator, not an additional movable empty line.
Thus `a\n` has one movable line and moving it down is a no-op. Real empty lines
before that terminator can move: swapping the lines in `a\n\n` yields `\na\n`.
A collapsed caret just after the final terminator targets the final movable
line; an empty document has no available movement.

This command moves physical source lines, including inside code; it does not
promote nested items, renumber lists, or move an entire fenced block implicitly.
At the document's top or bottom, an unavailable move is handled without editing.
One undo/redo restores the entire swap and its selection.

## Chrome

Retain the existing window dimensions, editor insets, Menlo metrics, rounded
paper, controls, and native traffic-light geometry. Header and footer still
reveal independently, overlay text without reflow, and preserve F6/Escape focus
behavior, pointer selection, dragging, and resizing.

Use the active theme's paper color for the header. Circular buttons use a quiet
idle appearance without an outline and the theme's selection color for the
hover/selected background. Keyboard focus uses a one-pixel accent outline.
Increased contrast adds a one-pixel ink outline even at idle. Disabled actions
remain distinguishable, and labels continue to expose their action and shortcut.

Retain the inset capsule footer and its theme surface background, with a softer
idle border using the theme line color at 25% alpha; increased contrast uses
its full opacity. Keep date/position and textual save/sync information available
in the footer. Retain the current page path as a compact secondary header label
and accessible metadata. Actionable save/sync failures remain persistently
visible when chrome is hidden. Theme assignment and paper preferences are
untouched.

## Search previews

Keep the existing all-page query semantics, connection/folder filtering,
visit-based result ordering, pagination, cache availability, and keyboard
selection. Search improvements do not reorder scratchpads or download uncached
bodies merely to build a preview.

For nonempty queries, use the existing FTS5 match context to select a short
excerpt around matching body text, rather than always taking the first 160
characters. Use SQLite's maintained `snippet()` and `highlight()` facilities
so displayed match boundaries agree with its tokenizer. The existing index
columns are title, path, and body; this requires no index rebuild.

Use a maximum of 32 tokens for a matching body snippet and display up to two
wrapped preview lines, with an ellipsis when truncated. Choose the visible
portion around a highlighted span so at least one matched body token remains
visible when the text is clipped to those lines. If a hit is only in
title/path, retain the beginning of the cached body as context and highlight
the matching title/path instead. Empty queries retain ordinary beginning-of-note
previews. Online-only entries retain title/path and availability without a
fabricated body excerpt.

Return plain excerpt text and explicit UTF-8 match ranges to presentation.
If temporary marker strings are used to extract FTS highlights, choose strings
absent from the indexed row's literal text and remove them before presentation.
Literal marker-like text must roundtrip without being mistaken for a highlight.
Validate resulting ranges before rendering; invalid highlight metadata falls
back to plain text rather than hiding the result.

Render note text literally and add only the supplied match emphasis. Markdown,
HTML-like text, and links inside a preview must not become active markup or
actions. Use theme-aware emphasis that remains visible in increased contrast.
Search result rows retain title, full path, cache/read-only state, modified date,
and accessible labels. Allow the row's preview area to grow to two lines while
keeping scrolling and keyboard-selected rows visible at minimum window size.

Keep highlight metadata confined to search presentation. Any shared row data
used by recovery defaults to no highlighting and retains its existing behavior.
Existing invalid-folder, indexing completeness, query failure, and stale-result
handling remain visible and generation-checked.

## Boundaries and failure handling

The expected change locations are the maintained Slint patch, a focused Rust
source-command module alongside the existing Markdown code, `NoteEditor`,
`AppWindow`, action buttons, search/store presentation types, and the existing
fixtures. This is a scope guide, not a file-by-file implementation plan.

Keep commands synchronous and local. They make no network calls and require no
store migration. Once admitted, edits use the existing revision/debounce/save
machinery. Save failures retain the buffer and remain visible; the existing
transition barrier continues to prevent losing unsaved text on navigation.

Routine diagnostics may record command kind, outcome category, and duration at
command boundaries. Do not log keystrokes, source, excerpts, titles, paths,
selection text, or credentials. Rejected/no-op commands need no user-facing
error dialog.

## Acceptance and verification

1. **Pure edits:** table-driven checks assert exact changed bytes, replacement
   range, and directed selection for each command. Include accented text, CJK,
   emoji/graphemes, LF/CRLF/mixed separators, missing final newline, reverse
   selections, line-start endpoints, nested/quoted lists, ordered tasks,
   escaped markers, code, metadata, combined formatting, and empty contexts.
2. **History:** native and headless checks require one notification, one undo,
   and one redo per command, with exact pre/post selections. Include insertion,
   deletion, surrounding typing, branching after undo, and permission changes.
   Existing typing, paste, and IME replacement tests must retain their contracts.
3. **Pointer behavior:** exercise actual checkbox shape hit regions, padding,
   text selection, raw-prefix reveal, dragging/cancellation/modifiers, ordered
   and quoted tasks, scrolling, wrapping/bidi, resize, and display scale. Checking
   one task must never toggle a different task after the layout changes.
4. **IME and accessibility:** command attempts during composition leave source,
   preedit, and selection intact; the later native commit still works. Reuse the
   real input-method fixture where available. Check keyboard task toggling,
   focus indicators, accessible labels, and cursor/selection exposure. Report
   actual VoiceOver and unavailable host/input coverage separately.
5. **Persistence:** temporary-store checks prove a successful command reaches
   local save, restart recovery, and the existing guarded sync path. Injected
   save failure retains text; read-only/recovery/lifecycle guards prevent edits.
6. **Search:** temporary FTS fixtures cover matches beyond the first 160
   characters, title/path-only hits, phrase and case behavior, non-ASCII text,
   literal highlight markers/markup, empty queries, online-only entries,
   connection/folder isolation, pagination, and stale results. Highlight offsets
   must identify the same text returned by the matching engine.
7. **Visual/native:** compare generated-note before/after screenshots at default
   and minimum sizes in representative light/dark themes and increased contrast.
   Verify both independent hover states, F6/Escape, native traffic lights,
   selection/scroll preservation, and no reflow on chrome reveal. Verify two-line
   search previews and selected-row visibility.
8. **Existing gates:** update the historical floating-chrome fixture's obsolete
   two-step undo expectation to the current one-step contract, and ensure its
   generated screenshot directory exists. Then run relevant native contracts,
   `scripts/check.sh`, and `python3 scripts/check-markdown.py`. Verify the
   authoritative Slint patch applies to fresh pinned sources. Record platform
   limitations and existing ignored tests rather than presenting them as passes.

No development verification opens or migrates the user's normal database or
contacts their SilverBullet service. Use generated notes, temporary stores, and
the existing controlled synchronization fixtures.

## Reference sources

- [Direct app research and screenshots](../../validation/antinote-research-2026-10-22.md).
- [Existing editor geometry and atomic replacement evidence](../../validation/editor-geometry.md).
- [Slint TextInput documentation](https://docs.slint.dev/latest/docs/slint/reference/keyboard-input/textinput/).
  The locally pinned compiler/core sources and maintained patch remain the
  authority for the proposed private editor extension.
- [SQLite FTS5 snippet/highlight documentation](https://www.sqlite.org/fts5.html#the_snippet_function).
- [AntiNote list guide](https://antinote.io/user-manual/checklists) and
  [formatting guide](https://antinote.io/user-manual/formatting-shortcuts).
  This specification states Bulletnotes' chosen behavior where exact source
  preservation or the approved scope differs from the reference.
