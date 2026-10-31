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
pub struct MarkdownColors {
    pub heading1: Color,
    pub heading2: Color,
    pub heading3: Color,
    pub link: Color,
    pub syntax: Color,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<Color>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scrollbar: Option<Color>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub markdown: Option<MarkdownColors>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grid: Option<Color>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub swatches: Option<[Color; 8]>,
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
        palette::BUILTIN_THEME_FILES
            .iter()
            .any(|(id, _)| self.0 == *id)
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
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PaperType {
    #[default]
    #[serde(alias = "theme")]
    Smooth,
    Bone,
    Eggshell,
    Linen,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PaperIntensity {
    Subtle,
    #[default]
    Clear,
    Bold,
}
fn enabled() -> bool {
    true
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AppearanceSettings {
    pub mode: AppearanceMode,
    pub light_theme: ThemeId,
    pub dark_theme: ThemeId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_light_theme: Option<ThemeId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_dark_theme: Option<ThemeId>,
    pub paper: PaperStyle,
    pub font_size: f32,
    #[serde(default)]
    pub paper_type: PaperType,
    #[serde(default)]
    pub paper_intensity: PaperIntensity,
    #[serde(default)]
    pub keep_bottom_bar_visible: bool,
    #[serde(default = "enabled")]
    pub show_note_count: bool,
    #[serde(default = "enabled")]
    pub show_note_date: bool,
    #[serde(default = "enabled")]
    pub show_newest_indicator: bool,
}
impl Default for AppearanceSettings {
    fn default() -> Self {
        Self {
            mode: AppearanceMode::System,
            light_theme: ThemeId("bulletnotes".into()),
            dark_theme: ThemeId("bulletnotes-dark".into()),
            previous_light_theme: None,
            previous_dark_theme: None,
            paper: PaperStyle::Plain,
            font_size: 15.0,
            paper_type: PaperType::Smooth,
            paper_intensity: PaperIntensity::Clear,
            keep_bottom_bar_visible: false,
            show_note_count: true,
            show_note_date: true,
            show_newest_indicator: true,
        }
    }
}
impl AppearanceSettings {
    pub(crate) fn assign_theme(&mut self, id: ThemeId, slot: SystemAppearance) {
        let (current, previous) = match slot {
            SystemAppearance::Light => (&mut self.light_theme, &mut self.previous_light_theme),
            SystemAppearance::Dark => (&mut self.dark_theme, &mut self.previous_dark_theme),
        };
        if *current != id {
            *previous = Some(std::mem::replace(current, id));
        }
    }
    pub(crate) fn restore_theme(&mut self, slot: SystemAppearance, excluded: &ThemeId) {
        let (previous, default, alternative) = match slot {
            SystemAppearance::Light => (&self.previous_light_theme, "bulletnotes", "paper"),
            SystemAppearance::Dark => (&self.previous_dark_theme, "bulletnotes-dark", "graphite"),
        };
        let restored = previous
            .as_ref()
            .filter(|id| *id != excluded)
            .cloned()
            .unwrap_or_else(|| {
                ThemeId(
                    if excluded.0 == default {
                        alternative
                    } else {
                        default
                    }
                    .into(),
                )
            });
        self.assign_theme(restored, slot);
    }
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
    pub paper_type: PaperType,
}

pub(crate) fn resolve_paper_pattern(theme: &Theme, intensity: PaperIntensity) -> Color {
    let pattern = theme
        .grid
        .unwrap_or_else(|| theme.paper.mix(theme.ink, 0.10));
    match intensity {
        PaperIntensity::Subtle => theme.paper.mix(pattern, 0.55),
        PaperIntensity::Clear => pattern,
        PaperIntensity::Bold => pattern.mix(theme.ink, 0.10),
    }
}

pub fn resolve_theme(
    settings: &AppearanceSettings,
    system: SystemAppearance,
    themes: &[ThemeEntry],
) -> Result<ResolvedTheme, ThemeError> {
    settings.validate()?;
    for previous in [
        &settings.previous_light_theme,
        &settings.previous_dark_theme,
    ]
    .into_iter()
    .flatten()
    {
        if !themes.iter().any(|entry| entry.id == *previous) {
            return Err(ThemeError::UnknownTheme);
        }
    }
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
        pattern: resolve_paper_pattern(&entry.theme, settings.paper_intensity),
        paper_type: settings.paper_type,
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
    let slot = if candidate.uses_dark(system) {
        SystemAppearance::Dark
    } else {
        SystemAppearance::Light
    };
    candidate.assign_theme(entry.id.clone(), slot);
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
