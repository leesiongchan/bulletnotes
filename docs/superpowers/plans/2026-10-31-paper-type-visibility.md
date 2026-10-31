# Paper Type Visibility Implementation Plan

Subsequent user correction: [universal theme and explicit paper types](2026-10-31-universal-theme.md) supersedes the Theme default option and the separate settings palette described in this historical plan.

> **For agentic workers:** Use superpowers:executing-plans to implement this plan task by task. Approved by the user and implemented.

**Goal:** Make Bone, Eggshell, and Linen visibly distinguishable at normal viewing size in both the note and its settings preview.

**Architecture:** Keep the existing settings, persistence and shared material-active gate. Centralize finish-specific rendering in PaperMaterial and calibrate the texture assets and opacity together. Do not change theme palettes or paper-style geometry.

**Tech Stack:** Existing Rust/Slint, Skia/Metal, PNG/SVG assets and rendered integration tests. No new dependency.

**Spec:** ../specs/2026-10-31-general-appearance-design.md (paper-type semantics and compatibility constraints).

## Diagnosis and evidence

An isolated Skia rendering probe on 2026-10-31 applied each preference through
`present_settings` and sampled a 128 × 128 text-free region of the actual note.
Measurements compare each finish with Smooth; channels use the 0–255 scale.

| Theme | Finish | Pixels changed | Largest channel change | Mean absolute channel change |
| --- | --- | ---: | ---: | ---: |
| Bulletnotes light | Bone | 1.17% | 1 | 0.0117 |
| Bulletnotes light | Eggshell | 15.01% | 1 | 0.1486 |
| Bulletnotes light | Linen | 32.81% | 1 | 0.3281 |
| Bulletnotes dark | Bone | 1.95% | 4 | 0.0397 |
| Bulletnotes dark | Eggshell | 31.24% | 3 | 0.5140 |
| Bulletnotes dark | Linen | 54.86% | 3 | 0.9485 |

The setting changes rendered pixels, but normal-size captures show that the
finishes are too faint to identify reliably. The light renderer passes opacity
0.005 (0.5%); Bone and Linen multiply it by four, then their assets further
attenuate it with partial opacity, sparse coverage and antialiasing. Dark uses
0.015 before the multiplier. Theme default intentionally uses the same Eggshell
texture as explicit Eggshell for the two default Bulletnotes palettes.

macOS reported Reduce Transparency=false and Increase Contrast=false during the
investigation. Those preferences can suppress texture, but do not explain the
observed faint rendering here. This did not inspect or alter the user's notes.

Existing tests check inequality between rendered images. A one-level change in
one pixel can satisfy that assertion; it does not establish useful visibility.
The temporary diagnostic harness was removed after gathering evidence.

## Global constraints

- Paper type remains independent of palette colors and Plain/Dots/Ruled/Grid paper style.
- Theme default retains its existing output; Smooth stays flat.
- Explicit Bone, Eggshell and Linen work on built-in and imported palettes.
- Preserve saved enum values, schema compatibility, serialized writes and rollback.
- Increased Contrast and Reduce Transparency continue to suppress grain.
- Applying a finish must preserve note source, selection, scroll and undo history.
- Keep text and accent contrast at least 4.5:1 on the default supported palettes.
- No application changes or replacement build are authorized by this planning document alone.

## Review focus

- Thin SVG features can disappear at particular display scale factors.
- A finish visible on warm light paper may wash out on dark or colored palettes.
- Raising opacity can reduce text contrast or obscure ruled/grid styles.
- Preview and note must use the same material inputs for crossed light/dark assignments.
- Suppressed effects must be explained without inviting users to disable accessibility preferences.

### Task 1: Calibrate and verify the finishes

**Files:** `ui/components/paper-material.slint`, `assets/paper/bone.svg`, `assets/paper/linen.svg`, `ui/components/note-editor.slint`, `ui/components/appearance-panel.slint`, `tests/editor_materials.rs`.

**Interfaces:** Keep PaperType and persisted values unchanged. PaperMaterial consumes `kind`, `tint`, `active` and a new `dark` boolean. Replace caller-owned `strength` with one internal per-finish/per-scheme opacity calculation. Both preview and editor supply the same resolved scheme and ink.

