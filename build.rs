fn main() {
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
