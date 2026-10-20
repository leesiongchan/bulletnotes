# Source-width-preserving Markdown markers

The note editor paints bullets, outlined task boxes, and checked boxes with a
filled center over unchanged Markdown source. The standalone probe exercises the
same maintained Slint extension with fixed colors independently of the app.

The implementation extends the existing Slint 1.18.1 TextInput/Parley painting path.
It retains the full source prefix width, so text does not move and the task box
has more following space than AntiNote's compact rendering. It adds no alternate
text buffer or source/display offset mapping. Existing editing, selection, undo,
IME, and hit testing continue to use the source layout. Boxes are not clickable.

## Reproduce

From the worktree root with the repository Rust toolchain, Python 3.12+, `patch`,
and the normal native build dependencies:

```sh
python3 scripts/prepare-slint.py
CARGO_TARGET_DIR="$PWD/target" rustup run 1.99.1 cargo test --locked \
  --manifest-path prototypes/slint-source-highlighting/Cargo.toml
CARGO_TARGET_DIR="$PWD/target" rustup run 1.99.1 cargo clippy --locked --all-targets \
  --manifest-path prototypes/slint-source-highlighting/Cargo.toml -- -D warnings
CARGO_TARGET_DIR="$PWD/target" rustup run 1.99.1 cargo run --locked \
  --manifest-path prototypes/slint-source-highlighting/Cargo.toml -- --markers --verify
scripts/check.sh
rustup run 1.99.1 cargo run --locked --all-features --example ui_review -- --contract
rustup run 1.99.1 cargo run --locked --all-features --example ui_review -- \
  --capture "$PWD/target/source-highlighting-research/markers-app-after.png"
```

Omit `--verify` to edit the generated fixture interactively. Verification writes
three PNGs under `target/source-highlighting-research`, asserts exact source bytes
and zero edit callbacks, and exits. It uses generated content without a user store.

## Experimental contract

`TextInput.text-markers` takes sorted, non-overlapping `TextInputMarker` entries:
`start` and `end` are half-open UTF-8 source byte offsets; `kind` is `bullet`,
`checkbox`, or `checked-checkbox`; `foreground` supplies the marker color.
The model defaults to empty. Password inputs ignore it.

Invalid, empty, overlapping, unsorted, multiline, transparent, or grapheme-splitting
ranges remain raw source. Existing Unicode segmentation validates grapheme
boundaries; Parley cursor boundaries provide an additional shaping check. A prefix
must resolve to one unfragmented rectangle across its entire layout. Wrapped or
bidirectionally fragmented prefixes remain raw. Geometry is validated independently
of the visible clipping band; ordinary wrapped body text keeps its existing layout.

Raw source appears when the logical caret is anywhere in the inclusive prefix
boundary, or a selection overlaps it. This uses logical position rather than
caret blink visibility. Composition inside the prefix also reveals it. Composition
before a prefix shifts only its temporary visual offsets. The original source and
source offsets stay unchanged. Model mutations repaint through Slint tracking.

The fixture parser uses pulldown-cmark 0.13.4 events and original-source offsets,
including task lists and YAML metadata. It handles unordered prefixes, nested lists,
and checked/unchecked task states; ordered task lists retain their numbering.
Code and metadata examples do not become markers. The app uses the same parsing
rules in `src/app/markdown.rs`, supplying theme ink. Its pure
`AppWindow.mark-markdown(source, theme)` callback drives a private marker model into
the retained NoteEditor. Source loading/replacement, typing, undo/redo, and theme
changes invalidate this binding without replacing the TextInput or changing any
storage, synchronization, or reconciliation path. Palette application logs the
color/marker setup at debug level; source text is not logged.

## Evidence and limits

Verified on macOS with Rust 1.99.1 and Slint 1.18.1:

- Fresh preparation applied the authoritative patch to checksum-verified sources.
- All 16 probe tests passed: seven existing color tests and nine marker tests.
  Probe formatting and strict Clippy passed.
