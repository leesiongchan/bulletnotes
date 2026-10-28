use bulletnotes::{
    app::theme_files::{ThemeFileError, load_theme_file, save_theme_file},
    theme::{ThemeError, format::decode_theme},
};
#[test]
fn import_reads_only_the_bounded_own_format() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("theme.json");
    let valid = include_bytes!("fixtures/themes/valid.json");
    let mut padded = valid.to_vec();
    padded.resize(32768, b' ');
    std::fs::write(&path, &padded).unwrap();
    assert_eq!(
        load_theme_file(&path).unwrap(),
        decode_theme(valid).unwrap()
    );
    padded.push(b' ');
    std::fs::write(&path, padded).unwrap();
    assert!(matches!(
        load_theme_file(&path),
        Err(ThemeFileError::Theme(ThemeError::TooLarge))
    ));
    std::fs::write(&path, b"[Theme]\nbackground=white").unwrap();
    assert!(load_theme_file(&path).is_err());
}
#[test]
fn export_atomically_replaces_selected_file_with_valid_palette() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("theme.json");
    std::fs::write(&path, b"previous export").unwrap();
    let theme = decode_theme(include_bytes!("fixtures/themes/valid.json")).unwrap();
    save_theme_file(&path, &theme).unwrap();
    assert_eq!(load_theme_file(&path).unwrap(), theme);
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
}
#[test]
fn invalid_export_preserves_existing_file() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("theme.json");
    let old = b"previous export";
    std::fs::write(&path, old).unwrap();
    let mut invalid = decode_theme(include_bytes!("fixtures/themes/valid.json")).unwrap();
    invalid.name.clear();
    assert!(save_theme_file(&path, &invalid).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), old);
}
