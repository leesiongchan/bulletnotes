//! Capture synthetic search states, or interact with a temporary offline demo.
use bulletnotes::{
    AppWindow, PageRow,
    app::ui::present_settings,
    store::settings::AppSettings,
    theme::{AppearanceMode, SystemAppearance},
};
use slint::{ComponentHandle, ModelRc};
use std::{fs, io::Write, path::PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::args_os().nth(1).as_deref() == Some(std::ffi::OsStr::new("--interactive")) {
        return interactive();
    }
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
    let mut rows = vec![
        PageRow { key: "sample-1".into(), title: "Today".into(), path: "Quick Notes/today.md".into(), excerpt: "# Today\n- [ ] Review the draft\n- [x] Book train tickets\nA smaller first version is easier to finish.".into(), modified_label: "Edited 31 Oct".into(), availability: "Cached".into(), ..Default::default() },
        PageRow { key: "sample-2".into(), title: "Project ideas".into(), path: "Projects/ideas.md".into(), excerpt: "# Project ideas\nBuild a quiet place to write.\n日本語 café — keep the details close.".into(), modified_label: "Edited 30 Oct".into(), availability: "Cached".into(), ..Default::default() },
        PageRow { key: "sample-3".into(), title: "Reading list".into(), path: "Reading/list.md".into(), availability: "Online only".into(), ..Default::default() },
    ];
    for row in &mut rows {
        let (line, _) = bulletnotes::store::search::highlights::condense(&row.excerpt, &[]);
        row.list_excerpt = line.into();
        row.can_trash = row.availability != "Online only";
        row.trash_disabled_reason = if row.can_trash {
            ""
        } else {
            "Open and download this note first"
        }
        .into();
    }
    window.set_page_results(ModelRc::from(rows.as_slice()));
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
            for (cards, view) in [(false, "list"), (true, "cards")] {
                window.set_search_cards(cards);
                for scene in [
                    "browse",
                    "filtered",
                    "selected",
                    "scratchpads-selected",
                    "empty",
                    "confirmation",
                    "invalid-folder",
                    "error",
                ] {
                    let selected = scene.ends_with("selected");
                    window.set_search_scratchpads(scene == "scratchpads-selected");
                    window.set_search_selecting(selected);
                    window.set_search_selected_count(if selected { 2 } else { 0 });
                    window.set_search_can_trash(selected);
                    window.set_search_can_promote(scene == "scratchpads-selected");
                    window.set_search_can_create(scene == "empty");
                    window.set_search_confirm_count(if scene == "confirmation" { 2 } else { 0 });
                    window.set_search_confirm_id(
                        if scene == "confirmation" { "demo" } else { "" }.into(),
                    );
                    window.set_search_folder(
                        if scene == "invalid-folder" {
                            "/invalid"
                        } else {
                            ""
                        }
                        .into(),
                    );
                    window.set_search_query(
                        if scene == "empty" {
                            "a new thought"
                        } else if scene == "filtered" {
                            "draft"
                        } else {
                            ""
                        }
                        .into(),
                    );
                    window.set_index_status(
                        match scene {
                            "invalid-folder" => "Enter a relative folder path",
                            "error" => "Search could not finish. Try again.",
                            "filtered" => "1 page · 2 of 3 indexed · content search incomplete",
                            "empty" => "0 pages · 2 of 3 indexed · content search incomplete",
                            _ => "3 pages · 2 of 3 indexed · content search incomplete",
                        }
                        .into(),
                    );
                    let mut scene_rows = rows.clone();
                    if selected {
                        scene_rows[0].checked = true;
                        scene_rows[1].checked = true;
                    }
                    if scene == "scratchpads-selected" {
                        scene_rows.truncate(2);
                        for (index, row) in scene_rows.iter_mut().enumerate() {
                            row.path = format!("Quick Notes/{index}.md").into();
                            row.can_promote = true;
                        }
                    }
                    if scene == "filtered" {
                        scene_rows.truncate(1);
                        let row = &mut scene_rows[0];
                        let at = row.excerpt.find("draft").unwrap();
                        let range = at..at + 5;
                        let (excerpt, ranges) = bulletnotes::store::search::highlights::excerpt(
                            &row.excerpt,
                            std::slice::from_ref(&range),
                            320,
                        );
                        let (line, line_ranges) =
                            bulletnotes::store::search::highlights::condense(&excerpt, &ranges);
                        let decorations = |ranges: Vec<std::ops::Range<usize>>| {
                            ModelRc::from(
                                ranges
                                    .into_iter()
                                    .map(|range| {
                                        let mut hit =
                                            slint::language::TextInputDecoration::default();
                                        hit.start = range.start as i32;
                                        hit.end = range.end as i32;
                                        hit.bold = true;
                                        hit.code = true;
                                        hit.foreground = window.get_theme().ink;
                                        hit
                                    })
                                    .collect::<Vec<_>>()
                                    .as_slice(),
                            )
                        };
                        row.excerpt = excerpt.into();
                        row.list_excerpt = line.into();
                        row.excerpt_highlights = decorations(ranges);
                        row.list_excerpt_highlights = decorations(line_ranges);
                    }
                    window.set_page_results(
                        if ["empty", "invalid-folder", "error"].contains(&scene) {
                            ModelRc::default()
                        } else {
                            ModelRc::from(scene_rows.as_slice())
                        },
                    );
                    snapshot(
                        &window,
                        output.join(format!("search-{view}-{scene}-{label}-{width}.pam")),
                    )?;
                }
            }
        }
    }
    window.hide()?;
    Ok(())
}

fn snapshot(window: &AppWindow, path: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let image = window.window().take_snapshot()?;
    let mut file = fs::File::create(path)?;
    write!(
        file,
        "P7\nWIDTH {}\nHEIGHT {}\nDEPTH 4\nMAXVAL 255\nTUPLTYPE RGB_ALPHA\nENDHDR\n",
        image.width(),
        image.height()
    )?;
    file.write_all(image.as_bytes())?;
    Ok(())
}

fn interactive() -> Result<(), Box<dyn std::error::Error>> {
    use bulletnotes::{
        app::{appearance_ui, local_ui},
        domain::{
            connection::ConnectionConfig,
            page::{LocalRevision, PageKey, PagePath},
        },
        store::StoreHandle,
    };
    let runtime = tokio::runtime::Runtime::new()?;
    let _entered = runtime.enter();
    let directory = tempfile::tempdir()?;
    let connection = ConnectionConfig::parse("https://search-demo.example")?
        .id()
        .clone();
    let store = runtime.block_on(StoreHandle::open(&directory.path().join("demo.sqlite")))?;
    for (n, body) in [
        "# Today\nReview the draft\nBook train tickets",
        "# Project ideas\nA quiet place to write. 日本語 café",
        "# Reading\nKeep a smaller first version.",
    ]
    .into_iter()
    .enumerate()
    {
        let key = PageKey {
            connection: connection.clone(),
            path: PagePath::parse(&format!("Quick Notes/{n}.md"))?,
        };
        runtime.block_on(store.create_draft(key.clone()))?;
        runtime.block_on(store.save_local(key, body.into(), LocalRevision(1)))?;
    }
    println!("Temporary offline demo: {}", directory.path().display());
    let window = AppWindow::new()?;
    let appearance = appearance_ui::bind(
        &window,
        store.clone(),
        AppSettings::default(),
        SystemAppearance::Light,
    )?;
    let _local = local_ui::bind(&window, store.clone(), connection, appearance)?;
    window.show()?;
    window.invoke_browse_pages();
    window.run()?;
    runtime.block_on(store.shutdown())?;
    Ok(())
}
