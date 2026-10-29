# Automated Release Pipeline Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Every green push to `main` that contains a `feat`, `fix`, `perf` or
breaking commit publishes a tagged, attested, Latest GitHub Release of all five
desktop targets, with no human step.

**Architecture:** git-cliff (`cliff.toml`) computes the next version and the
release notes from Conventional Commits. `release.yml` is triggered by
`workflow_run` after "Desktop checks" succeeds on `main`. It plans, builds the
five targets at the tested SHA, then attests and publishes. `gh release create`
creates the tag. `Cargo.toml` holds a `0.0.0` placeholder that
`package-desktop.py` stamps temporarily during the build, so nothing is ever
committed back to `main`.

**Tech Stack:**
- GitHub Actions
- git-cliff 2.14.2, installed through `taiki-e/install-action`
- `gh` CLI
- `actions/attest` v4.2.2
- Python 3.12 `unittest`
- Rust 1.99.1 and cargo

**Spec:** `docs/superpowers/specs/2026-10-29-automated-release-pipeline-design.md`

## Global Constraints

- **Default branch.** It is `main` (not `master`). Merges are rebase-only, so
  every commit lands on `main` as written.
- **Commit messages.** Every commit on this branch must be a Conventional
  Commit (`type(scope)!: subject`), because the new `commits` CI job lints this
  PR itself. Use `build:`, `ci:` and `docs:`; none of them bump the version.
  No `Co-Authored-By` lines (AGENTS.md).
- **Action pinning.** Pin every action by full commit SHA with a version
  comment, matching the existing style:
  - `actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7.0.1`
  - `actions/setup-python@ece7cb06caefa5fff74198d8649806c4678c61a1 # v6.3.0`
  - `Swatinem/rust-cache@d1527c8f67ef9af357290475d4af4b0fab8f12e0 # v2.9.3`
  - `actions/upload-artifact@cf430e030ddbb5b0abf93d22962f4752f3646cd9 # v7.0.2`
  - `actions/download-artifact@9000827ccba6bdab643e8b6fd33ac0654aef8333 # v8.0.2`
  - `taiki-e/install-action@86f7a0ffb25f40114606501a311ebcfaf882ea32 # v2.88.0`
  - `actions/attest@1e69f48acb82d1966a394da916b4c1698aa569d6 # v4.2.2`
- **git-cliff.** Version `2.14.2`.
- **Placeholder version.** The literal `0.0.0`.
- **Tag format.** `vMAJOR.MINOR.PATCH`, matching the regex
  `^v[0-9]+\.[0-9]+\.[0-9]+$`. The first release is `v0.1.0`.
- **Releasing types.** `feat`, `fix`, `perf` and breaking changes. These never
  release: `docs|chore|ci|test|style|refactor|build`.
- **Breaking changes on 0.x** bump the minor version, never to 1.0.0.
- **Artifact labels stay development.** Artifact names keep `-development`
  and reports keep `kind: development`. Releases are not prereleases and are
  marked `--latest`.
- **Toolchain.** Pinned at `1.99.1`; commands use `rustup run 1.99.1 cargo …`.
- **Logging.** Follow the existing idioms: `print(..., flush=True)` in Python
  and `::notice::` lines in workflows.
- **Local tools.** git-cliff 2.14.2 and actionlint 1.7.12 may not be
  installed. On this Nix machine, prefix them with comma: `, git-cliff …`,
  `, actionlint …`.
- **Pre-validated content.** The plan's `cliff.toml`, check script, workflow
  YAML (actionlint-clean) and Python changes (10/10 tests,
  pyflakes-clean) were verified against scratch copies while the plan was
  written. If a step's output differs from what the plan expects, stop and
  investigate rather than adjusting the expectation.

## File Map

| File | Change | Responsibility |
|---|---|---|
| `cliff.toml` | Create | Version bump rules and release-note template |
| `Cargo.toml`, `Cargo.lock` | Modify | Root package version becomes `0.0.0` |
| `scripts/package-desktop.py` | Modify | Accept the placeholder and stamp the version temporarily |
| `tests/test_package_desktop.py` | Modify | Contracts for the placeholder and stamping |
| `.github/workflows/ci.yml` | Modify | Add the PR `commits` lint job |
| `.github/workflows/release.yml` | Rewrite | Plan, build, attest and publish |
| `docs/build.md` | Modify | Document the new release flow |

---

### Task 1: git-cliff configuration

**Files:**
- Create: `cliff.toml`

**Interfaces:**
- Produces:
  - `git-cliff --bumped-version` prints the next tag, e.g. `v0.2.0`. It
    prints the latest tag unchanged when nothing qualifies, and `v0.1.0`
    when no tags exist.
  - `git-cliff --unreleased --tag vX.Y.Z --strip all` prints Markdown notes,
    or `Initial release.` when there are no grouped entries.

- [ ] **Step 1: Write the bump and notes check (it fails without the config)**

Save this as `/tmp/cliff-check.sh`, outside the repo; it is throwaway. It
creates a scratch repo, copies `cliff.toml` into it, and checks every rule.

