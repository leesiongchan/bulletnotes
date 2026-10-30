# Bulletnotes editor and settings refresh

Status: implemented and verified in the editor/settings refresh worktree; local integration approved.

## Intent and agreed scope

Refresh the complete editor and settings experience around the approved Bulletnotes identity: warm eggshell paper, persimmon orange, satin silver, restrained depth, and subtle human imperfection. The writing surface stays calm and the controls feel deliberately crafted.

The user chose a full editor-and-settings refresh rather than a Paper-theme-only refinement. They approved adding Bulletnotes light and dark themes as the defaults for new installations, while preserving existing users' saved theme choices. They requested planning before implementation.

## Approaches

1. **Shared visual refresh plus branded settings chrome — recommended and accepted in the proposed plan.** Refresh shared controls and editor composition. Settings use a consistent Bulletnotes light/dark surface, independent of the selected note palette. Each note and its editing controls retain their selected palette. This follows the existing separation between settings chrome and note themes, while replacing today's hardcoded gray/blue settings treatment.
2. **Palette-driven settings everywhere — considered.** Every note palette would also color settings. This is visually consistent with the active note but makes settings vary across 43 built-ins and arbitrary imported palettes, weakening the stable branded settings experience. It is not the selected proposal.
3. **Add only a Bulletnotes palette — ruled out by the requested scope.** Smallest implementation, but would leave the existing settings, component styling, and visual hierarchy largely unchanged.

## Theme identity, defaults, and persistence

- Add `assets/themes/bulletnotes.json`, built-in ID `bulletnotes`, display name **Bulletnotes**, `dark: false`.
- Add `assets/themes/bulletnotes-dark.json`, built-in ID `bulletnotes-dark`, display name **Bulletnotes Dark**, `dark: true`.
- Fresh settings keep System mode, Plain paper, and the existing 15 px text default. Their light assignment becomes `bulletnotes`; their dark assignment becomes `bulletnotes-dark`.
- Saved settings keep their explicit mode, both theme assignments, imports, paper style, font size, window placement, shortcut, and active connection. Do not migrate or rewrite existing choices.
- Keep all 41 existing built-in IDs valid. With the two additions, the registry contains 43 themes. Imported themes retain their UUID-based IDs even if their display names match Bulletnotes.
- Put the two Bulletnotes cards first, followed by the existing featured order and remaining themes. Preserve unchanged catalog models and keyboard focus during appearance updates.
- Keep theme JSON and stored application settings at schema version 1. No database migration, new theme-file field, remote resource, or persisted material preference is needed.
- Exported/imported theme files remain palettes. The built-in paper material is presentation associated with the two built-in IDs; an imported copy receives the normal imported-theme presentation.

## Proposed palette

These exact starting values are the designer's proposal, derived from the approved identity and checked against the current contrast policy.

| Role | Bulletnotes | Bulletnotes Dark |
| --- | --- | --- |
| paper | `#F7F3EB` | `#25272B` |
| surface | `#FFFCF7` | `#303238` |
| ink | `#332E29` | `#F4EEE5` |
| muted | `#6E675F` | `#B9B0A5` |
| accent / cursor | `#C3431C` | `#FF9A71` |
| line | `#95877A` | `#8E8C88` |
| selection | `#F3C8B4` | `#633E30` |
| scrollbar | `#95877A` | `#8E8C88` |
| grid | `#DAD1C3` | `#44464B` |

Use ink for headings 2/3, accent for heading 1 and links, and muted for Markdown syntax. Check every explicit Markdown role against paper and surface because explicit colors bypass the normal fallback. Provide eight swatches using the palette plus decorative persimmon `#EE5928`, silver `#D0D2D6`, and related warm neutrals. Decorative bright orange does not replace the accessible text accent.

Current minimum contrast over paper and surface: light ink 12.14:1, muted 5.04:1, accent 4.58:1, line 3.15:1; dark ink 11.11:1, muted 5.99:1, accent 6.17:1, line 3.82:1. Ink over selection is 8.78:1 light and 8.05:1 dark.

## Presentation boundaries

Keep the existing note `theme: UiTheme` as the active note palette. Introduce a non-persisted `settings-theme: UiTheme` on `AppWindow`, resolved from the two built-in Bulletnotes palettes in the existing presentation mapping. Pass it into settings and associated settings forms. Apply the existing high-contrast adaptation to both presentation palettes.

