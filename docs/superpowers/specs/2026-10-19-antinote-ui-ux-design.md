# Antinote UI and scratchpad navigation for BulletNotes

Date: 2026-10-19

Status: conversational design and written specification approved on 2026-10-19.

## Intent and approved decisions

Apply the UI and everyday note-taking interactions observed in the installed Antinote 2.1.3 app to BulletNotes. The user selected both visual changes and scratchpad/navigation behavior, then chose automatic SilverBullet synchronization with manual deletion. The proposed design was approved with "ok".

The result is a compact writing surface with controls that appear on hover, an ordered scratchpad stack, and access to the full SilverBullet space through search. This specification supersedes the original design's visit-history behavior for the visible previous/next controls and their shortcuts.

The scratchpad stack contains Markdown pages below the exact `Quick Notes/` folder in the active connection. A page named `Quick Notes.md` is an ordinary page. Existing matching notes join the stack without being renamed or rewriting their contents. Ordinary pages remain accessible through All pages.

The alternative of placing every SilverBullet page in the stack was rejected in favor of keeping quick capture navigation short. Keeping scratchpads local until explicit export was also considered; the user chose automatic synchronization.

## Reference observations

Antinote was opened and inspected directly using focused screenshots, accessibility control labels, menu commands, and its Notes and Appearance settings. Observations used by this design:

- Rounded paper fills the note window, with monospace text and subtle paper patterns.
- The header reveals native window buttons plus Settings, Search Notes, New Note, and Pin.
- The footer is an inset rounded capsule containing Delete, note date, Previous, Next, Jump to First, and Quick Export controls.
- The visible navigation shortcuts are Command+[ and Command+]; jumping to the first note uses Command+1. New note uses Command+N, Settings uses Command+comma, and Pin uses Command+P.
- A small accent dot marks the most recent note even while the footer is hidden.
- Search replaces the writing surface and offers note previews. Settings group related controls and place font/paper controls before the theme collection.

Antinote also exposes fixed slotted notes, split view, archive, locking, exports, and expiry settings. The approved change adopts the scratchpad stack and the controls described below; those additional subsystems have no implementation requirement in this specification. The user explicitly chose manual deletion.

The BulletNotes baseline was built and captured with the existing `ui_review` fixture using generated notes and a temporary database. That fixture avoids the user's normal store and automatic sync.

## Window and editing surface

Retain the existing resizable native window and its 505 by 552 logical-pixel default, with a 400 by 440 minimum. Use the observed Antinote proportions within those supported dimensions: rounded paper, a quiet monospace editor, generous writing margins, circular icon controls, and an inset capsule footer.

The header places settings and all-page search after the native traffic lights, with New Note and Pin at the right. Dragging remains available in unused header space. The full SilverBullet path stays available as accessible metadata and a compact secondary label where it fits; it must not crowd out the primary actions at minimum width.

The footer contains recoverable Delete, compact date/position metadata, Previous, Next, Jump to Newest, and an entry to Changes/recovery. Save and sync state remains textual and accessible, with failures visible when action is required. A newest-note accent dot is visible on the paper when the current note is first in the stack. Its accessible description supplies the meaning without relying on color.

Header and footer reveal independently on pointer proximity or keyboard focus, and hide after leaving their interaction regions. Revealing either overlay never moves, rewraps, or replaces editor text. Hover regions and native traffic-light positioning must match the final geometry. Selection dragging must not turn into window dragging or note navigation.

F6 continues to cycle through header, footer, and the active view; Escape returns to the editor or closes the auxiliary view. Reduced motion, increased contrast, and keyboard focus indicators remain supported. Every icon has an accessible name and a visible hover/focus treatment.

## Scratchpad lifecycle and durable order

Command+N immediately opens a blank scratchpad, focuses the editor, and schedules its initial local commit using the existing guarded save machinery. Its existing generated `Quick Notes/YYYY-MM-DD/HHmmss-<random-id>.md` identity stays stable through retries and restart. Nonempty notes synchronize automatically through the existing outbox; empty drafts remain local under the existing policy.

The stack is scoped to the active connection and ordered newest first by creation, independently of visits, search result order, text edits, and sync acknowledgements. Locally created scratchpads enter at the front. Persist the ordering metadata so restart and catalogue refresh cannot reorder existing entries. Notes with the same creation key use their full path as a deterministic tie breaker.

For existing generated paths, initialize creation ordering from their encoded date/time. For manually named or older scratchpads without that format, capture their available modification timestamp once on admission; use first local observation when that timestamp is unavailable. This fallback becomes durable and is never recomputed from later edits. There is no remote creation-time claim for these legacy pages.

New remote scratchpads join according to the same admission rule without changing the relative order of existing entries. Remote body downloads and acknowledgements cannot promote an old note. Stack metadata is local to the connection and does not add metadata to Markdown source.

