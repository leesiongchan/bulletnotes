fn main() {
    let rustc = std::process::Command::new(std::env::var_os("RUSTC").expect("Cargo compiler"))
        .arg("--version")
        .output()
        .expect("read build compiler identity");
    assert!(rustc.status.success(), "read build compiler identity");
    let rustc = std::str::from_utf8(&rustc.stdout).expect("compiler identity is UTF-8");
    println!("cargo:rustc-env=BULLETNOTES_BUILD_RUSTC={}", rustc.trim());
    println!("cargo:rerun-if-env-changed=BULLETNOTES_SLINT_PROBE");
    // Development-only compile experiments; neither is a fallback editor.
    if std::env::var_os("CARGO_FEATURE_NATIVE_VALIDATION").is_some() {
        let probe = match std::env::var("BULLETNOTES_SLINT_PROBE").as_deref() {
            Ok("styled-input") => Some("ui/probes/styled-input.slint"),
            Ok("text-format") => Some("ui/probes/text-format.slint"),
            _ => None,
        };
        if let Some(probe) = probe {
            slint_build::compile(probe).expect("native input capability probe");
        }
    }
    slint_build::compile("ui/app-window.slint").expect("compile native validation UI");
}
