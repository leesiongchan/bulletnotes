# Automatic sync acceptance — 2026-10-18

Tasks 1–5 and floating chrome are implemented; this checkpoint adds process-loss,
native and real-upstream evidence. It is development acceptance on one Apple M4
Pro (Mac16,11, 48 GiB), macOS 27.0.1, Rust 1.99.1. It does not waive the release
and native-fidelity gates below. Machine-readable observations are in
[automatic-sync-evidence.json](automatic-sync-evidence.json).

## Foreground latency and note layout follow-up

The selected cached note is now checked immediately on selection and about once
per second while the window is shown. Unchanged source uses HEAD; changed source
is downloaded through the existing guarded reconciliation path. Other pages keep
the 30-second visible / 60-second hidden catalogue cadence. Foreground checks
stop when hidden or cleared and preserve error backoff, local-only drafts and
retired-delete watch admission.

Local durability remains 250 ms from the first dirty edit. Upload batching now
waits 250 ms after a committed revision, capped at one second of continuous
batching; network, storage and retry time are additional. The editor uses a
16 px top margin and 40 px side margins. Hover chrome may cover the first line,
as explicitly selected by the user; native first-line click/type/undo passed.
The full automated check passed 297 tests, with 24 explicit ignores, and the
30-case native hover contract passed with its original watchdog unchanged.
The updated bundle and source hashes are recorded in
[the responsiveness follow-up](automatic-sync-responsiveness-followup.json).

## Packaged app runtime correction

A subsequent report of an app-bundle crash after a remote edit exposed a gap in
the earlier native evidence: the fixtures entered Tokio around Slint, while the
production entry point did not. Removing the fixture guard reproduced a panic
in `MergeExecutor::merge` through the UI's editor rebase: “there is no reactor
running.” `LocalApplication::run` now takes the runtime and enters it for the
native event loop; its guard drops before runtime shutdown. Both reconciliation
and sync acceptance fixtures start without an ambient runtime guard and use this
same runner. Their passing results before this correction did not establish the
packaged app's runtime setup was correct.

The native editor regression now passes in debug and release profiles, and the
supervised native sync scenario passes without a fixture runtime guard. The full
check script passes 289 tests, with 24 explicitly ignored native/helper tests;
the Rust 1.92 all-target check also passes. The replacement development bundle
and source hashes are recorded in
[the runtime follow-up](automatic-sync-runtime-followup.json). It also includes
the compact, full-width top bar and reduced editor top clearance.

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
Developer ID signing/notarization and distribution remain unverified. The final
source review and ad-hoc development bundle verification are complete; the
[final record](automatic-sync-final-review.json) binds the reviewed source, test
results, artifact hashes, remaining gates and ordered implementation rulings.


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

## Final branch review corrections

The final correction based on `44ded26` fixes permission-denied canonical PUTs
stranded as unknown, and repeated editor handoff reads behind immutable operations.
Permission errors now allow bounded readback; ordinary writes still require fresh
writable evidence and the existing guarded claim. Refresh reopens read admission
without restarting the worker. Fenced observations use their own per-page retry
delay, leaving unknown readback and recovery settlement independently eligible.
Immutable payloads, authentication guards and conservative unknown DELETE handling
remain covered by the existing suites.

The worker regression reproduced 543 recovery-fence GETs in 500 ms before the fix.
Afterward, canonical denial with writable metadata, canonical denial with read-only
metadata, and recovery-fence cases each measured zero GETs in that window (asserted
bound: three). Unrelated uploads progressed, the exact latest authored source was
replayed, and earlier drafts/displaced source and immutable recovery originals
remained preserved. Another test settles unknown recovery readback within 500 ms
while ordinary observation is still deferred, then uploads the latest source
without Refresh. Permission restoration passes both with and without Refresh.

Browser readiness now includes the current nonce and exact owned page. The browser
driver must write JSON with those same `nonce` and `page` values to `browser-done`
after its existing exact-source observation and session drain. Plain, stale,
wrong-page and missing-page markers cannot advance the phase or mark its summary
complete. The headless regression passed; the dated actual browser observation and
both three-path live cleanup receipts remain valid and were not rerun.

