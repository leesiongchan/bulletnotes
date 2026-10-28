# Bulletnotes

A native Rust/Slint macOS quick-note development client for SilverBullet, with automatic sync, whole-space browsing, and custom themes.

**Current state: implemented development build.** Local persistence, themes, automatic guarded synchronization, compatible-edit merging and separate recovery copies are implemented. The note window uses Antinote-style circular header controls and an independent capsule footer, with a durable synced scratchpad stack under `Quick Notes/`. Visible Markdown source has theme-aware heading weights/colors, bold, italics, strikethrough, muted punctuation, underlined link destinations, and code backgrounds. List/task prefixes paint bullets and clickable boxes through a pinned Slint extension. Source editing commands use one-step undo/redo; search shows literal, match-centered highlighted previews. Blur remains deferred. Exact selection/IME/VoiceOver, hardware/OS coverage, performance and distribution gates remain explicit.

## Documentation

- [Development usage](docs/usage.md): features, shortcuts, sync and recovery behavior.
- [Build and artifact verification](docs/build.md): toolchain, pinned Slint patch, packaging and release prerequisites.
- [Themes](docs/themes.md): palette file format and the bundled catalog.

## Development

```sh
scripts/check.sh   # fmt, strict all-feature Clippy, and the test suite
```

Use `rustup run 1.99.1 cargo ...` when the session sets a different `RUSTUP_TOOLCHAIN` override.
