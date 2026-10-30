//! Render synthetic UI states with the real editor; never open storage or a connection.
use bulletnotes::{
    AboutWindow, AppWindow, PageRow,
    app::ui::{present_about, present_settings},
    store::settings::AppSettings,
    theme::{AppearanceMode, SystemAppearance},
};
use slint::ComponentHandle;
use std::{fs, io::Write, path::Path, time::Duration};

fn capture(
    window: &slint::Window,
    output: &Path,
    name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let _ = window.take_snapshot()?;
    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(250));
    let image = window.take_snapshot()?;
    let path = output.join(format!("{name}.pam"));
    let mut file = fs::File::create(&path)?;
    write!(
        file,
        "P7\nWIDTH {}\nHEIGHT {}\nDEPTH 4\nMAXVAL 255\nTUPLTYPE RGB_ALPHA\nENDHDR\n",
        image.width(),
        image.height()
    )?;
    file.write_all(image.as_bytes())?;
    println!("Rendered {}", path.display());
    Ok(())
}

fn scroll_settings_to_top(window: &AppWindow) {
    window
        .window()
        .dispatch_event(slint::platform::WindowEvent::PointerScrolled {
            position: slint::LogicalPosition::new(250.0, 300.0),
            delta_x: 0.0,
            delta_y: 20000.0,
        });
    window
        .window()
        .dispatch_event(slint::platform::WindowEvent::PointerExited);
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = std::env::args_os().skip(1);
    let output = arguments.next().ok_or("provide one output directory")?;
    if arguments.next().is_some() {
        return Err("provide exactly one output directory".into());
    }
    let output = Path::new(&output);
    fs::create_dir_all(output)?;
    slint::platform::set_platform(Box::new(i_slint_backend_testing::TestingBackend::new(
        i_slint_backend_testing::TestingBackendOptions {
            mock_time: true,
            threading: false,
            renderer_name: Some("skia".into()),
        },
    )))?;
    let window = AppWindow::new()?;
    let about = AboutWindow::new()?;
    about.set_app_version("Version 0.0.0 · development".into());
    window.set_note_text("# A little space\n\nFor the next thought.\n\n## Today\n- [x] Take the long way home\n- [ ] Sketch a smaller first version\n- [ ] Share something worth keeping\n\nA notebook for **unfinished ideas**.\n\n[Open the studio](https://example.com)\n\ncafé · 日本語 · 👩🏽‍💻".into());
    window.set_note_read_only(true);
    window.set_current_path("Quick Notes/Scratchpad.md".into());
    window.set_note_date("Today, 10:42".into());
    window.set_note_counter("3 of 8".into());
    window.set_status_text("Saved locally".into());
    window.set_can_go_older(true);
    window.set_prefers_reduced_motion(true);
    window.show()?;
    for (scheme, mode) in [
        ("light", AppearanceMode::Light),
        ("dark", AppearanceMode::Dark),
    ] {
        let mut settings = AppSettings::default();
        settings.appearance.mode = mode;
        present_settings(&window, &settings, SystemAppearance::Light)?;
        for (size, width, height) in [
            ("minimum", 400.0, 440.0),
            ("default", 505.0, 552.0),
            ("wide", 800.0, 700.0),
        ] {
            window
                .window()
                .set_size(slint::LogicalSize::new(width, height));
            for state in ["editor", "chrome", "find", "settings"] {
                window.invoke_reset_chrome();
                window.set_active_view(
                    if state == "settings" {
                        "appearance"
                    } else {
                        "note"
                    }
                    .into(),
                );
                window.set_find_visible(state == "find");
                window.set_find_query("thought".into());
                window.set_find_status("1 of 1".into());
                if state == "chrome" {
                    window.set_chrome_active(true);
                    window.set_header_hovered(true);
                    window.set_footer_hovered(true);
                }
                if state == "settings" {
                    scroll_settings_to_top(&window);
                }
                capture(window.window(), output, &format!("{scheme}-{size}-{state}"))?;
            }
        }
        window
            .window()
            .set_size(slint::LogicalSize::new(505.0, 552.0));
        for preference in [
            "focus",
            "high-contrast",
            "reduce-transparency",
            "reduce-motion",
        ] {
            window.set_system_high_contrast(preference == "high-contrast");
            window.set_prefers_reduced_transparency(preference == "reduce-transparency");
            window.set_prefers_reduced_motion(preference == "reduce-motion");
            present_settings(&window, &settings, SystemAppearance::Light)?;
            for state in ["chrome", "settings"] {
                window.invoke_reset_chrome();
                window.set_find_visible(false);
                window.set_active_view(
                    if state == "settings" {
                        "appearance"
                    } else {
                        "note"
                    }
                    .into(),
                );
                if state == "chrome" {
                    window.set_chrome_active(true);
                    window.set_header_hovered(true);
                    window.set_footer_hovered(true);
                    if preference == "focus" {
                        window
                            .window()
                            .dispatch_event(slint::platform::WindowEvent::KeyPressed {
                                text: slint::platform::Key::F6.into(),
                            });
                        i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(32));
                    }
                }
                if state == "settings" {
                    scroll_settings_to_top(&window);
                }
                capture(
                    window.window(),
                    output,
                    &format!("{scheme}-{preference}-{state}"),
                )?;
            }
        }
        window.set_system_high_contrast(false);
        window.set_prefers_reduced_transparency(false);
        window.set_prefers_reduced_motion(true);
        present_settings(&window, &settings, SystemAppearance::Light)?;
        window.set_active_view("note".into());
        window.set_chrome_active(true);
        window.set_header_hovered(true);
        window.set_footer_hovered(true);
        let position = slint::LogicalPosition::new(479.0, 22.0);
        window
            .window()
            .dispatch_event(slint::platform::WindowEvent::PointerMoved { position });
        capture(window.window(), output, &format!("{scheme}-hover-chrome"))?;
        window
            .window()
            .dispatch_event(slint::platform::WindowEvent::PointerPressed {
                position,
                button: slint::platform::PointerEventButton::Left,
            });
        capture(window.window(), output, &format!("{scheme}-pressed-chrome"))?;
        window
            .window()
            .dispatch_event(slint::platform::WindowEvent::PointerReleased {
                position,
                button: slint::platform::PointerEventButton::Left,
            });
        window
            .window()
            .dispatch_event(slint::platform::WindowEvent::PointerExited);
        window.invoke_reset_chrome();
        window.set_find_visible(false);
        let rows = [
            PageRow {
                key: "studio".into(),
                title: "Notes from the studio".into(),
                path: "Projects/Studio.md".into(),
                excerpt: "A smaller first version. A little space for unfinished ideas.".into(),
                availability: "Local".into(),
                modified_label: "Today, 10:42".into(),
                ..Default::default()
            },
            PageRow {
                key: "weekend".into(),
                title: "The long way home".into(),
                path: "Journal/Weekend.md".into(),
                excerpt: "Take the train. Bring the sketchbook.".into(),
                availability: "Synced".into(),
                modified_label: "Yesterday".into(),
                ..Default::default()
            },
        ];
        window.set_page_results(slint::ModelRc::from(rows.as_slice()));
        window.set_index_status("2 pages available offline".into());
        window.set_active_view("pages".into());
        capture(window.window(), output, &format!("{scheme}-pages"))?;
        window.set_page_results(Default::default());
        window.set_index_status("No matching pages. Your local notes remain available.".into());
        capture(window.window(), output, &format!("{scheme}-pages-empty"))?;
        window.set_active_view("connection".into());
        window.set_connection_url("https://notes.example.com".into());
        window.set_connection_message(
            "The server could not be reached. Check the address and try again.".into(),
        );
        capture(
            window.window(),
            output,
            &format!("{scheme}-connection-error"),
        )?;
        window.set_connection_busy(true);
        window.set_connection_message("Checking the connection…".into());
        capture(
            window.window(),
            output,
            &format!("{scheme}-connection-busy"),
        )?;
        window.set_connection_busy(false);
        window.set_active_view("recovery".into());
        window.set_recovery_items(slint::ModelRc::from(rows.as_slice()));
        window.set_recovery_selected(true);
        window.set_recovery_selected_id("studio".into());
        window.set_recovery_path("Projects/Studio.md".into());
        window.set_recovery_local("# Studio\nKeep a little space for the next thought.".into());
        window.set_recovery_remote("# Studio\nShare the next draft on Friday.".into());
        window.set_recovery_merge(
            "# Studio\nKeep a little space. Share the next draft on Friday.".into(),
        );
        window.set_recovery_writable(true);
        window.set_recovery_message("Both versions are kept until you choose a result.".into());
        capture(window.window(), output, &format!("{scheme}-recovery"))?;
        window.set_recovery_busy(true);
        window.set_recovery_message("Saving your merged result…".into());
        capture(window.window(), output, &format!("{scheme}-recovery-busy"))?;
        window.set_recovery_busy(false);
        window.set_recovery_stale(true);
        window.set_recovery_message(
            "The remote version changed again. Reload before applying; your merged draft is kept."
                .into(),
        );
        capture(
            window.window(),
            output,
            &format!("{scheme}-recovery-warning"),
        )?;
        window.set_recovery_stale(false);
        window.set_active_view("appearance".into());
        window
            .window()
            .dispatch_event(slint::platform::WindowEvent::PointerScrolled {
                position: slint::LogicalPosition::new(250.0, 300.0),
                delta_x: 0.0,
                delta_y: -20000.0,
            });
        capture(window.window(), output, &format!("{scheme}-settings-end"))?;
        let mut imported_settings = settings.clone();
        let mut imported = settings.themes()?.remove(0).theme;
        imported.name = "Handmade notes from the studio".into();
        imported_settings
            .imported_themes
            .push(bulletnotes::theme::palette::ThemeEntry::import(imported));
        present_settings(&window, &imported_settings, SystemAppearance::Light)?;
        window.set_active_view("note".into());
        window.set_active_view("appearance".into());
        window
            .window()
            .dispatch_event(slint::platform::WindowEvent::PointerScrolled {
                position: slint::LogicalPosition::new(250.0, 300.0),
                delta_x: 0.0,
                delta_y: -20000.0,
            });
        window
            .window()
            .dispatch_event(slint::platform::WindowEvent::PointerScrolled {
                position: slint::LogicalPosition::new(250.0, 300.0),
                delta_x: 0.0,
                delta_y: 160.0,
            });
        capture(
            window.window(),
            output,
            &format!("{scheme}-settings-imported-card"),
        )?;
        let mut low_settings = imported_settings.clone();
        let mut low = low_settings.imported_themes[0].theme.clone();
        low.name = "Low contrast proof".into();
        low.ink = low.paper;
        low_settings
            .imported_themes
            .push(bulletnotes::theme::palette::ThemeEntry::import(low));
        window.set_system_high_contrast(true);
        present_settings(&window, &low_settings, SystemAppearance::Light)?;
        window
            .window()
            .dispatch_event(slint::platform::WindowEvent::PointerScrolled {
                position: slint::LogicalPosition::new(250.0, 300.0),
                delta_x: 0.0,
                delta_y: -20000.0,
            });
        window
            .window()
            .dispatch_event(slint::platform::WindowEvent::PointerScrolled {
                position: slint::LogicalPosition::new(250.0, 300.0),
                delta_x: 0.0,
                delta_y: 160.0,
            });
        capture(
            window.window(),
            output,
            &format!("{scheme}-settings-imported-high-contrast"),
        )?;
        window.set_system_high_contrast(false);
        present_settings(&window, &imported_settings, SystemAppearance::Light)?;
        window.set_appearance_message(
            "Theme file could not be imported. Choose a valid palette JSON file.".into(),
        );
        window
            .window()
            .dispatch_event(slint::platform::WindowEvent::PointerScrolled {
                position: slint::LogicalPosition::new(250.0, 300.0),
                delta_x: 0.0,
                delta_y: -20000.0,
            });
        capture(
            window.window(),
            output,
            &format!("{scheme}-settings-import-error"),
        )?;
        window.set_appearance_message("".into());
        present_settings(&window, &settings, SystemAppearance::Light)?;
        present_about(&about, &window);
        about.show()?;
        capture(about.window(), output, &format!("{scheme}-about"))?;
        window.set_system_high_contrast(true);
        window.set_prefers_reduced_transparency(true);
        present_settings(&window, &settings, SystemAppearance::Light)?;
        let _ = window.window().take_snapshot()?;
        capture(
            about.window(),
            output,
            &format!("{scheme}-about-high-contrast-live"),
        )?;
        window.set_system_high_contrast(false);
        window.set_prefers_reduced_transparency(false);
        present_settings(&window, &settings, SystemAppearance::Light)?;
        about.hide()?;
    }
    window.hide()?;
    Ok(())
}