All matching active pages count toward the stack, including catalogue-only pages. Opening an uncached entry uses the existing download flow; an offline failure leaves the current editor intact and explains that the destination requires a connection. Counts represent the known catalogue, with its existing completeness feedback available during indexing. Navigation must work beyond the first search-result batch.

Deleting uses the existing recoverable, conflict-aware Trash operation. A deleted/trashed page leaves the active stack. Restoring it reinstates its durable creation order. Deleting the current note selects the next older available note, falling back to the next newer note; when none remains, create a new blank scratchpad. Local save or delete failures retain the current buffer and expose the error.

## Navigation and shortcuts

Previous means the adjacent older scratchpad; Next means the adjacent newer scratchpad. At either end, the unavailable action is disabled. Command+[ and Command+] invoke those same actions. Command+1 opens the newest scratchpad; it is disabled when that destination is already current. Opening a note from search does not change stack order.

The footer shows the current stack position as `1 of N` for the newest entry. An ordinary SilverBullet page shows page metadata instead of a scratchpad position; Previous/Next are disabled there, while Jump to Newest returns to the stack. When there are no scratchpads, Jump to Newest creates a blank one through the normal local draft flow.

Command+K remains all-page search, Command+F remains find within the current note, Command+N creates a scratchpad, and Command+comma opens settings. Add Command+P for Pin. Existing Command+S, Command+W, Command+Q, and ordinary macOS editing shortcuts retain their established actions.

Each transition saves the outgoing note and editing position through the existing durability barrier. Reopening a note restores its selection and scroll position. Failed saves block a transition that would lose the buffer. Rapid navigation and stale asynchronous completions must not replace a newer selection. Connection changes must discard results and pending stack requests belonging to the old connection.

## Search and settings

Restyle All pages as a quieter search surface with a prominent query field, restrained preview rows, and compact modification metadata. Keep title/path distinctions, cached versus online-only availability, folder filtering, keyboard selection, and pagination. Escape restores the note's editing state.

Group settings so text size and paper style precede the long theme collection. Light/Dark/System assignment, existing built-in and imported palettes, the global shortcut, theme import/export, and connection access remain reachable at minimum window size. All settings use the existing atomic persistence and failed-save rollback behavior.

Paper patterns, font metrics, text selection colors, and toolbar colors consume the active semantic theme. Preserve source bytes and supported Slint text editing. Existing deferrals for native translucency and inline Markdown coloring continue to apply.

## Implementation boundaries

Use the current Rust/Slint architecture and existing maintained dependencies. The principal surfaces are `ui/app-window.slint`, `ui/components/note-editor.slint`, `ui/components/action-button.slint`, `ui/components/page-browser.slint`, `ui/components/appearance-panel.slint`, the icon assets, and `src/platform/window_chrome.rs`.

Implement stack order/query support in the existing SQLite worker/store boundary, with connection-scoped metadata and migration coverage if schema changes are needed. Stack reads must not depend on the search limit. Add coordinator actions/events and local UI bindings/presentation for stack navigation; reuse existing revision checks, transition barriers, recovery operations, and selection restoration.

No production database is opened or migrated during development verification. Use temporary databases and generated notes. Preserve the established migration backup/failure behavior for the eventual user upgrade.

Routine diagnostics record opaque identifiers, outcome categories, and timings at stack loading/navigation and persistence boundaries. They must not contain note bodies, titles, paths, or credentials.

## Acceptance and verification

1. Capture the generated-note fixture before and after with controls visible and hidden. Compare header/footer arrangement, circular controls, paper spacing, newest indicator, and minimum-size layouts in light and dark themes.
2. Verify independent hover reveal, F6/Escape focus, native traffic lights, drag/resize, and no editor geometry or source changes when controls appear. Exercise reduced motion and increased contrast.
3. Verify stack membership, creation order, tie breaking, legacy admission, new-note insertion, unchanged order after edits/visits/sync, and restart persistence using temporary stores.
4. Verify older/newer/newest destinations, disabled endpoints, position counts, ordinary-page behavior, empty stacks, uncached/offline entries, and more entries than one search batch.
5. Verify rapid requests, delayed/stale completions, dirty and failed saves, restored caret/scroll, connection isolation, recoverable deletion, and restoration without source loss.
6. Verify offline scratchpad creation, restart recovery, and subsequent guarded synchronization through the existing controlled server fixtures. Keep manual deletion recoverable and conflict-aware.
7. Verify search keyboard behavior, folder filtering/pagination, settings reachability, theme persistence, and failed settings-save rollback after restyling.
8. Run the repository formatting, strict Clippy, and regression checks plus the relevant native UI contracts. Report checks requiring actual VoiceOver, IME, or physical trackpad input separately if they are not exercised.

Success means the approved Antinote layout and scratchpad flows are demonstrated by screenshots and interaction checks, while local durability and guarded SilverBullet sync remain covered by regressions.
