//! Read-only validation through Slint's supported handle and public AppKit APIs.
use crate::domain::error::PlatformError;
use objc2::MainThreadMarker;
use objc2_app_kit::NSView;
use objc2_foundation::NSProcessInfo;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

/// Validation states deliberately distinguish missing evidence from observed failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Observation {
    Unobserved,
    Failed,
    Passed,
}

/// A native observation record, never an inferred production capability.
#[derive(Debug)]
pub struct NativeProbeReport {
    pub os_version: String,
    pub window_number: isize,
    pub material_composition: Observation,
    pub focus_restoration: Observation,
    pub pinning: Observation,
    pub activation: Observation,
    pub composition: Observation,
    pub undo_redo: Observation,
    pub selection: Observation,
    pub wrapped_scrolling: Observation,
    pub editable_accents: Observation,
    pub voiceover: Observation,
}

impl NativeProbeReport {
    /// Every required behavior needs affirmative evidence; opening a window is insufficient.
    pub fn passed(&self) -> bool {
        [
            self.material_composition,
            self.focus_restoration,
            self.pinning,
            self.activation,
            self.composition,
            self.undo_redo,
            self.selection,
            self.wrapped_scrolling,
            self.editable_accents,
            self.voiceover,
        ]
        .iter()
        .all(|state| *state == Observation::Passed)
    }
}

/// Inspect an already shown Slint window without changing its ownership or contents.
pub fn probe_window(window: &slint::Window) -> Result<NativeProbeReport, PlatformError> {
    let _main_thread = MainThreadMarker::new().ok_or(PlatformError::WrongThread)?;
    let provider = window.window_handle();
    let handle = provider.window_handle().map_err(PlatformError::Handle)?;
    let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
        return Err(PlatformError::WindowUnavailable);
    };
    // SAFETY: AppKit's handle is an NSView. Slint retains ownership; both handle provider
    // and Slint window outlive this borrowed reference. The main-thread check above
    // prevents calling NSView on a worker. No ownership transfer or mutation occurs.
    let view = unsafe { handle.ns_view.cast::<NSView>().as_ref() };
    let native_window = view.window().ok_or(PlatformError::WindowUnavailable)?;
    Ok(NativeProbeReport {
        os_version: NSProcessInfo::processInfo()
            .operatingSystemVersionString()
            .to_string(),
        window_number: native_window.windowNumber(),
        material_composition: Observation::Unobserved,
        focus_restoration: Observation::Unobserved,
        pinning: Observation::Unobserved,
        activation: Observation::Unobserved,
        composition: Observation::Unobserved,
        undo_redo: Observation::Unobserved,
        selection: Observation::Unobserved,
        wrapped_scrolling: Observation::Unobserved,
        editable_accents: Observation::Unobserved,
        voiceover: Observation::Unobserved,
    })
}
