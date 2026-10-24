# Release acceptance follow-up — 2026-10-24

Current source: `b3c7d7a` plus benchmark, TLS lockfile and documentation changes.
This remains a development build. No editor, renderer, theme, storage schema or
synchronization implementation changed. The subsequent
[non-UI security follow-up](dependency-audit-2026-10-24.md) records two patched
certificate dependencies and current audit/compiler/test evidence.

## Verified

- `scripts/check.sh`: 396 passed, zero failures, 24 explicit ignores; formatting
  and strict all-target/all-feature Clippy passed.
- `python3 scripts/check-markdown.py`: 45 renderer/history/composition tests
  passed, zero failures or ignores; formatting and strict Clippy passed.
- A final focused `cargo clippy --bench interaction --all-features --locked --
  -D warnings` and formatting check passed after the benchmark edits.
- Nine existing native fixture executables built in the release profile. Building
  them does not establish native behavioral acceptance.
- Independent read-only review found no material issue in the benchmark and
  release-documentation diff. Local documentation links and diff whitespace pass.

The [structured record](release-acceptance-2026-10-24.json) contains source hashes,
commands, results and local log paths.

## Benchmark startup diagnosis

Two unchanged benchmark runs stopped with `page is unavailable locally` before
producing a timing report. The fixture previously assumed that a 300 ms delay
was enough for startup; the application dispatches its initial page only after
its first rendered frame and native window setup.

The benchmark now waits for the existing `local-startup-ready` acknowledgement,
then retains its separate page-load and rendered-frame checks. A 30-second
startup timeout reports frame count, visibility, alpha, occlusion and status.
There is no foreground, alpha or rendering-backend override.

The diagnostic run reported zero frames, visible true, alpha 0, occlusion 8192
and the initial local-note status. A read-only console check then confirmed
`IOConsoleLocked=true` and `CGSSessionScreenIsLocked=true`; a later recheck still
reported a locked desktop. This run does not establish a product startup defect
or any performance result. The queued rerun was cancelled after the lock was
identified. The user was asked to make an unlocked desktop available.

No fresh timing was accepted. The last recorded 1 MiB p95 remains 132.203 ms
against the unchanged 100 ms target in [performance evidence](performance.md).
No speculative performance fix was applied without a current baseline.

## Deferred native and remaining distribution work

The user subsequently deferred performance measurements, screenshots and
package-launch verification. Those checks are not prerequisites to continuing
the independent work recorded in the security follow-up. No new visual or native
acceptance is claimed. Native input/lifecycle reruns retain their separate desktop
requirements, and no visual change was made without current visual evidence.

The [previous position/input acceptance](editor-position.md) remains recorded
evidence on this host. VoiceOver speech/navigation/editing, other IMEs, physical
accessibility preference changes, full-screen/Spaces, supported hardware/macOS
coverage, native latency under executor stall, large-note performance, clean
installation, Developer ID signing and notarization retain their own gates.
No user note store, live server or Keychain entry was opened. Nothing was
published or notarized.
