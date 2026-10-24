# Release acceptance follow-up

**Goal:** Verify the current approved v1, address demonstrated blockers, and keep
the release status tied to current evidence.

**Scope:** Existing Rust/Slint editor, Apple Silicon/macOS 14+, generated notes
and isolated stores. Preserve exact source, selection, viewport, undo and IME
behavior. No new feature scope, dependency upgrades, user-store changes or
distribution upload.

- [x] Reconcile the release/build documents with the latest position and native
  input acceptance. Check local links and diff whitespace.
- [ ] Run the native release benchmark. Replace its fixed startup delay with
  the existing `local-startup-ready` acknowledgement; retain bounded failure.
  Require a completed report before treating any timing as a baseline.
- [ ] Profile the measured large-note path before selecting an optimization.
  If a safe fix is justified, reproduce its failure, make the smallest change,
  then repeat correctness and release performance checks without relaxing targets.
- [ ] Capture and inspect current default/minimum/contrast UI states. Change
  visuals only for demonstrated defects within the approved design.
- [ ] Run application and renderer checks, affected native contracts, and a
  fresh isolated development-package verification. Record actual results and
  remaining manual/hardware/distribution requirements.

## User-directed deferrals and independent checks

The user subsequently deferred performance measurements, screenshots and
package-launch verification. Do not run those steps while completing this pass.

- [x] Verify Rust 1.92.0 locked all-target compilation and the two existing Python
  validation-runner failure regressions before dependency changes.
- [x] Regenerate attribution against the original lockfile and confirm it is
  byte-identical; run the two static architecture-rejection tests without launch.
- [x] Resolve the two fresh RustSec certificate-trust findings with precise
  updates to `rustls-webpki` 0.103.16 and `webpki-root-certs` 1.0.11 in the
  application lockfile. Preserve all unrelated dependency edges. The independent
  editor lockfile reports zero vulnerabilities and needs no update.
- [x] Repeat audit, application checks, Rust 1.92 checking and attribution
  generation after the targeted patch update; retain the unchanged independent
  editor's renderer evidence and record warnings and review.

## 24 October checkpoint

Startup acknowledgement and bounded visibility diagnostics are implemented and
reviewed. The native benchmark remains incomplete: zero-frame diagnostics and
read-only console checks establish that the desktop is locked. No timing or
visual acceptance was claimed and no speculative performance/UI fix was made.
The user has been asked for an unlocked desktop.

Application checks passed 396/0/24 and renderer checks passed 45/0/0, including
formatting and strict Clippy. All nine selected native fixture executables built
in release mode. The remaining unchecked steps depend on native acceptance or
external release resources. See the [validation record](../../validation/release-acceptance-2026-10-24.md).
