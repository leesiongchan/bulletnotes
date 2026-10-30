# AntiNote Search Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Adapt Bulletnotes' existing search to AntiNote's list/card browsing and note management, with safe batch Trash and durable manual scratchpad order.

**Architecture:** Extend the native Slint PageBrowser and existing Rust coordinator/store pipeline. Keep selection and confirmation keyed by PageKey, apply order changes transactionally, and run batch Trash through the save/metadata transition barrier. Reuse the complete scratchpad stack for ordering independently of paginated search results.

**Tech Stack:** Rust 1.99.1, Slint 1.18.1 at the repository's pinned fork revision, bundled SQLite through rusqlite 0.40.2, Tokio, serde, tracing, and regex 1.13.1.

**Spec:** `docs/superpowers/specs/2026-10-31-antinote-search-design.md` (approved).

## Global Constraints

- One browser with Scratchpads and All Pages scopes; All Pages is the initial scope. Keep Cmd/Ctrl+K for global search and Cmd/Ctrl+F for in-note find.
- Matching is literal, case-insensitive substring matching, including partial words; no fuzzy matching, accent folding, interpreted regex, SQL wildcards, or FTS operators.
- Initial result limit is 200; keep More pages, exact result counts, index completeness, active-connection isolation, and honest uncached metadata.
- Promotion and drag reordering apply only to Scratchpads. Drag requires an empty query. Ordering is local per connection; preserve creation timestamps and all note text, paths, revisions, and sync operations.
- Selection covers explicitly checked, loaded results only. Query/scope/folder/connection changes clear it; pagination preserves it; refresh prunes missing keys.
- Batch Trash is atomic, recoverable, revision-checked, and guarded by durable saves. Read-only, uncached, conflicting, missing, and already-trashed notes are ineligible. Search remains open after row/batch management.
- The editor remains mounted. Preserve its source, selection, scroll, composition, save-failure recovery, and current lifecycle guards.
- Use existing themes and toolkit; no native Liquid Glass, new storage service, slotted notes, expiry, permanent deletion in search, or unrelated cleanup.
- Never write live AntiNote notes/captures into the repository or use the live note database for testing. Logs omit queries, bodies, credentials, and secret-bearing paths.
- Performance target: 10,000 cached notes averaging 4 KiB; at least 100 warmed queries; p95 at most 150 ms from latest query issue to first-batch presentation, including count/highlights. Report hardware and method. Measure 50,000 notes separately as stress, with no identical latency promise.
- Work in the existing isolated worktree. Before execution, load the worktree, execution, TDD, Rust, security, and verification skills as applicable. Do not modify historical migrations or vendor Slint.

## Review Focus

- A query such as `.*`, `%`, `_`, quotes, or a non-ASCII letter must match literally with valid original UTF-8 ranges, including one- and two-character queries (Task 1).
- A drag after sync changes the complete stack must fail safely; pagination must not cause hidden notes to move unexpectedly (Task 2).
- One stale member or an SQLite failure halfway through a batch must leave every source, Trash snapshot, and outbox operation unchanged (Task 3).
- Deleting the mounted editor's note must not let Return to note resurrect its buffer or select another deleted batch member; a failed save must preserve the buffer (Task 4).
- Appearance/window/hotkey settings saved alongside a view toggle must not overwrite one another; narrow layouts, selection, and action clicks must remain usable without accidental opening (Tasks 5–6).

## File Map

- `src/store/search.rs`: scope, literal matching, ordered/paged summaries, mutation eligibility.
- New `src/store/search/literal.rs`: private compiled literal matcher returning original byte ranges. `src/store/search/highlights.rs`: excerpt clipping and newline condensation, preserving ranges.
- `src/store/scratchpads.rs`: validate expected complete stack and perform stable block moves transactionally; existing schema is sufficient.
- `src/store/restoration.rs`: transaction-scoped Trash helper, preparation read, atomic batch transaction. `src/store/mod.rs`: async store interfaces.
- `src/app/recovery.rs`, `src/app/coordinator.rs`, `src/app/local_ui/effects.rs`: prepare/confirm Trash and order effects behind current barriers, tagged completions, mounted-editor recovery.
- New `src/app/search_management.rs`: focused pure selection/confirmation state and operation request types, exported by `src/app/mod.rs`; no alternate store owner.
- `src/app/local_ui/search.rs`, `bindings.rs`, `presentation.rs`, `synchronization.rs`: scope, callbacks, stale result handling, refreshed eligibility, operation status, create-from-query.
- `src/store/settings.rs`, `src/app/appearance.rs`, `appearance_ui.rs`, `ui.rs`: persisted list/card choice through the existing serialized metadata queue.
- `ui/types.slint`, `ui/app-window.slint`, `ui/components/page-browser.slint`; new `ui/components/search-result.slint` and `search-toolbar.slint`: result presentation, keyboard flow, selection and gestures. Add only missing list/grid/select icons in `assets/icons/`.
- Tests extend existing search, pagination, scratchpad, recovery, settings, and navigation fixtures; add `tests/search_management.rs`, `tests/search_performance.rs`, and `examples/capture_search.rs` for the feature's integration/performance/visual checks.
- `docs/usage.md` and `README.md`: search behavior and explicit ordering; preserve existing shortcuts.

