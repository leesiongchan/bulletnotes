//! Isolated generated UI only; never opens storage, Keychain or HTTP.
#[cfg(target_os = "macos")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    bulletnotes::platform::installation::check()
}
#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("macOS required");
    std::process::exit(1);
}
