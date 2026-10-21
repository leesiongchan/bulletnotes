//! AppKit input checks against an owned generated editor; no user store or global events.
use bulletnotes::{
    AppWindow,
    app::ui::present_settings,
    platform::{application::NativeApplication, renderer, window_chrome},
    store::settings::AppSettings,
    theme::{SystemAppearance, ThemeId},
};
use objc2::{
    MainThreadMarker, msg_send,
    rc::Retained,
    runtime::{AnyObject, ProtocolObject},
    sel,
};
use objc2_app_kit::{
    NSApplication, NSApplicationDidBecomeActiveNotification,
    NSApplicationDidResignActiveNotification, NSEvent, NSEventModifierFlags, NSEventType, NSWindow,
    NSWindowDidBecomeKeyNotification, NSWindowDidResignKeyNotification,
};
use objc2_foundation::{
    NSArray, NSNotification, NSNotificationCenter, NSObjectProtocol, NSPoint, NSProcessInfo,
    NSRange, NSRect, NSString,
};
use slint::ComponentHandle;
use std::{cell::Cell, rc::Rc, time::Duration};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

struct FocusTrace(Vec<Retained<ProtocolObject<dyn NSObjectProtocol>>>);

impl FocusTrace {
    fn install(window: &AppWindow) -> Result<Option<Self>> {
        if !std::env::args().any(|arg| arg == "--trace-input") {
            return Ok(None);
        }
        let owned = window_chrome::native_window(window.window())?.windowNumber();
        let mut trace = Self(Vec::new());
        // SAFETY: AppKit exports immutable notification names; the guard removes
        // copied blocks at the end of the native check.
        for (name, phase) in unsafe {
            [
                (NSWindowDidBecomeKeyNotification, "became_key"),
                (NSWindowDidResignKeyNotification, "resigned_key"),
                (NSApplicationDidBecomeActiveNotification, "became_active"),
                (NSApplicationDidResignActiveNotification, "resigned_active"),
            ]
        } {
            let block = block2::RcBlock::new(
                move |notification: std::ptr::NonNull<NSNotification>| {
                    let Some(mtm) = MainThreadMarker::new() else {
                        return;
                    };
                    let app = NSApplication::sharedApplication(mtm);
                    // SAFETY: Foundation supplies a live notification during this callback.
                    let object = unsafe { notification.as_ref() }.object();
                    let notified = object
                        .as_ref()
                        .and_then(|object| object.downcast_ref::<NSWindow>())
                        .map(|window| (window.windowNumber(), window.class().name()));
                    let key = app
                        .keyWindow()
                        .map(|window| (window.windowNumber(), window.class().name()));
                    eprintln!(
                        "native_focus: phase={phase} uptime={:.6} notified={notified:?} owned={owned} key={key:?} active={}",
                        NSProcessInfo::processInfo().systemUptime(),
                        app.isActive()
                    );
                },
            );
            // SAFETY: observes only this process, synchronously on the posting thread;
            // the block checks main-thread access and only reads owned AppKit state.
            trace.0.push(unsafe {
                NSNotificationCenter::defaultCenter().addObserverForName_object_queue_usingBlock(
                    Some(name),
                    None,
                    None,
                    &block,
                )
            });
        }
        Ok(Some(trace))
    }
}

impl Drop for FocusTrace {
    fn drop(&mut self) {
        for token in &self.0 {
            // SAFETY: each token belongs to this main-thread guard and is removed once.
            unsafe {
                NSNotificationCenter::defaultCenter().removeObserver((**token).as_ref());
            }
        }
    }
}

async fn pause(ms: u64) -> Result<()> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    slint::Timer::single_shot(Duration::from_millis(ms), move || {
        let _ = tx.send(());
    });
    rx.await?;
    Ok(())
}

fn native_key(window: &AppWindow, text: &str, code: u16) -> Result<()> {
    native_key_with_flags(window, text, code, NSEventModifierFlags::empty())
}