## Task 1: Literal search, scopes, excerpts, and baseline performance

**Files:** Modify `Cargo.toml`, `Cargo.lock`, `src/store/search.rs`, `src/store/search/highlights.rs`, `src/app/local_ui/search.rs`; create `src/store/search/literal.rs`, `tests/search_performance.rs`; update `SearchQuery` and `PageSummary` construction sites found with `rg`, including `tests/store_search.rs`, `tests/search_preview.rs`, and `tests/page_pagination.rs`.

**Interfaces:**
- Add `SearchScope { AllPages, Scratchpads }` in `store::search`; add `scope: SearchScope` to `SearchQuery`.
- Add `local_revision: LocalRevision` and `conflicted: bool` to `PageSummary`; retain existing cached/permission fields for eligibility and existing highlights/excerpt fields for display.
- Private `LiteralQuery::new(text: &str) -> Result<Option<LiteralQuery>, StoreError>`, `is_match(&self, text: &str) -> bool`, and `ranges(&self, text: &str) -> Vec<Range<usize>>`. Whitespace-only queries return None; nonblank queries retain their literal whitespace.
- Private `excerpt(text: &str, matches: &[Range<usize>], max_chars: usize) -> (String, Vec<Range<usize>>)` and `condense(text: &str, matches: &[Range<usize>]) -> (String, Vec<Range<usize>>)`. Use a 320-character bounded source excerpt around the first body match; list display replaces line endings with ` | `, cards retain line breaks. Never truncate source before determining matches.

- [x] Capture the current browser with synthetic data before UI changes, following `examples/capture_product_themes.rs`; retain images in a temporary directory for Task 7.
- [x] Add failing `substring_search_matches_inside_words_and_unicode_case`, `punctuation_is_literal_including_short_queries`, and `scratchpad_scope_uses_stack_order` tests. Assert `eAdE` matches `Header`, `CAFÉ` matches `café`, Greek sigma variants follow Unicode simple folding, `%`, `_`, `.*`, and quotes only match themselves, and accent differences remain distinct. Assert all highlight ranges slice the matched original text and remain valid after excerpt clipping/CRLF condensation. Verify scope, folder, connection, uncached metadata, trash exclusion, exact counts, and offsets past the first 200 results.
- [x] Run `rustup run 1.99.1 cargo test --locked --test store_search --test page_pagination` and confirm new behavioral tests fail for existing token matching/missing scope. Check failures before editing implementation.
- [x] Promote the already-locked `regex` crate to direct dependency `regex = "=1.13.1"`, using `RegexBuilder::new(&regex::escape(text)).case_insensitive(true)` once per query. Do not hand-roll Unicode folding or interpret the user's input as regex. Refresh the lockfile's root dependency list without unrelated version changes. Use `is_match` to filter and compute `ranges` only for retained rows.
- [x] Stream the connection/folder/scope-filtered rows from SQLite in final display order, count all matches, and retain only the requested batch. All Pages keeps visit/path ordering; Scratchpads joins existing `scratchpad_order` and ignores the saved All Pages folder. Select conflict/revision facts in the same read. Preserve index progress and metadata-only behavior; do not retain the whole corpus in memory or add an index speculatively. Keep the existing latest-request watch channel so typing does not enqueue every query ahead of writes.
- [x] Run the targeted tests above plus `rustup run 1.99.1 cargo test --locked --test search_preview`. Preserve invalid-range/plain-source safety while updating deliberately changed FTS assertions.
- [x] Add an ignored release-only synthetic performance test using the real StoreHandle, Slint testing backend, and LocalUi binding. Generate the specified mixed corpus outside the timed region in a temporary store, then issue at least 100 query-to-presentation samples with exact counts and highlighted rows. Report p95 for 10k and 50k notes; additionally type rapidly while saving and assert the latest result and saved revision settle without an expanding search backlog. Run `rustup run 1.99.1 cargo test --release --locked --test search_performance -- --ignored --nocapture`. If the 10k p95 exceeds 150 ms, profile before changing strategy and bring a measured index/workload tradeoff back if it cannot be met; do not quietly change semantics or acceptance values.
- [x] Commit the passing search contract and benchmark with `feat: add scoped literal substring search`.

