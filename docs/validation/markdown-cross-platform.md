# Markdown editor cross-platform validation

Checked on 2026-10-20 using Rust 1.99.1, pinned Slint 1.18.1, and the maintained
source-decoration/marker patch. The scope is the standalone editor probe. The full
Bulletnotes client still has a macOS-only application shell.

## Observed results

| Platform / target | Probe source check | Strict target Clippy | Native execution |
| --- | --- | --- | --- |
| macOS ARM64 | Passed previously and exercised by host tests | Passed, including probe tests | Marker verification passed; exact source, zero edits |
| Windows x64 / `x86_64-pc-windows-msvc` | Passed | Passed natively | All 16 headless Skia tests passed on Windows Server 2025 |
| iOS ARM64 Simulator / `aarch64-apple-ios-sim` | Passed | Passed | Not run; no installed iOS Simulator runtime |
| Android ARM64 / `aarch64-linux-android` | Passed | Passed | Not run; activity bootstrap and configured SDK/NDK absent |
| Linux x64 / `x86_64-unknown-linux-gnu` | Native build passed; macOS cross-check blocked by Fontconfig | Passed natively | All 16 headless Skia tests passed on Ubuntu 24.04 |

`cargo check` and `cargo clippy` do not establish final linking, packaging, launch,
input behavior, or accessibility. In particular, Android source-check success is
not a launchable Android app. No physical mobile device was exercised.

An independent feature audit inspected the actual per-target Cargo fingerprints,
not only metadata's host/dev feature union. The patched core's `shared-parley`
feature is enabled on Windows, iOS Simulator, and Android, and their Skia renderer
was checked. The Android check therefore includes the extension rather than
silently excluding its painting modules. The Activity backend is not initialized.

A linked simulator executable also built with the same pinned manifest and SDK:

```sh
CARGO_TARGET_DIR="$PWD/target" rustup run 1.99.1 cargo build --locked --offline \
  --manifest-path prototypes/slint-source-highlighting/Cargo.toml \
  --bins --target aarch64-apple-ios-sim
```

The artifact is `target/aarch64-apple-ios-sim/debug/slint-source-highlighting-probe`.
It is an ARM64 Mach-O executable targeting the iOS simulator. This is not an iOS
application bundle; packaging, scene configuration, installation, launch, and
rendering are not established by the linked binary.

## Reproduce the source checks

From the worktree root, with the pinned Rust toolchain and ordinary host build
dependencies, prepare the private Slint copies first:

```sh
python3 scripts/prepare-slint.py
rustup target add --toolchain 1.99.1 aarch64-apple-ios-sim \
  aarch64-linux-android x86_64-unknown-linux-gnu x86_64-pc-windows-msvc
```

For each target in the table, substitute its triple below:

```sh
CARGO_TARGET_DIR="$PWD/target" rustup run 1.99.1 cargo check --locked --offline \
  --manifest-path prototypes/slint-source-highlighting/Cargo.toml \
  --lib --bins --target aarch64-apple-ios-sim
CARGO_TARGET_DIR="$PWD/target" rustup run 1.99.1 cargo clippy --locked --offline \
  --manifest-path prototypes/slint-source-highlighting/Cargo.toml \
  --lib --bins --target aarch64-apple-ios-sim -- -D warnings
```

`--offline` requires the dependency cache to be populated. It controls Cargo's
registry access, not third-party native build scripts' archive downloads. This
host had Xcode 27.0 iOS/device and simulator SDKs. Rust target standard libraries
were added for these checks. The exact logs and command exit codes remain under
`target/cross-platform-research/{results,strict-results}.json` and adjacent logs.

The checks exposed an unnecessary `mut` in the probe's renderer setup on non-macOS
platforms. The mutation is now scoped to macOS, preserving its Metal override and
using the unchanged WGPU defaults elsewhere. Host probe formatting, strict Clippy,
all 16 regression tests, and native marker verification were rerun after this fix.

### macOS-to-Linux cross-compilation blocker

The normal Linux check stopped in `yeslogic-fontconfig-sys` 6.0.1: cross-compiling
pkg-config needs a target sysroot and Fontconfig metadata. Host macOS libraries
must not be substituted for target Linux libraries.

A diagnostic retry with `RUST_FONTCONFIG_DLOPEN=on` also failed. It changes the
sys crate API while Fontique 0.11.1 remains configured for direct function imports,
producing unresolved `FcCharSetAddChar` and related imports. This is not a supported
workaround for the current feature graph. No dependency feature or source patch
was added to make the check appear green.

