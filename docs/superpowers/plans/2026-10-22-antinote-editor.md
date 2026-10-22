# AntiNote Editor Interactions Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make Markdown tasks, lists, formatting, and line movement work through one native undoable edit per command.

**Architecture:** Pure Rust computes source replacements; the private Slint input applies them and owns history, composition, pointer handling, and selection. Existing `edited` bindings retain local persistence and sync. Establish composition evidence before enabling commands.

**Tech Stack:** Rust, Slint 1.18.1, existing winit 0.30.13, pulldown-cmark 0.13.4, unicode-segmentation 1.13.3.

**Spec:** [Approved design](../specs/2026-10-22-antinote-interaction-fidelity-design.md).

## Global Constraints

- "All offsets refer to UTF-8 source bytes." Preserve graphemes, CRLF pairs, and directed selection.
- "Each command creates one nonmerging history entry, including pure insertion or deletion."
- "Commands must not implement editing by assigning the app's `note-text` property."
- "While that state is unknown, decline custom commands without disturbing native input."
- "Use the same validated layout geometry for painting and hit testing."
- Preserve Slint 1.18.1 and existing dependency versions; materialize private sources through the authoritative patch, never Cargo's shared cache.
- Use generated text, isolated databases, process/window-targeted native input, and existing Rust 1.99.1 checks.

## Review Focus

- Empty preedit before commit or during an external candidate session must remain guarded: E1.
- AltGr and extra modifiers must not trigger formatting or turn rejected task toggling into all-page search: E4.
- Clicking with an offscreen caret must preserve scroll while still updating native cursor/IME/accessibility state: E5.
- A layout/source change during a checkbox press must cancel even if its rectangle is unchanged: E5.
- Final newlines and full-line selections containing structural prefixes must retain their literal contracts: E3.

---

## File Structure

- `patches/slint/slint-1.18.1.patch`: all retained private compiler/core/backend/winit changes.
- `patches/slint/sources.json`, both Cargo manifests/locks: materialize and resolve additional already-pinned backend sources.
- NEW `src/app/source_commands.rs`: command types, dispatcher, shared source context, and test-only application helper.
- NEW `src/app/source_commands/{lists,format,lines}.rs`: source transformations by responsibility; task toggling belongs with lists.
- `src/app/ui.rs`, `ui/types.slint`, `ui/app-window.slint`, `ui/components/note-editor.slint`: synchronous command bridge and key routing.
- Existing prototype harness/tests: atomic edit, composition and marker regressions.
- NEW generated native examples: composition proof and command/save/restart checks.

Generated source abbreviations below are locations inside `target/slint-source-highlighting/crates/`; retain their changes only in the authoritative patch. `CORE=i-slint-core-1.18.1`, `COMPILER=i-slint-compiler-1.18.1`, `BACKEND=i-slint-backend-winit-1.18.1`, `WINIT=winit-0.30.13`.

### Task E1: Observable Composition Guard and Native Proof

**Files:** Modify authoritative patch, `patches/slint/sources.json`, `Cargo.toml`, `Cargo.lock`, `prototypes/slint-source-highlighting/Cargo.toml`, its `Cargo.lock`, and `probe.slint`. Create `prototypes/slint-source-highlighting/tests/composition.rs` and `examples/editor_composition_review.rs`.

**Interfaces (NEW):**

```rust
// In CORE/input.rs; backend maps its identically named enum one-for-one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompositionStatus { Unknown, Idle, Active }
// In CORE/platform.rs::InternalEvent:
CompositionStatusChanged { context_id: u64, status: crate::input::CompositionStatus }
```

Add NEW `winit::event::CompositionStatus` with those variants and `Ime::CompositionStatusChanged { context_id:u64, status:CompositionStatus }` in the private winit source. Context IDs increase monotonically within a window whenever its native input context/focus ownership is replaced or invalidated. Core accepts a newer ID with its authoritative status and ignores events from older IDs; focus invalidation emits a fresh Unknown before any teardown callbacks from the old context. Native callbacks carry their originating context ID rather than relabeling old teardown events with the new ID. Core window state receives status before shortcuts/capture handlers. NEW TextInput output `source-edits-allowed: bool` is true only when enabled, writable, and status is Idle. Initialize Unknown; the backend explicitly establishes Idle for a newly initialized native context. Headless fixtures inject explicit lifecycle events rather than assume Idle. The prototype adds NEW `read-only:bool=false` and `input-enabled:bool=true` inputs bound to its TextInput, plus the guard output for tests.

