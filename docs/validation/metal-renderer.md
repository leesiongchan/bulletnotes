# Metal renderer evaluation — 2026-10-18

The user authorized evaluating and adopting a Metal-backed Slint renderer while
retaining macOS 14+ and Apple Silicon only. Production now explicitly selects
Winit + Skia + WGPU 29 with Metal as its only backend. OpenGL is no longer the
production rendering path. This changes neither the editor nor the OS minimum.

## Choice and maintenance

Slint 1.18.1 documents Skia/WGPU support and its public, version-specific
`unstable-wgpu-29` feature. That feature also supplies the rendering notifier used
by the application's first-frame startup. Selection is centralized, main-thread-only,
idempotent, and occurs before Slint components are created. Inherited
`SLINT_BACKEND` and `WGPU_BACKEND` cannot replace the explicit production choices.
The dev-only `renderer-review` feature permits a Skia/OpenGL comparison.

FemtoVG/WGPU and Skia/WGPU 30 trials could not resolve
`wgpu-core-deps-emscripten = ^30.0.0` in the available package set. Slint's supported
WGPU 29 route resolved without a framework fork, registry workaround or vendored
patch. WGPU 30 remains a transitive lockfile dependency; runtime selection is
explicitly WGPU 29. The public versioned API adds an upgrade review obligation;
Slint/WGPU upgrades must repeat native startup and Metal verification. No private
Slint editor API or unstable winit API is used.

References: [Slint Winit renderers](https://docs.slint.dev/latest/docs/slint/guide/backends-and-renderers/backend_winit/),
[Slint WGPU 29 API](https://docs.slint.dev/latest/docs/rust/slint/wgpu_29/),
and [Apple OpenGL deprecation](https://developer.apple.com/library/archive/documentation/GraphicsImaging/Conceptual/OpenGL-MacProgGuide/opengl_intro/opengl_intro.html).
The latest documentation may evolve; this implementation is pinned to Slint 1.18.1.

## Verification

All native fixtures use generated notes and temporary stores. No real notes,
Keychain credentials or deployed endpoint were accessed. Test host: Apple M4 Pro,
Mac16,11, 48 GiB, macOS 27.0.1 (26A434).

- `scripts/check.sh`: 174 passed, zero failed, 19 explicit ignored native/manual
  gates; strict all-target/all-feature Clippy and formatting passed.
- `rustup run 1.92.0 cargo check --locked --bin bulletnotes`: passed with the new
  locked renderer graph. Pinned development Rust remains 1.99.1.
- The explicit native renderer fixture passed first-frame startup, real
  `CAMetalLayer` detection in the application's public view/layer hierarchy, exact
  CRLF/tab/CJK/emoji source, four selected theme identities, edits, shutdown and
  reopening. It overrides an inherited incompatible `SLINT_BACKEND` value.
- Native generated-data sync, recovery/merge-draft durability, local callbacks,
  appearance persistence, installation smoke and signed LaunchServices lifecycle
  checks passed. The full-application renderer and lifecycle checks exercise the
  explicit production WGPU 29 selection; older widget-only fixtures use their
  configured Skia backend and do not prove that selection separately.
- The original OpenGL fixture failed the actual Metal-layer acceptance check.
  The release-verifier regression first failed because a signed ARM64 fixture
  without Metal acknowledgement was accepted; it passes after requiring both
  installation and Metal acknowledgements.
- Dependency audit: zero known vulnerabilities, five maintenance warnings.
  Rechecked all five with `cargo tree` for the default and all-feature Apple
  Silicon target: no selected paths. See [audit data](dependency-audit.json).
- Attribution regeneration passed after changing the renderer's verified Slint
  license clarification to Skia and regenerating runtime notices.

Paper and Graphite snapshots were inspected against the earlier OpenGL layout.
The layout and opaque surfaces remain consistent, with small font-rasterization
changes. Four snapshots were generated. The existing tab missing-glyph box remains
visible in both renderers; exact source bytes are preserved. Snapshot checks do
not establish full Antinote fidelity or native input/accessibility acceptance.

## Performance

Same host, optimized thin-LTO profile, 20 samples per interaction; p95 measures
Slint AfterRendering/GPU submission, not physical display presentation.

| Interaction | Fresh OpenGL/FemtoVG | Skia/WGPU 29/Metal | Target |
| --- | ---: | ---: | ---: |
| Activation | 14.280 ms | 11.411 ms | 150 ms |
| Cached short-page open | 25.296 ms | 17.833 ms | 100 ms |
| Cached search | 31.420 ms | 16.931 ms | 100 ms |
| Cached 1 MiB open | 152.452 ms | 132.203 ms | 100 ms |
| Catalogue reachable | 10,000/10,000 | 10,000/10,000 | Every page |

[OpenGL data](interaction-opengl-comparison.json), [Metal data](interaction-latest.json).
Both runs intentionally exit nonzero because the original 1 MiB target is unmet.
Metal is favorable on this host; the 132.203 ms result remains a release blocker.
Supported selection restoration measured 120.904 ms p95. Stage percentiles are
not additive. No restoration was removed to improve the benchmark.

## Targeted review and remaining gates

A read-only independent review of this renderer delta found no Critical or
Important defects. It verified pinned upstream selection behavior, callback
availability, retained AppKit traversal and the release-verifier condition.
Its minor finding was that theme snapshots alone could pass without a selected
identity assertion; the fixture now asserts each requested theme and passed again.
The reviewer did not execute GUI tests or review generated notices for legal
sufficiency, broader branch behavior, manual input, older OS behavior or signing.

IME, VoiceOver, undo/clipboard, caret and prior-app focus, fullscreen/Spaces,
actual macOS 14/15 execution, exact directional selection capture, tab rendering,
the 100 ms large-note target and Developer ID/notarized distribution remain open.
The user skipped AppKit editor prototyping; this work does not reopen it.
A development package is evidence of packaging and Metal startup, not production
readiness. See [release matrix](release-matrix.md).
