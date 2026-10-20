use slint::ComponentHandle;
use slint_source_highlighting_probe::{
    EditorHarness, MARKER_SOURCE, SOURCE, markdown_decorations, markdown_markers,
};
use std::{cell::RefCell, path::Path, rc::Rc, time::Duration};

fn save(ui: &EditorHarness, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let pixels = ui.window().take_snapshot()?;
    let mut encoder = png::Encoder::new(
        std::fs::File::create(path)?,
        pixels.width(),
        pixels.height(),
    );
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()?
        .write_image_data(pixels.as_bytes())?;
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let settings = slint::wgpu_29::WGPUSettings::default();
    #[cfg(target_os = "macos")]
    let settings = {
        let mut settings = settings;
        settings.backends = slint::wgpu_29::wgpu::Backends::METAL;
        settings
    };
    slint::BackendSelector::new()
        .backend_name("winit".into())
        .renderer_name("skia".into())
        .require_wgpu_29(slint::wgpu_29::WGPUConfiguration::Automatic(settings))
        .select()?;
    let ui = EditorHarness::new()?;
    let marker_mode = std::env::args().any(|arg| arg == "--markers");
    let source = if marker_mode { MARKER_SOURCE } else { SOURCE };
    ui.set_source(source.into());
    ui.invoke_select(source.len() as i32, source.len() as i32);
    let verify = std::env::args().any(|arg| arg == "--verify");
    if !verify || marker_mode {
        ui.set_decorations(slint::ModelRc::from(
            markdown_decorations(source).as_slice(),
        ));
    }
    if marker_mode && !verify {
        ui.set_markers(slint::ModelRc::from(markdown_markers(source).as_slice()));
    }
    let weak = ui.as_weak();
    ui.on_source_edited(move || {
        if let Some(ui) = weak.upgrade() {
            ui.set_decorations(slint::ModelRc::from(
                markdown_decorations(&ui.get_source()).as_slice(),
            ));
            if marker_mode {
                ui.set_markers(slint::ModelRc::from(
                    markdown_markers(&ui.get_source()).as_slice(),
                ));
            }
        }
    });
    let outcome = Rc::new(RefCell::new(None));
    if verify {
        let completed = outcome.clone();
        let weak = ui.as_weak();
        slint::Timer::single_shot(Duration::from_millis(400), move || {
            let result = (|| -> Result<(), Box<dyn std::error::Error>> {
                let ui = weak.upgrade().ok_or("probe window closed")?;
                let directory = Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../../target/source-highlighting-research");
                std::fs::create_dir_all(&directory)?;
                let prefix = if marker_mode { "markers-" } else { "" };
                save(&ui, &directory.join(format!("{prefix}before.png")))?;
                let before = ui.window().take_snapshot()?.as_bytes().to_vec();
                if marker_mode {
                    ui.set_markers(slint::ModelRc::from(markdown_markers(source).as_slice()));
                } else {
                    ui.set_decorations(slint::ModelRc::from(
                        markdown_decorations(source).as_slice(),
                    ));
                }
                save(&ui, &directory.join(format!("{prefix}after.png")))?;
                if ui.window().take_snapshot()?.as_bytes() == before {
                    return Err("enabling decorations did not change rendered pixels".into());
                }
                if marker_mode {
                    let at = source.find("- [ ]").ok_or("missing generated task")? as i32;
                    ui.invoke_select(at + 2, at + 2);
                    save(&ui, &directory.join("markers-revealed.png"))?;
                }
                if ui.get_source().as_bytes() != source.as_bytes() || ui.get_edits() != 0 {
                    return Err("formatting changed source or emitted an edit".into());
                }
                println!(
                    "source_highlighting_probe: source_exact=true edits=0 markers={marker_mode} screenshots={}",
                    if marker_mode { 3 } else { 2 }
                );
                Ok(())
            })();
            *completed.borrow_mut() = Some(result.map_err(|error| error.to_string()));
            let _ = slint::quit_event_loop();
        });
        slint::Timer::single_shot(Duration::from_secs(15), || {
            let _ = slint::quit_event_loop();
        });
    }
    eprintln!(
        "source decoration feasibility probe: pinned Slint 1.18.1, source-width-preserving markers={marker_mode}"
    );
    ui.run()?;
    if verify {
        outcome
            .borrow_mut()
            .take()
            .ok_or("probe timed out")?
            .map_err(std::io::Error::other)?;
    }
    Ok(())
}