- [ ] **Step 1: Pin failing lifecycle tests.** Extend the existing prototype harness to expose `source-edits-allowed`. In `composition.rs`, create named tests `empty_preedit_does_not_admit_commands`, `commit_releases_guard_after_text_change`, and `context_cancellation_reestablishes_idle`. Required assertions:

```rust
assert!(!ui.get_source_edits_allowed()); // Unknown, then Active
assert_eq!(ui.get_source(), original);  // UpdateComposition, including empty
assert_eq!(ui.get_edits(), 0);
// Synthetic empty preedit immediately before Commit must not clear the guard.
assert!(!ui.get_source_edits_allowed());
// After native commit, then explicit Idle:
assert_eq!(ui.get_source(), committed);
assert!(ui.get_source_edits_allowed());
```

Include permission/focus/context replacement: readonly stays blocked after Idle; old-context Idle cannot unlock a replacement context.

- [ ] **Step 2: Run red checks.** Run `rustup run 1.99.1 cargo test --manifest-path prototypes/slint-source-highlighting/Cargo.toml --locked --test composition`. Expect compile failure for the NEW lifecycle event/output, then behavioral failures until propagation is implemented.
- [ ] **Step 3: Implement lifecycle propagation and the proof fixture.** Patch `WINIT/src/event.rs`, native `src/platform_impl/macos/view.rs`, `BACKEND/winitwindowadapter.rs`, `CORE/{input,platform,api,window}.rs`, `CORE/items/text.rs`, and `COMPILER/builtin_elements.rs`. Emit Active when native composition begins, retain Active/Unknown across synthetic empty preedit before commit, emit Idle after actual commit/unmark/context teardown, and use Unknown for ambiguous empty marked-text updates. Do not equate Enabled/Disabled, `hasMarkedText`, or private `ImeState` alone with a complete lifecycle. Preserve candidate and native key ordering.

  Materialize BACKEND with SHA-256 `dae4fcf1b19e9cb109984d2af87d2b9485388c619265bff71dd523133888bee2` and WINIT with `a6755fa58a9f8350bd1e472d4c3fcc25f824ec358933bba33306d0b63df5978d` from the current `Cargo.lock`. Verify the official crate archive bytes against these checksums before materialization, as source preparation already does. Add root/prototype path patches; update only those existing lockfile entries at their current versions. Keep all five private packages on a single resolution.

  NEW `examples/editor_composition_review.rs::run() -> Result<(), Box<dyn std::error::Error>>` creates an owned native context with generated text and uses the actual Pinyin input source when `--pinyin` is supplied. Reuse the existing native-input fixture's source restoration and PID-targeted key pattern. Record event kinds/status/order, window/context identity and counts, not text. Exercise candidate entry, empty update, commit, Escape cancellation and context recreation. Verify ordinary input remains usable and known Idle eventually permits commands after explicit end/cancel. Unknown on ambiguous transitions is a safe result; an adapter permanently stuck Unknown is not passing acceptance.
- [ ] **Step 4: Verify green and native evidence.** Run `python3 scripts/prepare-slint.py`, rerun the composition tests, build `editor_composition_review` with `--features native-validation --locked`, then run `target/debug/examples/editor_composition_review --pinyin`. Require exit zero and NEW `native_composition_guard_passed: true`, actual input-source restoration, and trace evidence before allowing E4 activation. Core synthetic tests cannot substitute for native proof. If a native transition cannot be classified safely or recovered, record it and stop editor activation; do not weaken the guard. Other-platform adapters must supply equivalent native lifecycle evidence before claiming support; default Unknown remains safe.
- [ ] **Step 5: Commit** only E1 files as `Preserve composition lifecycle before source commands`.

### Task E2: Atomic Native Source Replacement

**Files:** Modify authoritative patch and `prototypes/slint-source-highlighting/probe.slint`; create `prototypes/slint-source-highlighting/tests/source_edits.rs`.

**Interfaces:** Consumes E1's `source-edits-allowed`. Add NEW:

```rust
// CORE/items/text.rs
pub fn apply_source_edit(self: Pin<&Self>, window_adapter: &Rc<dyn WindowAdapter>,
    self_rc: &ItemRc, start: i32, end: i32, replacement: SharedString,
    anchor_after: i32, caret_after: i32) -> bool;
```

Expose NEW `TextInput.apply-source-edit(start:int,end:int,replacement:string,anchor:int,caret:int) -> bool`. True means one source change occurred. Reject/no-op returns false without side effects. The prototype forwards it as NEW `apply-edit(...) -> bool` and consumes E1's read-only/input-enabled inputs for tests.

