# Native and release evidence

Updated 2026-10-19. This is a development build; unobserved acceptance gates are
open. Supported hardware is Apple Silicon only on macOS 14+; Intel and universal
builds are excluded by the user’s 2026-10-18 scope correction. The older live BN/BN 2 previews and the user's store were not replaced.

## Antinote interface and scratchpad navigation

[The 2026-10-19 evidence](antinote-ui-ux.md) records the installed-app inspection,
ordered synced scratchpads, guarded navigation/metadata checks, native layout
contracts, and generated before/after screenshots. These changes preserve the
existing open selection-capture, physical IME/VoiceOver, OS/hardware, performance,
and distribution gates below.

## Current automatic-sync checkpoint

[Task 6 acceptance](automatic-sync-acceptance.md) is the current source of truth
for automatic merging, immutable recovery, killed-process boundaries, generated
native unsaved typing/offline restart, real upstream worker/browser peer evidence
and release merge distributions. Earlier milestone counts and packaged-app live
checks below remain historical; the current live check used an isolated temporary
store and exact-path allowlist relay. Both final and corrected first live attempts
confirmed all three owned server paths absent after guarded cleanup.

Floating UI commit `803bd84` passed 30 controlled native cases and a fresh scoped
review. Its palette/static Slint instantiation-chain diagnostic remains a minor
with unknown attribution. Physical selection/IME/VoiceOver, native typing latency
under deliberate executor stall, macOS/hardware matrix, existing large-open
performance and distribution gates remain open. Controller owns final branch
review and a fresh development package; no notarization/upload is authorized.

## Lifecycle

Run `scripts/native-lifecycle-check.sh` for the generated-content, ad-hoc signed
LaunchServices check. It creates a unique bundle and temporary SQLite store;
its loopback server holds a response without releasing it. It does not connect
to the deployed space, inspect existing notes, or synthesize physical key events.

| Contract | Observed result |
| --- | --- |
| Restore exact CRLF, Unicode/emoji source and appearance in a new process | Passed native fixture |
| Close saves the current buffer before hiding while HTTP remains blocked | Passed native fixture |
| Quit saves locally and cancels sync without waiting for the held response | Passed native fixture |
| Failed local quit retains a visible editable buffer; retry persists it | Passed injected SQLite failure and process restart |
| Failed shortcut reservation retains the prior shortcut and settings | Passed exclusive Carbon reservation with owned competing process |
| Failed shortcut settings save releases only the candidate | Passed injected SQLite failure and owned availability probes |
| Successful shortcut change releases the old shortcut | Passed owned availability probes |
| Stale shortcut settings completion cannot replace a later registration | Passed registry regression |
| Second launch refuses a second writer and reopens the verified owner | Passed terminal and ad-hoc signed LaunchServices fixtures |
| Appearance changes update the theme without altering source | Passed own-application Aqua/Dark Aqua KVO observation |
| Off-screen placement is clamped; font and placement persist together | Passed native fixture |
| Edge/corner resize and full top hover/drag strip | User confirmed in the packaged build `build-20261018T040718Z` |
| Physical Option+A hide/show, previous-app focus, pinning | User confirmed in the packaged build `build-20261018T040718Z` |
| Physical full-screen/Spaces behavior | Manual observation open |
| Four built-in themes, font sizes and paper styles | User confirmed in the packaged build `build-20261018T040718Z`; physical system appearance switching and restart persistence remain unobserved |
| Real IME, clipboard/undo behavior, VoiceOver selection and editing | Manual observation open |
| High contrast palette and minimum-size layout | Palette regressions passed; native Paper/Graphite captures inspected; physical OS toggle still open |
| Reduced transparency/motion | Policy flags observed via public API; opaque/static UI; physical preference change still open |
| Long page list | Native benchmark reached all 10,000 generated rows; physical keyboard/VoiceOver traversal still open |
| Diagnostics | Sentinel, seven-day expiry, failed export preservation regressions passed; explicit native save dialog awaits manual interaction |
| Exact directional caret/selection capture and restoration after relaunch | Open supported API gate; see [selection investigation](selection-api.md) |

The initial signed-bundle run failed because `activateFromApplication:options:`
was declined for the short-lived launcher. The verified owner now receives an
explicit PID-filtered reopen notification and shows its own window. This channel
accepts no command, path, userInfo, or note payload; an untrusted local sender can
only request the same reopen action as the menu. A successful queued request does
not establish physical focus behavior across Spaces.

