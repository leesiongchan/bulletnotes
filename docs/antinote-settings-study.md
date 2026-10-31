# AntiNote settings study

Observed on 2026-10-31 in the installed Antinote 2.1.3 macOS application.
The Settings window was opened and each of its eleven categories inspected.
Appearance controls were also read through macOS accessibility. Other categories
were inspected visually at their initial scroll position; this is not a complete
inventory of controls below the fold. No preferences were intentionally changed.
Observed labels describe the reference UI, not verified runtime behavior.

## Design observations

- A persistent icon-and-label toolbar switches between categories and shows the
  current selection. The window title follows the selected category.
- Each category uses a scrolling form with section headings, rounded groups,
  subtle row separators, and consistent spacing.
- Labels lead each row; controls align at the trailing edge. Longer explanations
  appear beneath labels in secondary text.
- Small exclusive choices use segmented controls. Boolean options use switches;
  larger choice sets use menus. Actions use explicit button labels.
- Appearance provides independent light and dark theme assignments. Controls
  distinguish paper type, paper intensity, and text size.
- Status and dependency explanations appear near affected actions. For example,
  Updates explains that privacy settings have disabled update checks.

## Capabilities and fit

| Category | Visible reference capabilities | Relationship to Bulletnotes |
| --- | --- | --- |
| General | iCloud note/preference sync, launch at login, new note at startup, language, menu bar/Dock presence | Retain SilverBullet sync; startup and OS integration need separate platform decisions. |
| Appearance | Text sizes, doubled text size, paper types/intensity, translucency, footer information, theme assignments, custom themes/icons | Existing text size, paper, theme slots and import/export are reusable. Paper intensity and footer preferences are direct extensions. |
| Editor | Indentation continuation, macOS text replacement, spelling, Vim mode, automatic task/bullet/numbered lists | Bulletnotes already has Markdown commands and list continuation. Native spelling and Vim mode require separate editor work. |
| Notes | Automatic new notes, expiry, archive visibility, locked-note authentication timing | Preserve the current no-expiry behavior and recoverable Trash. Locking and automatic expiry change product behavior. |
| Keywords | Custom list/math activation words, completion keywords, task completion markers, list style | A keyword interpreter would be a separate feature; preserve Markdown source semantics. |
| Shortcuts | Action list, selected shortcut details, reset/remove actions | Move the existing global shortcut configuration here and document existing commands. General shortcut remapping requires broader dispatch changes. |
| Timer | Quit behavior, menu bar countdown, notification/full-screen/sound alerts | Requires a timer subsystem; no equivalent settings exist today. |
| Export | Text, Markdown, other-app destinations, custom URL schemes, quick-export destination | File export and application integrations require real implementations before settings can advertise them. |
| Extensions | Extension enablement, loaded extension list, community browsing, secure API keys | Requires an extension runtime and permission model. |
| Privacy | Network access overview and controls by purpose | Explain actual SilverBullet/diagnostic behavior. Network switches require sync lifecycle work; do not imply they already exist. |
| Updates | Current version, update checks/install/beta preferences, privacy dependency state | Bulletnotes currently distributes through release artifacts; an updater is separate work. |

## Current project implementation

- `ui/components/appearance-panel.slint` contains a single scrolling Settings
  panel combining writing, appearance, themes, global shortcut, and connection.
- `ui/app-window.slint` hosts that panel inside the existing app window and
  implements the note header/footer and newest-note indicator.
- `src/theme/mod.rs` defines independent appearance preferences and resolves
  palettes. `src/store/settings.rs` validates and persists application settings.
- `src/app/appearance_ui.rs` serializes durable settings writes and restores
  committed state after failed saves. New preferences should use that ownership
  model rather than add competing writers.
- `tests/settings_presentation.rs` already covers narrow-window keyboard access,
  contrast, and persistence failure behavior. `scripts/check.sh` is the standard
  formatting, strict Clippy, and test entry point.

## Implementation approaches for selection

1. **Adapt design and relevant preferences (recommended).** Introduce Appearance,
   Editor, Shortcuts, and Connection navigation, with grouped form controls.
   Keep existing settings functional; add paper intensity and footer
   visibility/detail preferences, plus a reference for existing shortcuts.
   Reuse Rust/Slint, current persistence, theme identities, and platform behavior.
   New settings must default to current rendering and load older settings safely.
2. **Redesign existing settings only.** Apply category navigation and grouped
   forms to current capabilities. This is smaller but adds no missing preferences.
3. **Full capability parity.** Include the timer, keyword interpreter, extension
   system, exports, locking, updater, and OS integrations as separately designed
   features. This substantially expands the project beyond settings presentation.

The user selected General and Appearance first, with fitting capabilities only,
and explicitly requested paper types separate from paper style. See
[the scoped design](superpowers/specs/2026-10-31-general-appearance-design.md)
and [implementation plan](superpowers/plans/2026-10-31-general-appearance.md).

## Verification for a subsequent implementation

Preserve existing settings, shortcut registration failure recovery, theme
assignments, pending note edits, caret/selection, and the durable save barrier.
Verify new settings across restart and failed writes, and load settings without
new fields. Check actual rendered layouts at the 400 × 440 minimum and larger
sizes, keyboard navigation, selected states, light/dark appearance, increased
contrast, and reduced motion. Compare native before/after screenshots; this is a
Slint desktop app rather than a browser UI. Run the repository checks and report
platform behavior that could not be exercised.
