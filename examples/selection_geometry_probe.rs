//! Tests whether public cursor geometry disambiguates the direction lost by NSRange.
#[cfg(all(target_os = "macos", feature = "native-validation"))]
#[path = "support/selection_ax.rs"]
mod selection_ax;
#[cfg(all(target_os = "macos", feature = "native-validation"))]
slint::slint! {
    export component SelectionGeometryWindow inherits Window {
        title: "Bulletnotes generated selection probe";
        width: 280px; height: 480px;
        background: #ffffff;
        in-out property <string> text;
        out property <Point> caret-point;
        public function select(anchor: int, focus: int) { input.set-selection-offsets(anchor,focus); input.focus(); }
        input := TextInput {
            x: 20px; y: 20px; width: parent.width - 40px; height: parent.height - 40px;
            text <=> root.text; font-family: "Menlo"; font-size: 15px;
            single-line: false; wrap: word-wrap;
            accessible-label: "Markdown note";
            selection-background-color: #cce0ff; selection-foreground-color: #111111; color: #111111;
            cursor-position-changed(point) => { root.caret-point = { x: point.x + self.x, y: point.y + self.y }; }
        }
    }
}
#[cfg(all(target_os = "macos", feature = "native-validation"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use slint::ComponentHandle;
    use std::{cell::Cell, rc::Rc, time::Duration};
    let window = SelectionGeometryWindow::new()?;
    let source = "A café 👩🏽‍💻 日本語\r\nsecond line\nאבג xyz\nab\u{200b}cd\nThis long line wraps across several visual rows while preserving exact byte offsets.";
    window.set_text(source.into());
    let emoji = source.find('👩').ok_or("emoji")?;
    let cjk = source.find('日').ok_or("CJK")?;
    let second = source.find("second").ok_or("second")?;
    let rtl = source.find('א').ok_or("RTL")?;
    let zero = source.find('\u{200b}').ok_or("zero-width")?;
    let wrap = source.find("This long").ok_or("wrap")?;
    let cases = vec![
        ("emoji-forward", emoji, cjk - 1),
        ("emoji-backward", cjk - 1, emoji),
        ("caret", emoji, emoji),
        ("caret-cjk-end", second - 2, second - 2),
        ("caret-rtl-start", rtl, rtl),
        ("caret-zero-before", zero, zero),
        ("caret-zero-after", zero + 3, zero + 3),
        ("caret-wrap-start", wrap, wrap),
        ("caret-document-end", source.len(), source.len()),
        ("cjk-forward", cjk, second - 2),
        ("cjk-backward", second - 2, cjk),
        ("multiline-forward", emoji, second + 6),
        ("multiline-backward", second + 6, emoji),
        ("rtl-forward", rtl, rtl + 6),
        ("rtl-backward", rtl + 6, rtl),
        ("zero-width-forward", zero, zero + 3),
        ("zero-width-backward", zero + 3, zero),
        ("wrapped-forward", wrap, source.len()),
        ("wrapped-backward", source.len(), wrap),
    ];
    let count = cases.len();
    let ranged = Rc::new(Cell::new(0));
    let focused = Rc::new(Cell::new(0));
    {
        let weak = window.as_weak();
        slint::Timer::single_shot(Duration::from_millis(200), move || {
            if let Some(window) = weak.upgrade()
                && let Ok(root) = selection_ax::root(window.window())
            {
                let _ = selection_ax::find_editor(&root, 0);
            }
        });
    }
    for (index, (name, anchor, focus)) in cases.into_iter().enumerate() {
        let weak = window.as_weak();
        slint::Timer::single_shot(Duration::from_millis(500 + index as u64 * 260), move || {
            if let Some(window) = weak.upgrade() {
                window.invoke_select(anchor as i32, focus as i32);
                if let Ok((range, _)) = selection_ax::selection(window.window()) {
                    println!(
                        "{name} immediate_range=({}, {})",
                        range.location, range.length
                    );
                }
            }
        });
        let weak = window.as_weak();
        let ranged = ranged.clone();
        let focused = focused.clone();
        slint::Timer::single_shot(Duration::from_millis(650 + index as u64 * 260), move || {
            let check = || -> Result<(), Box<dyn std::error::Error>> {
                let window = weak.upgrade().ok_or("closed")?;
                let (range, _) = selection_ax::selection(window.window())?;
                let ax_text = selection_ax::document_text(window.window())?;
                let normalized = source.replace("\r\n", "\n");
                if ax_text != normalized {
                    return Err("AX text has differences beyond CRLF normalization".into());
                }
                let lo = source[..anchor.min(focus)]
                    .replace("\r\n", "\n")
                    .encode_utf16()
                    .count();
                let hi = source[..anchor.max(focus)]
                    .replace("\r\n", "\n")
                    .encode_utf16()
                    .count();
                let range_ok = range.location == lo && range.length == hi - lo;
                if range_ok {
                    ranged.set(ranged.get() + 1);
                }
                let point = window.get_caret_point();
                let actual =
                    selection_ax::offset_at_point(window.window(), point.x, point.y + 6.0)?;
                let expected = source[..focus].replace("\r\n", "\n").encode_utf16().count();
                let focus_ok = actual == expected;
                if focus_ok {
                    focused.set(focused.get() + 1);
                }
                println!(
                    "{name}: actual_range=({}, {}) expected_range=({lo}, {}) range_ok={range_ok} focus_ok={focus_ok} expected_focus_utf16={expected} actual_focus_utf16={actual} point=({},{})",
                    range.location,
                    range.length,
                    hi - lo,
                    point.x,
                    point.y
                );
                if window.get_text().as_bytes() != source.as_bytes() {
                    return Err("source changed".into());
                }
                Ok(())
            };
            if let Err(error) = check() {
                eprintln!("geometry {name}: {error}");
            }
            if index + 1 == count {
                println!(
                    "ranges={}/{} geometry_focus={}/{}",
                    ranged.get(),
                    count,
                    focused.get(),
                    count
                );
                let _ = slint::quit_event_loop();
            }
        });
    }
    window.run()?;
    // Direction mismatches are the intended measurement, not silently passing assertions.
    if ranged.get() != count {
        return Err("public AX range failed generated cases".into());
    }
    if std::env::args().any(|arg| arg == "--require-exact-focus") && focused.get() != count {
        return Err("public geometry cannot recover every exact selection focus".into());
    }
    Ok(())
}
#[cfg(not(all(target_os = "macos", feature = "native-validation")))]
fn main() {
    eprintln!("selection_geometry_probe requires macOS and --features native-validation");
}
