//! Read-only validation through Slint's supported handle and public AppKit APIs.
pub use super::integration::{LifecycleAction, MacIntegration};
use crate::domain::error::PlatformError;
use objc2::MainThreadMarker;
use objc2_app_kit::{NSView, NSWindowStyleMask, NSWindowTitleVisibility};
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
    pub has_native_titlebar: bool,
    pub can_become_key_window: bool,
    pub native_window_level: isize,
    pub window_alpha: f64,
    pub native_resizable: bool,
    pub fullsize_content: bool,
    pub title_hidden: bool,
    pub transparent_titlebar: bool,
    pub resizing: Observation,
    pub dragging: Observation,
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
    /// Inline accents and native blur are excluded by user-approved revisions A and B.
    pub fn passed(&self) -> bool {
        [
            self.resizing,
            self.dragging,
            self.focus_restoration,
            self.pinning,
            self.activation,
            self.composition,
            self.undo_redo,
            self.selection,
            self.wrapped_scrolling,
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
        has_native_titlebar: native_window
            .styleMask()
            .contains(NSWindowStyleMask::Titled),
        can_become_key_window: native_window.canBecomeKeyWindow(),
        native_window_level: native_window.level(),
        window_alpha: native_window.alphaValue(),
        native_resizable: native_window
            .styleMask()
            .contains(NSWindowStyleMask::Resizable),
        fullsize_content: native_window
            .styleMask()
            .contains(NSWindowStyleMask::FullSizeContentView),
        title_hidden: native_window.titleVisibility() == NSWindowTitleVisibility::Hidden,
        transparent_titlebar: native_window.titlebarAppearsTransparent(),
        resizing: Observation::Unobserved,
        dragging: Observation::Unobserved,
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

#[cfg(test)]
mod tests {
    use super::{NativeProbeReport, Observation};

    // Synthetic observations exercise the gate policy only, not native behavior.
    fn otherwise_verified_report() -> NativeProbeReport {
        NativeProbeReport {
            os_version: "test fixture".into(),
            window_number: 0,
            has_native_titlebar: true,
            can_become_key_window: true,
            native_window_level: 0,
            window_alpha: 1.0,
            native_resizable: true,
            fullsize_content: true,
            title_hidden: true,
            transparent_titlebar: true,
            resizing: Observation::Passed,
            dragging: Observation::Passed,
            material_composition: Observation::Passed,
            focus_restoration: Observation::Passed,
            pinning: Observation::Passed,
            activation: Observation::Passed,
            composition: Observation::Passed,
            undo_redo: Observation::Passed,
            selection: Observation::Passed,
            wrapped_scrolling: Observation::Passed,
            editable_accents: Observation::Failed,
            voiceover: Observation::Passed,
        }
    }

    #[test]
    fn approved_inline_accent_deferral_does_not_block_verified_native_gate() {
        assert!(otherwise_verified_report().passed());
    }

    #[test]
    fn approved_material_deferral_does_not_block_verified_native_gate() {
        let mut report = otherwise_verified_report();
        report.material_composition = Observation::Failed;
        assert!(report.passed());
    }

    #[test]
    fn unobserved_voiceover_still_blocks_native_gate() {
        let mut report = otherwise_verified_report();
        report.voiceover = Observation::Unobserved;
        assert!(!report.passed());
    }

    #[test]
    fn unobserved_composition_still_blocks_native_gate() {
        let mut report = otherwise_verified_report();
        report.composition = Observation::Unobserved;
        assert!(!report.passed());
    }
    #[test]
    fn observed_resize_failure_blocks_native_gate() {
        let mut report = otherwise_verified_report();
        report.resizing = Observation::Failed;
        assert!(!report.passed());
    }
    #[test]
    fn unobserved_dragging_blocks_native_gate() {
        let mut report = otherwise_verified_report();
        report.dragging = Observation::Unobserved;
        assert!(!report.passed());
    }
}
