//! Actual native Slint measurements with a generated 10k-page SQLite space.
//! Frame completion is AfterRendering (submission), not a physical display probe.
#[path = "support/merge_distribution.rs"]
mod merge_distribution;
#[cfg(target_os = "macos")]
mod native {
    use bulletnotes::{
        app::{coordinator::AppAction, native_app::LocalApplication, startup},
        domain::{connection::ConnectionConfig, page::*},
        platform::application::NativeApplication,
        store::StoreHandle,
    };
    use slint::{ComponentHandle, Model};
    use std::{
        cell::RefCell,
        process::Command,
        rc::Rc,
        time::{Duration, Instant},
    };
    type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
    async fn pause() {
        let (tx, rx) = tokio::sync::oneshot::channel();
        slint::Timer::single_shot(Duration::from_millis(1), move || {
            let _ = tx.send(());
        });
        let _ = rx.await;
    }
    async fn frame(app: &LocalApplication) -> Result<()> {
        let before = app.rendered_frame_count();
        let deadline = Instant::now() + Duration::from_secs(30);
        app.window().window().request_redraw();
        while app.rendered_frame_count() <= before {
            if Instant::now() > deadline {
                return Err("native frame timed out".into());
            }
            pause().await;
        }
        Ok(())
    }
    fn metadata(program: &str, args: &[&str]) -> String {
        Command::new(program)
            .args(args)
            .output()
            .ok()
            .filter(|output| output.status.success())
            .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
            .unwrap_or_else(|| "unavailable".into())
    }
    fn p95(values: &mut [f64]) -> f64 {
        values.sort_by(f64::total_cmp);
        values[((values.len() as f64 * 0.95).ceil() as usize).saturating_sub(1)]
    }
    fn key(config: &ConnectionConfig, index: usize) -> PageKey {
        PageKey {
            connection: config.id().clone(),
            path: PagePath::parse(&format!("bench/{index:05}.md")).unwrap(),
        }
    }
    pub fn run() -> Result<()> {
        let native = NativeApplication::prepare()?;
        let renderer =
            std::env::var("BULLETNOTES_BENCH_RENDERER").unwrap_or_else(|_| "skia".into());
        match renderer.as_str() {
            #[cfg(feature = "renderer-review")]
            "skia-opengl" => bulletnotes::platform::renderer::select_opengl_for_review()?,
            "skia" => bulletnotes::platform::renderer::select()?,
            _ => return Err("unsupported benchmark renderer".into()),
        }
        let runtime = tokio::runtime::Runtime::new()?;
        let dir = tempfile::Builder::new()
            .prefix("bulletnotes-interaction-")
            .tempdir()?;
        let path = dir.path().join("generated.sqlite");
        let config = ConnectionConfig::parse("https://benchmark.example")?;
        let store = runtime.block_on(StoreHandle::open(&path))?;
        let mut settings = runtime.block_on(store.load_settings())?;
        settings.active_connection = Some(config.id().clone());
        settings.hotkey.key = "KeyP".into();
        settings.hotkey.alt = true;
        settings.hotkey.control = true;
        settings.hotkey.shift = true;
        runtime.block_on(store.save_settings(settings))?;
        runtime.block_on(store.shutdown())?;
        {
            let mut db = rusqlite::Connection::open(&path)?;
            let tx = db.transaction()?;
            let mut statement=tx.prepare(r#"INSERT INTO pages(connection,path,title,body,remote_body,remote_revision,catalogued,permission,last_visited_ms) VALUES(?1,?2,?3,?4,?4,'"bench"',1,1,?5)"#)?;
            for index in 0..10000 {
                let body = if index == 9999 {
                    let mut body="needle generated large source line with ordinary words and numbers 0123456789\n".repeat(15000);
                    body.truncate(1024 * 1024);
                    body
                } else {
                    format!(
                        "# Generated {index:05}\nneedle unique{index:05}\nA short cached page.\n"
                    )
                };
                statement.execute(rusqlite::params![
                    config.id().as_str(),
                    key(&config, index).path.as_str(),
                    format!("Generated {index:05}"),
                    body,
                    if index == 0 { Some(1) } else { None }
                ])?;
            }
            drop(statement);
            tx.execute(
                "INSERT INTO catalogue_state(connection) VALUES(?1)",
                [config.id().as_str()],
            )?;
            tx.commit()?;
        }
        let diagnostics = bulletnotes::diagnostics::init(&dir.path().join("diagnostics"))?;
        let state = runtime.block_on(startup::open(&path, config.id().clone()))?;
        let app = Rc::new(LocalApplication::new(state, native)?);
        let report = Rc::new(RefCell::new(None));
        let completed = report.clone();
        let running = app.clone();
        slint::Timer::single_shot(Duration::from_millis(300), move || {
            let _ = slint::spawn_local(async move {
                let result=async {
                running.local().wait_until_loaded(&key(&config,0)).await?;frame(&running).await?;
                let mut activation=Vec::new();let mut open=Vec::new();let mut large=Vec::new();let mut search=Vec::new();let mut large_load=Vec::new();let mut large_render=Vec::new();
                for _ in 0..20 {
                    running.window().hide()?;pause().await;let start=Instant::now();running.show()?;frame(&running).await?;activation.push(start.elapsed().as_secs_f64()*1000.0);
                }
                for index in 1..=20 {
                    let start=Instant::now();let page=key(&config,index);
                    running.local().dispatch(AppAction::OpenPage(page.clone()));running.local().wait_until_loaded(&page).await?;frame(&running).await?;open.push(start.elapsed().as_secs_f64()*1000.0);
                }
                for _ in 0..20 {
                    let page=key(&config,0);running.local().dispatch(AppAction::OpenPage(page.clone()));running.local().wait_until_loaded(&page).await?;frame(&running).await?;
                    let start=Instant::now();let page=key(&config,9999);
                    running.local().dispatch(AppAction::OpenPage(page.clone()));running.local().wait_until_loaded(&page).await?;
                    let loaded=start.elapsed().as_secs_f64()*1000.0;frame(&running).await?;
                    large_load.push(loaded);large_render.push(start.elapsed().as_secs_f64()*1000.0-loaded);
                    if running.window().get_note_text().len()!=1024*1024{return Err("large fixture has incorrect source length".into());}
                    large.push(start.elapsed().as_secs_f64()*1000.0);
                }
                running.window().invoke_browse_pages();running.local().search_current().await?;frame(&running).await?;
                let visible=running.window().get_page_results().row_count();
                while running.window().get_can_load_more_pages() {
                    let count=running.window().get_page_results().row_count();
                    running.window().invoke_load_more_pages();
                    let deadline=Instant::now()+Duration::from_secs(10);
                    while running.window().get_page_results().row_count()<=count {
                        if Instant::now()>deadline{return Err("page continuation did not advance".into());}pause().await;
                    }
                    frame(&running).await?;
                }
                let reached=running.window().get_page_results().row_count();
                for _ in 0..20 {
                    running.window().set_search_query("needle".into());let start=Instant::now();running.local().search_current().await?;frame(&running).await?;search.push(start.elapsed().as_secs_f64()*1000.0);
                }
                let a=p95(&mut activation);let o=p95(&mut open);let l=p95(&mut large);let s=p95(&mut search);
                let report=serde_json::json!({"schema":1,"pages":10000,"large_note_bytes":1024*1024,"samples":20,
                    "renderer":format!("winit-{renderer}"),"frame_boundary":"Slint AfterRendering; native GPU submission",
                    "profile":if cfg!(debug_assertions){"debug"}else{"release"},"os":metadata("sw_vers",&["-productVersion"]),
                    "os_build":metadata("sw_vers",&["-buildVersion"]),"hardware":metadata("sysctl",&["-n","hw.model"]),
                    "cpu":metadata("sysctl",&["-n","machdep.cpu.brand_string"]),"memory_bytes":metadata("sysctl",&["-n","hw.memsize"]),
                    "rust":metadata("rustup",&["run","1.99.1","rustc","--version"]),"slint":"1.18.1",
                    "warm_activation_p95_ms":a,"cached_open_p95_ms":o,"large_open_p95_ms":l,"large_load_p95_ms":p95(&mut large_load),"large_render_p95_ms":p95(&mut large_render),"cached_search_p95_ms":s,
                    "initial_visible_pages":visible,"catalogue_pages_reached":reached,"catalogue_reachable":reached==10000,"thresholds_ms":{"warm_activation":150,"cached_open":100,"large_open":100,"cached_search":100},
                    "performance_pass":a<=150.0&&o<=100.0&&l<=100.0&&s<=100.0});
                Ok::<_,Box<dyn std::error::Error>>(report)
            }.await;
                *completed.borrow_mut() = Some(result.map_err(|error| error.to_string()));
                running.request_native_termination();
            });
        });
        let deadline_app = app.clone();
        slint::Timer::single_shot(Duration::from_secs(180), move || {
            deadline_app.request_native_termination()
        });
        app.run(&runtime)?;
        drop(diagnostics);
        let diagnostic_path = dir.path().join("profile.jsonl");
        bulletnotes::diagnostics::export(&diagnostic_path)?;
        let mut stages = std::collections::BTreeMap::<String, Vec<f64>>::new();
        for line in std::fs::read_to_string(diagnostic_path)?.lines() {
            let record: serde_json::Value = serde_json::from_str(line)?;
            if let (Some(operation), Some(micros)) = (
                record["event"]["operation"].as_str(),
                record["event"]["elapsed_micros"].as_f64(),
            ) {
                stages
                    .entry(operation.to_owned())
                    .or_default()
                    .push(micros / 1000.0);
            }
        }
        let stages = stages
            .into_iter()
            .map(|(operation, mut values)| (operation, p95(&mut values)))
            .collect::<std::collections::BTreeMap<_, _>>();
        let mut report = report
            .borrow_mut()
            .take()
            .ok_or("native benchmark did not finish")?
            .map_err(|error| format!("native benchmark: {error}"))?;
        report["stages_p95_ms"] = serde_json::to_value(stages)?;
        report["app_version"] = env!("CARGO_PKG_VERSION").into();
        let output = serde_json::to_string_pretty(&report)?;
        println!("{output}");
        std::fs::create_dir_all("target/interaction")?;
        std::fs::write(
            format!(
                "target/interaction/{}.json",
                report["renderer"].as_str().ok_or("missing renderer")?
            ),
            &output,
        )?;
        std::fs::write("target/interaction/latest.json", output)?;
        if report["performance_pass"] != true || report["catalogue_reachable"] != true {
            return Err(
                "native responsiveness thresholds not met; see target/interaction/latest.json"
                    .into(),
            );
        }
        Ok(())
    }
}
#[cfg(target_os = "macos")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::args().any(|arg| arg == "--merge-only") {
        merge_distribution::run()
    } else {
        native::run()
    }
}
#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("native interaction benchmark requires macOS");
    std::process::exit(1);
}
