# AntiNote-style search and note management

Status: approved by the user on 2026-10-31; implementation planning authorized.

## Intent and agreed direction

Make Bulletnotes search closely follow AntiNote's content-first browsing and note-management experience. The user chose one search browser rather than a separate scratchpad browser. List/card views, selection, batch Trash, scratchpad promotion, and scratchpad reordering belong in that browser. Existing SilverBullet pages remain searchable with their paths, folder filtering, and offline availability visible.

This is a project-sized change to UI, local ordering, search, and recovery operations. It builds on the native Rust/Slint app and existing SQLite worker/coordinator; it does not introduce a web UI or a second storage service.

## Reference and existing behavior

Studied the installed AntiNote 2.1.3 through macOS accessibility and window captures. Observed a rounded query field with an Escape hint; content previews and edited dates; colored highlights; always-visible promote and Trash actions; bottom Select, scope, and list/card controls; miniature source previews in cards; and a no-results state offering to create a note from the query. Searching inside a word returned results. Escape closed search in the observed session, and reopening cleared the query. Do not claim the complete keyboard and drag behavior was exercised: drag reordering, promotion, deletion, and bulk mutation were deliberately not performed on the user's notes.

Official references describe click/Return to open, Shift+Return to promote, drag reordering only with an empty query, and dragging a selected block:

- https://antinote.io/user-manual/search
- https://antinote.io/updates/antinote-2-0

The manual describes staged Escape behavior that differs from the observed installed app. Bulletnotes' chosen behavior below is explicit.

Current Bulletnotes global search uses literal FTS token phrases, folder filtering, visit order, 200-result pages, match excerpts, and request generations that reject stale responses. The current note editor remains mounted behind auxiliary views. Scratchpads have durable creation order; browsing and editing do not change it. Trash validates revisions and permissions, retains recoverable contents, and uses the existing sync outbox.

No user note text or screenshots containing personal notes will be added to the repository. Verification uses synthetic notes.

## Approaches

1. **One search browser — selected.** Extend the current browser with AntiNote's layout and controls. Scratchpads is a scope within it; All Pages retains SilverBullet navigation. This reuses search, opening, and recovery while requiring scope-aware ordering and actions.
2. **Separate scratchpad browser — not selected.** Copy AntiNote for scratchpads and retain the existing all-page search separately. This simplifies the scratchpad model but duplicates browsing surfaces and makes finding notes depend on choosing the right entry point.

## Proposed experience

The following details were proposed by the agent and accepted with approval of this document.

### Layout and display

- Replace the prominent heading and stacked form layout with AntiNote's search-first composition: a rounded query input at the top, results in the middle, and a stable bottom toolbar.
- Keep the folder filter in a compact secondary row in All Pages. Scratchpads uses the complete scratchpad scope without an additional folder filter; preserve the All Pages folder value when switching back.
- Bottom controls: Select, Scratchpads / All Pages, and mutually exclusive list/card buttons. Show selected count and Promote selected / Move selected to Trash while selecting. Promotion appears only in Scratchpads.
- List results emphasize note content, using literal Markdown previews with line breaks condensed as in AntiNote. Keep the page title/path as compact identification where needed, plus edited date and availability. Do not duplicate a first-line title unnecessarily.
- Cards form a responsive grid of miniature multiline source previews, with edited date and action buttons. Use one column in narrow windows and additional columns when readable. Do not render interactive Markdown or load full uncached bodies to populate previews.
- Use existing theme colors, typography, icons, focus treatment, and Slint controls. Match AntiNote's spacing and hierarchy closely without adding native Liquid Glass or a new theme system. Match highlights use a contrasting theme-derived background; retain readable selected/focused states.
- Remember list/card preference across restarts through existing settings persistence, with list as the backward-compatible default. Initial scope is All Pages; retain the chosen scope while the app runs. Closing and reopening search clears the query and selection and focuses the input; preserve the folder value.
- Keep index completeness and pagination visible in a compact status area without crowding the toolbar. Empty results show “Nothing found” and an action to create a scratchpad starting with the literal query. Blank queries never create a note through Return.

