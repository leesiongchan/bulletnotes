# Automatic sync lifecycle

The current main application connects after native local startup. Local editing, navigation, hiding, and quitting depend on SQLite acknowledgements, never a network response. The older already-running local preview bundles remain unchanged.

## Causality and preservation

A first-dirty 250 ms local save precedes a per-page 750 ms remote debounce. Four bounded network lanes share per-page admission; a stalled page cannot reserve all lanes. Selected uncached pages take priority over queued background reads. Complete validated catalogues populate metadata progressively, with search completeness and cache counts exposed while bodies download. A failed listing never implies deletion.

Pending operations have immutable leased payloads. An acknowledgement advances only the sent local revision and base; newer local text stays pending. Timeouts, ambiguous responses, and 5xx outcomes require readback before retry. Revision guards use server-supplied opaque strong revisions. Clean remote updates install transactionally; dirty changes and remote deletion retain base/local/remote conflict copies. An editor baseline includes both its last committed source and original remote base, protecting unsaved text against incoming updates without mistaking our own acknowledgement for a foreign edit.

The UI derives status from its current dirty buffer, durable revision, pending operations, conflict state, and connection evidence. A newer unsaved buffer cannot display Synced. Remote metadata-only updates preserve stored selection, while changed text clamps persisted UTF-8 offsets. This does not establish exact live Slint caret capture.

Foreground/background polling uses 30/60 seconds, with immediate activation and explicit refresh. Initial offline connection checks retry automatically; worker retries use exponential jitter capped at 60 seconds. Authentication, permission, compatibility, and resource failures pause rather than spin. A regression reproduced over a thousand catalogue requests in 100 ms before the nontransient pause was corrected. A per-page failure leaves unrelated work eligible.

A failed temporary compatibility probe retains its exact generated identity, including after an absence read. Reconnect reuses that identity and verified workers recheck it for late arrival. Cleanup requires the exact generated marker source and strong guarded deletion; foreign replacements survive. Read-only workers do not mutate. Clean successful new probes remove their journal entries.

Shutdown cancels in-flight requests and marks leased operations unknown before SQLite closes. A normal native quit first commits the latest body and serialized settings, drains local queues, and closes SQLite. The remaining Tokio runtime shutdown is bounded at 100 ms to avoid waiting indefinitely on noncancellable OS credential/DNS jobs. Local durability does not depend on that runtime drop.

## Verification

Generated loopback and real SQLite tests exercise exact source bytes, debounce boundaries with an injected monotonic clock, deterministic retry jitter, old acknowledgements, timeout-after-success, delayed probe creation, malformed proxy acknowledgements, catalogue failure, bounded parallel downloads, selected-page priority, per-page serialization, connection isolation, read-only operation, nontransient pauses, unsaved-buffer races, and offline navigation intent.

The separate native `sync_review` fixture uses the actual Slint window, local coordinator, temporary SQLite, and generated loopback server. It checks initial offline retry while saving text, automatic upload, another client’s remote update, offline CRLF/emoji/CJK edits, reconnection, local-first quit, and a separate-process restart recovering the exact final source. It never opens the live user database or contacts the deployed space. The connection-form fixture separately checks masked/cleared input and source preservation.

The compiled compatibility verifier also passed against isolated official SilverBullet 2.12.1 and the deployed endpoint. It reads the file catalogue and its exact generated probe only; no existing note bodies are downloaded or changed by that external check.

```sh
rustup run 1.99.1 cargo test --all-targets --all-features --locked
rustup run 1.99.1 cargo clippy --all-targets --all-features --locked -- -D warnings
rustup run 1.99.1 cargo check --all-targets --locked
rustup run 1.99.1 cargo build --example sync_review --features native-validation,protocol-validation --locked
rustup run 1.99.1 cargo test --test sync_presentation --features native-validation,protocol-validation --locked native_two_client_offline_reconnect_and_restart_preserve_source -- --ignored --exact
```

The Task 9 checkpoint passed **137 tests, 0 failures, 16 explicit ignores**, plus both named native checks and both official/deployed exact-owned-probe verifications run explicitly. Strict all-feature Clippy, the default all-target check, formatting, and diff checks passed.

## Remaining gates

Conflict review and Trash restoration are implemented and tested in the [recovery milestone](recovery.md). Real IME composition during an incoming update, VoiceOver editing, undo behavior, exact live selection capture/restoration, previous-app focus and full-screen/Spaces behavior still need observed evidence. Diagnostic privacy/retention, 10,000-page and 1 MiB performance measurements, and signed clean-install/upgrade validation remain release work. Current search results are bounded to 5,000 rows; large-space browsing needs the planned scale assessment. No production release or native fidelity signoff follows from the automated sync evidence alone.