```bash
#!/usr/bin/env bash
set -euo pipefail
config=$(realpath "$1")
scratch=$(mktemp -d)
cd "$scratch"
export GIT_AUTHOR_NAME=t GIT_AUTHOR_EMAIL=t@t GIT_COMMITTER_NAME=t GIT_COMMITTER_EMAIL=t@t NO_COLOR=1
git init -q
git config commit.gpgsign false
git config tag.gpgsign false
cp "$config" cliff.toml
git add cliff.toml
git commit -q -m "Old style commit"
git commit -q --allow-empty -m "ci: add release pipeline"
expect() { [[ "$2" == "$3" ]] || { echo "FAIL $1: expected '$3' got '$2'"; exit 1; }; echo "ok $1"; }
bump() { git-cliff --bumped-version 2>/dev/null; }
expect "no tags" "$(bump)" "v0.1.0"
expect "initial notes" "$(git-cliff --unreleased --tag v0.1.0 --strip all 2>/dev/null | sed '/^$/d')" "Initial release."
git tag -a v0.1.0 -m v0.1.0
for message in "docs: readme" "chore: tidy" "ci: tweak" "test: more" "style: fmt" "refactor: move" "build: deps"; do
  git commit -q --allow-empty -m "$message"
done
expect "non-releasing types" "$(bump)" "v0.1.0"
git commit -q --allow-empty -m "fix(sync): retry on 503"
expect "fix" "$(bump)" "v0.1.1"
git commit -q --allow-empty -m "perf: faster load"
expect "perf" "$(bump)" "v0.1.1"
git commit -q --allow-empty -m "feat: add export"
expect "feat" "$(bump)" "v0.2.0"
git commit -q --allow-empty -m "feat!: change store format"
expect "breaking on 0.x" "$(bump)" "v0.2.0"
git commit -q --allow-empty -m "Stray unconventional commit"
expect "stray commit does not fail release" "$(bump)" "v0.2.0"
notes=$(git-cliff --unreleased --tag v0.2.0 --strip all 2>/dev/null)
grep -q '^### Features$' <<<"$notes" && grep -q '^### Fixes$' <<<"$notes" && grep -q '^### Performance$' <<<"$notes" \
  && grep -q '^- \*\*Breaking:\*\* Change store format' <<<"$notes" && grep -q '^- sync: Retry on 503' <<<"$notes" \
  && ! grep -qi 'readme\|stray' <<<"$notes" || { echo "FAIL notes:"; echo "$notes"; exit 1; }
echo "ok notes"
printf '[git]\nconventional_commits = true\nfilter_unconventional = false\nrequire_conventional = true\n' > "$scratch/lint.toml"
if git-cliff --config "$scratch/lint.toml" HEAD~1..HEAD >/dev/null 2>&1; then echo "FAIL lint accepted bad commit"; exit 1; fi
echo "ok lint rejects"
git-cliff --config "$scratch/lint.toml" HEAD~3..HEAD~1 >/dev/null 2>&1 || { echo "FAIL lint rejected good commits"; exit 1; }
echo "ok lint accepts"
echo "ALL PASS"
```

- [ ] **Step 2: Run it to verify it fails**

Run: `bash /tmp/cliff-check.sh cliff.toml`. If git-cliff is not on PATH, use
`, git-cliff` on Nix, or prefix with `PATH=$(dirname "$(, which git-cliff)"):$PATH`.
Expected: failure, because `realpath: cliff.toml: No such file or directory`.

- [ ] **Step 3: Create `cliff.toml`**

```toml
# git-cliff computes release versions and notes; see docs/build.md.
[changelog]
body = """
{% for group, commits in commits | group_by(attribute="group") %}
### {{ group | striptags | trim }}
{% for commit in commits %}
- {% if commit.breaking %}**Breaking:** {% endif %}{% if commit.scope %}{{ commit.scope }}: {% endif %}{{ commit.message | upper_first }} ({{ commit.id | truncate(length=7, end="") }})\
{% endfor %}
{% else %}
Initial release.
{% endfor %}
"""
trim = true

[git]
conventional_commits = true
filter_unconventional = false
# The PR commit lint uses its own strict config; releases must not fail on stray commits.
require_conventional = false
split_commits = false
protect_breaking_commits = true
filter_commits = false
tag_pattern = "^v[0-9]+\\.[0-9]+\\.[0-9]+$"
topo_order = false
sort_commits = "oldest"
commit_parsers = [
  { message = "^feat", group = "<!-- 0 -->Features" },
  { message = "^fix", group = "<!-- 1 -->Fixes" },
  { message = "^perf", group = "<!-- 2 -->Performance" },
  { message = ".*", skip = true },
]

[bump]
features_always_bump_minor = true
# A breaking change on 0.x must not silently declare 1.0.
breaking_always_bump_major = false
no_increment_regex = "^(docs|chore|ci|test|style|refactor|build)$"
initial_tag = "v0.1.0"
```

- [ ] **Step 4: Run the check to verify it passes**

Run: `bash /tmp/cliff-check.sh cliff.toml`
Expected: every line starts with `ok`, and the last line is `ALL PASS`.

Then run it against the real repo history: `git-cliff --bumped-version`.
Expected: `v0.1.0` on stdout. A WARN about skipped unparseable commits is
expected on stderr.

- [ ] **Step 5: Commit**

