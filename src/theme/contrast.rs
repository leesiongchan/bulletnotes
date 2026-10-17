//! WCAG relative luminance, evaluated without rounding ratios.
use super::{Color, Theme};
#[derive(Clone, Debug, PartialEq)]
pub struct ContrastPair {
    pub foreground: &'static str,
    pub background: &'static str,
    pub ratio: f64,
    pub required: f64,
}
#[derive(Clone, Debug, PartialEq)]
pub struct ContrastReport {
    pub pairs: Vec<ContrastPair>,
}
impl ContrastReport {
    pub fn passes(&self) -> bool {
        self.pairs.iter().all(|pair| pair.ratio >= pair.required)
    }
    pub fn failures(&self) -> impl Iterator<Item = &ContrastPair> {
        self.pairs.iter().filter(|pair| pair.ratio < pair.required)
    }
}
fn luminance(color: Color) -> f64 {
    let linear = |channel: u8| {
        let value = f64::from(channel) / 255.0;
        if value <= 0.04045 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * linear(color.r) + 0.7152 * linear(color.g) + 0.0722 * linear(color.b)
}
pub fn contrast_ratio(a: Color, b: Color) -> f64 {
    let a = luminance(a);
    let b = luminance(b);
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}
pub fn check_contrast(theme: &Theme) -> ContrastReport {
    let mut pairs = Vec::new();
    for (background, color) in [("paper", theme.paper), ("surface", theme.surface)] {
        for (foreground, fg, required) in [
            ("ink", theme.ink, 4.5),
            ("muted", theme.muted, 4.5),
            ("accent", theme.accent, 4.5),
            ("line", theme.line, 3.0),
        ] {
            pairs.push(ContrastPair {
                foreground,
                background,
                ratio: contrast_ratio(fg, color),
                required,
            });
        }
    }
    pairs.push(ContrastPair {
        foreground: "ink",
        background: "selection",
        ratio: contrast_ratio(theme.ink, theme.selection),
        required: 4.5,
    });
    ContrastReport { pairs }
}