- [ ] **Step 1: Write failing tests.** In `source_edits.rs`, add `atomic_edit_restores_both_directed_selections`, `pure_insert_and_delete_do_not_merge_with_typing`, `rejected_edit_preserves_history_and_composition`, and `new_command_discards_redo`. Example assertions for source `aéz`, before selection `(3,1)`, replacing bytes `1..3` with `**é**`, post-selection `(5,3)`:

```rust
assert!(ui.invoke_apply_edit(1, 3, "**é**".into(), 5, 3));
assert_eq!(ui.get_source(), "a**é**z");
assert_eq!((ui.get_anchor(), ui.get_caret(), ui.get_edits()), (5, 3, 1));
ui.invoke_undo_input();
assert_eq!((ui.get_source().as_str(), ui.get_anchor(), ui.get_caret()), ("aéz", 3, 1));
ui.invoke_redo_input();
assert_eq!((ui.get_anchor(), ui.get_caret(), ui.get_edits()), (5, 3, 3));
```

For split UTF-8/grapheme/CRLF, out-of-bounds selection, disabled/readonly and Active/Unknown cases assert false and identical source/selection/preedit/undo count.

- [ ] **Step 2: Run red:** `rustup run 1.99.1 cargo test --manifest-path prototypes/slint-source-highlighting/Cargo.toml --locked --test source_edits`; expect the NEW native operation to be missing.
- [ ] **Step 3: Implement the operation.** Add a dedicated nonmerging source-edit history variant retaining removed text and pre/post selections. Validate the entire prospective source and selection before changing native state; use `set_text_internal`, normal cursor/IME/accessibility updates, then one edited callback. Preserve existing typing/paste/IME history semantics. Wire NEW compiler `BuiltinFunction::ApplySourceEdit` in `expression_tree.rs`, `builtin_elements.rs`, Rust/C++ generators and exhaustive builtin classifications, following `SetSelectionOffsets`. Its type is `(ElementReference,Int32,Int32,String,Int32,Int32)->Bool`; keep it impure and non-inlineable. Do not use clipboard simulation or external text assignment.
- [ ] **Step 4: Verify green:** reprepare sources, run `--test source_edits --test undo --test composition` in the prototype, then `python3 scripts/check-markdown.py`. Require all new and existing assertions to pass, especially exactly one edit per operation and directed redo selection.
- [ ] **Step 5: Commit** as `Add atomic source edits with directed undo and redo`.

### Task E3: Pure Markdown Source Commands

**Files:** Create `src/app/source_commands.rs`, `src/app/source_commands/lists.rs`, `src/app/source_commands/format.rs`, `src/app/source_commands/lines.rs`; modify `src/app/mod.rs` with a private module declaration.

**Interfaces (NEW, pub(super) where needed):**

```rust
pub struct Selection { pub anchor: usize, pub caret: usize }
pub enum SourceCommand {
    ToggleTask, ToggleTaskAt { start: usize, end: usize }, ContinueList,
    IndentList, OutdentList, Bold, Italic, Strikethrough, CycleHeading, MoveUp, MoveDown,
}
pub struct SourceEdit {
    pub range: std::ops::Range<usize>, pub replacement: String,
    pub selection: Selection, pub preserve_scroll: bool,
}
pub enum CommandResult { Native, NoChange, Edit(SourceEdit) }
pub fn compute(source: &str, selection: Selection, command: SourceCommand) -> CommandResult;
```

`Native` means let the key follow native behavior; `NoChange` consumes a recognized command without editing. Each child exports `pub(super) fn compute(source:&str, selection:Selection, command:SourceCommand)->CommandResult`, with the root dispatcher choosing the owner. Shared parser context remains in the root module. A test-only NEW `run(source:&str, selection:Selection, command:SourceCommand)->(String,Selection)` applies a returned edit; it is not the production mutation path.

- [ ] **Step 1: Pin failing source/selection tests.** Add inline unit tests in these modules using the spec's exact values. Include these named assertions:

```rust
// task_pointer_changes_only_state_byte_and_preserves_selection
assert_eq!(run("1. [X] café", sel(12,12), task_at(3,7)).0, "1. [ ] café");
// return_continues_checked_tasks_as_unchecked
assert_eq!(run("- [x] one", sel(9,9), ContinueList).0, "- [x] one\n- [ ] ");
// return_exits_empty_nested_item
assert_eq!(run("  - ", sel(4,4), ContinueList).0, "  ");
// selected_list_prefixes_remain_outside_inline_formatting
assert_eq!(run("- one\n- two", sel(0,11), Bold).0, "- **one**\n- **two**");
// move_preserves_crlf_terminator
assert_eq!(run("a\r\nb", sel(0,0), MoveDown).0, "b\r\na");
// terminal_separator_is_not_a_movable_extra_line
assert_eq!(run("a\n", sel(0,0), MoveDown).0, "a\n");
assert_eq!(run("a\n\n", sel(0,0), MoveDown).0, "\na\n");
```

