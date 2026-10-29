# Build and release desktop artifacts

The desktop build targets are macOS 14+ on Apple Silicon (ARM64), and Windows
and Linux on x64 and ARM64. Intel macOS is unsupported. Mobile, web, and embedded
targets are outside this release matrix.
Windows/Linux native execution and GPU behavior must be verified on those hosts;
a successful macOS check alone does not establish their support.

Build requirements: Python 3.12+, Rust 1.99.1, and the native platform SDK/libraries.
On macOS, install Xcode command-line tools. Linux package prerequisites are listed
in `.github/workflows/release.yml`; the workflow uses Ubuntu 24.04. On Windows,
use the MSVC toolchain. On the project's Nix setup, a missing CLI can be run through
comma. Commands use `rustup run` because an inherited `RUSTUP_TOOLCHAIN` can
override the toolchain file.

```sh
bash scripts/check.sh
python3 -m unittest discover -s tests -p 'test_package_desktop.py'
rustup run 1.99.1 cargo install cargo-about --version 0.9.2 --features cli --locked
rustup run 1.99.1 cargo install tauri-cli --version 2.12.4 --locked
python3 scripts/package-desktop.py --version 0.1.0 --target aarch64-apple-darwin --output target/packages/local
```

Packaging uses Tauri's maintained bundler around the existing Slint executable.
It does not add a Tauri runtime. Run packaging on the matching native host;
substitute one of these targets:

| OS | Rust targets | Downloads |
| --- | --- | --- |
| macOS | `aarch64-apple-darwin` | DMG and app ZIP |
| Windows | `x86_64-pc-windows-msvc`, `aarch64-pc-windows-msvc` | NSIS installer |
| Linux | `x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu` | deb and AppImage |

`--version` must be valid SemVer and equal the `Cargo.toml` version or its `0.0.0` placeholder. Output directories must be fresh; existing
artifacts are never overwritten. Each target includes SHA-256 checksums and a
verification report recording source revision, dirty state, lockfile hash,
packaged binary hashes, attribution, architecture, version, and launch evidence.
The wrapper extracts/installs each package and verifies its contents. Linux ELF
dependencies come from `dpkg-shlibdeps`; Winit/WGPU libraries loaded dynamically
are declared explicitly in deb dependencies and included in AppImages. GPU drivers
and a desktop session remain host requirements. Tauri AppImage packaging also
includes GTK support libraries even though the app uses Slint. macOS
additionally checks deployment target, required custom Info.plist metadata,
ad-hoc hardened runtime signature, empty entitlements, and the DMG's Applications
shortcut. `--static-only` checks packaged `--version` but explicitly records UI
launch as unobserved.

## CI and development releases

