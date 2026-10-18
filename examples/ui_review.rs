//! Native UI contract fixture with generated text only; no SilverBullet connection.
#[cfg(all(target_os = "macos", feature = "native-validation"))]
use bulletnotes::platform::window_chrome as chrome;
use bulletnotes::{
    AppWindow,
    app::ui::{apply_theme, present_settings},
    store::settings::AppSettings,
    theme::{AppearanceMode, SystemAppearance, ThemeId, resolve_theme},
};
use slint::ComponentHandle;
use std::{cell::Cell, rc::Rc, time::Duration};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::args().any(|arg| arg == "--shortcut-contract") {
        return shortcut_contract();
    }
    let window = AppWindow::new()?;
    let mut settings = AppSettings {
        appearance: bulletnotes::theme::AppearanceSettings {
            mode: AppearanceMode::Light,
            ..Default::default()
        },
        ..Default::default()
    };
    let args: Vec<_> = std::env::args().collect();
    window.set_system_high_contrast(args.iter().any(|arg| arg == "--high-contrast"));
    window.set_can_load_more_pages(args.iter().any(|arg| arg == "--more-pages"));
    let argument = |name: &str| {
        args.windows(2)
            .find(|pair| pair[0] == name)
            .map(|pair| pair[1].clone())
    };
    if let Some(id) = argument("--theme") {
        settings.appearance.light_theme = ThemeId(id);
    }
    if let Some(value) = argument("--font") {
        settings.appearance.font_size = value.parse()?;
    }
    if args.iter().any(|arg| arg == "--ruled") {
        settings.appearance.paper = bulletnotes::theme::PaperStyle::Ruled;
    }
    present_settings(&window, &settings, SystemAppearance::Light)?;
    if args.iter().any(|arg| arg == "--minimum") {
        window
            .window()
            .set_size(slint::LogicalSize::new(400.0, 440.0));
    }
    if args.iter().any(|arg| arg == "--find") {
        window.set_find_visible(true);
        window.set_find_query("café".into());
        window.set_find_status("1 of 1".into());
    }
    if let Some(view) = argument("--view") {
        window.set_active_view(view.into());
    }
    window.set_page_results(slint::ModelRc::new(slint::VecModel::from(vec![
        bulletnotes::PageRow {
            key: "sample-1".into(),
            title: "A little space to think".into(),
            path: "Journal/Saturday.md".into(),
            excerpt: "Start with one thought. Let the rest follow.".into(),
            availability: "Cached".into(),
        },
        bulletnotes::PageRow {
            key: "sample-2".into(),
            title: "Weekend ideas".into(),
            path: "Personal/Weekend.md".into(),
            excerpt: "A walk, a book, and a few unhurried hours.".into(),
            availability: "Online only".into(),
        },
    ])));
    window.set_index_status("Preview · 2 generated pages".into());
    {
        let weak = window.as_weak();
        window.on_browse_pages(move || {
            if let Some(window) = weak.upgrade() {
                window.set_active_view("pages".into());
            }
        });
    }
    {
        let weak = window.as_weak();
        window.on_show_appearance(move || {
            if let Some(window) = weak.upgrade() {
                window.set_active_view("appearance".into());
            }
        });
    }
    {
        let weak = window.as_weak();
        window.on_return_to_note(move || {
            if let Some(window) = weak.upgrade() {
                window.set_active_view("note".into());
            }
        });
    }
    #[cfg(all(target_os = "macos", feature = "native-validation"))]
    if let Some(path) = argument("--capture") {
        let weak = window.as_weak();
        slint::Timer::single_shot(Duration::from_millis(1000), move || {
            let capture = || -> Result<(), Box<dyn std::error::Error>> {
                let window = weak.upgrade().ok_or("review window was closed")?;
                let native = chrome::native_window(window.window())?;
                // This native handle belongs to this fixture. Never select a desktop or another app.
                let status = std::process::Command::new("/usr/sbin/screencapture")
                    .args(["-x", "-o", &format!("-l{}", native.windowNumber()), &path])
                    .status()?;
                if !status.success() {
                    return Err("native window capture failed".into());
                }
                Ok(())
            };
            if let Err(error) = capture() {
                eprintln!("capture: {error}");
            }
            let _ = slint::quit_event_loop();
        });
    }
    let runtime = tokio::runtime::Runtime::new()?;
    let directory = tempfile::tempdir()?;
    let store = runtime.block_on(bulletnotes::store::StoreHandle::open(
        &directory.path().join("review.sqlite"),
    ))?;
    runtime.block_on(store.save_settings(settings.clone()))?;
    let appearance_ui = bulletnotes::app::appearance_ui::bind(
        &window,
        store.clone(),
        settings.clone(),
        SystemAppearance::Light,
    )?;
    if args.iter().any(|arg| arg == "--appearance-contract") {
        let weak = window.as_weak();
        let binding = appearance_ui.clone();
        let saved = store.clone();
        let path = directory.path().join("review.sqlite");
        slint::spawn_local(async move {
            let check = async {
                let window = weak.upgrade().ok_or("review window closed")?;
                window.set_note_text("original\r\n日本語 👩🏽‍💻".into());
                window.invoke_theme_selected("matcha".into());
                binding.flush().await?;
                if saved.load_settings().await?.appearance.light_theme != ThemeId("matcha".into()) { return Err("theme callback did not persist settings".into()); }
                let injector = rusqlite::Connection::open(path)?;
                injector.execute_batch("CREATE TRIGGER fail_appearance BEFORE UPDATE ON settings BEGIN SELECT RAISE(ABORT,'fixture failure'); END;")?;
                window.invoke_theme_selected("rose".into());
                binding.flush().await?;
                if window.get_selected_theme_id() != "matcha" || window.get_appearance_message().is_empty() { return Err("failed settings save did not restore palette and surface error".into()); }
                if window.get_note_text() != "original\r\n日本語 👩🏽‍💻" { return Err("settings callback changed editor text".into()); }
                Ok::<(),Box<dyn std::error::Error>>(())
            }.await;
            match check {
                Ok(()) => println!("native_appearance_contract_passed: true"),
                Err(error) => eprintln!("appearance contract: {error}"),
            }
            let _ = slint::quit_event_loop();
        })?;
    }
    let active =
        bulletnotes::domain::connection::ConnectionConfig::parse("https://notes.example/")?;
    let _connection = bulletnotes::app::connection_ui::bind(
        &window,
        bulletnotes::app::connection::ConnectionController::new(
            store.clone(),
            bulletnotes::platform::credentials::CredentialStore::keychain(),
        ),
        active.id(),
        runtime.handle().clone(),
    );
    {
        let weak = window.as_weak();
        window.on_show_connection(move || {
            if let Some(window) = weak.upgrade() {
                window.set_active_view("connection".into());
            }
        });
    }
    if args.iter().any(|arg| arg == "--connection-contract") {
        let weak = window.as_weak();
        slint::Timer::single_shot(Duration::from_millis(300), move || {
            let window = weak.upgrade().unwrap();
            window.invoke_show_connection();
            // Slint queues changed handlers. Model separate user events rather
            // than opening and closing the view inside one callback.
            let weak = window.as_weak();
            slint::Timer::single_shot(Duration::from_millis(30), move || {
                let window = weak.upgrade().unwrap();
                let source = window.get_note_text();
                assert_eq!(window.get_primary_view_count(), 1);
                window.set_connection_url("http://notes.example/".into());
                window.set_connection_token("generated-private-token".into());
                window.invoke_test_connection();
                assert!(window.get_connection_token().is_empty());
                assert!(!window.get_connection_busy());
                assert!(window.get_connection_message().contains("HTTPS"));
                window.set_connection_token("another-generated-token".into());
                window.invoke_return_to_note();
                let weak = window.as_weak();
                slint::Timer::single_shot(Duration::from_millis(30), move || {
                    let window = weak.upgrade().unwrap();
                    assert!(window.get_connection_token().is_empty());
                    assert_eq!(window.get_note_text(), source);
                    println!("native_connection_contract_passed: true");
                    let _ = slint::quit_event_loop();
                });
            });
        });
    }
    let original = "---\r\ntitle: café\r\n---\r\n\t👩🏽‍💻 日本語\r\n# A little space to think\r\n\r\nPlain Markdown, exactly as typed.\r\n";
    window.set_note_text(original.into());
    window.set_current_path("Journal/Saturday.md".into());
    window.set_status_text("Preview · temporary text".into());
    let passed = Rc::new(Cell::new(false));
    #[cfg(all(target_os = "macos", feature = "native-validation"))]
    let monitor = Rc::new(std::cell::RefCell::new(None));
    #[cfg(all(target_os = "macos", feature = "native-validation"))]
    {
        let owner = monitor.clone();
        let weak = window.as_weak();
        slint::Timer::single_shot(Duration::from_millis(220), move || {
            if let Some(window) = weak.upgrade() {
                match chrome::ChromeMonitor::install(&window, None) {
                    Ok(guard) => *owner.borrow_mut() = Some(guard),
                    Err(error) => eprintln!("native frame: {error}"),
                }
            }
        });
    }
    if std::env::args().any(|arg| arg == "--contract") {
        let weak = window.as_weak();
        let result = passed.clone();
        slint::Timer::single_shot(Duration::from_millis(420), move || {
            if let Some(window) = weak.upgrade() {
                let check = || -> Result<(), Box<dyn std::error::Error>> {
                    let themes = settings.themes()?;
                    for id in ["paper", "graphite", "matcha", "rose"] {
                        let mut appearance = settings.appearance.clone();
                        appearance.light_theme = ThemeId(id.into());
                        for font in [11.0, 15.0, 24.0] {
                            appearance.font_size = font;
                            let resolved =
                                resolve_theme(&appearance, SystemAppearance::Light, &themes)?;
                            apply_theme(&window, &resolved);
                            let line_height = window.get_editor_line_height();
                            if line_height <= font || line_height > font * 3.0 {
                                return Err("measured editor line spacing is invalid".into());
                            }
                            for view in [
                                "note",
                                "pages",
                                "appearance",
                                "connection",
                                "recovery",
                                "unexpected",
                            ] {
                                window.set_active_view(view.into());
                                if window.get_primary_view_count() != 1 {
                                    return Err("primary view visibility is not exclusive".into());
                                }
                                if window.get_note_text().as_bytes() != original.as_bytes() {
                                    return Err("view/palette change altered source bytes".into());
                                }
                            }
                        }
                    }
                    Ok(())
                };
                match check() {
                    Ok(()) => {
                        result.set(true);
                        println!("native_ui_contract_passed: true");
                    }
                    Err(error) => eprintln!("native UI contract: {error}"),
                }
            }
            let _ = slint::quit_event_loop();
        });
    }
    window.run()?;
    if std::env::args().any(|arg| arg == "--contract") && !passed.get() {
        return Err("native UI contract failed".into());
    }
    Ok(())
}

