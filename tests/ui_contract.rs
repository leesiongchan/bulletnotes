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

#[test]
#[ignore = "opens the generated-text native UI fixture; build example ui_review first"]
fn native_palette_and_view_changes_preserve_source() {
    let executable = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("examples/ui_review");
    let output = std::process::Command::new(executable)
        .arg("--contract")
        .env("SLINT_BACKEND", "winit-skia")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("native_ui_contract_passed: true"));
}

#[test]
#[ignore = "opens generated-text native shortcut fixture; build ui_review first"]
fn command_shortcuts_dispatch_actions_without_editing_note_source() {
    let executable = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("examples/ui_review");
    let output = std::process::Command::new(executable)
        .arg("--shortcut-contract")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("native_shortcut_contract_passed: true")
    );
}

#[test]
#[ignore = "opens generated-text native fixture; build ui_review first"]
fn native_appearance_actions_persist_and_roll_back_on_failure() {
    let executable = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("examples/ui_review");
    let output = std::process::Command::new(executable)
        .arg("--appearance-contract")
        .env("SLINT_BACKEND", "winit-skia")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("native_appearance_contract_passed: true"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
#[ignore = "captures the generated native review window; build ui_review first"]
fn floating_chrome_header_meets_top_edge_at_maximum_text_size() {
    let executable = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("examples/ui_review");
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("large-text.png");
    for shown in [false, true] {
        let mut command = std::process::Command::new(&executable);
        if shown {
            command.arg("--show-chrome");
        }
        let output = command
            .args(["--ruled", "--font", "24", "--capture"])
            .arg(&path)
            .env("SLINT_BACKEND", "winit-skia")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let decoder =
            png::Decoder::new(std::io::BufReader::new(std::fs::File::open(&path).unwrap()));
        let mut reader = decoder.read_info().unwrap();
        let mut pixels = vec![0; reader.output_buffer_size().unwrap()];
        let frame = reader.next_frame(&mut pixels).unwrap();
        let channels = match frame.color_type {
            png::ColorType::Rgb => 3,
            png::ColorType::Rgba => 4,
            other => panic!("unsupported capture format: {other:?}"),
        };
        for y in [4, 20, frame.height as usize - 30] {
            let expected = if shown && y > 44 {
                [243, 240, 231]
            } else {
                [252, 250, 244]
            };
            let mut surface_pixels = 0;
            for x in 40..frame.width as usize - 40 {
                let offset = (y * frame.width as usize + x) * channels;
                if pixels[offset..offset + 3]
                    .iter()
                    .zip(expected)
                    .all(|(actual, expected)| actual.abs_diff(expected) <= 2)
                {
                    surface_pixels += 1;
                }
            }
            assert!(
                surface_pixels > frame.width as usize / 2,
                "chrome surface mismatch (shown={shown}, y={y}): {surface_pixels}"
            );
        }
    }
}

#[test]
#[ignore = "opens generated-text native connection form; build ui_review first"]
fn native_connection_form_preserves_editor_and_clears_credentials() {
    let executable = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("examples/ui_review");
    let output = std::process::Command::new(executable)
        .arg("--connection-contract")
        .env("SLINT_BACKEND", "winit-skia")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("native_connection_contract_passed: true")
    );
}

#[test]
#[ignore = "opens the owned native floating-chrome fixture; build ui_review first"]
fn native_floating_chrome_preserves_editor_and_handles_input_lifecycle() {
    let executable = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("examples/ui_review");
    let output = std::process::Command::new(executable)
        .arg("--floating-contract")
        .env_remove("SLINT_BACKEND")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("floating_chrome_contract_passed: true")
    );
}

