# Bulletnotes Editor and Settings Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans for native execution, or superpowers:subagent-driven-development if the user chooses review after every task. Execute the tasks in order; use checkbox steps for tracking.

**Goal:** Refresh the complete app around the approved paper/orange/silver identity, with Bulletnotes light/dark defaults for new installations and no changes to saved user choices.

**Architecture:** Keep note palettes and branded settings chrome separate. Extend the existing Rust-to-Slint presentation boundary, reuse the persistent native editor and shared controls, and express materials in presentation without changing palette or settings schemas.

**Tech Stack:** Existing Rust 1.99.1 toolchain, pinned Slint 1.18.1, schema-1 JSON palettes, existing SQLite settings, native Skia test captures, static site. No new dependency.

**Spec:** `docs/superpowers/specs/2026-10-31-editor-settings-refresh-design.md` (approved).

## Global Constraints

- New IDs: `bulletnotes` / **Bulletnotes**, and `bulletnotes-dark` / **Bulletnotes Dark**. Registry grows from 41 to 43; all existing IDs remain valid.
- Fresh defaults: System, light `bulletnotes`, dark `bulletnotes-dark`, Plain, 15 px. Keep every saved preference and imported theme unchanged. No migration or schema-version bump.
- Settings follow Light/Dark/System independently of note colors. Editor, find, search, and recovery retain the selected note palette; Connection and About use branded settings chrome.
- Preserve one persistent native `TextInput`, source bytes, directed selection, native IME/undo, caret behavior, scroll restoration, marker editing, navigation, save/recovery barriers, and credential handling.
- Keep preferred 505 × 552, minimum 400 × 440; preserve 16 px native edge triggers, 52 px header reveal depth, 62 px footer reveal depth, and existing 44 px visible control regions.
- Preserve schema-1 palette import/export, model retention, and persistence rollback. Paper materials belong only to the two built-in IDs, not same-named imports.
- Normal text contrast ≥4.5:1; line ≥3:1; enhanced ink ≥7:1. Check explicit Markdown colors and composited material backgrounds separately.
- High Contrast and Reduce Transparency flatten decorative effects and remove grain. Reduce Motion suppresses transitions. No animated paper, blur, custom renderer, new settings, sidebar, tabs, or theme search.
- Preserve the uncommitted logo/site rollout. Planning changes are documentation only. Before execution, establish the agreed workspace and record that baseline; do not discard it or stage unrelated work. No push, publication, release, or commit without the agreed execution workflow.

## Review Focus

1. Legacy settings and same-named imports: only settings-free installations get new defaults; imported IDs never gain built-in materials. Covered in Tasks 1–3.
2. App/OS appearance disagreement and cached About: settings follow app mode, and reopening About reflects the current palette/preferences. Covered in Tasks 2 and 4.
3. Native preference changes: high contrast, reduced transparency, and reduced motion must update an already-created view without persisting presentation changes. Covered in Tasks 2–4.
4. Small windows, long labels, and card focus: no clipped settings actions or lost keyboard selection at minimum/default/wide sizes. Covered in Tasks 3–5.
5. Unsaved Unicode/CRLF notes and active native editing state: theme/settings navigation must not replace the editor or mutate source, undo, selection, or scroll. Covered by Tasks 2–4 and the final regression run.

## File Structure and Ownership

- **Task 1:** new `assets/themes/bulletnotes.json` and `bulletnotes-dark.json`; registry/defaults in `src/theme/palette.rs` and `src/theme/mod.rs`; catalog order in `src/app/ui.rs`; theme/default contract tests.
- **Task 2:** presentation plumbing in `src/app/ui.rs`, `src/app/appearance_ui.rs` only where needed, `src/app/native_ui.rs` only if preference refresh needs it, `ui/app-window.slint`, and `ui/components/appearance-panel.slint`; new `tests/settings_presentation.rs`.
- **Task 3:** existing shared button/find/scroll/editor components; new `ui/components/paper-material.slint`, `assets/paper/eggshell.png`, and `tests/editor_materials.rs`; existing UI/editor regressions.
- **Task 4:** settings, page browser, Connection, Recovery, About, and native About handoff; extend `tests/settings_presentation.rs` and existing navigation/theme-selection tests.
- **Task 5:** extend `examples/capture_product_themes.rs`; create `examples/capture_design_states.rs`; update actual README/site images, site gallery entries and counts, theme/brand documentation.
- **Task 6:** final validation and review, with fixes restricted to findings from this scope.