### Searching and opening

- Query filters immediately in both scopes. Match literal, case-insensitive substrings in title, path, and cached body, including partial words; do not interpret regex, SQL wildcards, or FTS operators. This deliberately changes existing FTS phrase semantics to follow AntiNote more closely. Highlight and excerpt generation must use the same matching semantics, with valid source offsets for Unicode. Accent folding and fuzzy ranking are outside scope.
- Search only the active connection and exclude trashed or remotely deleted pages. Uncached pages can match metadata; body matching becomes available as indexing completes. Never invent a preview or imply an incomplete index is complete.
- All Pages keeps visit-descending ordering and path tie-breaking. Scratchpads uses durable stack order for blank and nonblank queries. Searching, opening, editing, and sync never implicitly promote a scratchpad.
- Keep bounded result batches and More pages. A changed query/scope/folder resets pagination, selection, and scroll; stale results cannot replace the latest results. Heavy matching runs on the existing worker, not the UI thread.
- Keep Cmd/Ctrl+K for global search and Cmd/Ctrl+F for in-note find. Arrow keys navigate results; list navigation follows rows and grid navigation follows visual rows/columns. Return opens the focused result, or the first result from the query field. In select mode, Space toggles a result and Return remains an explicit open action.
- Click opens outside select mode; click toggles membership inside select mode. Row action buttons never trigger opening or selection as a side effect. Tab reaches query, available filters, results, actions, and footer with visible focus and accessible names.
- Escape first cancels an active drag, otherwise exits selection mode, otherwise closes search and clears its query. Respect IME composition before handling Escape/Return. Retain the mounted editor, its selection, scroll, and save guards.
- Clicking Create or pressing Return from the query field with zero results creates a normal scratchpad with the query as body content, saves it through existing draft handling, and opens it. Folder filtering never changes its destination from Quick Notes.

### Scratchpad promotion and ordering

- Show Promote to front beside scratchpad results and enable Shift+Return to promote the focused scratchpad and open it. Row and batch promotion keep search open; they update results and the footer stack after durable success.
- Drag anywhere on a result except controls to reorder in Scratchpads, in either view. A movement threshold distinguishes a click from a drag. Provide a visible insertion target and edge autoscrolling.
- Allow drag only with an empty query in Scratchpads. A search query or All Pages disables dragging and provides a short explanation when relevant. All Pages never offers a synthetic global stack.
- In selection mode, dragging a selected scratchpad moves the selected loaded notes together, preserving their current stack order. Promoting a selection likewise preserves relative order. Unselected notes preserve their relative order.
- Selection covers explicitly checked, loaded results only; there is no implicit select-all across unseen result pages. Loading more preserves selected keys. Query/scope/folder changes or a connection switch clear selection; refresh removes keys no longer eligible.
- Persist explicit ordering per connection using the existing scratchpad order metadata; retain immutable creation timestamps. Existing users retain their current order until they explicitly rearrange. New scratchpads enter at the front; Older/Newer and Jump to Newest traverse the resulting manual stack order. Update the usage guide's creation-order guarantee accordingly.
- Ordering is local and survives restart; it does not rename files, alter bodies, or sync ordering into SilverBullet. Read-only scratchpads may be rearranged locally because this does not edit server content. Store changes are transactional; failure retains the old order and reports an actionable notice. A stale drag after sync/deletion is rejected and refreshed rather than silently applying to another list.
- Pagination is not an ordering boundary: a drop is relative to a visible target key in the complete durable stack, preserving unloaded entries. Promotion always means the front of the complete stack.

### Trash and batch actions