```bash
git add cliff.toml
git commit -m "build: add git-cliff release configuration" -m "Defines which Conventional Commit types bump the version and how release notes are grouped, so releases can be computed from history."
```

---

### Task 2: Placeholder version and temporary stamping in the packager

**Files:**
- Modify: `scripts/package-desktop.py`. The imports change at lines 3-15,
  `validate_release` at lines 37-45, and the build section of `main()` at
  lines 241-246.
- Modify: `Cargo.toml:3`. Change `version = "0.1.0"` to `version = "0.0.0"`.
- Modify: `Cargo.lock`. In the `[[package]]` block with
  `name = "bulletnotes"` (line 677), change `version = "0.1.0"` to
  `version = "0.0.0"`.
- Test: `tests/test_package_desktop.py`

**Interfaces:**
- Produces, in `scripts/package-desktop.py`:
  - `PLACEHOLDER_VERSION = '0.0.0'`
  - `validate_release(version, package_version, ref)` now also accepts
    `package_version == '0.0.0'`.
  - `stamp_version(manifest: str, lock: str, version: str) -> list[str]`
    returns `[manifest, lock]` with the root `bulletnotes` placeholder
    replaced. It raises `ValueError` unless each file holds exactly one
    placeholder.
  - `stamped_version(root: pathlib.Path, version: str)` is a context
    manager. It writes stamped `Cargo.toml` and `Cargo.lock` and restores
    the original bytes on exit, including on exceptions.
- Consumed by: Task 4. `release.yml` calls
  `package-desktop.py --version X --release-ref refs/tags/vX` on a checkout
  whose `Cargo.toml` says `0.0.0`.

- [ ] **Step 1: Write the failing tests**

In `tests/test_package_desktop.py`, add these methods to
`DesktopPackagingTests`. Place them directly after
`test_release_inputs_reject_injection_and_wrong_tags`.

```python
    def test_release_version_accepts_cargo_placeholder(self):
        self.package.validate_release('1.2.3', '0.0.0', None)
        self.package.validate_release('1.2.3', '0.0.0', 'refs/tags/v1.2.3')
        self.package.validate_release('0.0.0', '0.0.0', None)
        with self.assertRaises(ValueError):
            self.package.validate_release('1.2.3', '0.0.1', None)

    def test_stamp_version_rewrites_only_the_root_package(self):
        manifest = '[package]\r\nname = "bulletnotes"\r\nversion = "0.0.0"\r\n\r\n[dependencies]\r\nother = "0.0.0"\r\n'
        lock = ('[[package]]\r\nname = "aaa"\r\nversion = "0.0.0"\r\n\r\n'
                '[[package]]\r\nname = "bulletnotes"\r\nversion = "0.0.0"\r\ndependencies = [\r\n "aaa",\r\n]\r\n')
        stamped_manifest, stamped_lock = self.package.stamp_version(manifest, lock, '1.2.3')
        self.assertEqual(stamped_manifest, manifest.replace('version = "0.0.0"', 'version = "1.2.3"', 1))
        self.assertEqual(stamped_lock, lock.replace('name = "bulletnotes"\r\nversion = "0.0.0"',
                                                    'name = "bulletnotes"\r\nversion = "1.2.3"'))
        with self.assertRaises(ValueError):
            self.package.stamp_version(stamped_manifest, lock, '1.2.3')
        with self.assertRaises(ValueError):
            self.package.stamp_version(manifest, stamped_lock, '1.2.3')

    def test_repository_cargo_files_hold_the_placeholder(self):
        self.package.stamp_version((ROOT / 'Cargo.toml').read_text(), (ROOT / 'Cargo.lock').read_text(), '1.2.3')

    def test_stamped_version_restores_the_checkout(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            manifest = b'[package]\nname = "bulletnotes"\nversion = "0.0.0"\n'
            lock = b'[[package]]\nname = "bulletnotes"\nversion = "0.0.0"\n'
            (root / 'Cargo.toml').write_bytes(manifest)
            (root / 'Cargo.lock').write_bytes(lock)
            with self.assertRaises(RuntimeError):
                with self.package.stamped_version(root, '1.2.3'):
                    self.assertIn(b'version = "1.2.3"', (root / 'Cargo.toml').read_bytes())
                    self.assertIn(b'version = "1.2.3"', (root / 'Cargo.lock').read_bytes())
                    raise RuntimeError('build failed')
            self.assertEqual((root / 'Cargo.toml').read_bytes(), manifest)
            self.assertEqual((root / 'Cargo.lock').read_bytes(), lock)
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `python3 -m unittest discover -s tests -p 'test_package_desktop.py' -v`
Expected: these four tests error or fail:
- `test_release_version_accepts_cargo_placeholder` fails with `ValueError`.
- `test_stamp_version_rewrites_only_the_root_package` and
  `test_repository_cargo_files_hold_the_placeholder` fail with
  `AttributeError: ... has no attribute 'stamp_version'`.
- `test_stamped_version_restores_the_checkout` fails with `AttributeError`.

The existing tests still pass.

- [ ] **Step 3: Implement in `scripts/package-desktop.py`**

Add `import contextlib` to the import block, keeping it alphabetical, so it
goes before `import hashlib`:

```python
import argparse
import contextlib
import hashlib
```

Add the constant after `TAURI_VERSION = '2.12.4'`:

```python
TAURI_VERSION = '2.12.4'
# Cargo.toml keeps this version on main; release tags are the source of truth.
PLACEHOLDER_VERSION = '0.0.0'
```

In `validate_release`, replace the line
`if not re.fullmatch(semver, version) or version != package_version:` and the
error message that follows it with:

```python
    if not re.fullmatch(semver, version) or package_version not in (PLACEHOLDER_VERSION, version):
        raise ValueError('--version must be valid SemVer and match Cargo.toml or its ' + PLACEHOLDER_VERSION + ' placeholder')
