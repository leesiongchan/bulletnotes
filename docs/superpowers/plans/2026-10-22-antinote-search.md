# AntiNote Search Preview Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Show literal match-centered search excerpts with visible highlighted hits and usable two-line result rows.

**Architecture:** SQLite FTS5 supplies snippets and match boundaries; Rust decodes them into plain strings plus explicit UTF-8 ranges. Extend the already maintained Slint `Text` renderer with validated decorations, retaining native wrapping/elision and noninteractive plain-text behavior.

**Tech Stack:** Rust, existing rusqlite/SQLite FTS5, uuid, Slint 1.18.1 private patch, existing search/UI fixtures.

**Spec:** [Approved design, Search previews section](../specs/2026-10-22-antinote-interaction-fidelity-design.md#search-previews).

## Global Constraints

- "Use a maximum of 32 tokens for a matching body snippet and display up to two wrapped preview lines, with an ellipsis when truncated."
- "Render note text literally and add only the supplied match emphasis."
- "Search improvements do not reorder scratchpads or download uncached bodies merely to build a preview."
- Keep existing query semantics, connection/folder filters, visit order, pagination, cache state and generation checks.
- Keep the existing `title,path,body` index and `unicode61 remove_diacritics 0` tokenizer; no migration, dependency upgrade or new package.
- Use temporary FTS fixtures, generated text, Rust 1.99.1 and the authoritative Slint patch.

## Review Focus

- Note text containing generated marker delimiters must survive decoding literally: S1 collision/malformed-marker tests.
- Unicode, unsorted or invalid byte ranges must render plain text safely: S1/S2 validation tests.
- A late match, long token, or narrow window must still show a highlighted hit: S1 trimming and S2 layout tests.
- Changing query/folder during pagination must not attach stale highlights or preserve an invalid selected index: S2 integration tests.
- Markdown/HTML/URLs in excerpts must not create formatting or actions beyond supplied match emphasis: S2 literal-rendering tests.

---

## File Structure

- `src/store/search.rs`: FTS context retrieval and search-only `SearchHighlights` metadata.
- NEW `src/store/search/highlights.rs`: marker creation/decoding and plain excerpt windowing, kept independent from SQL.
- `src/app/local_ui/presentation.rs`: validated ranges to decoration models.
- `ui/types.slint`, `ui/components/page-browser.slint`: default-empty highlight models and consistent 112px result-row geometry.
- Authoritative Slint patch: ordinary `Text.text-decorations`; reuse existing `TextInputDecoration` and renderer machinery.
- Existing FTS/pagination tests plus NEW literal-rendering tests and generated native example.

`CORE` and `COMPILER` below mean the prepared `i-slint-core-1.18.1` and `i-slint-compiler-1.18.1` folders under `target/slint-source-highlighting/crates/`. Retain changes to them only through `patches/slint/slint-1.18.1.patch`.

### Task S1: Match-Centered FTS Data with Explicit Ranges

**Files:** Modify `src/store/search.rs`, `tests/store_search.rs`, `tests/page_pagination.rs`; create `src/store/search/highlights.rs`. Update other `PageSummary` constructors found by `rg -n 'PageSummary' src tests examples benches` only to supply default-empty metadata.

**Interfaces (NEW):**

```rust
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SearchHighlights {
    pub title: Vec<std::ops::Range<usize>>,
    pub path: Vec<std::ops::Range<usize>>,
    pub excerpt: Vec<std::ops::Range<usize>>,
}
// Add to existing PageSummary; retain existing String/Option<String> fields:
pub highlights: SearchHighlights;
// Private helpers in search/highlights.rs:
pub(super) fn markers(title:&str, path:&str, body:Option<&str>) -> (String,String);
pub(super) fn decode(marked:&str, open:&str, close:&str)
    -> Option<(String,Vec<std::ops::Range<usize>>)>;
pub(super) fn visible_excerpt(text:String, matches:Vec<std::ops::Range<usize>>)
    -> (String,Vec<std::ops::Range<usize>>);
```

- [ ] **Step 1: Write failing FTS/data tests.** Add `late_body_match_is_visible_in_excerpt`, `title_only_hit_keeps_body_beginning`, `literal_marker_text_survives`, and `online_only_hit_has_no_body_excerpt` to `tests/store_search.rs`. Use the existing temp-store fixture. Save `format!("{}needle café", "unrelated ".repeat(80))`, search `needle`, and assert:

```rust
assert!(!item.excerpt.as_deref().unwrap().starts_with("unrelated "));
assert_eq!(&item.excerpt.as_ref().unwrap()[item.highlights.excerpt[0].clone()], "needle");
assert!(item.highlights.excerpt[0].start <= "…".len());
```

  Add `decode_rejects_unmatched_or_nested_markers` with `assert!(decode("a<o>b","<o>","<c>").is_none())` and a nested-pair case. Test original-case FTS text, phrase matching, café/CJK/emoji byte boundaries, an existing literal UUID-like marker string, title/path-only hits, empty query, and null body. Assert valid sorted nonoverlapping ranges and unchanged scope/order/count/pagination behavior. Existing recovery/page fixtures use default highlights.

- [ ] **Step 2: Run red:** `rustup run 1.99.1 cargo test --locked --test store_search --test page_pagination`; expect missing metadata or the existing beginning-only excerpt to fail the late-match assertion.
- [ ] **Step 3: Implement context retrieval and decode.** Add row ID to the existing result SELECT without changing its predicate/order/limits. Only for nonempty queries, reuse one prepared per-row context statement:

```sql
SELECT highlight(page_fts,0,?1,?2), highlight(page_fts,1,?1,?2),
       snippet(page_fts,2,?1,?2,'…',32)
FROM page_fts WHERE rowid=?3 AND page_fts MATCH ?4
```

  Keep the existing literal quoted FTS phrase and SQL parameters. Use uuid to choose delimiters explicitly absent from title/path/body; do not rely on probability alone. Decode into exact plain text/ranges. Malformed metadata falls back to original literal title/path and cached-body beginning, with empty ranges. A title/path-only hit keeps the beginning excerpt; null body keeps `None`.

  For a body hit, trim leading snippet context through the first match start and prepend one ellipsis if anything was removed, then shift all retained match ranges. This places the first match at the first rendered line without guessing font widths; keep the remaining snippet tail for native two-line elision. Empty queries retain the current first-160-character excerpt and no ranges. Do not log text or run context queries for empty queries.
- [ ] **Step 4: Verify green:** run `--test store_search --test page_pagination --test sync_presentation`; require all cases and existing search contracts to pass. Run module tests for `store::search::highlights`. Inspect lockfile/migration diff: none is expected for S1.
- [ ] **Step 5: Commit** as `Return match-centered FTS snippets and highlight ranges`.

### Task S2: Literal Decorated Text and Two-Line Search Rows

**Files:** Modify authoritative Slint patch, `ui/types.slint`, `ui/components/page-browser.slint`, `src/app/local_ui/presentation.rs`, `src/app/recovery_ui.rs`, `examples/ui_review.rs`; create `tests/search_preview.rs`, `prototypes/slint-source-highlighting/tests/text_decorations.rs`, and `examples/search_preview_review.rs`. Modify prototype `probe.slint` for literal Text testing. Update any remaining PageRow constructors with default-empty new fields.

**Interfaces (NEW):**

- Private patched `Text.in property <[TextInputDecoration]> text-decorations`, default empty; ordinary Text remains `PlainOrStyledText::Plain`.
- PageRow fields `title-highlights`, `path-highlights`, `excerpt-highlights`: `[TextInputDecoration]`, default empty.
- `src/app/local_ui/presentation.rs::search_decorations(text:&str,ranges:&[Range<usize>],ink:slint::Color)->Vec<TextInputDecoration>` validates the entire range list; malformed metadata returns empty. Each match uses ink plus bold/underline; no code or link styling.
- PageBrowser `result-row-height: length = 112px`, passed to ResultRow and used by both keyboard scroll calculations. Preview height 30px, date y85px; title/path/cache labels retain existing positions. Excerpts wrap with `max-lines:2; overflow:elide`.

- [ ] **Step 1: Write failing renderer/UI tests.** In prototype `text_decorations.rs`, add `ordinary_text_decorations_are_literal_and_optional`: snapshot `<b>needle</b> **text** https://example.invalid` with no model, then a declared `needle` range. Assert identical literal source/accessibility text, no active link, different emphasis pixels only with ranges, and identical pixels again with the empty model. In root `tests/search_preview.rs`, add `invalid_ranges_render_plain_text`, `row_keyboard_geometry_uses_112_pixels`, `stale_query_cannot_reuse_old_highlights`, and `two_line_preview_starts_with_visible_match`.

  For reversed/out-of-bounds/split-UTF-8/unsorted/overlapping ranges assert `search_decorations(...).is_empty()` in presentation unit tests. For valid `"café needle"` range `6..12`, assert one decoration with `start==6`, `end==12`, `bold` and `underline`. Recovery rows must have zero highlight decorations. Resize 505×552 → 400×440 and verify the selected result remains visible with 112px rows and a highlighted body token in the preview area.

- [ ] **Step 2: Run red:** prototype `cargo test --locked --test text_decorations` and root `cargo test --locked --test search_preview`, through Rust 1.99.1. Expect missing `Text.text-decorations`, models or old 94px geometry.
- [ ] **Step 3: Implement literal rendering and mapping.** Patch `COMPILER/builtin_elements.rs`'s `ComplexText:SimpleText` declaration, `CORE/items/text.rs::ComplexText`, and its `RenderString::text_decorations` implementation. Reuse the existing decoration range/style conversion; extract only the shared validation, leaving TextInput preedit/marker transformations in place. Existing shared-Parley shaping already consumes generic decorations. Preserve native wrap/max-lines/elide, ordinary font weights, and the empty-model output. Do not route excerpts through Markdown/HTML/StyledText parsing or editable/focusable TextInput.

  Convert S1 ranges into models, refresh when results or the effective theme/contrast policy change, and keep recovery defaults empty. Fix the generated online-only fixture's fabricated body excerpt to empty. Update ResultRow and all row-index scrolling calculations to 112px. Reset an out-of-range selection on result replacement and retain existing generation/pagination behavior. Keep accessible labels and a single row activation path; preview text must not consume clicks or keyboard focus.

  NEW `examples/search_preview_review.rs::run()->Result<(),Box<dyn std::error::Error>>` mounts real generated result rows at both sizes and representative light/dark/high-contrast settings. Check literal text, supplied match emphasis, two-line elision, long tokens/CJK/emoji, arrow/Return/Escape, folder/pagination controls, and selected-row visibility after resize/theme change. Capture owned windows and emit NEW `native_search_preview_passed: true` only after all assertions.
- [ ] **Step 4: Verify green:** reprepare fresh pinned sources, run prototype text-decoration/marker/undo tests, root search-preview/store-search/pagination/UI-contract tests, and build/run `search_preview_review` with native-validation. Inspect captures at both sizes and require visible highlighted text, no active markup/links, and no overlap. Then run `python3 scripts/check-markdown.py` to prove the Text extension did not regress editor decoration/undo rendering.
- [ ] **Step 5: Commit** as `Render literal highlighted search previews`.

Continue with [integrated verification F1](2026-10-22-antinote-interaction-fidelity.md#task-f1-integrated-evidence-and-handoff).

## Verified Reference Boundaries

The public StyledText API does not expose arbitrary range construction, and its renderer hardcodes clipping. Ordinary Text already supports native wrapping, max-lines and elision while staying literal; the decoration property above is a deliberate NEW private extension. FTS behavior follows [SQLite's official auxiliary functions](https://www.sqlite.org/fts5.html#the_snippet_function).
