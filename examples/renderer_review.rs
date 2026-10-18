//! Generated-data renderer acceptance: real Metal layer, startup, themes, save/reopen.
use bulletnotes::{
    app::{coordinator::AppAction, native_app::LocalApplication, startup},
    domain::{connection::ConnectionConfig, page::*},
    platform::application::NativeApplication,
    store::StoreHandle,
};
use slint::ComponentHandle;
use std::{
    cell::RefCell,
    rc::Rc,
    time::{Duration, Instant},
};
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
const SOURCE: &str =
    "---\r\ntitle: Renderer fixture\r\n---\r\n# Generated note\r\n\t日本語 café 👩🏽‍💻\r\n";
async fn pause() {
    let (tx, rx) = tokio::sync::oneshot::channel();
    slint::Timer::single_shot(Duration::from_millis(20), move || {
        let _ = tx.send(());
    });
    let _ = rx.await;
}
fn select(renderer: &str) -> Result<()> {
    match renderer {
        "production" => bulletnotes::platform::renderer::select()?,
        "skia" => bulletnotes::platform::renderer::select()?,
        "skia-opengl" => bulletnotes::platform::renderer::select_opengl_for_review()?,
        _ => return Err("unsupported review renderer".into()),
    }
    Ok(())
}
fn main() -> Result<()> {
    let renderer = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "production".into());
    let allow_opengl = std::env::args().any(|arg| arg == "--allow-opengl");
    let native = NativeApplication::prepare()?;
    select(&renderer)?;
    let runtime = tokio::runtime::Runtime::new()?;
    let directory = tempfile::Builder::new()
        .prefix("bulletnotes-renderer-")
        .tempdir()?;
    let path = directory.path().join("generated.sqlite");
    let config = ConnectionConfig::parse("https://renderer.example/")?;
    let key = PageKey {
        connection: config.id().clone(),
        path: PagePath::parse("generated.md")?,
    };
    let store = runtime.block_on(StoreHandle::open(&path))?;
    runtime.block_on(store.create_draft(key.clone()))?;
    runtime.block_on(store.save_local(key.clone(), SOURCE.into(), LocalRevision(1)))?;
    runtime.block_on(store.record_visit(&key, 1))?;
    let mut settings = runtime.block_on(store.load_settings())?;
    settings.active_connection = Some(config.id().clone());
    settings.hotkey.control = true;
    settings.hotkey.shift = true;
    settings.hotkey.alt = true;
    settings.hotkey.key = "KeyP".into();
    runtime.block_on(store.save_settings(settings))?;
    runtime.block_on(store.shutdown())?;
    let startup = runtime.block_on(startup::open(&path, config.id().clone()))?;
    let app = Rc::new(LocalApplication::new(startup, native)?);
    let outcome = Rc::new(RefCell::new(None));
    let completed = outcome.clone();
    let running = app.clone();
    let selected = renderer.clone();
    let key_for_restart = key.clone();
    slint::spawn_local(async move {
        let result: Result<()>=async {
            let deadline=Instant::now()+Duration::from_secs(12);
            while running.local().current_key()!=Some(key.clone()) || running.rendered_frame_count()==0 {
                if Instant::now()>deadline {return Err("renderer did not deliver first-frame startup".into());}pause().await;
            }
            running.local().wait_until_loaded(&key).await?;
            if running.window().get_note_text()!=SOURCE {return Err("renderer changed source bytes".into());}
            let has_metal=bulletnotes::platform::renderer::uses_metal(running.window().window())?;
            if !has_metal && !allow_opengl {return Err("actual native content has no CAMetalLayer; Metal acceptance failed".into());}
            let output=std::path::Path::new("target/renderer-review").join(&selected);std::fs::create_dir_all(&output)?;
            for theme in ["paper","graphite","matcha","rose"] {
                running.window().invoke_theme_selected(theme.into());running.appearance().flush().await?;pause().await;
                if running.window().get_selected_theme_id()!=theme {return Err(format!("theme selection was not applied: {theme}").into());}
                if running.window().get_note_text()!=SOURCE {return Err("theme switch changed source bytes".into());}
                let pixels=running.window().window().take_snapshot()?;
                let file=std::fs::File::create(output.join(format!("{theme}.png")))?;
                let mut encoder=png::Encoder::new(file,pixels.width(),pixels.height());encoder.set_color(png::ColorType::Rgba);encoder.set_depth(png::BitDepth::Eight);encoder.write_header()?.write_image_data(pixels.as_bytes())?;
            }
            running.local().dispatch(AppAction::EditText(format!("{SOURCE}Exact saved edit\r\n")));running.local().flush().await?;
            println!("renderer_acceptance: {{\"renderer\":\"{selected}\",\"metal_layer\":{has_metal},\"first_frame\":true,\"source_exact\":true,\"themes\":4}}");
            Ok(())
        }.await;
        *completed.borrow_mut() = Some(result.map_err(|error| error.to_string()));
        running.request_native_termination();
    })?;
    slint::Timer::single_shot(Duration::from_secs(20), || {
        let _ = slint::quit_event_loop();
    });
    app.run()?;
    outcome
        .borrow_mut()
        .take()
        .ok_or("renderer review did not finish")?
        .map_err(|error| format!("renderer review: {error}"))?;
    drop(app);
    let store = runtime.block_on(StoreHandle::open(&path))?;
    let saved = runtime
        .block_on(store.load_page(&key_for_restart))?
        .ok_or("saved fixture disappeared")?;
    if saved.body != format!("{SOURCE}Exact saved edit\r\n") {
        return Err("saved source changed after renderer shutdown".into());
    }
    runtime.block_on(store.shutdown())?;
    Ok(())
}
