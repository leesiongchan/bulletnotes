# Antinote interface and scratchpad validation

Date: 2026-10-19. Development implementation on macOS 27.0.1 (26A434), arm64, Rust 1.99.1, Slint 1.18.1.

## Reference and scope

Opened the installed Antinote 2.1.3 app and inspected its paper window, independent hover controls, settings, search, footer tooltips, navigation commands, and scratchpad/retention options. The user approved adopting its visuals and scratchpad navigation with automatic SilverBullet sync and manual deletion. The [approved specification](../superpowers/specs/2026-10-19-antinote-ui-ux-design.md) and [implementation plan](../superpowers/plans/2026-10-19-antinote-ui-ux.md) record the decisions.

Antinote screenshots and existing notes were not copied into the repository. Every screenshot below contains generated BulletNotes content. Native checks used temporary SQLite stores; controlled HTTP checks used a local fixture.

## Result

The header now has circular Settings, Search, New, and Pin controls beside native traffic lights. The footer is an inset capsule with recoverable Trash, creation date/stack position, save/sync text, Older, Newer, Newest, and Changes. A small accessible accent dot identifies the newest scratchpad while controls are hidden. A local-save or sync failure needing attention remains visible outside the hidden controls.

The editor has 32 px top clearance and unchanged 40 px side margins. The window remains 505 by 552 logical pixels by default, with a 400 by 440 minimum. Revealing controls overlays the paper without changing editor geometry, source, scroll position, or wrapping. Native traffic lights are aligned 22 px below the top after AppKit completes its layout/reveal animation; a window-coordinate regression verifies that position.

Scratchpads are active Markdown pages under exact `Quick Notes/`, scoped to the selected connection. Immutable metadata survives page-row replacement, Trash/recovery, edits, visits, acknowledgements, and restart. Catalogue-only notes count, and a 275-entry regression exceeds the production 200-result search batch. Generated filenames provide initial creation time; other legacy notes freeze their modification or first-observation time. New local notes enter first even if the clock moves backward.

Command+[ moves older, Command+] newer, Command+1 newest, Command+N creates a scratchpad, Command+K searches all pages, Command+F finds within the note, Command+comma opens settings, and Command+P toggles Pin. Ordinary pages have no stack position and disable relative steps. Empty local drafts are not uploaded; nonempty scratchpads use the existing guarded outbox. Deletion is manual and recoverable.

Text/paper settings precede the theme collection. Search keeps title/path/preview/availability and adds compact modification dates, with folder filtering, keyboard selection, and pagination.

## Screenshots

| View | Evidence |
| --- | --- |
| Previous note layout | [Before](antinote-ui-ux/before-note.png) |
| Circular header and capsule footer | [After](antinote-ui-ux/after-note.png) |
| Minimum supported window | [400 by 440](antinote-ui-ux/minimum.png) |
| Hidden controls and newest marker | [Writing view](antinote-ui-ux/hidden.png) |
| Dark palette | [Graphite](antinote-ui-ux/graphite.png) |
| Search at minimum size | [Search](antinote-ui-ux/search-minimum.png) |
| Text/paper controls above themes | [Settings](antinote-ui-ux/settings-minimum.png) |
| Actionable error with controls hidden | [Save failure](antinote-ui-ux/attention.png) |
| Increased contrast, reduced motion, 24 px ruled paper | [Accessibility policy fixture](antinote-ui-ux/accessible.png) |

The images were inspected for control overlap, clipping, contrast, and settings reachability. Hover controls intentionally cover paper while visible. The source and geometry checks establish that they do not reflow the editor.

## Verification

`CARGO_TARGET_DIR` pointed at the shared local target directory during this worktree run. Run the following from the checkout:

```sh
scripts/check.sh
rustup run 1.99.1 cargo build --example local_review --example ui_review --features native-validation --locked
target/debug/examples/local_review --scratchpad-contract
target/debug/examples/local_review
target/debug/examples/ui_review --floating-contract
target/debug/examples/ui_review --shortcut-contract
target/debug/examples/ui_review --contract
target/debug/examples/ui_review --appearance-layout-contract
target/debug/examples/ui_review --appearance-contract
target/debug/examples/ui_review --scrollbar-contract
```

