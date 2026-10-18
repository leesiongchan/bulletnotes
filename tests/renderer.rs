#![cfg(target_os = "macos")]
#[test]
#[ignore = "opens generated-data Metal renderer fixture; build renderer_review with native-validation,renderer-review first"]
fn metal_renderer_preserves_startup_themes_and_durable_edits() {
    let executable = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("examples/renderer_review");
    let output = std::process::Command::new(executable)
        .arg("production")
        .env("SLINT_BACKEND", "winit-software")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("\"metal_layer\":true"));
}
