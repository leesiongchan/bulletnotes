# Automatic sync reconciliation design

Status: proposed specification; implementation has not started.

## Intent and decision

The user requires automatic synchronization with the existing SilverBullet space,
including browser and external-file edits, without a mandatory conflict-resolution
step. Notes must remain readable and competing source must remain recoverable.
On 2026-10-18 the user delegated the overlap policy: follow established industry
practice. This specification selects compatible-edit merging with separate synced
recovery copies for incompatible edits. It does not claim one universal industry
policy or that preserving an alternative reconstructs its intended meaning.

Keep ordinary Markdown and conditional file writes as the interoperability
boundary. Use three-way merging against the last shared source. Do not introduce
a CRDT operation store or require changes to SilverBullet's browser or server.

The floating header/footer request is a separate UI change. Its hover behavior
must not own reconciliation or change when a save becomes durable.

## Research and alternatives

- [SilverBullet collaboration](https://docs.silverbullet.md/Collaboration) uses
  line- and word-aware three-way merging. Genuine overlaps produce conflict
  markers; concurrent edits survive deletions. Its marker fallback does not meet
  this application's requirement to avoid mandatory resolution.
- [Obsidian Sync](https://obsidian.md/help/sync/troubleshoot) defaults to automatic
  Markdown merging and documents possible duplicate text and formatting problems.
  Its alternative conflict-file mode keeps the remote original and local copy.
- [Dropbox](https://help.dropbox.com/organize/conflicted-copy) preserves a competing
  edit as a separate conflicted copy.
- [Syncthing](https://docs.syncthing.net/users/syncing.html#conflicting-changes)
  selects a canonical file and propagates the competing version as an ordinary
  conflict file. Its choice of winner differs from Dropbox's and from this design.
- [Yjs document updates](https://docs.yjs.dev/api/document-updates) and
  [Automerge conflicts](https://automerge.org/docs/reference/documents/conflicts/)
  describe convergence over shared CRDT history. A native-only CRDT would still
  need a bridge for arbitrary Markdown snapshot edits from nonparticipating tools.

These sources support the preservation pattern, not the exact policy below.
Keeping the observed server passage on an incompatible overlap is this product's
choice: it avoids repeatedly replacing a committed passage with an offline edit.
Modification timestamps do not determine which source wins.

## Resolution policy

1. Preserve the existing one-sided and identical-source fast paths.
2. With a common base, merge compatible edits from both sides. Refine line-level
   collisions at word level where safe. Keep unrelated local edits even if a
   different passage conflicts.
3. For an incompatible overlap, retain the observed server passage in the main
   note and keep the complete original local version as a separate recovery page.
   Preserve base, local, remote and chosen result in local kept-version history.
   Do not concatenate incompatible passages or inject conflict markers.
4. Without a common base, differing creates at the same path retain the observed
   server document; preserve the complete local document as a recovery page.
   A genuinely missing remote remains a create-only operation, including empty
   documents created explicitly by the user.
5. A deletion against an unchanged document proceeds under existing guards. A
   concurrent edit survives deletion: cancel superseded deletion intent or
   recreate the edited document with create-only semantics. Preserve the deletion
   and source history. If the path becomes occupied, reconcile its current source.
6. An unknown DELETE followed by a present file cannot establish file incarnation,
   even with identical bytes. Retain that file and retire the uncertain delete
   intent after preserving its evidence; never retry destruction on that evidence.
7. A merge that exceeds a fixed resource budget uses the same whole-document
   server-original/local-recovery fallback. It must not block other pages or the UI.

No strategy can infer the intended meaning of contradictory edits. Completion
here means automatic canonical selection and durable preservation, with optional
later review. It does not promise a semantically correct combination of every edit.

## Recovery copy contract

Recovery pages live in
`Bulletnotes Recovery/<original-name>-<event-id>.md` in the same space. The original
name is a bounded, validated filename stem; use `note` if it has no usable stem.
The page body is the exact original local Markdown, without generated headings,
frontmatter or wrappers. The original path, observation, reason and timestamps
belong in local recovery metadata. Do not reproduce the original folder hierarchy;
the readable stem and unique event identifier keep destinations distinguishable.

Persist each event identifier, destination and immutable source once. Reobserving
the same connection/path/base/local/remote tuple reuses its event. A new competing
edit creates a new event. Upload with `If-None-Match: *`; read back an uncertain
result before retry. Matching bytes acknowledge an existing copy; different bytes
at that destination select and persist another destination without overwriting it.

Recovery pages are normal browsable, editable, synced Markdown. Creating or
receiving an unchanged recovery page must not generate another recovery event.
Later concurrent edits to that page use the same reconciliation policy. Copies
are retained until explicitly removed; no automatic expiry or cross-device
deduplication based only on similar text is introduced.

In one SQLite transaction, persist the originals and recovery upload obligation
before installing the resolved source, updating its base and replacing eligible
queued work. Failed persistence leaves the prior state and work intact. Report
sync pending until both canonical and recovery obligations are acknowledged.
Offline or denied recovery uploads remain locally durable and visibly pending or
failed; they must never be reported as fully synced or silently discarded.

## Integration and ordering

- Extend the pure decision boundary in `src/sync/reconcile.rs` to describe merged
  source and preservation obligations. The decision carries no network or SQLite
  side effects. Use a maintained merging library; dependency selection and its
  bounded execution strategy must be established in the implementation plan.
- Apply decisions transactionally in `src/store/remote.rs`, reusing source-history
  machinery from `src/store/restoration.rs`. Do not invoke the existing manual
  resolution action from a background worker: it changes revision and selection
  assumptions that the open editor currently depends on.
- Route ordinary observations, stale editor saves and divergent readback after an
  unknown PUT through the same policy. Existing leased/unknown payloads remain
  immutable until their outcome has been determined.
- Use existing guarded PUT/DELETE operations. On a precondition failure, read the
  latest remote and recompute; never upload against an obsolete revision. Keep
  bounded retries and backoff. Do not use SilverBullet's upstream POST reconcile
  endpoint for this version: it can commit markers before the client applies its
  preservation policy, and deployed support has not been verified.
- Authentication, permissions, validators and protocol validation retain their
  existing meaning. Automatic content resolution does not authorize an otherwise
  forbidden upload. A read-only space preserves local work and exposes the failure
  without asking for a content-resolution choice.

## Open editor contract

Reconciliation must include typing that has not reached the store. A clean editor
may accept a revision-tagged resolved snapshot after checking its page and source
generation. A dirty or writing editor rebases its current buffer against its
acknowledged baseline and the newly resolved source through the same policy.
Do not replace the buffer with an older stored snapshot or acknowledge later typing
as saved by an earlier completion. A stale completion is rejected and recomputed.

The current equality guards in `src/app/local_ui/synchronization.rs` and
`src/app/editor.rs` cannot simply be relaxed. Introduce an explicit rebase event
whose expected baseline and result are checked by the coordinator/editor. Preserve
editing admission, map available selection/scroll observations through changes,
and retain the existing editor instance. Never claim exact directional-selection
or undo fidelity without native evidence; those existing release gates remain.

Existing unresolved records are automatically reconsidered once their mutation
outcomes are known. Preserve authored merge drafts before replacing a conflict
record. Drain admitted draft writes and revision-check their acknowledgement before
committing resolution; a newer draft invalidates that attempt. Preserve the draft
as a local kept version and a separate synced recovery page. Do not automatically
apply unfinished draft text to the canonical note. Kept-version browsing remains
optional.

## Acceptance and verification

- Independent paragraph edits and compatible same-line edits both survive without
  review. Incompatible passages retain the server side while independent local
  changes survive and the full alternative is available as a synced page.
- Cover same-path creation, missing versus empty source, delete/edit in both
  directions, unknown DELETE with identical-content recreation, and server changes
  during merge. No replacement file is destroyed by an uncertain old delete.
- Compare exact CRLF, CJK, emoji, missing final newline, frontmatter and code-fence
  source. Unaffected bytes are retained; fallback copies match the original bytes.
- Exercise crashes and restarts before/after local commit, recovery creation and
  canonical upload. Retried/uncertain operations reuse persisted recovery identity
  and never overwrite an occupied recovery path.
- Inject local storage failure, denied recovery upload, missing validators and
  merge-budget exhaustion. Preserve originals and report truthful status; unrelated
  pages continue syncing.
- Exercise typing during remote observation, during an in-flight local save and
  immediately after a rebase. Older completions cannot discard newer edits or
  strand the editor behind a stale revision.
- Reconsider existing conflicts and preserve authored drafts. Repeated unchanged
  observations do not multiply recovery pages or create sync loops.
- Extend the existing reconciliation, recovery and sync-lifecycle suites, then run
  strict repository checks and generated-data native fixtures. Verify one owned
  browser/native round trip, including overlapping offline edits and cleanup.
  Tests must not author or inspect unrelated real notes.

Any necessary additive schema migration must retain the existing backup/rollback
workflow and legacy recovery records. No runtime behavior or schema is changed by
this specification. Implementation requires review of this document and its plan.
