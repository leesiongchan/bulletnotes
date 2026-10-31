# Bulletnotes themes

New installations default to System appearance, **Bulletnotes** for light, **Bulletnotes Dark** for dark, Plain paper, and 15 px text. Existing saved choices and imported theme IDs are preserved. The catalog contains 43 built-in themes. All app views, including Settings, Connection, recovery, search, and About, share the selected palette and accessibility adjustments. Paper finishes are Smooth (the default), Bone, Eggshell, and Linen; legacy Theme default choices load as Smooth.

Themes are UTF-8 JSON palette files, at most 32,768 bytes. Version 1 requires `schemaVersion: 1`, a `name` of 1–32 Unicode scalars after trimming, `dark` as a boolean, and seven opaque `#RRGGBB` colors: `paper`, `surface`, `ink`, `muted`, `accent`, `line`, and `selection`. See [Bulletnotes](../assets/themes/bulletnotes.json) for a complete example.

Optional `cursor` and `scrollbar` colors use the same `#RRGGBB` format and survive import, export, and saved settings. Older files keep their existing caret color and use `muted` for the scrollbar. On macOS, Slint renders the caret using the selection brush at full opacity; an explicit `cursor` supplies its RGB and a 20% matching selection tint, preserving native blinking, selection, and IME behavior.

Optional `markdown` colors (`heading1`, `heading2`, `heading3`, `link`, and
`syntax`) preserve distinct source-formatting roles. Optional `grid` sets the
paper-pattern color. All use opaque `#RRGGBB` values and survive import, export,
and settings. Without these fields, existing accent/muted mixes and contrast
fallbacks remain. Explicit Markdown colors are honored in normal mode; Increase
Contrast uses `ink`. List bullets and task boxes always use `ink`.

Optional `swatches` contains exactly eight opaque `#RRGGBB` colors for the theme
card. It survives import, export, and saved settings. The bundled palettes use
`accent1Main`, `accent1Secondary`, `accent1Tertiary`, `accent2Main`, `accent3Main`,
`accent4Main`, `accent5Main`, and `typeHighlight`, in that order. Translucent source
colors are composited over paper. Older themes derive previews from their existing
palette and Markdown colors.

Unknown fields are ignored on import and omitted on export. Duplicate keys are rejected recursively, including inside unknown fields. Invalid JSON, missing fields, unsupported versions, and invalid values reject the entire import. Files are data only; they cannot execute code or load remote resources.

Every import receives a new local ID, even when its display name matches a built-in theme. Light/dark assignments, paper type, paper style, intensity, and text size remain separate preferences. Smooth has no grain; Bone, Eggshell, and Linen work with every palette, including imported copies. The same paper renderer applies finish and pattern throughout the app and in theme previews; materials remain application preferences rather than part of theme JSON schema 1. Increase Contrast and Reduce Transparency remove grain and decorative depth; Reduce Motion disables transitions. Native blur remains deferred.

The built-in Bulletnotes, Bulletnotes Dark, Paper, Graphite, Matcha, and Rose palettes check normal text against paper/surface at 4.5:1, control boundaries at 3:1, and selected text at 4.5:1. Ratios are evaluated without rounding using the [W3C contrast calculation](https://www.w3.org/WAI/WCAG22/Understanding/contrast-minimum.html). New low-contrast file imports require confirmation before they enter the picker.
Every theme already in the picker applies immediately when selected, including
low-contrast palettes. Invalid theme data is still rejected.

The theme picker also bundles 14 featured palettes and 23 community palettes. The palettes retain their authored color components, including low-contrast
palettes. Every bundled palette can be selected directly. Both light and dark assignments can use any palette. These are bundled snapshots and require no network access.

The featured palettes are Vancouver, Tokyo Drift, Gundam Wing, Vendetta, Totoro, Mononoke, Piccolo, A24, Tartan, Agrabah, Muad'Dib, Knight, Sanrio, and Shadow Moses.
The picker presents Bulletnotes and Bulletnotes Dark first, then those featured themes. Square cards use two columns in narrow windows and four in wider windows. The eight swatches form one continuous segmented rectangle. One assignment icon cycles Light → Dark → Both, preserving appearance mode. Vacating a slot restores its previous palette (or a nonmatching built-in fallback); history survives restart and both assignments save atomically. Clicking the card assigns the currently effective slot.
A native capture of the open Tartan note verified its displayed paper, text,
heading, link, and syntax colors against Bulletnotes. Both native renderers apply
macOS color conversion: Tartan background `#F3F1EB` displays as sRGB `#F5F4EF`,
and its main accent `#FF7828` displays as `#FF8D33`. The JSON retains the original
components to avoid applying conversion twice. Other built-ins and community
palettes were not individually compared against native captures. The community palettes retain their source names, including the two distinct Catppuccin Mocha variants (`catppuccin` and `catpuccin-mocha` in the source catalog). See the bundled JSON files for palette values.

The bundled files map source palette roles as follows:

| Bulletnotes | Source role |
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
Other mapped text colors are opaque. Markdown punctuation and strikethrough share the syntax role. No network access is required at runtime.
