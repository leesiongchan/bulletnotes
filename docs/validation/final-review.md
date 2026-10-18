# Final branch review

An independent gpt-6-astra reviewer inspected b206e0a..911439a against the approved plan, spec, ledger, and five explicit review-focus classes. It used generated loopback/SQLite and pure coordinator reproductions; it did not access user notes, Keychain, or the deployed server. The author fixed all four behavioral findings in one regression-driven pass, without a second review.

## Behavioral findings

1. **Critical: unknown DELETE can delete a same-content recreation.** Independent generated reproduction deleted the replacement twice without conflict. Content-derived revisions cannot identify incarnations. Fix: a present unknown-delete readback requires review; absence acknowledges completion. Tests cover live and restarted workers with content-derived validators.
2. **Critical: merge authoring remains possible after the shutdown flush.** Review identified the asynchronous gap statically; the author's native keyboard/SQLite-contention fixture reproduced a lost accepted edit. Fix: freeze authoring before the first wait through complete hide/quit, and release on failure. The fixture also checks that a failed close restores editing and restart restores the durable draft.
3. **Important: failed uncached downloads retain a downloading message.** Propagate selected-key permission/resource/protocol failures without replacing the current editor. Tests cover classification, recovery, stale-key rejection, and cancellation through history navigation.
4. **Important: unavailable history entries block traversal.** Independent coordinator reproduction repeatedly selected a deleted entry. Skip confirmed PageMissing entries in both directions, preserving the old editor until a valid destination loads. Transient storage failures retain the entry for retry. Consecutive deletions are covered.

These severities were retained based on user effect. Verification passed: 171 tests, zero failures, 18 explicit ignored gates; strict all-target/all-feature Clippy and formatting. The generated native merge/restart, offline sync/restart, and lifecycle fixtures passed, as did the ad-hoc signed LaunchServices lifecycle check. Production code checks with Rust 1.92.0. Each behavioral regression was observed failing before its fix. No production-readiness claim follows from these results.

All chronological execution decisions are preserved in [the decision record](implementation-decisions.md).

## Deferred minor

Search rows omit the specified modification indicator. Stored timestamps are retained. This is deferred as presentation work under the execution workflow; it does not enter the behavioral fix pass.

## Rulings on matters the reviewer declined to judge

Each ruling retains its cost if wrong.

- Physical IME, directional selection, undo, VoiceOver, focus, Spaces/fullscreen, and trackpad behavior remain open acceptance gates. Generated fixtures do not waive physical observations. Cost: the editor/native adapter may need additional work before release.
- Native performance remains open: measured 1 MiB opening exceeds the original 100 ms target. Do not replace the editor or relax the target without approval. Cost: delayed release or an approved architectural change.
- Older supported macOS execution on Apple Silicon, Developer ID signing, notarization, and distribution remain unobserved or unavailable. Intel was included at review time and removed from scope by the user on 2026-10-18, so Intel execution is no longer an acceptance gate. Static/ad-hoc artifacts establish development packaging only. Cost: supported hardware/signing failures may still require packaging changes.
- Current deployed reachability is unverified after DNS timeouts; this proves neither server failure nor incompatibility. Preserve the endpoint and retry only with working connectivity. Cost: deployment-specific behavior remains unobserved.
- Real Keychain and user-note behavior remain manual acceptance work. Generated isolated data proves the exercised boundaries only. Cost: permission UX or real-space edge cases may need changes.
- Inline Markdown colors, native blur, and Antinote theme import remain approved deferrals. Cost: initial visual/theme compatibility is incomplete relative to Antinote.
- A dedicated forget-credentials/sign-out control is outside the approved credential replacement/public-access contract. Do not add account management in this pass. Cost: users currently use credential replacement or public access instead of an explicit forget action.
- Do not sniff Markdown for HTML: valid Markdown can contain HTML. Retain strict supported-media and mutation acknowledgement checks. Cost: a server that deliberately mislabels an HTML response as valid Markdown cannot always be distinguished from a legitimate note.
- Current literal search semantics remain documented; case-insensitive/prefix search is a product extension. Cost: users may need exact literal terms.
- Attribution generation is verified for recognized components and exact upstream text, but this is not a legal audit. Preserve full notices and the About widget; no legal-clearance claim. Cost: distribution may require attribution/license corrections.
- Screenshots are not fresh visual acceptance. The optional About capture failed; observed native window behavior does not establish visual fidelity. Cost: human visual review may identify defects before release.