## Task 2: Durable stable block movement

**Files:** Modify `src/store/scratchpads.rs`, `src/store/mod.rs`, `tests/scratchpad_stack.rs`, and `tests/scratchpad_navigation.rs`.

**Interfaces:** Add `StoreHandle::move_scratchpads(&self, connection: &ConnectionId, expected: Vec<PageKey>, selected: Vec<PageKey>, before: Option<PageKey>) -> Result<Vec<ScratchpadEntry>, StoreError>`. `expected` is the complete stack, not loaded results. `before: None` means append; promotion targets the first unselected member. Reject a target within the moved block; the UI normalizes a drop there to a no-op.

- [x] Add failing tests `block_move_preserves_unloaded_entries_and_creation_dates`, `promotion_is_complete_stack_front`, `stale_order_or_cross_connection_move_is_atomic`, and `manual_order_survives_restart_and_new_note_enters_front`. Use a stack larger than 200; assert selected members retain stack order regardless of selection click order, untouched keys retain relative order, and bodies/paths/outbox are unchanged. Extend navigation assertions to the explicit order.
- [x] Run `rustup run 1.99.1 cargo test --locked --test scratchpad_stack --test scratchpad_navigation` and confirm the missing behavior fails.
- [x] Validate duplicates, membership, target, active connection, and full expected order inside one transaction. Derive the moved block from current stack order and assign distinct descending `order_ms` ranks for the new complete sequence; retain every `created_ms`. Use checked integer conversion. Return the durable complete stack, notify local changes after success, and trace only operation/count/result. Reuse the existing schema and existing admission-at-front behavior.
- [x] Run the same tests; retain the existing version-seven backfill, restore, and edit/visit/sync invariance tests. Also allow local reordering of read-only scratchpads and reject deleted/missing members.
- [x] Commit with `feat: persist explicit scratchpad stack order`.

## Task 3: Atomic recoverable batch Trash

**Files:** Modify `src/store/restoration.rs`, `src/store/mod.rs`, `tests/conflict_recovery.rs`, and `tests/store_recovery.rs`.

**Interfaces:** Add `StoreHandle::prepare_trash(&self, connection: &ConnectionId, keys: Vec<PageKey>) -> Result<Vec<(PageKey, LocalRevision)>, StoreError>` and `StoreHandle::move_many_to_trash(&self, connection: &ConnectionId, pages: Vec<(PageKey, LocalRevision)>) -> Result<Vec<TrashId>, StoreError>`. Preparation returns revision pins after save barriers; it never fetches uncached bodies or mutates data. Single-note Trash keeps its public interface.

- [x] Add failing `batch_trash_is_atomic_and_restores_every_full_snapshot`, `batch_rejects_stale_uncached_readonly_conflicting_or_cross_connection_member`, and `sqlite_failure_on_second_trash_rolls_back_first`. Inject an aborting SQLite trigger using the existing failed-transaction fixture pattern; compare sources, snapshot count, and pending outbox before/after. Test duplicate/empty requests as invalid rather than double deletion.
- [x] Run `rustup run 1.99.1 cargo test --locked --test conflict_recovery --test store_recovery` and inspect expected failures.
- [x] Extract the existing snapshot/outbox body into a private transaction-scoped helper. The single operation and batch each open their own one outer transaction and use that helper; batch prevalidates every member and revisions, then commits all mutations together. Preserve existing draft/rebase/conflict and in-flight sync handling. Notify local changes only after successful commit. Do not implement a batch as repeated public calls or nested transactions.
- [x] Run the same tests, including the existing `trash_survives_restart_with_source_base_and_delete_intent` and `deletion_waiting_on_a_put_uses_its_acknowledged_revision`. Test restart and restoration of all batch members.
- [x] Commit with `feat: add atomic batch move to Trash`.

