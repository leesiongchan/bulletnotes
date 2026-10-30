# Bulletnotes development usage

Bulletnotes is a native Rust/Slint client for macOS, Windows, and Linux for a SilverBullet space. It is
currently a development build; input, accessibility, selection, performance, and
distribution release gates remain open. The note window has solid paper. Markdown source stays visible and editable,
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
Click a visible task box to toggle its Markdown source, or use Command+Shift+K
on the current task. Dragging, modifier clicks, and clicks on text or padding do
not toggle tasks. Markdown inside code stays literal, and YAML frontmatter stays
plain. Standard Markdown semantics apply,
including `__bold__` for bold text.
Themes without explicit Markdown colors retain contrast-based fallbacks;
Increase Contrast uses ink. Blur remains
deferred.

## Capture and browse

Use **BN → Show Bulletnotes** to reopen the window. The default shortcut is
Option+A on macOS and Alt+A on Windows/X11. Wayland global shortcuts are
currently unavailable; use the window or tray menu instead. If another application owns it, keep using the menu and choose another
combination in Appearance. Supported physical keys are A–Z, digits, Space and
F1–F20 with at least one modifier. A failed change keeps the working shortcut.

Hover near the top or bottom edge to reveal the matching header or footer.
F6 moves keyboard focus through header, footer and the active view; Escape returns
to that view. Revealing controls does not move or rewrap the note. The header
includes native window controls, Settings, Search, New scratchpad, Pin, and the drag area. The capsule footer contains Trash, the note date/position, Older, Newer, Jump to Newest, and Changes. Resize from edges/corners; Pin keeps the note above ordinary windows. Closing
hides the window after local saves finish on macOS and Windows with a tray.
On Linux, or Windows without a tray, closing saves and exits. **BN → Quit Bulletnotes** drains local
writes and cancels network work. A failed local save retains the editable buffer;
retry or copy the source using normal editing commands.

Command+K opens search; Command+F searches exact source within the current note.
Search matches literal text, including parts of words, without case sensitivity.
Use the bottom scope control to switch between **All Pages** (with an optional
folder filter) and **Scratchpads**. All Pages follows recent visits; Scratchpads
follows stack order. The list/card buttons switch between compact source previews
and note miniatures; the view preference survives restart. Highlights mark matching
text while preview markup and links remain literal.

**More pages** loads the next result batch. Cached pages open offline. Online-only
pages match available metadata and require a download to open; index progress
indicates when full content search is incomplete. Arrow keys navigate results
(including columns in card view), and Return opens one. With no results, Return
or **Create a note** starts a scratchpad containing the query. Escape cancels a
drag or confirmation, exits Select mode, then closes search; it respects active
text composition. Reopening search focuses an empty query and clears selection.

Use **Select** to check individual loaded notes. Pagination preserves the
selection; changing the query, scope, folder, or connection clears it. **Move
selected to Trash** confirms the count and moves the entire batch atomically.
Individual notes also have a Trash button. Trash remains recoverable and follows
normal SilverBullet sync. Read-only, conflicting, and online-only notes cannot be
trashed from search; resolve conflicts or open/download those notes first.

Command+N creates a scratchpad that saves locally and automatically syncs under
`Quick Notes/` once it contains text. Older/Newer (Command+[ / Command+]) traverse
this folder's stack in durable order, initially based on creation. In search's
Scratchpads scope, **Promote to front** moves a note or selection to the front;
Shift+Return promotes and opens the focused note. Drag a result to reorder the
stack when the query is empty; dragging a checked note moves the selected block
together. Explicit order is local to the connection and survives restart without
renaming notes or syncing their order. Edits, visits, and sync do not implicitly
reorder existing notes; new scratchpads enter at the front. Command+1 jumps to
the front. The footer shows `1 of N` there, and a
small accent dot marks it while controls are hidden. Ordinary SilverBullet pages
have no stack position; Jump to Newest returns to scratchpads.

