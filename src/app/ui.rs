//! Presentation mapping; applying appearance never touches the note buffer.
use crate::{
    AboutWindow, AppWindow, MarkdownCommand, MarkdownEdit, ThemeOption, ThemePair,
    UiMarkdownColors, UiTheme, UiThemeSwatches,
    store::settings::AppSettings,
    theme::{
        self, AppearanceMode, AppearanceSettings, PaperStyle, ResolvedTheme, SystemAppearance,
        Theme, ThemeError, palette::ThemeEntry,
    },
};
use slint::{ComponentHandle, Model, ModelRc, VecModel};

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
    window.on_edit_markdown(|source, anchor, caret, command, marker_start, marker_end| {
        use super::source_commands::{CommandResult, Selection, SourceCommand, compute};
        let (Ok(anchor), Ok(caret)) = (usize::try_from(anchor), usize::try_from(caret)) else {
            return MarkdownEdit::default();
        };
        let command = match command {
            MarkdownCommand::ToggleTask if marker_start >= 0 && marker_end >= 0 => {
                SourceCommand::ToggleTaskAt {
                    start: marker_start as usize,
                    end: marker_end as usize,
                }
            }
            MarkdownCommand::ToggleTask => SourceCommand::ToggleTask,
            MarkdownCommand::ContinueList => SourceCommand::ContinueList,
            MarkdownCommand::IndentList => SourceCommand::IndentList,
            MarkdownCommand::OutdentList => SourceCommand::OutdentList,
            MarkdownCommand::Bold => SourceCommand::Bold,
            MarkdownCommand::Italic => SourceCommand::Italic,
            MarkdownCommand::Strikethrough => SourceCommand::Strikethrough,
            MarkdownCommand::CycleHeading => SourceCommand::CycleHeading,
            MarkdownCommand::MoveUp => SourceCommand::MoveUp,
            MarkdownCommand::MoveDown => SourceCommand::MoveDown,
        };
        let result = compute(&source, Selection { anchor, caret }, command);
        match result {
            CommandResult::Native => MarkdownEdit::default(),
            CommandResult::NoChange => MarkdownEdit {
                handled: true,
                ..Default::default()
            },
            CommandResult::Edit(edit) => {
                let (Ok(start), Ok(end), Ok(anchor), Ok(caret)) = (
                    i32::try_from(edit.range.start),
                    i32::try_from(edit.range.end),
                    i32::try_from(edit.selection.anchor),
                    i32::try_from(edit.selection.caret),
                ) else {
                    return MarkdownEdit {
                        handled: true,
                        ..Default::default()
                    };
                };
                MarkdownEdit {
                    handled: true,
                    changed: true,
                    start,
                    end,
                    replacement: edit.replacement.into(),
                    anchor,
                    caret,
                    preserve_scroll: edit.preserve_scroll,
                }
            }
        }
    });
    window.on_highlight_markdown(|source, palette, high_contrast| {
        ModelRc::from(super::markdown::decorations(&source, &palette, high_contrast).as_slice())
    });
    window.on_mark_markdown(|source, palette| {
        ModelRc::from(super::markdown::markers(&source, &palette).as_slice())
    });
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
fn settings_palette(
    settings: &AppearanceSettings,
    system: SystemAppearance,
    themes: &[ThemeEntry],
    high_contrast: bool,
) -> Result<UiTheme, ThemeError> {
    let dark = match settings.mode {
        AppearanceMode::Light => false,
        AppearanceMode::Dark => true,
        AppearanceMode::System => system == SystemAppearance::Dark,
    };
    let id = if dark {
        "bulletnotes-dark"
    } else {
        "bulletnotes"
    };
    let theme = &themes
        .iter()
        .find(|entry| entry.id.0 == id)
        .ok_or(ThemeError::InvalidSettings)?
        .theme;
    // Utility views use neutral chrome, independent of the selected writing paper.
    let mut chrome = theme.clone();
    let colors = if dark {
        [
            "#292929", "#353535", "#F1EFEB", "#B9B6AF", "#DBC9A7", "#696762", "#4B473F",
        ]
    } else {
        [
            "#F2F1ED", "#E7E7E7", "#4E4638", "#706B63", "#66512C", "#B8B8B5", "#DAD6CB",
        ]
    };
    [
        chrome.paper,
        chrome.surface,
        chrome.ink,
        chrome.muted,
        chrome.accent,
        chrome.line,
        chrome.selection,
    ] = [
        theme::Color::parse(colors[0])?,
        theme::Color::parse(colors[1])?,
        theme::Color::parse(colors[2])?,
        theme::Color::parse(colors[3])?,
        theme::Color::parse(colors[4])?,
        theme::Color::parse(colors[5])?,
        theme::Color::parse(colors[6])?,
    ];
    chrome.cursor = Some(chrome.accent);
    chrome.scrollbar = Some(chrome.muted);
    let adjusted = high_contrast.then(|| theme::accessibility::high_contrast(&chrome));
    Ok(palette(adjusted.as_ref().unwrap_or(&chrome), chrome.line))
}

fn apply_about_presentation(window: &AboutWindow, source: &AppWindow) {
    window.set_theme(source.get_settings_theme());
    window.set_high_contrast(source.get_system_high_contrast());
    window.set_reduce_transparency(source.get_prefers_reduced_transparency());
}

/// Refresh cached About windows and keep visible windows in sync with native preferences.
pub fn present_about(window: &AboutWindow, source: &AppWindow) {
    apply_about_presentation(window, source);
    let about = window.as_weak();
    let weak = source.as_weak();
    source.on_about_presentation_changed(move || {
        if let (Some(about), Some(source)) = (about.upgrade(), weak.upgrade()) {
            apply_about_presentation(&about, &source);
        }
    });
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
    window.set_search_cards(settings.search_view == crate::store::settings::SearchView::Cards);
    let themes = settings.themes().map_err(|_| ThemeError::InvalidSettings)?;
    let resolved = theme::resolve_theme(&settings.appearance, system, &themes)?;
    let chrome = settings_palette(
        &settings.appearance,
        system,
        &themes,
        window.get_system_high_contrast(),
    )?;
    let native_order = [
        "bulletnotes",
        "bulletnotes-dark",
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
    window.set_appearance_mode(
        match settings.appearance.mode {
            AppearanceMode::Light => "light",
            AppearanceMode::Dark => "dark",
            AppearanceMode::System => "system",
        }
        .into(),
    );
    window.set_settings_theme(chrome);
    apply_theme(window, &resolved);
    Ok(())
}