Build metadata now captures Cargo's selected `RUSTC --version` through
`cargo:rustc-env`, and the merge distribution embeds it at compile time. The test
build recorded Rust 1.99.1; the final MSRV build recorded Rust 1.92.0. This follows
the [Cargo build-script contract](https://doc.rust-lang.org/cargo/reference/build-scripts.html#rustc-env)
and [compiler environment contract](https://doc.rust-lang.org/cargo/reference/environment-variables.html#environment-variables-cargo-sets-for-build-scripts).
Existing raw benchmark samples actually used 1.99.1 and remain unchanged.

An affected crash test exposed an empty ready-file publication race. Both newly
added crash helpers now write their PID/nonce marker completely before publishing
it with a same-directory rename. The parent assertions are unchanged; both crash
parent tests passed afterward. This was a test fixture failure, not source loss.

After all code changes, one `scripts/check.sh` run passed formatting, strict
all-target/all-feature Clippy and **288 tests, zero failures, 24 explicit ignores
across 49 test binaries**. One Rust 1.92.0 `cargo check --all-targets --locked`
passed without warnings. Affected checks passed 91 tests across seven targets;
the two crash parents were additionally rechecked after their marker correction.
The older 281-test full run and 83-test scoped run above are historical evidence.

One fresh owned loopback native acceptance passed independent edits during typing,
overlap/exact recovery, unchanged-poll deduplication, offline restart, resumed
upload and delete/edit handling. Its relay rejected zero requests and confirmed
three guarded absences after both child processes exited. Event/attempt counts
stayed at 1/1 and kept-history count at 14 across unchanged polling. No live server,
browser, normal app database or Keychain was opened. The fresh overlap capture was
inspected; no additional floating/hover input assumptions were introduced.

The lock SHA-256 still matches the dated audit, with zero known vulnerabilities and
five existing maintenance warnings; no dependency, migration or notice changed.
The static/palette Slint convergence diagnostic remains an explicit investigation:
attribution unknown, no equivalent baseline and no demonstrated product failure.
The accepted floating contract is unchanged. No diagnostic was suppressed or
speculative UI fix made. Existing physical/native-fidelity and distribution gates
remain open. Subsequent scoped review and development packaging passed, as
recorded in [the final record](automatic-sync-final-review.json).

## Denied readback follow-up

Scoped review of `6fb0bf7` confirmed those four corrections, then found an uncovered
HTTP 403 readback path: recording a Permission error allowed the same unknown
operation to be read again immediately. Resolution now applies the existing
per-page retry before releasing admission when it returns Permission. The error
remains visible, the unknown payload stays immutable, and write/authentication and
unknown-DELETE guards are unchanged.

A real-worker regression reproduced 2,436 denied GETs in 1.3 seconds. The correction
measured two in each of two restoration variants (limit: four). During denial the
exact unknown operation remained, only the original PUT had been sent, and an
unrelated page uploaded. Restored access settled the queue without restarting,
both with Refresh and autonomously. Existing read-only metadata, recovery and
guard regressions also passed: 99 tests, zero failures, three explicit ignores
across six affected targets.

After this narrow code change, one new `scripts/check.sh` passed formatting, strict
Clippy and **289 tests, zero failures, 24 explicit ignores across 49 binaries**.
One new Rust 1.92.0 `cargo check --all-targets --locked` passed without warnings in
6.59 seconds. The `denied_readback_followup` JSON object records current source
hashes and evidence; `final_review_correction` explicitly retains the preceding
`6fb0bf7` hashes and 288-test evidence as historical. The lock hash and notices are
unchanged. No native, live-server or browser run was repeated for this error path.
Scoped recheck approved this correction at source `0f5a346`; the ARM64/macOS14
ad-hoc bundle passed signature and isolated Metal-launch verification. The
[final record](automatic-sync-final-review.json) contains the exact artifact and
source hashes. No installed application was replaced, and no merge or push ran.
