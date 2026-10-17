# Historical sync-readiness audit

Audited 2026-10-17 against feature-branch commit e473cb1 and `https://notes.harflix.lol`. Automatic synchronization remains required; it was not removed from scope. This audit distinguishes implementation status, observed deployed behavior, and remaining verification.

## Implementation follow-up

The audit below describes commit e473cb1, before the transport existed. Tasks 7–9 have since added the guarded HTTP adapter, connection verification, progressive full-space cache, and local-first automatic worker. See [protocol contract](protocol-contract.md), [connection security](connection-security.md), and [sync lifecycle](sync-lifecycle.md) for current evidence. Existing already-running local preview bundles are unchanged; they do not gain sync until replaced by a newly built app. Conflict review/Trash, remaining native observations, diagnostics/performance, and release packaging still have their own completion gates.

## Execution cause (at the audited commit)

The plan separated a local/UI milestone (Tasks 1–6) from remote synchronization (Tasks 7–10). Work stayed on native feasibility, theme UI, local saves, and the safe startup/quit prerequisite from Task 11. Unresolved editor selection capture and human native observations led to repeated local checkpoints. Neither depends on the HTTP transport or sync reconciliation algorithms. Treating those UI gates as reasons to stop independent remote work was an execution-sequencing mistake, not a demonstrated Slint or server limitation. The local preview's tests establish local behavior; they do not establish remote synchronization.

At the audited commit, the implementation had validated connection/page identities, remote revision types, durable local notes, pending mutation payloads, guarded-write conditions, acknowledgement handling, and crash recovery of leased operations. However, there is no `src/sync` module, HTTP transport dependency, protocol conformance target, background sync worker, or full remote catalogue downloader. `src/main.rs` passes a connection identity to local startup; that does not establish a network connection. LocalUi explicitly displays “Saved locally · not syncing.” The outbox is storage for planned remote changes; no transport consumes it yet. No app switch can enable the missing behavior.

## Read-only deployment evidence

Unauthenticated HTTPS requests with `X-Sync-Mode: true`, without following redirects:

| Request | Observed |
| --- | --- |
| `GET /.ping` | 200 |
| `GET /.config` | 200 JSON, `readOnly: false` |
| `GET /.fs` | 200 JSON; 66 catalogue files, 59 Markdown pages |
| One existing page, HEAD only | 200, `text/markdown`, permission `rw`, strong SHA-256 ETag and dedicated `X-SilverBullet-Revision`, modification metadata |

No existing page body was fetched. These observations establish API access and relevant metadata, not unconditional trust in writes.

## Real guarded-write probe

The planned path `Quick Notes/.bulletnotes-probe/<uuid>.md` returned 404 on create-only PUT. Its subsequent HEAD remained 404. Inspection of the [upstream router](https://github.com/silverbulletmd/silverbullet/blob/main/server/src/router.rs) identified `refuse_hidden_fs_paths`: any decoded slash/backslash-separated component starting with a dot is rejected before file handlers. The source also includes tests covering this rule. The path in our plan was incompatible; no server upgrade or access-policy change was necessary.

A new, visible, independently generated path was used:
`Quick Notes/Bulletnotes Probe/c5f447a6-7d50-4cbd-bfbf-88b52c111a7c.md`.
Only that owned file received writes/deletes; its bodies were generated strings containing the probe ID, CRLF, emoji, and CJK. The deployed behavior matched the [official conditional-write contract](https://docs.silverbullet.md/HTTP%20API):

| Check | Observed |
| --- | --- |
| Nonexistent probe before creation | 404 |
| Create with `If-None-Match: *` | 200 |
| Exact generated body and content hash | Match |
| Create collision on the same key | 412 |
| Update with matching revision | 200; revision changed; bytes exact |
| Update with the old revision | 412; current bytes preserved |
| Delete with the old revision | 412; current bytes preserved |
| Two concurrent updates using the same revision | One 200, one 412; one complete winning body |
| Cleanup delete with current matching revision | 200; subsequent HEAD 404 |
| Catalogue after cleanup | 66 files, 59 Markdown; owned probe absent |

All 13 recorded checks passed for the visible probe. This is evidence from one controlled deployment test, not proof of every concurrency/failure scenario. It does not cover a timeout after a successful write, process crashes during sync, out-of-band filesystem writers, authentication changes, or a later server upgrade. The app must establish its own connection-scoped capability evidence and test its transport/state machine. Neither the native app nor its current live preview was changed or connected by this diagnostic.

## Current upstream Rust client research

Upstream now includes `HttpSpacePrimitives` in [server-common/src/space/http.rs](https://github.com/silverbulletmd/silverbullet/blob/main/server-common/src/space/http.rs). It is a blocking filesystem adapter used by SilverBullet, including conditional operations and preference for the dedicated content-revision header over proxy ETags. It is not a published stable client SDK: [server-common/Cargo.toml](https://github.com/silverbulletmd/silverbullet/blob/main/server-common/Cargo.toml) marks the crate internal-only and `publish = false`. Reusing it would couple this app to the upstream workspace and still require our durable worker/reconciliation integration. No dependency decision or framework fork is made in this audit. The planned small adapter over maintained reqwest remains a reasonable option, informed by these official semantics.

## Work identified by the audit

1. Implement and fixture-test the typed HTTPS client: correct path encoding, raw bytes, dedicated content revisions/strong ETags, conditional mutations, classified errors, and redirects/auth privacy.
2. Connect the complete Markdown catalogue and progressive cache to local page browsing and search, reporting incomplete/offline availability accurately.
3. Drain the existing durable queue after local commits, with per-page serialization, read-back after unknown outcomes, revision-aware acknowledgements, and preserved base/local/remote conflict copies. UI changes while requests run must retain newer local edits.
4. Integrate connection/auth state and repeat the app's owned-path compatibility verification before enabling its automatic writes. Tests must include two clients, offline edits, restart, stale revisions, and timeout-after-success.

No server-side blocker was demonstrated by these checks. The remaining main cause is unimplemented client work, independently actionable from the remaining native UI gates. No native or sync task is marked complete by this audit.