fn chrome_window() -> bulletnotes::AppWindow {
    use slint::ComponentHandle;
    slint::platform::set_platform(Box::new(i_slint_backend_testing::TestingBackend::new(
        i_slint_backend_testing::TestingBackendOptions {
            mock_time: true,
            threading: false,
            renderer_name: Some("skia".into()),
        },
    )))
    .unwrap();
    let ui = bulletnotes::AppWindow::new().unwrap();
    bulletnotes::app::ui::present_settings(&ui, &AppSettings::default(), SystemAppearance::Light)
        .unwrap();
    ui.set_prefers_reduced_motion(true);
    ui.set_note_text("Generated chrome proof".into());
    ui.show().unwrap();
    ui.window()
        .dispatch_event(slint::platform::WindowEvent::WindowActiveChanged(true));
    ui.invoke_focus_editor();
    ui.set_header_hovered(true);
    ui.set_footer_hovered(true);
    i_slint_backend_testing::mock_elapsed_time(std::time::Duration::from_millis(50));
    ui
}
fn chrome_pixel(ui: &bulletnotes::AppWindow, x: usize, y: usize) -> [u8; 3] {
    use slint::ComponentHandle;
    let image = ui.window().take_snapshot().unwrap();
    let p = image.as_bytes();
    let i = (y * image.width() as usize + x) * 4;
    [p[i], p[i + 1], p[i + 2]]
}
#[test]
fn header_uses_paper_and_footer_keeps_surface() {
    let ui = chrome_window();
    assert_eq!(chrome_pixel(&ui, 250, 10), [252, 250, 244]);
    assert_eq!(chrome_pixel(&ui, 250, 504), [243, 240, 231]);
}
#[test]
fn circular_idle_focus_and_contrast_are_distinct() {
    use slint::ComponentHandle;
    let ui = chrome_window();
    let idle = ui.window().take_snapshot().unwrap();
    // Left edge of the pin circle is paper until accessibility contrast adds its outline.
    assert_eq!(chrome_pixel(&ui, 463, 22), [252, 250, 244]);
    ui.set_system_high_contrast(true);
    let contrast = ui.window().take_snapshot().unwrap();
    assert!(idle.as_bytes() != contrast.as_bytes());
    assert_ne!(chrome_pixel(&ui, 463, 22), [252, 250, 244]);
    ui.window()
        .dispatch_event(slint::platform::WindowEvent::KeyPressed {
            text: slint::platform::Key::F6.into(),
        });
    i_slint_backend_testing::mock_elapsed_time(std::time::Duration::from_millis(50));
    i_slint_backend_testing::mock_elapsed_time(std::time::Duration::from_millis(50));
    let focus = ui.window().take_snapshot().unwrap();
    assert!(focus.as_bytes() != contrast.as_bytes());
}
#[test]
fn minimum_chrome_retains_path_and_actions() {
    use slint::ComponentHandle;
    let ui = chrome_window();
    ui.window().set_size(slint::LogicalSize::new(400.0, 440.0));
    ui.set_current_path("generated/long/path/that/must/elide/without/overlapping.md".into());
    let before = (
        ui.get_note_text(),
        ui.get_editor_width(),
        ui.get_editor_height(),
    );
    assert_eq!(ui.window().take_snapshot().unwrap().width(), 400);
    ui.set_header_hovered(false);
    ui.set_footer_hovered(false);
    i_slint_backend_testing::mock_elapsed_time(std::time::Duration::ZERO);
    i_slint_backend_testing::mock_elapsed_time(std::time::Duration::from_millis(150));
    assert_eq!(
        (
            ui.get_note_text(),
            ui.get_editor_width(),
            ui.get_editor_height()
        ),
        before
    );
}
#[test]
fn hidden_chrome_keeps_attention_visible() {
    let ui = chrome_window();
    ui.set_header_hovered(false);
    ui.set_footer_hovered(false);
    i_slint_backend_testing::mock_elapsed_time(std::time::Duration::ZERO);
    i_slint_backend_testing::mock_elapsed_time(std::time::Duration::from_millis(150));
    ui.set_status_attention(true);
    ui.set_status_text("Local save failed. Retry.".into());
    assert!(!ui.get_header_revealed() && !ui.get_footer_revealed());
    assert_eq!(chrome_pixel(&ui, 25, 460), [243, 240, 231]);
    assert!(ui.get_status_text().contains("Retry"));
}