Define test aliases `sel(a:usize,c:usize)->Selection` and `task_at(start:usize,end:usize)->SourceCommand` locally. Add exact tests for all ten keyboard commands, reverse selection mapping, mixed separators, code/metadata/escaped prefixes, real blank lines, selection ending at a line start, empty words, structural-prefix cuts, partial styled-span rejection, `***word***` toggling, quote prefixes, 2-space/tab indentation and numbered marker limits. Follow every behavior in the spec rather than reserializing Markdown.

- [ ] **Step 2: Run red:** `rustup run 1.99.1 cargo test --locked --lib app::source_commands`; expect unresolved command functions or failing exact-source assertions.
- [ ] **Step 3: Implement the borrowed pure API.** Use the existing pinned parser's source offsets and Unicode segmentation. Keep transformations in their owner files; share only the context used by multiple commands. Return the smallest contiguous replacement; copy untouched bytes exactly. Use the spec's multiline formatting and indentation policies. Marker-range toggling must validate both bounds against a currently parsed task, not infer the state byte from `start+3`. All task toggles set `preserve_scroll=true`.
- [ ] **Step 4: Verify green:** rerun the module tests and `rustup run 1.99.1 cargo test --locked --test editor_roundtrip --test editor_reconciliation`. Require exact source/selection results and unchanged existing editor contracts.
- [ ] **Step 5: Commit** as `Compute source-preserving Markdown editing commands`.

### Task E4: Keyboard Bridge, Persistence, and Native Command Verification

**Files:** Modify `ui/types.slint`, `ui/app-window.slint`, `ui/components/note-editor.slint`, `src/app/ui.rs`, `examples/local_review.rs`, `examples/sync_review.rs`; create `tests/editor_commands.rs` and `examples/editor_commands_review.rs`.

**Interfaces:** Consumes E1/E2/E3. Add NEW Slint enum `MarkdownCommand` with the ten keyboard actions from E3, kebab-cased. Add NEW struct `MarkdownEdit { handled:bool, changed:bool, start:int, end:int, replacement:string, anchor:int, caret:int, preserve-scroll:bool }`. Add NEW pure AppWindow callback `edit-markdown(source:string,anchor:int,caret:int,command:MarkdownCommand,marker-start:int,marker-end:int)->MarkdownEdit`; negative marker offsets request the caret-line task. Register its adapter in `src/app/ui.rs::apply_theme`, alongside the existing Markdown callbacks, mapping to E3's types. Expose the same callback through NoteEditor. NEW NoteEditor outputs `selection-anchor:int` and `selection-caret:int` forward the actual maintained input offsets; NEW AppWindow outputs `editor-anchor:int` and `editor-caret:int` forward those values for native verification. These are private fork contracts, not a claim about upstream documented selection getters.

- [ ] **Step 1: Write failing integration tests.** `tests/editor_commands.rs` reuses the existing headless AppWindow setup pattern. Named cases: `command_shortcuts_edit_once_and_undo_once`, `extra_modifiers_and_altgr_do_not_format`, `task_shortcut_does_not_open_search`, `readonly_and_composition_do_not_edit`, `native_return_and_tab_survive_outside_list`. Pin `⌘B` on `word` to `**word**`, one note-edited event, exact one-step undo/redo; pin `⌘⇧K` on `- [ ] one` to `- [x] one` with `active-view == "note"`. Inject E1 events explicitly. Existing base Command+K/F must still open their existing views.
- [ ] **Step 2: Run red:** `rustup run 1.99.1 cargo test --locked --test editor_commands`. Expect missing command bridge or unchanged source.
- [ ] **Step 3: Implement routing and generated fixture.** Read actual directed offsets from the maintained input, not a stored restore target. Match exact modifiers: Command+B/I, Command+Shift+X/H/K, Option+Up/Down; do not treat Control+Alt/AltGr or extra modifiers as these commands. Run editor handling before fallback app shortcuts so Shift+Command+K cannot become search. Return/Tab handling follows E3 Native/NoChange/Edit results. Active/Unknown composition yields to native input without invoking Rust transforms; rejected custom shortcut combinations must not fall through to a different app action.

  Set a scoped NoteEditor `suppress-caret-reveal` flag while applying an edit with `preserve-scroll`; skip only its scroll adjustment, not native cursor/IME/accessibility updates. Pass E3 edits to E2's native operation; never set note-text for commands. Successful edits continue through the existing callback bindings.

  NEW native example `run() -> Result<(), Box<dyn std::error::Error>>` exercises actual keyboard commands on generated text with a temporary store, as `local_review` does. Add an actual command to `local_review`'s failed-save/retry/restart sequence. Verify durable body, source/selection history, injected save failure retention, and restart recovery. In `sync_review` add a generated `- [ ] source command` case, dispatch the real task shortcut through the window's key-event path, then assert `- [x] source command` reaches both local persistence and the existing controlled server after offline/reconnect. Preserve all earlier fixture phases and do not substitute invoking the pure transform callback for input routing. Success marker: `native_editor_commands_passed: true`; the sync fixture retains `native_sync_contract_passed: true`.
