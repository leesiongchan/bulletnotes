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

## Styled-source follow-up

The source editor now follows the supplied Antinote reference: a consistent base
font size and line spacing, bold H1/H2 headings, regular-weight lower headings,
theme-accent heading colors, muted emphasis delimiters, true bold/italic/combined
emphasis, and muted strikethrough text. Inline link labels retain their surrounding
style; destinations are accented and underlined. Inline and fenced code use subtle
backgrounds at the base font size. Markdown punctuation stays editable, and
frontmatter remains plain. SilverBullet/CommonMark semantics are retained,
including `__bold__` rather than Antinote's underline extension.

The existing decoration model adds false-by-default `bold`, `italic`,
`strikethrough`, `underline`, and `code` flags. Parser nesting becomes disjoint
combined-style source runs. The renderer expands each validated run into existing
Parley styles, emitting every enabled style for the left preedit fragment before
the right fragment. Font-changing ranges reject split graphemes. Source-code
backgrounds reuse the existing painting path with a separate internal style that
keeps the base font size. No second buffer, source rewriting, new dependency,
storage change, or synchronization change is involved.

Reproduce the generated reference specimen using the native fixture (temporary
store, no user notes):

```sh
python3 scripts/prepare-slint.py
rustup run 1.99.1 cargo run --locked --features native-validation --example ui_review -- \
  --markdown-style --grid --capture "$PWD/target/markdown-style-after.png"
```

Use `--theme graphite` for the dark palette, `--minimum` for the minimum window,
or `--markdown-style --contract` for the palette/view/source contract.

Verification on macOS with native Winit/Skia/Metal:

- `scripts/check.sh` passed formatting, strict all-target/all-feature Clippy, and
  346 tests; 24 existing tests remain explicitly ignored.
- All 20 independent renderer tests passed, including combined typography around
  multiline IME preedit, backwards Unicode selection and undo, live style-model
  changes, split-grapheme rejection, and unchanged source-code line height.
  Probe formatting and strict all-target Clippy also passed.
- Native [before](markdown-highlighting/styled-source-before.png),
  [after](markdown-highlighting/styled-source-after.png), and
  [dark](markdown-highlighting/styled-source-dark.png) captures were visually
  inspected. Heading weight, emphasis, muted syntax and strike lines change;
  base text size, line positions, paper geometry and source-prefix spacing stay
  consistent. The 400-by-440 native capture also fits the generated specimen.
- The reference specimen passed the native palette/view/source-preservation
  contract (`native_ui_contract_passed: true`). Native captures retain the
  previously recorded Slint instantiation-chain diagnostic.
- Independent review identified an empty-link-label styling gap. A failing test
  reproduced it; advancing past the opening bracket independently of label text
  fixed it. The regression passes and the reviewer confirmed the correction.

## Native AntiNote palette and task spacing

The open AntiNote 2.1.3 note used Tartan. A read-only native window capture and
pixel sampling distinguished its heading, link, syntax, and strikethrough roles.
Both applications apply macOS color conversion; preserving the original palette
components produces matching solid sRGB pixels in native captures:

| Role | Palette component | Native sRGB |
| --- | --- | --- |
| Paper | `#F3F1EB` | `#F5F4EF` |
| Ink | `#3C280A` | `#4D3509` |
| H1 | `#FF7828` | `#FF8D33` |
| H2 | `#C85A1E` | `#D46F27` |
| H3 | `#FFA050` | `#FFB062` |
| Link destination | `#8C460F` | `#9F5910` |
| Syntax / strike | `#828282` | `#949494` |

All 37 bundled AntiNote palettes now retain their separate Markdown and grid
roles. Existing theme files remain compatible through optional fields. The two
translucent built-in heading colors and translucent grids are flattened over
paper to fit the existing opaque theme format. Other palettes were not
individually compared against native AntiNote captures. The private reference
capture is not included in the repository; saved previews contain generated
Bulletnotes text only.

