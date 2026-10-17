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
pub fn builtin_themes() -> Result<Vec<ThemeEntry>, ThemeError> {
    [
        (
            "paper",
            include_bytes!("../../assets/themes/paper.json").as_slice(),
        ),
        (
            "graphite",
            include_bytes!("../../assets/themes/graphite.json").as_slice(),
        ),
        (
            "matcha",
            include_bytes!("../../assets/themes/matcha.json").as_slice(),
        ),
        (
            "rose",
            include_bytes!("../../assets/themes/rose.json").as_slice(),
        ),
    ]
    .into_iter()
    .map(|(id, bytes)| {
        Ok(ThemeEntry {
            id: ThemeId(id.to_owned()),
            theme: decode_theme(bytes)?,
        })
    })
    .collect()
}
