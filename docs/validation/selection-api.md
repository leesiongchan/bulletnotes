# Supported selection API boundary

Slint 1.18.1 is pinned. Its [public TextInput reference](https://docs.slint.dev/latest/docs/slint/reference/keyboard-input/textinput/) documents `set-selection-offsets(anchor, focus)` and `cursor-position-changed(position: Point)`, but no getter for the UTF-8 anchor and caret. A geometric cursor point cannot reliably reconstruct both selection endpoints, direction, bidi behavior, and composition state.

The installed 1.18.1 compiler source (`builtin_elements.rs`, lines 2297–2300) explicitly labels `cursor-position-byte-offset` and `anchor-position-byte-offset` as “Internal, undocumented property, only exposed for tests.” The application does not use them. The public TextInputInterface only reports whether a text input has focus.

Keeping the same editor instantiated supports retaining its own editing state during search, Appearance, and hide/show. Cross-page history and relaunch need a way to capture selection before replacing the buffer. Persisting fabricated zero offsets would not meet the approved exact-restoration requirement. A public AppKit accessibility range was considered but does not by itself preserve anchor/focus direction; no such bridge is implemented or validated.

A user decision is pending: defer exact cross-page/relaunch caret restoration, retain supported APIs and scroll restoration, or keep exact restoration required and revisit the editor approach. The approved plain Slint editor remains in place until that decision. No private API, custom renderer, or replacement editor is presumed authorized. Task 6 editor-state integration has not begun.