fn native_key_with_flags(
    window: &AppWindow,
    text: &str,
    code: u16,
    flags: NSEventModifierFlags,
) -> Result<()> {
    let native = window_chrome::native_window(window.window())?;
    let app =
        NSApplication::sharedApplication(MainThreadMarker::new().ok_or("main thread required")?);
    let text = NSString::from_str(text);
    let modifiers = [
        (NSEventModifierFlags::Command, 55),
        (NSEventModifierFlags::Shift, 56),
    ];
    let mut held = NSEventModifierFlags::empty();
    let mut events = Vec::new();
    // Slint tracks physical modifier transitions, not winit's ModifiersChanged alone.
    for (flag, code) in modifiers {
        if flags.contains(flag) {
            held.insert(flag);
            events.push((NSEventType::FlagsChanged, code, held));
        }
    }
    events.extend([
        (NSEventType::KeyDown, code, flags),
        (NSEventType::KeyUp, code, flags),
    ]);
    for (flag, code) in modifiers.into_iter().rev() {
        if flags.contains(flag) {
            held.remove(flag);
            events.push((NSEventType::FlagsChanged, code, held));
        }
    }
    for (kind, code, flags) in events {
        let event = NSEvent::keyEventWithType_location_modifierFlags_timestamp_windowNumber_context_characters_charactersIgnoringModifiers_isARepeat_keyCode(
            kind, NSPoint::new(0.0, 0.0), flags,
            NSProcessInfo::processInfo().systemUptime(), native.windowNumber(), None,
            &text, &text, false, code,
        ).ok_or("native key construction failed")?;
        app.postEvent_atStart(&event, false);
    }
    Ok(())
}

fn undo(window: &AppWindow) -> Result<()> {
    native_key_with_flags(window, "z", 6, NSEventModifierFlags::Command)
}

fn expect_source(window: &AppWindow, source: &str) -> Result<()> {
    if window.get_note_text().as_bytes() != source.as_bytes() {
        return Err(format!("native source differs: {:?}", window.get_note_text()).into());
    }
    Ok(())
}

fn anchor(view: &objc2_app_kit::NSView) -> NSRect {
    // SAFETY: main-thread owned Winit view implements this public NSTextInputClient selector.
    unsafe {
        msg_send![view, firstRectForCharacterRange: NSRange::new(0, 0), actualRange: std::ptr::null_mut::<NSRange>()]
    }
}

fn capture(window: &AppWindow, name: &str) -> Result<()> {
    let native = window_chrome::native_window(window.window())?;
    let path = format!("target/editor-geometry/{name}.png");
    let status = std::process::Command::new("/usr/sbin/screencapture")
        .args(["-x", "-o", &format!("-l{}", native.windowNumber()), &path])
        .status()?;
    if !status.success() {
        return Err("owned window capture failed".into());
    }
    println!("native_capture: {path}");
    Ok(())
}

// Public CoreGraphics events target only this fixture PID, never the global event stream.
#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGEventCreateKeyboardEvent(
        source: *const std::ffi::c_void,
        code: u16,
        down: bool,
    ) -> *const std::ffi::c_void;
    fn CGEventPostToPid(pid: i32, event: *const std::ffi::c_void);
    fn CGEventGetFlags(event: *const std::ffi::c_void) -> u64;
    fn CGPreflightPostEventAccess() -> bool;
}
#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFRelease(object: *const std::ffi::c_void);
}

async fn system_key(window: &AppWindow, code: u16) -> Result<()> {
    let native = window_chrome::native_window(window.window())?;
    let app = NSApplication::sharedApplication(MainThreadMarker::from(&*native));
    if !app.isActive() || !native.isKeyWindow() {
        return Err("native input interrupted: owned editor is not the active key window".into());
    }
    for down in [true, false] {
        // SAFETY: documented Quartz ABI; each created event is released after posting
        // to our own PID. No external window can receive these generated keystrokes.
        unsafe {
            let event = CGEventCreateKeyboardEvent(std::ptr::null(), code, down);
            if event.is_null() {
                return Err("Quartz event creation failed".into());
            }
            if std::env::args().any(|arg| arg == "--trace-input") {
                eprintln!(
                    "quartz_key_created: code={code} down={down} flags={:#x} uptime={:.6}",
                    CGEventGetFlags(event),
                    NSProcessInfo::processInfo().systemUptime()
                );
            }
            CGEventPostToPid(std::process::id() as i32, event);
            CFRelease(event);
        }
        pause(30).await?;
    }
    pause(120).await
}

