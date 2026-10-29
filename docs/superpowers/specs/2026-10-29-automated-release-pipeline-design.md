# Automated release pipeline design

## Goal

Every merge to `main` that contains a user-facing change publishes a GitHub
Release automatically, with no human step: version computed from commits,
five-target installers built, release notes generated, tag and release
created. Merges without user-facing changes publish nothing.

## Decisions

| Topic | Decision |
|---|---|
| Commit convention | Conventional Commits on every commit (merges are rebase-only), linted on PRs |
| Version source of truth | The git tag. Nothing is committed back to `main` |
| What releases | `feat`, `fix`, `perf`, and breaking changes. `docs`, `chore`, `ci`, `test`, `style`, `refactor`, `build` do not |
| Release type | Normal release marked Latest (no longer a prerelease). Builds stay unsigned/ad-hoc and labelled `development` |
| Version stamping | Inside `scripts/package-desktop.py`, after provenance capture |
| Changelog | GitHub Release bodies only; no committed `CHANGELOG.md` |
| cargo-edit | Not used (see Research) |
| Provenance | `actions/attest` build-provenance attestations on published assets |

## Research findings

Verified on 2026-10-29.

- **git-cliff 2.14.2.** With no config, non-conventional commits such as
  `Fix race in test` produce no bump, and git-cliff prints the existing
  tag. 75 of the 104 existing commits do not parse. With the `[bump]`
  settings below, tested in a scratch repo starting at `v0.1.0`:
  - `docs`, `ci`, `refactor` stay at `v0.1.0`.
  - `fix` gives `v0.1.1`.
  - `feat` gives `v0.2.0`.
  - `feat!` gives `v0.3.0`.

  With no tags at all, `--bumped-version` returns `initial_tag`
  (`0.1.0`), whatever the commits are.
- **Commit linting.** git-cliff's `require_conventional = true` (with
  `filter_unconventional = false`) exits 1 with `UnconventionalCommitsError`
  when a range contains a non-conventional commit, and exits 0 otherwise.
  No commitlint or Node toolchain is needed. A catch-all `skip` commit
  parser hides non-conventional commits from this check, so the lint must
  use its own minimal config rather than the release `cliff.toml`. The
  `initial_tag` value must include the `v` prefix to satisfy
  `tag_pattern`.
- **cargo-edit 0.13.14.** `cargo add` and `cargo rm` moved into cargo itself.
  `cargo set-version` and `cargo upgrade` remain cargo-edit only. Changing
  the `Cargo.toml` version alone breaks every `--locked` build. Both
  `cargo set-version` and a version edit followed by
  `cargo update -p <crate>` keep `--locked` working. cargo-edit ships no
  prebuilt binaries and is not in `taiki-e/install-action`, so CI would
  compile it from source on each runner just to replace two lines.
  `cargo upgrade` overlaps with Dependabot or Renovate. Verdict: not
  adopted.
- **Release-PR tools are rejected.** release-plz and release-please need
  a human to merge a release PR, which conflicts with "no human
  intervention".
- **Workflow triggers.** Tags pushed with `GITHUB_TOKEN` do not trigger
  other workflows, so the pipeline cannot hand off through a tag-push
  trigger. The `Main` ruleset (PR required, rebase-only, admin bypass only)
  blocks bot commits to `main`. The tag-only model needs neither.
- **Attestations.** `actions/attest-build-provenance` v4 is a wrapper; its
  README directs new setups to `actions/attest` (v4.2.2).

## Components

### `cliff.toml` (new, repo root)

- `[git]`: `conventional_commits = true`, `filter_unconventional = true`,
  `require_conventional = false`, `tag_pattern = "^v[0-9]+\\.[0-9]+\\.[0-9]+$"`.
- Commit parsers:
  - `feat` goes to Features, `fix` to Fixes, `perf` to Performance.
  - Breaking commits of any other type go to a Breaking changes group.
    Breaking commits are also marked in the notes.
  - Everything else is skipped. This catch-all skip is what keeps
    non-releasing types from bumping.

  Pre-convention history is therefore skipped too.
