//! Render product-page images with the real editor, without a store or network.
use bulletnotes::{
    AppWindow,
    app::ui::present_settings,
    store::settings::AppSettings,
    theme::{AppearanceMode, PaperStyle, SystemAppearance, ThemeId},
};
use slint::ComponentHandle;
use std::{fs, io::Write, path::PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = PathBuf::from(
        std::env::args_os()
            .nth(1)
            .ok_or("provide an output directory")?,
    );
    fs::create_dir_all(&output)?;
    slint::platform::set_platform(Box::new(i_slint_backend_testing::TestingBackend::new(
        i_slint_backend_testing::TestingBackendOptions {
            mock_time: true,
            threading: false,
            renderer_name: Some("skia".into()),
        },
    )))?;
    let window = AppWindow::new()?;
    window.set_prefers_reduced_motion(true);
    window.set_note_read_only(true);
    window.set_note_text(
        "# Scratchpad\n\n## Today\n- [x] Reply to Alex\n- [ ] Review the draft\n- [ ] Book the train tickets\n\n## Quick notes\nStart with a smaller first version.\n\n- Check keyboard shortcuts\n- Try a darker theme\n\n**Next:** share the draft on Friday."
            .into(),
    );
    window.show()?;
    window
        .window()
        .set_size(slint::LogicalSize::new(505.0, 552.0));
    window.invoke_reset_chrome();

    for (slug, id) in [
        ("bulletnotes", "bulletnotes"),
        ("bulletnotes-dark", "bulletnotes-dark"),
        ("paper", "paper"),
        ("graphite", "graphite"),
        ("matcha", "matcha"),
        ("rose", "rose"),
        ("tartan", "antinote-tartan"),
        ("sanrio", "antinote-sanrio"),
        ("dracula", "antinote-community-dracula"),
        ("nord", "antinote-community-nord"),
        ("catppuccin-mocha", "antinote-community-catppuccin-mocha"),
        ("dot-matrix", "antinote-community-dotmatrix"),
        ("tokyo-drift", "antinote-tokyo-drift"),
        ("mononoke", "antinote-mononoke"),
    ] {
        for (pattern, paper) in [
            ("plain", PaperStyle::Plain),
            ("dotted", PaperStyle::Dots),
            ("ruled", PaperStyle::Ruled),
            ("grid", PaperStyle::Grid),
        ] {
            let mut settings = AppSettings::default();
            settings.appearance.mode = AppearanceMode::Light;
            settings.appearance.light_theme = ThemeId(id.into());
            settings.appearance.paper = paper;
            present_settings(&window, &settings, SystemAppearance::Light)?;
            let image = window.window().take_snapshot()?;
            let path = output.join(format!("{slug}-{pattern}.pam"));
            let mut file = fs::File::create(&path)?;
            write!(
                file,
                "P7\nWIDTH {}\nHEIGHT {}\nDEPTH 4\nMAXVAL 255\nTUPLTYPE RGB_ALPHA\nENDHDR\n",
                image.width(),
                image.height()
            )?;
            file.write_all(image.as_bytes())?;
            println!("Rendered {}", path.display());
        }
    }
    window.hide()?;
    Ok(())
}
