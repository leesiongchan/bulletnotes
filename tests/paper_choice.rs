use bulletnotes::{
    AppWindow,
    app::appearance_ui,
    store::{StoreHandle, settings::AppSettings},
    theme::SystemAppearance,
};
use slint::ComponentHandle;
use std::{cell::Cell, rc::Rc};

#[test]
fn paper_choice_renders_immediately_and_survives_store_reopen() {
    use bulletnotes::theme::PaperType;
    use slint::platform::{Key, WindowEvent};
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
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("paper.sqlite");
    let store = runtime.block_on(StoreHandle::open(&path)).unwrap();
    let mut settings = AppSettings::default();
    settings.appearance.paper_type = PaperType::Smooth;
    let binding =
        appearance_ui::bind(&window, store.clone(), settings, SystemAppearance::Light).unwrap();
    window.set_prefers_reduced_motion(true);
    window.set_note_text("A saved finish".into());
    window.show().unwrap();
    let finished = Rc::new(Cell::new(false));
    let done = finished.clone();
    slint::spawn_local(async move {
        let crop = || {
            let image = window.window().take_snapshot().unwrap();
            (160..288)
                .flat_map(|y| (8..136).map(move |x| (x, y)))
                .flat_map(|(x, y)| {
                    let i = (y * image.width() as usize + x) * 4;
                    image.as_bytes()[i..i + 3].to_vec()
                })
                .collect::<Vec<_>>()
        };
        let flat = crop();
        window.set_active_view("appearance".into());
        let _ = window.window().take_snapshot().unwrap();
        // Walk the real controls; Space activates each focused choice.
        for _ in 0..12 {
            for text in [slint::SharedString::from(Key::Tab), Key::Space.into()] {
                window
                    .window()
                    .dispatch_event(WindowEvent::KeyPressed { text: text.clone() });
                window
                    .window()
                    .dispatch_event(WindowEvent::KeyReleased { text });
            }
            binding.flush().await.unwrap();
            if window.get_paper_type() == "bone" {
                break;
            }
        }
        assert_eq!(window.get_paper_type(), "bone");
        window.set_active_view("note".into());
        let grain = crop();
        assert_ne!(grain, flat);
        window.set_prefers_reduced_transparency(true);
        assert_eq!(crop(), flat);
        assert_eq!(window.get_paper_type(), "bone");
        window.set_prefers_reduced_transparency(false);
        assert_eq!(crop(), grain);
        assert_eq!(window.get_note_text(), "A saved finish");
        store.shutdown().await.unwrap();
        let reopened = StoreHandle::open(&path).await.unwrap();
        let restored = reopened.load_settings().await.unwrap();
        assert_eq!(restored.appearance.paper_type, PaperType::Bone);
        bulletnotes::app::ui::present_settings(&window, &restored, SystemAppearance::Light)
            .unwrap();
        assert_eq!(crop(), grain);
        reopened.shutdown().await.unwrap();
        done.set(true);
        slint::quit_event_loop().unwrap();
    })
    .unwrap();
    slint::Timer::single_shot(std::time::Duration::from_secs(15), || {
        slint::quit_event_loop().unwrap();
    });
    slint::run_event_loop_until_quit().unwrap();
    assert!(finished.get());
}