| Check | Observed result |
| --- | --- |
| Format and strict all-target/all-feature Clippy | Passed; no compiler warnings |
| Full automated lib/bin/test/example suite | 326 passed, 0 failed, 24 explicitly ignored across 52 targets |
| Durable membership/order, legacy dates, DST, clock rollback, upgrade, recovery, restart | Passed store regressions |
| Late stack/barrier/load results, connection roundtrip, repeated steps, missing/uncached destinations | Passed coordinator regressions |
| Failed metadata write retains note; explicit retry reaches intended neighbor | Reproduced before fix; coordinator and native regressions pass |
| Cancelled/failed recovery readiness releases editing and close admission | Reproduced before fix; coordinator and native regressions pass |
| Position changes during a page load invalidate readiness and reject stale completion | Reproduced before fix; coordinator regression passes |
| Alternate restoration over a reusable deleted destination retains source order after restart | Reproduced before fix; store regression passes |
| Offline scratchpad capture, restart, subsequent guarded sync, empty draft stays local | Passed controlled HTTP regression |
| Generated native stack controls, counters, ordinary-page behavior, known position target | `native_scratchpad_contract_passed: true` |
| Existing generated native editing/find/save/retry/restart callbacks | `native_local_callbacks_and_restart: true` |
| Independent hover/focus, pointer editing, source/undo/geometry, native buttons | `floating_chrome_contract_passed: true`; 30 size/font/theme cases |
| Command shortcuts preserve source | `native_shortcut_contract_passed: true` |
| Theme/view changes preserve source | `native_ui_contract_passed: true` |
| Text/paper controls precede themes | Native layout contract passed |
| Appearance persistence and injected failed-save rollback | `native_appearance_contract_passed: true` |
| Scrollbar drag/track and wheel input | `native_scrollbar_contract_passed: true` |

The first full run found lifecycle model fixtures that still assumed synchronous draft admission. Their helpers now explicitly acknowledge the new metadata-readiness event, like the other coordinator fixtures; the full suite then passed.

## Independent review

A fresh read-only review found three Important defects and no Critical defects. The consolidated fix pass explicitly completes cancelled or failed pending recovery requests, invalidates page-load readiness when outgoing position changes, and replaces historical destination ordering only after restoration validates destination reuse. Each finding reproduced before its fix and passed afterward. The native generated scratchpad contract also verifies recovery cancellation clears the busy state, allows editing, and admits close. Its loop stays alive until explicit quit so hiding the last window cannot truncate the check; the final contract passed twice consecutively. The complete suite then passed with 326 tests.

The additional signed LaunchServices lifecycle check failed before the first rendered frame (`frames=0`), before note admission. The unchanged base commit `1c90b58` also fails the same fixture at startup on this host in this session. This check is **not passing** and does not establish a full-application Metal/startup regression caused by this change; it remains an unresolved validation gate. Widget-level native UI fixtures pass. Temporary diagnostic instrumentation was removed after the baseline comparison; no renderer or startup policy was changed.

## Local preview

An ad-hoc signed development bundle was built and verified at:

`/Users/leesiongchan/Projects/bulletnotes/target/local-preview/build.gD92le/Bulletnotes Local.app`

The bundle was not launched against the user's store. Changes are committed in the managed detached worktree `/Users/leesiongchan/.codex/worktrees/8b11/bulletnotes`; `main` still requires an explicit integration choice. The unresolved full-application first-frame validation gate above also applies to this handoff.

## Implementation decisions

- Used the managed detached worktree under the existing execution approval. Cost if wrong: the work needs explicit integration before it appears on `main`.
- Retained the approved Slint selection API limitation. Cost if wrong: live directional selection may not restore exactly across page changes or restart.
- Coalesced stack reads in the coordinator, with one request in flight and one refresh flag. Cost if wrong: refresh latency could grow; generation tests cover stale replies.
- Adapted existing coordinator fixtures with an explicit-success readiness helper; raw regressions exercise errors and races. Cost if wrong: helper-based tests alone would miss barrier failures.
- Used 275 catalogue entries because the actual search batch is 200. Cost if wrong: larger batching would require expanding that regression.
- Split the generated native scratchpad fixture into its own support module. Cost if wrong: fixture maintenance spans two files.
- Kept physical IME, VoiceOver, trackpad, OS/hardware, performance, distribution and notarization gates open. Cost if wrong: automated checks would overstate release confidence.
- Kept translucency, inline Markdown coloring, slotted notes, split view, archive, locking, exports and expiry outside the approved scope. Cost if wrong: these Antinote capabilities remain absent.
- Left the reproduced baseline first-frame startup failure open. Cost if wrong: the signed app may fail to present its editor on this host; no successful current full-app lifecycle claim is made.

## Limits

Exact live directional anchor/focus capture across page changes/relaunch remains the documented [supported Slint API gap](selection-api.md). Tests restore supplied, validated targets and preserve the existing editor instance across auxiliary views; they do not establish complete native selection capture.

Real IME composition, physical VoiceOver use, physical trackpad gestures, OS/hardware coverage, existing large-open performance targets, and distribution/notarization gates remain open in the [release matrix](release-matrix.md). A recurring Slint instantiation-chain diagnostic is still visible in the fixture and was already recorded before this change.