`src/store/settings.rs` and `src/platform/window_chrome.rs` are contracts to verify; no storage or native-geometry changes are planned. Keep `UiTheme` as the existing semantic color contract; no material fields in JSON or new serialized UI types.

## Task 1: Add and Persist the New Defaults Safely

**Files:** Create the two palette JSON files. Modify `src/theme/palette.rs`, `src/theme/mod.rs`, catalog ordering in `src/app/ui.rs`, `tests/theme_contract.rs`, `tests/theme_contrast.rs`, `tests/bundled_themes.rs`, `tests/theme_selection.rs`, and default-dependent expectations in `tests/ui_contract.rs` only where the expected default actually changes.

**Interfaces:** Keep `AppearanceSettings::default()`, `builtin_themes()`, `resolve_theme(...)`, palette decode/encode, and store load/save signatures unchanged. Later tasks consume the two stable built-in IDs.

- [x] **Write failing tests for new defaults, round trips, contrast, and legacy settings.** Add `fresh_defaults_use_bulletnotes_pair`, `bulletnotes_palettes_round_trip`, and `legacy_theme_choices_survive_default_change`. The first test's core assertions are:

```rust
let settings = AppearanceSettings::default();
assert_eq!(settings.mode, AppearanceMode::System);
assert_eq!(settings.light_theme.0, "bulletnotes");
assert_eq!(settings.dark_theme.0, "bulletnotes-dark");
assert_eq!(settings.paper, PaperStyle::Plain);
assert_eq!(settings.font_size, 15.0);
let themes = builtin_themes().unwrap();
assert_eq!(themes.len(), 43);
for (system, id) in [
    (SystemAppearance::Light, "bulletnotes"),
    (SystemAppearance::Dark, "bulletnotes-dark"),
] {
    assert_eq!(resolve_theme(&settings, system, &themes).unwrap().id.0, id);
}
```

For legacy coverage, save an explicit Paper/Graphite pair plus a same-named imported Bulletnotes palette, non-default pattern/text size, shortcut, and window placement through `StoreHandle`; reopen and assert complete `AppSettings` equality. A separate settings-free store must load the new defaults. Decode→encode→decode both new palettes and assert complete equality, all optional Markdown roles, eight swatches, normal contrast, and unchanged authored data after high-contrast conversion.

- [x] **Run the red verification.** `rustup run 1.99.1 cargo test --locked --test theme_contract --test theme_contrast --test bundled_themes`. Expected: new defaults/count/ID assertions fail before implementation; existing parsing protections remain meaningful.
- [x] **Implement palette addition and defaults.** Copy the exact role table and Markdown policy from the spec; add eight swatches. Register both files, set only fresh defaults, and place both cards before the existing featured order. Do not change `src/store/settings.rs` or JSON schemas. Retain existing boundary logging; no notes, tokens, or full settings in logs.
- [x] **Update intentional default/order assertions.** Non-`antinote-` built-ins become six; total becomes 43. Assert the first card pair is the two Bulletnotes IDs and the existing featured order follows it. Find named swatches by ID rather than retaining obsolete row indices. Keep keyboard traversal assertions and adjust their concrete expected first-card sequence to the new order. Update old `paper` assertions only when the setup uses the fresh default; explicit legacy fixtures must still expect Paper.
- [x] **Run green verification.** `rustup run 1.99.1 cargo test --locked --test theme_contract --test theme_contrast --test bundled_themes --test theme_files --test theme_selection --test ui_contract --test release_upgrade`. Expected: all pass; existing settings fixtures remain unchanged.

## Task 2: Separate Branded Settings Presentation From Note Palettes

**Files:** `src/app/ui.rs`, narrowly necessary refresh wiring in `src/app/appearance_ui.rs` / `src/app/native_ui.rs`, `ui/app-window.slint`, `ui/components/appearance-panel.slint`; create `tests/settings_presentation.rs`.