```

Add these two functions immediately after `validate_release`:

```python
def stamp_version(manifest, lock, version):
    # A text edit keeps Cargo.lock consistent without network access; --locked builds catch any drift.
    stamped = []
    for text, header in ((manifest, r'\[package\]'), (lock, r'\[\[package\]\]')):
        pattern = r'(?m)^(' + header + r'\r?\nname = "bulletnotes"\r?\nversion = )"' + re.escape(PLACEHOLDER_VERSION) + '"'
        text, count = re.subn(pattern, lambda match: match.group(1) + json.dumps(version), text)
        if count != 1:
            raise ValueError('Cargo.toml and Cargo.lock must hold the ' + PLACEHOLDER_VERSION + ' placeholder')
        stamped.append(text)
    return stamped


@contextlib.contextmanager
def stamped_version(root, version):
    """Build with a release version while leaving the checkout byte-for-byte unchanged."""
    paths = [root / 'Cargo.toml', root / 'Cargo.lock']
    originals = [path.read_bytes() for path in paths]
    stamped = stamp_version(*(data.decode() for data in originals), version)
    try:
        for path, text in zip(paths, stamped):
            path.write_bytes(text.encode())
        yield
    finally:
        for path, data in zip(paths, originals):
            path.write_bytes(data)
```

In `main()`, the current block is:

```python
    lock_hash = sha(ROOT / 'Cargo.lock')
    run(cargo + ['build', '--release', '--locked', '--bin', 'bulletnotes', '--target', args.target], cwd=ROOT, env=environment)
    metadata = json.loads(capture(cargo + ['metadata', '--no-deps', '--locked', '--format-version', '1'], cwd=ROOT, env=environment))
```

Replace it with:

```python
    lock_hash = sha(ROOT / 'Cargo.lock')
    stamping = package['version'] == PLACEHOLDER_VERSION != args.version
    if stamping:
        print('Stamping Cargo version ' + args.version + ' over the ' + PLACEHOLDER_VERSION + ' placeholder', flush=True)
    with stamped_version(ROOT, args.version) if stamping else contextlib.nullcontext():
        run(cargo + ['build', '--release', '--locked', '--bin', 'bulletnotes', '--target', args.target], cwd=ROOT, env=environment)
        metadata = json.loads(capture(cargo + ['metadata', '--no-deps', '--locked', '--format-version', '1'], cwd=ROOT, env=environment))
```

Provenance (`revision`, `dirty`, `lock_hash`) is captured on the lines just
above. It therefore describes the committed checkout. Do not move those
lines.

- [ ] **Step 4: Set the placeholder in Cargo files**

- In `Cargo.toml` line 3, change `version = "0.1.0"` to `version = "0.0.0"`.
- In `Cargo.lock`, change only the version line directly under
  `name = "bulletnotes"`, from `version = "0.1.0"` to `version = "0.0.0"`.

Check with: `rg -n -A1 '^name = "bulletnotes"$' Cargo.lock`
Expected: the next line is `version = "0.0.0"`.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `python3 -m unittest discover -s tests -p 'test_package_desktop.py' -v`
Expected: every test passes, with `OK` at the end.

- [ ] **Step 6: Verify the lockfile still works under `--locked`, both stamped and unstamped**

```bash
rustup run 1.99.1 cargo metadata --locked --format-version 1 > /dev/null && echo "placeholder locked ok"
python3 - <<'PY'
import importlib.util, json, pathlib, subprocess
spec = importlib.util.spec_from_file_location('package_desktop', 'scripts/package-desktop.py')
package = importlib.util.module_from_spec(spec)
spec.loader.exec_module(package)
with package.stamped_version(pathlib.Path.cwd(), '9.8.7'):
    output = subprocess.run(['rustup', 'run', '1.99.1', 'cargo', 'metadata', '--locked', '--format-version', '1'],
                            check=True, capture_output=True, text=True).stdout
    print('stamped', [p['version'] for p in json.loads(output)['packages'] if p['name'] == 'bulletnotes'])
