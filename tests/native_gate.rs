use std::{
    io::Write,
    process::{Command, Stdio},
};

// This characterization catches a backend normalizing source bytes at the UI boundary.
// It does not prove IME composition or real edit correctness.
#[test]
#[ignore = "opens a native macOS window; build example native_gate first"]
fn text_input_roundtrips_unicode_and_crlf() {
    let original = "---\r\ntitle: café\r\n---\r\n\t👩🏽‍💻 日本語\r\n";
    let executable = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("examples/native_gate");
    let mut child = Command::new(executable)
        .arg("--roundtrip")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("build example native_gate before native test");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(original.as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let roundtripped = String::from_utf8(output.stdout).unwrap();
    assert_eq!(roundtripped.as_bytes(), original.as_bytes());
}

#[test]
#[ignore = "native acceptance matrix is incomplete; must fail until evidence is recorded"]
fn native_fidelity_matches_approved_design() {
    let executable = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("examples/native_gate");
    let output = Command::new(executable)
        .arg("--acceptance")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