**Interfaces:** Add `in property <UiTheme> settings-theme` to `AppWindow`. Replace AppearancePanel's private hardcoded `chrome` value with `in property <UiTheme> chrome`; consume `root.settings-theme` at its call site. Existing note `theme`, callbacks, mode strings, catalog models, and selected IDs retain their meaning.

Add a private Rust helper in `src/app/ui.rs`:

```rust
fn settings_palette(
    settings: &AppearanceSettings,
    system: SystemAppearance,
    themes: &[ThemeEntry],
    high_contrast: bool,
) -> Result<UiTheme, ThemeError>
```

It selects the built-in branded light/dark palette from app mode plus OS scheme, applies existing high-contrast conversion when requested, then reuses the existing color mapping. `present_settings(...)` sets this palette in the same presentation transaction as the note palette. No new storage field or alternate persistence path.

- [x] **Write `settings_scheme_follows_app_mode_not_note_palette`.** Use the current Skia testing backend pattern and `present_settings`. Cover Light/Dark against both OS schemes and System against each scheme. Assign Matcha/Graphite or an imported palette to the note slots; assert note paper is the assigned palette while `window.get_settings_theme().paper` equals the appropriate branded paper. Core expectations:

```rust
assert_eq!(window.get_settings_theme().dark, expected_chrome_dark);
assert_eq!(window.get_theme().paper, expected_note_paper);
assert_eq!(window.get_note_text().as_bytes(), original_source.as_bytes());
```

Add preference-refresh coverage: toggle native high contrast through the existing presentation path and assert both palettes reach enhanced contrast, the source/settings are unchanged, and the theme catalog model identity is retained. Exercise the real `appearance_ui` mode/system callbacks and failed-save rollback, not just the private helper.

- [x] **Run red verification.** `rustup run 1.99.1 cargo test --locked --test settings_presentation`. Expected: missing new presentation property or wrong scheme assertions until implemented.
- [x] **Implement the boundary and replace hardcoded settings colors.** Route mode, OS, and accessibility updates through the existing presentation path. Update the settings backdrop in AppWindow as well as panel content. Preserve the macOS cursor/selection workaround. Log only the chosen theme IDs and presentation flags at the existing boundary, using the current `tracing` idiom.
- [x] **Run green verification.** `rustup run 1.99.1 cargo test --locked --test settings_presentation --test theme_selection --test ui_contract --test app_navigation`. Expected: independent note/settings palettes, rollback, and navigation pass.

## Task 3: Refresh Shared Controls and the Writing Surface

**Files:** `ui/components/action-button.slint`, `find-bar.slint`, `theme-scroll-view.slint`, `theme-scrollbar.slint`, `note-editor.slint`, `ui/app-window.slint`; create `ui/components/paper-material.slint`, `assets/paper/eggshell.png`, and `tests/editor_materials.rs`; extend meaningful state checks in `tests/ui_contract.rs`.

**Interfaces:** Keep current control callbacks and dimensions inside the existing chrome bounds. Add `material-effects: bool` (default true) to ActionButton/ChoiceButton, and a `high-contrast: bool` input (default false) to ChoiceButton to match ActionButton. Bind AppWindow's private `decorative-effects` to `!root.system-high-contrast && !root.prefers-reduced-transparency`; pass the equivalent real preference values through each view to consumers. Each control also gates decoration with its own high-contrast input. Do not leave accessibility callers using a decorative default.

Add `paper-material: bool` (default false) to NoteEditor. AppWindow enables it only for selected built-in IDs `bulletnotes` / `bulletnotes-dark` and only when neither High Contrast nor Reduce Transparency is enabled. The new `PaperMaterial` component takes `active: bool` (default false), `tint: color`, and `strength: float`; it is shared by NoteEditor and branded settings in Task 4. It has no pointer handler and no accessibility role.