## Task 4: Guarded search management and editor recovery

**Files:** Create `src/app/search_management.rs`, `tests/search_management.rs`; modify `src/app/mod.rs`, `coordinator.rs`, `recovery.rs`, `local_ui.rs`, `local_ui/effects.rs`, `local_ui/search.rs`, `local_ui/bindings.rs`, `local_ui/presentation.rs`, `local_ui/synchronization.rs`, and affected navigation/recovery tests.

**Interfaces:**
- `SearchManagement` owns a `HashSet<PageKey>` selection and an optional immutable prepared confirmation. Methods: `toggle(&mut self, key: PageKey)`, `clear(&mut self)`, `retain_loaded(&mut self, keys: &[PageKey])`, `selected(&self) -> &HashSet<PageKey>`. Pagination retains selection; a changed filter clears it before issuing the query. Keep operation request IDs separate from query generations.
- Extend `RecoveryIntent` with `PrepareTrash(Vec<PageKey>)` and `TrashMany(Vec<(PageKey, LocalRevision)>)`; change `RecoveryOutcome.affected` to `Vec<PageKey>` and add `prepared_trash: Option<Vec<(PageKey, LocalRevision)>>`. Update all construction/match sites.
- Add `AppAction::PrepareSearchTrash(Vec<PageKey>)`, `ConfirmSearchTrash(u64)`, `CancelSearchTrash`, `MoveSearchScratchpads { expected: Vec<PageKey>, selected: Vec<PageKey>, before: Option<PageKey>, open_after: Option<PageKey> }`, and `NewNoteFromSearch(String)`. The u64 confirmation ID pins the prepared revisions and connection in coordinator state. A cancelled or superseded confirmation is never reusable. Promotions capture `expected` when requested; a drag captures it when the gesture starts and preserves that snapshot through drop.
- Add request-tagged order effect/event carrying Task 2's complete expected stack and its returned stack. Only completion matching both request ID and active connection may affect UI. Reuse `Transition::Recovery` for preparation/execution so both pass the existing flush and metadata barriers; add transition variants only where needed for seeded-note creation and preserving the search-origin return view.

- [x] Add failing integration tests for selection across pagination/refresh/filter/connection changes, confirmation revision pinning after a dirty editor saves, cancellation, mutation-time stale rejection, failed save, and late completions. Assert editing after confirmation never silently refreshes its revision pins. Test that preparing Trash does not change data.
- [x] Add failing coordinator tests for batch deletion of current note plus adjacent notes, all scratchpads deleted, ordinary-page deletion, and an unrelated editor retained. Assert search stays visible, a surviving neighbor/blank replaces only a trashed editor, deleted history is removed, and a later navigation intent cancels stale follow-up work. Add `create_from_query_saves_literal_body_and_respects_navigation_guard`, including whitespace-only rejection and the normal Quick Notes destination even with a folder filter.
- [x] Run `rustup run 1.99.1 cargo test --locked --test search_management --test scratchpad_navigation --test app_navigation --test history_recovery` and check failures.
- [x] Implement preparation through the transition barrier, then present confirmation from the resulting revision pins. Confirmation count/explanation refers to exactly those keys; retain state on failure for retry. Execute through Task 3, account for all affected keys, choose successors excluding the entire batch, and preserve `View::AllPages` while replacing the mounted editor. Ordinary recovery keeps its existing navigation behavior.
- [x] Implement ordering through Task 2 with the complete expected stack supplied by the action, disabling gestures while the coordinator's stack is loading or a management mutation is pending. Refresh search/footer after durable success. Shift+Return uses `open_after`; row/batch promotion do not open. Guard all actions against connection changes and lifecycle shutdown. Implement seeded creation as one guarded transition, rather than dispatching NewNote and immediately editing a possibly old editor.
- [x] Run all tests named above plus `rustup run 1.99.1 cargo test --locked --test draft_admission --test editor_reconciliation --test macos_lifecycle`; log only request IDs, counts, scope, and failures.
- [x] Commit with `feat: coordinate search note management safely`.

## Task 5: Search-first list/card UI and persisted view

