# Native appearance checkpoint

Captured 2026-10-17 on macOS 27.0.1 arm64 with Rust 1.99.1, Slint 1.18.1, winit/FemtoVG. These are WindowServer captures of the generated-content `ui_review` fixture's own window. No live SilverBullet pages or user preview text were read or captured. PNGs are at this display's 2× scale; sizes below are logical pixels.

These captures are a historical UI checkpoint. Native input/accessibility gates remain open; synchronization has since been integrated and is documented in [sync evidence](sync-lifecycle.md). This fixture is not a production release.

| Palette | Default 505×552 | Minimum 400×440 |
| --- | --- | --- |
| Paper | [Capture](native-ui/paper-default.png) | [Capture](native-ui/paper-minimum.png) |
| Graphite | [Capture](native-ui/graphite-default.png) | [Capture](native-ui/graphite-minimum.png) |
| Matcha | [Capture](native-ui/matcha-default.png) | [Capture](native-ui/matcha-minimum.png) |
| Rose | [Capture](native-ui/rose-default.png) | [Capture](native-ui/rose-minimum.png) |
| All pages, Paper | [Capture](native-ui/pages-default.png) | [Capture](native-ui/pages-minimum.png) |
| Appearance, Paper | [Capture](native-ui/appearance-default.png) | [Capture](native-ui/appearance-minimum.png) |

The note editor remains instantiated while another primary view is shown. All pages has literal text search and a folder field, with virtualized keyboard-navigable rows. The fixture's page rows are explicitly generated examples; the main application's local coordinator now opens stored pages and executes local search. Native frame/hover behavior uses the public-AppKit configuration tested in Preview 2; the user confirmed physical resizing and hover/drag there. The main application now adopts the public window adapter and local save/hide/quit flow; the remaining native lifecycle gates are tracked in [the local checkpoint](local-milestone.md).

Appearance provides four built-ins, independent Light/Dark assignments, System mode, four paper patterns, 11–24 point text, and native JSON import/export dialogs. Palette cards show their own colors. Long imported names elide instead of overlapping the selection indicator. Import/export remain outside the scrolling controls at minimum size. The file picker callbacks are wired; manual picker interaction and live macOS appearance notifications remain unobserved. No translucency or inline Markdown coloring is claimed under approved revisions B/A.

The native callback check selects Matcha, awaits the actual SQLite acknowledgement, injects a real SQLite settings failure, and selects Rose. It verifies rollback to Matcha, an actionable error, and exact preservation of CRLF/CJK/emoji source. The separate native view/palette check verifies exclusive primary-view visibility and byte preservation across four palettes, three text sizes, and all view states. Those checks do not establish IME, undo, selection preservation, VoiceOver, or full-screen focus behavior.

Ruled paper uses the difference between documented single-line and two-line Text preferred heights at the same font and line-height factor as TextInput. Measured line spacing is checked in the actual native fixture at 11/15/24 points. Captures: [11 points](native-ui/ruled-11.png), [15 points](native-ui/ruled-15.png), [24 points](native-ui/ruled-24.png). Rules follow wrapped visual lines and scroll with the editor. Actual editing/selection/IME at these sizes still requires human validation.

Visual review corrected missing positioned close buttons, long theme-name collisions, and selected paper-style visibility. One earlier maximum-size capture showed the editor obscuring surrounding controls. Geometry measurements subsequently matched expected bounds, and three repeated window-capture checks passed both with and without temporary geometry reads. The earlier failure has no confirmed root cause; it remains an open native stability observation rather than a claimed fix. Slint also emits `long callback/instantiation chain detected` during fixture startup; investigate before completing the native gate. A small missing-glyph box appears at the tab/emoji line in these captures; its exact cause and behavior during editing are unverified. No source bytes were changed to hide it.

Rebuild and inspect without touching the live Preview 2 bundle:

```sh
rustup run 1.99.1 cargo build --example ui_review --features native-validation --locked
SLINT_BACKEND=winit-femtovg target/debug/examples/ui_review --theme paper --capture /tmp/bulletnotes-paper.png
SLINT_BACKEND=winit-femtovg target/debug/examples/ui_review --view appearance --minimum --capture /tmp/bulletnotes-appearance.png
rustup run 1.99.1 cargo test --test ui_contract --features native-validation --locked -- --include-ignored
```

The current-note find bar was reviewed separately at minimum size in [Paper](native-ui/find-paper-minimum.png) and [Graphite](native-ui/find-graphite-minimum.png). The path, field, match navigation, editor, and footer remain visible; the extra row reduces the editor viewport. These generated presentation captures do not prove physical keyboard behavior or selection direction. The existing missing-glyph observation at the tab/emoji line is still visible and remains open.

## Accessibility and pagination presentation (2026-10-18)

Generated native minimum-size captures show the programmatically applied system
high-contrast policy in [Paper](native-ui/high-contrast-paper.png) and
[Graphite](native-ui/high-contrast-graphite.png), plus the new
[More pages footer](native-ui/pages-more-minimum.png). Text and footer controls fit
at 400×440 logical pixels. The existing tab/emoji missing-glyph observation is
still visible and remains open. These captures do not substitute for physical
VoiceOver, keyboard/IME, or toggling actual system accessibility settings.

The first pagination capture failed with WindowServer's “could not create image
from window”; its explicit repeat succeeded and was inspected. Successful image
files were checked directly; the fixture's exit status alone is insufficient
because its capture path reports an error without failing the process.
