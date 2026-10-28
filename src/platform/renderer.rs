//! Metal selection and read-only verification through public Slint/AppKit APIs.
use objc2::{
    msg_send,
    rc::Retained,
    runtime::{AnyClass, AnyObject},
};
use objc2_app_kit::NSView;
use objc2_foundation::NSArray;
use std::cell::Cell;
thread_local! { static SELECTED: Cell<bool> = const { Cell::new(false) }; }
fn select_once(
    initialize: impl FnOnce() -> Result<(), slint::PlatformError>,
) -> Result<(), slint::PlatformError> {
    if objc2::MainThreadMarker::new().is_none() {
        return Err("renderer selection requires the main thread".into());
    }
    SELECTED.with(|selected| {
        if !selected.get() {
            initialize()?;
            selected.set(true);
        }
        Ok(())
    })
}

/// Select before constructing Slint components. Explicit choices override
/// inherited SLINT_BACKEND values; Metal initialization errors remain visible.
pub fn select() -> Result<(), slint::PlatformError> {
    select_once(|| {
        let mut settings = slint::wgpu_29::WGPUSettings::default();
        settings.backends = slint::wgpu_29::wgpu::Backends::METAL;
        slint::BackendSelector::new()
            .backend_name("winit".into())
            .renderer_name("skia".into())
            .require_wgpu_29(slint::wgpu_29::WGPUConfiguration::Automatic(settings))
            .select()
    })
}

/// Check the application's own view hierarchy after native rendering starts.
/// No accessibility client, other process, or private view property is involved.
pub fn uses_metal(window: &slint::Window) -> Result<bool, Box<dyn std::error::Error>> {
    let native = super::window_chrome::native_window(window)?;
    Ok(native
        .contentView()
        .is_some_and(|view| has_metal_layer(&view)))
}
fn has_metal_layer(view: &NSView) -> bool {
    // SAFETY: native_window enforces main-thread access. layer is the public
    // nullable NSView getter; Retained keeps its CALayer alive for the class query.
    let layer: Option<Retained<AnyObject>> = unsafe { msg_send![view, layer] };
    if layer.is_some_and(|layer| layer_contains_metal(&layer)) {
        return true;
    }
    view.subviews().iter().any(|child| has_metal_layer(&child))
}

fn layer_contains_metal(layer: &AnyObject) -> bool {
    if let Some(class) = AnyClass::get(c"CAMetalLayer") {
        // SAFETY: CALayer inherits NSObject and supports this public BOOL query.
        let metal: bool = unsafe { msg_send![layer, isKindOfClass: class] };
        if metal {
            return true;
        }
    }
    // SAFETY: CALayer's public sublayers getter returns a nullable NSArray of
    // CALayer objects. The hierarchy is owned by this main-thread app window.
    let children: Option<Retained<NSArray<AnyObject>>> = unsafe { msg_send![layer, sublayers] };
    children.is_some_and(|children| children.iter().any(|child| layer_contains_metal(&child)))
}