PY
git diff --numstat -- Cargo.toml Cargo.lock
```

Expected output:
- `placeholder locked ok`
- `stamped ['9.8.7']`
- `1	1	Cargo.toml` and `1	1	Cargo.lock`. Only the Step 4 placeholder edit
  remains; the stamp was restored.

- [ ] **Step 7: Run the full local gate**

Run: `bash scripts/check.sh`
Expected: it exits 0. `CARGO_PKG_VERSION` is now `0.0.0`. No Rust test
asserts the crate version; `tests/release_artifact.rs` uses its own plist
fixture.

- [ ] **Step 8: Commit**

```bash
git add scripts/package-desktop.py tests/test_package_desktop.py Cargo.toml Cargo.lock
git commit -m "build: stamp release versions over a 0.0.0 Cargo placeholder" -m "Release tags become the version source of truth. The packager stamps the tagged version into Cargo.toml and Cargo.lock only for the build, so provenance reflects a clean checkout and main never needs a version-bump commit."
```

---

### Task 3: Lint PR commits as Conventional Commits

**Files:**
- Modify: `.github/workflows/ci.yml`. Add a `commits` job before
  `check:`, under `jobs:`.

**Interfaces:**
- Consumes: nothing from earlier tasks. The lint uses its own inline strict
  config, not `cliff.toml`.
- Produces: the job `commits` inside workflow "Desktop checks". Task 4's
  `workflow_run` trigger watches this workflow by name.

- [ ] **Step 1: Prove the lint command rejects a bad commit locally (the failing case)**

```bash
printf '[git]\nconventional_commits = true\nfilter_unconventional = false\nrequire_conventional = true\n' > /tmp/cliff-commits.toml
git-cliff --config /tmp/cliff-commits.toml 21e3570~1..21e3570 > /dev/null; echo "rc=$?"
```

Expected: `rc=1` with `UnconventionalCommitsError`. Commit `21e3570` is
`Fix race in missing-page repair integration test`.

- [ ] **Step 2: Prove it accepts this branch's commits**

```bash
git-cliff --config /tmp/cliff-commits.toml "$(git merge-base origin/main HEAD)..HEAD" > /dev/null; echo "rc=$?"
```

Expected: `rc=0`. If it fails, a commit on this branch is not conventional.
Reword it before continuing; rebase is allowed on this unpushed branch.

- [ ] **Step 3: Add the job to `.github/workflows/ci.yml`**

Insert the following directly after the `jobs:` line, above `  check:`:

```yaml
  commits:
    # Rebase merges land every commit on main, so each one feeds git-cliff's version calculation.
    if: github.event_name == 'pull_request'
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7.0.1
        with:
          fetch-depth: 0
          persist-credentials: false
      - uses: taiki-e/install-action@86f7a0ffb25f40114606501a311ebcfaf882ea32 # v2.88.0
        with:
          tool: git-cliff@2.14.2
      - name: Require Conventional Commits
        env:
          BASE_SHA: ${{ github.event.pull_request.base.sha }}
          HEAD_SHA: ${{ github.event.pull_request.head.sha }}
        run: |
          printf '[git]\nconventional_commits = true\nfilter_unconventional = false\nrequire_conventional = true\n' > "$RUNNER_TEMP/cliff-commits.toml"
          git-cliff --config "$RUNNER_TEMP/cliff-commits.toml" "$BASE_SHA..$HEAD_SHA" > /dev/null