**Files:** Modify `ui/types.slint`, `ui/app-window.slint`, `ui/components/page-browser.slint`, `src/store/settings.rs`, `src/app/appearance.rs`, `appearance_ui.rs`, `ui.rs`, `local_ui/bindings.rs`, `local_ui/search.rs`, `local_ui/presentation.rs`, `tests/search_preview.rs`, and `tests/theme_selection.rs`; create `ui/components/search-result.slint`, `ui/components/search-toolbar.slint`, and missing toolbar SVG icons.

**Interfaces:**
- Add serde `SearchView { List, Cards }` with List default and `#[serde(default)] pub search_view: SearchView` on `AppSettings`. Extend `MetadataChange::SearchView(SearchView)` and `AppearanceUi::save_search_view(&self, view: SearchView) -> Result<(), StoreError>`. Include this metadata in appearance ticket rebasing so previews cannot overwrite it.
- Add AppWindow properties `search-scratchpads: bool`, `search-cards: bool`, `search-selecting: bool`, `search-selected-count: int`, `search-busy: bool`, `search-confirm-count: int`, `search-confirm-id: string`, `search-action-message: string`, `search-can-promote: bool`, and `search-can-trash: bool`. Confirmation ID is transported as text to avoid narrowing u64 into a Slint int; validate on the callback boundary.
- Add callbacks `search-scope-changed(bool)`, `search-view-changed(bool)`, `search-select-toggled()`, `search-selection-toggled(string)`, `search-promote(string, bool)`, `search-promote-selected()`, `search-trash(string)`, `search-trash-selected()`, `search-trash-confirmed(string)`, `search-trash-cancelled()`, and `search-create()`. Strings for notes use existing serialized PageKey transport; the bool promotion parameter is `open_after`.
- Extend `PageRow` with `checked: bool`, `can-promote: bool`, `can-trash: bool`, `trash-disabled-reason: string`, `list-excerpt: string`, and `list-excerpt-highlights: [TextInputDecoration]`; existing excerpt stays multiline for cards. Populate every eligibility field from current facts, not path-only assumptions.
- PageBrowser exposes `cancel-or-close()` to handle IME, confirmation cancellation, selection exit, and close in order; `focus-query()` resets row focus. AppWindow delegates page Escape to it before generic auxiliary-view handling.

- [x] Add failing settings tests that old JSON defaults to List, the view survives restart, and overlapping appearance/window/hotkey/connection commits preserve the chosen view. Add Slint tests for scope, grid/list keyboard navigation and reveal, action-event isolation, row checkbox behavior, create-state readiness, IME, Escape, and opening focus with an empty query.
- [x] Run `rustup run 1.99.1 cargo test --locked --test search_preview --test theme_selection --test search_management --test ui_contract` and check failures.
- [x] Implement the serialized preference patch and UI state transport. Use a search field at top (24px horizontal inset, 38px height), a compact All Pages folder row, middle results, and a fixed footer. At 400px window width, use one card column and stack selection actions onto a second footer row as necessary; at wider widths use columns with at least 220px available per card. Keep a compact status/More pages row above footer controls. Use consistent geometry values for pointer hit testing and keyboard reveal; do not hard-code old 112px scrolling arithmetic for cards.
- [x] Implement reusable source-preview result and toolbar components using existing ActionButton/ChoiceButton patterns. Content leads, path/title identify, edited date/availability remain readable, and row controls always stay visible. Reuse existing decoration `code` background for theme-derived monospace match emphasis; it supports original byte ranges without modifying the pinned Slint renderer. Keep invalid ranges plain. Never render active HTML/links from notes.
- [x] Wire scope, view, selection, promotion, Trash confirmation, and Create to Task 4. Hide promotion outside Scratchpads; disable Trash with eligibility reason. Readiness matters: never offer Create while search is pending, failed, or folder input invalid. Keep Ctrl/Cmd+F and Ctrl/Cmd+K behavior intact; empty-query Return opens a result if one exists and otherwise does nothing. Escape cancels confirmation first, then selection, then closes/clears query; Task 6 adds drag as the highest priority. Keep All Pages folder value when switching scope.
- [x] Run the same tests plus `rustup run 1.99.1 cargo test --locked --test note_find --test page_pagination`. Update only obsolete layout/contract assertions, retaining the tests' behavioral intent and invalid-Unicode safety.
- [x] Commit with `feat: add AntiNote-style search list and cards`.

