# Bulletnotes themes

Themes are UTF-8 JSON palette files, at most 32,768 bytes. Version 1 requires `schemaVersion: 1`, a `name` of 1–32 Unicode scalars after trimming, `dark` as a boolean, and seven opaque `#RRGGBB` colors: `paper`, `surface`, `ink`, `muted`, `accent`, `line`, and `selection`. See [Paper](../assets/themes/paper.json) for a complete example.

Unknown fields are ignored on import and omitted on export. Duplicate keys are rejected recursively, including inside unknown fields. Invalid JSON, missing fields, unsupported versions, and invalid values reject the entire import. Files are data only; they cannot execute code or load remote resources.

Every import receives a new local ID, even when its display name matches a built-in theme. Light and dark assignments, paper pattern, and text size remain separate preferences. The first release uses solid paper; native blur and Antinote theme import are deferred.

The built-in Paper, Graphite, Matcha, and Rose palettes check normal text against paper/surface at 4.5:1, control boundaries at 3:1, and selected text at 4.5:1. Ratios are evaluated without rounding using the [W3C contrast calculation](https://www.w3.org/WAI/WCAG22/Understanding/contrast-minimum.html). Imported low-contrast palettes require explicit confirmation before selection.
