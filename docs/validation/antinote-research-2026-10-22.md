# AntiNote UI and interaction research

Date: 22 October 2026. Research baseline: Bulletnotes `52e1201` and installed
Antinote 2.1.3 (`com.chabomakers.Antinote`). This records observations and
candidate work; it is not an approved implementation specification.

## Intent and existing decisions

The request is to study the installed app carefully, exercise its UI, and apply
the useful findings to Bulletnotes. The existing
[approved design](../superpowers/specs/2026-10-19-antinote-ui-ux-design.md)
already adopts the paper window, independent hover controls, capsule footer,
scratchpad stack, search, and appearance controls. That work is on the current
branch. Later Markdown and editor geometry work is also present.

The earlier choices remain the starting point: automatic SilverBullet sync,
manual recoverable deletion, and a durable creation-ordered `Quick Notes/`
stack. Split view, slotted notes, archive, locking, exports, and expiry were
previously deferred. No new scope decision was received during this research.

## Method and coverage

Opened the installed native app through LaunchServices. Inspected menus and
accessibility controls; visited all eleven settings tabs; created one disposable
note; exercised editing, search, pinning, view switches, deletion, and recovery.
Temporarily resized the note from 324 × 369 to 505 × 552 logical pixels for
comparison. Restored its original size, position, and unpinned state afterward.
Only the generated note was edited or deleted. It was restored to verify
recovery, then returned to The Void after testing.

Initial global keyboard automation reached the chat instead of the app.
Subsequent keyboard events targeted AntiNote's PID. The confirmed checkbox
click required checking that AntiNote owned the window at the clicked point.
Early captures taken during fades were superseded by settled hover captures.

Bulletnotes comparisons used the existing `ui_review` executable with generated
content, a temporary SQLite store, and no SilverBullet connection. Native
captures reported visible rendered windows using Metal. The reference and
Bulletnotes note bodies differ, so these are layout observations, not pixel
identity tests.

This is broad UI coverage, not exhaustive behavioral verification. Physical
trackpad gestures, real VoiceOver operation, retention timers, biometric locks,
iCloud conflicts, external exports, OCR, extension execution, and backup restore
were not tested. Settings inventory does not establish those features work.

## Confirmed interactions

| Interaction | Observed result |
| --- | --- |
| New note | File → Create New Note produced an empty editor. |
| Markdown word formatting | Command+B wrapped the word at the caret with `**`; repeating removed the markers. |
| Checkbox continuation | Return at the end of a task inserted another `- [ ] ` prefix. |
| Checkbox click and undo | Clicking the generated task changed `[ ]` to `[x]`; one Command+Z restored `[ ]`. |
| Heading shortcut | Command+Shift+H inserted a heading marker on the current line. |
| Line movement | Option+Up moved the current line above its neighbor. |
| Independent hover | Header and footer could be revealed separately; hidden chrome left the paper and newest-note dot visible. |
| Pin | Accessibility action toggled pinning and produced pinned/unpinned feedback. |
| Search result | A short term and the generated date marker found the disposable note; results included a preview, relative modification date, promote, and delete. |
| Search cards | Switched to cards, inspected the generated note miniature, and restored list mode. |
| Search keyboard open | Down/Return opened the selected result; opening the current note produced feedback. |
| Empty search results | A nonmatching query exposed an explicit action to create a note from the query. |
| Escape | Escape closed search with a nonempty query in this installed configuration. |
| Find and Replace | Opened its separate bottom bar with query, matching options, hit count, previous/next, Done, and a collapsed replacement control; replacement was not executed. |
| Split / slotted views | Toggled split view and observed two editors; toggled slotted view and returned without editing slots. |
| Recoverable deletion | Deleted the generated note, found its marked entry in The Void, restored only that entry, and opened it from search. |

The complete hyphenated review identifier did not find the note; `First` and
`20261022` did. The matching cause was not investigated. Do not copy this
behavior as a search requirement. Likewise, do not prescribe a two-stage Escape
interaction based only on the online manual: the installed behavior differed.

## Visual evidence

Only generated note content is included in these captures.

| Surface | Evidence and observation |
| --- | --- |
| Reference header | [AntiNote](antinote-research-2026-10-22/reference-header.png): paper-colored backdrop, circular controls, no centered page path. |
| Reference footer | [AntiNote](antinote-research-2026-10-22/reference-footer.png): inset capsule, centered date, navigation and quick export. |
| Reference cards | [AntiNote](antinote-research-2026-10-22/reference-search-card.png): query highlighting, multiline miniature, inline actions, view controls at bottom. |
| Current chrome | [Bulletnotes](antinote-research-2026-10-22/bulletnotes-chrome.png): tinted solid header/footer, prominent circular borders, centered path, two metadata lines. |
| Current search | [Bulletnotes](antinote-research-2026-10-22/bulletnotes-search.png): explicit page title/path/cache state, folder field, one-line excerpt, modification date. |
| Current appearance | [Bulletnotes](antinote-research-2026-10-22/bulletnotes-appearance.png): text/paper controls above appearance assignment and theme cards, connection access below. |

Both apps overlay part of the first text line when the header is revealed.
The initial impression that AntiNote avoids overlap was incorrect. A proposed
change should preserve the already approved invariant that reveal does not
reflow text, rather than increase editor margins based on that impression.

