# macOS Command shortcut correction — 2026-10-18

The packaged-app live-sync check exposed Command+K inserting `k` instead of
opening All Pages. The shared Slint handler checked `event.modifiers.meta`.
Pinned Slint 1.18.1 maps physical macOS Command to its `control` modifier and
physical Control to `meta`. The production fix changes that one shared guard;
existing actions and native editing behavior otherwise retain their routing.

## Regression and verification

The generated-text `ui_review --shortcut-contract` mode dispatches public Slint
key events through the actual AppWindow. It observes action callbacks and exact
CRLF/Unicode source preservation for Command+K, N, comma, F, G, Shift+G, S,
left/right brackets, W and Q. Physical Control+comma must not invoke an application
action. Callback observers keep the fixture from hiding, quitting, creating
real notes or accessing credentials/network.

The test was observed failing on the original guard: Command+K produced no
Browse action. It passed after the guard correction. The normal suite passed
184 tests with zero failures and 20 explicit ignores; formatting and strict
all-target/all-feature Clippy passed. This native regression is one of the
explicit ignores and was run separately:

```sh
rustup run 1.99.1 cargo build --example ui_review --all-features --locked
rustup run 1.99.1 cargo test --test ui_contract --all-features --locked \
  command_shortcuts_dispatch_actions_without_editing_note_source -- --ignored --exact
```

A fresh read-only review found no blocking issue for the supported macOS target.
The new native fixture's Metal selection is macOS-specific, matching application
scope; portability to unsupported operating systems is not added.

## Packaged-app checks

The exact development package was also exercised through public macOS CGEvent
delivery to its verified process ID, including proper modifier-change events.
All Command combinations passed: page search and Appearance opened without
inserting text; find/next/previous navigated two matches; brackets navigated
history; Command+N from search created an editable note; save preserved text;
hide/reopen and quit/restart restored source.

Three generated test pages were guarded-deleted and their absence confirmed.
The original note selection was restored. The disposable desktop driver needed
corrections for event targeting, modifier event types and asynchronous UI state;
its successful checks are recorded separately from the regression fixture.
These are automated OS-event checks, not claims of human keyboard, IME or
VoiceOver acceptance. [Structured evidence](command-shortcuts-evidence.json)
records results and the tested binary hash.
