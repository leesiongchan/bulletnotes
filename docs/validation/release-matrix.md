# Native and release evidence

Updated 2026-10-18. This is a development build; unobserved acceptance gates are
open. The older live BN/BN 2 previews and the user's store were not replaced.

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
| Edge/corner resize and full top hover/drag strip | User confirmed in Preview 2; latest production composition needs manual repeat |
| Physical shortcut delivery, previous-app focus, full-screen/Spaces, pinning | Manual observation open |
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

Developer ID signing, notarization, clean Apple Silicon/Intel macOS 14+ installs,
physical accessibility checks, exact selection restoration, and measured native
performance remain release gates. Ad-hoc fixture signing does not satisfy the
Developer ID/notarization gate.
