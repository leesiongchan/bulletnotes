use bulletnotes::theme::{
    Color,
    contrast::{check_contrast, contrast_ratio},
    palette::builtin_themes,
};
#[test]
fn black_and_white_have_twenty_one_to_one_contrast() {
    let ratio = contrast_ratio(
        Color::parse("#000000").unwrap(),
        Color::parse("#FFFFFF").unwrap(),
    );
    assert!((ratio - 21.0).abs() < f64::EPSILON);
}
#[test]
fn original_builtins_meet_contrast() {
    let themes: Vec<_> = builtin_themes()
        .unwrap()
        .into_iter()
        .filter(|entry| !entry.id.0.starts_with("antinote-"))
        .collect();
    assert_eq!(themes.len(), 4);
    for entry in themes {
        let report = check_contrast(&entry.theme);
        for pair in &report.pairs {
            assert!(
                pair.ratio >= pair.required,
                "{} {} / {}: {} < {}",
                entry.theme.name,
                pair.foreground,
                pair.background,
                pair.ratio,
                pair.required
            );
        }
        assert!(report.passes());
        assert!(report.pairs.iter().any(|p| p.required == 4.5));
        assert!(report.pairs.iter().any(|p| p.required == 3.0));
    }
}

#[test]
fn system_high_contrast_changes_only_the_presentation_palette() {
    use bulletnotes::theme::{
        accessibility::high_contrast,
        contrast::{check_contrast, contrast_ratio},
        palette::builtin_themes,
    };
    for entry in builtin_themes().unwrap() {
        let original = entry.theme.clone();
        let adjusted = high_contrast(&original);
        assert!(contrast_ratio(adjusted.ink, adjusted.paper) >= 7.0);
        assert!(contrast_ratio(adjusted.ink, adjusted.surface) >= 7.0);
        assert!(check_contrast(&adjusted).passes());
        assert_eq!(entry.theme, original);
    }
    let mut low = builtin_themes().unwrap().remove(0).theme;
    low.paper = bulletnotes::theme::Color::parse("#777777").unwrap();
    low.ink = low.paper;
    low.selection = low.paper;
    assert!(check_contrast(&high_contrast(&low)).passes());
}
