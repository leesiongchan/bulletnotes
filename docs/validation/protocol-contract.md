# SilverBullet HTTP protocol contract

The adapter uses reqwest 0.13.5 with platform certificate validation, HTTPS user connections, a 10-second connect timeout, a 30-second operation timeout, redirects disabled, and transport retries disabled. The sync worker owns retries and concurrency. Loopback HTTP and shorter deadlines are explicitly gated by the `protocol-validation` feature for integration fixtures.

`ConnectionId` and segment-encoded page paths scope every request. Literal percent signs remain literal filename characters. Every write supplies `If-None-Match: *` or a validated strong `If-Match` revision. A dedicated `X-SilverBullet-Revision` takes precedence over a proxy ETag; an invalid dedicated revision never falls back to another revision namespace. Missing permission or revision permits reading but prevents writable metadata.

Reads validate media type, identity content encoding, UTF-8, and metadata. Neither login HTML nor malformed bytes become notes. A complete catalogue is bounded at 16 MiB and a page at 64 MiB, enforced both before and during streaming; excess responses fail without truncating or changing remote content. These are transport resource budgets, not claims that oversized notes are cached. The progressive cache must surface the classified failure.

Mutation timeouts, transport failures, server 5xx responses, missing write revisions, and successful-looking login HTML preserve an unknown outcome. A caller must read back before considering a retry. Error types and token Debug formatting exclude URLs, paths, source text, and secrets; HTTP errors are classified rather than exposed verbatim.

## Automated evidence

The stateful axum 0.8.9 fixture uses generated data and ephemeral loopback listeners. Its eleven contract cases cover exact CRLF/frontmatter/Unicode source preservation, URL prefixes and encoded filenames, guarded creation/update/deletion, stale revisions, authentication/permission errors, invalid catalogue identity and metadata, HTML/UTF-8/encoding rejection, dedicated revisions behind proxy ETags, cross-origin token isolation, foreign-space refusal, and a timeout after a server-applied write with exactly one request.

```sh
rustup run 1.99.1 cargo test --test protocol_contract --features protocol-validation --locked
rustup run 1.99.1 cargo test --all-targets --all-features --locked
```

The full suite passed 95 tests, with 12 native/manual tests explicitly ignored. Native editing/accessibility evidence remains open. This adapter does not yet enable automatic synchronization in the running local preview; connection-scoped verification and the durable sync worker are separate plan tasks.

## Official references

- [SilverBullet HTTP API](https://docs.silverbullet.md/HTTP%20API)
- [SilverBullet filesystem handler](https://github.com/silverbulletmd/silverbullet/blob/main/server/src/handlers/fs.rs)
- [reqwest ClientBuilder](https://docs.rs/reqwest/0.13.5/reqwest/struct.ClientBuilder.html)
- [axum serve](https://docs.rs/axum/0.8.9/axum/fn.serve.html)
