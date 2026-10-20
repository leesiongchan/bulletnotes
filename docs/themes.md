# Bulletnotes themes

Themes are UTF-8 JSON palette files, at most 32,768 bytes. Version 1 requires `schemaVersion: 1`, a `name` of 1–32 Unicode scalars after trimming, `dark` as a boolean, and seven opaque `#RRGGBB` colors: `paper`, `surface`, `ink`, `muted`, `accent`, `line`, and `selection`. See [Paper](../assets/themes/paper.json) for a complete example.

Optional `cursor` and `scrollbar` colors use the same `#RRGGBB` format and survive import, export, and saved settings. Older files keep their existing caret color and use `muted` for the scrollbar. On macOS, Slint renders the caret using the selection brush at full opacity; an explicit `cursor` supplies its RGB and a 20% matching selection tint, preserving native blinking, selection, and IME behavior.

Headings and Markdown links use `accent` when its contrast against paper is at least
4.5:1, otherwise `ink`. Increase Contrast uses `ink`. This adds no palette fields;
selection foreground keeps the existing ink color. List bullets and task boxes
always use `ink`, including Increase Contrast, with no additional palette fields.

Unknown fields are ignored on import and omitted on export. Duplicate keys are rejected recursively, including inside unknown fields. Invalid JSON, missing fields, unsupported versions, and invalid values reject the entire import. Files are data only; they cannot execute code or load remote resources.

Every import receives a new local ID, even when its display name matches a built-in theme. Light and dark assignments, paper pattern, and text size remain separate preferences. The first release uses solid paper; native blur and direct import of Antinote-format JSON remain deferred.

The built-in Paper, Graphite, Matcha, and Rose palettes check normal text against paper/surface at 4.5:1, control boundaries at 3:1, and selected text at 4.5:1. Ratios are evaluated without rounding using the [W3C contrast calculation](https://www.w3.org/WAI/WCAG22/Understanding/contrast-minimum.html). Imported low-contrast palettes require explicit confirmation before selection.

The theme picker also bundles all 14 Antinote 2.1.3 built-ins and all 23 JSON palettes in its [community catalog at revision a19824d](https://github.com/johnsonfung/antinote-extensions/tree/a19824dc3bdcc1b9c12cc8323ec007070b3a3a70/themes-community). They retain their published colors, including low-contrast palettes, and use the same confirmation before selection. Both light and dark assignments can use any palette. These are bundled snapshots and require no network access.

Antinote's built-ins are Vancouver, Tokyo Drift, Gundam Wing, Vendetta, Totoro, Mononoke, Piccolo, A24, Tartan, Agrabah, Muad'Dib, Knight, Sanrio, and Shadow Moses. Their colors were read from the installed Antinote 2.1.3 palette definitions. The community palettes retain their source names, including the two distinct Catppuccin Mocha variants (`catppuccin` and `catpuccin-mocha` in the source catalog). See the linked catalog for contributors and original theme files.

The [Antinote theme maker](https://antinote.io/theme-maker) defines `accent1Main` as its caret color and `typeHighlight` as its scrollbar color. The bundled files map the supported roles as follows:

| Bulletnotes | Antinote |
| --- | --- |
| `paper` | `background` |
| `surface` | `backgroundFade` |
| `ink` | `typeMain` |
| `muted` | `typeSubtle` |
| `accent`, `cursor` | `accent1Main` |
| `line` | `typeLight` |
| `selection` | 20% cursor mixed onto paper |
| `scrollbar` | `typeHighlight` |

North Star's translucent white background and fade become solid white; its `#0000000D` scrollbar becomes the equivalent `#F2F2F2` over white. The other bundled palettes' mapped colors are already opaque. Antinote's additional Markdown accent and grid roles are not part of Bulletnotes' palette format.
