# Secure connection and compatibility evidence

Connection setup uses a canonical credential-free HTTPS URL. The optional bearer token has redacted Debug output and is never serialized into application settings, SQLite, or diagnostic messages. The themed Connection view is reached from Appearance, masks token input, clears it after submission and when leaving the view, and preserves the existing note editor. Network operations run on Tokio; security-framework 3.7.0 generic-password calls run through `spawn_blocking`.

Keychain entries use service `lol.harflix.bulletnotes.silverbullet.bearer.v1` and the canonical connection identity as account. Missing entries are normal. A denied or unavailable Keychain produces a warning while the supplied token remains usable in memory. Choosing a session with a newly supplied token and no persistence requests removal of a previously saved token; any failure is surfaced. Denied credential loads are remembered for this process session, preventing automatic network retries from repeatedly requesting OS access. An explicit successful save/delete clears that denial. No fallback credential file exists.

Read access alone does not grant write capability. Each client construction has an independent opaque instance ID. `CompatibilityEvidence` is created only after all six checks pass and cannot authorize another client instance, even at the same URL. Clones of that validated client retain the same identity.

## Owned temporary-page protocol

The controller commits the exact generated `Quick Notes/Bulletnotes Probe/<v4 UUID>.md` identity to a dedicated SQLite journal before making any mutation. Migration 002 adds only that journal and backs up an existing database through SQLite. Upgrade coverage preserves the original CRLF/Unicode body and immutable pending operation.

The verifier creates generated source A with `If-None-Match: *`, changes it to B with the returned strong revision, rejects a stale update and deletion, rejects a create collision, reads back exact B and its revision/permission, guarded-deletes B, and confirms absence. Any failure withholds evidence. Timed-out or failed operations leave the journal entry across restart. A later absence read cannot retire that uncertainty: a delayed create could still arrive. Reconnect reuses the historical generated key, and verified workers check these durable cleanup watches on successful catalogue polls. Read-only workers never perform cleanup mutations. Cleanup reads only that exact key, requires one of its generated marker bodies and a strong current revision, and uses a guarded delete. Replacement content is retained and cleanup remains pending. Neither wildcard cleanup nor unconditional mutation exists.

The main application’s Connect action activates the progressive cache and automatic worker after verification. A readable connection without matching evidence stays read-only. Initial transient network failures retry automatically with bounded jitter; authentication/permission/compatibility failures require resolution. The standalone validation form can still run without activating a worker. See [sync lifecycle](sync-lifecycle.md). No release/native editing gate is waived by connection verification.

## Evidence, 2026-10-17

- Stateful fixture: ignored preconditions withhold evidence; denial retains authenticated sessions; redirect destination receives zero requests; connection-scoped outboxes remain isolated; timeout cleanup survives restart; replacement content survives cleanup; form validation rejects insecure URLs and header injection.
- Compiled controller/verifier against the official arm64 SilverBullet **2.12.1-0-g4d5eac53-2026-10-12T06-21-21Z**: passed, with guarded cleanup confirmed. The GitHub release archive SHA-256 was checked as `cd83db88e1759e929eadd5f78130a41f0d4c8ed150a3fa79c914fb8985721c37`. The server bound only to 127.0.0.1 with a separate generated space.
- The same compiled verifier against **notes.harflix.lol**: passed; only the file catalogue and its own generated probe were read, and only that probe was written/deleted. The deployed release identity was not established by this check.
- Native generated-text form contract: passed; invalid input does not start requests, submitted/abandoned tokens clear, and source text survives the round trip. Paper and Graphite captures were inspected at 400×440. This does not substitute for VoiceOver or real IME testing.
- Task 8 checkpoint all-targets/all-features suite: **105 passed, 0 failed, 15 explicitly ignored** native/external checks. The new native check and the external verifier were additionally exercised explicitly. The real Keychain round trip remains ignored, as the plan requires unless requested. Strict Clippy and default-feature compilation passed.

```sh
rustup run 1.99.1 cargo test --test connection_security --test compatibility_probe --features protocol-validation --locked
```

## Official references

- [Apple Keychain services](https://developer.apple.com/documentation/security/keychain-services)
- [SecItemAdd blocking behavior](https://developer.apple.com/documentation/security/secitemadd(_:_:))
- [security-framework password APIs](https://docs.rs/security-framework/3.7.0/security_framework/passwords/index.html)
- [SilverBullet 2.12.1 release](https://github.com/silverbulletmd/silverbullet/releases/tag/2.12.1)
