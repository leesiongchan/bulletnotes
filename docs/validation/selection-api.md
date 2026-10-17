# Selection state: deeper investigation

Investigated 2026-10-17 on the current macOS arm64 host, Rust 1.99.1, Slint 1.18.1, winit/FemtoVG, AccessKit macOS 0.26.3. Production code and requirements remain unchanged. The earlier proposal to defer all cross-page/relaunch caret restoration was premature: a public macOS range-reading path exists and is now demonstrated in disposable native examples.

## What the user would notice

Caret persistence means returning to the same typing position after changing notes or relaunching. Selection persistence additionally means restoring highlighted text and which end is active. Select left-to-right or right-to-left and the highlight can look identical, but Shift+Arrow extends a different end. Exact `anchor`/`focus` restoration in the approved plan includes that direction.

This is an editor-state integration gap. It does not imply loss of Markdown bytes or inability to implement themes, resizing, durable saves, or SilverBullet sync. The source-preservation checks below remained exact.

## Direct Slint API and upstream status

The [public TextInput reference](https://docs.slint.dev/latest/docs/slint/reference/keyboard-input/textinput/) documents `set-selection-offsets(anchor, focus)` and a geometric `cursor-position-changed(position: Point)` callback. The pinned compiler marks its two offset properties internal and test-only; the application does not use them. Inspection of upstream master still found that restriction. The [release list](https://github.com/slint-ui/slint/releases/tag/v1.18.1) identifies 1.18.1 as the latest stable release at investigation time.

This is a known upstream gap. [Issue 3949](https://github.com/slint-ui/slint/issues/3949) requests reading/writing selection, and a maintainer [explicitly confirms that the setter landed while the getter remains missing](https://github.com/slint-ui/slint/issues/3949#issuecomment-1929672899). [Issue 11701](https://github.com/slint-ui/slint/issues/11701) discusses a first-class cursor abstraction; a maintainer prefers an opaque cursor type with cursor geometry operations, and the proposal remains under API design. [PR 6481](https://github.com/slint-ui/slint/pull/6481), proposing a selection callback, closed without merging. The 1.19 API review mentions `has-selection`; that boolean cannot recover endpoints.

A small amount of existing internal state already contains the information. That does not make a wrapper around test-only properties a supported production API, nor establish that an upstream change will be accepted or released promptly.

## Public macOS route: actual native results

`examples/selection_probe.rs` uses the existing AppWindow and its actual editor. It traverses only that process's own NSView accessibility children, finds the editor through the public `accessibilityTitle` mapping, and calls public NSAccessibility range methods. No system-wide AX client, accessibility permission prompt, clipboard operation, keyboard monitor, private field, or replacement editor is involved. The initial fixture mistakenly searched `accessibilityLabel`; AccessKit's macOS mapping uses `accessibilityTitle`. After correcting that fixture error, the probe observed:

| Programmed state | Public UTF-16 range | Insertion line |
| --- | --- | --- |
| Forward emoji selection | `(7, 7)` | 0 |
| Same selection backwards | `(7, 7)` | 0 |
| Caret at its start | `(7, 0)` | 0 |

The native source retained its original CRLF, accented characters, emoji, and CJK bytes. This proves public range reading is available. It does not prove production capture/restore across page changes or relaunch.

`examples/selection_geometry_probe.rs` isolates the documented TextInput primitive and its public cursor-point callback. It covers 19 generated cases: seven collapsed carets and twelve directional selections across emoji, CJK, CRLF, multiple lines, RTL text, zero-width space, wrapping, and document end.

| Measurement | Result | Limit |
| --- | --- | --- |
| Settled public accessibility ranges | 19/19 match | Expected offsets account for the AX text's CRLF normalization |
| Cursor point → public AX character hit-test | 13/19 match exact focus | Fails at some line boundaries, RTL starts, and zero-width boundaries |
| Immediate read after selection change | Prior range observed repeatedly | AX state updates asynchronously; a synchronous navigation snapshot can be stale |
| Stored/source text bytes | Exact in all cases | Only the accessibility representation was normalized |
| Strict exact-focus acceptance | Exits 1 as expected | Research evidence of a remaining gap, not a passing native editor gate |

The accessibility document text obtained through `accessibilityStringForRange` equalled the generated source with CRLF changed to LF. Its indices are UTF-16 indices into that representation, not the original UTF-8 Markdown. The probe compares against normalized expected indices; a production bridge still needs validated conversion back into the original buffer, plus a reliable freshness strategy. Waiting 150 ms in this diagnostic is not such a strategy or a latency guarantee. IME composition and real editing were not tested by these programmed selections.

A concrete ambiguity defeats the tested geometry reconstruction: selecting the zero-width space forward and backward produces the same AX range `(41, 1)` and the same cursor point `(38.061523, 72.38281)`, while the expected active UTF-16 offsets differ (42 versus 41). Those public observations cannot distinguish the two states. Other hit-test failures alone might be fixable; this identical-observation case cannot be solved by pixel rounding.

[AccessKit's pinned macOS implementation](https://github.com/AccessKit/accesskit/blob/c88605b96d04431f9c3c792464a0f2f253480e94/platforms/macos/src/node.rs) returns a normalized text range. Its [conversion helper](https://github.com/AccessKit/accesskit/blob/c88605b96d04431f9c3c792464a0f2f253480e94/platforms/macos/src/util.rs) converts the range's start/end to NSRange. The original anchor/focus orientation is not returned through that API. Winit's `NSTextInputClient.selectedRange` implementation returns NSNotFound; it is not an alternative selection getter.

## Public native-editor alternative

Apple documents [NSTextSelection.affinity](https://developer.apple.com/documentation/appkit/nstextselection/affinity-swift.property) as representing caret placement at an empty selection and the logical direction of the active edge for a nonempty selection. [NSTextLayoutManager.textSelections](https://developer.apple.com/documentation/appkit/nstextlayoutmanager/textselections) exposes the selection state, and [NSTextView.textLayoutManager](https://developer.apple.com/documentation/appkit/nstextview/textlayoutmanager) exposes that text layout manager. These are public text-editor APIs, rather than attempting to recover state from accessibility output.

This makes TextKit a concrete architecture candidate for a macOS-first client. It does not prove that embedding NSTextView alongside the Slint view will satisfy focus, layering, resize, IME, themes, undo, accessibility, and lifecycle requirements. Those need a separate native proof before changing the approved editor. No full editor rewrite is started by this research.

## Options against the unchanged requirement

| Approach | What it can cover | Remaining cost or gap |
| --- | --- | --- |
| Keep the current editor instantiated | Retains its own state during search, Appearance, hide/show | Already the chosen layout; manual editing/IME observations remain open |
| Retain an editor instance per page | Could retain per-page session state through public component factories | Inference, not a tested cache; requires bounded memory and cannot survive process restart |
| Public macOS accessibility bridge | Demonstrated caret/range reading | Must solve freshness and source-offset mapping; tested range/geometry APIs cannot guarantee selection direction |
| First-class upstream Slint snapshot/cursor API | Direct access to the state needed for exact history/relaunch | Cleanest fit for the current design; requires upstream API agreement and a released implementation, or explicit acceptance of maintaining a framework patch |
| AppKit editor embedded beside Slint UI | Public TextKit APIs expose selection ranges and affinity/direction | Requires a new reviewed design and native embedding proof; no implementation or exact-restoration claim is made here |

Recommendation: retain the approved exact-restoration requirement. For this macOS-first, production-grade client, evaluate a narrowly scoped public TextKit editor integration next, preserving Slint for the surrounding UI; this requires review of the earlier editor choice and an embedding proof. Keeping Slint's own input primitive instead requires a first-class upstream editor-state API, with acceptance/release timing outside our control, or explicit acceptance of maintaining a framework patch. The tested accessibility route is useful evidence but is not an exact production snapshot mechanism. There is no approved private-property workaround, fork, or editor replacement. No scope reduction is inferred from the user's request to investigate.

## Reproduce the evidence

```sh
rustup run 1.99.1 cargo build --example selection_probe --example selection_geometry_probe --features native-validation --locked
SLINT_BACKEND=winit-femtovg target/debug/examples/selection_probe
SLINT_BACKEND=winit-femtovg target/debug/examples/selection_geometry_probe
# Expected nonzero: the public range + cursor geometry approach is not exact.
SLINT_BACKEND=winit-femtovg target/debug/examples/selection_geometry_probe --require-exact-focus
```
