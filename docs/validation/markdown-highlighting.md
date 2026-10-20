# Markdown source-color integration

The actual note editor now colors CommonMark headings and links using a maintained
Slint 1.18.1 foreground-decoration extension. This supersedes the earlier inline
color deferral for this bounded milestone. The existing TextInput owns editing,
selection, undo, IME, scrolling, and accessibility. Its source property is unchanged.

A pure `AppWindow.highlight-markdown` callback derives sorted, disjoint UTF-8 source
ranges from the current source, effective palette, and high-contrast setting.
The result is a property binding into the retained NoteEditor. Typed edits,
undo/redo, direct source hydration, page replacements, remote source assignments,
and palette changes all invalidate the same binding. Existing storage, sync, and
reconciliation contracts are unchanged.

The parser uses pulldown-cmark 0.13.4 source offsets and its YAML metadata option.
Headings own their nested links. Standalone fenced/inline code, escaped heading
and link syntax outside colored ranges, and frontmatter stay plain. An entire
heading or link expression is colored, including its punctuation and nested code.
A theme's accent is used when its contrast against paper is at least 4.5:1, otherwise ink. High contrast uses ink. Selection foreground retains its
existing precedence. No palette schema fields or remote resources are added.

## Reproduction

From the worktree root, with Python 3.12+, `patch`, and the repository Rust toolchain:

```sh
python3 scripts/prepare-slint.py
scripts/check.sh
CARGO_TARGET_DIR="$PWD/target" rustup run 1.99.1 cargo test --locked \
  --manifest-path prototypes/slint-source-highlighting/Cargo.toml
python3 scripts/generate-notices.py
rustup run 1.99.1 cargo run --locked --all-features --example ui_review -- --contract
rustup run 1.99.1 cargo run --locked --all-features --example ui_review -- \
  --capture "$PWD/target/source-highlighting-research/app-after.png"
```

The native fixture uses generated content and a temporary store. Capturing its
window does not require starting the production client against a user's store.

## Observed evidence

Verified on macOS with native Winit/Skia/WGPU 29 Metal:

- Before integration, three of four initial real-editor tests failed because the
  requested colors were absent; the plain-code test passed.
- Seven real-AppWindow tests now cover loaded/replaced source and link-only notes,
  actual key input, code exclusions, frontmatter exclusions, theme/high-contrast
  changes, a backwards Unicode selection, and undo/redo after recoloring. Two
  parser unit tests check exact Unicode/CRLF offsets, nesting, and escaped/code text.
- `scripts/check.sh` passed: formatting, strict all-target/all-feature Clippy,
  335 tests passed, 24 existing tests ignored.
- All seven independent framework probe tests passed, including pixel masks around
  active IME preedit, invalid ranges, live model mutations, selection precedence,
  and exact source/undo preservation. Probe strict Clippy passed.
- Fresh bootstrap succeeded in a temporary project with no Cargo manifest and an
  empty CARGO_HOME, downloading and verifying all three pinned upstream archives.
  Concurrent fresh preparation passed three reviewer-run trials with two callers
  each, producing exactly one materialization and two successful exits per trial.
- License notice generation passed, retaining the existing exact license-byte and
  checksum verification. No changes to generated notices were necessary.
- The existing native palette/view/source-preservation contract passed:
  `native_ui_contract_passed: true`.
- Native [before](markdown-highlighting/before.png) and
  [after](markdown-highlighting/after.png) captures show the actual retained note
  editor. The heading changes from ink to the Paper theme's green accent; YAML,
  ordinary text, layout, and source punctuation remain unchanged. Link coloring
  is separately covered by a link-only rendered-pixel test.

The signed development app bundle also built successfully with
`scripts/local-preview.sh --build-only`; it was not opened against a user store.

Independent review found a concurrent-bootstrap race; the preparation script now
holds a per-project OS lock before its cache check through materialization and
stamp writing. The macOS fix was independently reproduced. The Windows lock path
uses the standard library and retries only lock-contention errors. Preparation
completed in the [hosted Windows job](markdown-cross-platform.md); concurrent
Windows bootstrap remains unverified.

## Scope and remaining gates

The [build instructions](../build.md#pinned-slint-source-decorations) explain the
bootstrap requirement and pinned upstream provenance. Raw Cargo commands require
preparation after a fresh checkout or cleaning the generated sources. The small
patch is local maintenance work; it is not an upstream Slint API release.

The follow-up [marker integration](markdown-markers.md) adds painted bullets and
task boxes while retaining prefix width. Heading font metrics, compact hidden-syntax
layout, and clickable links remain unimplemented. Color changes invalidate shaping;
there is no paint-only optimization. Parsing is synchronous over current source;
large-note interaction performance needs separate measurement. Partial-ligature
coloring follows existing Parley behavior. The [platform matrix](markdown-cross-platform.md)
records hosted native headless results. Native OS IME/VoiceOver, Windows/Linux
window integration, and iOS/Android runtime coverage remain separate gates;
headless events are not a substitute for those device checks. Existing release
limitations still apply.

Primary references: [Slint TextInput](https://docs.slint.dev/latest/docs/slint/reference/keyboard-input/textinput/),
[pulldown-cmark releases](https://github.com/pulldown-cmark/pulldown-cmark/releases).
