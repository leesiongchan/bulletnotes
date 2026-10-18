//! Presentation mapping; applying appearance never touches the note buffer.
use crate::{
    AppWindow, ThemeOption, ThemePair, UiTheme,
    store::settings::AppSettings,
    theme::{self, AppearanceMode, PaperStyle, ResolvedTheme, SystemAppearance, Theme, ThemeError},
};
use slint::{ModelRc, VecModel};

fn color(value: theme::Color) -> slint::Color {
    slint::Color::from_rgb_u8(value.r, value.g, value.b)
}
fn palette(theme: &Theme, pattern: theme::Color) -> UiTheme {
    UiTheme {
        paper: color(theme.paper),
        surface: color(theme.surface),
        ink: color(theme.ink),
        muted: color(theme.muted),
        accent: color(theme.accent),
        line: color(theme.line),
        selection: color(theme.selection),
        pattern: color(pattern),
        dark: theme.dark,
    }
}
/// Change presentation without replacing the editor or modifying its source text.
pub fn apply_theme(window: &AppWindow, resolved: &ResolvedTheme) {
    let adjusted = window
        .get_system_high_contrast()
        .then(|| theme::accessibility::high_contrast(&resolved.palette));
    window.set_theme(palette(
        adjusted.as_ref().unwrap_or(&resolved.palette),
        resolved.pattern,
    ));
    window.set_font_size_px(resolved.font_size);
    window.set_paper_style(
        match resolved.paper {
            PaperStyle::Plain => "plain",
            PaperStyle::Dots => "dots",
            PaperStyle::Ruled => "ruled",
            PaperStyle::Grid => "grid",
        }
        .into(),
    );
    window.set_selected_theme_id(resolved.id.0.as_str().into());
}
/// Validate before changing any presentation properties.
pub fn present_settings(
    window: &AppWindow,
    settings: &AppSettings,
    system: SystemAppearance,
) -> Result<(), ThemeError> {
    settings
        .validate()
        .map_err(|_| ThemeError::InvalidSettings)?;
    let themes = settings.themes().map_err(|_| ThemeError::InvalidSettings)?;
    let resolved = theme::resolve_theme(&settings.appearance, system, &themes)?;
    let options: Vec<_> = themes
        .iter()
        .map(|entry| ThemeOption {
            id: entry.id.0.as_str().into(),
            name: entry.theme.name.as_str().into(),
            palette: palette(&entry.theme, entry.theme.paper.mix(entry.theme.ink, 0.10)),
        })
        .collect();
    let pairs = options
        .chunks(2)
        .map(|chunk| ThemePair {
            left: chunk[0].clone(),
            right: chunk.get(1).unwrap_or(&chunk[0]).clone(),
            has_right: chunk.len() == 2,
        })
        .collect::<Vec<_>>();
    window.set_theme_options(ModelRc::new(VecModel::from(pairs)));
    window.set_appearance_mode(
        match settings.appearance.mode {
            AppearanceMode::Light => "light",
            AppearanceMode::Dark => "dark",
            AppearanceMode::System => "system",
        }
        .into(),
    );
    apply_theme(window, &resolved);
    Ok(())
}
