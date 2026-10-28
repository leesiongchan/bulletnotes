//! Local palette identity never becomes part of the export format.
use super::{Theme, ThemeError, ThemeId, format::decode_theme};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThemeEntry {
    pub id: ThemeId,
    pub theme: Theme,
}
impl ThemeEntry {
    pub fn import(theme: Theme) -> Self {
        Self {
            id: ThemeId(format!("import-{}", uuid::Uuid::new_v4())),
            theme,
        }
    }
}
pub(super) const BUILTIN_THEME_FILES: &[(&str, &[u8])] = &[
    ("paper", include_bytes!("../../assets/themes/paper.json")),
    (
        "graphite",
        include_bytes!("../../assets/themes/graphite.json"),
    ),
    ("matcha", include_bytes!("../../assets/themes/matcha.json")),
    ("rose", include_bytes!("../../assets/themes/rose.json")),
    (
        "antinote-vancouver",
        include_bytes!("../../assets/themes/bundled/bundled-vancouver.json"),
    ),
    (
        "antinote-tokyo-drift",
        include_bytes!("../../assets/themes/bundled/bundled-tokyo-drift.json"),
    ),
    (
        "antinote-gundam-wing",
        include_bytes!("../../assets/themes/bundled/bundled-gundam-wing.json"),
    ),
    (
        "antinote-vendetta",
        include_bytes!("../../assets/themes/bundled/bundled-vendetta.json"),
    ),
    (
        "antinote-totoro",
        include_bytes!("../../assets/themes/bundled/bundled-totoro.json"),
    ),
    (
        "antinote-mononoke",
        include_bytes!("../../assets/themes/bundled/bundled-mononoke.json"),
    ),
    (
        "antinote-a24",
        include_bytes!("../../assets/themes/bundled/bundled-a24.json"),
    ),
    (
        "antinote-agrabah",
        include_bytes!("../../assets/themes/bundled/bundled-agrabah.json"),
    ),
    (
        "antinote-knight",
        include_bytes!("../../assets/themes/bundled/bundled-knight.json"),
    ),
    (
        "antinote-shadow-moses",
        include_bytes!("../../assets/themes/bundled/bundled-shadow-moses.json"),
    ),
    (
        "antinote-piccolo",
        include_bytes!("../../assets/themes/bundled/bundled-piccolo.json"),
    ),
    (
        "antinote-tartan",
        include_bytes!("../../assets/themes/bundled/bundled-tartan.json"),
    ),
    (
        "antinote-muad-dib",
        include_bytes!("../../assets/themes/bundled/bundled-muad-dib.json"),
    ),
    (
        "antinote-sanrio",
        include_bytes!("../../assets/themes/bundled/bundled-sanrio.json"),
    ),
    (
        "antinote-community-flexoki-dark",
        include_bytes!("../../assets/themes/bundled/bundled-community-flexoki-dark.json"),
    ),
    (
        "antinote-community-flexoki-light",
        include_bytes!("../../assets/themes/bundled/bundled-community-flexoki-light.json"),
    ),
    (
        "antinote-community-north-star",
        include_bytes!("../../assets/themes/bundled/bundled-community-north-star.json"),
    ),
    (
        "antinote-community-ayu-mirage",
        include_bytes!("../../assets/themes/bundled/bundled-community-ayu-mirage.json"),
    ),
    (
        "antinote-community-catppuccin-frappe",
        include_bytes!("../../assets/themes/bundled/bundled-community-catppuccin-frappe.json"),
    ),
    (
        "antinote-community-catppuccin-latte",
        include_bytes!("../../assets/themes/bundled/bundled-community-catppuccin-latte.json"),
    ),
    (
        "antinote-community-catppuccin-macchiato",
        include_bytes!("../../assets/themes/bundled/bundled-community-catppuccin-macchiato.json"),
    ),
    (
        "antinote-community-catppuccin-mocha",
        include_bytes!("../../assets/themes/bundled/bundled-community-catppuccin-mocha.json"),
    ),
    (
        "antinote-community-catpuccinmocha",
        include_bytes!("../../assets/themes/bundled/bundled-community-catpuccinmocha.json"),
    ),
    (
        "antinote-community-dotmatrix",
        include_bytes!("../../assets/themes/bundled/bundled-community-dotmatrix.json"),
    ),
    (
        "antinote-community-dracula",
        include_bytes!("../../assets/themes/bundled/bundled-community-dracula.json"),
    ),
    (
        "antinote-community-gruvbox",
        include_bytes!("../../assets/themes/bundled/bundled-community-gruvbox.json"),
    ),
    (
        "antinote-community-horizon",
        include_bytes!("../../assets/themes/bundled/bundled-community-horizon.json"),
    ),
    (
        "antinote-community-icebergvim",
        include_bytes!("../../assets/themes/bundled/bundled-community-icebergvim.json"),
    ),
    (
        "antinote-community-material-deep-ocean",
        include_bytes!("../../assets/themes/bundled/bundled-community-material-deep-ocean.json"),
    ),
    (
        "antinote-community-monokai",
        include_bytes!("../../assets/themes/bundled/bundled-community-monokai.json"),
    ),
    (
        "antinote-community-neo",
        include_bytes!("../../assets/themes/bundled/bundled-community-neo.json"),
    ),
    (
        "antinote-community-nord",
        include_bytes!("../../assets/themes/bundled/bundled-community-nord.json"),
    ),
    (
        "antinote-community-nordaurora",
        include_bytes!("../../assets/themes/bundled/bundled-community-nordaurora.json"),
    ),
    (
        "antinote-community-oceanicnext",
        include_bytes!("../../assets/themes/bundled/bundled-community-oceanicnext.json"),
    ),
    (
        "antinote-community-onedarkpro",
        include_bytes!("../../assets/themes/bundled/bundled-community-onedarkpro.json"),
    ),
    (
        "antinote-community-solarized",
        include_bytes!("../../assets/themes/bundled/bundled-community-solarized.json"),
    ),
    (
        "antinote-community-wumpus",
        include_bytes!("../../assets/themes/bundled/bundled-community-wumpus.json"),
    ),
];

pub fn builtin_themes() -> Result<Vec<ThemeEntry>, ThemeError> {
    BUILTIN_THEME_FILES
        .iter()
        .map(|(id, bytes)| {
            Ok(ThemeEntry {
                id: ThemeId((*id).to_owned()),
                theme: decode_theme(bytes)?,
            })
        })
        .collect()
}