- Individual and batch Move to Trash work in both scopes through the existing recoverable Trash and sync behavior. Do not add permanent deletion to search.
- Disable Trash for read-only, conflicting, or uncached pages, with a reason. Uncached pages must be opened/downloaded before trashing so the recovery snapshot has the full note; do not silently fetch a batch. Revalidate current eligibility and revisions at mutation time.
- Batch Trash shows one confirmation containing the count and explaining recoverability and existing server sync behavior. It acts only on the selected keys shown in that confirmation. An ineligible selection disables the batch action and identifies the reason rather than silently skipping notes.
- Flush any affected open editor before mutation and keep its buffer if saving fails. Execute a batch atomically in one worker transaction after eligibility/revision checks; any stale or invalid member aborts the whole batch with no deletions. Reuse the existing snapshot/outbox logic without bypassing its safeguards.
- Keep search open after Trash and refresh results and stack. If the mounted editor's note was trashed, choose a surviving scratchpad neighbor using existing recovery navigation, or a fresh blank if necessary. Returning from search must never resurrect a trashed buffer. Retain selection on failure for retry; remove successfully trashed keys on success.

## Boundaries and implementation discretion

No slotted notes, expiry, archive, export, replacement of in-note find, cross-connection actions, synchronized ordering, or unrelated editor/theme redesign. No new runtime dependency is expected; if one becomes necessary, verify it against an official current release before proposing it.

The builder chooses component decomposition, gesture implementation, exact responsive dimensions, storage query strategy, and any compatible migration needed for explicit ordering/settings. Reuse current settings serialization and migration conventions; opening an older database must preserve all note text, revisions, pending sync operations, and order. Do not rewrite previous migration files. The substring-search strategy must be measured with synthetic large catalogues; avoid an unbounded UI-thread scan or truncation that changes which notes can match. If acceptable latency cannot be achieved within this scope, bring the measured tradeoff back rather than silently retaining different matching semantics.

Performance acceptance is an agent-proposed engineering target: on the development Mac, use 10,000 cached notes averaging 4 KiB of body text, with mixed ASCII/Unicode, short and long queries, common and rare matches, and no matches. Across at least 100 warmed queries, target p95 at most 150 ms from issuing the latest query to presenting its first result batch, including exact count and highlights. Report machine, corpus, and timing method. Also measure 50,000 notes as a stress case without promising the same latency. Superseded queries must not accumulate ahead of local saves; keep the existing durable save path responsive under continuous typing. These are implementation acceptance targets, not published product guarantees.

Use existing tracing at operation boundaries and failures with counts, scope, and request IDs; never log note bodies, search queries, tokens, or secret-bearing paths. This work does not require inspecting the user's live note database.

## Verification and acceptance

1. Search tests cover middle-of-word and case-insensitive matching, literal punctuation, Unicode highlight offsets, folder/connection/scope isolation, uncached metadata, empty queries, and result pagination. Measure typing/query responsiveness with synthetic large catalogues and record dataset size and timings.
2. Ordering tests cover upgrade preservation, restart persistence, creation at front, single/block moves and promotions, stable untouched order, pagination targets, connection isolation, and stale mutations. Navigation follows explicit order without visits/edits/sync reordering it.
3. Recovery tests cover single/batch Trash, full snapshot restoration, atomic rejection on stale revisions or permissions, uncached/conflicting rejection, failed local saves, pending outbox behavior, and deletion of the mounted editor's note.
4. Slint interaction tests cover list/grid movement and reveal, query focus, click versus drag, action propagation, select mode, no-result creation, remembered view, staged Escape, IME, and stale search results. Existing search/navigation/recovery tests must remain passing or be updated only for the deliberately changed contracts.
5. Capture before/after Bulletnotes windows with synthetic notes at narrow and wide sizes in light and dark themes. Compare against the studied AntiNote hierarchy; verify readable previews/highlights, unobscured bottom controls, no overlap/clipping, keyboard focus, insertion feedback, and uncached/empty/error states. Never commit live AntiNote note captures.
6. Run project formatting, targeted tests, the normal test suite, and relevant lint/build checks. Report commands and actual outcomes; visual inspection alone is not completion evidence.

## Approval

The builder review identified one missing decision: a measurable workload and latency target for substring search. The engineering target above resolves it as an agent proposal. No remaining product question was identified by that review.

The user approved this design, including substring matching, unchanged shortcut assignments, local manual scratchpad order, and guarded batch Trash. Proceed to implementation planning.