Tasks and bullets now use a common two-em slot. Boxes have a rounded outline and
a tick for completed tasks. This adjusts source-cluster letter spacing without
changing font metrics or source offsets. Caret/selection reveal restores raw
syntax and spacing; IME preedit follows source offsets. RTL paragraphs, unsupported
prefixes, overlapping formatting, invalid cluster advances, and insufficient
width retain raw source. Rendering verifies the shaped slot width before hiding
the prefix. Empty task-only lines retain their height.

Reproduce the native preview with `ui_review --markdown-style --grid --theme
antinote-tartan --font 15 --capture <path>`. Generated native captures:
[Tartan](markdown-highlighting/antinote-tartan-after.png),
[Mononoke](markdown-highlighting/antinote-mononoke-after.png), and
[minimum window](markdown-highlighting/antinote-minimum-after.png).
The earlier styled-source captures above show the original wide task gap.

Verification: application formatting, strict all-target/all-feature Clippy, and
348 tests passed (24 existing ignored); all 23 independent renderer tests passed,
with probe formatting and strict Clippy clean. Tests include compact task/bullet
alignment, body pointer hit testing and undo/redo, inclusive raw-prefix reveal,
backward selection, narrow fallback, task-only line height, and compact markers
around IME composition. Native captures were visually inspected and the
palette/view/source contract passed. Independent review found no blocking issue.

## Earlier editor pass

Rechecked the current editor on 21 October 2026 after the theme-configuration
update. Fresh native Tartan, Mononoke, and minimum-window captures are byte-identical
to the three generated previews linked above. Visual review found aligned task
and bullet bodies, intact ticks and emphasis, and no clipping or overlap.
Tartan's colors and heading hierarchy closely match the retained native AntiNote
reference. Editor row spacing, grid pitch, and text insets still differ from
AntiNote; this is not a claim of identical editor geometry. The open AntiNote
note now uses Sanrio, and no genuine native Mononoke comparison was captured.

Fresh verification passed 15 application highlighting/marker tests, three
editor roundtrip tests, and 23 independent renderer tests, with one explicitly
ignored native roundtrip test. Renderer strict Clippy also passed. These checks
cover pointer-to-source hit testing, raw-prefix reveal, backwards Unicode
selection, undo/redo, simulated preedit, and exact source persistence. The native
generated-source contract passed across every palette and font sizes 11/15/24.
Real OS IME and cross-page/relaunch selection remain the separate gates below.

The local development package built successfully with `scripts/package-macos.sh --version
0.1.0 --output target/packages/editor-theme-review-20261021 --development`.
Its verifier passed attribution resources, ARM64/macOS 14 metadata, hardened
ad-hoc signing, empty entitlements, and a generated-content Metal launch without
opening the user's store. Package outputs remain local under `target/`.
The archive SHA-256 matched its verification report. No production code changed
during this final pass; the running local app was left open.

The subsequent [editor geometry update](editor-geometry.md) matches the measured
row/grid spacing, insets, and marker sizes. It supersedes the spacing differences
recorded in this earlier pass; native IME validation remains incomplete.

## Scope and remaining gates

The [build instructions](../build.md#pinned-slint-source-decorations) explain the
bootstrap requirement and pinned upstream provenance. Raw Cargo commands require
preparation after a fresh checkout or cleaning the generated sources. The small
patch is local maintenance work; it is not an upstream Slint API release.

The follow-up [marker integration](markdown-markers.md) adds painted bullets and
rounded task boxes in compact source slots. Variable heading sizes, general
hidden-syntax layout, and clickable links remain unimplemented. Source-style changes invalidate shaping;
there is no paint-only optimization. Parsing is synchronous over current source;
large-note interaction performance needs separate measurement. Partial-ligature
coloring follows existing Parley behavior. The [platform matrix](markdown-cross-platform.md)
records hosted native headless results. Native OS IME/VoiceOver, Windows/Linux
window integration, and iOS/Android runtime coverage remain separate gates;
headless events are not a substitute for those device checks. Existing release
limitations still apply.

Primary references: [Slint TextInput](https://docs.slint.dev/latest/docs/slint/reference/keyboard-input/textinput/),
[pulldown-cmark releases](https://github.com/pulldown-cmark/pulldown-cmark/releases).