- `[bump]`: `initial_tag = "v1.0.0"`. Releases start at 1.0, so the 0.x
  bump options do not apply.
- Body template: grouped entries. When no entries survive filtering, it
  prints `Initial release.`

`require_conventional` stays off in this file, so an admin bypass push
cannot break a release run. The PR lint uses its own minimal strict config
(see `ci.yml`).

### `Cargo.toml`

`version = "0.0.0"`, a placeholder. Local builds report `0.0.0`. Release
builds report the tagged version. `docs/build.md` explains this.

### `scripts/package-desktop.py`

- `validate_release(version, package_version, ref)` accepts a
  `package_version` of either `0.0.0` or exactly `version`. The SemVer and
  `refs/tags/v<version>` checks are unchanged.
- When `Cargo.toml` holds the placeholder and `--version` differs from it,
  with or without `--release-ref`:
  1. Record `revision` and `dirty` first, as today.
  2. Rewrite the root package's `version` in `Cargo.toml` and in
     `Cargo.lock` as text: exactly one match each, line endings preserved.
     This needs no network or `cargo update`.
  3. Build with `--locked` and read `cargo metadata`.
  4. Restore both files byte-for-byte, even on failure.

  Provenance therefore reflects the clean checkout, and local runs leave no
  diff. The `--locked` build is the safety net if the text edit were ever
  wrong.
- `tests/test_package_desktop.py` adds cases for:
  - the placeholder, with and without a release ref
  - a mismatch
  - stamping only the root package
  - CRLF preservation
  - restoring the files after an exception

### `.github/workflows/ci.yml`

Add a `commits` job, run only for `pull_request`:

1. Check out with `fetch-depth: 0` and `persist-credentials: false`.
2. Install git-cliff 2.14.2 with `taiki-e/install-action`, pinned by SHA.
3. Write a four-line strict config to `$RUNNER_TEMP`:
   `conventional_commits = true`, `filter_unconventional = false`,
   `require_conventional = true`.
4. Run
   `git-cliff --config <that file> <pull_request.base.sha>..<pull_request.head.sha>`.
   Output goes to `/dev/null`; the job fails on a non-zero exit.

The existing `check` matrix, its triggers, and
`cancel-in-progress: true` are unchanged.

### `.github/workflows/release.yml` (rewritten)

**Triggers:**
- `workflow_run` on workflow `Desktop checks`, types `[completed]`,
  branches `[main]`.
- `workflow_dispatch`, which builds artifacts only and never publishes.

**Workflow settings:**
- Concurrency: one fixed group `release` with `cancel-in-progress: false`.
- Permissions: top-level `contents: read`. Only `publish` gets more.

**Jobs:**

1. **plan** (ubuntu-24.04). For `workflow_run`, it runs only when
   `conclusion == 'success'` and `event == 'push'`.
   - Check out `workflow_run.head_sha` (or `github.sha` on dispatch) with
     `fetch-depth: 0` and `persist-credentials: false`. Install git-cliff
     as in CI.
   - `latest` is the highest tag that matches the pattern and is merged
     into HEAD; `next` is `git-cliff --bumped-version`.
   - Set `release = true` when all three hold:
     - the event is `workflow_run`
     - `next != latest`
     - tag `next` does not exist remotely (`git ls-remote`), which makes
       reruns idempotent.
   - Outputs: `release`, `version` (without the `v`), `sha`.
   - When releasing, render the notes with
     `git-cliff --unreleased --tag v<version> --strip all` and upload them
     as artifact `release-notes`.
2. **build** (the existing five-target matrix). It runs when
   `release == 'true'` or the event is `workflow_dispatch`.
   - Check out `sha`. Toolchain, Linux packages, cache and pinned
     packaging tools are the same as today.
   - The `check.sh` and packaging-contract steps are removed, because CI
     passed on this exact SHA.
   - The packaging step passes `--version`. Release runs also pass
     `--release-ref refs/tags/v<version>`, so the version gets stamped.
     Dispatch runs package the checkout's own `Cargo.toml` version.
   - Upload artifacts as today.