- [x] **Capture the real baseline and add meaningful failing assertions.** Record current editor, revealed chrome, and find snapshots using the existing testing renderer. Add `paper_material_is_builtin_only_and_preference_gated` and `rendered_material_preserves_text_contrast`; compare an empty paper region with grain enabled/disabled and check the actual composited background against accent/ink thresholds. Same-named imports and non-Bulletnotes IDs must remain flat. Assert source bytes and selection do not change during preference toggles.
- [x] **Verify the new assertions fail before rendering changes.** `rustup run 1.99.1 cargo test --locked --test editor_materials --test ui_contract`. Expected: material gating/detail assertions fail; existing editor contracts remain intact.
- [x] **Create the static paper asset and shared renderer.** Use the image-generation workflow for a neutral, transparent 128 × 128 paper-tooth asset without letters, shadows, seams, or embedded page color. Display at stable logical scale with bounded native Slint repetition and clipping; do not stretch a high-resolution icon over the editor or generate noise each frame. Verify any new Image/layout feature against official pinned Slint documentation before choosing syntax. A native repeated Image grid with a shared cached source is sufficient; no renderer dependency.
- [x] **Keep grain below the contrast budget.** Start at 0.005 strength over light paper and 0.015 over dark paper, tinted from ink. Lower opacity if composited accent contrast falls below 4.5:1; the light accent has little spare margin. Verify all four paper patterns; patterns retain their existing scroll alignment. Grain stays visually subordinate and vanishes under the specified preferences.
- [x] **Apply the control/editor visual treatment.** Fine borders, theme-derived shallow highlights, restrained radii, stable icon alignment, readable metadata, and distinguishable selected/hovered/pressed/focused/disabled/attention states. Retain Menlo, insets needed for markers, source editing, current action ordering, and reveal geometry. Keep accessibility surfaces solid; do not introduce new layout motion.
- [x] **Run green regressions and inspect native captures.** `rustup run 1.99.1 cargo test --locked --test editor_materials --test ui_contract --test editor_commands --test editor_position --test editor_roundtrip --test markdown_highlighting --test markdown_markers --test note_find --test app_navigation`. Expected: all pass. Check fresh captures at 400 × 440, 505 × 552, and 800 × 700 in both branded themes; reveal must not resize or rewrap the editor. Change pixel expectations only where the design intentionally changes visuals.

## Task 4: Refresh Settings and Related Views

**Files:** `ui/components/appearance-panel.slint`, `page-browser.slint`, `ui/connection-view.slint`, `ui/recovery-view.slint`, AboutWindow in `ui/app-window.slint`, `src/app/native_app.rs`, and `src/app/ui.rs`; extend `tests/settings_presentation.rs`, `tests/theme_selection.rs`, `tests/ui_contract.rs`, and relevant existing navigation coverage.

**Interfaces:** Settings and Connection consume `AppWindow.settings-theme`; editor/find/page browser/recovery consume `AppWindow.theme`. Add `theme: UiTheme`, `high-contrast: bool`, and `reduce-transparency: bool` presentation inputs to AboutWindow. Add this presentation function in `src/app/ui.rs`, consumed both by the actual native About-open callback and the capture example in Task 5:

```rust
pub fn present_about(window: &AboutWindow, source: &AppWindow)
```

It refreshes the About inputs from the current AppWindow; version text and native activation remain in the existing native callback. Call before every About show, including reused windows. Keep Slint attribution and current native close behavior.

- [x] **Write failing behavior/state tests.** `cached_about_uses_current_branded_scheme` calls `present_about` twice on the same About instance after changing main appearance/preferences, then compares its presentation inputs to current main values. `settings_minimum_size_keeps_actions_reachable` exercises keyboard focus and scrolling through existing groups at 400 × 440. Extend card tests to cover independent assignments, import errors, save failures, long imported names, and model/focus retention with the new controls. Retain existing native preference/selection tests rather than replacing them with source-string checks.
- [x] **Run red verification.** `rustup run 1.99.1 cargo test --locked --test settings_presentation --test theme_selection --test ui_contract`. Expected: new About/presentation or reachability assertions fail before corresponding changes.
- [x] **Implement settings composition.** Visible title **Settings**; groups Writing, Appearance, Desktop shortcut, Connection in that order. Keep text/paper controls ahead of the theme collection, current callbacks and view identifiers, single-column compact cards and supported two-column wide cards. Use 13–14 px labels/supporting text with scrolling at small sizes. Pass branded chrome into cards' selection/assignment treatment while their content keeps each palette's authored colors/swatches. State must remain legible without color alone.
- [x] **Apply the same craft to related views.** Route Connection and About to settings chrome; reuse refreshed controls in browsing/recovery/find with note colors. Apply material suppression consistently. Preserve native form semantics, validation, busy/disabled/read-only/error states, credential handling, recovery warnings, and active-view/focus behavior. Do not change connection or sync logic for a visual refresh.
- [x] **Run green verification.** `rustup run 1.99.1 cargo test --locked --test settings_presentation --test theme_selection --test ui_contract --test app_navigation --test editor_position --test connection_security --test history_recovery`. Expected: all relevant tests pass and no saved/source state changes during navigation.

