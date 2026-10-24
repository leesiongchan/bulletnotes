# Dependency and non-UI release checks — 2026-10-24

The user deferred performance measurements, screenshots and package-launch
verification. This pass used compilation, audits, generated loopback/storage
tests, attribution generation and static architecture checks only.

## Certificate-trust fixes

Cargo-audit 0.22.2 was verified against its [official release source](https://github.com/rustsec/rustsec/releases/tag/cargo-audit%2Fv0.22.2)
and run through Nix comma against the existing application lockfile. The current
RustSec database has 1,351 advisories, commit
`84d943a6deeb8cc6c6f632531aaf4f1c62275239`, updated 23 October 2026.

The initial audit failed with two vulnerabilities published after the previous
17 October audit. Only these two transitive versions and their checksums changed:

| Package | Before | After | Advisory |
| --- | --- | --- | --- |
| rustls-webpki | 0.103.15 | 0.103.16 | [RUSTSEC-2026-0382](https://rustsec.org/advisories/RUSTSEC-2026-0382.html) |
| webpki-root-certs | 1.0.9 | 1.0.11 | [RUSTSEC-2026-0383](https://rustsec.org/advisories/RUSTSEC-2026-0383.html) |

The patched versions were checked against the official
[webpki](https://github.com/rustls/webpki/releases/tag/v%2F0.103.16) and
[root-certificate](https://github.com/rustls/webpki-roots/releases/tag/v%2F1.0.11)
releases. They address trust in distrusted emSign intermediates/root certificates.
`rustls-webpki` is selected through the macOS reqwest/rustls graph; the bundled
root package is absent from that selected graph. Presence in the dependency graph
does not establish an exploitable certificate path on this host.

The post-update application audit passed with zero known vulnerabilities and no
ignored advisories. The independent editor lockfile also reports zero known
vulnerabilities and was left unchanged. No HTTP configuration, credential flow,
TLS exception, manifest dependency or unrelated lockfile edge changed.

## Retained warnings

Both audits report five unmaintained packages (`bincode`, `ttf-parser`,
`zbus_names`, `zvariant`, `zvariant_derive`) and one yanked package
(`clipboard-win` 5.4.1). The yanked warning has no associated vulnerability
advisory. All six are absent from the supported `aarch64-apple-darwin` graph
with both default and all features, checked using `cargo tree --locked`.

These warnings are retained without suppression. Recheck them before production
distribution or on 1 November 2026, whichever comes first; the independent editor
CI's other platforms need their own reachability assessment before a dependency
change is chosen for those platforms.

## Verification

- After the update, `scripts/check.sh` passed formatting, strict all-target/all-feature
  Clippy and 396 application tests, zero failures, 24 explicit ignores.
- After the update, `rustup run 1.92.0 cargo check --all-targets --locked` passed
  without warnings. The same check also passed before the update.
- The unchanged independent editor retains this session's 45 passing renderer,
  composition and history tests; its lockfile and code did not change.
- Two existing Python validation-runner failure regressions passed. Two existing
  static Intel/universal artifact rejection tests passed without package launch.
- Attribution regeneration verified full Slint license text and covered nine
  recognized custom-license warnings. Before the update, notices were byte-identical;
  afterward, the only notice change is the `rustls-webpki` version label.
- Independent review confirmed exactly two version/checksum changes, unchanged
  unrelated lock entries, matching cached crate checksums and the narrow notice diff.

[The structured audit](dependency-audit-2026-10-24.json) retains the complete
post-update report. Initial/final audits, reachability, compiler and test logs are
under `target/release-acceptance-20261024/`. This is release preparation, not native
or production distribution acceptance. No live space, user store or Keychain was
opened, and nothing was published or notarized.