fn trace_input_state(
    phase: &str,
    window: &AppWindow,
    view: &objc2_app_kit::NSView,
    context: &AnyObject,
) -> Result<()> {
    if !std::env::args().any(|arg| arg == "--trace-input") {
        return Ok(());
    }
    let native = window_chrome::native_window(window.window())?;
    let app = NSApplication::sharedApplication(MainThreadMarker::from(view));
    // SAFETY: public selectors on this checked main-thread NSTextInputClient/context.
    let selected: Option<Retained<NSString>> =
        unsafe { msg_send![context, selectedKeyboardInputSource] };
    let current_class = objc2::runtime::AnyClass::get(c"NSTextInputContext")
        .ok_or("input context class missing")?;
    let current: Option<Retained<AnyObject>> =
        unsafe { msg_send![current_class, currentInputContext] };
    let context_active = current
        .as_ref()
        .is_some_and(|current| std::ptr::eq(&**current, context));
    let marked: bool = unsafe { msg_send![view, hasMarkedText] };
    let range: NSRange = unsafe { msg_send![view, markedRange] };
    let responder = native.firstResponder();
    let key_window = app.keyWindow().map(|window| window.windowNumber());
    let editor_responder = responder.as_ref().is_some_and(|responder| {
        Retained::as_ptr(responder).cast::<AnyObject>()
            == (view as *const objc2_app_kit::NSView).cast::<AnyObject>()
    });
    eprintln!(
        "pinyin_input_state: phase={phase} uptime={:.6} source={selected:?} active={} key={} window={} key_window={key_window:?} editor_responder={editor_responder} context_active={context_active} marked={marked} marked_len={} source_len={} scroll={}",
        NSProcessInfo::processInfo().systemUptime(),
        app.isActive(),
        native.isKeyWindow(),
        native.windowNumber(),
        range.length,
        window.get_note_text().len(),
        window.get_editor_scroll_y()
    );
    Ok(())
}

