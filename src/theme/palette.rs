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
    (
        "bulletnotes",
        include_bytes!("../../assets/themes/bulletnotes.json"),
    ),
    (
        "bulletnotes-dark",
        include_bytes!("../../assets/themes/bulletnotes-dark.json"),
    ),
    ("paper", include_bytes!("../../assets/themes/paper.json")),
    (
        "graphite",
        include_bytes!("../../assets/themes/graphite.json"),
    ),
    ("matcha", include_bytes!("../../assets/themes/matcha.json")),
    ("rose", include_bytes!("../../assets/themes/rose.json")),
    (
        "bundled-vancouver",
        include_bytes!("../../assets/themes/bundled/bundled-vancouver.json"),
    ),
    (
        "bundled-tokyo-drift",
        include_bytes!("../../assets/themes/bundled/bundled-tokyo-drift.json"),
    ),
    (
        "bundled-gundam-wing",
        include_bytes!("../../assets/themes/bundled/bundled-gundam-wing.json"),
    ),
    (
        "bundled-vendetta",
        include_bytes!("../../assets/themes/bundled/bundled-vendetta.json"),
    ),
    (
        "bundled-totoro",
        include_bytes!("../../assets/themes/bundled/bundled-totoro.json"),
    ),
    (
        "bundled-mononoke",
        include_bytes!("../../assets/themes/bundled/bundled-mononoke.json"),
    ),
    (
        "bundled-a24",
        include_bytes!("../../assets/themes/bundled/bundled-a24.json"),
    ),
    (
        "bundled-agrabah",
        include_bytes!("../../assets/themes/bundled/bundled-agrabah.json"),
    ),
    (
        "bundled-knight",
        include_bytes!("../../assets/themes/bundled/bundled-knight.json"),
    ),
    (
        "bundled-shadow-moses",
        include_bytes!("../../assets/themes/bundled/bundled-shadow-moses.json"),
    ),
    (
        "bundled-piccolo",
        include_bytes!("../../assets/themes/bundled/bundled-piccolo.json"),
    ),
    (
        "bundled-tartan",
        include_bytes!("../../assets/themes/bundled/bundled-tartan.json"),
    ),
    (
        "bundled-muad-dib",
        include_bytes!("../../assets/themes/bundled/bundled-muad-dib.json"),
    ),
    (
        "bundled-sanrio",
        include_bytes!("../../assets/themes/bundled/bundled-sanrio.json"),
    ),
    (
        "bundled-community-flexoki-dark",
        include_bytes!("../../assets/themes/bundled/bundled-community-flexoki-dark.json"),
    ),
    (
        "bundled-community-flexoki-light",
        include_bytes!("../../assets/themes/bundled/bundled-community-flexoki-light.json"),
    ),
    (
        "bundled-community-north-star",
        include_bytes!("../../assets/themes/bundled/bundled-community-north-star.json"),
    ),
    (
        "bundled-community-ayu-mirage",
        include_bytes!("../../assets/themes/bundled/bundled-community-ayu-mirage.json"),
    ),
    (
        "bundled-community-catppuccin-frappe",
        include_bytes!("../../assets/themes/bundled/bundled-community-catppuccin-frappe.json"),
    ),
    (
        "bundled-community-catppuccin-latte",
        include_bytes!("../../assets/themes/bundled/bundled-community-catppuccin-latte.json"),
    ),
    (
        "bundled-community-catppuccin-macchiato",
        include_bytes!("../../assets/themes/bundled/bundled-community-catppuccin-macchiato.json"),
    ),
    (
        "bundled-community-catppuccin-mocha",
        include_bytes!("../../assets/themes/bundled/bundled-community-catppuccin-mocha.json"),
    ),
    (
        "bundled-community-catpuccinmocha",
        include_bytes!("../../assets/themes/bundled/bundled-community-catpuccinmocha.json"),
    ),
    (
        "bundled-community-dotmatrix",
        include_bytes!("../../assets/themes/bundled/bundled-community-dotmatrix.json"),
    ),
    (
        "bundled-community-dracula",
        include_bytes!("../../assets/themes/bundled/bundled-community-dracula.json"),
    ),
    (
        "bundled-community-gruvbox",
        include_bytes!("../../assets/themes/bundled/bundled-community-gruvbox.json"),
    ),
    (
        "bundled-community-horizon",
        include_bytes!("../../assets/themes/bundled/bundled-community-horizon.json"),
    ),
    (
        "bundled-community-icebergvim",
        include_bytes!("../../assets/themes/bundled/bundled-community-icebergvim.json"),
    ),
    (
        "bundled-community-material-deep-ocean",
        include_bytes!("../../assets/themes/bundled/bundled-community-material-deep-ocean.json"),
    ),
    (
        "bundled-community-monokai",
        include_bytes!("../../assets/themes/bundled/bundled-community-monokai.json"),
    ),
    (
        "bundled-community-neo",
        include_bytes!("../../assets/themes/bundled/bundled-community-neo.json"),
    ),
    (
        "bundled-community-nord",
        include_bytes!("../../assets/themes/bundled/bundled-community-nord.json"),
    ),
    (
        "bundled-community-nordaurora",
        include_bytes!("../../assets/themes/bundled/bundled-community-nordaurora.json"),
    ),
    (
        "bundled-community-oceanicnext",
        include_bytes!("../../assets/themes/bundled/bundled-community-oceanicnext.json"),
    ),
    (
        "bundled-community-onedarkpro",
        include_bytes!("../../assets/themes/bundled/bundled-community-onedarkpro.json"),
    ),
    (
        "bundled-community-solarized",
        include_bytes!("../../assets/themes/bundled/bundled-community-solarized.json"),
    ),
    (
        "bundled-community-wumpus",
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