fn shortcut_contract() -> Result<(), Box<dyn std::error::Error>> {
    use slint::platform::{Key, WindowEvent};
    use std::cell::RefCell;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Action {
        Browse,
        New,
        Appearance,
        Find,
        FindStep(i32),
        Save,
        History(i32),
        Hide,
        Quit,
    }
    fn record(actions: &Rc<RefCell<Vec<Action>>>, action: Action) -> impl Fn() + 'static {
        let actions = actions.clone();
        move || actions.borrow_mut().push(action)
    }
    fn chord(window: &AppWindow, modifier: Key, text: &str, shift: bool) {
        let event = |key: slint::SharedString, pressed| {
            window.window().dispatch_event(if pressed {
                WindowEvent::KeyPressed { text: key }
            } else {
                WindowEvent::KeyReleased { text: key }
            });
        };
        event(modifier.into(), true);
        if shift {
            event(Key::Shift.into(), true);
        }
        event(text.into(), true);
        event(text.into(), false);
        if shift {
            event(Key::Shift.into(), false);
        }
        event(modifier.into(), false);
    }

    bulletnotes::platform::renderer::select()?;
    let window = AppWindow::new()?;
    present_settings(&window, &AppSettings::default(), SystemAppearance::Light)?;
    let source = "Generated shortcut note\r\n日本語 café 👩🏽‍💻\r\n";
    window.set_note_text(source.into());
    let actions = Rc::new(RefCell::new(Vec::new()));
    window.on_browse_pages(record(&actions, Action::Browse));
    window.on_new_note(record(&actions, Action::New));
    window.on_show_appearance(record(&actions, Action::Appearance));
    window.on_find_opened(record(&actions, Action::Find));
    window.on_save_now(record(&actions, Action::Save));
    window.on_request_hide(record(&actions, Action::Hide));
    window.on_request_quit(record(&actions, Action::Quit));
    let observed = actions.clone();
    window.on_find_step(move |step| observed.borrow_mut().push(Action::FindStep(step)));
    let observed = actions.clone();
    window.on_history_step(move |step| observed.borrow_mut().push(Action::History(step)));
    let passed = Rc::new(Cell::new(false));
    let result = passed.clone();
    let weak = window.as_weak();
    slint::Timer::single_shot(Duration::from_millis(300), move || {
        let window = weak.upgrade().unwrap();
        for (key, shift, expected) in [
            ("k", false, Action::Browse),
            ("n", false, Action::New),
            (",", false, Action::Appearance),
            ("f", false, Action::Find),
            ("g", false, Action::FindStep(1)),
            ("G", true, Action::FindStep(-1)),
            ("s", false, Action::Save),
            ("[", false, Action::History(-1)),
            ("]", false, Action::History(1)),
            ("w", false, Action::Hide),
            ("q", false, Action::Quit),
        ] {
            window.invoke_close_find();
            window.invoke_focus_editor();
            actions.borrow_mut().clear();
            // Slint's public Control key represents macOS Command after Winit mapping.
            chord(&window, Key::Control, key, shift);
            assert_eq!(*actions.borrow(), [expected], "Command shortcut: {key}");
            assert_eq!(
                window.get_note_text(),
                source,
                "shortcut changed source: {key}"
            );
            if expected == Action::Find {
                assert!(window.get_find_visible());
            }
        }
        actions.borrow_mut().clear();
        chord(&window, Key::Meta, ",", false);
        assert!(
            actions.borrow().is_empty(),
            "physical Control is not Command"
        );
        result.set(true);
        println!("native_shortcut_contract_passed: true");
        let _ = slint::quit_event_loop();
    });
    window.run()?;
    if !passed.get() {
        return Err("native shortcut contract failed".into());
    }
    Ok(())
}
