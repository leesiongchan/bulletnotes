//! Atomic application settings contain no authentication credentials.
use crate::{
    domain::{connection::ConnectionId, error::StoreError},
    theme::{
        AppearanceSettings, SystemAppearance, ThemeId,
        palette::{ThemeEntry, builtin_themes},
        resolve_theme,
    },
};
use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WindowPlacement {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub pinned: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HotkeySettings {
    pub key: String,
    pub alt: bool,
    pub control: bool,
    pub shift: bool,
    pub command: bool,
}
impl Default for HotkeySettings {
    fn default() -> Self {
        Self {
            key: "KeyA".into(),
            alt: true,
            control: false,
            shift: false,
            command: false,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AppSettings {
    pub schema_version: u32,
    pub appearance: AppearanceSettings,
    pub imported_themes: Vec<ThemeEntry>,
    pub active_connection: Option<ConnectionId>,
    pub window: Option<WindowPlacement>,
    pub hotkey: HotkeySettings,
}
impl Default for AppSettings {
    fn default() -> Self {
        Self {
            schema_version: 1,
            appearance: AppearanceSettings::default(),
            imported_themes: Vec::new(),
            active_connection: None,
            window: None,
            hotkey: HotkeySettings::default(),
        }
    }
}
impl AppSettings {
    pub fn themes(&self) -> Result<Vec<ThemeEntry>, StoreError> {
        let mut themes = builtin_themes().map_err(|_| StoreError::InvalidSettings)?;
        themes.extend(self.imported_themes.iter().cloned());
        Ok(themes)
    }
    pub fn validate(&self) -> Result<(), StoreError> {
        if self.schema_version != 1 {
            return Err(StoreError::InvalidSettings);
        }
        self.appearance
            .validate()
            .map_err(|_| StoreError::InvalidSettings)?;
        let mut ids = HashSet::<&ThemeId>::new();
        for entry in &self.imported_themes {
            if entry.id.is_builtin() || !ids.insert(&entry.id) {
                return Err(StoreError::InvalidSettings);
            }
            let suffix = entry
                .id
                .0
                .strip_prefix("import-")
                .ok_or(StoreError::InvalidSettings)?;
            uuid::Uuid::parse_str(suffix).map_err(|_| StoreError::InvalidSettings)?;
            entry
                .theme
                .validate()
                .map_err(|_| StoreError::InvalidSettings)?;
        }
        let themes = self.themes()?;
        // Validate both assignments even if one is currently inactive.
        let mut settings = self.appearance.clone();
        settings.mode = crate::theme::AppearanceMode::System;
        for system in [SystemAppearance::Light, SystemAppearance::Dark] {
            resolve_theme(&settings, system, &themes).map_err(|_| StoreError::InvalidSettings)?;
        }
        if let Some(window) = &self.window
            && (![window.x, window.y, window.width, window.height]
                .iter()
                .all(|v| v.is_finite())
                || window.width < 400.0
                || window.height < 440.0)
        {
            return Err(StoreError::InvalidSettings);
        }
        let key = &self.hotkey.key;
        let valid_key = key
            .strip_prefix("Key")
            .is_some_and(|s| s.len() == 1 && s.as_bytes()[0].is_ascii_uppercase())
            || key
                .strip_prefix("Digit")
                .is_some_and(|s| s.len() == 1 && s.as_bytes()[0].is_ascii_digit())
            || key == "Space"
            || key
                .strip_prefix('F')
                .and_then(|s| s.parse::<u8>().ok())
                .is_some_and(|n| (1..=24).contains(&n));
        if !valid_key
            || !(self.hotkey.alt || self.hotkey.control || self.hotkey.shift || self.hotkey.command)
        {
            return Err(StoreError::InvalidSettings);
        }
        Ok(())
    }
}
pub(super) fn load(db: &Connection) -> Result<AppSettings, StoreError> {
    let json = db
        .query_row("SELECT value FROM settings WHERE id=1", [], |row| {
            row.get::<_, String>(0)
        })
        .optional()?;
    let settings = match json {
        Some(json) => serde_json::from_str(&json).map_err(|_| StoreError::InvalidSettings)?,
        None => AppSettings::default(),
    };
    settings.validate()?;
    Ok(settings)
}
pub(super) fn save(db: &mut Connection, settings: AppSettings) -> Result<(), StoreError> {
    settings.validate()?;
    let json = serde_json::to_string(&settings).map_err(|_| StoreError::InvalidSettings)?;
    let tx = db.transaction()?;
    tx.execute("INSERT INTO settings(id,value) VALUES(1,?1) ON CONFLICT(id) DO UPDATE SET value=excluded.value",[json])?;
    tx.commit()?;
    Ok(())
}
