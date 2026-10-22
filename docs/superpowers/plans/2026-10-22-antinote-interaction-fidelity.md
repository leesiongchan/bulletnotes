# AntiNote Interaction Fidelity Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver the approved editing interactions, quieter chrome, and clearer search previews.

**Architecture:** Execute three component plans in sequence, then validate the integrated application. They share the maintained Slint patch, so their source mutations must not run concurrently. The editor plan establishes native composition and atomic-edit contracts before exposing editing commands.

**Tech Stack:** Rust, Slint 1.18.1, existing winit 0.30.13, pulldown-cmark, unicode-segmentation, SQLite FTS5.

**Spec:** [Approved design](../specs/2026-10-22-antinote-interaction-fidelity-design.md).

## Global Constraints

- "Keep the current Rust/Slint editor and maintained Slint 1.18.1 patch."
- "No dependency upgrade, new package, or database migration is required by this design."
- "All offsets refer to UTF-8 source bytes."
- "Each command creates one nonmerging history entry, including pure insertion or deletion."
- "While that state is unknown, decline custom commands without disturbing native input."
- "Render note text literally and add only the supplied match emphasis."
- Use generated notes, temporary stores, and controlled sync fixtures; never open the user's store for verification.
- Use the repository's Rust 1.99.1 verification commands. If a CLI is absent on this Nix host, retry through `,`.
- All signatures described as NEW in these plans are additions to implement, not existing upstream APIs.

## Review Focus

- Empty-preedit candidate transitions must not admit a command: editor Task E1 includes native and synthetic tests.
- A pointer activation with an offscreen caret must not scroll: editor Task E5 includes viewport assertions.
- Full-line formatting selections must retain structural prefixes: editor Task E3 pins exact resulting bytes.
- Revealing controls must not change editor layout or hide failures: chrome Task C1 owns these tests.
- A highlighted hit must remain visible without interpreting note markup: search Tasks S1/S2 own literal-text and layout tests.

---

## File Structure and Component Plans

| Order | Plan | Independently testable deliverable |
| --- | --- | --- |
| 1 | [Editor interactions](2026-10-22-antinote-editor.md) | Native composition guard, atomic undo, source commands, keyboard and checkbox interaction. |
| 2 | [Chrome](2026-10-22-antinote-chrome.md) | Paper header, quiet buttons/footer, working existing hover contract. |
| 3 | [Search](2026-10-22-antinote-search.md) | FTS excerpts, explicit highlight ranges, literal two-line previews. |
| 4 | Task F1 below | Integrated verification and documentation. |

Editor and search both modify `patches/slint/slint-1.18.1.patch`; search extends ordinary `Text` using the existing decoration model. Each component lists exact files and owns its own red/green checks. Do not rewrite existing Markdown painting, reconciliation, or sync algorithms.

## Execution Preparation

- [ ] Use `superpowers:using-git-worktrees` at execution time, following its environment detection. Preserve this checkout's approved docs and any subsequent user edits.
- [ ] Read the spec and all four plan documents. Establish the current baseline with `scripts/check.sh` and `python3 scripts/check-markdown.py`; retain complete output under `target/antinote-interactions/`. The known historical floating fixture fails its obsolete two-undo expectation until C1 updates it; do not represent that baseline as passing.
- [ ] Execute E1 first. If native evidence cannot establish the composition contract, report the specific unsupported transition and keep editor command activation blocked. Chrome/search can still be completed independently, but the full feature must not be reported complete without editor acceptance.

### Task F1: Integrated Evidence and Handoff

**Files:** Modify `docs/usage.md`, `README.md`, `docs/validation/release-matrix.md`; create `docs/validation/antinote-interactions.md` and generated-only images under `docs/validation/antinote-interactions/`.

**Interfaces:** Consumes editor E1–E5, chrome C1, and search S1–S2. Produces documented implemented shortcuts, evidence, and explicit remaining platform/input gates.

- [ ] **Step 1: Verify the integrated build.** Run `scripts/check.sh` and `python3 scripts/check-markdown.py`. Require exit zero, formatting and strict Clippy success, and zero failing automated tests. Record ignored tests separately. Subsequent changes require rerunning affected checks; repeat the full suite only if those changes invalidate it.
- [ ] **Step 2: Verify native behavior after rebuilding examples.** Run:

```sh
rustup run 1.99.1 cargo build --locked --features native-validation --example ui_review --example local_review --example editor_commands_review --example editor_composition_review --example search_preview_review --example editor_native_input_review
rustup run 1.99.1 cargo build --locked --features native-validation,protocol-validation --example sync_review
target/debug/examples/editor_composition_review --pinyin
target/debug/examples/editor_commands_review
target/debug/examples/search_preview_review
target/debug/examples/ui_review --floating-contract
target/debug/examples/ui_review --shortcut-contract
target/debug/examples/ui_review --contract
target/debug/examples/ui_review --scrollbar-contract
target/debug/examples/local_review --scratchpad-contract
target/debug/examples/sync_review
target/debug/examples/editor_native_input_review --pinyin --require-single-undo
```

Require each existing fixture's success marker, including `native_sync_contract_passed: true`, and the NEW `native_composition_guard_passed: true`, `native_editor_commands_passed: true`, and `native_search_preview_passed: true`. A source unavailable for real input-method testing is an unverified gate, not a synthetic substitute.

- [ ] **Step 3: Review evidence.** Inspect before/after images at 505×552 and 400×440 in Sanrio/Tartan and Mononoke, with hidden/header/footer chrome, increased contrast, visible save failure, and long search matches. Require no reflow, wrong task activation, clipping of required actions, or hidden match. Run existing controlled sync/restart fixtures after exercising an actual command; require exact source on both sides. Scope any unavailable native platform checks explicitly.
- [ ] **Step 4: Update documentation.** Document the exact shortcuts, list/formatting boundaries, one-step undo, preserved sync behavior, native composition proof/limitations, and search changes. Do not promote historical evidence to fresh coverage. Keep user note content, titles, paths, credentials, and clipboard contents out of evidence.
- [ ] **Step 5: Commit and review.** Run `git diff --check`, inspect changed-file scope, and commit `Record Antinote interaction validation`. Obtain the final review required by the selected execution workflow, resolve material findings, and rerun affected checks before reporting completion.

## Coverage and Execution Choice

| Spec area | Owner |
| --- | --- |
| Atomic edit/history contract | E2 |
| Composition and permission guards | E1, E2, E4 |
| Checkbox pointer/keyboard behavior | E3, E4, E5 |
| Lists, formatting, line movement | E3, E4 |
| Existing persistence and failure barriers | E4, F1 |
| Chrome/accessibility/reveal invariants | C1, F1 |
| Search matching, literal rendering, row geometry | S1, S2 |
| Fresh patched sources and final evidence | E1/E2/S2 verification, F1 |

Recommend **Native execution** in this session, followed by an independent whole-branch review: the editor tasks and search rendering share a tightly coupled private patch, and intermediate tests already provide task-level checks. Subagent-driven execution remains available with a fresh implementer and reviewer for each task. The user must review the plans and choose a method before implementation.