async fn pinyin(
    window: &AppWindow,
    view: &objc2_app_kit::NSView,
    context: &AnyObject,
    source: &str,
    theme: &str,
) -> Result<()> {
    // SAFETY: read-only permission preflight; this fixture never requests a permission.
    if !unsafe { CGPreflightPostEventAccess() } {
        return Err("native event-post access unavailable".into());
    }
    let method = NSString::from_str("com.apple.inputmethod.SCIM.ITABC");
    let previous: Option<Retained<NSString>> =
        unsafe { msg_send![context, selectedKeyboardInputSource] };
    // Keep cleanup outside the inner check so every Result error restores the source.
    unsafe {
        let _: () = msg_send![context, setSelectedKeyboardInputSource: &*method];
    }
    let check: Result<()> = async {
        pause(300).await?;
        let current: Option<Retained<NSString>> =
            unsafe { msg_send![context, selectedKeyboardInputSource] };
        if current.as_deref() != Some(&*method) {
            return Err("Pinyin did not activate".into());
        }
        let offset = source.rfind("Task").ok_or("generated Task absent")?;
        window.invoke_restore_editor_position(offset as i32, offset as i32, window.get_editor_scroll_y());
        pause(100).await?;
        trace_input_state("before_typing", window, view, context)?;
        for code in [45, 34, 4, 0, 31] {
            system_key(window, code).await?;
        }
        trace_input_state("after_typing", window, view, context)?;
        let has_marked: bool = unsafe { msg_send![view, hasMarkedText] };
        if !has_marked {
            return Err("Pinyin did not compose nihao".into());
        }
        expect_source(window, source)?;
        capture(window, &format!("native-{theme}-pinyin-preedit"))?;
        let native = window_chrome::native_window(window.window())?;
        let screen = anchor(view);
        let local = view.convertRect_fromView(native.convertRectFromScreen(screen), None);
        let visible = local.origin.x >= 0.0 && local.origin.x < view.bounds().size.width
            && local.origin.y >= 0.0 && local.origin.y + local.size.height <= view.bounds().size.height + 1.0;
        println!("native_pinyin_candidate_anchor: {theme} screen={screen:?} local={local:?} visible={visible}");
        if !visible { return Err("native Pinyin anchor outside visible editor".into()); }
        let app = NSApplication::sharedApplication(MainThreadMarker::new().ok_or("main thread required")?);
        let mut panels = 0;
        for panel in app.windows() {
            if panel.windowNumber() != native.windowNumber() && panel.isVisible() {
                let path = format!("target/editor-geometry/native-{theme}-candidate-{panels}.png");
                let status = std::process::Command::new("/usr/sbin/screencapture")
                    .args(["-x", "-o", &format!("-l{}", panel.windowNumber()), &path]).status()?;
                if !status.success() { return Err("owned candidate panel capture failed".into()); }
                let frame = panel.frame();
                let horizontal_distance = (screen.origin.x - screen.origin.x.clamp(frame.origin.x, frame.origin.x + frame.size.width)).abs();
                let vertical_distance = (screen.origin.y - frame.origin.y - frame.size.height).abs()
                    .min((frame.origin.y - screen.origin.y - screen.size.height).abs());
                if horizontal_distance > 20.0 || vertical_distance > screen.size.height * 2.0 {
                    return Err("native candidate panel is detached from the caret".into());
                }
                println!("native_candidate_panel: {theme} frame={frame:?} path={path}");
                panels += 1;
            }
        }
        if panels == 0 { return Err("no owned native Pinyin candidate panel".into()); }
        println!(
            "pinyin_candidate_capture_window: {}",
            window_chrome::native_window(window.window())?.windowNumber()
        );
        trace_input_state("candidate_captured", window, view, context)?;
        pause(2500).await?;
        trace_input_state("before_space", window, view, context)?;
        system_key(window, 49).await?; // Space commits the leading conversion candidate.
        trace_input_state("after_space", window, view, context)?;
        let mut committed = source.to_string();
        committed.insert_str(offset, "你好");
        expect_source(window, &committed)?;
        capture(window, &format!("native-{theme}-pinyin-committed"))?;
        undo(window)?;
        pause(100).await?;
        trace_input_state("after_undo", window, view, context)?;
        expect_source(window, source)?;
        // A second real conversion must cancel without modifying the source.
        for code in [45, 34] {
            system_key(window, code).await?;
            trace_input_state(&format!("cancel_typing_{code}"), window, view, context)?;
        }
        let has_marked: bool = unsafe { msg_send![view, hasMarkedText] };
        if !has_marked {
            return Err("Pinyin cancellation had no active composition".into());
        }
        system_key(window, 53).await?;
        trace_input_state("after_escape", window, view, context)?;
        let has_marked: bool = unsafe { msg_send![view, hasMarkedText] };
        if has_marked {
            return Err("Pinyin Escape did not cancel composition".into());
        }
        expect_source(window, source)?;
        println!("native_pinyin_conversion_commit_cancel_passed: {theme}");
        Ok(())
    }
    .await;
    if check.is_err() {
        let _ = trace_input_state("failed", window, view, context);
        if let Err(error) = capture(window, &format!("native-{theme}-pinyin-failed")) {
            eprintln!("failure capture: {error}");
        }
    }
    // SAFETY: public selectors on the same main-thread input client/context.
    unsafe {
        let _: () = msg_send![view, unmarkText];
        let _: () = msg_send![context, setSelectedKeyboardInputSource: previous.as_deref()];
    }
    pause(200).await?;
    let restored: Option<Retained<NSString>> =
        unsafe { msg_send![context, selectedKeyboardInputSource] };
    if restored != previous {
        return Err("input source restoration failed".into());
    }
    println!("native_input_source_restored: true");
    check
}

