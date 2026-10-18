# Merge library evaluation, 2026-10-18

This is dependency-selection evidence, not production implementation or release
acceptance. All executable probes used generated text in a temporary standalone
Cargo project. The application manifest and lockfile were not modified.

## Decision

Use `gix-merge = "=0.22.0"` with default features disabled and `sha256` enabled,
`gix-imara-diff = "=0.3.1"`, and `unicode-segmentation = "=1.13.3"`.
Gitoxide supplies the diff and three-way merge algorithm. Bulletnotes supplies
source-preserving token adaptation, the server-overlap policy and durable recovery.

The exact stable, non-yanked releases were checked against the crates.io API and
downloaded published source, rather than inferred from cached search results.
The merge and diff crates were published on October 8, 2026 and require Rust 1.88;
the tokenizer was published June 1, 2026 and requires Rust 1.85. The selected probe
also compiled and passed on the application's Rust 1.92 minimum.

References: [merge release](https://crates.io/crates/gix-merge/0.22.0),
[diff release](https://crates.io/crates/gix-imara-diff/0.3.1),
[Gitoxide repository](https://github.com/GitoxideLabs/gitoxide),
[Unicode segmentation](https://docs.rs/unicode-segmentation/1.13.3/unicode_segmentation/).

## Candidates and tradeoffs

| Candidate, verified stable | Finding |
| --- | --- |
| `gix-merge 0.22.0` | Maintained Gitoxide engine; public prepared merge API and `ResolveWithTheirs` preserve independent changes while selecting a side only in conflicts. Its native input unit is a line. Selected with a lossless word-token adapter. MIT/Apache-2.0. |
| `diffy 0.5.2` | Current maintained release, Rust 1.85. Public merge output exposes marker-rendered strings rather than structured regions or a favor-server policy. Avoid parsing markers out of arbitrary user Markdown. |
| `merge3 0.2.1` | Generic structured regions are a good API fit. Default `difflib` backend passed correctness probes but performed poorly on repeated text. Its faster optional `patiencediff 0.3.0` backend passed, but introduces GPL-2.0+ and additional dependencies. Not selected. |
| `reconcile-text 0.13.2` | Retains competing insertions through OT-inspired merging, rather than the selected server-overlap policy. Requires Rust 1.94, above this application's minimum. Not selected. |
| `threeway_merge 0.1.20` | Git xdiff wrapper with side-favoring, but no public word-token or structured-region API. Adds C FFI and statically linked LGPL-2.1+ xdiff distribution obligations. Not selected. |
| `diffy-imara 0.3.2` | Faster diff backend, but retains the marker-rendered public merge contract. Not selected. |

API/source references: [Diffy options](https://docs.rs/diffy/0.5.2/diffy/struct.MergeOptions.html),
[Merge3](https://github.com/breezy-team/merge3-rs),
[reconcile-text](https://docs.rs/reconcile-text/0.13.2/reconcile_text/),
[threeway_merge](https://docs.rs/threeway_merge/0.1.20/threeway_merge/),
[xdiff licensing](https://github.com/levish0/threeway-merge-rs#license).

## Executable probe

The Gitoxide probe used `text::Merge::new` and `Merge::run`, Histogram diff, and
`Conflict::ResolveWithTheirs`. Unicode `split_word_bounds` retains punctuation,
whitespace, line endings and combining sequences. A shared per-request dictionary
encoded each token as an integer followed by LF; the engine merged those internal
lines and the decoder reconstructed exact original token bytes. Raw user text
was never parsed as a conflict-marker format. This adapter does not implement a
diff, conflict detector or three-way alignment algorithm.

Ten targeted cases passed: independent lines, compatible words on one line,
overlap plus a separate local change, Unicode/CRLF, literal conflict markers,
missing final newline, competing insertions, identical changes, combining marks
and multi-codepoint emoji, and overlapping deletion/edit. Another 3,000 generated
identity and one-sided cases passed. Both Rust 1.99.1 and 1.92.0 passed the same
cases in release mode on the current Apple Silicon development host.

Single-run timings on repeated seven-word vocabulary, replacing different words
on each side (milliseconds; these are probes, not latency guarantees):

| Words | Merge3 default | Merge3 patience | Gitoxide token adapter, Rust 1.99.1 |
| --- | ---: | ---: | ---: |
| 128 | 9.419 | 0.072 | 0.097 |
| 512 | 501.968 | 0.262 | 0.484 |
| 1,024 | 4,033.657 | 0.495 | 0.883 |
| 2,048 | 36,336.055 | 0.980 | 1.739 |

The production adapter must additionally verify raw line merging, admission
budgets, generated concurrent edit cases, mapping of available cursor positions,
malformed internal output handling and the complete store/editor/network workflow.
No library supplies the application's crash recovery, retry policy or durability.

## Dependency audit

Cargo audit 0.22.2 checked the selected probe's 113-package lockfile against RustSec
commit `999ec89d5d3f10f20be14750ec8c2d8b44e0ac9b`, updated October 17, 2026:
zero reported vulnerabilities and zero informational warnings. This is a dated
advisory check, not proof that a dependency has no defects. Repeat against the
actual application lockfile after integration.

The unchanged application's 644-package lockfile had zero reported vulnerabilities
and maintenance warnings for `bincode`, `ttf-parser`, `zbus_names`, `zvariant` and
`zvariant_derive`. Default Apple Silicon normal-dependency inverse trees contained
neither the flagged bincode nor ttf-parser packages; build/feature/target reachability
still needs classification before release. Do not silently suppress these warnings
or describe the full application graph as warning-free.

## Resource contract

The selected synchronous merge API has no cooperative cancellation hook. A timeout
does not stop a running blocking task. Production must limit admitted input and
tokens, run outside UI/SQLite ownership, bound concurrently executing jobs, keep
the slot until the actual computation ends, and reject late results. Budget fallback
uses the already agreed server-document/local-recovery policy. Exact limits and
tests are specified in the implementation plan.