Existing generated filenames supply their creation time. Manually named legacy
scratchpads use their modification time once, or first local observation when
unknown. Ordering metadata is local to the selected connection and survives
restart. Catalogue-only entries count too; opening one needs a download. A failed
save or metadata check retains the current note. Delete moves a note to recoverable
Trash and selects an older neighbor, a newer neighbor, or a fresh blank when the
stack is empty. There is no automatic expiry. Command+P toggles Pin.

Caret, directed selection and scroll are saved when switching notes or leaving
the app, and restored when returning or relaunching. Auxiliary views retain the
editor. VoiceOver and the broader supported-device release checks remain open.

## Source editing shortcuts

The Command shortcuts below use Control on Windows/Linux; Option uses Alt.

On macOS, Command+B/I toggles bold/italic, Command+Shift+X toggles strikethrough,
and Command+Shift+H cycles the current line through plain, H1, H2, H3, and plain.
Command+Shift+K toggles an existing task. Option+Up/Down moves the current line or
selected lines. Return continues an eligible list/task, Tab indents it, and
Shift+Tab outdents it; an empty list item exits its list. New task items start
unchecked. Each command is one undo/redo step and preserves directed selection.

Formatting acts on the word at a collapsed caret or on selected content. It
preserves complete quote/list/task/heading prefixes; selections cutting a prefix,
delimiter, code, or frontmatter boundary decline the command. Heading cycling
also declines list items. Line movement preserves literal content, mixed line
endings, and the final newline. Commands wait while native composition is active
or its state is unknown, and respect read-only, recovery, and local-save barriers.

## Appearance

Text size and paper controls appear before the theme collection. Command+comma
opens these settings.

Themes use grid-backed cards, eight color swatches, and separate sun
and moon buttons. The sun assigns a palette to Light; the moon assigns it to Dark.
Both preserve the current Light/Dark/System mode. Clicking the rest of a card
assigns the currently effective slot. The 14 featured palettes appear first in
their catalog order, followed by the remaining palettes. Cards use two columns
when space permits and a single column in narrow windows. Settings controls use
the desktop light/dark appearance independently of note colors.

Paper, Graphite, Matcha and Rose are built in. Light and Dark assignments are
independent; System follows desktop appearance changes. Font size (11–24
points) and Plain/Dots/Ruled/Grid paper are independent of the palette. macOS
Increase Contrast adjusts the displayed palette without replacing saved theme
colors. Solid paper and static transitions honor reduced transparency/motion.

Import/export uses Bulletnotes JSON schema version 1, at most 32 KiB. Theme names
contain 1–32 Unicode scalars after trimming; colors are opaque `#RRGGBB`. Duplicate
JSON keys are rejected. New low-contrast imports require confirmation before
adding them. Existing themes, including every bundled palette, apply
and save immediately when selected. Imported
names cannot overwrite a built-in theme identity.

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
`https://notes.harflix.lol`. Tokens are stored in macOS Keychain, Windows Credential Manager, or Linux Secret
Service only when chosen; session-only use remains available. Linux needs a running
Secret Service provider. Denied credential-store access does not prevent local
capture. Server writes require successful conditional-write verification with an
owned temporary probe page. An incompatible server remains read only.

Local edits save independently of network availability. Sync uses guarded writes,
backoff, and readback for unknown outcomes. Compatible changes merge automatically.
For overlapping changes, the server passage stays in the main note, independent
local edits survive, and the complete original local source becomes a separate
recovery page. Changes is optional; retained drafts, Trash and kept versions remain
available for explicit restoration. Sync stays pending until recovery copies are
acknowledged too. Reused destinations use guarded reconciliation and preservation
instead of silent overwrite.

On macOS the local store is in
`~/Library/Application Support/lol.harflix.bulletnotes/bulletnotes.sqlite`.
Windows uses the user-local application data directory; Linux uses the XDG data
directory. See [desktop integration](build.md#desktop-integration).
Keep this directory when replacing a development app. Do not copy a live SQLite
file alone: its WAL may contain committed data. Stop the app before filesystem
backup or use SQLite's backup facility. A second launch cannot open a second writer. On macOS it asks the existing owner
to reopen; on Windows/Linux, reopen the existing window from the taskbar or tray.

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
regressions.
