# Bulletnotes

A native Rust/Slint macOS quick-note development client for SilverBullet, with automatic sync, whole-space browsing, and custom themes.

**Current state: implemented development build.** Local persistence, themes, automatic guarded synchronization, compatible-edit merging and separate recovery copies are implemented. The note window uses independent floating header/footer controls. Inline Markdown coloring and blur remain deferred; exact selection/IME/VoiceOver, hardware/OS coverage, performance and distribution gates remain explicit.

See [development usage](docs/usage.md), [current automatic-sync acceptance](docs/validation/automatic-sync-acceptance.md), and the [release matrix](docs/validation/release-matrix.md). The [original approved design](docs/superpowers/specs/2026-10-17-bulletnotes-design.md), [implementation plan](docs/superpowers/plans/2026-10-17-bulletnotes.md), and [native feasibility record](docs/validation/native-gate.md) provide historical context.

The following disposable examples preserve the earlier feasibility investigations.

On an interactive macOS host, `cargo run --example native_window_probe -- --auto-close` opens the comparison window. The evidence document lists the source-preservation test, intentionally failing acceptance gate, renderer selectors, and isolated compiler probes.

Add `--frameless` to the native window example to view the disposable title-bar-free variant. It is a feasibility probe, not the finished Antinote-style chrome.

The disposable native-material probe currently fails actual compositor acceptance on both tested renderers (the effect covers the note). `--material` requires `--features native-validation`; always use `--auto-close` for this known-failing diagnostic. It is excluded from the normal app path. Use `rustup run 1.99.1 cargo ...` when the session sets a different `RUSTUP_TOOLCHAIN` override.