Settings chrome follows the app's Light/Dark/System mode: Light chooses Bulletnotes chrome, Dark chooses Bulletnotes Dark chrome, and System follows the operating system. This replaces the current behavior where settings always follow the OS regardless of the app's mode. Independently assigned light/dark note palettes retain their current resolution rules.

Editor, find, page browsing, and recovery use the active note palette with the refreshed shared control shapes. Settings and its Connection form use the branded settings palette. About uses the same branded palette and the approved icon; refresh its presentation on opening so a previously created About window does not retain a stale scheme. Keep its Slint attribution.

Reuse existing components and callbacks. Any new helper should serve multiple concrete consumers; do not introduce a general theme engine or configurable material system. Preserve all existing navigation and persistence boundaries.

## Editor design

- Keep the compact window: preferred 505 × 552, minimum 400 × 440, and the existing rounded boundary.
- Refine spacing, note metadata, icon alignment, header/footer surfaces, find bar, scrollbar, and active/pinned/attention states. Keep familiar actions and their ordering.
- Use shallow, theme-colored surfaces with fine neutral edge highlights to suggest silver. Orange accents come from the Bulletnotes palette; other themes retain their own accent colors.
- Keep Menlo and the existing adjustable text-size range for note content. Keep normal UI typography for controls. The handwritten identity appears in branding and the movement of shapes, without changing the editing font.
- For the two built-in Bulletnotes IDs, add an extremely low-contrast static paper tooth with sparse irregularities. Light mode is eggshell; dark mode is quiet warm graphite. The dark variant keeps paper character without bright flecks. Other existing and imported themes retain their established paper surfaces.
- Keep Plain, Dots, Ruled, and Grid choices and their scrolling alignment. Grain remains subordinate to all four patterns and the text.
- Texture does not animate, move independently of the note, take pointer events, or participate in accessibility. No real-time noise, blur, glass rendering, custom text renderer, or thick metallic controls.

## Settings design

Keep settings inside the existing window and navigation model. The visible heading can become **Settings** while existing component names, callbacks, and active-view identifiers remain stable.

Group the existing controls in this order:

1. **Writing:** text size and paper pattern.
2. **Appearance:** Light/Dark/System, theme assignments, the theme collection, and import/export.
3. **Desktop shortcut:** existing accelerator editing and feedback.
4. **Connection:** existing entry to the connection form and its current status.

Use generous group separation, readable labels, restrained silver-toned choice controls, and orange selection/focus feedback appropriate to the branded scheme. Aim for 13–14 px settings labels and supporting text, with clear section headings. Use scrolling to preserve legibility at minimum size.

Theme cards keep their own palette and eight swatches. Clarify active selection and independent light/dark assignment without relying on color alone. Retain accessible names, selection announcements, keyboard activation, and focused-card retention. Keep the compact single-column layout and two-column layout when the available width supports it; do not add a new sidebar, tabs, theme search, or new preference.

Refresh Connection, page browsing, recovery, find, and About through the shared visual treatment so the app has no isolated old blue/gray controls. Preserve validation, errors, read-only/busy states, recovery warnings, credential handling, and available actions. Keep native controls where they provide required platform behavior; apply compatible colors rather than replacing their behavior for appearance alone.

## Functional and native constraints

Keep the single persistent native `TextInput`. Do not reconstruct it during settings changes, theme changes, or navigation. Preserve exact source bytes, Unicode/CRLF behavior, native IME composition gating, directed selection, caret blinking, scroll restoration, editable Markdown markers, clipboard/context menu, and single-step undo/redo.

Preserve save/recovery/lifecycle barriers, connection handling, sync, hotkeys, navigation, tray lifecycle, and note storage. Do not add persistence or logging of note content, server tokens, or credential values. Existing state/error logging stays at the current boundaries.

Keep header/footer reveal behavior, F6 traversal, Escape return, focus-after-visible-frame sequencing, and stable editor dimensions during reveal. Preserve current native hover geometry: 16 px edge triggers, 52 px revealed header depth, and 62 px footer depth, alongside the existing 44 px visible controls. Any implementation discovery requiring these dimensions to change must be brought back for a design decision; this refresh does not authorize a new chrome interaction model.

## Accessibility and rendering