```

Leave the existing `check` job, the triggers and the concurrency block
unchanged.

- [ ] **Step 4: Lint the workflow**

Run: `actionlint .github/workflows/ci.yml`
Expected: no output, exit 0.

- [ ] **Step 5: Commit**

```bash
git add .github/workflows/ci.yml
git commit -m "ci: require Conventional Commits on pull requests" -m "Release versions are computed from commit types, and rebase merges land every PR commit on main, so each commit must parse."
```

---

### Task 4: Merge-driven release workflow

**Files:**
- Rewrite: `.github/workflows/release.yml`

**Interfaces:**
- Consumes:
  - `cliff.toml` from Task 1.
  - From Task 2, `package-desktop.py --version V [--release-ref refs/tags/vV]`,
    which accepts a `0.0.0` checkout.
  - The workflow name "Desktop checks" from `ci.yml`.
- Produces:
  - The tag `vX.Y.Z` and a Latest GitHub Release with assets
    `packages/*` from five targets.
  - Build-provenance attestations for every asset.

- [ ] **Step 1: Replace `.github/workflows/release.yml` with:**

```yaml
name: Release

on:
  workflow_run:
    workflows: [Desktop checks]
    types: [completed]
    branches: [main]
  workflow_dispatch:

permissions:
  contents: read

# Serialize releases; a newer pending run replaces an older one and already includes its commits.
concurrency:
  group: release
  cancel-in-progress: false

jobs:
  plan:
    if: >-
      github.event_name == 'workflow_dispatch' ||
      (github.event.workflow_run.conclusion == 'success' && github.event.workflow_run.event == 'push')
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    outputs:
      release: ${{ steps.plan.outputs.release }}
      version: ${{ steps.plan.outputs.version }}
      sha: ${{ steps.plan.outputs.sha }}
    steps:
      - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7.0.1
        with:
          ref: ${{ github.event.workflow_run.head_sha || github.sha }}
          fetch-depth: 0
          persist-credentials: false
      - uses: taiki-e/install-action@86f7a0ffb25f40114606501a311ebcfaf882ea32 # v2.88.0
        with:
          tool: git-cliff@2.14.2
      - name: Decide release version
        id: plan
        env:
          EVENT: ${{ github.event_name }}
        run: |
          sha=$(git rev-parse HEAD)
          latest=$(git tag --list 'v[0-9]*' --merged HEAD --sort=-v:refname | sed -n 1p)
          next=$(git-cliff --bumped-version)
          release=false
          version=$(python3 -c 'import tomllib; print(tomllib.load(open("Cargo.toml", "rb"))["package"]["version"])')
          if [[ "$EVENT" != workflow_run ]]; then
            echo "::notice::Manual run builds $version artifacts without publishing"
          elif [[ "$next" == "$latest" ]]; then
            echo "::notice::No releasable commits since ${latest:-the start of history}"
          elif git ls-remote --exit-code --tags origin "refs/tags/$next" > /dev/null; then
            echo "::notice::$next already exists; nothing to publish"
          else
            release=true
            version=${next#v}
            git-cliff --unreleased --tag "$next" --strip all --output "$RUNNER_TEMP/release-notes.md"
            echo "::notice::Releasing $next from $sha (previous: ${latest:-none})"
          fi
          {
            echo "release=$release"
            echo "version=$version"
            echo "sha=$sha"
          } >> "$GITHUB_OUTPUT"
      - name: Upload release notes
        if: steps.plan.outputs.release == 'true'
        uses: actions/upload-artifact@cf430e030ddbb5b0abf93d22962f4752f3646cd9 # v7.0.2
        with:
          name: release-notes
          path: ${{ runner.temp }}/release-notes.md
          if-no-files-found: error

  build:
    needs: plan
    if: needs.plan.outputs.release == 'true' || github.event_name == 'workflow_dispatch'
    strategy:
      fail-fast: false
      matrix:
        include:
          - runner: macos-15
            target: aarch64-apple-darwin
          - runner: windows-latest
            target: x86_64-pc-windows-msvc
          - runner: windows-11-arm
            target: aarch64-pc-windows-msvc
          - runner: ubuntu-24.04
            target: x86_64-unknown-linux-gnu
          - runner: ubuntu-24.04-arm
            target: aarch64-unknown-linux-gnu
    runs-on: ${{ matrix.runner }}
    timeout-minutes: 120
    defaults:
      run:
        shell: bash
    env:
      CARGO_BUILD_JOBS: '2'
      CARGO_INCREMENTAL: '0'
      CARGO_PROFILE_DEV_DEBUG: '0'
      CARGO_PROFILE_TEST_DEBUG: '0'
      TARGET: ${{ matrix.target }}
    steps:
      - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7.0.1
        with:
          ref: ${{ needs.plan.outputs.sha }}
          persist-credentials: false
      - uses: actions/setup-python@ece7cb06caefa5fff74198d8649806c4678c61a1 # v6.3.0
        with:
          python-version: '3.12'
      - name: Install Linux build and bundle libraries
        if: runner.os == 'Linux'
        run: |
          sudo apt-get update
          sudo apt-get install -y libfontconfig1-dev libfreetype-dev libxkbcommon-dev libxkbcommon-x11-0 libx11-xcb1 libxcursor1 libxi6 libvulkan1 libwayland-dev libgtk-3-dev libayatana-appindicator3-dev libwebkit2gtk-4.1-dev libssl-dev pkg-config dpkg-dev patchelf librsvg2-bin xvfb xauth
      - name: Install pinned Rust
        run: |
          rustup set default-host "$TARGET"
          rustup toolchain install 1.99.1 --profile minimal --target "$TARGET"
      - name: Cache Rust dependencies and packaging tools
        uses: Swatinem/rust-cache@d1527c8f67ef9af357290475d4af4b0fab8f12e0 # v2.9.3
        with:
          # Invalidate cached tools when their workflow pins change.
          key: ${{ matrix.target }}-${{ hashFiles('.github/workflows/release.yml') }}
          cache-on-failure: true
      - name: Install pinned packaging tools
        run: |
          rustup run 1.99.1 cargo install cargo-about --version 0.9.2 --features cli --locked
          rustup run 1.99.1 cargo about --version
          rustup run 1.99.1 cargo install tauri-cli --version 2.12.4 --locked
      # Desktop checks already passed on this exact commit, so only packaging runs here.
      - name: Package and verify installers
        env:
          VERSION: ${{ needs.plan.outputs.version }}
          RELEASE: ${{ needs.plan.outputs.release }}
        run: |
          arguments=(--version "$VERSION" --target "$TARGET" --output target/packages/ci)
          if [[ "$RELEASE" == true ]]; then arguments+=(--release-ref "refs/tags/v$VERSION"); fi
          if [[ "$RUNNER_OS" == macOS ]]; then
            # Hosted macOS does not establish a hardware Metal launch; local release verification does.
            python scripts/package-desktop.py "${arguments[@]}" --static-only
          elif [[ "$RUNNER_OS" == Linux ]]; then
            SLINT_BACKEND=winit-software xvfb-run -a python scripts/package-desktop.py "${arguments[@]}"
          else
            SLINT_BACKEND=winit-software python scripts/package-desktop.py "${arguments[@]}"
          fi
      - name: Upload installers, hashes, and verification report
        uses: actions/upload-artifact@cf430e030ddbb5b0abf93d22962f4752f3646cd9 # v7.0.2
        with:
          name: bulletnotes-${{ matrix.target }}-development
          path: target/packages/ci/*
          if-no-files-found: error

  publish:
    needs: [plan, build]
    if: needs.plan.outputs.release == 'true'
    runs-on: ubuntu-24.04
    timeout-minutes: 15
    permissions:
      contents: write
      id-token: write
      attestations: write
      artifact-metadata: write
    steps:
      - uses: actions/download-artifact@9000827ccba6bdab643e8b6fd33ac0654aef8333 # v8.0.2
        with:
          pattern: bulletnotes-*-development
          merge-multiple: true
          path: packages
      - uses: actions/download-artifact@9000827ccba6bdab643e8b6fd33ac0654aef8333 # v8.0.2
        with:
          name: release-notes
          path: notes
      - name: Verify all target reports and checksums
        run: |
          python3 - <<'PY'
          import hashlib, json, pathlib
          reports = list(pathlib.Path('packages').glob('*.verification.json'))
          expected = {'aarch64-apple-darwin', 'x86_64-pc-windows-msvc', 'aarch64-pc-windows-msvc', 'x86_64-unknown-linux-gnu', 'aarch64-unknown-linux-gnu'}
          evidence = [json.loads(report.read_text()) for report in reports]
          if len(evidence) != 5 or {report['target'] for report in evidence} != expected:
              raise SystemExit('All five target reports are required before publication')
          for report in evidence:
              for filename, digest in report['artifacts'].items():
                  path = pathlib.Path('packages') / filename
                  if path.parent != pathlib.Path('packages') or hashlib.sha256(path.read_bytes()).hexdigest() != digest:
                      raise SystemExit('Downloaded artifact checksum mismatch')
          PY
      - uses: actions/attest@1e69f48acb82d1966a394da916b4c1698aa569d6 # v4.2.2
        with:
          subject-path: packages/*
      - name: Publish release
        env:
          GH_TOKEN: ${{ github.token }}
          GH_REPO: ${{ github.repository }}
          VERSION: ${{ needs.plan.outputs.version }}
          SHA: ${{ needs.plan.outputs.sha }}
        run: |
          {
            cat <<'NOTES'
          Development desktop builds for macOS 14+ on ARM64, and Windows and Linux on x64 and ARM64.

          macOS: open the DMG and drag Bulletnotes to Applications. The app is ad-hoc signed and is not notarized by Apple. Windows: run the unsigned NSIS installer. Linux: install the deb package or make the AppImage executable and run it. These are development artifacts, and operating-system trust prompts are expected.

          Each verification report records exact source provenance, target, hashes, and observed launch evidence. Hosted macOS UI/Metal launch remains unobserved. Windows/Linux CI smoke tests use the Slint software renderer; production GPU rendering requires native hardware verification.

          ## Changes
          NOTES
            cat notes/release-notes.md
          } > "$RUNNER_TEMP/release-notes.md"
          # Creating the tag here means a failed build never leaves an orphan tag behind.
          gh release create "v$VERSION" packages/* \
            --target "$SHA" --latest \
            --title "Bulletnotes $VERSION" \
            --notes-file "$RUNNER_TEMP/release-notes.md"
          echo "::notice::Published v$VERSION at $SHA"
```

Notes for the implementer:
- **The `NOTES` heredoc indentation is intentional.** YAML strips the
  block's common indentation, so the heredoc body and the closing `NOTES`
  start at column 0 in the executed script. That is what the existing
  release.yml relies on too.
- **`plan` is skipped for failed or cancelled CI runs.** `build` and
  `publish` then skip automatically, because their `needs` did not succeed.
- **Use `sed -n 1p`, not `head -n1`.** Under the runner's `-o pipefail`,
  `head` can SIGPIPE `git tag` and fail the step.

- [ ] **Step 2: Lint**

Run: `actionlint .github/workflows/release.yml .github/workflows/ci.yml`
Expected: no output, exit 0. If actionlint reports `artifact-metadata` as an
unknown permission scope, check that your actionlint is current
(`actionlint -version`). If it is current and still reports this, keep the
permission, because `actions/attest` v4.2.2's README requires it, and
record the false positive in the commit body.

- [ ] **Step 3: Dry-run the plan logic locally**

Run this from the repo root on this branch. It mirrors the `plan` step, with
`EVENT=workflow_run`:

```bash
latest=$(git tag --list 'v[0-9]*' --merged HEAD --sort=-v:refname | sed -n 1p)
next=$(git-cliff --bumped-version 2>/dev/null)
echo "latest='${latest}' next='${next}'"
git ls-remote --exit-code --tags origin "refs/tags/$next" > /dev/null; echo "remote tag exists rc=$? (2 = absent)"
git-cliff --unreleased --tag "$next" --strip all 2>/dev/null
```

Expected:
- `latest='' next='v0.1.0'`
- `rc=2`
- notes that read `Initial release.`

This branch only adds `build:`, `ci:` and `docs:` commits, so no group
survives.

- [ ] **Step 4: Commit**

```bash
git add .github/workflows/release.yml
git commit -m "ci: publish releases automatically after main passes checks" -m "Replaces the manual tag-push prerelease flow. After Desktop checks succeeds on main, git-cliff decides whether a release is due, all five targets are packaged at that commit, assets are attested, and gh creates the tag and Latest release together so failed builds never leave orphan tags."
```

---

### Task 5: Release documentation

**Files:**
- Modify: `docs/build.md`. The section `## CI and development releases`
  starts at line 48. The `## Distribution prerequisites` section starts at
  line 129.

**Interfaces:**
- Consumes: the behaviour from Tasks 1 to 4. Workflow names are
  "Desktop checks" and "Release"; the placeholder is `0.0.0`.

- [ ] **Step 1: Replace the release paragraphs**

In `docs/build.md`, replace everything from the paragraph starting
`The **Desktop checks** workflow runs formatting` through the line
`It refuses to replace an existing release.` with the text below. Keep the
`## CI and development releases` heading. Keep everything from
`macOS CI uses \`--static-only\`` onward unchanged.

````markdown
The **Desktop checks** workflow runs formatting, strict all-feature Clippy, Rust
tests, and packaging contract tests on all five native runners for pushes and pull
requests. On pull requests it also requires every commit to follow
[Conventional Commits](https://www.conventionalcommits.org/). Merges are
rebase-only, so each commit lands on `main` as written and feeds the version
calculation.

Releases are automatic. When **Desktop checks** passes on a push to `main`, the
**Release** workflow asks git-cliff (configured in `cliff.toml`) for the next
version since the latest `v*` tag:

- `feat` bumps the minor version.
- `fix` and `perf` bump the patch version.
- A breaking change (`!` or a `BREAKING CHANGE` footer) bumps the minor
  version while below 1.0.
- `docs`, `chore`, `ci`, `test`, `style`, `refactor` and `build` never
  release.

When a release is due, all five targets are packaged from the tested commit.
Only after every target passes are the `vX.Y.Z` tag and a GitHub Release
marked Latest created together. The release notes are the installation text
followed by the git-cliff changelog. Nothing is committed back to `main`.

`Cargo.toml` holds the placeholder version `0.0.0`; release tags are the source
of truth. `package-desktop.py` stamps `--version` into `Cargo.toml` and
`Cargo.lock` for the build and restores both afterwards. Local builds
therefore report `0.0.0`, while packaged builds report the version passed to
the packager. To download the five artifact sets without publishing, use
**Actions → Release → Run workflow**.

Release assets carry GitHub build provenance attestations:

```sh
gh attestation verify path/to/downloaded-file -R leesiongchan/bulletnotes
```

If publishing fails after the tag was created, delete the partial release and
tag with `gh release delete vX.Y.Z --cleanup-tag --yes`. Then re-run the
failed jobs of that **Release** run. The workflows require no custom secrets.
Only the publication job receives write permissions.
````

- [ ] **Step 2: Note the placeholder in the notarized path**

In `## Distribution prerequisites`, directly after the sentence ending
`requires a clean committed worktree and explicit \`--notarize\`:`, the code
block follows. Directly before that code block, insert this paragraph:

```markdown
`package-macos.sh` requires `--version` to equal the `Cargo.toml` version, and
`main` and release tags hold the `0.0.0` placeholder. A numbered notarized
build therefore needs release-version support in that script first.
```

- [ ] **Step 3: Check the rendered text**

Run: `rg -n 'git tag v0.1.0|prerelease|--generate-notes' docs/build.md`
Expected: no matches. The old manual tag instructions and the prerelease
wording are gone.

- [ ] **Step 4: Commit**

```bash
git add docs/build.md
git commit -m "docs: describe the automated release flow" -m "Explains the commit convention, version rules, placeholder stamping, attestation verification and the one manual recovery step."
```

---

### Task 6: Final verification

- [ ] **Step 1: Run every local check**

```bash
python3 -m unittest discover -s tests -p 'test_package_desktop.py'
bash scripts/check.sh
actionlint .github/workflows/*.yml
bash /tmp/cliff-check.sh cliff.toml
printf '[git]\nconventional_commits = true\nfilter_unconventional = false\nrequire_conventional = true\n' > /tmp/cliff-commits.toml
git-cliff --config /tmp/cliff-commits.toml "$(git merge-base origin/main HEAD)..HEAD" > /dev/null && echo "branch commits conventional"
git status --porcelain
```

Expected:
- every command exits 0
- the cliff check ends with `ALL PASS`
- `branch commits conventional` is printed
- `git status` shows a clean tree

- [ ] **Step 2: After merge, check the bootstrap release end to end**

This is for the maintainer, once the PR is merged.
- **Release run.** **Actions → Release** shows a run triggered by
  `workflow_run`, with `plan`, five `build` jobs and `publish` all green.
- **The release.** Run `gh release view v0.1.0` and check:
  - it is not a prerelease and is Latest
  - the title is `Bulletnotes 0.1.0`
  - the assets come from all five targets, plus `.verification.json` and
    `.sha256` files
  - the notes end with `## Changes` followed by `Initial release.`
- **Attestation.** Run
  `gh release download v0.1.0 -p '*x86_64-unknown-linux-gnu*.deb' -D /tmp/rel`,
  then `gh attestation verify /tmp/rel/*.deb -R leesiongchan/bulletnotes`.
  Expected: verification succeeds.
- **Nothing to release.** A later `docs:`-only merge produces a Release run
  whose `plan` logs `No releasable commits since v0.1.0`. `build` and
  `publish` are skipped.
