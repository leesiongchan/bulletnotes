# Bulletnotes development usage

Bulletnotes is a native Rust/Slint macOS client for a SilverBullet space. It is
currently a development build. [Release evidence](validation/release-matrix.md)
tracks the remaining input, accessibility, selection, performance, and distribution
gates. The note window has solid paper. Markdown source stays visible and editable,
with bold first/second-level headings, regular-weight lower headings, true bold and
italics (including combined emphasis), muted punctuation and strikethrough text,
underlined link destinations, and subtle code backgrounds. Heading, link, and syntax colors
follow the theme’s Markdown roles when supplied; text keeps the selected base size and line spacing.
Rows and grid cells use 1.6 times the selected font size, with half-point grid
lines (at least one physical pixel) and a 45-point text inset. Font and paper
choices remain independent.
Unordered lists paint bullets and tasks paint rounded outlined boxes, with a tick
for completed tasks, in theme ink. Their text aligns in compact two-em slots;
moving the caret into a prefix or selecting across it reveals its raw syntax
and original spacing. Unsupported or narrow prefixes remain raw.
Boxes are edited as Markdown rather than toggled by clicking. Markdown inside code
stays literal, and YAML frontmatter stays plain. Standard Markdown semantics apply,
including `__bold__` rather than Antinote's underline convention.
Themes without explicit Markdown colors retain contrast-based fallbacks;
Increase Contrast uses ink. Blur remains
deferred.

## Capture and browse

Use **BN → Show Bulletnotes** to reopen the window. The default shortcut is
Option+A. If another application owns it, keep using the menu and choose another
combination in Appearance. Supported physical keys are A–Z, digits, Space and
F1–F20 with at least one modifier. A failed change keeps the working shortcut.

Hover near the top or bottom edge to reveal the matching header or footer.
F6 moves keyboard focus through header, footer and the active view; Escape returns
to that view. Revealing controls does not move or rewrap the note. The header
includes native window controls, Settings, Search, New scratchpad, Pin, and the drag area. The capsule footer contains Trash, the note date/position, Older, Newer, Jump to Newest, and Changes. Resize from edges/corners; Pin keeps the note above ordinary windows. Closing
hides the window after local saves finish. **BN → Quit Bulletnotes** drains local
writes and cancels network work. A failed local save retains the editable buffer;
retry or copy the source using normal editing commands.

All pages searches the local catalogue by literal phrase and optional folder.
**More pages** loads the next result batch, including pages beyond the initial
batch. Cached pages open offline. Online-only pages require a successful download;
index progress indicates when full content search is incomplete. Command+K opens
all-page search; Command+F searches exact source within the current note.

Command+N creates a scratchpad that saves locally and automatically syncs under
`Quick Notes/` once it contains text. Older/Newer (Command+[ / Command+]) traverse
this folder's stack in durable creation order; edits, visits, and sync do not
reorder it. Command+1 jumps to newest. The footer shows `1 of N` at newest, and a
small accent dot marks it while controls are hidden. Ordinary SilverBullet pages
have no stack position; Jump to Newest returns to scratchpads.

Existing generated filenames supply their creation time. Manually named legacy
scratchpads use their modification time once, or first local observation when
unknown. Ordering metadata is local to the selected connection and survives
restart. Catalogue-only entries count too; opening one needs a download. A failed
save or metadata check retains the current note. Delete moves a note to recoverable
Trash and selects an older neighbor, a newer neighbor, or a fresh blank when the
stack is empty. There is no automatic expiry. Command+P toggles Pin.

Known selection targets can be restored, and auxiliary views retain the editor.
Exact live directional selection capture across page changes/relaunch remains an
open supported-API requirement; see the [selection investigation](validation/selection-api.md).

## Appearance

Text size and paper controls appear before the theme collection. Command+comma
opens these settings.

Themes use AntiNote's grid-backed cards, eight color swatches, and separate sun
and moon buttons. The sun assigns a palette to Light; the moon assigns it to Dark.
Both preserve the current Light/Dark/System mode. Clicking the rest of a card
assigns the currently effective slot. The 14 AntiNote built-ins appear first in
their original order, followed by the remaining palettes. Cards use two columns
when space permits and a single column in narrow windows. Settings controls use
the macOS light/dark appearance independently of note colors.

Paper, Graphite, Matcha and Rose are built in. Light and Dark assignments are
independent; System follows macOS appearance notifications. Font size (11–24
points) and Plain/Dots/Ruled/Grid paper are independent of the palette. macOS
Increase Contrast adjusts the displayed palette without replacing saved theme
colors. Solid paper and static transitions honor reduced transparency/motion.

Import/export uses Bulletnotes JSON schema version 1, at most 32 KiB. Theme names
contain 1–32 Unicode scalars after trimming; colors are opaque `#RRGGBB`. Duplicate
JSON keys are rejected. New low-contrast imports require confirmation before
adding them. Existing themes, including every bundled AntiNote palette, apply
and save immediately when selected. Imported
names cannot overwrite a built-in theme identity. Antinote theme import is deferred.

```json
{
  "schemaVersion": 1,
  "name": "Custom Paper",
  "dark": false,
  "paper": "#FCFAF4",
  "surface": "#F3F0E7",
  "ink": "#252420",
  "muted": "#5B584F",
  "accent": "#725326",
  "line": "#817B6D",
  "selection": "#E2D9C5"
}
```

## Connection and recovery

Connection settings target an HTTPS SilverBullet server; the initial endpoint is
`https://notes.harflix.lol`. Tokens are stored in macOS Keychain only when chosen;
session-only use remains available. Denied Keychain access does not prevent local
capture. Server writes require successful conditional-write verification with an
owned temporary probe page. An incompatible server remains read only. See the
[protocol contract](validation/protocol-contract.md) for exact behavior.

Local edits save independently of network availability. Sync uses guarded writes,
backoff, and readback for unknown outcomes. Compatible changes merge automatically.
For overlapping changes, the server passage stays in the main note, independent
local edits survive, and the complete original local source becomes a separate
recovery page. Changes is optional; retained drafts, Trash and kept versions remain
available for explicit restoration. Sync stays pending until recovery copies are
acknowledged too. Reused destinations use guarded reconciliation and preservation
instead of silent overwrite. See
[recovery behavior](validation/recovery.md).

The local store is in
`~/Library/Application Support/lol.harflix.bulletnotes/bulletnotes.sqlite`.
Keep this directory when replacing a development app. Do not copy a live SQLite
file alone: its WAL may contain committed data. Stop the app before filesystem
backup or use SQLite's backup facility. A second launch asks the existing owner
to reopen and cannot open a second writer.

## Diagnostics and development checks

**BN → Export diagnostics…** opens a native save dialog. Export is local and
explicit. Events contain only opaque IDs, outcome categories, timings and counts;
no note text, titles/paths, server URLs, tokens or clipboard data are accepted.
Diagnostics are non-blocking and expire after seven UTC calendar dates. Exports
include complete records already written; a few in-flight diagnostic events may
still be buffered. Diagnostic buffering is independent of note durability.

See [build and artifact verification](build.md) for development packages and
separate distribution prerequisites. The top-level About action includes Slint
attribution.

Run `scripts/check.sh` for formatting, strict all-feature Clippy, and automated
regressions. Explicit native checks use `scripts/native-lifecycle-check.sh` and
the named commands in validation documents. Run the native release benchmark with
`rustup run 1.99.1 cargo bench --bench interaction --locked`; it opens generated
content only and writes `target/interaction/latest.json`. The benchmark reports
failure when a threshold is unmet; routine tests do not run it implicitly.
