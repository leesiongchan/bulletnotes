use bulletnotes::{
    app::appearance::AppearanceController,
    store::{StoreHandle, settings::AppSettings},
    theme::{format::decode_theme, *},
};
fn controller() -> AppearanceController {
    AppearanceController::new(AppSettings::default(), SystemAppearance::Light).unwrap()
}
#[test]
fn invalid_import_keeps_previous_palette() {
    let mut appearance = controller();
    let previous = appearance.resolved().unwrap();
    let invalid = decode_theme(b"not JSON");
    assert!(invalid.is_err());
    assert_eq!(appearance.resolved().unwrap(), previous);
    let mut low = decode_theme(include_bytes!("fixtures/themes/valid.json")).unwrap();
    low.ink = low.paper;
    assert!(matches!(
        appearance.confirm_import(low, false),
        Err(ThemeError::LowContrast)
    ));
    assert_eq!(appearance.resolved().unwrap(), previous);
    assert_eq!(appearance.committed(), &AppSettings::default());
}
#[test]
fn same_named_import_preserves_builtin_and_waits_for_commit() {
    let mut appearance = controller();
    let theme = decode_theme(include_bytes!("fixtures/themes/valid.json")).unwrap();
    let candidate = appearance.confirm_import(theme, false).unwrap();
    assert_eq!(candidate.imported_themes.len(), 1);
    assert_eq!(candidate.imported_themes[0].theme.name, "Paper");
    assert_ne!(candidate.appearance.light_theme, ThemeId("paper".into()));
    assert_eq!(appearance.committed(), &AppSettings::default());
    assert_eq!(appearance.resolved().unwrap().id, ThemeId("paper".into()));
    appearance.preview_candidate(&candidate, false).unwrap();
    assert_eq!(
        appearance.resolved().unwrap().id,
        candidate.appearance.light_theme
    );
    let ticket = appearance
        .begin_settings_commit(candidate.clone(), false)
        .unwrap();
    appearance.complete_settings_commit(ticket, Ok(())).unwrap();
    assert_eq!(appearance.committed(), &candidate);
    assert!(
        candidate
            .themes()
            .unwrap()
            .iter()
            .any(|entry| entry.id == ThemeId("paper".into()))
    );
}
#[tokio::test]
async fn failed_settings_commit_restores_last_durable_palette() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("settings.sqlite");
    let store = StoreHandle::open(&path).await.unwrap();
    let previous = AppSettings::default();
    store.save_settings(previous.clone()).await.unwrap();
    let mut appearance = controller();
    assert!(
        appearance
            .preview(ThemeId("matcha".into()))
            .unwrap()
            .passes()
    );
    assert_eq!(appearance.resolved().unwrap().id, ThemeId("matcha".into()));
    assert_eq!(appearance.committed(), &previous);
    let candidate = appearance
        .selection_candidate(ThemeId("matcha".into()), false)
        .unwrap();
    let injector = rusqlite::Connection::open(&path).unwrap();
    injector.execute_batch("CREATE TRIGGER fail_theme BEFORE UPDATE ON settings BEGIN SELECT RAISE(ABORT,'settings failure'); END;").unwrap();
    let ticket = appearance.begin_settings_commit(candidate, false).unwrap();
    let result = store.save_settings(ticket.settings().clone()).await;
    assert!(appearance.complete_settings_commit(ticket, result).is_err());
    assert_eq!(appearance.committed(), &previous);
    assert_eq!(appearance.resolved().unwrap().id, ThemeId("paper".into()));
    assert_eq!(store.load_settings().await.unwrap(), previous);
    store.shutdown().await.unwrap();
}
#[test]
fn theme_switch_keeps_independent_pattern_font_and_dark_assignment() {
    let mut settings = AppSettings::default();
    settings.appearance.paper = PaperStyle::Ruled;
    settings.appearance.font_size = 21.0;
    let mut appearance =
        AppearanceController::new(settings.clone(), SystemAppearance::Light).unwrap();
    appearance.preview(ThemeId("rose".into())).unwrap();
    let candidate = appearance
        .selection_candidate(ThemeId("rose".into()), false)
        .unwrap();
    assert_eq!(candidate.appearance.paper, PaperStyle::Ruled);
    assert_eq!(candidate.appearance.font_size, 21.0);
    assert_eq!(
        candidate.appearance.dark_theme,
        settings.appearance.dark_theme
    );
    appearance.set_system_appearance(SystemAppearance::Dark);
    assert_eq!(
        appearance.resolved().unwrap().id,
        settings.appearance.dark_theme
    );
    assert_eq!(appearance.committed(), &settings);
}

#[test]
fn older_settings_acknowledgement_keeps_newer_preview() {
    let mut appearance = controller();
    appearance.preview(ThemeId("matcha".into())).unwrap();
    let older = appearance
        .selection_candidate(ThemeId("matcha".into()), false)
        .unwrap();
    let ticket = appearance
        .begin_settings_commit(older.clone(), false)
        .unwrap();
    appearance.preview(ThemeId("rose".into())).unwrap();
    appearance.complete_settings_commit(ticket, Ok(())).unwrap();
    assert_eq!(appearance.committed(), &older);
    assert_eq!(appearance.resolved().unwrap().id, ThemeId("rose".into()));
}

#[test]
fn stale_failure_does_not_cancel_a_newer_preview_of_the_same_palette() {
    use bulletnotes::domain::error::StoreError;
    let mut appearance = controller();
    let candidate = appearance
        .selection_candidate(ThemeId("matcha".into()), false)
        .unwrap();
    let ticket = appearance.begin_settings_commit(candidate, false).unwrap();
    appearance.preview(ThemeId("rose".into())).unwrap();
    appearance.preview(ThemeId("matcha".into())).unwrap();
    assert!(
        appearance
            .complete_settings_commit(ticket, Err(StoreError::Closed))
            .is_err()
    );
    assert_eq!(appearance.resolved().unwrap().id, ThemeId("matcha".into()));
    assert_eq!(appearance.committed(), &AppSettings::default());
}