- The marker tests cover exact Unicode/CRLF source, live model changes, unchanged
  layout metrics and caret geometry, body hit testing, editing/undo/redo, caret and
  backwards-selection reveal, preedit inside/before prefixes, invalid and wrapped
  ranges, transparent markers, and unchanged wrapped body pixels.
- Review exposed two reproduced regressions: clipped italic ink after the final
  advance, and partial combining-grapheme suppression. Both regression tests now
  pass. Clipping retains the renderer's original bounds; invalid grapheme ranges
  leave every pixel unchanged while a complete grapheme remains accepted.
- The production check script passed formatting, strict Clippy, and its test suite:
  341 tests passed, 24 existing tests ignored. Four real-AppWindow tests verify
  loaded/replaced source with zero extra edits, real task-state typing, undo/redo
  after a palette change, raw-prefix reveal, backwards Unicode selection, code
  exclusions, and rendered marker pixels using updated theme ink. Two new parser
  unit tests check exact prefix offsets, nesting/quoted/ordered task cases, CRLF,
  metadata, code, and escapes.
- Native Winit/Skia/Metal captures report `source_exact=true edits=0`:
  [raw prefixes](../../prototypes/slint-source-highlighting/evidence/markers-before.png),
  [painted markers](../../prototypes/slint-source-highlighting/evidence/markers-after.png),
  and [caret reveal](../../prototypes/slint-source-highlighting/evidence/markers-revealed.png).
  Visual inspection confirms that task/list prefixes change while text stays in
  place; moving the caret into the first task reveals its original syntax.

The actual app's native [before](markdown-markers/before.png) and
[after](markdown-markers/after.png) captures show task/list prefixes changing to
boxes and a bullet in the Paper theme's ink. Body text, heading color, and reserved
prefix positions stay in place. The native palette/font/view/source contract also
passes (`native_ui_contract_passed: true`). Independent review found no remaining
correctness findings in this integration. Both native captures emitted the same
existing Slint `long callback/instantiation chain detected` diagnostic; the before
capture reproduces it without the app marker binding.

## Marker-density diagnostic

Before adoption, a temporary real-AppWindow diagnostic exposed geometry work for
fully offscreen prefixes. The renderer now rejects fully offscreen paragraphs
before resolving marker geometry, using the same conservative padding as selection
painting. Visible prefixes still receive complete, unclipped fragment validation.

On this macOS host with the debug build and headless Skia, a generated note repeated
`- [ ] café 日本語` at three densities. After an initial snapshot, the diagnostic
averaged three snapshots with markers, then three with the marker callback disabled.
All cases retained exact source bytes. These are single-run diagnostic means:

| Task count | Before culling | After culling | Markers disabled after culling |
| --- | ---: | ---: | ---: |
| 100 | 8.43 ms | 7.71 ms | 6.85 ms |
| 1,000 | 76.04 ms | 10.29 ms | 7.05 ms |
| 5,000 | 1,592.83 ms | 20.47 ms | 7.20 ms |

Initial source assignment plus the first snapshot for 5,000 tasks still took
741 ms in this debug diagnostic. These numbers do not establish release-mode input
latency, display frame rate, or device readiness. The temporary diagnostic was
removed after measurement; it does not add a timing threshold to the test suite.

This implements a bounded painting extension, not compact Markdown layout or complete
CommonMark rendering. Suppressing source advances would require a separate design
for layout, source/display mapping, selections, IME, and accessibility. Checked
boxes use a filled center rather than a tick, and there are no independent widget
focus or accessibility nodes. The underlying TextInput still exposes source text.

Native Winit window capture has only been exercised on macOS. The follow-up
[cross-platform audit](markdown-cross-platform.md) records source checks, hosted
native headless Skia results, and remaining platform/bootstrap blockers. Mobile
runtime, Windows/Linux window integration, native OS IME, assistive technology,
and other renderer paths require their own checks. The private Slint API is not
an upstream release. Full-note parsing and marker geometry still need release-mode
interaction measurements on
representative devices. No cross-platform readiness claim follows from this probe.
