# Bulletnotes

A planned native Rust/Slint macOS quick-note client for SilverBullet, with automatic sync, whole-space browsing, and custom themes.

**Current state: native feasibility harness.** The user accepted plain-text editing with inline heading/link colors deferred. The remaining native gate has not passed; persistence, themes, and server synchronization have not been implemented. This build must not be used for storing notes.

See the [approved design](docs/superpowers/specs/2026-10-17-bulletnotes-design.md), [implementation plan](docs/superpowers/plans/2026-10-17-bulletnotes.md), and [native validation evidence](docs/validation/native-gate.md).

On an interactive macOS host, `cargo run --example native_window_probe -- --auto-close` opens the comparison window. The evidence document lists the source-preservation test, intentionally failing acceptance gate, renderer selectors, and isolated compiler probes.

Add `--frameless` to the native window example to view the disposable title-bar-free variant. It is a feasibility probe, not the finished Antinote-style chrome.

The disposable native-material probe currently fails actual compositor acceptance on both tested renderers (the effect covers the note). `--material` requires `--features native-validation`; always use `--auto-close` for this known-failing diagnostic. It is excluded from the normal app path. Use `rustup run 1.99.1 cargo ...` when the session sets a different `RUSTUP_TOOLCHAIN` override.
