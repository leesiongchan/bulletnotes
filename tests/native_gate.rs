use std::{
    io::{BufRead, BufReader, Write},
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

// Exercises actual native renderer pixels and window state, not a synthetic report.
#[test]
#[ignore = "opens native macOS material window; build native_window_probe first"]
fn native_material_keeps_ink_opaque_over_translucent_paper() {
    let executable = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("examples/native_window_probe");
    let mut child = Command::new(executable)
        .args(["--frameless", "--material", "--auto-close"])
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    let mut reader = BufReader::new(child.stdout.take().unwrap());
    let mut report = String::new();
    let window_number = loop {
        let mut line = String::new();
        assert!(
            reader.read_line(&mut line).unwrap() > 0,
            "no native window report: {report}"
        );
        report.push_str(&line);
        if let Some(number) = line.trim().strip_prefix("window_number: ") {
            break number.trim_end_matches(',').parse::<u32>().unwrap();
        }
    };
    let directory = tempfile::tempdir().unwrap();
    let capture = directory.path().join("native-material.png");
    let status = Command::new("/usr/sbin/screencapture")
        .args(["-x", "-l", &window_number.to_string()])
        .arg(&capture)
        .status()
        .unwrap();
    assert!(status.success(), "app-window capture failed");
    assert!(
        child.wait().unwrap().success(),
        "native material probe failed"
    );
    let decoder = png::Decoder::new(BufReader::new(std::fs::File::open(capture).unwrap()));
    let mut image = decoder.read_info().unwrap();
    let mut buffer = vec![0; image.output_buffer_size().unwrap()];
    let frame = image.next_frame(&mut buffer).unwrap();
    let channels = match frame.color_type {
        png::ColorType::Rgb => 3,
        png::ColorType::Rgba => 4,
        other => panic!("unexpected native screenshot format: {other:?}"),
    };
    let mut visible_paper = 0;
    let mut visible_ink = 0;
    // Ignore borders and shadows; require paper and glyphs in the window interior.
    for y in 50..frame.height.saturating_sub(50) as usize {
        for x in 50..frame.width.saturating_sub(50) as usize {
            let offset = (y * frame.width as usize + x) * channels;
            let rgb = &buffer[offset..offset + 3];
            if rgb.iter().all(|channel| *channel > 180) {
                visible_paper += 1;
            }
            if rgb.iter().all(|channel| *channel < 100) {
                visible_ink += 1;
            }
        }
    }
    assert!(
        visible_paper > 10_000,
        "native material obscures paper: {visible_paper} light pixels"
    );
    assert!(
        visible_ink > 100,
        "native material obscures ink: {visible_ink} dark pixels"
    );

    assert!(report.contains("material_view_installed: true"), "{report}");
    assert!(report.contains("window_alpha: 1.0"), "{report}");
    for field in ["translucent_paper_pixels", "opaque_ink_pixels"] {
        let value = report
            .lines()
            .find_map(|line| line.trim().strip_prefix(&format!("{field}: ")))
            .and_then(|value| value.trim_end_matches(',').parse::<usize>().ok())
            .unwrap_or(0);
        assert!(value > 100, "missing native evidence for {field}: {report}");
    }
}

#[test]
#[ignore = "opens a native window; build native_window_probe first"]
fn pinning_changes_native_level_and_restores_it() {
    let executable = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("examples/native_window_probe");
    let output = Command::new(executable)
        .args(["--frameless", "--pin-probe", "--auto-close"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report = String::from_utf8(output.stdout).unwrap();
    let levels: Vec<isize> = report
        .lines()
        .find_map(|line| line.strip_prefix("pin_levels: "))
        .unwrap_or_default()
        .split(',')
        .filter_map(|part| part.parse().ok())
        .collect();
    assert_eq!(levels.len(), 3, "missing native pin observations: {report}");
    assert!(
        levels[1] > levels[0],
        "pin did not raise native level: {levels:?}"
    );
    assert_eq!(levels[2], levels[0], "unpin did not restore native level");
    assert!(report.contains("has_native_titlebar: false"), "{report}");
}
