# Build and verify macOS artifacts

Bulletnotes supports **macOS 14+ on Apple Silicon only**. Intel and universal
bundles are unsupported.

Build requirements: macOS/Xcode CLI tools, Python 3.12+, `patch`, Rust 1.99.1,
the `aarch64-apple-darwin` standard library, and cargo-about
0.9.2. On the project's Nix setup, a missing CLI can be run through comma.
The toolchain file pins Rust; commands use `rustup run` explicitly because an
inherited `RUSTUP_TOOLCHAIN` can override that file. Rust 1.92.0 also passes the
locked production `cargo check`, including the final source.

```sh
scripts/check.sh
scripts/package-macos.sh --version 0.1.0 --output target/packages/local --development
scripts/verify-release.sh --development target/packages/local/Bulletnotes-0.1.0-arm64-development.app
```

The version must match Cargo.toml. Output directories must be fresh: existing
artifacts are not overwritten. The ARM64 build receives an `.app`, `.zip` and
`.verification.json`. Deployment target is 14.0 in both Info.plist and Mach-O.
The native binary and attribution resources are signed with hardened runtime and
an empty entitlement dictionary. Development mode uses an explicit ad-hoc
signature and never uploads to Apple. The verifier accepts only a thin `arm64`
Mach-O; Intel and universal binaries are rejected. `--static-only` explicitly
leaves launch unobserved; run without it on Apple Silicon to establish
launchability. Explicit Intel application builds fail at compile time.

`--verify-installation` is an explicit binary smoke path: it shows generated UI
briefly and exits before opening SQLite, credentials, or HTTP. The verifier uses
it to avoid launching against a user's store. It also requires a real native
`CAMetalLayer`; the verification report records `graphics_api: "Metal"`. Normal app launch uses the real
local store and automatic connection/synchronization. Older live previews are not
replaced or stopped by packaging. Close an older local app normally before trying
a new client that uses the same store; the writer lock prevents simultaneous use.

## Pinned Slint source decorations

Application highlighting uses the foreground-decoration API in the maintained Slint
1.18.1 patch in `patches/slint/`. The editor also uses its experimental marker API for
[source-width-preserving bullets and task boxes](validation/markdown-markers.md). Run this before direct Cargo commands on a fresh checkout, after
`cargo clean`, or after changing the patch:

```sh
python3 scripts/prepare-slint.py
```

The check, preview, lifecycle, notice-generation, and packaging scripts run it
automatically. Preparation uses cached crate archives when their SHA256 matches
`patches/slint/sources.json`; otherwise it downloads the exact archives from
`static.crates.io`, verifies them, and patches private copies under
`target/slint-source-highlighting/crates`. It does not modify Cargo's cache.
Preparation has a per-project OS lock to protect simultaneous first builds.
Cargo cannot bootstrap absent path patches itself, so `cargo fetch` is not a
replacement for this step. Once prepared and dependencies cached, offline Cargo
commands work normally. The generated path stays fixed even with `CARGO_TARGET_DIR`.

The original archive checksums remain committed independently of Cargo.lock, whose
three patched entries refer to local paths. Review those checksums, the compact
patch, and both lockfiles together for a Slint upgrade; retain all upstream license
files. [Highlighting evidence](validation/markdown-highlighting.md) records the
editor contract and current verification scope.

## Production renderer

Production explicitly selects Winit + Skia + WGPU 29 with Metal as the only GPU
backend before constructing Slint components. Inherited `SLINT_BACKEND` and
`WGPU_BACKEND` settings cannot switch the application to OpenGL. Selection is
main-thread-only and idempotent. Startup uses the corresponding rendering notifier.
Slint 1.18.1's documented `unstable-wgpu-29` feature exposes a public, version-specific
API; it is pinned and isolated in `src/platform/renderer.rs`. It requires review
when upgrading Slint/WGPU. The editor uses the locally maintained source-decoration property described above;
no private selection getters or unstable winit API are enabled.
The macOS 14 deployment target remains unchanged.

[Renderer evaluation](validation/metal-renderer.md) records native checks,
performance comparisons, candidate limitations and remaining acceptance gates.
The optional `renderer-review` feature enables an explicit Skia/OpenGL comparison
path in development fixtures; production defaults do not enable it.

## Distribution prerequisites

Distribution is not available merely because a development check passed. The
current [release matrix](validation/release-matrix.md) retains unobserved manual
and hardware checks, an exact selection-capture API gate, and the 1 MiB performance
target. Developer ID and notarization credentials are not configured in this
session. No notarized artifact or production readiness is claimed.

For an explicitly requested release, supply `BULLETNOTES_SIGNING_IDENTITY` and a
preconfigured `BULLETNOTES_NOTARY_PROFILE` from the release environment/Keychain.
Do not store signing keys, Apple passwords, API keys or notary secrets in the
repository. `BULLETNOTES_BUILD_NUMBER` may be set to 1–9999. Distribution also
requires a clean committed worktree and explicit `--notarize`:

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
Resources. `scripts/generate-notices.py` uses cargo-about's native target filtering
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
