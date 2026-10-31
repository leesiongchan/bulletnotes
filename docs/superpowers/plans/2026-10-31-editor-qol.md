# Editor QOL Implementation Plan

> **For agentic workers:** Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox syntax for tracking.

**Goal:** Implement all seven approved editor and chrome QOL changes.
**Architecture:** Keep the native Slint editor and existing persistence. Project typed sync facts into an accessible status component; share scrollbar behavior and typography. Tokenize source code locally without changing source.
**Tech Stack:** Rust 1.99.1, Slint 1.18.1 pinned fork, local fonts/SVGs, Syntect.
**Spec:** `docs/superpowers/specs/2026-10-31-editor-qol-design.md` (approved by user).

## Global Constraints

- Preserve source bytes, undo, selection, custom themes, paper settings and user work.
- Keep all existing network scheduling and guarded writes unchanged.
- Do not modify external checkouts; use accurate wrapped geometry for code backgrounds.
- Work within the existing checkout; leave implementation uncommitted for review.

## Review Focus

- New dirty revision while an old upload completes must never appear green (Task 2).
- Cancellation/reconnection must clear executing activity (Task 2).
- Hidden scrollbars must still accept edge hover/drag (Task 1).
- Unknown languages, Unicode and wrapped/empty code lines remain editable (Task 4).
- Narrow layouts and reduced-motion/high-contrast remain usable (Tasks 1, 3, 5).

### Task 1: Tray and shared scrollbar

**Files:** `ui/app-window.slint`, `ui/components/theme-scrollbar.slint`, tray SVG variants; `tests/editor_qol.rs`.
**Interfaces:** Existing tray `color-scheme`; shared scrollbar `content-y`.
- [x] Add rendered idle/scroll/hover verification; run it and observe current always-visible behavior.
- [x] Remove tray title, bind monochrome variants to system appearance. Implement 900 ms idle and 180 ms fade, 5 px thumb, active hover/drag retention; reduce motion as appropriate.
- [x] Verify overflow/no-overflow and existing scrollbar interactions.

### Task 2: Real sync status

**Files:** `src/domain/status.rs`, `src/sync/worker.rs`, `src/app/local_ui/{synchronization,presentation}.rs`, `ui/app-window.slint`, new `ui/components/sync-indicator.slint`, `tests/automatic_sync.rs`.
**Interfaces:** `SyncSnapshot.executing: HashSet<PageKey>`; typed UI state property and label.
- [x] Add regression coverage proving saved/base-matched versus pending and active states; execute failing tests.
- [x] Publish executing keys at lane execution start/end using lifecycle cleanup. Keep queued admission separate and clear cancellation/shutdown activity.
- [x] Project current revision and errors with the approved precedence; render accessible status independently of newest-note preference. Retain compatibility loading of the legacy setting.
- [x] Verify fixture-backed execution lifecycle and local/remote revision safety.

### Task 3: Typography, icons and bars

**Files:** `build.rs`, UI shared font definitions, `ui/components/{action-button,note-editor,appearance-panel}.slint`, `ui/app-window.slint`, `assets/icons/*.svg`, documentation.
**Interfaces:** Shared DM Mono/DM Sans font family names; existing action callbacks unchanged.
- [x] Capture baseline native/test-rendered UI and inspect AntiNote footer.
- [x] Register bundled licensed fonts, update actual editor/measurement/preview typography. Flatten toolbar controls and redraw the SVG set on a 24 unit grid.
- [x] Align header/footer and reserve sync target space. Remove obsolete latest-dot setting UI; preserve legacy deserialization and newest navigation.
- [x] Check narrow/default layouts and keyboard reveal/navigation.

### Task 4: Language-highlighted code blocks

**Files:** `Cargo.toml`, `Cargo.lock`, `src/app/markdown.rs`, focused code-highlighting module, `ui/components/note-editor.slint`, `tests/markdown_highlighting.rs`.
**Interfaces:** Existing `highlight-markdown` decoration callback; source-preserving code block range model if accurate layout can be obtained.
- [x] Add tests for differing code tokens, unknown/unclosed fences, Unicode/CRLF, code budgets and unchanged source/undo; observe missing highlighting failure.
- [x] Verify stable Syntect release, pin Rust backend and lock dependencies. Cache grammars/results and apply contrast-safe disjoint byte-range token decorations within 32 KiB/block and 128 KiB/note budgets.
- [x] Verify accurate wrapped block geometry using the pinned public API. Implement continuous shaded panels if possible; report any API constraint before changing the pinned dependency.
- [x] Run Markdown/editor tests and measure a representative 100 KiB note.

### Task 5: Final verification and review

**Files:** Reusable regression coverage and `docs/usage.md`, branding/reference documentation.
- [x] Compare native/rendered light/dark screenshots at minimum/default/larger sizes, high contrast and reduced motion; exercise scrollbar and keyboard behavior.
- [x] Run `scripts/check.sh`, inspect all results and fix relevant defects.
- [x] Request a fresh whole-diff review; fix material findings and rerun affected checks.
- [x] Remove temporary harnesses, review final diff and report actual outcomes and limitations.

## Verification record

- Scrollbar regression observed failing with the original always-visible thumb, then passing with idle/hover/drag/reduced-motion behavior.
- Code keyword/string regression observed failing before Syntect; source, Unicode, unknown/incomplete language and budget regressions pass.
- Fresh review findings for dirty disconnected state, font licensing, obsolete docs, text-size invalidation and hover idle timing were addressed. Both rendered review regressions were observed failing, then passing.
- Native before/after captures inspected in light/dark, minimum 400 × 440, and larger high-contrast/reduced-motion layouts. Matching transient preview used no note storage or network; the harness was removed afterward.
- Release decoration timing on a 100,130-byte note: original plain-code decorations median 0.07 ms; initial language implementation median 140.35 ms. Per-block caching reduced edits outside unchanged blocks to median 0.24 ms/p95 0.28 ms. These measurements cover decoration computation, not total input-to-frame latency.
- Actual HTTP fixture verifies held PUT activity and cleanup on cancellation. Live SilverBullet and Windows/Linux native behavior were not exercised; system appearance icon selection follows the tray's independent Palette rather than note colors, without changing the user's OS appearance preference.

Final clean-state verification: `scripts/check.sh` exited 0 after cleanup and the
per-block cache change: fmt and strict Clippy passed; 462 tests passed across
47 targets, with 6 intentionally ignored helper cases. Editing within one block
in the same release benchmark measured median 15.93 ms/p95 21.67 ms. No total
input-to-frame latency claim is made. Changes remain uncommitted in the current checkout; the final branch check
reported `main`.

## Follow-up after testing the app

Removed the persistent dot and its now-unused presentation state. Recovery access
moved from the writing footer to Settings → General → Recovery; its data, guarded
sync paths, and normal note deletion remain unchanged. The create-note glyph is a
plain plus. AppKit owns traffic-light geometry; reveal changes alpha/enabled state
without hiding views or moving frames. A 32 px note-header row aligns the icons.

The new native `chrome-check` probe reproduced a 6 pt downward jump before the fix
and passed afterward, including reset during reveal, re-reveal during fade, and
settled hidden/enabled states. The recovery action is keyboard reachable at
400 × 440. Final full checks passed (462 tests, six intentionally ignored helpers),
and strict Clippy was rerun after the probe extension. A native window screenshot
was inspected. The v2 development bundle passed signature and isolated Metal
launch verification under `target/packages/qol-test-20261031-v2`.
