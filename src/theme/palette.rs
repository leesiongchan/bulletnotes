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
        include_bytes!("../../assets/themes/antinote/antinote-vancouver.json"),
    ),
    (
        "antinote-tokyo-drift",
        include_bytes!("../../assets/themes/antinote/antinote-tokyo-drift.json"),
    ),
    (
        "antinote-gundam-wing",
        include_bytes!("../../assets/themes/antinote/antinote-gundam-wing.json"),
    ),
    (
        "antinote-vendetta",
        include_bytes!("../../assets/themes/antinote/antinote-vendetta.json"),
    ),
    (
        "antinote-totoro",
        include_bytes!("../../assets/themes/antinote/antinote-totoro.json"),
    ),
    (
        "antinote-mononoke",
        include_bytes!("../../assets/themes/antinote/antinote-mononoke.json"),
    ),
    (
        "antinote-a24",
        include_bytes!("../../assets/themes/antinote/antinote-a24.json"),
    ),
    (
        "antinote-agrabah",
        include_bytes!("../../assets/themes/antinote/antinote-agrabah.json"),
    ),
    (
        "antinote-knight",
        include_bytes!("../../assets/themes/antinote/antinote-knight.json"),
    ),
    (
        "antinote-shadow-moses",
        include_bytes!("../../assets/themes/antinote/antinote-shadow-moses.json"),
    ),
    (
        "antinote-piccolo",
        include_bytes!("../../assets/themes/antinote/antinote-piccolo.json"),
    ),
    (
        "antinote-tartan",
        include_bytes!("../../assets/themes/antinote/antinote-tartan.json"),
    ),
    (
        "antinote-muad-dib",
        include_bytes!("../../assets/themes/antinote/antinote-muad-dib.json"),
    ),
    (
        "antinote-sanrio",
        include_bytes!("../../assets/themes/antinote/antinote-sanrio.json"),
    ),
    (
        "antinote-community-flexoki-dark",
        include_bytes!("../../assets/themes/antinote/antinote-community-flexoki-dark.json"),
    ),
    (
        "antinote-community-flexoki-light",
        include_bytes!("../../assets/themes/antinote/antinote-community-flexoki-light.json"),
    ),
    (
        "antinote-community-north-star",
        include_bytes!("../../assets/themes/antinote/antinote-community-north-star.json"),
    ),
    (
        "antinote-community-ayu-mirage",
        include_bytes!("../../assets/themes/antinote/antinote-community-ayu-mirage.json"),
    ),
    (
        "antinote-community-catppuccin-frappe",
        include_bytes!("../../assets/themes/antinote/antinote-community-catppuccin-frappe.json"),
    ),
    (
        "antinote-community-catppuccin-latte",
        include_bytes!("../../assets/themes/antinote/antinote-community-catppuccin-latte.json"),
    ),
    (
        "antinote-community-catppuccin-macchiato",
        include_bytes!("../../assets/themes/antinote/antinote-community-catppuccin-macchiato.json"),
    ),
    (
        "antinote-community-catppuccin-mocha",
        include_bytes!("../../assets/themes/antinote/antinote-community-catppuccin-mocha.json"),
    ),
    (
        "antinote-community-catpuccinmocha",
        include_bytes!("../../assets/themes/antinote/antinote-community-catpuccinmocha.json"),
    ),
    (
        "antinote-community-dotmatrix",
        include_bytes!("../../assets/themes/antinote/antinote-community-dotmatrix.json"),
    ),
    (
        "antinote-community-dracula",
        include_bytes!("../../assets/themes/antinote/antinote-community-dracula.json"),
    ),
    (
        "antinote-community-gruvbox",
        include_bytes!("../../assets/themes/antinote/antinote-community-gruvbox.json"),
    ),
    (
        "antinote-community-horizon",
        include_bytes!("../../assets/themes/antinote/antinote-community-horizon.json"),
    ),
    (
        "antinote-community-icebergvim",
        include_bytes!("../../assets/themes/antinote/antinote-community-icebergvim.json"),
    ),
    (
        "antinote-community-material-deep-ocean",
        include_bytes!("../../assets/themes/antinote/antinote-community-material-deep-ocean.json"),
    ),
    (
        "antinote-community-monokai",
        include_bytes!("../../assets/themes/antinote/antinote-community-monokai.json"),
    ),
    (
        "antinote-community-neo",
        include_bytes!("../../assets/themes/antinote/antinote-community-neo.json"),
    ),
    (
        "antinote-community-nord",
        include_bytes!("../../assets/themes/antinote/antinote-community-nord.json"),
    ),
    (
        "antinote-community-nordaurora",
        include_bytes!("../../assets/themes/antinote/antinote-community-nordaurora.json"),
    ),
    (
        "antinote-community-oceanicnext",
        include_bytes!("../../assets/themes/antinote/antinote-community-oceanicnext.json"),
    ),
    (
        "antinote-community-onedarkpro",
        include_bytes!("../../assets/themes/antinote/antinote-community-onedarkpro.json"),
    ),
    (
        "antinote-community-solarized",
        include_bytes!("../../assets/themes/antinote/antinote-community-solarized.json"),
    ),
    (
        "antinote-community-wumpus",
        include_bytes!("../../assets/themes/antinote/antinote-community-wumpus.json"),
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
