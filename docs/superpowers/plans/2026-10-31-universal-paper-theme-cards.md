# Universal Paper and Square Theme Cards

> Use superpowers:executing-plans inline. User authorizes universal paper finish/style and square theme cards with one Light → Dark → Both assignment control.

**Goal:** Share paper rendering throughout the app and simplify theme selection.
**Architecture:** Extract PaperSurface for finish + style + scale/scroll alignment. Reuse it for notes, auxiliary-view backdrop, preview and About. View roots become transparent; controls keep opaque surfaces for legibility. Theme cards become square with an edge-to-edge eight-color strip and one accessible assignment button.
**Assignment semantics:** Unassigned → Light → Dark → Both → Light. Restore a previous nonmatching theme when vacating a slot; use the matching built-in fallback if no prior choice exists. Save Light/Dark/Both changes as one atomic settings write, preserving appearance mode, source and metadata.

- [x] Reproduce missing paper in auxiliary views with rendered regressions.
- [x] Extract shared rendering without changing note scroll/grid geometry; propagate to About and previews.
- [x] Add assignment-history defaults/validation and regression coverage for full cycles, persistence and rollback.
- [x] Replace theme card layout and paired icons with squares, contiguous swatches and one cycle button.
- [x] Check keyboard focus and responsive layout, run project checks, review, capture native UI and build a new test app.

**Checks:** scripts/check.sh, actual rendered paper across six views, existing marker/selection contracts, theme assignment atomicity/rollback and cross-restart history. Native on-screen captures (GPU snapshots omit texture); retain older test builds.

**Verification:** `scripts/check.sh` completed with formatting and Clippy clean; 451 tests passed and 6 were ignored. Rendered regressions cover universal paper, preserved editor geometry, square/contiguous theme cards, intensity, keyboard cycles, atomic persistence and failed-save rollback. Native macOS captures cover wide/narrow galleries, settings, search, connection, recovery and About. Code review findings were resolved. Final development app: `target/test-build-paper-cards/Bulletnotes.app`; ad-hoc signature verified and `--verify-installation` reported `graphics_backend: metal` and `installation_check: true`. Native testing used a 2× display; Windows/Linux and physical 1× display behavior were not exercised.