- Palette ink/muted/accent meet 4.5:1 against paper and surface; line meets 3:1; ink against selection meets 4.5:1. Verify actual rendered focus and control boundaries separately.
- High contrast uses the existing enhanced-color policy, retains visible focus and state distinction, and removes paper grain, translucent styling, shadows, and gradients where those reduce clarity.
- Reduce Motion disables nonessential transitions. Reduce Transparency selects solid surfaces and suppresses paper grain and translucent/depth effects. Do not add animations behind typing.
- Keep controls keyboard reachable and labeled at default, minimum, and wide sizes; selected, hovered, pressed, focused, disabled, busy, and warning states remain distinguishable.
- Use native Slint presentation and small static assets, with no new dependency or custom renderer. Confirm any newly used rendering API against the pinned Slint version before implementation. Avoid window-size full-resolution material textures and per-frame decorative work.

## Files and module map

Expected primary scope:

- `assets/themes/bulletnotes.json`, `assets/themes/bulletnotes-dark.json`: new palettes.
- `src/theme/palette.rs`, `src/theme/mod.rs`: registry and fresh defaults.
- `src/app/ui.rs`, `src/app/appearance_ui.rs`, `ui/types.slint`: presentation mapping and narrowly required property plumbing.
- `ui/app-window.slint`, `ui/components/action-button.slint`, `note-editor.slint`, `appearance-panel.slint`, `find-bar.slint`, `page-browser.slint`, `theme-scroll-view.slint`, `theme-scrollbar.slint`: composition and shared controls. Component filenames without a prefix in this list live under `ui/components/`.
- `ui/connection-view.slint`, `ui/recovery-view.slint`, `src/app/native_app.rs`: related views and About palette handoff.
- Small material assets under `assets/` if supported native drawing alone cannot express the approved paper finish.

`src/store/settings.rs` is a behavior to verify, not an intended schema change. `src/platform/window_chrome.rs` remains the native-geometry contract. Leave the existing uncommitted logo/site rollout intact.

## Visual proof and documentation

Extend the existing real-Slint capture approach in `examples/capture_product_themes.rs` to cover the two new palettes and the refreshed editor. Add a focused capture path for settings, revealed chrome, find, and related views using synthetic notes/settings, without opening a real store, credentials, or network.

Capture default 505 × 552, minimum 400 × 440, and wide 800 × 700 states in light/dark, plus high contrast and keyboard focus. Include clean and textured paper comparisons. Review real native-renderer output before accepting visual changes; generated imagery or browser replicas are not proof of the implemented app.

Update `docs/themes.md`, the approved branding documentation, README light/dark screenshots, and the site's actual editor captures. Show Bulletnotes and Bulletnotes Dark in the site gallery and default hero preview, updating its theme count and labels while retaining the existing twelve themes and four papers. This is collateral alignment with the new app; retain the site's recently approved layout and styling.

## Verification and acceptance

1. New palette decode/export round trips pass, both palettes pass normal and enhanced contrast, and all 43 built-ins resolve.
2. Empty settings receive the two new defaults. Existing saved Paper/Graphite, imported themes, independent assignments, modes, sizes, and patterns reload unchanged; no migration occurs.
3. Settings chrome follows Light/Dark/System independently of note palette. Reopening About and navigating to Connection show the current branded scheme.
4. Theme cards preserve focus and assignments through changes, including failed persistence and failed imports; existing rollback behavior remains intact.
5. Source, IME gates, selection, undo/redo, scroll, find, Markdown markers, read-only/busy states, F6/Escape, and reveal geometry pass existing regression tests. Update pixel expectations only for intentional visual changes, not behavioral failures.
6. Capture and inspect the required sizes/states. Controls, labels, theme cards, and error messages are not clipped or obscured. Existing custom themes keep their palette identity; grain/depth suppression works under native accessibility preferences.
7. Run `scripts/check.sh`, theme/import/default regression checks, and actual app captures. Validate the refreshed site gallery and screenshots after collateral updates.

## Left to implementation planning

The builder may settle exact component radii, shallow shadow values, inset spacing within the existing chrome bounds, low-contrast grain density, the eight-swatch ordering, and the smallest reusable rendering helper needed. These are proposed implementation details, not additional user decisions. Preserve the palette thresholds and semantic boundaries above. Implementation requires a separate reviewed task plan after this design is approved.
