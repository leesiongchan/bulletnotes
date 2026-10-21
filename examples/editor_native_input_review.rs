//! Generated-text native cursor and NSTextInputClient checks; no store or network.
#[cfg(all(target_os = "macos", feature = "native-validation"))]
#[path = "support/editor_native_input.rs"]
mod editor_native_input;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(all(target_os = "macos", feature = "native-validation"))]
    return editor_native_input::run();
    #[cfg(not(all(target_os = "macos", feature = "native-validation")))]
    Err("requires macOS and --features native-validation".into())
}