The **Desktop checks** workflow runs formatting, strict all-feature Clippy, Rust
tests, and packaging contract tests on all five native runners for pushes and pull
requests. On pull requests it also requires every commit to follow
[Conventional Commits](https://www.conventionalcommits.org/). Merges are
rebase-only, so each commit lands on `main` as written and feeds the version
calculation.

Releases are automatic. When **Desktop checks** passes on a push to `main`, the
**Release** workflow asks git-cliff (configured in `cliff.toml`) for the next
version since the latest `v*` tag:

- `feat` bumps the minor version.
- `fix` and `perf` bump the patch version.
- A breaking change (`!` or a `BREAKING CHANGE` footer) bumps the minor
  version while below 1.0.
- `docs`, `chore`, `ci`, `test`, `style`, `refactor` and `build` never
  release.

When a release is due, all five targets are packaged from the tested commit.
Only after every target passes are the `vX.Y.Z` tag and a GitHub Release
marked Latest created together. The release notes are the installation text
followed by the git-cliff changelog. Nothing is committed back to `main`.

`Cargo.toml` holds the placeholder version `0.0.0`; release tags are the source
of truth. `package-desktop.py` stamps `--version` into `Cargo.toml` and
`Cargo.lock` for the build and restores both afterwards. Local builds
therefore report `0.0.0`, while packaged builds report the version passed to
the packager. To download the five artifact sets without publishing, use
**Actions → Release → Run workflow**.

Release assets carry GitHub build provenance attestations:

```sh
gh attestation verify path/to/downloaded-file -R leesiongchan/bulletnotes
```

If publishing fails after the tag was created, delete the partial release and
tag with `gh release delete vX.Y.Z --cleanup-tag --yes`. Then re-run the
failed jobs of that **Release** run. The workflows require no custom secrets.
Only the publication job receives write permissions.
macOS CI uses `--static-only`; Windows/Linux perform isolated software-rendered
UI smoke checks (Xvfb on Linux). These checks do not establish production GPU
behavior. Reports distinguish observed from unobserved launch evidence.

Development DMGs need no Apple Developer Program membership. Open the DMG and drag
Bulletnotes into Applications. The app is ad-hoc signed; neither the app nor DMG
is notarized. For a trusted downloaded build blocked by Gatekeeper, try opening
it, then use **System Settings → Privacy & Security → Open Anyway** as described
in [Apple's instructions](https://support.apple.com/102445). Windows installers
are unsigned. Linux AppImages need executable permission and an Ubuntu 24.04-compatible glibc. Operating-system trust
prompts remain expected for these development artifacts.

`--verify-installation` shows generated UI briefly and exits before opening
SQLite, credentials, or HTTP. On macOS it requires a native Metal layer; on
Windows/Linux it checks a nonuniform rendered window snapshot. Normal launch
uses the real store and automatic sync. Close an older app normally before
starting a new version against the same store; the writer lock prevents
simultaneous use.

## Desktop integration

macOS preserves its existing Application Support path and Keychain identifiers.
Windows uses the user's local application data directory and native Credential
Manager. Linux uses the XDG data directory and a running Secret Service provider
(such as GNOME Keyring or KWallet); credentials never fall back to plaintext.
A missing/locked credential provider leaves local notes available and reports
credential failure when connecting.

Windows/Linux use native window decorations and file dialogs. Registered global
shortcuts work on Windows and X11; Wayland global shortcuts are currently
unavailable and the app reports that limitation. On Windows, closing hides to the
tray when it is available. On Linux, closing saves and exits, including on desktops
without a visible tray. All close paths retain the durable save barrier.

## Pinned Slint source decorations

Application highlighting uses the foreground-decoration API from a maintained Slint
1.18.1 fork. The editor also uses its experimental marker API for
source-width-preserving bullets and task boxes. `[patch.crates-io]` in `Cargo.toml`
pins the Slint crates to the `bulletnotes-1.18.1` branch of
<https://github.com/leesiongchan/slint> and `winit` to the `bulletnotes-0.30.13`
branch of <https://github.com/leesiongchan/winit>. Both branches sit on the upstream
release tags, so `git diff v1.18.1` (or `v0.30.13`) in each fork shows the whole
change. Unmodified Slint workspace crates (`vtable`, `const-field-offset`,
`i-slint-core-macros`, `i-slint-renderer-skia`, `i-slint-renderer-software`) are patched to the same commit so
each resolves once; the winit fork takes `dpi` from crates.io like the published
crate. Cargo fetches the forks normally and `Cargo.lock` records the exact commits.

For a Slint upgrade, rebase both branches onto the new upstream tags, update the
`rev` values in `Cargo.toml`, and review the fork diffs together with `Cargo.lock`.

## Production renderer

Production selects Winit + Skia + WGPU 29. macOS requires Metal and ignores
inherited renderer overrides. Windows uses DX12/Vulkan and Linux uses Vulkan;
`SLINT_BACKEND=winit-software` explicitly selects the portable software renderer
on Windows/Linux for CI or fallback. Native GPU validation remains necessary.
Slint's version-specific `unstable-wgpu-29` and `unstable-winit-030` APIs are pinned
and isolated in the platform/desktop adapters; review them when upgrading Slint.
The macOS ARM64 deployment target is 14.0.

## Distribution prerequisites

Distribution is not available merely because a development check passed. VoiceOver,
other IMEs, physical accessibility preferences, full-screen/Spaces, supported
OS/hardware coverage and the 1 MiB performance target remain open release checks.
Developer ID signing and notarization remain separate requirements. No notarized artifact or production readiness is
claimed.

The existing `package-macos.sh` notarized distribution path remains ARM64-only.
For an explicitly requested signed macOS release, supply `BULLETNOTES_SIGNING_IDENTITY` and a
preconfigured `BULLETNOTES_NOTARY_PROFILE` from the release environment/Keychain.
Do not store signing keys, Apple passwords, API keys or notary secrets in the
repository. `BULLETNOTES_BUILD_NUMBER` may be set to 1–9999. Distribution also
requires a clean committed worktree and explicit `--notarize`:

`package-macos.sh` requires `--version` to equal the `Cargo.toml` version, and
`main` and release tags hold the `0.0.0` placeholder. A numbered notarized
build therefore needs release-version support in that script first.

```sh
scripts/package-macos.sh --version 0.1.0 --output target/packages/release --notarize
scripts/verify-release.sh path/to/Bulletnotes-0.1.0-arm64-distribution.app
```

The explicit flag uploads only to Apple's notary service using the Keychain
profile. The script checks Accepted status, staples, verifies the staple and
Gatekeeper assessment, then recreates the archive. It does not publish a site,
GitHub release, or download. The strict verifier rejects development markers,
ad-hoc signatures, missing resources, wrong deployment targets, unexpected
entitlements and missing notarization. Developer ID/notary and clean-install
checks still require real release credentials and supported hardware.

## Attribution and upgrade evidence

The top-level **BN → About Bulletnotes** action displays the official AboutSlint
widget. Full Slint license text and runtime dependency notices are included in
each platform package. `scripts/generate-notices.py` uses cargo-about's native target filtering
and supplements only its known custom Slint-license omissions after matching the
upstream license grant, exact source hashes and bundled full text. Any uncovered
warning fails generation. This chooses the offered Slint desktop license branch
and assigns no license to this private application's own source.

A separate-process generated-store regression verifies committed source, exact
CRLF/CJK/emoji offline edits, conditional queued writes, custom theme assignment,
font and paper settings after reopening with the current code. The existing
migration-failure regression preserves the original store and backup; corrupt or
newer-schema stores are not reset. This does not substitute for swapping app
versions on clean macOS 14+ Apple Silicon systems. Keep the application data
directory when replacing the app. A pre-migration SQLite backup is retained when
schema changes require it; older binaries can reject newer schemas safely.
