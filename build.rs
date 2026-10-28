fn main() {
    let rustc = std::process::Command::new(std::env::var_os("RUSTC").expect("Cargo compiler"))
        .arg("--version")
        .output()
        .expect("read build compiler identity");
    assert!(rustc.status.success(), "read build compiler identity");
    let rustc = std::str::from_utf8(&rustc.stdout).expect("compiler identity is UTF-8");
    println!("cargo:rustc-env=BULLETNOTES_BUILD_RUSTC={}", rustc.trim());
    slint_build::compile("ui/app-window.slint").expect("compile application UI");
}
