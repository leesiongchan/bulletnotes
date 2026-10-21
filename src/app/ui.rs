//! Presentation mapping; applying appearance never touches the note buffer.
use crate::{
    AppWindow, ThemeOption, ThemePair, UiMarkdownColors, UiTheme, UiThemeSwatches,
    store::settings::AppSettings,
    theme::{self, AppearanceMode, PaperStyle, ResolvedTheme, SystemAppearance, Theme, ThemeError},
};
use slint::{Model, ModelRc, VecModel};

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
        // macOS Slint draws the caret using the selection brush at full opacity.
        cursor: theme
            .cursor
            .map(|value| slint::Color::from_argb_u8(51, value.r, value.g, value.b))
            .unwrap_or_else(|| color(theme.selection)),
        scrollbar: color(theme.scrollbar.unwrap_or(theme.muted)),
        pattern: color(pattern),
        markdown: theme
            .markdown
            .as_ref()
            .map(|colors| UiMarkdownColors {
                heading1: color(colors.heading1),
                heading2: color(colors.heading2),
                heading3: color(colors.heading3),
                link: color(colors.link),
                syntax: color(colors.syntax),
            })
            .unwrap_or_default(),
        dark: theme.dark,
    }
}
/// Change presentation without replacing the editor or modifying its source text.
pub fn apply_theme(window: &AppWindow, resolved: &ResolvedTheme) {
    // The pure callback binding refreshes for every source and effective palette change.
    window.on_highlight_markdown(|source, palette, high_contrast| {
        ModelRc::from(super::markdown::decorations(&source, &palette, high_contrast).as_slice())
    });
    window.on_mark_markdown(|source, palette| {
        ModelRc::from(super::markdown::markers(&source, &palette).as_slice())
    });
    tracing::debug!(
        slint_version = "1.18.1",
        "Markdown source styles and marker palette applied"
    );
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
    let native_order = [
        "antinote-vancouver",
        "antinote-tokyo-drift",
        "antinote-gundam-wing",
        "antinote-vendetta",
        "antinote-totoro",
        "antinote-mononoke",
        "antinote-piccolo",
        "antinote-a24",
        "antinote-tartan",
        "antinote-agrabah",
        "antinote-muad-dib",
        "antinote-knight",
        "antinote-sanrio",
        "antinote-shadow-moses",
    ];
    let mut ordered: Vec<_> = themes.iter().collect();
    ordered.sort_by_key(|entry| {
        native_order
            .iter()
            .position(|id| *id == entry.id.0)
            .unwrap_or(native_order.len())
    });
    let options: Vec<_> = ordered
        .into_iter()
        .map(|entry| ThemeOption {
            id: entry.id.0.as_str().into(),
            name: entry.theme.name.as_str().into(),
            palette: palette(
                &entry.theme,
                entry
                    .theme
                    .grid
                    .unwrap_or_else(|| entry.theme.paper.mix(entry.theme.ink, 0.10)),
            ),
            swatches: {
                let theme = &entry.theme;
                let [
                    primary,
                    secondary,
                    tertiary,
                    accent2,
                    accent3,
                    accent4,
                    accent5,
                    highlight,
                ] = theme
                    .swatches
                    .unwrap_or([
                        theme.accent,
                        theme.markdown.as_ref().map_or(theme.muted, |c| c.heading2),
                        theme.markdown.as_ref().map_or(theme.line, |c| c.heading3),
                        theme.ink,
                        theme.muted,
                        theme.line,
                        theme.selection,
                        theme.scrollbar.unwrap_or(theme.muted),
                    ])
                    .map(color);
                UiThemeSwatches {
                    primary,
                    secondary,
                    tertiary,
                    accent2,
                    accent3,
                    accent4,
                    accent5,
                    highlight,
                }
            },
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
    // Replacing an unchanged catalog destroys the focused theme card.
    if !window.get_theme_options().iter().eq(pairs.iter().cloned()) {
        window.set_theme_options(ModelRc::new(VecModel::from(pairs)));
    }
    window.set_selected_light_theme_id(settings.appearance.light_theme.0.as_str().into());
    window.set_selected_dark_theme_id(settings.appearance.dark_theme.0.as_str().into());
    window.set_system_appearance_dark(system == SystemAppearance::Dark);
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
