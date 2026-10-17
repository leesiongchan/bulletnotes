//! Disposable public in-process NSAccessibility helpers; only owned test windows.
use objc2::{msg_send, rc::Retained, runtime::AnyObject, sel};
use objc2_foundation::{NSArray, NSRange, NSString};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
pub fn supports(object: &AnyObject, selector: objc2::runtime::Sel) -> bool {
    // SAFETY: All objects are retained AppKit accessibility objects in this process.
    unsafe { msg_send![object, respondsToSelector: selector] }
}
pub fn find_editor(object: &AnyObject, depth: usize) -> Option<Retained<AnyObject>> {
    if depth > 24 {
        return None;
    }
    if supports(object, sel!(accessibilityTitle)) {
        // SAFETY: Checked selector; Apple's protocol returns an optional NSString.
        let label: Option<Retained<NSString>> = unsafe { msg_send![object, accessibilityTitle] };
        if label
            .as_ref()
            .is_some_and(|label| label.to_string() == "Markdown note")
        {
            // SAFETY: The accessibility object is alive for this borrow; retain owns it.
            return unsafe { Retained::retain(object as *const _ as *mut _) };
        }
    }
    if supports(object, sel!(accessibilityChildren)) {
        // SAFETY: Checked public selector; traverse only this window's own AX children.
        let children: Option<Retained<NSArray<AnyObject>>> =
            unsafe { msg_send![object, accessibilityChildren] };
        if let Some(children) = children {
            for child in children.iter().take(512) {
                if let Some(editor) = find_editor(&child, depth + 1) {
                    return Some(editor);
                }
            }
        }
    }
    None
}
pub fn root(window: &slint::Window) -> Result<Retained<AnyObject>, Box<dyn std::error::Error>> {
    let provider = window.window_handle();
    let handle = provider.window_handle()?;
    let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
        return Err("AppKit required".into());
    };
    // SAFETY: Supported raw-window-handle NSView, retained while the Slint window lives.
    unsafe { Retained::retain(handle.ns_view.cast::<AnyObject>().as_ptr()) }
        .ok_or_else(|| "view unavailable".into())
}
pub fn selection(window: &slint::Window) -> Result<(NSRange, isize), Box<dyn std::error::Error>> {
    let view = root(window)?;
    let editor = find_editor(&view, 0).ok_or("editor absent from public AX tree")?;
    if !supports(&editor, sel!(accessibilitySelectedTextRange)) {
        return Err("AX selected range unavailable".into());
    }
    // SAFETY: Checked public NSAccessibility selectors on this retained editor object.
    let range: NSRange = unsafe { msg_send![&*editor, accessibilitySelectedTextRange] };
    let line: isize = unsafe { msg_send![&*editor, accessibilityInsertionPointLineNumber] };
    Ok((range, line))
}

/// Hit-test a Slint top-origin logical point using the public macOS accessibility API.
pub fn offset_at_point(
    window: &slint::Window,
    x: f32,
    y: f32,
) -> Result<usize, Box<dyn std::error::Error>> {
    use objc2_app_kit::NSView;
    use objc2_foundation::NSPoint;
    let root = root(window)?;
    let editor = find_editor(&root, 0).ok_or("editor absent")?;
    if !supports(&editor, sel!(accessibilityRangeForPosition:)) {
        return Err("AX range-for-position unavailable".into());
    }
    // SAFETY: root() retains the supported raw-window-handle NSView of this live window.
    let view = unsafe { &*(Retained::as_ptr(&root).cast::<NSView>()) };
    let native = view.window().ok_or("window absent")?;
    let point = NSPoint::new(
        f64::from(x),
        if view.isFlipped() {
            f64::from(y)
        } else {
            view.bounds().size.height - f64::from(y)
        },
    );
    let screen = native.convertPointToScreen(view.convertPoint_toView(point, None));
    // SAFETY: Public selector checked above; AppKit's documented argument/return ABI.
    let range: NSRange = unsafe { msg_send![&*editor, accessibilityRangeForPosition: screen] };
    Ok(range.location)
}

pub fn document_text(window: &slint::Window) -> Result<String, Box<dyn std::error::Error>> {
    let root = root(window)?;
    let editor = find_editor(&root, 0).ok_or("editor absent")?;
    if !supports(&editor, sel!(accessibilityNumberOfCharacters))
        || !supports(&editor, sel!(accessibilityStringForRange:))
    {
        return Err("AX document text unavailable".into());
    }
    // SAFETY: Checked public selectors on the fixture-owned accessibility object.
    let length: isize = unsafe { msg_send![&*editor, accessibilityNumberOfCharacters] };
    if !(0..=10000).contains(&length) {
        return Err("fixture AX document length invalid".into());
    }
    let text: Option<Retained<NSString>> = unsafe {
        msg_send![&*editor, accessibilityStringForRange: NSRange::new(0,length as usize)]
    };
    text.map(|text| text.to_string())
        .ok_or_else(|| "AX document text absent".into())
}
