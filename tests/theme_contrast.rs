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
fn builtins_meet_contrast() {
    let themes = builtin_themes().unwrap();
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
