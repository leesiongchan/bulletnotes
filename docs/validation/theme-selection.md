# Theme selection and AntiNote catalog audit

The old picker blocked 28 of its 41 themes on contrast checks. Its confirmation
was below the entire catalog and shortcut settings, so theme cards appeared
unresponsive. Existing picker selections now apply and save directly. New
low-contrast file imports still require confirmation before being added; invalid
JSON and invalid palettes remain rejected. An unchanged theme catalog retains its
model so selection and save acknowledgments do not destroy keyboard focus.

## Catalog verification

Opened the installed AntiNote 2.1.3 Appearance panel on 21 October 2026. Its binary
registry contains 14 themes, all included in Bulletnotes. The installed binary
SHA-256 is `8094e2356d929426958cd4ee3c39038ce8d6175efbbfb2b8ea1a5a5688e5f0c6`.
Fresh native palette extraction matched the bundled base, Markdown, caret,
scrollbar, selection, and flattened grid roles. AntiNote's custom-theme directory
(`Library/Containers/com.chabomakers.Antinote/Data/Documents/Themes`) is empty.
No user notes or credentials were read for the audit.

Bulletnotes also contains all 23 community palettes from the pinned catalog
revision documented in [themes](../themes.md). Both Catppuccin Mocha variants are
present under distinct IDs. There are no missing installed themes or missing
palettes from that snapshot. The resulting picker has 41 themes: four original
Bulletnotes palettes, 14 AntiNote built-ins, and 23 community palettes. This audit
does not claim to include later online community additions. The subsequent
[theme configuration update](theme-configuration.md) includes eight source swatches
and matches AntiNote's theme cards.

## Verification

- The new `theme_selection` integration test reproduced model replacement first,
  then Vancouver's rejected selection. It now exercises all 41 bundled themes
  plus a saved low-contrast import in Light, Dark, System/light, and System/dark.
  It checks production UI callbacks, saved assignments, rendered palette values,
  inactive assignment preservation, unchanged source, retained card models,
  actual Tab/Space/Return card activation, save-failure rollback, and reopening
  the database.
- `scripts/check.sh` passed formatting, strict all-target/all-feature Clippy, and
  349 tests; 24 existing tests remain ignored.
- Native Winit/Skia/Metal `ui_review --appearance-contract` passed all 164 theme
  selections (41 themes across four appearance cases), taking a rendered snapshot
  of each. It uses production AppearanceUi and temporary SQLite, verifies source
  preservation and saving, and retains the existing injected-failure rollback.
- Generated native [Sanrio](theme-selection/sanrio.png) and
  [A24](theme-selection/a24.png) captures were visually inspected. Both were
  previously blocked by selection contrast checks. Native runs retain the
  previously recorded Slint instantiation-chain diagnostic.
- Independent review found no blocking issue. The corrected local app was rebuilt
  and reopened; no saved theme assignment was changed by this follow-up.

Reproduce with `scripts/check.sh`, then build `ui_review` with `--all-features`
and run `target/debug/examples/ui_review --appearance-contract` on macOS.