## Task 5: Capture the Implemented App and Align Documentation

**Files:** `examples/capture_product_themes.rs`; create `examples/capture_design_states.rs`; `docs/themes.md`, `docs/branding/README.md`, `docs/screenshots/light.png`, `dark.png`, `site/assets/themes/*.png`, `site/index.html`, `site/script.js`, `site/README.md`. Keep the site's approved styling/layout.

**Interfaces:** Extend the existing product capture list with `bulletnotes` and `bulletnotes-dark`; each uses the existing `plain`, `dotted`, `ruled`, `grid` filename suffixes. There are 14 showcased themes × 4 papers = 56 native editor PNGs. The new design capture CLI accepts exactly one output directory, following the existing example; it builds synthetic AppSettings/notes in memory and uses the same testing Skia renderer. It must not open a real store, keychain, connection, or network.

- [x] **Implement focused capture coverage.** `capture_design_states.rs` consumes existing `present_settings(...)`, Task 4's `present_about(...)`, and actual AppWindow/AboutWindow components. Write PAM outputs for editor, revealed chrome, find, settings, page browser, Connection, recovery, and About. Capture editor/settings at minimum/default/wide sizes in light/dark. Capture focus, High Contrast, Reduce Transparency, and Reduce Motion at default size. Supply harmless synthetic populated and error/busy states for related forms.
- [x] **Run real captures.** `rustup run 1.99.1 cargo run --locked --example capture_design_states -- /tmp/bulletnotes-design-captures` and `rustup run 1.99.1 cargo run --locked --example capture_product_themes -- /tmp/bulletnotes-product-captures`. Expected: native rendered PAM files for the defined review matrix, plus exactly 56 product-theme PAMs. Convert using the existing `sips` workflow. Keep the full review matrix in temporary output; track only intended README/site images.
- [x] **Inspect the screenshots.** Compare baseline/refreshed editor and settings, default/minimum/wide, light/dark, focused/hovered/disabled/warning states, all paper patterns, and preference-flattened surfaces. Check there are no clipped controls, pattern/text collisions, decorative focus distractions, metal-like noise behind text, or unreadable imported-card labels. Adjust only observed material/layout issues and rerun the affected regression/capture, not unrelated tests.
- [x] **Update actual product collateral.** README light/dark images use the two new defaults. Add the two new gallery options before existing themes, select Bulletnotes initially in HTML and script state, update caption/alt/default neighboring screenshots and hero image, and change counts from twelve to fourteen. Keep all existing themes and four patterns. Update docs with defaults, preserved saved choices, material/export limitations, and capture commands. Do not claim an editable SVG master or a release.
- [x] **Verify the gallery in a local browser.** Use agent-browser after reading its version-matched workflow. Exercise all 56 combinations with image decoding, caption/alt and pressed-state checks. Check 320/390/768/1024/1440 px, 200% text, reduced motion, keyboard focus, and readable static content with the script blocked. Expected: no broken images, overflow, editable fake app fields, or new accessibility violations. The site's approved design remains intact.

## Task 6: Final Regression, Evidence, and Review

**Files:** Any necessary fixes in the above owned files; record results in this plan. No new feature or unrelated cleanup.

