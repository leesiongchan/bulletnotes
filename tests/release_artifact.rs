#![cfg(target_os = "macos")]
use std::{
    path::{Path, PathBuf},
    process::{Command, Output},
};
fn fixture(minimum: &str, signed: bool, resources: bool) -> (tempfile::TempDir, PathBuf) {
    fixture_for_architecture("arm64", minimum, signed, resources)
}
fn fixture_for_architecture(
    architecture: &str,
    minimum: &str,
    signed: bool,
    resources: bool,
) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let app = dir.path().join("Generated Artifact.app");
    let mac = app.join("Contents/MacOS");
    std::fs::create_dir_all(&mac).unwrap();
    let source = dir.path().join("main.c");
    std::fs::write(
        &source,
        b"#include <stdio.h>\nint main(void){puts(\"installation_check: true\");return 0;}\n",
    )
    .unwrap();
    let status = Command::new("xcrun")
        .args([
            "clang",
            "-target",
            &format!("{architecture}-apple-macos{minimum}"),
        ])
        .arg(&source)
        .arg("-o")
        .arg(mac.join("bulletnotes"))
        .status()
        .unwrap();
    assert!(status.success());
    let plist = String::from(
        r#"<?xml version="1.0" encoding="UTF-8"?><plist version="1.0"><dict>
 <key>CFBundleIdentifier</key><string>lol.harflix.bulletnotes</string>
 <key>CFBundleExecutable</key><string>bulletnotes</string>
 <key>CFBundlePackageType</key><string>APPL</string>
 <key>CFBundleShortVersionString</key><string>0.1.0</string>
 <key>LSMinimumSystemVersion</key><string>14.0</string>
 <key>BulletnotesDistributionKind</key><string>development</string>
 </dict></plist>"#,
    );
    std::fs::write(app.join("Contents/Info.plist"), plist).unwrap();
    if resources {
        let path = app.join("Contents/Resources");
        std::fs::create_dir(&path).unwrap();
        for name in ["ThirdPartyNotices.html", "Slint-License.md"] {
            std::fs::write(path.join(name), b"generated attribution fixture").unwrap();
        }
    }
    if !signed {
        let _ = Command::new("codesign")
            .arg("--remove-signature")
            .arg(mac.join("bulletnotes"))
            .output();
    }
    if signed {
        let status = Command::new("codesign")
            .args([
                "--force",
                "--sign",
                "-",
                "--options",
                "runtime",
                "--timestamp=none",
            ])
            .arg(&app)
            .status()
            .unwrap();
        assert!(status.success());
    }
    (dir, app)
}
fn verify(app: &Path, development: bool) -> Output {
    let mut command =
        Command::new(Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/verify-release.sh"));
    if development {
        command.arg("--development");
    }
    command.arg(app).output().unwrap()
}
fn reason(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).to_string()
}
#[test]
fn verifier_checks_real_macho_artifacts_and_rejects_incomplete_distribution() {
    let (_dir, unsigned) = fixture("14.0", false, true);
    let output = verify(&unsigned, true);
    assert!(!output.status.success());
    assert!(reason(&output).contains("signature"), "{}", reason(&output));
    let (_dir, wrong) = fixture("15.0", true, true);
    let output = verify(&wrong, true);
    assert!(!output.status.success());
    assert!(
        reason(&output).contains("deployment target"),
        "{}",
        reason(&output)
    );
    let (_dir, missing) = fixture("14.0", true, false);
    let output = verify(&missing, true);
    assert!(!output.status.success());
    assert!(reason(&output).contains("resource"), "{}", reason(&output));
    let (_dir, valid) = fixture("14.0", true, true);
    let output = verify(&valid, true);
    assert!(output.status.success(), "{}", reason(&output));
    let output = verify(&valid, false);
    assert!(!output.status.success());
    let staple = Command::new("xcrun")
        .args(["stapler", "validate"])
        .arg(&valid)
        .output()
        .unwrap();
    assert!(
        !staple.status.success(),
        "ad-hoc fixture cannot have a production notarization staple"
    );
}

fn verify_static(app: &Path) -> Output {
    Command::new(Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/verify-release.sh"))
        .args(["--development", "--static-only"])
        .arg(app)
        .output()
        .unwrap()
}
#[test]
fn verifier_rejects_intel_even_with_valid_metadata_and_signature() {
    let (_dir, app) = fixture_for_architecture("x86_64", "14.0", true, true);
    let output = verify_static(&app);
    assert!(
        !output.status.success(),
        "Intel artifact was accepted: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        reason(&output).contains("Apple Silicon"),
        "{}",
        reason(&output)
    );
}
#[test]
fn verifier_rejects_universal_bundles_with_an_intel_slice() {
    let (_arm_dir, arm) = fixture("14.0", true, true);
    let (_intel_dir, intel) = fixture_for_architecture("x86_64", "14.0", true, true);
    let binary = arm.join("Contents/MacOS/bulletnotes");
    let combined = arm.parent().unwrap().join("universal");
    let status = Command::new("xcrun")
        .args(["lipo", "-create"])
        .arg(&binary)
        .arg(intel.join("Contents/MacOS/bulletnotes"))
        .arg("-output")
        .arg(&combined)
        .status()
        .unwrap();
    assert!(status.success());
    std::fs::rename(combined, &binary).unwrap();
    let status = Command::new("codesign")
        .args([
            "--force",
            "--sign",
            "-",
            "--options",
            "runtime",
            "--timestamp=none",
        ])
        .arg(&arm)
        .status()
        .unwrap();
    assert!(status.success());
    let output = verify_static(&arm);
    assert!(!output.status.success());
    assert!(
        reason(&output).contains("Apple Silicon"),
        "{}",
        reason(&output)
    );
}
