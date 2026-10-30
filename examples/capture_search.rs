//! Capture the real search component with synthetic notes and no network or store.
use bulletnotes::{
    AppWindow, PageRow,
    app::ui::present_settings,
    store::settings::AppSettings,
    theme::{AppearanceMode, SystemAppearance},
};
use slint::{ComponentHandle, ModelRc};
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
    window.set_active_view("pages".into());
    window.set_page_results(ModelRc::from(vec![
        PageRow { key: "sample-1".into(), title: "Today".into(), path: "Quick Notes/today.md".into(), excerpt: "# Today\n- [ ] Review the draft\n- [x] Book train tickets\nA smaller first version is easier to finish.".into(), modified_label: "Edited 31 Oct".into(), availability: "Cached".into(), ..Default::default() },
        PageRow { key: "sample-2".into(), title: "Project ideas".into(), path: "Projects/ideas.md".into(), excerpt: "# Project ideas\nBuild a quiet place to write.\n日本語 café — keep the details close.".into(), modified_label: "Edited 30 Oct".into(), availability: "Cached".into(), ..Default::default() },
        PageRow { key: "sample-3".into(), title: "Reading list".into(), path: "Reading/list.md".into(), availability: "Online only".into(), ..Default::default() },
    ].as_slice()));
    window.set_index_status("3 pages · 2 of 3 indexed · content search incomplete".into());
    window.show()?;
    for (mode, label) in [
        (AppearanceMode::Light, "light"),
        (AppearanceMode::Dark, "dark"),
    ] {
        let mut settings = AppSettings::default();
        settings.appearance.mode = mode;
        present_settings(&window, &settings, SystemAppearance::Light)?;
        for (width, height) in [(400, 440), (505, 552), (900, 700)] {
            window
                .window()
                .set_size(slint::LogicalSize::new(width as f32, height as f32));
            let image = window.window().take_snapshot()?;
            let mut file = fs::File::create(output.join(format!("search-{label}-{width}.pam")))?;
            write!(
                file,
                "P7\nWIDTH {}\nHEIGHT {}\nDEPTH 4\nMAXVAL 255\nTUPLTYPE RGB_ALPHA\nENDHDR\n",
                image.width(),
                image.height()
            )?;
            file.write_all(image.as_bytes())?;
        }
    }
    window.hide()?;
    Ok(())
}
