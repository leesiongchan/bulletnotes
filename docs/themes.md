# Bulletnotes themes

Themes are UTF-8 JSON palette files, at most 32,768 bytes. Version 1 requires `schemaVersion: 1`, a `name` of 1–32 Unicode scalars after trimming, `dark` as a boolean, and seven opaque `#RRGGBB` colors: `paper`, `surface`, `ink`, `muted`, `accent`, `line`, and `selection`. See [Paper](../assets/themes/paper.json) for a complete example.

Optional `cursor` and `scrollbar` colors use the same `#RRGGBB` format and survive import, export, and saved settings. Older files keep their existing caret color and use `muted` for the scrollbar. On macOS, Slint renders the caret using the selection brush at full opacity; an explicit `cursor` supplies its RGB and a 20% matching selection tint, preserving native blinking, selection, and IME behavior.

Optional `markdown` colors (`heading1`, `heading2`, `heading3`, `link`, and
`syntax`) preserve distinct source-formatting roles. Optional `grid` sets the
paper-pattern color. All use opaque `#RRGGBB` values and survive import, export,
and settings. Without these fields, existing accent/muted mixes and contrast
fallbacks remain. Explicit Markdown colors are honored in normal mode; Increase
Contrast uses `ink`. List bullets and task boxes always use `ink`.

Optional `swatches` contains exactly eight opaque `#RRGGBB` colors for the theme
card. It survives import, export, and saved settings. The AntiNote palettes use
`accent1Main`, `accent1Secondary`, `accent1Tertiary`, `accent2Main`, `accent3Main`,
`accent4Main`, `accent5Main`, and `typeHighlight`, in that order. Translucent source
colors are composited over paper. Older themes derive previews from their existing
palette and Markdown colors.

Unknown fields are ignored on import and omitted on export. Duplicate keys are rejected recursively, including inside unknown fields. Invalid JSON, missing fields, unsupported versions, and invalid values reject the entire import. Files are data only; they cannot execute code or load remote resources.

Every import receives a new local ID, even when its display name matches a built-in theme. Light and dark assignments, paper pattern, and text size remain separate preferences. The first release uses solid paper; native blur and direct import of Antinote-format JSON remain deferred.

The built-in Paper, Graphite, Matcha, and Rose palettes check normal text against paper/surface at 4.5:1, control boundaries at 3:1, and selected text at 4.5:1. Ratios are evaluated without rounding using the [W3C contrast calculation](https://www.w3.org/WAI/WCAG22/Understanding/contrast-minimum.html). New low-contrast file imports require confirmation before they enter the picker.
Every theme already in the picker applies immediately when selected, including
low-contrast palettes. Invalid theme data is still rejected.

The theme picker also bundles all 14 Antinote 2.1.3 built-ins and all 23 JSON palettes in its [community catalog at revision a19824d](https://github.com/johnsonfung/antinote-extensions/tree/a19824dc3bdcc1b9c12cc8323ec007070b3a3a70/themes-community). The palettes retain their authored color components, including low-contrast
palettes. Every bundled palette can be selected directly. Both light and dark assignments can use any palette. These are bundled snapshots and require no network access.

Antinote's built-ins are Vancouver, Tokyo Drift, Gundam Wing, Vendetta, Totoro, Mononoke, Piccolo, A24, Tartan, Agrabah, Muad'Dib, Knight, Sanrio, and Shadow Moses. Their colors were read from the installed Antinote 2.1.3 palette definitions.
The picker presents those themes first in AntiNote's two-column order. Each card's
sun and moon assign its palette to Light and Dark independently, preserving the
current appearance mode. Clicking the card assigns the currently effective slot.
A native capture of the open Tartan note verified its displayed paper, text,
heading, link, and syntax colors against Bulletnotes. Both native renderers apply
macOS color conversion: Tartan background `#F3F1EB` displays as sRGB `#F5F4EF`,
and its main accent `#FF7828` displays as `#FF8D33`. The JSON retains the original
components to avoid applying conversion twice. Other built-ins and community
palettes were not individually compared against native captures. The community palettes retain their source names, including the two distinct Catppuccin Mocha variants (`catppuccin` and `catpuccin-mocha` in the source catalog). See the linked catalog for contributors and original theme files.

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
| `markdown.heading1` | `accent1Main` |
| `markdown.heading2` | `accent1Secondary` |
| `markdown.heading3` | `accent1Tertiary` |
| `markdown.link` | `typeSubtlePlus` |
| `markdown.syntax` | `typeLight` |
| `grid` | `gridSuperlight`, composited over paper |

North Star's translucent white background and fade become solid white; its `#0000000D` scrollbar becomes the equivalent `#F2F2F2` over white. Piccolo and Knight also flatten their translucent second-heading colors over paper.
Other mapped text colors are opaque. Markdown punctuation and strikethrough share the syntax role. No runtime access
to Antinote is required.

[Theme-selection checks and installed catalog audit](validation/theme-selection.md)
cover every palette through the production appearance callbacks and persistence.
See [theme configuration evidence](validation/theme-configuration.md) for native
layout comparisons and captures.
