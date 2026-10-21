//! System accessibility policy changes presentation, never saved theme data.
use super::{Color, Theme, contrast::contrast_ratio};
pub fn high_contrast(theme: &Theme) -> Theme {
    let mut result = theme.clone();
    let (paper, ink) = if theme.dark {
        (
            Color { r: 0, g: 0, b: 0 },
            Color {
                r: 255,
                g: 255,
                b: 255,
            },
        )
    } else {
        (
            Color {
                r: 255,
                g: 255,
                b: 255,
            },
            Color { r: 0, g: 0, b: 0 },
        )
    };
    result.ink = ink;
    result.muted = ink;
    result.accent = ink;
    result.line = ink;
    result.cursor = theme.cursor.map(|_| ink);
    result.scrollbar = theme.scrollbar.map(|_| ink);
    result.markdown = theme.markdown.as_ref().map(|_| super::MarkdownColors {
        heading1: ink,
        heading2: ink,
        heading3: ink,
        link: ink,
        syntax: ink,
    });
    // Retain the paper's tint when it already reaches enhanced text contrast.
    for step in 0..=100 {
        let candidate = theme.paper.mix(paper, f64::from(step) / 100.0);
        if contrast_ratio(ink, candidate) >= 7.0 {
            result.paper = candidate;
            break;
        }
    }
    result.surface = result.paper;
    result.selection = result.paper.mix(ink, 0.10);
    result
}