- [x] Extend rendered regressions before changing implementation. For explicit Bone/Eggshell/Linen on both default palettes, require at least 2% of sampled pixels to differ from Smooth by at least 3 channel levels, and a maximum difference of at least 6 levels. Require each pair of explicit textured finishes to differ by at least 3 levels over at least 2% of the sample. These are initial regression guardrails, not a general perceptual standard; retain a normal-size visual review as acceptance.
- [x] Run `rustup run 1.99.1 cargo test --test editor_materials --locked`. Expected: new visibility assertions fail for the current rendering; existing contrast and accessibility tests pass.
- [x] Add explicit per-finish opacity profiles in PaperMaterial. Preserve Theme's current 0.005/0.015 behavior separately; do not increase all materials globally. Starting candidates for explicit finishes are Bone 0.08, Eggshell 0.04 and Linen 0.08, then calibrate light/dark values against captures and contrast results rather than treating these candidates as final constants.
- [x] Increase Bone's fiber coverage so it reads as fine paper rather than rare specks. Make Linen's weave identifiable without resembling the independent Grid paper style. Keep assets deterministic and tileable; avoid random per-frame rendering.
- [x] Capture Smooth/Bone/Eggshell/Linen at normal size on the native Metal renderer at 1× and 2× where available, in light/dark and a colored/imported palette. Identify unavailable physical display coverage explicitly. Check Plain and Grid styles, preview parity, and small-window clipping. Review the images without magnification or contrast enhancement.
- [x] Run material, theme contrast, settings and selection regressions. Add source/selection/scroll/undo assertions to the material-switch scenario and retain the crossed-assignment preview regression. Expected: all pass, with visibly distinct textures and readable text.

### Task 2: Explain behavior and deliver a testable build

**Files:** `ui/components/appearance-panel.slint`, `ui/app-window.slint` (only if needed to pass the accessibility reason), `tests/settings_presentation.rs`, `docs/usage.md`.

**Interfaces:** Keep the shared `paper-material-active` gate. Pass the existing accessibility preference state to the panel if necessary to distinguish Smooth from a suppressed finish.

- [x] Show a short explanation beside Paper type when grain is suppressed by Reduce Transparency or Increase Contrast. Explain that Paper intensity affects dots/lines only. Keep the selected finish saved while effects are suppressed.
- [x] Verify preference click → immediate preview/note change → persisted save → reload through the actual UI binding. Check failed-save rollback, switching themes and accessibility suppression/recovery. Expected: selected state and rendered finish agree at each step.
- [x] Run `scripts/check.sh`; inspect the complete result. Expected: formatting, strict Clippy and all tests pass. Remove task-created diagnostic harnesses and rerun any affected checks after cleanup.
- [x] Build a fresh local macOS app with a distinct output path, preserve the prior test app, verify native launch, and provide the new app path with normal-size before/after captures. Do not replace `/Applications/Bulletnotes.app` automatically.

## Completion criteria

The user can see the finish change immediately at normal size, identify Bone,
Eggshell and Linen from their surface patterns, and obtain the same finish after
restarting. Theme default remains compatible. Passing pixel assertions alone is
insufficient; native visual review and contrast/accessibility checks are required.

## Execution results

- Calibrating stronger shadow grain first exposed a real default-light accent
  contrast failure. Final explicit finishes use raised white grain on warm light
  paper with highlight headroom, and ink grain on near-white/dark paper. Theme
  retains its original tint and 0.005/0.015 opacity. Bone/Linen assets now have
  denser fibers and interrupted weave; Eggshell keeps the original asset.
- Final shadow opacities are Bone 0.07, Eggshell 0.05 and Linen 0.08. Raised
  highlight opacity is 0.8 for Bone/Linen and 0.45 for Eggshell. Default palette
  contrast and the visibility/pairwise guardrails all pass.
- Source, directed selection, nonzero scroll and undo survive finish changes.
  The actual keyboard UI choice updates rendering, saves, reopens from SQLite
  and renders the same finish after reapplying restored settings. Accessibility
  suppression retains the selection and restores the texture when disabled.
- The Slint native GPU snapshot path omitted the material image in this
  verification harness, despite the texture being present on screen. Native
  acceptance therefore used macOS window captures rather than GPU snapshots.
  Twelve on-screen captures cover light/dark/Matcha, Grid, minimum-size preview
  and Reduce Transparency. They are in `target/paper-visibility-review/`.
- Native captures were made on a 2× display. Off-screen rendering was checked at
  1×; a physical 1× display and Windows/Linux native UIs were unavailable.
- `scripts/check.sh`: 441 passed, 0 failed, 6 ignored. After cleanup, formatting,
  strict all-target/all-feature Clippy and affected tests passed again. The final
  restored-settings rendering assertion also passed. Review found no remaining
  important issues. Temporary capture sources have been removed.
- New ad-hoc-signed development app:
  `target/test-build-paper-visibility/Bulletnotes.app`. Signature verification and
  `--verify-installation` passed (`graphics_backend: metal`). The prior test app
  remains in `target/test-build/`; nothing was installed or published.
