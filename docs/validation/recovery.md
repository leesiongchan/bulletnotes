# Conflict review and recoverable deletion

Automatic reconciliation handles compatible edits, overlaps, no-common-base creates and delete/edit races without a mandatory prompt. Overlaps keep the observed server passage plus independent local edits; a separate create-only recovery page preserves the complete original. Immutable upload/readback obligations keep status pending until both canonical and preservation work are acknowledged. See [current acceptance](automatic-sync-acceptance.md).

Optional Changes, Trash and Kept versions remain available from the floating note footer (F6 can reveal controls). The native note editor stays mounted when opening or leaving it. All palettes, including imported themes, supply its paper, source panes, selection, labels, and controls. Minimum-size Paper and Graphite captures were inspected with generated text; they do not establish VoiceOver or IME correctness.

Changes shows separately labelled local, remote, and common-base source, plus an editable merged result. A deleted remote page has an explicit deletion label; an existing empty page remains a present document. Use remote, Keep local/Recreate local, and Use merge are distinct intents. Legacy retained records use review IDs to reject stale actions; automatic reconciliation closes admission and drains the latest authored draft before checked consumption. Reload retains the merged draft. Local revision guards also reject a review if local source changed.

Merged drafts coalesce into SQLite within 250 ms. Normal native hide and quit drain those writes before closing. A failed draft save keeps the source available and prevents closing; retry can complete after storage recovers. Resolution keeps the prior local, remote, common-base, and authored merged draft as labelled, dated Kept versions. Version bodies load only for the selected item; the list uses metadata.

Moving a note to Trash is an explicit editor context-menu action. Its source, base, and pending operation snapshots commit before any conditional deletion can run. Empty existing pages never disappear because they are empty. If an upload is already in flight, deletion waits for its acknowledged revision. A deletion whose result is unknown reads back before retry, and never deletes a replacement that reused the path. An uncertain create leaves durable deletion evidence so a late arrival meets checked preservation/reconciliation. Retired DELETE watches repair late server effects without deleting an indistinguishable same-content recreation. History cannot reopen trashed/deleted snapshots for ordinary editing.

Trash and Kept versions restore with create-only intent, including an explicitly restored empty document. An occupied local path requires an explicit destination. Server collisions, including identical source at a reused restore path, are reconciled under the same create-only guards and preservation policy. A known hidden remote-deletion snapshot can be restored to its original path using the same create-only guard. Restoration retains its original recovery copy. Trash purge requires an exact selected ID and an explicit in-app confirmation; it does not run on a timer or cancel queued remote deletion.

## Evidence

The storage contracts cover stale review IDs, retained base/local/remote/merge source, empty-versus-missing semantics, source/base/queue persistence across restart, immutable upload→delete ordering, create-only restoration, identical-content path reuse, late creates after deletion intent, and an already-editing buffer racing a remote deletion.

The multi-file loopback fixture keeps independent per-file revisions and atomic guards. It verifies a timed-out deletion followed by path recreation, further remote changes after a merge, unrelated-page progress during conflict, and resumption after resolution. This fixture uses only generated source, unlike the earlier single-file protocol fixture.

The `recovery_review` native fixture exercises the actual window/coordinator/store and generated server. It checks review labels, leaving review, incoming changes, reload without losing a merge, upload, deletion, normal quit, separate-process restart, and exact create-only restoration. An independent native process edits a merged draft and immediately quits before its timer; an injected SQLite transaction failure keeps the buffer and store open, then retry succeeds and a separate process restores the unapplied draft into review. Exact CRLF, CJK, and emoji source is compared throughout.

```sh
rustup run 1.99.1 cargo test --test conflict_recovery --test sync_presentation --features protocol-validation --locked
rustup run 1.99.1 cargo build --example recovery_review --features native-validation,protocol-validation --locked
rustup run 1.99.1 cargo test --test conflict_recovery --features native-validation,protocol-validation --locked native_review_merge_delete_restart_and_restore_preserve_source -- --ignored --exact
```

The Task 10 checkpoint passed **152 automated tests, 0 failures, 17 explicit ignores**, with native recovery, sync/restart, and existing close/termination checks additionally run explicitly. Strict all-feature Clippy, the default all-target check, formatting, and diff checks passed.

Real keyboard-only review, VoiceOver selection/editing, and IME composition remain native observations to collect. All native and network fixtures use generated data; live preview bundles and the user database remain untouched. This milestone does not claim production-release readiness.