## Settings inventory

| Tab | Inspected controls |
| --- | --- |
| General | Note/preference sync, launch and new-note behavior, language, Dock/menu presence, hide-on-blur, pin restoration, swipe enablement/sensitivity. |
| Appearance | Font sizes and doubling; blank/lined/dotted/mini-squared/squared paper; pattern strength; translucency; footer metadata and newest-dot switches; separate light/dark theme assignment and icon customization. |
| Editor | Indentation, system substitutions and spelling, Vim, automatic checkbox/bullet/number lists, checked-item behavior, copy rules, paste cleanup, links, code syntax themes, font and caret/RTL controls. |
| Notes | New-note and expiry policy, archive visibility, lock timing, recoverable trash, slots/split preferences, backup and migration controls. |
| Keywords | List, math, sum, average, count, code and timer triggers, their aliases and mode-specific options. |
| Shortcuts | Note/window navigation, formatting, line moves/duplication, task/list actions, read-only/archive/lock, export/print, and shortcut rebinding. |
| Timer | Quit/menu behavior, notification/fullscreen/sound controls and volume. |
| Export | Destination enablement, quick destination, keyword omission, title policy and whole-library export. |
| Extensions | Installed commands, reload/install controls, keychain-backed API key UI, logging and extension sync. No extension was installed or executed. |
| Privacy | Separate network switches for sync, features, extensions and updates. |
| Updates | Update and beta controls, support and reset actions. No update or reset was performed. |

## Gaps that matter to this project

1. **Editing behavior is the largest everyday gap.** Bulletnotes' Markdown module
   explicitly decorates source without editing it. Its editor forwards shortcuts
   to app navigation, but has no task-click action, list continuation, formatting
   shortcuts, or line movement. The drawn task boxes currently communicate an
   interaction the app does not implement.
2. **Chrome can be quieter.** The structure already matches. The remaining
   differences are the paper-versus-surface backdrop, icon treatment, visible
   path and metadata density. Normal save/sync feedback must remain accessible;
   actionable failures must remain visible.
3. **Search can give more useful context.** Matching excerpts/highlights and an
   explicit create-from-query action are candidates. Preserve full-space search,
   folder filtering, cache availability, keyboard use and pagination. AntiNote's
   promotion/reordering would conflict with the approved immutable creation order
   and needs a separate decision.
4. **Paper controls have smaller fidelity gaps.** Mini-grid and pattern strength
   are absent. Existing Menlo metrics, source preservation and theme assignments
   already have dedicated implementations and evidence.
5. **Additional subsystems are separate scope.** Slots/split, calculators, timers,
   extensions, OCR, locking and export should each earn their place through the
   intended Bulletnotes workflow; the reference's settings inventory alone is
   not a reason to add them all.

Relevant implementation boundaries are `ui/app-window.slint`,
`ui/components/note-editor.slint`, `ui/components/page-browser.slint`,
`ui/components/appearance-panel.slint`, `src/app/markdown.rs`,
`src/app/editor.rs`, and the maintained Slint patch. Editing commands need to
preserve source, directed selection, IME behavior, and one-step undo. Their
design must fit the existing editor and persistence boundary.

## Fresh validation and limitations

Four generated native captures succeeded: Tartan writing view and Sanrio
chrome, search, and appearance. The captures reported Metal rendering and
visible frames. No product code was changed and no full Rust suite was run.

The existing `target/debug/examples/ui_review --floating-contract` exited 101.
It reached visible rendering, independent hover checks, and the selection
replacement check. At `examples/support/floating_chrome.rs:338`, its first undo
expected the selection deletion to remain, but the actual source had already
returned to the complete original. The fixture still explicitly expects two
undo entries, whereas the later
[atomic replacement work](editor-geometry.md#atomic-selection-replacement-undo-follow-up)
intentionally changed replacement to one undo. This historical contract needs
updating before it can serve as a passing acceptance gate; it was not changed
as part of the research. Earlier captures in that contract also failed because
its hardcoded `.superpowers/sdd/2026-10-19-antinote-ui-ux/` directory is absent.
The standalone screenshot commands above do not use that directory.

## Candidate approaches for review

- **Interaction fidelity first (recommended):** implement core source editing
  and task/list behavior, then refine chrome. This addresses daily friction
  while retaining the existing sync and scratchpad design.
- **Visual refinement only:** focus on chrome, paper controls and search
  presentation. Smaller scope, but rendered checkboxes remain noninteractive.
- **Broader feature parity:** evaluate slots/split and utility modes as separate
  projects. This requires new persistence and interaction decisions and is not
  a single UI polish task.

The user subsequently selected interaction fidelity first and approved the
conversational design on 22 October. The
[written specification](../superpowers/specs/2026-10-22-antinote-interaction-fidelity-design.md)
records that scope and awaits user review before implementation planning.

## Official references

The [manual index](https://antinote.io/user-manual) supplied coverage topics.
The [list guide](https://antinote.io/user-manual/checklists),
[formatting guide](https://antinote.io/user-manual/formatting-shortcuts),
[search guide](https://antinote.io/user-manual/search), and
[navigation guide](https://antinote.io/user-manual/navigation) supplemented
direct testing. Documented capabilities were not counted as executed checks.
