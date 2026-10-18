# Native responsiveness and diagnostic evidence

Measured 2026-10-18 with the native Rust/Slint application, using a temporary
10,000-page SQLite space and a 1,048,576-byte note. No live notes or deployed space
were accessed. Hardware: Apple M4 Pro, Mac16,11, 48 GiB; macOS 27.0.1 (26A434),
Rust 1.99.1, Slint 1.18.1, Winit/Skia on WGPU 29/Metal, optimized Cargo bench profile. Twenty
samples per interaction; p95 uses the nearest-rank method. The frame boundary is
Slint AfterRendering/GPU submission, not a physical display-presentation probe.
The latest release profile enables thin LTO, one codegen unit, and stripped debug
information; panic unwinding remains enabled for the native callback boundary.

Run `rustup run 1.99.1 cargo bench --bench interaction --locked`. It writes
`target/interaction/latest.json` and exits nonzero when any target is unmet.
[Initial baseline](interaction-baseline.json) and
[latest measurement](interaction-latest.json) preserve the actual results.

| Interaction | Initial p95 | Latest p95 | Target | Result |
| --- | ---: | ---: | ---: | --- |
| Warm activation | 31.915 ms | 11.411 ms | 150 ms | Met on this host |
| Cached short-page open | 17.433 ms | 17.833 ms | 100 ms | Met on this host |
| Cached literal search | 33.505 ms | 16.931 ms | 100 ms | Met on this host |
| Cached 1 MiB note open | 250.162 ms | 132.203 ms | 100 ms | **Open release blocker** |
| Catalogue reachability | 5,000/10,000 | 10,000/10,000 | Every page | Met through result continuation |

The original UI hard limit exposed only 5,000 pages. The native harness reproduced
it; result continuation now loads bounded 200-row batches through **More pages**.
Coordinator regressions verify append, stale-query rejection and preserving loaded
results on a failed continuation. Search does not copy/render the entire catalogue
on each new query. The list remains virtualized, and the harness exercised every
batch until all 10,000 rows were reachable.

Classified stage timing located the large-note cost in the supported Slint
selection-restoration call: 120.904 ms p95. Cached storage opening measured
8.362 ms, setting the text property 0.611 ms, and editor focus <0.001 ms. The
large-note interval before the next frame was 127.982 ms p95, with a subsequent
frame interval of 4.222 ms. Stage percentiles have different sample populations
and are not additive. These numbers establish where the measured cost occurs;
they do not establish a supported framework fix. The benchmark continues failing
its original 100 ms requirement. No restoration was removed, private editor API
used, or editor rewrite introduced to change that result.

A fresh same-host OpenGL/FemtoVG comparison measured 14.280 ms activation,
25.296 ms cached open, 31.420 ms search and 152.452 ms large-note open. Both runs
used the same optimized profile and 20 samples. The Metal results are favorable
on this host; they do not establish a speedup on every supported machine.
[OpenGL comparison data](interaction-opengl-comparison.json) and
[Metal evaluation](metal-renderer.md) retain the context. Both runs failed the
unchanged 100 ms large-note gate.

## Local diagnostics

Operational questions are whether local durability succeeded, why synchronization
is paused, and which stage took time. Events accept only opaque UUIDs, enumerated
operations/outcomes, microsecond durations and counts. Raw errors are classified
before construction. A scoped tracing subscriber receives only the application's
typed records; no global subscriber collects HTTP-library logs or foreign spans.
A 1,024-event lossy non-blocking queue keeps telemetry off the UI I/O path; the
guard exposes its dropped-event counter. Diagnostic loss cannot acknowledge or
cancel a note save.

Logs rotate daily. Startup pruning and a stoppable midnight UTC maintenance worker
retain seven calendar dates even across quiet days without new rotations. Only
strictly named application log files are pruned; unrelated files remain untouched.
Explicit local export validates typed records, caps reads at 64 MiB, and atomically
replaces the destination only after successful reading/writing. It includes
complete records already on disk; in-flight diagnostic events may still be queued.
The destination cannot be an active log. Failed validation or replacement preserves
existing destinations and source logs. Tests inject note/token/path sentinels and
confirm they do not reach exported records, including foreign tracing messages.

## Dependency audit

Verified the current tool version from RustSec's
[official manifest](https://github.com/rustsec/rustsec/blob/main/cargo-audit/Cargo.toml)
and ran cargo-audit 0.22.2 via Nix comma against Cargo.lock. Advisory database commit
`999ec89d5d3f10f20be14750ec8c2d8b44e0ac9b`, updated 2026-10-17, contained
1,338 advisories. **No known vulnerabilities** were reported in the locked graph;
[structured results](dependency-audit.json) retain the five maintenance warnings.

| Package | Warning | Assessment |
| --- | --- | --- |
| bincode 2.0.1 | RUSTSEC-2025-0141 | Optional serialization dependency of typed-index-collections; absent from selected macOS default/all-feature graph |
| ttf-parser 0.25.1 | RUSTSEC-2026-0192 | Through owned_ttf_parser; absent from selected macOS default/all-feature graph |
| zbus_names 4.3.5 | RUSTSEC-2026-0371 | Linux/D-Bus accessibility dependency; absent from selected macOS graph |
| zvariant 5.15.1 | RUSTSEC-2026-0370 | Linux/D-Bus accessibility dependency; absent from selected macOS graph |
| zvariant_derive 5.15.1 | RUSTSEC-2026-0372 | Linux/D-Bus derive dependency; absent from selected macOS graph |

`cargo tree --locked --target aarch64-apple-darwin -i PACKAGE` with and without
`--all-features` confirmed no selected path for these warnings. No advisory was
silenced. Repeat this audit for the final release lockfile and each distribution
target; new dependencies or feature selections can change reachability.
