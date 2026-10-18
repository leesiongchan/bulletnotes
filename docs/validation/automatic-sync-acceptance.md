# Automatic sync acceptance — 2026-10-18

Tasks 1–5 and floating chrome are implemented; this checkpoint adds process-loss,
native and real-upstream evidence. It is development acceptance on one Apple M4
Pro (Mac16,11, 48 GiB), macOS 27.0.1, Rust 1.99.1. It does not waive the release
and native-fidelity gates below. Machine-readable observations are in
[automatic-sync-evidence.json](automatic-sync-evidence.json).

## Process loss and durability

Two parent-supervised integration tests kill only a nonce/PID-verified child,
bypassing Rust destructors and SQLite close. Seven editor stages cover blocked
admission, admitted U1 before editor acknowledgement, coalesced U2, blocked and
committed checked replay, and old leased/unknown PUTs. Reopening presents exact
latest accepted U, retains canonical S and genuine HTTP H, and leaves the journal
pending. Replay preserves S-only text and captured originals; old completion
cannot acknowledge U2. The chosen upload payload is checked exactly.

Eight network stages cover blocked/committed resolution, recovery create and
readback before local acknowledgement, recovery acknowledgement, canonical upload
with newer text, and before/after durable draft drain while Close acknowledgement
is withheld. The parent HTTP server survives child loss. A real worker resumes,
reads back immutable recovery attempts and synchronizes the exact chosen source.
Before-commit cases use a second SQLite connection's write lock, not an internal
production rendezvous. Committed-but-unobserved input survives; never-committed
keyboard input is not promised durable. No arbitrary production hook was added.

## Native and live worker observations

`sync_review --acceptance` keeps a generated multi-file server and exact-path
relay alive across two native child processes. The relay holds an owned incoming
observation until public Slint keyboard input is demonstrably uncommitted. Both
independent changes and overlapping changes synchronize automatically. Overlap
retains the server passage plus the independent local tail and publishes the
exact complete original as an ordinary recovery file, without opening Changes.
Three unchanged refreshes retain one immutable event/attempt, 14 local history
records and the same owned catalogue paths.

The fixture quits with the relay still returning 503. A new process restores
exact LF/Unicode authored source while it is still pending and confirms offline
transport before reconnecting. Resumed upload and delete/edit recreation match
exact readback. Native keyboard input uses LF: pinned Slint rejects an entire
synthetic KeyPressed payload containing CR. CRLF preservation remains covered by
the process/store contracts and the older programmatic `sync_review` fixture.

`sync_review --live-acceptance` repeated those observations against
`https://notes.harflix.lol`, then a third native process received an independent
browser edit through the actual `SyncHandle`. The browser used the actual owned
CodeMirror editor: verified Select All, Backspace, then public CDP `inserttext`
(editor input/paste semantics). Exact editor and HTTP readback passed. No fetch
mutation, private CodeMirror API or physical typing is claimed. A blank browser
bootstrap required one exact-page reload before it became editable.

This is live upstream HTTP/worker acceptance **through a controlled relay**.
The native client uses explicit loopback fixture transport; the relay uses normal
upstream HTTPS/TLS verification. It filters the upstream catalogue before worker
exposure, journals the exact UUID note/probe/recovery destinations and permitted
generated sources before forwarding, and rejects unregistered routes or source
bytes. New recovery requests wait at most one second for exact registration;
there is no prefix body allowance. The final run forwarded no rejected routes.
SessionOnly credentials and an explicit temporary SQLite store avoid the normal
app database and Keychain. No unrelated note body was inspected.

Before cleanup, browser exact readback and networkidle completed and its owned
session closed. Every native child exited, removing worker/watch authority.
Cleanup checked exact expected generated source and a current strong validator,
then confirmed all three journaled paths absent. A first browser attempt inherited
one autoindent space; cleanup refused that unexpected source and retained the
journal. After exact source repair and browser drain, its three paths also passed
guarded deletion/absence. There are no known outstanding fixture paths.

Viewed owned captures show the independent `Q` and remote change together,
canonical overlap plus local tail without a prompt, exact offline restart source,
and matching browser/native peer text. [Browser](assets/automatic-sync/live-browser-owned-editor.png)
and [native](assets/automatic-sync/live-browser-peer.png) capture the same generated
source. These are owned-window captures, not desktop screenshots.

## Resource and telemetry evidence