- [ ] **Step 4: Verify green:** run `--test editor_commands --test markdown_markers --test editor_roundtrip --test sync_presentation`, rebuild/run `editor_commands_review` and `local_review` with native-validation, and run the existing Pinyin/atomic-undo fixture. Build `sync_review` with `--features native-validation,protocol-validation --locked` and run it, requiring its success marker and command-derived server body. Require E1 native proof first. Record real input coverage; a headless Idle injection is not native composition evidence.
- [ ] **Step 5: Commit** as `Wire Markdown commands through the native editor`.

### Task E5: Checkbox Pointer Activation from Rendered Geometry

**Files:** Modify authoritative patch, `ui/components/note-editor.slint`, prototype `probe.slint`, existing `prototypes/slint-source-highlighting/tests/markers.rs`, `tests/editor_commands.rs`, and `examples/editor_commands_review.rs`.

**Interfaces:** Add NEW native `TextInput.marker-activated(start:int,end:int)` callback. Add NEW internal `TextInputMarkerHit { source_range:Range<usize>, kind:TextInputMarkerKind, box_rect:LogicalRect }` and renderer method `text_input_marker_at_position(&self,input:Pin<&TextInput>,item_rc:&ItemRc,position:LogicalPoint)->Option<TextInputMarkerHit>`. Its shared-Parley implementation uses the same validated geometry as painting; unsupported renderers return None. Callback offsets are committed source offsets, not preedit-shifted display offsets. NoteEditor calls E4's existing ToggleTask request with the supplied range.

- [ ] **Step 1: Write failing pointer tests.** In the existing marker harness, name cases `only_visible_checkbox_shape_activates`, `layout_change_cancels_pressed_checkbox`, `cancelled_drag_does_not_escape_capture`, and `offscreen_caret_toggle_preserves_viewport`. Assert one activation and one edited event for a matching primary press/release; zero for body/padding/bullet/raw prefix, modifiers, drag/exit/cancel/focus loss, source or marker-model change, scroll/resize/scale and a shape invalidated by bidi/wrapping. Repeat with a layout change that leaves the rectangle numerically unchanged. On real AppWindow assert unchanged directed selection and scroll, then exactly one undo restoring source.
- [ ] **Step 2: Run red:** prototype `cargo test --locked --test markers` and root `cargo test --locked --test editor_commands` through Rust 1.99.1; expect missing activation or unchanged task state.
- [ ] **Step 3: Share geometry and implement pointer state.** Factor only marker geometry/validation from `CORE/textlayout/sharedparley/markers.rs` into a shared helper consumed by painting and the new renderer hit test; extend `CORE/textlayout/sharedparley.rs` and `CORE/renderer.rs`. Preserve brush/clip/fragmentation/compact-width validation. Do not duplicate rectangle formulas. In `CORE/items/text.rs`, intercept a qualifying press before native caret placement, retain capture until release even after cancellation, and invalidate on source/layout/visibility changes. Revalidate on release; fire the callback only for the same currently visible checkbox. Existing text selection and window dragging paths remain unchanged elsewhere.
- [ ] **Step 4: Verify green:** run prototype marker/source-edit/composition tests, root editor command and Markdown marker tests, and the generated native command example. Inspect native checkbox before/after screenshots and require exact source, no wrong-task activation, no unwanted scroll, and unchanged IME candidate placement.
- [ ] **Step 5: Commit** as `Activate task checkboxes using rendered hit regions`.

After E5, continue with the [chrome plan](2026-10-22-antinote-chrome.md), then search. Do not parallelize edits to the shared patch.
