use bulletnotes::{
    AppWindow,
    app::appearance_ui,
    store::{StoreHandle, settings::AppSettings},
    theme::{AppearanceMode, SystemAppearance, ThemeId},
};
use slint::ComponentHandle;
use std::{cell::Cell, rc::Rc};
#[test]
fn assignment_cycle_is_atomic_durable_and_rolls_back_on_failure() {
    slint::platform::set_platform(Box::new(i_slint_backend_testing::TestingBackend::new(
        i_slint_backend_testing::TestingBackendOptions {
            mock_time: false,
            threading: true,
            renderer_name: Some("skia".into()),
        },
    )))
    .unwrap();
    let window = AppWindow::new().unwrap();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let _entered = runtime.enter();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cycle.sqlite");
    let store = runtime.block_on(StoreHandle::open(&path)).unwrap();
    let mut settings = AppSettings::default();
    settings.appearance.light_theme = ThemeId("paper".into());
    settings.appearance.dark_theme = ThemeId("graphite".into());
    runtime
        .block_on(store.save_settings(settings.clone()))
        .unwrap();
    let injector = rusqlite::Connection::open(&path).unwrap();
    injector.execute_batch("CREATE TABLE cycle_writes(id INTEGER); CREATE TRIGGER record_cycle AFTER UPDATE ON settings BEGIN INSERT INTO cycle_writes VALUES (1); END;").unwrap();
    let binding =
        appearance_ui::bind(&window, store.clone(), settings, SystemAppearance::Light).unwrap();
    window.set_note_text("unsaved café 👩🏽‍💻".into());
    window.show().unwrap();
    let complete = Rc::new(Cell::new(false));
    let done = complete.clone();
    slint::spawn_local(async move {
        window.set_active_view("appearance".into());
        let _ = window.window().take_snapshot().unwrap();
        window.invoke_reveal_theme_choices();
        let key = |text: slint::SharedString| {
            window.window().dispatch_event(slint::platform::WindowEvent::KeyPressed { text: text.clone() });
            window.window().dispatch_event(slint::platform::WindowEvent::KeyReleased { text });
        };
        key(slint::platform::Key::Tab.into());
        for (i,(light,dark)) in [("bulletnotes","graphite"),("paper","bulletnotes"),("bulletnotes","bulletnotes")].into_iter().enumerate() {
            key(slint::platform::Key::Space.into());binding.flush().await.unwrap();
            let saved=store.load_settings().await.unwrap();
            assert_eq!(saved.appearance.light_theme.0,light);assert_eq!(saved.appearance.dark_theme.0,dark);assert_eq!(saved.appearance.mode,AppearanceMode::System);
            let writes:i64=injector.query_row("SELECT COUNT(*) FROM cycle_writes",[],|r|r.get(0)).unwrap();assert_eq!(writes,i as i64+1);
        }
        let both=store.load_settings().await.unwrap();
        injector.execute_batch("CREATE TRIGGER fail_cycle BEFORE UPDATE ON settings BEGIN SELECT RAISE(ABORT,'fixture failure'); END;").unwrap();
        window.invoke_cycle_theme_assignment("bulletnotes".into());binding.flush().await.unwrap();
        assert_eq!(store.load_settings().await.unwrap(),both);
        assert_eq!(window.get_selected_light_theme_id(),"bulletnotes");assert_eq!(window.get_selected_dark_theme_id(),"bulletnotes");
        assert!(window.get_appearance_message().contains("could not be saved"));
        assert_eq!(window.get_note_text(),"unsaved café 👩🏽‍💻");
        injector.execute_batch("DROP TRIGGER fail_cycle").unwrap();drop(injector);
        store.shutdown().await.unwrap();let reopened=StoreHandle::open(&path).await.unwrap();
        let saved=reopened.load_settings().await.unwrap();assert_eq!(saved,both);
        let controller=bulletnotes::app::appearance::AppearanceController::new(saved,SystemAppearance::Light).unwrap();
        let light=controller.cycle_assignment_candidate(ThemeId("bulletnotes".into())).unwrap();assert_eq!(light.appearance.dark_theme.0,"graphite");
        reopened.shutdown().await.unwrap();done.set(true);slint::quit_event_loop().unwrap();
    }).unwrap();
    slint::Timer::single_shot(std::time::Duration::from_secs(15), || {
        slint::quit_event_loop().unwrap();
    });
    slint::run_event_loop_until_quit().unwrap();
    assert!(complete.get());
}