[Integrated release samples](automatic-sync-merge-distributions.json) use
`MergeExecutor::merge`, with exact `merge_text` output/disposition assertions
outside timing, three warmups and 20 measured samples per case. Generated repeated
seven-word vocabulary covers independent/overlap changes at 128, 512, 1,024 and
2,048 words, a 19,989 aggregate word-token case, the exact/over 1 MiB byte boundary,
and a 20,404-token fallback. All expected sources/dispositions matched. p95 at
2,048 words was 1.393 ms independent and 1.741 ms overlap; exact 1 MiB changed
inputs took 48.133 ms. Over-byte and over-token cases fell back in 20/20 samples.
These are this host's observations, not a production latency guarantee.

Limits are 1 MiB per changed input, 20,000 aggregate tokens per admitted pass,
one blocking slot and a 150 ms wait deadline. Decoded merged output is bounded by
combined input bytes, not a nonexistent 1 MiB output cap. One-sided fast paths can
retain larger unambiguous sources. Decoder invalid-ID/encoding/output-bound tests
remain explicit; saturation and late results are exercised under repeated load.

The existing private executor stall test now records 20 baseline and 20 stalled
SQLite acknowledgements while its blocking closure holds the actual store executor
slot through a cfg(test)-only clone getter. A new regression first failed because
the former independent executor did not stall real journal replay; sharing the
store executor made checked replay take the busy fallback and preserve exact U.
No production feature, public rendezvous or arbitrary runtime hook was added. Saves
complete while stalled; timeout retains the slot, repeated work falls back, and
late output cannot replace selected source or durable text. Baseline/stalled p95
SQLite acknowledgement was 5.141/5.086 ms in the shared-slot run. [Raw samples](automatic-sync-stalled-save.json)
are debug storage latency, **not native frame/typing latency under a stall**.
Typed `text_merge`, `automatic_recovery` and `editor_replay` operation timers use
finite outcomes; privacy tests exclude source, path, URL and validator sentinels.

## Reviewed UI and remaining gates

Floating chrome commit `803bd84` passed its controlled 30-case native contract,
palette/shortcut/preview-hover/static pixel follow-ups and independent scoped
review. The final physical-event contamination count was zero. Hidden paper and
opaque inset surfaces preserve editor geometry, with max-font first/last lines
reachable. Supported selection restoration and undo controls passed in that
fixture; exact live selection capture, IME and VoiceOver remain open.

Palette/static routes emitted `Slint: long callback/instantiation chain detected`
after ten instantiation/change iterations. Those contracts passed, and the accepted
floating route did not emit it. Attribution is unknown; this minor is retained
for final review rather than suppressed or declared preexisting.

Physical gestures/Spaces, older supported Apple Silicon/macOS versions, exact
live selection/IME/VoiceOver, native typing/frame latency under deliberate shared
executor stall, the existing 1 MiB native-open target, clean install/upgrade,
Developer ID signing/notarization and distribution remain unverified. Controller
owns the fresh whole-branch review and subsequent development package.


## Final automated checks

`scripts/check.sh` passed formatting, strict all-target/all-feature Clippy and
281 tests with zero failures and 23 explicit ignores across 47 test binaries.
The two new ignored helpers are run by their parent crash tests; unrelated native,
Keychain, external-server and intentionally open fidelity gates are not counted
as passing. The new native/live supervised example routes were executed directly
as described above. Rust 1.92.0 `cargo check --all-targets --locked` passed without
warnings. `git diff --check` passed. Runtime notices were regenerated and the
unchanged lockfile matched the dated zero-vulnerability/five-warning audit.


## Scoped review correction

The full 281/0/23 integration check above belongs to commit `4827892`. Its scoped
review found preservation timers covering only a test-used wrapper and missing
child cleanup on early supervisor errors. Actual HTTP/editor preservation
transactions now report success only after commit, and report finite storage
failure on rollback. A nonce-owned headless regression proves that filesystem
and validation errors terminate and reap the supervisor's child before returning;
unsafe cleanup still retains its owned journal.

Covering checks after these fixes passed 83 tests with zero failures and three
explicit child-helper ignores across six targets, plus formatting, affected
all-feature strict Clippy and diff whitespace checks. No full/native/live run was
repeated for these instrumentation/ownership fixes. Deferred review minors remain:
browser completion markers are not nonce-bound across invocations, and benchmark
compiler metadata hardcodes 1.99.1 (the actual compiler of the recorded samples).
