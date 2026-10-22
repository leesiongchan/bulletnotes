//! Owned native search preview proof with generated local data and no network.
#[cfg(all(target_os = "macos", feature = "native-validation"))]
mod native {
    use bulletnotes::{
        AppWindow,
        app::{appearance_ui, coordinator::AppAction, local_ui, ui::present_settings},
        domain::{connection::ConnectionConfig, page::*},
        platform::{application::NativeApplication, renderer, window_chrome},
        store::{StoreHandle, settings::AppSettings},
        theme::{SystemAppearance, ThemeId},
    };
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSApplication, NSEvent, NSEventModifierFlags, NSEventType};
    use objc2_foundation::{NSPoint, NSProcessInfo, NSString};
    use slint::{ComponentHandle, Model};
    use std::{cell::RefCell, rc::Rc, time::Duration};
    type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
    async fn pause() -> Result<()> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        slint::Timer::single_shot(Duration::from_millis(120), move || {
            let _ = tx.send(());
        });
        rx.await?;
        Ok(())
    }
    fn check(value: bool, message: &'static str) -> Result<()> {
        if value { Ok(()) } else { Err(message.into()) }
    }
    async fn key(ui: &AppWindow, text: &str, code: u16) -> Result<()> {
        let window = window_chrome::native_window(ui.window())?;
        let app = NSApplication::sharedApplication(MainThreadMarker::from(&*window));
        check(
            window.isKeyWindow() && app.isActive(),
            "owned search window lost activation",
        )?;
        let text = NSString::from_str(text);
        for kind in [NSEventType::KeyDown, NSEventType::KeyUp] {
            let event=NSEvent::keyEventWithType_location_modifierFlags_timestamp_windowNumber_context_characters_charactersIgnoringModifiers_isARepeat_keyCode(
                kind,NSPoint::new(0.0,0.0),NSEventModifierFlags::empty(),NSProcessInfo::processInfo().systemUptime(),window.windowNumber(),None,&text,&text,false,code
            ).ok_or("owned search key construction failed")?;
            app.postEvent_atStart(&event, false);
        }
        pause().await
    }
    fn capture(ui: &AppWindow, name: &str) -> Result<()> {
        std::fs::create_dir_all("target/antinote-interactions")?;
        let native = window_chrome::native_window(ui.window())?;
        let path = format!("target/antinote-interactions/search-{name}.png");
        let status = std::process::Command::new("/usr/sbin/screencapture")
            .args(["-x", "-o", &format!("-l{}", native.windowNumber()), &path])
            .status()?;
        check(status.success(), "owned search capture failed")
    }
    fn visible_selection(ui: &AppWindow) -> Result<()> {
        let inner = i_slint_core::window::WindowInner::from_pub(ui.window());
        let item = inner
            .focus_item
            .borrow()
            .upgrade()
            .ok_or("search selection lost focus")?;
        let geometry = item.geometry();
        let point = item.map_to_window(geometry.origin);
        check(
            geometry.height() == 112.0
                && point.y >= 180.0
                && point.y + 112.0
                    <= ui
                        .window()
                        .size()
                        .to_logical(ui.window().scale_factor())
                        .height
                        - 34.0,
            "selected result is not fully visible",
        )
    }
    pub fn run() -> Result<()> {
        let application = NativeApplication::prepare()?;
        renderer::select()?;
        let runtime = tokio::runtime::Runtime::new()?;
        let directory = tempfile::tempdir()?;
        let store = runtime.block_on(StoreHandle::open(&directory.path().join("search.sqlite")))?;
        let connection = ConnectionConfig::parse("https://search-preview.example/")?
            .id()
            .clone();
        let first = PageKey {
            connection: connection.clone(),
            path: PagePath::parse("Generated/000.md")?,
        };
        let long_token = format!("needle{}", "x".repeat(500));
        runtime.block_on(async {
            for n in 0..202 {
                let page=PageKey {connection:connection.clone(),path:PagePath::parse(&format!("Generated/{n:03}.md"))?};
                let tail=if n==1 {long_token.as_str()}else{"needle café 日本語 👩🏽‍💻 <b>needle</b> **literal** https://example.invalid tail context that wraps"};
                let body=format!("# Generated {n:03}\n{}{tail}","unrelated ".repeat(80));
                store.create_draft(page.clone()).await?;store.save_local(page,body,LocalRevision(1)).await?;
            }
            let online=PageKey {connection:connection.clone(),path:PagePath::parse("Online/Needle.md")?};
            store.record_catalogue(&connection,vec![RemoteMetadata {key:online,modified_ms:None,content_length:None,permission:Permission::ReadOnly,revision:None}],true).await?;
            Ok::<_,Box<dyn std::error::Error>>(())
        })?;
        let ui = AppWindow::new()?;
        let appearance = appearance_ui::bind(
            &ui,
            store.clone(),
            AppSettings::default(),
            SystemAppearance::Light,
        )?;
        let local = local_ui::bind(&ui, store.clone(), connection, appearance)?;
        let outcome = Rc::new(RefCell::new(None));
        let finished = outcome.clone();
        let weak = ui.as_weak();
        slint::spawn_local(async move {
            let result: Result<()> = async {
                let ui = weak.upgrade().ok_or("owned search window closed")?;
                for _ in 0..4 {
                    pause().await?;
                }
                window_chrome::configure(ui.window())?;
                let native = window_chrome::native_window(ui.window())?;
                native.makeKeyAndOrderFront(None);
                NSApplication::sharedApplication(MainThreadMarker::from(&*native)).activate();
                let view = native.contentView().ok_or("owned search view absent")?;
                check(
                    native.makeFirstResponder(Some(&view)),
                    "owned search responder absent",
                )?;
                local.dispatch(AppAction::OpenPage(first.clone()));
                local.wait_until_loaded(&first).await?;
                ui.invoke_browse_pages();
                ui.set_search_query("needle".into());
                local.search_current().await?;
                pause().await?;
                check(
                    ui.get_page_results().row_count() == 200 && ui.get_can_load_more_pages(),
                    "initial native search pagination differs",
                )?;
                let row = ui
                    .get_page_results()
                    .row_data(0)
                    .ok_or("generated first result absent")?;
                check(
                    row.excerpt.starts_with("…needle")
                        && row.excerpt.contains("<b>needle</b>")
                        && row.excerpt.contains("**literal**"),
                    "native preview lost match or literal markup",
                )?;
                check(
                    row.excerpt_highlights.row_count() > 0,
                    "native preview lacks highlight metadata",
                )?;
                for (name, width, height, theme, contrast) in [
                    ("tartan", 505.0, 552.0, "antinote-tartan", false),
                    ("mononoke-min", 400.0, 440.0, "antinote-mononoke", false),
                    ("contrast-min", 400.0, 440.0, "antinote-tartan", true),
                ] {
                    let mut settings = AppSettings::default();
                    settings.appearance.light_theme = ThemeId(theme.into());
                    ui.set_system_high_contrast(contrast);
                    present_settings(&ui, &settings, SystemAppearance::Light)?;
                    ui.window().set_size(slint::LogicalSize::new(width, height));
                    pause().await?;
                    let row = ui
                        .get_page_results()
                        .row_data(0)
                        .ok_or("theme refresh lost rows")?;
                    check(
                        row.excerpt_highlights
                            .iter()
                            .all(|hit| hit.foreground == ui.get_theme().ink),
                        "native search ink did not follow appearance",
                    )?;
                    capture(&ui, name)?;
                }
                for _ in 0..8 {
                    key(&ui, "\u{f701}", 125).await?;
                }
                visible_selection(&ui)?;
                ui.window().set_size(slint::LogicalSize::new(505.0, 552.0));
                pause().await?;
                visible_selection(&ui)?;
                key(&ui, "\u{f700}", 126).await?;
                key(&ui, "\r", 36).await?;
                local.flush().await?;
                let opened = PageKey {
                    connection: first.connection.clone(),
                    path: PagePath::parse("Generated/007.md")?,
                };
                local.wait_until_loaded(&opened).await?;
                check(
                    ui.get_active_view() == "note",
                    "native Return did not open selected result",
                )?;
                ui.invoke_browse_pages();
                pause().await?;
                key(&ui, "x", 7).await?;
                check(
                    ui.get_search_query().len() == 7
                        && ui.get_search_query().replace('x', "") == "needle",
                    "reentered query lost keyboard focus",
                )?;
                key(&ui, "\u{7f}", 51).await?;
                check(
                    ui.get_search_query() == "needle",
                    "reentered query did not delete text",
                )?;
                key(&ui, "\u{1b}", 53).await?;
                check(
                    ui.get_active_view() == "note",
                    "native Escape did not return to note",
                )?;
                ui.invoke_browse_pages();
                ui.set_search_folder("Generated".into());
                ui.set_search_query("needle".into());
                local.search_current().await?;
                ui.invoke_load_more_pages();
                for _ in 0..25 {
                    if ui.get_page_results().row_count() == 201 {
                        break;
                    }
                    pause().await?;
                }
                check(
                    ui.get_page_results().row_count() == 201 && !ui.get_can_load_more_pages(),
                    "native folder continuation failed",
                )?;
                ui.set_search_query("absent-token".into());
                local.search_current().await?;
                check(
                    ui.get_page_results().row_count() == 0,
                    "new query retained old highlights",
                )?;
                ui.set_search_query(long_token.into());
                local.search_current().await?;
                pause().await?;
                check(
                    ui.get_page_results().row_count() == 1,
                    "long token search failed",
                )?;
                capture(&ui, "long-token")?;
                ui.set_search_folder("Online".into());
                ui.set_search_query("needle".into());
                local.search_current().await?;
                let row = ui
                    .get_page_results()
                    .row_data(0)
                    .ok_or("online row absent")?;
                check(
                    row.excerpt.is_empty() && row.excerpt_highlights.row_count() == 0,
                    "online result fabricated body context",
                )?;
                store.shutdown().await?;
                Ok(())
            }
            .await;
            *finished.borrow_mut() = Some(result);
            let _ = slint::quit_event_loop();
        })?;
        ui.show()?;
        application.activate();
        slint::run_event_loop()?;
        ui.hide()?;
        outcome
            .borrow_mut()
            .take()
            .ok_or("native search proof unfinished")??;
        println!("native_search_preview_passed: true");
        Ok(())
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(all(target_os = "macos", feature = "native-validation"))]
    return native::run();
    #[cfg(not(all(target_os = "macos", feature = "native-validation")))]
    Err("requires macOS and --features native-validation".into())
}
