# Slint source highlighting feasibility probe

A standalone experiment extending the existing Slint `TextInput` with foreground
colors over unchanged Markdown source. The application now uses the same framework
patch; this standalone fixture retains its original fixed-color parser to exercise
the framework independently. The default mode colors headings and links while preserving visible source syntax.
The optional `--markers` mode paints bullets and task boxes over their source
prefixes, retaining the original width. The application now uses the same marker
API with a separate parser bound to its source and theme ink.

## Run

Requirements: the repository Rust toolchain (1.99.1), Python 3.12+, `patch`, and
the normal native Slint/Skia build dependencies. From the repository root:

```sh
python3 prototypes/slint-source-highlighting/prepare.py

CARGO_TARGET_DIR="$PWD/target" rustup run 1.99.1 cargo test --locked \
  --manifest-path prototypes/slint-source-highlighting/Cargo.toml
CARGO_TARGET_DIR="$PWD/target" rustup run 1.99.1 cargo run --locked \
  --manifest-path prototypes/slint-source-highlighting/Cargo.toml
```

For the full native headless check sequence (prepare, compiler identity,
formatting, strict Clippy, and the 16 tests), run `python3 scripts/check-markdown.py`
from the root. Logs and a JSON status summary are written under
`target/markdown-validation/`. The [native Windows/Linux workflow](../../.github/workflows/markdown-editor.yml)
uses this same command. It is configured but has not yet run on hosted runners;
see the [platform matrix](../../docs/validation/markdown-cross-platform.md).

The preparation script verifies the three Slint 1.18.1 archives against the committed
`patches/slint/sources.json`, extracts private copies under `target/slint-source-highlighting`, and
applies `patches/slint/slint-1.18.1.patch`. It never modifies the global Cargo cache. The probe
has its own Cargo workspace, lockfile, and dependency overrides. Re-run preparation
after changing the patch; remove its generated `crates` directory to force a fresh
extraction. Cargo caches need to be populated before using `--offline`.

For a native before/after capture, append `-- --verify` to the run command. It
writes `target/source-highlighting-research/{before,after}.png`, checks exact
source bytes and zero edit callbacks, and closes the window. The interactive
version recomputes ranges synchronously after edits, including undo/redo.

To exercise the marker probe interactively, append `-- --markers`. Append
`-- --markers --verify` to capture `markers-before.png`, `markers-after.png`, and
`markers-revealed.png` in the same output directory. These compare colored raw
source, rendered markers, and raw syntax revealed by moving the caret into a task
prefix. See [marker evidence](../../docs/validation/markdown-markers.md).

## Experimental contract

`TextInput.text-decorations` accepts a model of `TextInputDecoration` values with
`start`, `end`, and `foreground` fields. Offsets address half-open UTF-8 byte ranges
in the current committed source, including Markdown punctuation. Submit ranges in
ascending, non-overlapping order. Invalid bounds, empty ranges, split UTF-8 code
points, overlaps, and out-of-order entries are ignored; accepted earlier ranges
win. Recompute ranges when the source changes. Password inputs ignore decorations.

The patch maps committed-source offsets around transient IME preedit without
coloring the preedit itself. Selection foreground overrides decorations. Existing
Slint input handling continues to own selection, editing, and undo. Model mutations
are tracked by the shaping cache. Changing colors invalidates shaping; this is not
a separate paint-only overlay. The Parley path clips global ranges into each
paragraph before applying existing color styles.

`pulldown-cmark` 0.13.4 provides original-source offsets. The sample styles heading
and link ranges, ignores fenced code, and gives a heading precedence over nested
links. No Markdown serialization or source replacement is involved.

## Verification and limits

Verified on macOS with Rust 1.99.1:

- Preparation applied the patch to fresh checksum-verified crate copies.
- All 16 probe tests passed (seven coloring tests and nine marker tests); probe
  formatting and strict Clippy passed.
- `scripts/check.sh` passed: 341 tests passed, 24 ignored.
- The native Winit/Skia/Metal capture preserved exact source bytes with zero edit
  callbacks. Visually inspected [before](evidence/before.png) and
  [after](evidence/after.png): headings and links acquire color at unchanged
  positions; task and list markers remain literal source.
- Independent review found no blocking correctness issues.

The regression suite exercises rendered pixels with headless Skia, live model
mutation, invalid ranges, selection foreground, exact Unicode/CRLF source bytes,
directed selection, width preservation, undo/redo, and active preedit. Internal
state access in the fixture is test instrumentation, not a proposed application API.

This probe targets the shared Parley/Skia path and has a native macOS verification
command. [Cross-target checks](../../docs/validation/markdown-cross-platform.md)
now pass for Windows, iOS Simulator, and Android; Linux cross-compilation is
blocked by Fontconfig setup. Native execution on those platforms remains unverified; existing platform IME and accessibility limitations remain.
Other renderer paths are not validated. There is no claim of complete CommonMark
rendering, hidden syntax, inline checkbox widgets, per-span font metrics, mobile
packaging, or upstream acceptance. Partial-ligature coloring follows Parley's
existing behavior and is not independently guaranteed for part of one glyph.

The application maintains this compact local patch; upstream acceptance remains
unconfirmed. See [application integration evidence](../../docs/validation/markdown-highlighting.md).
The marker mode proves source-width-preserving painting only. Compact AntiNote-like
spacing, clickable task boxes, and heading sizing
require separate work.
