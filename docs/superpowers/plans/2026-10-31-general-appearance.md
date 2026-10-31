# General and Appearance Implementation Plan

Subsequent user correction: [universal theme and explicit paper types](2026-10-31-universal-theme.md) supersedes the Theme default option and the separate settings palette described in this historical plan.

> **For agentic workers:** Use superpowers:executing-plans to implement this plan task by task.

**Goal:** Adapt the reference General/Appearance settings and add independent paper finishes.

**Architecture:** Extend existing settings and their serialized writer. Keep the
editor mounted; project preferences into Slint and reuse themed controls.

**Tech Stack:** Existing Rust, Serde, SQLite and Slint; no new dependencies.

**Spec:** ../specs/2026-10-31-general-appearance-design.md

## Global constraints

- Preserve existing defaults, note content, selections, and save barriers.
- Minimum window size is 400 × 440.
- Existing Plain/Dots/Ruled/Grid control is labeled Paper style.
- Texture respects increased contrast and reduced transparency.

## Review focus

- Old settings load with current defaults.
- A failed write restores every new option.
- Startup preference never replaces an existing saved note.
- Explicit paper finishes work on imported palettes without changing colors.
- Keyboard focus reaches both categories and scrolls controls into view.

### Task 1: Persist and render preferences

Files: src/theme/mod.rs, src/store/settings.rs, src/app/ui.rs,
src/app/appearance_ui.rs, src/app/startup.rs, ui/app-window.slint,
ui/components/note-editor.slint, ui/components/paper-material.slint,
assets/paper/linen.svg, tests/editor_materials.rs, tests/app_startup.rs.

Interfaces: PaperType and PaperIntensity enums in theme; new AppearanceSettings
fields paper_type, paper_intensity, keep_bottom_bar_visible, show_note_count,
show_note_date, show_newest_indicator. AppSettings.start_with_new_note.

- [x] Add failing rendered-finish and startup/persistence regressions.
- [x] Run regressions and observe the missing behavior.
- [x] Implement validated preferences, default-compatible deserialization,
  presentation, serialized save/rollback, startup choice, and rendering gates.
- [x] Run relevant settings, startup, material, and theme tests.

### Task 2: Categorize and group settings

Files: ui/components/appearance-panel.slint, ui/components/settings-controls.slint,
ui/app-window.slint, tests/settings_presentation.rs, docs/usage.md.

Interfaces: Existing AppearancePanel callbacks remain; General and Appearance
tabs share the existing changed callback and preferences from Task 1.

- [x] Extend keyboard and save-rollback coverage for the new controls.
- [x] Add General/Appearance category navigation and grouped rows, preserving
  theme reveal behavior and shortcut/connection actions.
- [x] Inspect rendered screenshots at minimum and normal sizes, in both schemes.
- [x] Update usage documentation, run scripts/check.sh, review the final diff.

Execution: inline in the existing isolated worktree under the user's instruction
to apply fitting features. No commits, merge, publication or installation needed.

## Verification result

Completed inline. `scripts/check.sh` passed formatting, strict all-target/all-feature
Clippy, and the full Rust test suite. Rendered regressions cover independent
finishes/intensity, preserved source, old settings, startup note preservation,
failed-write rollback, and keyboard access at minimum size. The preview/editor
material mismatch found during review was reproduced and fixed with a shared
resolved material state. Final review found no remaining important issues.

An isolated native Metal preview rendered both categories at 800 × 700 and
400 × 440; light/dark testing-backend captures were compared with the baseline.
Task-created capture harnesses were removed. Windows/Linux native UI and a full
VoiceOver session were not exercised. No installation, commit, or publication.
