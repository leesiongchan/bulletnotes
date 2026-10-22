# AntiNote Chrome Refinement Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the existing floating controls quieter while preserving their geometry, keyboard access, and error visibility.

**Architecture:** Change semantic backgrounds and circular-button outlines in the existing Slint components. Retain the mounted editor and independent native hover regions. Update the historical fixture's obsolete undo expectation before using it as the acceptance gate.

**Tech Stack:** Slint 1.18.1, existing Rust native/generated UI fixtures, PNG snapshots.

**Spec:** [Approved design, Chrome section](../specs/2026-10-22-antinote-interaction-fidelity-design.md#chrome).

## Global Constraints

- "Use the active theme's paper color for the header."
- "Keyboard focus uses a one-pixel accent outline."
- "Increased contrast adds a one-pixel ink outline even at idle."
- Footer border uses "the theme line color at 25% alpha; increased contrast uses its full opacity."
- "Actionable save/sync failures remain persistently visible when chrome is hidden."
- Preserve 505×552 default/400×440 minimum window, existing insets, font metrics, controls, hover regions, F6/Escape and no-reflow behavior.
- Use generated-note evidence only; no new dependencies, settings, or database changes.

## Review Focus

- High contrast overrides subtle borders while keyboard focus remains distinguishable: C1 snapshots.
- Reentering during fade and F6 focus retain independent header/footer visibility: C1 native contract.
- At 400×440, long paths cannot overlap required controls: C1 minimum-size capture.
- Disabled actions and selection drags cannot trigger actions or window movement: C1 input assertions.
- Save/sync errors remain visible outside hidden chrome: C1 attention capture and accessible-label assertion.

---

## File Structure

Modify `ui/app-window.slint`, `ui/components/action-button.slint`, `examples/support/floating_chrome.rs`, `examples/ui_review.rs`, and `tests/ui_contract.rs`. Native hover/platform geometry should not need changes; a mismatch is a regression to investigate, not permission for an adjacent redesign.

### Task C1: Paper Header, Quiet Circular Buttons, and Passing Hover Gate

**Interfaces (NEW):** `ActionButton.in property <bool> high-contrast:false`, passed from `AppWindow.system-high-contrast` to its circular header/footer actions. Default false preserves unrelated callers. No callback or action changes.

- [ ] **Step 1: Pin failing rendered/input assertions.** Extend the generated fixture and `tests/ui_contract.rs` with cases `header_uses_paper_and_footer_keeps_surface`, `circular_idle_focus_and_contrast_are_distinct`, `minimum_chrome_retains_path_and_actions`, and `hidden_chrome_keeps_attention_visible`. Test the actual sampled header separately from footer/outer paper; the old combined expectation `[243,240,231]` for all shown-chrome sample bands is incorrect after this change. The default Paper header should be `[252,250,244]` within PNG tolerance 2. Do not impose that header color on the capsule's surface.

  Add or retain assertions:

```rust
assert_eq!(window.get_note_text().as_bytes(), original.as_bytes());
assert_eq!((window.get_editor_width(), window.get_editor_height()), geometry);
assert_eq!(window.get_editor_scroll_y(), scroll);
// Existing selection replacement is already atomic; correct the old assertion:
undo(&window);
assert_eq!(window.get_note_text(), original);
```

  Require the same directed selection after undo using E4's `editor-anchor`/`editor-caret` outputs, not a second undo. In the fixture's `capture()`, create its owned screenshot parent before invoking screencapture and propagate capture errors instead of silently ignoring them. These corrections repair the known historical gate; they must not weaken any reveal/input checks.

- [ ] **Step 2: Run red.** Rebuild `ui_review` with native-validation. Run `rustup run 1.99.1 cargo test --locked --test ui_contract -- --list` to identify the native wrappers, then the new rendered tests and `target/debug/examples/ui_review --floating-contract`. Expect the new paper/border assertions to fail against existing colors. The corrected one-step undo check should already pass before style changes.
- [ ] **Step 3: Implement the exact style.** Set the header to `theme.paper`. Circular idle buttons keep paper background and no outline; hovered/selected backgrounds use `theme.selection`. Focus has a one-pixel accent outline; increased contrast adds a one-pixel ink outline at idle. Preserve enabled/disabled behavior and action labels. Set capsule border alpha to 0.25 normally and 1.0 in increased contrast. Keep current metadata/path text, failure presentation, geometry, animations and reduced-motion behavior.
- [ ] **Step 4: Verify green and inspect images.** Rebuild/run `ui_review --floating-contract`, `--shortcut-contract`, `--contract` and `--scrollbar-contract` separately. Require exit zero and each explicit existing success marker. Capture default/minimum Sanrio or Tartan and Mononoke with hidden/header/footer chrome, high contrast, reduced motion, and attention. Compare with [research baseline](../../validation/antinote-research-2026-10-22.md). Verify outlines, action hit areas, unchanged source/geometry/selection, long-path fit, disabled actions, failure visibility and F6/Escape. Run `rustup run 1.99.1 cargo test --locked --test ui_contract` after adding non-native assertions; run native ignored wrappers only after rebuilding and explicitly selecting them.
- [ ] **Step 5: Commit** only C1 files as `Refine paper-colored floating controls`.

Continue with the [search plan](2026-10-22-antinote-search.md); final cross-component checks belong to [F1](2026-10-22-antinote-interaction-fidelity.md#task-f1-integrated-evidence-and-handoff).