3. **publish** (ubuntu-24.04). It runs when `release == 'true'`.
   - Permissions: `contents: write`, `id-token: write`,
     `attestations: write`, `artifact-metadata: write`.
   - Download all target artifacts and `release-notes`. Run the existing
     check that all five reports are present and checksums match.
   - `actions/attest` (pinned by SHA) creates build provenance for
     `packages/*`.
   - Build the notes from the existing install and trust-prompt text
     followed by the git-cliff notes.
   - `gh release create v<version> packages/* --target <sha> --title "Bulletnotes <version>" --latest --notes-file <notes>`.
   - gh creates the tag together with the release. Nothing is tagged
     unless all five targets passed.

All actions are pinned by commit SHA with a version comment, matching the
existing style.

### Docs

`docs/build.md`, section "CI and development releases":

- Describe the merge-driven flow, the commit convention, and the
  `0.0.0` placeholder.
- Describe attestation verification (`gh attestation verify <file> -R leesiongchan/bulletnotes`).
- Remove the manual `git tag` instructions and the prerelease wording.

The README and site copy ("development builds") stay accurate and are
unchanged.

## Data flow

1. PR commits are linted, CI passes, and the PR is rebase-merged.
2. `Desktop checks` runs on the `main` push and succeeds. If a newer push
   lands first, its run is cancelled and the newer push's run covers its
   commits.
3. `workflow_run` starts `release.yml`. Runs are serialized by the
   `release` concurrency group.
4. `plan` decides whether to release:
   - No bump: the run ends green with nothing published.
   - Bump: the version and notes are computed.
5. `build` produces five verified, version-stamped packages at that SHA.
6. `publish` verifies them, attests them, and creates the tag and the
   Latest release in one `gh` call.

## Error handling

- **CI fails or is cancelled on `main`:** `plan` is skipped and nothing is
  released. The next green push re-evaluates all unreleased commits.
- **Any build target fails:** `publish` does not run and no tag is created.
  The next qualifying green push computes the same version again. A
  manual "Re-run failed jobs" also works.
- **The tag already exists** (rerun after success, or a manual tag): `plan`
  sets `release = false`.
- **Publish fails after the tag was created** (partial `gh` failure):
  rerunning hits the existing-tag guard. A maintainer deletes the partial
  release or tag and reruns. This is the one manual recovery path, and it
  is documented in `docs/build.md`.
- **A non-conventional commit reaches `main`** (admin bypass): it is
  skipped from the notes and does not bump. The release run does not fail.

## Bootstrap

No tags exist. The first green `main` push after this pipeline merges
publishes **v1.0.0**. Its notes list the earlier conventional `feat` and
`fix` commits, because the pre-convention commits are filtered out;
`Initial release.` only appears when no grouped entries exist. From then
on, only `feat`, `fix`, `perf` and breaking changes release.

## Testing

- **Scripts.** `python -m unittest discover -s tests -p 'test_package_desktop.py'`
  covers the new `validate_release` cases and stamping. Version stamping
  is tested on a temporary copy of `Cargo.toml` and `Cargo.lock`, and the
  test asserts that `cargo metadata --locked` succeeds afterwards.
- **`cliff.toml`.** Re-run the scratch-repo bump matrix against the real
  config: `docs` gives no release, `fix` a patch, `feat` a minor, `feat!` a
  major. Also check the lint range fails on `Bad message` and
  passes on conventional commits.
- **Workflows.** `actionlint` on both workflows.
- **End to end.** This pipeline's own merge produces v1.0.0. Afterwards,
  check:
  - the release is Latest and has 5 targets' assets plus reports
  - the notes render
  - `gh attestation verify` passes on one asset
  - a following `docs:` merge publishes nothing

## Out of scope

- Code signing and notarization. Builds stay labelled development.
- crates.io publishing (`publish = false`).
- Ruleset or repo-setting changes. These are recommended for the
  maintainer to apply manually:
  - mark the `commits` check as required
  - enable immutable releases