pub fn run() -> Result<()> {
    let _trace_worker = if std::env::args().any(|arg| arg == "--trace-input") {
        use tracing_subscriber::{Layer, layer::SubscriberExt, util::SubscriberInitExt};
        std::fs::create_dir_all("target/editor-ime")?;
        let path = format!("target/editor-ime/callbacks-{}.log", std::process::id());
        let file = std::fs::File::create(&path)?;
        let (writer, worker) = tracing_appender::non_blocking::NonBlockingBuilder::default()
            .lossy(false)
            .finish(file);
        eprintln!("native_callback_log: {path}");
        tracing_subscriber::registry()
            .with(
                tracing_subscriber::fmt::layer()
                    .with_ansi(false)
                    .with_writer(writer)
                    .with_filter(tracing_subscriber::filter::filter_fn(|metadata| {
                        metadata.target().starts_with("winit::platform_impl::")
                            && (metadata.target().ends_with("::view")
                                || metadata.target().ends_with("::util"))
                    })),
            )
            .try_init()?;
        Some(worker)
    } else {
        None
    };
    let application = NativeApplication::prepare()?;
    renderer::select()?;
    std::fs::create_dir_all("target/editor-geometry")?;
    let window = AppWindow::new()?;
    present_settings(&window, &AppSettings::default(), SystemAppearance::Light)?;
    if std::env::args().any(|arg| arg == "--trace-input") {
        window.on_note_edited(|text| {
            eprintln!(
                "native_edit: uptime={:.6} source_len={}",
                NSProcessInfo::processInfo().systemUptime(),
                text.len()
            )
        });
    }
    let frames = Rc::new(Cell::new(0));
    let count = frames.clone();
    window.window().set_rendering_notifier(move |state, _| {
        if count.get() == 0 || matches!(state, slint::RenderingState::RenderingTeardown) {
            eprintln!("native_render_state: {state:?}");
        }
        if matches!(state, slint::RenderingState::AfterRendering) {
            count.set(count.get() + 1);
        }
    })?;
    let source = "- [ ] Task café 日本語 👩🏽‍💻\r\n- List\r\n";
    window.set_note_text(source.into());
    let single_undo = Rc::new(Cell::new(true));
    let single_undo_result = single_undo.clone();
    let passed = Rc::new(Cell::new(false));
    let result = passed.clone();
    let weak = window.as_weak();
    slint::spawn_local(async move {
        let check: Result<()> = async {
            let window = weak.upgrade().ok_or("fixture closed")?;
            pause(300).await?;
            window_chrome::configure(window.window())?;
            let native = window_chrome::native_window(window.window())?;
            native.makeKeyAndOrderFront(None);
            eprintln!("native_startup: visible={} alpha={} key={} occlusion={:?} metal={:?}", native.isVisible(), native.alphaValue(), native.isKeyWindow(), native.occlusionState(), renderer::uses_metal(window.window()));
            for _ in 0..80 {
                if frames.get() > 0 { break; }
                pause(100).await?;
            }
            if frames.get() == 0 {
                eprintln!("native_frame_timeout: visible={} alpha={} occlusion={:?}", native.isVisible(), native.alphaValue(), native.occlusionState());
                return Err("native editor received no rendered frame".into());
            }
            pause(100).await?;
            if !native.isKeyWindow() { return Err("owned editor did not become key".into()); }
            let view = native.contentView().ok_or("native input view missing")?;
            if !native.makeFirstResponder(Some(&view)) {
                return Err("owned native input view did not become first responder".into());
            }
            if !view.respondsToSelector(sel!(setMarkedText:selectedRange:replacementRange:)) {
                return Err("native view does not implement NSTextInputClient".into());
            }
            // SAFETY: NSView's public inputContext getter and NSTextInputContext getters.
            let context: Retained<AnyObject> = unsafe { msg_send![&*view, inputContext] };
            let _focus_trace = FocusTrace::install(&window)?;
            let sources: Option<Retained<NSArray<NSString>>> = unsafe { msg_send![&*context, keyboardInputSources] };
            let selected: Option<Retained<NSString>> = unsafe { msg_send![&*context, selectedKeyboardInputSource] };
            println!("native_input_sources: {:?}; selected={selected:?}", sources.map(|s| s.iter().map(|s| s.to_string()).collect::<Vec<_>>()));
            for theme in ["antinote-tartan", "antinote-mononoke"] {
                let mut settings = AppSettings::default();
                settings.appearance.light_theme = ThemeId(theme.into());
                settings.appearance.font_size = 14.0;
                settings.appearance.paper = bulletnotes::theme::PaperStyle::Grid;
                present_settings(&window, &settings, SystemAppearance::Light)?;
                window.set_note_text(source.into());
                window.invoke_focus_editor();
                window.invoke_restore_editor_position(6, 6, 0.0);
                pause(100).await?;
                // Exercise AppKit's real event queue and Winit's key mapping, not Slint dispatch.
                native_key(&window, "\u{f703}", 124)?;
                pause(50).await?;
                native_key(&window, "x", 7)?;
                pause(50).await?;
                if window.get_note_text() != source.replacen("Task", "Txask", 1) {
                    return Err(format!("native Right-arrow/key input did not edit the expected source offset: {:?}", window.get_note_text()).into());
                }
                undo(&window)?;
                pause(50).await?;
                if window.get_note_text() != source { return Err("native undo changed source".into()); }
                // Check source offsets across accented text, CJK, and an emoji grapheme.
                for (label, start, end, flags) in [
                    ("accent", source.find('é').ok_or("accent absent")?, source.find('é').ok_or("accent absent")? + 2, NSEventModifierFlags::empty()),
                    ("cjk", source.find('日').ok_or("CJK absent")?, source.find('日').ok_or("CJK absent")? + 3, NSEventModifierFlags::empty()),
                    ("emoji", source.find('👩').ok_or("emoji absent")?, source.find('👩').ok_or("emoji absent")? + "👩🏽‍💻".len(), NSEventModifierFlags::empty()),
                    ("emoji-selection", source.find('👩').ok_or("emoji absent")?, source.find('👩').ok_or("emoji absent")? + "👩🏽‍💻".len(), NSEventModifierFlags::Shift),
                ] {
                    window.invoke_restore_editor_position(start as i32, start as i32, 0.0);
                    pause(60).await?;
                    native_key_with_flags(&window, "\u{f703}", 124, flags)?;
                    pause(60).await?;
                    native_key(&window, "x", 7)?;
                    pause(60).await?;
                    let mut expected = source.to_string();
                    let range = if flags.is_empty() { end..end } else { start..end };
                    expected.replace_range(range, "x");
                    expect_source(&window, &expected)?;
                    undo(&window)?;
                    pause(60).await?;
                    let mut undo_steps = 1;
                    if window.get_note_text() != source && !flags.is_empty() {
                        let mut deleted = source.to_string();
                        deleted.replace_range(start..end, "");
                        expect_source(&window, &deleted)?;
                        single_undo_result.set(false);
                        undo_steps = 2;
                        println!("native_selection_single_undo_passed: false; {theme}; replacement requires two undo steps");
                        undo(&window)?;
                        pause(60).await?;
                    }
                    expect_source(&window, source)?;
                    native_key_with_flags(&window, "z", 6, NSEventModifierFlags::Command | NSEventModifierFlags::Shift)?;
                    pause(60).await?;
                    if undo_steps == 2 {
                        native_key_with_flags(&window, "z", 6, NSEventModifierFlags::Command | NSEventModifierFlags::Shift)?;
                    }
                    pause(60).await?;
                    expect_source(&window, &expected)?;
                    undo(&window)?;
                    pause(60).await?;
                    if undo_steps == 2 { undo(&window)?; pause(60).await?; }
                    expect_source(&window, source)?;
                    println!("native_cursor_case: {theme}/{label} passed");
                }
                window.invoke_restore_editor_position(6, 6, 0.0);
                pause(60).await?;
                let raw_anchor = anchor(&view);
                native_key(&window, "\u{f703}", 124)?;
                pause(60).await?;
                let compact_anchor = anchor(&view);
                if raw_anchor.size.height <= 0.0 || compact_anchor.size.height <= 0.0
                    || compact_anchor.origin.x >= raw_anchor.origin.x {
                    return Err(format!("native candidate anchor did not follow compact prefix: {raw_anchor:?} -> {compact_anchor:?}").into());
                }
                println!("native_candidate_anchor: {theme} raw={raw_anchor:?} compact={compact_anchor:?}");
                window.invoke_restore_editor_position(6, 6, 0.0);
                pause(60).await?;
                capture(&window, &format!("native-{theme}-before"))?;
                window.invoke_restore_editor_position(6, 6, 0.0);
                pause(50).await?;
                let before = window.window().take_snapshot()?;
                let marked = NSString::from_str("にほん");
                let missing = NSRange::new(isize::MAX as usize, 0);
                // SAFETY: this owned main-thread view advertises the documented text-input
                // selector; NSString and UTF-16 ranges match NSTextInputClient's ABI.
                unsafe {
                    let _: () = msg_send![&*view, setMarkedText: &*marked, selectedRange: NSRange::new(3, 0), replacementRange: missing];
                }
                pause(100).await?;
                let has_marked: bool = unsafe { msg_send![&*view, hasMarkedText] };
                if !has_marked || window.get_note_text() != source
                    || window.window().take_snapshot()?.as_bytes() == before.as_bytes() {
                    return Err("native marked text was not rendered separately from source".into());
                }
                capture(&window, &format!("native-{theme}-preedit"))?;
                println!("native_preedit_anchor: {theme} {:?}", anchor(&view));
                // Cancellation must clear only the temporary marked text.
                unsafe { let _: () = msg_send![&*view, unmarkText]; }
                pause(60).await?;
                expect_source(&window, source)?;
                let has_marked: bool = unsafe { msg_send![&*view, hasMarkedText] };
                if has_marked { return Err("native cancellation retained marked text".into()); }
                unsafe {
                    let _: () = msg_send![&*view, setMarkedText: &*marked, selectedRange: NSRange::new(1, 1), replacementRange: missing];
                }
                pause(60).await?;
                expect_source(&window, source)?;
                let committed = NSString::from_str("日本");
                // SAFETY: same retained native input view and documented commit signature.
                unsafe { let _: () = msg_send![&*view, insertText: &*committed, replacementRange: missing]; }
                pause(100).await?;
                if window.get_note_text() != source.replacen("Task", "日本Task", 1) {
                    return Err("native IME commit did not preserve surrounding source".into());
                }
                capture(&window, &format!("native-{theme}-committed"))?;
                // Direct protocol calls bypass keyDown's usual end-of-commit cleanup.
                // SAFETY: public NSTextInputClient selector on the checked owned view.
                unsafe { let _: () = msg_send![&*view, unmarkText]; }
                pause(50).await?;
                let has_marked: bool = unsafe { msg_send![&*view, hasMarkedText] };
                if has_marked { return Err("native composition cleanup failed".into()); }
                undo(&window)?;
                pause(50).await?;
                if window.get_note_text() != source { return Err("native IME undo changed source".into()); }
                // Scrolled, wrapped text must keep the platform candidate anchor in the editor.
                let long = format!("{source}{}{}\r\n- [ ] Task", "Generated scroll line café 日本語\r\n".repeat(60), "generated wrap text ".repeat(18));
                window.set_note_text(long.clone().into());
                window.invoke_restore_editor_position(long.len() as i32, long.len() as i32, 0.0);
                pause(100).await?;
                let end_anchor = anchor(&view);
                let local = view.convertRect_fromView(native.convertRectFromScreen(end_anchor), None);
                let top = if view.isFlipped() { local.origin.y } else { view.bounds().size.height - local.origin.y - local.size.height };
                if window.get_editor_scroll_y() <= 0.0 || !(0.0..view.bounds().size.width).contains(&local.origin.x)
                    || !(0.0..view.bounds().size.height).contains(&top) {
                    return Err(format!("scrolled native candidate anchor is outside editor: {local:?}, top={top}").into());
                }
                expect_source(&window, &long)?;
                println!("native_scrolled_anchor: {theme} local={local:?} top={top} scroll={}", window.get_editor_scroll_y());
                capture(&window, &format!("native-{theme}-scrolled"))?;
                if std::env::args().any(|arg| arg == "--pinyin") {
                    pinyin(&window, &view, &context, &long, &format!("{theme}-scrolled")).await?;
                }
                window.set_note_text(source.into());
                window.invoke_restore_editor_position(6, 6, 0.0);
                pause(60).await?;
                if std::env::args().any(|arg| arg == "--pinyin") {
                    pinyin(&window, &view, &context, source, theme).await?;
                }
                println!("native_editor_input_verified: {theme}");
            }
            if std::env::args().any(|arg| arg == "--verify-focus-loss") {
                let source = window.get_note_text();
                println!("native_focus_loss_probe: begin");
                native.resignKeyWindow();
                pause(100).await?;
                if native.isKeyWindow() { return Err("focus-loss probe did not resign key".into()); }
                let input = system_key(&window, 49).await;
                native.becomeKeyWindow();
                pause(100).await?;
                if !native.isKeyWindow() { return Err("focus-loss probe did not restore key".into()); }
                let error = input.err().ok_or("unfocused native key was not rejected")?;
                expect_source(&window, &source)?;
                println!("native_focus_loss_probe_passed: {error}");
            }
            Ok(())
        }.await;
        match check {
            Ok(()) => {
                result.set(true);
                println!("native_editor_input_roundtrips_passed: true");
            }
            Err(error) => eprintln!("native editor input: {error}"),
        }
        let _ = slint::quit_event_loop();
    })?;
    window.show()?;
    application.activate();
    slint::run_event_loop()?;
    window.hide()?;
    if !passed.get() {
        return Err("native editor input contract failed".into());
    }
    if !single_undo.get() && std::env::args().any(|arg| arg == "--require-single-undo") {
        return Err("native selection replacement requires two undo steps".into());
    }
    Ok(())
}
