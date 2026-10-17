//! Read-only validation through Slint's supported handle and public AppKit APIs.
use crate::domain::error::PlatformError;
use objc2::MainThreadMarker;
use objc2_app_kit::{NSView, NSWindowStyleMask};
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
    /// Inline accents are excluded by the user-approved revision A.
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
    fn unobserved_materials_still_block_native_gate() {
        let mut report = otherwise_verified_report();
        report.material_composition = Observation::Unobserved;
        assert!(!report.passed());
    }

    #[test]
    fn unobserved_composition_still_blocks_native_gate() {
        let mut report = otherwise_verified_report();
        report.composition = Observation::Unobserved;
        assert!(!report.passed());
    }
}
