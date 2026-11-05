# UI conventions

The interface is Slint markup in `ui/`, compiled by `build.rs` from
`ui/app-window.slint`. Rust presents state through the generated `AppWindow` and
`AboutWindow` types; `src/app/ui.rs` holds the shared conversions.

## Theme and tokens

`ui/components/theme.slint` defines one global for state and four for constants.

| Global | Kind | Contents |
| --- | --- | --- |
| `Theme` | state, set by Rust | `palette` (a `UiTheme`), `high-contrast`, `reduce-transparency`, `reduced-motion`, and the derived `material-effects` |
| `Typography` | constant | font families and the six text sizes, `caption` to `display` |
| `Radius` | constant | corner radii from `tight` (fields) to `window` |
| `Spacing` | constant | gaps `xs`–`xl` and the full-page margin `page` |
| `Motion` | constant | `reveal`, `conceal` and `fade` durations |

Slint globals belong to one root window, so each root aliases them as properties
(`in-out property <UiTheme> theme <=> Theme.palette;`) and Rust sets those
properties on every window. Components read `Theme.palette.ink` or
`Spacing.lg` directly; they do not take the palette or tokens as properties.

Use a token whenever a value already exists in the scale. A new size that is
not in the scale is a design decision: add it to the token global with a
comment saying where it is used, rather than writing a literal in one view.
Changing a token value is a visible change and needs the screenshot comparison
below.

A component takes its own palette property only when it must draw something
other than the window's current theme. The one case today is `PaperSurface`,
whose `theme` defaults to `Theme.palette` and is overridden by the theme cards
in Appearance so each card previews its own palette. `material-effects`
follows the same rule: `ChoiceButton` and `PaperSurface` default it to
`Theme.material-effects`, and settings controls switch it off for a flat
surface.

`ThemeContext` keeps Slint's standard widgets in the same light or dark scheme;
place one in each root window.

## Area adapters

A view with its own state and actions declares them in an exported global next
to the view, instead of passing each value through `AppWindow`.
`RecoveryAdapter` in `ui/recovery-view.slint` holds the review list, the
selected item's sources, the merge draft and the actions the panel raises.
`SearchAdapter` in `ui/components/page-browser.slint` holds results, the query
and folder, selection and drag state, and the page actions.
`AppearanceAdapter` in `ui/components/appearance-panel.slint` holds the
appearance and general preferences and the panel's actions; the window also
reads its font size, paper and footer preferences. `app-window.slint`
re-exports all three, and Rust uses them through the window:

```rust
let recovery = window.global::<RecoveryAdapter>();
recovery.set_items(ModelRc::new(VecModel::from(rows)));
recovery.on_select(move |id| { /* load the item */ });
```

The view still takes properties for state it does not own, such as
`lifecycle-pending` on `RecoveryPanel`, and keeps navigation callbacks like
`return-to-note` on `AppWindow`. A global exists once per window, so this
fits views that appear once.

Slint's `changed` handlers only watch a component's own properties. A view that
reacts to adapter state declares a private mirror, such as
`private property <string> query: SearchAdapter.query;`, and keeps its
`changed query => …` handler. `tests/recovery_panel.rs` and
`tests/search_preview.rs` and `tests/settings_presentation.rs` check that view
actions reach the adapters and that adapter changes reach the views.

## Enums shared with Rust

Closed sets of UI states are Slint enums in `ui/types.slint`, not strings.

- An enum that mirrors a Rust domain type takes a `Ui` prefix (`UiView`,
  `UiRecoveryKind`, `UiAppearanceMode`, `UiPaperType`, `UiPaperStyle`,
  `UiPaperIntensity`). The prefix keeps the generated Rust names apart from the
  domain types such as `View` and `RecoveryKind`.
- An enum that exists only in the interface, such as `SettingsCategory`, has no
  prefix and no Rust counterpart.

Convert at the boundary with `mirror_enum!` in `src/app/ui.rs`. It writes both
`From` impls as exhaustive matches, so adding a variant on either side fails to
compile until the mapping is updated:

```rust
mirror_enum!(RecoveryKind <=> UiRecoveryKind {
    Conflict <=> Conflict,
    Trash <=> Trash,
    Copy <=> Copy,
});

window.set_paper_style(resolved.paper.into());
```

## Accessible choices

Slint requires `accessible-role` to be a constant, so a button cannot switch to
a radio role through a binding. Use the derived components in
`ui/components/action-button.slint`:

- `ChoiceOption` and `ActionOption` are `ChoiceButton` and `ActionButton` with
  `accessible-role: radio-button`, `accessible-checkable: true` and
  `accessible-checked` bound to `selected`. Use them for one-of-many choices
  such as paper style or settings category.
- Plain `ChoiceButton` and `ActionButton` are for commands.
- `SettingToggle` in `settings-controls.slint` is a checkable `switch`.

Screen readers only announce the checked state of checkable elements.
`tests/accessibility_states.rs` checks roles and states through the testing
backend's `ElementHandle`. That lookup needs debug info, which `build.rs` turns
on for every profile except `release`.

## Checking visual changes

Refactors of `.slint` files should leave every pixel unchanged. Intended visual
changes should be reviewed as before/after images. The method:

1. Write a temporary integration test that sets the Slint testing backend with
   the Skia renderer, binds `AppWindow` with `present_settings`, and walks each
   view (note, pages, appearance categories, connection, recovery sections,
   dialogs). Repeat for several bundled palettes, light and dark, and at least
   two window sizes. Save `window.window().take_snapshot()` for each case as a
   PNG, and record a SHA-256 hash of each image.
2. Run it on the base commit and on the change, writing to separate
   directories.
3. Compare the hash lists. A refactor must match exactly. For intended changes,
   look at the differing images side by side.
4. Delete the test before committing. Screenshot output depends on fonts and
   the renderer, so it is a review aid, not a regression test.

Behaviour that can be asserted, such as line height, marker colors or caret
reveal, belongs in a permanent test like `tests/markdown_markers.rs`.