## Task 6: Drag ordering and selected-block gestures

**Files:** Modify `ui/components/page-browser.slint`, `ui/components/search-result.slint`, `ui/app-window.slint`, `src/app/local_ui/bindings.rs`, and `tests/search_preview.rs` / `tests/search_management.rs`.

**Interfaces:** Add callback `search-reordered(string, bool)` to AppWindow, where the string is a serialized target PageKey (empty means append), and the bool indicates before/after the visible target. Resolve after-target to the next unselected key in the complete stack. Dragged members are captured when the drag begins and identified by PageKey, not changing indices; hand the resulting selected keys and normalized before target to Task 4. Add `search-drag-started(string)`, `search-drag-cancelled()`, and `search-drag-active: bool` for that capture and Escape handling.

- [x] Add failing pointer tests `click_threshold_does_not_reorder_or_swallow_open`, `controls_never_start_drag`, `selected_block_drag_preserves_stack_order`, `filtered_or_all_pages_drag_is_disabled`, and `grid_drag_reveals_insertion_and_autoscrolls`. Cover cross-row/cross-column moves, append, selected target no-op, Escape, stale query/connection/result changes, pagination, and outside-window cancellation.
- [x] Run `rustup run 1.99.1 cargo test --locked --test search_preview --test search_management` and inspect failures.
- [x] Use Slint's existing TouchArea/pointer and scroll primitives; a 6px movement threshold begins drag. Controls are excluded from the draggable region. In Scratchpads with an empty query, calculate insertion from the current list/grid geometry, show the target, and scroll a 24px edge zone using a 16ms timer with bounded movement. Stop the timer on cancel/drop/hide/filter/connection changes. Opening occurs only after release below threshold; a drag release must never open a note.
- [x] Resolve a selected block in full durable stack order, normalize a target within it to no-op, and pass the complete expected stack snapshot captured at drag start to the store request. Do not recapture a fresher expected order at drop: sync/reordering during the gesture must produce the stale-order rejection. Expose keyboard promotion as a non-drag path and explain why dragging is unavailable when filtering.
- [x] Run the same tests plus Task 2 ordering tests; inspect insertion feedback and autoscroll in both views at narrow and wide sizes.
- [x] Commit with `feat: support scratchpad drag ordering in search`.

## Task 7: Visual evidence, full verification, and usage documentation

**Files:** Create `examples/capture_search.rs`; update `docs/usage.md`, `README.md`, and this plan's checkboxes. Store generated images under a temporary output directory; use synthetic notes exclusively.

**Interfaces:** `rustup run 1.99.1 cargo run --locked --example capture_search -- <output-directory>` uses the existing Slint testing/Skia snapshot pattern. Render actual AppWindow states, not an HTML imitation. Document conversion of PAM snapshots to PNG using existing available tools.