- [x] **Run required checks.** `scripts/check.sh`, `python3 -m unittest discover -s tests -p test_package_desktop.py`, `node --check site/script.js`, `git diff --check`, and the site's existing formatting check. Expected: all pass. If sandboxing blocks local HTTP fixtures, rerun unchanged tests with permission rather than changing tests or application behavior. Distinguish intentional existing ignored checks from passed checks.
- [x] **Check preservation and scope.** Compare the fresh-default and saved-settings fixtures, all 43 built-ins/import identity, note editor state, native geometry, preference suppression, and the current dirty-rollout baseline. Confirm no database schema, token handling, functional action, sync behavior, package identity, or website layout was changed beyond this plan.
- [x] **Obtain a fresh review.** For native execution, one independent reviewer examines the complete task diff and actual captures against the approved spec. Subagent-driven execution additionally gates each task. Resolve material findings with a focused regression and recapture; report platform-specific/manual checks that could not be completed.
- [x] **Deliver concrete evidence.** Link the refreshed editor/settings light/dark previews, the two palette files and documentation, and summarize the commands and observed results. Do not call the work complete until those artifacts and checks exist. Do not publish or release as part of this UI work.

## Self-Review of This Plan

- Spec coverage: theme/default safety → Task 1; presentation/scheme boundaries → Task 2; shared materials/editor/native constraints → Task 3; settings/related views/About → Task 4; real visual proof/collateral → Task 5; complete checks and independent review → Task 6.
- Interface consistency: later tasks use the existing note `theme`, new `settings-theme`, panel `chrome`, NoteEditor `paper-material`, shared `PaperMaterial`, and `present_about(&AboutWindow, &AppWindow)` as defined above. No new serialized type.
- Review Focus: each of the five cases has a behavior test or real-render check in its owning task. The extra capture example supplies visual evidence rather than implementation-mirroring tests.
- Execution: task order is dependency-driven. Native execution is recommended because shared controls, palette plumbing, and visual captures build on each other. Parallel implementation of these tasks would introduce shared-file conflicts and more repeated context.

## Implementation evidence — 2026-10-31

Implemented on `codex/editor-settings-refresh` in `/tmp/bulletnotes-editor-refresh`, reviewed before local integration. The original checkout and approved logo assets are preserved.

- `scripts/check.sh`: formatting and Clippy clean; **410 passed, 0 failed, 6 ignored**, across 44 Rust suites.
- Packaging tests: **13 passed**. JavaScript syntax, Prettier, and `git diff --check` pass.
- **70** native Slint Skia design captures in `/tmp/bulletnotes-design-captures`; **56** product-theme PNGs (14 themes × 4 papers). README uses the two new defaults.
- Browser: all 56 combinations decoded with correct caption/alt/pressed state; no overflow at 320/390/768/1024/1440 px or 200% text. Keyboard focus visible, fresh reduced-motion image animations zero, script-blocked fallback readable, axe WCAG A/AA zero violations. Gradient contrasts retained from verified branding rollout.
- Independent review found two Important issues: visible About preference updates and high-contrast assignment controls on low-contrast imports. Both have failing-then-passing rendered regressions, and the full suite passes after the fixes. No deferred minors; no declined-to-judge items.
- Evidence uses the real testing renderer; live macOS accessibility toggles and platform-specific IME behavior were not manually exercised. Existing native editor regressions remain green.

### Execution decisions

1. Used Slint 1.18.1 native image tiling instead of a repeated grid; potential renderer-portability cost is checked by compilation and rendered pixel tests.
2. Started the capture tool in Task 3 for its required visual checks, then extended it in Task 5; cost is task ordering only.
3. Made Markdown fallback fixtures explicitly select legacy Paper and hide chrome, preserving their purpose under defaults with authored Markdown roles. Potential coverage loss for new themes is addressed separately by palette/selection tests and captures.
4. Enabled all test features for the planned Connection regression, which requires `protocol-validation`; cost is broader checks only.
5. Reviewed uncommitted files against the preserved branding baseline instead of an empty commit range. Commit-level provenance is unavailable, so the review diff, inventory, and execution ledger are retained in this plan's ignored workspace.

Local integration into `main` was authorized by the user after verification and review. Push, publication, and release are outside this integration step.