The kernel file lock remains writer authority. PID-only lock metadata is used
only to locate a running application, whose bundle identity or executable is
checked before requesting activation. Older previews without metadata produce an
actionable existing-instance message.

`global-hotkey` 0.9.0 uses non-exclusive Carbon registrations. An owned-process
regression demonstrated duplicate registration success. The production adapter
uses the public `RegisterEventHotKey` exclusive flag, main-thread ownership,
public physical-key constants, an application-owned event signature, and retained
registration references. It unregisters its keys and removes its event handler
before releasing callback storage. A handler-removal failure disables callbacks
and retains their allocation to avoid a dangling FFI pointer.

Source: Apple SDK `CarbonEvents.h`, `CarbonEventsCore.h`, `Events.h` and Apple's
[cooperative activation](https://developer.apple.com/documentation/appkit/nsrunningapplication),
[distributed notification](https://developer.apple.com/documentation/foundation/distributednotificationcenter)
and [notification observer](https://developer.apple.com/documentation/foundation/notificationcenter)
documentation. Appearance/accessibility observers are retained and removed from
their originating sources; callbacks queue work into Slint's main loop.

Solid paper and no animated transitions honor reduced-transparency/motion policy.
Native blur and inline Markdown colors remain the user-approved deferrals. High
contrast policy has palette regressions and native fixture captures; actual system
preference toggling and physical accessibility observations remain open.
[Performance evidence](performance.md) records the native measurements: the 1 MiB
open target remains unmet. Licensing and distribution have separate gates.

## Distribution gates

Developer ID signing, notarization, clean Apple Silicon macOS 14+ installs,
physical accessibility checks, exact selection restoration, and measured native
performance remain release gates. Ad-hoc fixture signing does not satisfy the
Developer ID/notarization gate.

## Packaging checkpoint (2026-10-18)

| Check | Evidence |
| --- | --- |
| Apple Silicon release build | Locked Rust1.99.1 build, thin LTO, one codegen unit; Mach-O/Info.plist target14.0; hardened ad-hoc signature verified; isolated generated-UI launch passed |
| Architecture boundary | Apple Silicon ARM64 only; Intel and universal packages are rejected. Earlier Intel artifacts are superseded historical evidence and are not supported releases. |
| Strict distribution rejection | Real generated Mach-O regression rejects unsigned, minOS15, missing resources and development distribution; ad-hoc staple validation fails as expected |
| Minimum Rust | Final production source compiles with Rust1.92.0 and Cargo.lock; no toolchain defaults changed |
| Upgrade persistence | Separate current-code process retains committed notes, exact offline source, guarded queue and custom appearance; migration-failure/corruption regressions pass |
| Metadata settings ordering | Native SQLite-contention regression prevents failed shortcut inheritance by a window save; queued theme changes retain a successful shortcut |
| Slint attribution | Official AboutSlint component connected to top-level About; native About window observed; optional WindowServer image capture refused despite native visibility and is not claimed |
| Production signing/notarization | Not performed; required release identity/profile absent; production packaging preflight refuses to proceed |
| Isolated official SilverBullet | Final guarded probe passed against the owned official2.12.1 server |
| Deployed endpoint final check | DNS lookup timed out before connection; owned probe journal has zero rows, no probe cleanup pending; earlier successful verification remains historical evidence |

`curl` timing identified the deployed recheck's failure as DNS resolution, with
zero connect/TLS time; this does not establish the remote server's health.
Automatic offline retry behavior is covered by local/native fixtures. No resolver,
proxy, TLS policy or server setting was changed to work around that observation.
The user's existing note bodies and live preview processes remain untouched.

## Final review verification

The [independent branch review and fix record](final-review.md) documents four behavioral fixes and one deferred presentation minor. Final automated verification: 171 passed, zero failed, 18 explicit ignores; strict Clippy/fmt and Rust 1.92.0 production check passed. Generated native merge-shutdown/restart, two-client offline sync/restart, and lifecycle checks passed; the signed LaunchServices lifecycle check also passed. Closing freezes authoring through the complete durability transition and restores it after failure. No manual gate above is waived.

## Apple Silicon scope correction (2026-10-18)

The user narrowed support to macOS 14+ on Apple Silicon only. Packaging and runtime attribution target `aarch64-apple-darwin`; the verifier accepts only a thin ARM64 Mach-O, and explicit Intel application compilation is rejected. Generated signed Intel and universal fixtures exercise rejection without executing either artifact. ARM64 positive verification remains covered.

Verification: 173 passed, zero failed, 18 explicit ignored gates; strict Clippy/fmt passed. An explicit Intel Cargo check failed with the intended Apple Silicon-only diagnostic. Rust 1.92.0 production checking passed. Attribution regeneration passed with the same verified full license text. Intel runtime testing is removed from release acceptance; supported macOS, input/accessibility, performance, signing and notarization gates remain open.


## Metal renderer checkpoint — 2026-10-18

Production now uses Winit/Skia/WGPU 29 with Metal only. [Renderer evaluation](metal-renderer.md)
records the selection, actual CAMetalLayer proof, native checks and targeted review.
Automated verification: 174 passed, zero failed, 19 explicit ignored gates; strict
Clippy/fmt and Rust 1.92.0 production checking passed. The renderer fixture and
signed LaunchServices lifecycle passed, as did generated-data sync, recovery,
local editing, appearance and installation checks. The verifier now requires
Metal acknowledgement from its no-store installation path.

Fresh same-host 1 MiB p95 is 132.203 ms versus 152.452 ms on the fresh OpenGL
comparison; the original 100 ms gate remains open. All supported-OS hardware,
manual input/accessibility, selection-capture and distribution gates remain open.


## Structural hardening checkpoint — 2026-10-18

[Code/design audit](structure-hardening.md): persisted position now belongs to the
domain; foreground lifecycle has one owner; LocalUi responsibilities are separated;
native/connection callbacks delegate named workflows. A reproduced dropped-future
lock and an observed About/activation reopening race were fixed. Verification:
184 passed, zero failed, 19 explicit ignored gates, strict Clippy/fmt, Rust1.92
checking, nine generated-data native contracts and signed LaunchServices lifecycle.
These checks do not waive the broader release gates above.

## Packaged UI acceptance — 2026-10-18

The user reported “all good” for the requested hover controls, dragging,
edge/corner resizing, pinning, Option+A hide/show and previous-app focus,
four built-in themes, font sizes and paper styles in `build-20261018T040718Z`.
This Apple Silicon development package was built from `063b935`; fresh checks
passed 184 tests with 19 explicit ignores, formatting and strict Clippy.
Packaging verified its ad-hoc signature and isolated Metal startup.

The report covers those requested checks only. Physical IME, clipboard/undo,
VoiceOver, full-screen/Spaces, system appearance switching, restart persistence
and deployed-server acceptance remain open.

## Sync acceptance recheck — 2026-10-18

The current-source native `sync_review` example was rebuilt with all features.
The explicitly selected
`native_two_client_offline_reconnect_and_restart_preserve_source` check passed
(one passed, zero failed). It exercised generated loopback data and a temporary
SQLite store, including offline editing, remote updates, reconnect and
separate-process restart. It did not open the user's store or use Keychain.

The deployed `.ping` check failed before HTTP with curl error 6 (hostname
resolution). This Mac's configured DNS resolver returned `NXDOMAIN` for both A
and AAAA queries for `notes.harflix.lol`; a control A query for `github.com`
succeeded. The system host lookup also returned no address. No deployed write
probe was started. Private-network/VPN access or a changed endpoint remains to
be clarified before live-server acceptance; this result does not establish the
server's health or protocol compatibility. No DNS or app configuration changed.

After the user reported restored connectivity, a retry returned HTTP 200 from
`.ping`. The current-source
`official_or_deployed_server_verifies_and_cleans_an_owned_page` check then passed
(one passed, zero failed) against `https://notes.harflix.lol`. It verified readable
catalogue access, guarded creation/update, rejection of stale updates/deletes and
create collisions, exact owned-page readback, guarded deletion and confirmed
absence. The separate local probe journal contained zero pending rows afterward.
Only the generated probe page was mutated; no existing note bodies or real
Keychain entries were accessed by this check. The earlier DNS observation is
superseded for connectivity; no DNS or app configuration was changed here.
This verifies the deployed protocol and write guards. A packaged-app/browser
round trip and offline/reconnect acceptance against the real space remain open.

## Live packaged-app round trip — 2026-10-18

The exact ARM64 development package `build-20261018T040718Z` from `063b935`
was exercised on macOS 27.0.1 against the deployed space and the real SilverBullet
browser editor. [Structured evidence](live-sync-evidence.json) identifies the
binary and results. Native editing used macOS accessibility actions; browser
editing used the agent-browser skill in an isolated named browser session.

| Check | Observed result |
| --- | --- |
| Native edit reaches the browser automatically | Passed, exact generated Japanese/emoji source |
| Browser edit reaches the packaged app through automatic polling | Passed, exact source and Synced status |
| App is offline while browser/server remain online | Passed; scoped HTTPS CONNECT tunnel disconnected only this app; upstream TLS validation remained enabled |
| Offline edit is saved locally without changing the server | Passed, native local-save acknowledgement and independent server/browser readback |
| Normal quit and restart while still offline restore the edit | Passed, exact generated source before reconnecting |
| Reconnection automatically uploads the restored edit | Passed, server/browser exact readback and native Synced status without manual Save or Connect |
| Cleanup and normal launch | Passed; guarded deletion and 404 confirmed, owned browser closed, proxy stopped, normal app reopened with original note selected and Synced |

The production binary has no alternate-store option, so this pass used its normal
application store with a uniquely named test page, rather than the isolated
database used by the earlier native fixture. The disposable driver's first two
attempts exposed browser text-replacement and initial accessibility-action timing
issues; corrected automation completed the full sequence. Its logs and generated
sources remain in the ignored evidence directory recorded by the JSON file.

An additional product defect was found: Command+K inserted `k` instead of
opening All Pages. The single temporary character was immediately removed and
the restored text reached Synced before testing continued on the generated page.
At this checkpoint the shared handler in `ui/app-window.slint` checked `event.modifiers.meta`, while
the pinned Slint source maps macOS Command to `control` and Control to `meta`.
The subsequent correction below resolves this finding. These observations do
not establish physical IME composition or VoiceOver behavior.

## Command shortcut correction — 2026-10-18

The shared handler now uses Slint's `control` modifier for macOS Command.
The [regression and native verification record](command-shortcuts.md) documents
the observed failing-then-passing test, 11 Command combinations with exact source
preservation, negative physical-Control routing, and a fresh packaged-app smoke
check covering every Command combination with OS-delivered events. Full checks
passed 184 tests with zero failures and 20 explicit ignores; the new native
regression was run explicitly. Three generated pages were cleaned up with guarded
deletion and confirmed absence. Review found no blocking macOS issue. Human
keyboard, IME, VoiceOver and the remaining release gates retain their own status.

The Antinote follow-up full suite passes with 326 tests. A fresh signed LaunchServices lifecycle rerun in this session stops before the first rendered frame; the unchanged `1c90b58` base fails the same startup check. This unresolved current-session gate is recorded in [the validation report](antinote-ui-ux.md#independent-review); earlier lifecycle successes above are historical evidence.


## Antinote interaction checkpoint — 2026-10-23

The [interaction validation record](antinote-interactions.md) covers native
Markdown commands with one-step undo, rendered checkbox activation, quieter
paper chrome and literal highlighted search previews. Search reentry now clears
a retained result selection so model replacement cannot steal query focus.
Two Important independent-review findings around nested prefixes and heading/code
context were reproduced and fixed. One unrelated lockfile resolution Minor is
recorded as deferred.

Fresh automated verification passed 393 application tests and 43 renderer tests,
with zero failures, formatting and strict Clippy; the application suite retains
24 explicit ignores. Nine named native contracts passed, including real Pinyin
composition guarding and a command followed by controlled sync/restart.
The full native input fixture remains **unaccepted**: one idle run failed to start
composition after undo, while later diagnostic attempts stalled before the first
rendered frame. Its passing guard fixture does not waive this gate. Other IMEs,
VoiceOver, cross-page/relaunch selection, hardware/OS, performance and distribution
retain their existing acceptance requirements. Main was not merged or pushed.


## Antinote native input follow-up — 2026-10-23

The [follow-up evidence](antinote-interactions.md#native-input-and-startup-follow-up--2026-10-23)
closes the full input-fixture gate left open at `7911aa0`. Two fresh complete
traced runs passed all four real Pinyin conversion/commit/undo/cancel cases,
candidate placement, source restoration and one-step undo in Tartan and Mononoke.
The implementation and fixture were unchanged. A third run's observed application
deactivation was correctly rejected and is retained as an interrupted attempt.
Normal startup and controlled cover/uncover with both chrome orderings passed;
the controlled occlusion recovers naturally, without an alpha or backend override.
Supplementary light/dark/minimum/contrast captures passed. This does not identify
the original historical occluder or explain every earlier untraced input failure.

Fresh application checks again passed 393 tests, zero failures, 24 explicit
ignores, formatting and strict Clippy. The unchanged renderer's 43 passes remain
applicable. The ten named interaction contracts are accepted on this host;
other IMEs, VoiceOver, cross-page/relaunch selection, hardware/OS, performance and
distribution remain separate release requirements.