The hosted Ubuntu job below supplies native Fontconfig and window-system build
dependencies and passes the rendered-pixel probe tests. Cross-compiling from macOS
still requires a genuine Linux target sysroot. The ordinary desktop backend supports
Linux X11 and Wayland; its runtime prerequisites are documented by
[Slint's Winit backend](https://docs.slint.dev/latest/docs/slint/guide/backends-and-renderers/backend_winit/).

## Native Windows/Linux workflow

The repository now contains `.github/workflows/markdown-editor.yml` for native
`ubuntu-24.04` and `windows-2025` runners. It runs the standalone editor verifier
on relevant pull requests, pushes to `main`, or manual dispatch. The helper is
also available locally:

```sh
python3 scripts/check-markdown.py
```

It prepares checksum-verified Slint sources before Cargo resolution, records the
pinned compiler identity, checks probe formatting and strict Clippy, and runs the
existing 16 rendered-pixel/source/selection/undo/preedit tests. These use native
headless Skia CPU rendering, so the job needs no display server or GPU. Native
Winit window capture and OS input/accessibility checks remain separate.

The Linux job installs Fontconfig/Freetype and the enabled window-system build
dependencies plus fonts. The Windows job exposes Git Bash's GNU `patch` directory
to native Python. Both use Python 3.14.6 and Rust 1.99.1; neither changes the pinned
Slint or renderer versions. The helper rejects an inherited `CARGO_BUILD_TARGET`
so these jobs cannot silently become cross-compilation checks.

Every stage writes its command, exit status, and log under
`target/markdown-validation/`. The helper returns the first failing status and
stops subsequent stages. It resets the JSON summary and removes old owned stage
logs before each run. The workflow retains the evidence for 14 days, including
when a verification step fails. Compiler setup failures before the helper starts
remain in the GitHub job log.

The actions are pinned to the verified commits of official stable releases:
[checkout v7.0.1](https://github.com/actions/checkout/releases/tag/v7.0.1),
[setup-python v7.0.0](https://github.com/actions/setup-python/releases/tag/v7.0.0),
and [upload-artifact v7.0.2](https://github.com/actions/upload-artifact/releases/tag/v7.0.2).
The workflow grants read-only repository access and does not persist checkout
credentials. Both runner jobs continue independently if one fails.

Local verification on macOS passed actionlint 1.7.12, both helper failure tests,
and the actual shared verifier with all 16 editor tests, formatting, and strict
Clippy. The failure tests use POSIX subprocess fixtures and run on Linux in CI;
Windows runs the real native verifier. Review found no actionable issues.

The [first hosted run](https://github.com/leesiongchan/bulletnotes/actions/runs/39427569262)
passed on Ubuntu. Windows passed preparation, formatting, and strict Clippy, then
failed a visual test that assumed the undecorated Unicode fixture contained no
red pixels; the observed baseline was one pixel. The test now compares decoration
colors against the original baseline and retains exact full-snapshot restoration.
The [corrected run](https://github.com/leesiongchan/bulletnotes/actions/runs/39428882095)
passed on both runners at commit `7e2997b63a735b84fcaed8ee62f932bff0a808b7`.
Downloaded artifacts confirm all five stages exited zero and `passed` is true on
both platforms: seven coloring tests and nine marker tests passed on each.
Font-dependent geometry assertions remain intact. The Windows preparation path
also ran successfully; this does not exercise concurrent Windows bootstrapping.

## Full-client blockers found in the repository

The source audit found application work beyond the shared text renderer:

- `src/main.rs` launches the client only under macOS; other targets currently print
  that macOS is required. It also explicitly rejects Intel macOS builds.
- `src/platform/mod.rs` and `src/app/mod.rs` gate application lifecycle, native
  composition, window chrome, hotkeys, renderer selection, and related modules to
  macOS. `src/platform/renderer.rs` selects Metal for the current supported shell.
- `src/platform/credentials.rs` returns `Unavailable` for persistent Keychain
  operations outside macOS. Portable startup cannot silently substitute an
  insecure credential store. Theme-file dialogs are also macOS-specific.
- `scripts/check.sh` checks all features and targets, including review examples
  that import macOS-only modules without target guards. It cannot currently serve
  as a native Linux/Windows/mobile validation command for the full client.
- Neither manifest selects the Android Activity feature, and neither crate
  provides an initialized Android activity entry point. The desktop probe's
  `main` and explicit Winit selection do not provide Android bootstrap.
- There is no iOS or Android packaging project. The new native CI workflow
  covers the standalone editor, while existing application packaging covers
  the Apple Silicon macOS client.
- The app window has desktop minimum sizes and hover-based chrome. The editor
  uses a Menlo preference and custom scrolling without mobile safe-area or touch
  panning configuration. These need device-specific layout, font-fallback, touch,
  and keyboard checks rather than desktop screenshot assumptions.

## Next executable milestones

Exercise native Winit windows and OS input/accessibility on Windows and Linux.
The hosted tests cover headless Skia pixels, source, selection, undo, and injected
preedit events; they do not establish desktop window integration or full-client
portability.

For Android, extend the editor-only probe with the pinned Activity backend,
`cdylib` entry point, and a generated-content mobile package before introducing
storage or sync. Slint's [Android Rust setup](https://docs.slint.dev/latest/docs/slint/guide/platforms/mobile/android/)
explains the Activity initialization and SDK/NDK requirements. This host had no
configured Android SDK/NDK or `adb` on PATH.

For iOS, wrap the editor probe in an Xcode application and run it in an installed
iOS Simulator before physical-device checks. Slint's
[iOS guide](https://docs.slint.dev/latest/docs/slint/guide/platforms/mobile/ios/)
uses Winit and Skia and specifies the application/scene packaging configuration.
The simulator service was queried successfully outside the sandbox; it listed
no iOS runtime. The available visionOS runtime is not iOS validation.

On both mobile targets, validate safe-area insets, virtual-keyboard overlap,
portrait/landscape layout, touch scrolling and selection handles, composition,
clipboard, undo, screen readers, and exact source bytes. Slint exposes keyboard
and safe-area properties, but application layout and touch behavior still require
integration; see its [mobile development guide](https://docs.slint.dev/latest/docs/slint/guide/platforms/mobile/general/).

The full-client port then needs explicit platform implementations for startup,
storage paths, secure credentials, lifecycle, dialogs, and desktop/mobile chrome.
This audit does not change those contracts or claim the client is cross-platform.