- [ ] Render before/after comparison and new list/cards in light and dark themes at 400×440, 505×552, and 900×700. Include selected, no-result/create, uncached, invalid-folder, error, and confirmation states; inspect screenshots for overlap/clipping, source preview hierarchy, readable highlights, action placement, selected focus, and stable bottom controls. Describe concrete visual differences from the baseline and AntiNote reference. Perform a live pointer/keyboard smoke check with a temporary Bulletnotes store to verify dragging/autoscroll beyond synthetic tests.
- [ ] Rerun the Task 1 release benchmark after UI integration so the 150ms target covers final presentation, not just storage filtering. Record results, dataset, machine, and latest-query/save-backlog evidence; fix causes rather than weakening targets.
- [ ] Update usage docs for literal substring matching, Scratchpads/All Pages, list/cards preference, select and batch Trash, explicit local ordering, no-query drag restriction, Shift+Return promotion, creation from no results, and staged Escape. State that visit/edit/sync do not implicitly reorder; replace the old immutable-creation-order guarantee. Keep README shortcuts unchanged and adjust only relevant feature copy.
- [ ] Run `bash scripts/check.sh`: formatting, strict all-target/all-feature Clippy, and all-feature Rust library/bin/integration tests. Also run `rustup run 1.99.1 cargo test --doc --all-features --locked` and `git diff --check`. Read complete failure output and resolve defects. No packaging or external publication is needed for this feature.
- [ ] Use the requesting-code-review skill for one independent whole-change review (or the selected execution workflow's required reviews); address findings and rerun only affected checks when justified. Use verification-before-completion before reporting success. Record native Windows/Linux UI behavior as unverified if those hosts were unavailable.
- [ ] Commit with `docs: describe search browsing and note management` after checks pass. Report changed behavior, commands/results, screenshots, benchmark p95, and any material remaining limitation. Do not merge or publish without existing authorization.

## Research and implementation choices

Checked 2026-10-31. `regex` 1.13.1 is already in Cargo.lock; its official changelog lists it as the latest released version (1.13.2 is marked TBD). A direct pinned dependency avoids maintaining custom Unicode offset/folding code. Use Unicode simple case folding; e.g. sigma variants match, but `ss` need not match `ß`. The input is always escaped, so no user-defined regex feature is exposed.

- https://raw.githubusercontent.com/rust-lang/regex/master/CHANGELOG.md
- https://docs.rs/regex/latest/regex/struct.RegexBuilder.html
- https://docs.rs/regex/latest/regex/fn.escape.html
- https://rustsec.org/packages/regex.html — the listed historical parsing advisory is patched from 1.5.5; checked before proposing 1.13.1.
- https://www.sqlite.org/fts5.html — trigram MATCH does not match substrings shorter than three Unicode characters, so it is not a complete replacement for the required short-query semantics. Start with the bounded worker scan and benchmark before adding an index.

## Self-review

All approved design sections map to Tasks 1–7. Store signatures are shared verbatim between producers and consumers; selection/confirmation/drag requests are key-based and connection-tagged. The five Review Focus conditions have explicit test steps. No database migration is planned because mutable order metadata already exists; settings load uses an additive serde default. The direct regex dependency and exact GUI gesture thresholds are implementation decisions for plan review, not claims that the user chose a library or numeric thresholds.

## Execution handoff

Plan is ready for user review. Recommended execution: Native, because the tasks share coordinator/recovery/UI contracts and serial integration is simpler, followed by an independent whole-change review. Subagent-driven execution remains available if the user prefers per-task implementation and review gates. Product implementation has not started.

## Verification record

Development host: Apple M4 Pro, 48 GiB RAM, macOS. Functional verification used the real Slint Skia testing backend and temporary SQLite stores. Strict all-target/all-feature Clippy passes. The final complete all-feature suite passed 422 tests with 7 intentionally ignored tests, including all 15 preview tests and the real-binding management tests. Documentation tests passed (0 defined). A final whole-change review and its final suite result will be recorded below.

Captured 96 synthetic states under `/tmp/bulletnotes-search-final` (PAM and PNG): list/cards, browse/filter, selection, scratchpad batch controls, empty/create, confirmation, invalid folder, and errors; each in light/dark at 400×440, 505×552, and 900×700. Compared with the original browser captures in `/tmp/bulletnotes-search-before`. The heading-heavy form is replaced by a search-first field, content previews, compact path/date metadata, and a stable capsule toolbar. Geometry regressions now cover confirmation containment and complete card footers at minimum size. No personal note text or live AntiNote captures are tracked.

Native offline demo launch, partial-query preview, and pointer reordering were checked via accessibility and window captures. A native drag changed `[Reading, Project ideas, Today]` to `[Project ideas, Today, Reading]`; a read-only query of the temporary store confirmed exactly the same durable order. Automated native Command modifier events did not establish reliable shortcut reentry, so that native automation is not claimed as verification of modifier shortcuts; backend keyboard interaction tests cover the implemented flows. Native Windows/Linux and VoiceOver remain unverified on this macOS host.

The initial isolated release measurement was 99.91 ms p95 for 10,000 notes and 492.99 ms for 50,000 notes. One concurrent run alongside full tests measured 186.57 ms at 10,000 notes; the final benchmark is run separately to resolve contention and additionally asserts save/latest-result completion after 250 superseded queries. The isolated final run passed: 10,000-note p95 103.31 ms; 50,000-note stress p95 628.39 ms. Saving and presenting the final result after 250 superseded requests took 343.17 ms and 1519.42 ms respectively (both below the 5-second backlog guard).

Implementation ruling: scratchpad order mutations share the existing guarded recovery effect queue instead of introducing a second mutation channel. This keeps save barriers and request tagging consistent; incorrect completion routing would affect feedback/navigation, covered by coordinator and real-binding tests.
