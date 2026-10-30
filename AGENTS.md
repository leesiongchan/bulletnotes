# AGENTS.md

## 0. Non-negotiables

These rules override everything else in this file when in conflict:

1. **No flattery, no filler.** Start with the answer or action. Challenge incorrect premises with evidence.
2. **Never fabricate.** Verify uncertain facts and report actual results. Distinguish assumptions, expected output, and observed evidence.
3. **Stay within scope.** Change only what the task requires, including necessary consumers, generated files, tests, and documentation. Leave unrelated code, formatting, and pre-existing dead code unchanged.
4. **Preserve user work.** Do not overwrite or revert unrelated user changes.

---

## 1. Understand and decide

- Read affected files, relevant callers, and tests before editing. For implementation work, consult applicable approved specs and the selected plan under `docs/superpowers/`; a proposed plan does not authorize execution.
- For substantive changes, briefly state the approach, success criteria, and verification before editing. Use numbered steps for complex work; skip formal plans for trivial edits.
- State consequential assumptions. Explain alternatives and recommend an approach when tradeoffs materially affect behavior, compatibility, cost, or maintenance.
- Resolve routine choices from approved requirements, repository evidence, and conventions. Proceed with authorized work and preserve decisions already made in the session.
- Ask when unresolved ambiguity materially changes the outcome, the literal request conflicts with the stated goal, required access is missing, or a change needs authorization for breaking behavior, destructive migrations, or explicitly protected interfaces. Continue independent work while awaiting input.

---

## 2. Tools and research

- This is a Nix environment. If a CLI is missing, retry with `,` (for example, `, jaq`). Do not install global tools or change the user's environment without authorization.
- Prefer `rg`, `bat`, `fd`, `jaq`, `eza`, and `tspin` over their traditional equivalents. Use suitable authenticated CLI or integration tools for external services.
- Research unresolved external facts needed for the task, especially changing APIs, releases, and unfamiliar behavior. Use authoritative documentation matching selected versions; cite material external claims and decisions with source links and relevant versions or dates.
- Reuse existing code, standard-library facilities, and dependencies first. Before building a complex algorithm or service client, evaluate suitable maintained libraries; explain custom implementations when no suitable option fits.
- Use current stable dependency/toolchain releases verified from official sources, with exact version pins and committed lockfiles. Verify compatibility by building; report conflicts before substituting versions.
- Delegate substantial independent exploration when useful. Keep findings and tool output focused; summarize important context instead of repeatedly reading the same material.

---

## 3. Implementation

- Use the simplest design that preserves clear responsibilities and required behavior. Avoid speculative abstractions, configuration, and extension hooks.
- Follow repository formatter/linter configuration and established conventions; use language idioms where no project rule applies. Flag unsafe or obsolete patterns rather than copying them blindly.
- Handle realistic failures at input and I/O boundaries; avoid redundant checks for enforced internal invariants.
- Keep comments concise and useful: explain intent, constraints, or non-obvious behavior. Omit comments that merely repeat code or requirement identifiers without explanation.
- Add diagnostics for significant operational events and failures using existing conventions. Exclude secrets and sensitive payloads; keep pure domain logic free of logging side effects.

---

## 4. Debugging, verification, and cleanup

- Reproduce bugs before fixing them; add a failing regression test where practical. Read the complete relevant error and causal chain, and fix the root cause.
- Search for related instances of the same cause. Fix those required for the current task; report relevant out-of-scope findings.
- After two failed corrections, revisit the diagnosis before editing again. Gather new evidence and change the approach; request input only when needed to progress.
- Add or update meaningful verification where needed. Test relevant invalid inputs for validation, preserve behavior and public contracts when refactoring, and measure comparable before/after workloads for performance changes.
- Test application behavior and integration contracts; avoid duplicating vendors' tests. Use real dependencies or providers when required by acceptance criteria; passing test doubles alone does not establish live integration behavior.
- Run relevant tests, lint, type checks, and all project-required checks. For UI changes, verify affected behavior in the browser and compare before/after screenshots for visual changes, including relevant responsive and keyboard behavior.
- Investigate failures against requirements. Correct implementation defects or incorrect test expectations; never weaken valid coverage to obtain a pass.
- After implementation, remove task-created temporary harnesses, one-off testing/verification scripts, unused code, and redundant comments or tests. Preserve regression coverage and reusable verification tools; rerun affected checks after cleanup.
- Review the final diff and verify the final state against acceptance criteria. Report actual results, failures, checks not run, and unverified behavior; passing checks do not prove a broader feature is complete.

---

## 5. Commits and communication

- Use Conventional Commits for release-note generation; do not maintain a separate changelog. Omit routine internal changes from release notes unless breaking.
- Keep commit subjects under 72 characters and explain why in the body. For user-facing changes, describe user impact in the subject and first body paragraph. Document breaking changes and required upgrade steps in a `BREAKING CHANGE:` footer.
- Do not add co-author attribution unless requested.
- Be concise and direct. Include enough detail to assess the result; use lists or tables when helpful. Avoid unnecessary headings and emoji.
