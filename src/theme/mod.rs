//! Data-only palettes and independent appearance preferences.
pub mod contrast;
pub mod format;
pub mod palette;
use palette::ThemeEntry;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}
impl Color {
    pub fn parse(input: &str) -> Result<Self, ThemeError> {
        let bytes = input.as_bytes();
        if bytes.len() != 7 || bytes[0] != b'#' || !bytes[1..].iter().all(u8::is_ascii_hexdigit) {
            return Err(ThemeError::InvalidColor);
        }
        let component =
            |range| u8::from_str_radix(&input[range], 16).map_err(|_| ThemeError::InvalidColor);
        Ok(Self {
            r: component(1..3)?,
            g: component(3..5)?,
            b: component(5..7)?,
        })
    }
    pub fn hex(self) -> String {
        format!("#{:02X}{:02X}{:02X}", self.r, self.g, self.b)
    }
    pub fn mix(self, foreground: Self, amount: f64) -> Self {
        let amount = amount.clamp(0.0, 1.0);
        let blend =
            |a: u8, b: u8| (f64::from(a) * (1.0 - amount) + f64::from(b) * amount).round() as u8;
        Self {
            r: blend(self.r, foreground.r),
            g: blend(self.g, foreground.g),
            b: blend(self.b, foreground.b),
        }
    }
}
impl Serialize for Color {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.hex())
    }
}
impl<'de> Deserialize<'de> for Color {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let input = String::deserialize(deserializer)?;
        Self::parse(&input).map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Theme {
    pub schema_version: u32,
    pub name: String,
    pub dark: bool,
    pub paper: Color,
    pub surface: Color,
    pub ink: Color,
    pub muted: Color,
    pub accent: Color,
    pub line: Color,
    pub selection: Color,
}
impl Theme {
    pub fn validate(&self) -> Result<(), ThemeError> {
        if self.schema_version != 1 {
            return Err(ThemeError::UnsupportedVersion);
        }
        if !(1..=32).contains(&self.name.trim().chars().count()) {
            return Err(ThemeError::InvalidName);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ThemeId(pub String);
impl ThemeId {
    pub fn is_builtin(&self) -> bool {
        matches!(self.0.as_str(), "paper" | "graphite" | "matcha" | "rose")
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum AppearanceMode {
    Light,
    Dark,
    #[default]
    System,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SystemAppearance {
    Light,
    Dark,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PaperStyle {
    #[default]
    Plain,
    Dots,
    Ruled,
    Grid,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AppearanceSettings {
    pub mode: AppearanceMode,
    pub light_theme: ThemeId,
    pub dark_theme: ThemeId,
    pub paper: PaperStyle,
    pub font_size: f32,
}
impl Default for AppearanceSettings {
    fn default() -> Self {
        Self {
            mode: AppearanceMode::System,
            light_theme: ThemeId("paper".into()),
            dark_theme: ThemeId("graphite".into()),
            paper: PaperStyle::Plain,
            font_size: 15.0,
        }
    }
}
impl AppearanceSettings {
    pub fn validate(&self) -> Result<(), ThemeError> {
        if !self.font_size.is_finite() || !(11.0..=24.0).contains(&self.font_size) {
            return Err(ThemeError::InvalidFontSize);
        }
        Ok(())
    }
    fn uses_dark(&self, system: SystemAppearance) -> bool {
        self.mode == AppearanceMode::Dark
            || (self.mode == AppearanceMode::System && system == SystemAppearance::Dark)
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedTheme {
    pub id: ThemeId,
    pub palette: Theme,
    pub paper: PaperStyle,
    pub font_size: f32,
    pub pattern: Color,
}

pub fn resolve_theme(
    settings: &AppearanceSettings,
    system: SystemAppearance,
    themes: &[ThemeEntry],
) -> Result<ResolvedTheme, ThemeError> {
    settings.validate()?;
    let id = if settings.uses_dark(system) {
        &settings.dark_theme
    } else {
        &settings.light_theme
    };
    let entry = themes
        .iter()
        .find(|entry| &entry.id == id)
        .ok_or(ThemeError::UnknownTheme)?;
    entry.theme.validate()?;
    Ok(ResolvedTheme {
        id: id.clone(),
        palette: entry.theme.clone(),
        paper: settings.paper,
        font_size: settings.font_size,
        pattern: entry.theme.paper.mix(entry.theme.ink, 0.10),
    })
}

/// Return a candidate; persistence and explicit confirmation belong to the app coordinator.
pub fn select_theme(
    previous: &AppearanceSettings,
    system: SystemAppearance,
    entry: &ThemeEntry,
    allow_low_contrast: bool,
) -> Result<AppearanceSettings, ThemeError> {
    entry.theme.validate()?;
    if !allow_low_contrast && !contrast::check_contrast(&entry.theme).passes() {
        return Err(ThemeError::LowContrast);
    }
    let mut candidate = previous.clone();
    if candidate.uses_dark(system) {
        candidate.dark_theme = entry.id.clone();
    } else {
        candidate.light_theme = entry.id.clone();
    }
    candidate.validate()?;
    Ok(candidate)
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ThemeError {
    #[error("appearance settings are invalid")]
    InvalidSettings,
    #[error("theme exceeds the 32 KiB limit")]
    TooLarge,
    #[error("theme is not valid JSON with unique object keys")]
    InvalidJson,
    #[error("theme fields are missing or have incorrect types")]
    InvalidFields,
    #[error("theme schema version is unsupported")]
    UnsupportedVersion,
    #[error("theme name must contain 1–32 Unicode scalars after trimming")]
    InvalidName,
    #[error("theme colors must be opaque #RRGGBB values")]
    InvalidColor,
    #[error("font size must be between 11 and 24 points")]
    InvalidFontSize,
    #[error("selected theme is unavailable")]
    UnknownTheme,
    #[error("theme needs explicit approval for insufficient contrast")]
    LowContrast,
}

pub mod accessibility;
