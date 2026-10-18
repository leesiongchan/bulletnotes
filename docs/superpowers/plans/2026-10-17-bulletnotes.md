# Bulletnotes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. The user must review this plan and choose the execution method before implementation.

**Goal:** Deliver the approved focus-first Rust/Slint macOS client, with first-class themes, durable local editing, automatic SilverBullet synchronization, and verified production packaging.

**Architecture:** One Rust application with custom Slint views, a UI coordinator, a dedicated SQLite worker, and an asynchronous HTTP sync worker. Typed, revision-tagged messages keep disk/network work off the UI thread and prevent stale acknowledgements from replacing newer edits. A small macOS adapter owns platform integration.

**Tech Stack:** Rust 2024, Slint, Tokio, reqwest, rusqlite/SQLite, Serde, objc2/AppKit, macOS Keychain; exact researched versions appear below.

**Spec:** [Approved design](../specs/2026-10-17-bulletnotes-design.md). Read it and its [interactive reference](../specs/assets/bulletnotes-focus-first.html) before implementing any task. The reference is a design artifact; no HTML or browser runtime belongs in the app.

## Global Constraints

- Target macOS 14 and newer on Apple Silicon only (`aarch64-apple-darwin`), per the user’s 2026-10-18 scope correction.
- Default window content is approximately 505 by 552 logical pixels; minimum is 400 by 440. Use the requested frameless floating-note direction; retain Dock access as a stated working default until the optional menu-bar-only preference is answered.
- Only one of Note, All pages, or Appearance is visible in the main window. Preserve text, selection, scroll, and search query during navigation.
- Ship Paper, Graphite, Matcha, and Rose, using Bulletnotes theme format version 1 only. Antinote theme import is deferred.
- Theme files are data-only UTF-8 JSON, limited to 32 KiB; names contain 1–32 Unicode scalar values after trimming; colors are opaque `#RRGGBB` strings.
- Independent light/dark theme assignments; plain, dotted, ruled, and grid paper; editor size 11–24 logical pixels, default 15.
- Built-in normal-text contrast is at least 4.5:1; essential control/focus contrast is at least 3:1. Preserve a user-accessible valid palette.
- User-approved revision B defers native blur/translucency. Use solid themed paper on every supported macOS version; text and controls stay opaque. No first-release translucency preference.
- Coalesce local edits for at most 250 ms; acknowledge Saved locally only after durable commit. Debounce remote writes for 750 ms after local commits.
- At most one mutation is in flight per page; stable operation IDs and note paths survive retries and restarts.
- Poll the catalogue approximately every 30 seconds active and every 60 seconds hidden; refresh on activation/reconnect; pause when offline.
- Remote changes use atomic revision guards; preserve both sides of conflicts. No unconditional-write fallback and no automatic expiry of existing pages.
- Source Markdown, unsupported syntax, indentation, Unicode, and frontmatter survive editing. No execution of note text or theme data. User-approved revision A uses the supported plain-text Slint editor; inline heading/link colors are deferred, while full themes remain required.
- Capture weak Slint handles in callbacks; event loop/components stay on the main thread. No blocking disk or network calls on that thread.
- Use Keychain for tokens, parameterized SQL, HTTPS, and local diagnostics without note contents, paths, titles, tokens, or clipboard text.
- Production distribution requires signed/notarized artifacts and recorded native evidence. A development build is not the finished production release.
- Defer math, conversions, OCR, AutoPaste, timers, utility extensions, SilverBullet runtime/plugins, attachment management, renames, automatic updates, and Antinote theme import.
- Preserve the user's untracked `AGENTS.md`. Create an isolated worktree at execution time using the worktree skill; do not create it while planning.

## Review Focus

1. Multi-code-point characters, IME composition, CRLF/frontmatter, and indented paste must not corrupt text or move the caret; Tasks 1 and 6 pin native and stored round trips.
2. A successful-looking login HTML response, literal percent signs, or traversal-shaped catalogue entry must never become a writable page; Task 7 pins media-type, origin, and path handling.
3. A timeout after the server applied a mutation, followed by a newer local edit or recreated remote path, must not lose either version; Tasks 9 and 10 pin reconciliation and delayed acknowledgements.
4. Disk-full failures, interrupted migrations, and two local app processes must preserve every acknowledged local revision and prevent closing an unsaved buffer; Tasks 4 and 11 pin these cases.
5. Duplicate JSON keys, unsupported versions, non-ASCII names, and low-contrast custom themes must fail or require explicit application without replacing the current valid palette; Tasks 3 and 5 pin this behavior.

## Delivery boundaries

This is one integrated client with three sequential, independently testable milestones. The modules share revision and persistence contracts, so do not implement them as unrelated parallel projects.

| Milestone | Tasks | Demonstrable result |
| --- | --- | --- |
| Native UI and durable local notes | 1–6 | Approved note/search/Appearance flows, working themes, crash-safe local notes |
| Automatic SilverBullet sync | 7–10 | Entire space browsable, guarded bidirectional sync, recoverable conflicts/deletion |
| macOS production release | 11–13 | Native lifecycle/accessibility, measured behavior, signed/notarized installation |

The first milestone serves the user's UI priority; it does not complete the request without the later sync and release milestones. Stop at a failed native feasibility gate rather than silently weakening the approved design.

## Dependency and environment decisions

The local commands reported Rust `1.98.0` and Cargo `1.98.0`. The official Rust release announcement identifies `1.99.1` as released on 2026-10-15; pin `rust-toolchain.toml` to `1.99.1` at execution time. Official crates.io API metadata checked on 2026-10-17 reported Slint `1.18.1` with Rust minimum `1.92`. Use edition `2024` and declare package `rust-version = "1.92"`; check both the declared minimum and pinned development toolchain before release.

These are researched starting versions, not an installed or compatibility-tested dependency graph. Add dependencies only in the owning task, using exact `=version` manifest requirements and a committed `Cargo.lock`. If execution starts later, recheck official metadata and advisories first; explicitly document any necessary change. Resolve the graph only after execution approval.

| Dependency | Exact version | Ownership / feature policy |
| --- | --- | --- |
| `slint`, `slint-build` | `1.18.1` each | Task 1; matching versions; explicit backend/features |
| `serde`, `serde_json` | `1.0.229`, `1.0.154` | Task 3; Serde derive; duplicate-key detection through a Serde visitor |
| `thiserror` | `2.0.21` | Task 2; typed errors |
| `uuid`, `jiff` | `1.28.0`, `0.2.38` | Task 2; UUID v4; local calendar paths with injected time in tests |
| `tokio` | `1.53.3` | Task 4; `rt-multi-thread`, `sync`, `time`, `macros`; add `net` for HTTP and `test-util` in tests |
| `rusqlite` | `0.40.2` | Task 4; disable defaults, enable `bundled`, `backup`; no extension loading |
| `tempfile` | `3.27.0` | Task 4 dev dependency; isolated persistence tests |
| `rfd` | `0.17.2` | Task 5; native theme import/export dialogs |
| `url` | `2.5.8` | Task 2; canonical connection identity and safe path construction |
| `reqwest` | `0.13.5` | Task 7; defaults disabled, `rustls`, `http2`, `json`, `stream`; no lossy charset conversion |
| `axum` | `0.8.9` | Task 7 dev dependency; controlled stateful HTTP fixture |
| `security-framework` | `3.7.0` | Task 8, macOS-only; Keychain wrapper, avoiding a second cross-platform credential abstraction |
| `objc2`, `objc2-app-kit`, `objc2-foundation` | `0.6.5`, `0.3.2`, `0.3.2` | Tasks 1/11, macOS-only; enable only used documented framework APIs |
| `raw-window-handle` | `0.6.2` | Task 1, macOS-only; supported Slint window handle |
| `global-hotkey` | `0.9.0` | Tasks 1/11; main-thread registration and conflict reporting |
| `tracing`, `tracing-subscriber`, `tracing-appender` | `0.1.44`, `0.3.23`, `0.2.5` | Task 12; local diagnostics and bounded retention |

Start Slint with defaults disabled and `std`, `backend-winit`, `renderer-femtovg`, `accessibility`, `compat-1-18`, `raw-window-handle-06`, and `system-tray`. Expose a development-only `native-validation` feature for native test harnesses and a `probe-skia` feature adding `slint/renderer-skia`. Compare the stable FemtoVG and Skia paths in Task 1 and keep only the proven production renderer. Do not adopt experimental Vello, a custom renderer, or private Slint APIs. Do not enable version-specific unstable winit APIs unless the user reviews that change.

Official metadata is available at `https://crates.io/api/v1/crates/<name>` and each version's linked repository. The HTTP library replaces transport machinery, not the application-specific revision protocol: use a small typed adapter for SilverBullet's documented filesystem API. No official reusable Rust client SDK was identified in the repository/docs research; reassess before replacing this adapter with another dependency.

## File and contract map

Create one package, `bulletnotes`, with a library for testable core behavior and a thin desktop binary. The following are new paths unless a previous task has already created them.

| Files | Responsibility |
| --- | --- |
| `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`, `.cargo/config.toml`, `build.rs` | Reproducible compiler/dependency/Slint build configuration; macOS deployment target 14.0 |
| `src/lib.rs`, `src/main.rs` | Core module exports and desktop composition root |
| `src/domain/{mod,page,connection,status,error}.rs` | Page/connection identity, revisions, operations, typed states/errors |
| `src/theme/{mod,format,palette,contrast}.rs`, `assets/themes/{paper,graphite,matcha,rose}.json` | Validated theme contract and built-ins |
| `src/store/{mod,worker,pages,settings,outbox,recovery,search}.rs`, `migrations/001_initial.sql` | Single-owner SQLite persistence, search, recovery, and operation queue |
| `src/app/{mod,model,coordinator,navigation,editor,appearance,connection,recovery}.rs` | Application state and UI/worker routing |
| `src/sync/{mod,client,protocol,probe,catalogue,worker,reconcile}.rs` | HTTP contracts, capability evidence, downloads, and sync transitions |
| `src/platform/{mod,macos,credentials}.rs` | Narrow macOS lifecycle/material/Keychain integration |
| `src/diagnostics.rs` | Local structured logs and redaction policy |
| `ui/{app-window,note-view,page-browser,appearance-view,connection-view,recovery-view,theme}.slint`, `ui/components/{icon-button,paper,status-strip}.slint` | Custom native UI and semantic palette |
| `examples/{native_gate,native_window_probe}.rs`, `tests/native_gate.rs`, `tests/ui_contract.rs` | Early native feasibility and UI contracts |
| `tests/{domain,theme_contract,theme_contrast,store_recovery,store_search,app_navigation,editor_roundtrip,connection_security,protocol_contract,compatibility_probe,sync_lifecycle,conflict_recovery,macos_lifecycle,diagnostics}.rs` | Behavioral tests, introduced by their owning tasks |
| `tests/support/{mod,http_fixture,clock,storage_fixture}.rs`, `tests/fixtures/` | Shared test-only helpers and representative inputs |
| `docs/validation/{native-gate,visual-matrix,release-matrix}.md`, `docs/usage.md` | Observed native evidence, visual comparisons, release results, user instructions |
| `benches/interaction.rs`, `scripts/{check,package-macos,verify-release}.sh`, `packaging/{Info.plist,entitlements.plist}` | Performance harness, reproducible verification, macOS artifacts |

Use module-level private helpers until an interface below needs to cross a module boundary. `src/lib.rs` exports `domain`, `theme`, `store`, `app`, and `sync` as those modules become available. Native entry points remain macOS gated. Tests must use isolated temporary data, never the user's real Application Support directory or server by default. Tests that open a real Slint window are explicitly ignored by default and launch a separate native process whose main thread owns the event loop; run them with `--ignored` in an interactive macOS environment. Pure UI contract tests exercise the presentation model without creating a window.

## Milestone 1: native UI and durable local notes

### Task 1: native fidelity gate and reproducible desktop harness

**Files:** Create the root build files, `src/lib.rs`, `src/main.rs`, `examples/native_gate.rs`, `examples/native_window_probe.rs`, `tests/native_gate.rs`, initial `ui/app-window.slint`, initial `src/platform/{mod,macos}.rs`, initial `src/domain/{mod,error}.rs`, and `docs/validation/native-gate.md`. Modify `.gitignore` for build outputs only.

**Interfaces:** Produce the compiled Slint `AppWindow`; `platform::macos::probe_window(window: &slint::Window) -> Result<NativeProbeReport, PlatformError>`. Define `NativeProbeReport` with named pass/fail observations for material composition, focus restoration, pinning, activation, and OS version. This report is validation data, not a production capability claim.

- [ ] **1. Establish a failing acceptance case.** Create the smallest compilable Rust/Slint harness, using the researched pins. In `tests/native_gate.rs`, add `text_input_roundtrips_unicode_and_crlf` asserting that `"---\r\ntitle: café\r\n---\r\n\t👩🏽‍💻 日本語\r\n"` survives set/get byte-for-byte. Define native observation rows for composition, undo/redo, selection, wrapped scrolling, plain Markdown rendering (inline accents deferred by revision A), opaque solid paper and glyphs (native blur deferred by revision B), global activation, and full-screen focus restoration; unobserved rows start failed.
  Required assertion after the native child process returns its observed text:
  ```rust
  assert_eq!(roundtripped.as_bytes(), original.as_bytes());
  ```
- [ ] **2. Verify the gate is red.** Run `cargo test --test native_gate -- --ignored --nocapture` and `cargo run --example native_gate`; inspect the native window and the acceptance matrix. Record actual failures; a harness that merely opens is insufficient.
- [ ] **3. Probe using public APIs.** Use the documented multiline plain-text input selected by user-approved revision A. Retain the styled-text probes as historical evidence; do not build a styled input overlay. Verify source/caret geometry and macOS input behavior. Compare `cargo run --example native_window_probe` with `cargo run --features probe-skia --example native_window_probe`. Implement the narrow platform probe, matching 505×552 and 400×440 dimensions. Keep disposable experiments in the examples; do not make an unproven editor a core dependency.
- [ ] **4. Decide from native evidence.** Record OS/toolchain/backend, screenshots, supported APIs, and observed outcomes. Verify actual IME and VoiceOver behavior on macOS rather than treating synthetic keystrokes as equivalent. Inline accents and native blur are explicitly deferred by user-approved revisions A and B. If plain-text editing correctness, solid frameless rendering, or required activation behavior cannot satisfy the revised spec through supported APIs, stop here with the concrete failure and request a design revision. No later task proceeds on an unapproved fallback.
- [ ] **5. Verify and commit the successful harness.** Run `cargo test --test native_gate -- --ignored`, `cargo check --all-targets --locked`, and `cargo fmt --check`; retain the renderer/editor choice and remove unused probe dependencies. Commit with `feat: establish validated native Slint foundation` only after the gate has evidence.

Execution scheduling clarification: keep Task 1 open while human IME/accessibility/lifecycle observations are pending. Independent domain, theme, and storage work (Tasks 2–4) may proceed after the approved scope revisions A/B; this does not waive native integration or release acceptance. After the user confirmed Preview 2 resizing and hover/drag, Task 5 may also proceed provisionally, keeping the remaining native editor/accessibility rows open.

### Task 2: page identities, revision types, and deterministic navigation

**Files:** Create `src/domain/{mod,page,connection,status,error}.rs`, `src/app/{mod,model,navigation}.rs`, and `tests/{domain,app_navigation}.rs`; modify library exports and the manifest.

**Interfaces:** Produce `PagePath::parse(&str) -> Result<PagePath, PathError>` and `ConnectionConfig::parse(&str) -> Result<ConnectionConfig, ConnectionError>`. A `ConnectionId` is the canonical credential-free HTTPS base URL, including a normalized path prefix; a `PageKey` contains `connection: ConnectionId` and `path: PagePath`. Produce newtypes `LocalRevision(u64)`, `RemoteRevision(String)`, `OperationId(Uuid)`, `TrashId(Uuid)`. Define `View::{Note, AllPages, Appearance, Connection, Recovery}`; `EditorPosition { anchor: usize, caret: usize, scroll_y: f32 }` uses UTF-8 byte offsets validated at boundaries. `NavigationState::open(PageKey, EditorPosition)`, `back() -> Option<PageKey>`, and `forward() -> Option<PageKey>` own visit history. `new_note_path(now: &jiff::Zoned, id: Uuid) -> PagePath` uses the spec's path convention.

- [ ] **1. Write failing tests.** `paths_preserve_unicode_and_literal_percent` asserts exact preservation of `Projects/50% café.md`; `paths_reject_traversal` rejects absolute paths, dot segments, backslashes, and NUL. `connection_identity_keeps_prefix` distinguishes two spaces under the same origin. `history_restores_selection` asserts the stored UTF-8 anchor/caret and scroll survive search→note and back/forward. `new_note_path_is_stable` fixes time/UUID and asserts the exact dated path.
  ```rust
  assert!(PagePath::parse("../outside.md").is_err());
  assert!(PagePath::parse("Projects/50% café.md").is_ok());
  ```
- [ ] **2. Run red.** Run `cargo test --test domain --test app_navigation`; confirm the new contract tests fail for the missing behavior.
- [ ] **3. Implement the named contracts.** Use UUID v4 once per note/operation, Jiff for local calendar formatting, and the URL library for connection normalization. Reject URL credentials/query/fragment; do not percent-decode page filenames. Define typed `PathError`, `ConnectionError`, `StoreError`, `SyncError`, `PlatformError`, and `AppError` in `domain/error.rs` as concrete variants are introduced. Do not expose secret-bearing `Debug` output.
- [ ] **4. Verify green.** Run the same two test targets and `cargo clippy --all-targets --locked -- -D warnings`; verify a renamed first line never changes `PageKey`.
- [ ] **5. Commit.** Stage the listed files and manifest/lockfile; commit `feat: define page identity and revision-aware app state`.

### Task 3: versioned themes, contrast, and independent appearance preferences

**Files:** Create `src/theme/{mod,format,palette,contrast}.rs`, four built-in palette JSON files, `tests/{theme_contract,theme_contrast}.rs`, and theme fixtures; modify library exports and dependencies.

**Interfaces:** Produce `decode_theme(bytes: &[u8]) -> Result<Theme, ThemeError>`, `encode_theme(theme: &Theme) -> Result<Vec<u8>, ThemeError>`, `check_contrast(theme: &Theme) -> ContrastReport`, and `resolve_theme(settings: &AppearanceSettings, system: SystemAppearance, themes: &[ThemeEntry]) -> Result<ResolvedTheme, ThemeError>`. Define `ThemeEntry { id: ThemeId, theme: Theme }` for local identity outside the format. Define `Theme` with the spec's ten fields; `ThemeId(String)` uses stable built-in names or an imported UUID. `AppearanceSettings { mode: AppearanceMode, light_theme: ThemeId, dark_theme: ThemeId, paper: PaperStyle, font_size: f32 }`. `AppearanceMode` is Light/Dark/System; `SystemAppearance` is Light/Dark; `PaperStyle` is Plain/Dots/Ruled/Grid. `ContrastReport` lists failed text/control color pairs; `ResolvedTheme` contains the validated palette and paper colors, not notes or credentials.

- [ ] **1. Write failing contract tests.** Assert 32,768-byte inputs are permitted only if valid JSON, 32,769 bytes are rejected, names count Unicode scalars after trimming, duplicate keys are rejected including unknown keys, unknown fields are ignored, and version 2 is rejected. `export_roundtrips_version_one` asserts all recognized values survive. `system_mode_preserves_both_assignments` switches system mode twice and asserts neither theme ID changes. `builtins_meet_contrast` asserts text ratios ≥4.5 and essential control/focus ratios ≥3.0 without rounding.
  In the round-trip test, using a validated theme fixture:
  ```rust
  assert_eq!(decode_theme(&encode_theme(&theme)?)?, theme);
  ```
- [ ] **2. Run red.** Run `cargo test --test theme_contract --test theme_contrast`; require failures to identify missing validation/behavior, not malformed fixtures.
- [ ] **3. Implement the named contracts.** Use Serde/serde_json, with a recursive object visitor tracking keys to reject duplicates before theme projection, including in ignored additional fields; do not write a JSON parser. Derive paper pattern colors rather than adding unspecified required schema fields. Adjust built-in metadata/control colors minimally to meet contrast while retaining the approved palettes. Allocate distinct IDs to imported palettes, preserving built-ins with the same display name.
- [ ] **4. Verify green.** Run both theme test targets and `cargo fmt --check`. Add a low-contrast imported theme assertion: parsing succeeds but application requires an explicit override, with the previous selection retained until then.
- [ ] **5. Commit.** Commit `feat: add validated Bulletnotes themes and appearance settings`.

### Task 4: transactional local persistence, search, and durable operation intents

**Files:** Create `src/store/{mod,worker,pages,settings,outbox,recovery,search}.rs`, `migrations/001_initial.sql`, `tests/{store_recovery,store_search}.rs`, `tests/support/{mod,storage_fixture,clock}.rs`; modify `src/domain/{page,error}.rs` and exports/dependencies.

**Interfaces:** Produce async `StoreHandle::open(path: &Path) -> Result<StoreHandle, StoreError>` and async methods `load_page(&PageKey) -> Result<Option<PageSnapshot>, StoreError>`, `save_local(PageKey, String, LocalRevision) -> Result<SavedRevision, StoreError>`, `create_draft(PageKey) -> Result<SavedRevision, StoreError>`, `load_settings() -> Result<AppSettings, StoreError>`, `save_settings(AppSettings) -> Result<(), StoreError>`, `search(SearchQuery) -> Result<SearchResults, StoreError>`, `claim_pending(&ConnectionId, usize) -> Result<Vec<PendingMutation>, StoreError>`, and `acknowledge(OperationId, MutationAck) -> Result<Option<PageSnapshot>, StoreError>`. A completed deletion returns no active snapshot. All async operations send commands to one blocking SQLite owner. Define `PageSnapshot { key, body: String, local_revision, remote_base: Option<RemoteFile>, position: EditorPosition, permission: Permission }`, `SavedRevision { key, local_revision }`, `PendingMutation { id, key, local_revision, kind: MutationKind, condition: WriteCondition }`, `MutationKind::{Put(String), Delete}`, `WriteCondition::{CreateOnly, Match(RemoteRevision)}`, and `MutationAck { remote_revision: Option<RemoteRevision> }` in `domain/page.rs`. `AppSettings` holds appearance, imported palettes with their IDs, active connection, window position/size, and hotkey; it never holds a token. `Permission` is ReadOnly/ReadWrite. `RemoteFile` holds key, exact UTF-8 body, `revision: Option<RemoteRevision>`, optional modified time, and permission; a missing revision makes it viewable but ineligible for mutation. `SearchQuery` holds connection, query text, optional folder, offset, and limit. Define `PageSummary { key: PageKey, title: String, excerpt: Option<String>, modified_ms: Option<u64>, cached: bool, permission: Permission, last_visited_ms: Option<u64> }` and `SearchResults { items: Vec<PageSummary>, total: usize, indexed: usize, catalogue_total: usize, complete: bool }`. Privately produce `acquire_instance_lock(path: &Path) -> Result<InstanceGuard, StoreError>` in the storage worker; the guard owns a `std::fs::File` lock for the store lifetime.

- [ ] **1. Write failing persistence tests.** `acknowledged_text_survives_process_exit` commits a revision in a child process, exits abruptly, and asserts reopening retains that body and one pending intent. `failed_transaction_preserves_previous_revision` injects a storage failure and asserts no save acknowledgement or partial outbox update. `second_writer_is_refused` opens the same database from another process and expects a clear AlreadyRunning error. `failed_migration_preserves_original` injects a migration error and compares the original committed notes. `corrupt_database_is_never_reset` asserts opening fails without replacing the file with an empty store. `search_is_literal_and_scoped` covers punctuation/quotes, duplicate titles, Unicode, and two connections.
  After reopening the child process's database:
  ```rust
  assert_eq!(reopened.load_page(&key).await?.unwrap().body, committed_body);
  ```
- [ ] **2. Run red.** Run `cargo test --test store_recovery --test store_search`; use temporary directories and deterministic faults rather than filling the user's disk.
- [ ] **3. Implement storage and ownership.** Create connection-scoped pages, immutable sent-operation records, pending intents, recovery/trash, settings, and FTS5 search tables. Enable foreign keys, a finite busy timeout, WAL, `synchronous=FULL`, and `fullfsync=ON` on macOS; verify actual pragma values. Serialize migrations, back up through SQLite's backup facility before destructive migrations, and never use a raw copy of a live WAL database. Obtain the instance lock with stable `File::try_lock` before opening writable app data; retain the file guard and reject a second writer. A duplicate launch later activates the owner through Task 11. Coalesce superseded unsent operations while preserving a leased operation's exact payload. On restart, an abandoned leased mutation is an unknown outcome requiring reconciliation, not permission to resend unconditionally. Empty new drafts have no remote intent until non-empty; clearing an existing remote page still queues an empty-body write.
- [ ] **4. Implement the search contract.** Maintain the search index transactionally. Quote/escape user queries as literals instead of exposing FTS operators; parameterize all SQL. Order empty searches by local visit order followed by path. Keep read-only/cached/index-completeness metadata in results. Page IDs and SQL constraints always include connection identity.
- [ ] **5. Verify green.** Run the two test targets, a commit/rollback/crash cycle, and `cargo test --lib --locked`; inspect that a delayed acknowledgement advances only the sent operation's base and leaves a newer local revision pending.
- [ ] **6. Commit.** Commit `feat: persist local notes and sync intents transactionally`.

### Task 5: native focus-first views and theme application

**Files:** Create the Slint view/component files except connection/recovery, `src/app/appearance.rs`, and `tests/ui_contract.rs`; modify `ui/app-window.slint`, palette files as needed, and dependencies.

**Interfaces:** `AppWindow` produces callbacks `browse-pages()`, `open-page(string)`, `new-note()`, `show-appearance()`, `return-to-note()`, `note-edited(string)`, `history-step(int)`, `appearance-changed()`, `import-theme()`, and `export-theme()`. Expose properties `active-view: string`, `current-path: string`, `note-text: string`, `status-text: string`, `is-pinned: bool`, `font-size-px: float`, `page-results: [PageRow]`, and `theme: UiTheme`. `PageRow` contains a stable page key string, title, path, excerpt, and availability label. `UiTheme` maps the seven semantic roles; `apply_theme(window: &AppWindow, theme: &ResolvedTheme)` changes presentation only. Produce `AppearanceController::preview(&mut self, id: ThemeId) -> Result<ContrastReport, ThemeError>` and `confirm_import(&mut self, theme: Theme, allow_low_contrast: bool) -> Result<AppSettings, ThemeError>`; confirmation returns candidate settings for asynchronous persistence, not a synchronous disk write. The coordinator adopts the settings only after the store acknowledges them. Settings writes are serialized; an immutable AppearanceCommit ticket tags each completion to its originating preview so an older completion cannot revert a newer preview.

- [ ] **1. Write failing UI contract cases.** Assert exactly one primary view is visible, all icon actions expose names, and changing palette does not change `note-text`. `invalid_import_keeps_previous_palette` asserts the old theme survives parser failure, a declined low-contrast warning, and a failed settings commit. `same_named_import_preserves_builtin` asserts distinct IDs. Record screenshot expectations from the approved reference at default and minimum sizes.
  In the presentation-model theme-switch case:
  ```rust
  assert_eq!(note_text_after, note_text_before);
  ```
- [ ] **2. Run red.** Run `cargo test --test ui_contract --locked`; open the native app and capture the baseline mismatches before styling.
- [ ] **3. Implement the views.** Match the approved proportions, system window controls, compact footer, full-space search rows, and appearance picker. Use custom themed components over documented Slint input/layout primitives, virtualized result rows, and non-interactive paper decorations. Connect rfd dialogs for import/export; importing validates completely before preview and assigning. Persist a confirmed change through `StoreHandle::save_settings`; restore the previous palette if persistence fails. Native macOS material affects only the background.
- [ ] **4. Verify green and visually.** Run `cargo test --test ui_contract --test theme_contrast --locked`; save native screenshots for all four palettes at both sizes in `docs/validation/visual-matrix.md`. Verify text-size changes align ruled paper to actual line height and reduced motion/transparency do not lose text or selection.
- [ ] **5. Commit.** Commit `feat: implement focus-first native views and theme controls`.

### Task 6: editor, page browser, and local-app coordination

**Files:** Create `src/app/{coordinator,editor}.rs`, `tests/editor_roundtrip.rs`; modify app models/navigation, `src/main.rs`, view bindings, and `tests/app_navigation.rs`.

**Interfaces:** Produce `Coordinator::dispatch(&mut self, action: AppAction) -> Vec<AppEffect>` and `Coordinator::receive(&mut self, event: AppEvent) -> Vec<AppEffect>`. Define `SearchRequest { id: u64, query: SearchQuery }` with monotonically increasing IDs. `AppAction` variants are EditText(String), Navigate(View), OpenPage(PageKey), NewNote, StepHistory(i8), Search(SearchRequest), SavePosition(EditorPosition), Flush, and RequestQuit. `AppEvent` variants are LocalSaved(SavedRevision), LocalSaveFailed(PageKey, LocalRevision, StoreError), SearchReady(u64, SearchResults), PageLoaded(PageSnapshot), and SettingsSaved(AppSettings); later tasks add sync/platform events. `AppEffect` requests store/worker work or UI updates; it never performs blocking I/O inside dispatch. Produce `EditorSession::snapshot() -> (String, EditorPosition)` and `restore(&mut self, &PageSnapshot)` using the editing technique validated in Task 1.

- [ ] **1. Write failing behavioral tests.** Under a paused clock, assert no local acknowledgement before commit, a save effect by 250 ms, immediate flush on navigation/quit, and preservation of an unsaved buffer after StoreError. `late_load_cannot_replace_active_note` opens A then B and delivers A's result last; assert B stays visible. `edit_roundtrip_preserves_markdown` inserts after CRLF frontmatter and an indented code block containing emoji/CJK, and compares the stored source. Native cases pin IME, undo, selected paste, word motion, and wrapped-line scrolling without mutating on open.
  After delivering A's stale load event:
  ```rust
  assert_eq!(active_page_after_late_a, page_b);
  ```
- [ ] **2. Run red.** Run `cargo test --test app_navigation --test editor_roundtrip --locked` and the native editor matrix from Task 1.
- [ ] **3. Implement coordination and editing.** Introduce bounded UI→worker queues, 250 ms local coalescing, immediate transition flushes, and revision-tagged results. On disk failure keep editing/copy available and prevent implicit discard. Implement Command+K/N/F/comma, bracket history, Escape, and selection/scroll restoration. The editor follows revision A: plain Markdown with one palette ink color; inline heading/link accents are deferred. Clipboard paste remains plain text with indentation. Search tracks query identity so a slow older query cannot replace newer results; apply folder filters and literal content matching locally.
- [ ] **4. Verify the first milestone.** Run `cargo test --test app_navigation --test editor_roundtrip --test store_search --locked`, then native note→search→note and Appearance flows while offline. Restart and verify notes, visit positions, theme assignments, window state, and pending operations. Confirm that no HTTP call is made in this milestone.
- [ ] **5. Commit.** Commit `feat: connect native editing and search to durable local notes`.

## Milestone 2: automatic SilverBullet synchronization

### Task 7: typed SilverBullet protocol and conformance fixture

**Files:** Create `src/sync/{mod,client,protocol}.rs`, `tests/protocol_contract.rs`, `tests/support/http_fixture.rs`; modify exports/dependencies.

**Interfaces:** Produce `SilverBulletClient::new(config: &ConnectionConfig, token: Option<SecretToken>) -> Result<SilverBulletClient, SyncError>` and async `list_files() -> Result<Vec<RemoteMetadata>, SyncError>`, `read(&PageKey) -> Result<RemoteRead, SyncError>`, `put(&PendingMutation) -> Result<MutationAck, SyncError>`, and `delete(&PendingMutation) -> Result<MutationAck, SyncError>`. Define `RemoteRead::{Present(RemoteFile), Missing(PageKey)}` in `domain/page.rs` so missing observations retain identity without making storage depend on transport. `RemoteMetadata` contains key, modified time, content length, permission, and revision when supplied. `SecretToken` has redacted Debug and exposes bytes only to request construction. `SyncError` distinguishes Authentication, Permission, Conflict, Missing, Incompatible, InvalidResponse, Network, and UnknownMutationOutcome.

- [ ] **1. Write failing HTTP cases.** The fixture asserts `X-Sync-Mode: true`, exact `If-Match` or `If-None-Match: *`, segment-encoded `Projects/50% café #1.md`, and absence of a token at a redirected origin. Assert HTML status-200 login content and invalid UTF-8 never become writable pages; traversal catalogue entries and missing revision/permission are rejected or read-only as appropriate. A 412 must be Conflict, 401 Authentication, and mutation timeout UnknownMutationOutcome. Preserve byte-for-byte CRLF through read/put.
  For the fixture's stale-ETag response:
  ```rust
  assert!(matches!(stale_write_result, Err(SyncError::Conflict)));
  ```
- [ ] **2. Run red.** Run `cargo test --test protocol_contract --locked` against the isolated axum fixture; no external requests.
- [ ] **3. Implement the adapter.** Use reqwest with certificate validation, a 10-second connect timeout and 30-second operation timeout, bounded concurrent downloads, and redirects disabled. Use `Url::path_segments_mut` for logical filenames without decoding literal percent signs. Preserve raw ETag values. Decode text strictly and verify status, media type, and response metadata. An absent read permission defaults to ReadOnly; an absent ETag preserves readable content but prevents mutation. Treat mutation timeouts, connection resets, and server 5xx responses as unknown outcomes unless non-delivery can be proved; a 5xx can follow a successful write. Retry policy belongs to the sync worker, not this adapter. Restrict plain HTTP to explicitly constructed loopback test fixtures; user connections remain HTTPS.
- [ ] **4. Verify green.** Run the protocol target and inspect fixture request logs to confirm every mutation is guarded and no note/token content enters production diagnostics. Test full base-URL prefixes and default/explicit ports.
- [ ] **5. Commit.** Commit `feat: add guarded SilverBullet filesystem protocol adapter`.

### Task 8: connection, Keychain, and deployed write-safety verification

**Files:** Create `src/sync/probe.rs`, `src/platform/credentials.rs`, `src/app/connection.rs`, `ui/connection-view.slint`, and `tests/{connection_security,compatibility_probe}.rs`; modify coordinator/settings and dependencies.

**Interfaces:** Produce async `CredentialStore::load(&self, &ConnectionId) -> Result<Option<SecretToken>, CredentialError>`, `save(&self, &ConnectionId, &SecretToken) -> Result<(), CredentialError>`, and `delete(&self, &ConnectionId) -> Result<(), CredentialError>`, wrapping blocking Keychain calls outside the UI thread. Produce async `verify_write_guards(client: &SilverBulletClient, probe_key: PageKey) -> Result<CompatibilityEvidence, SyncError>`, where `CompatibilityEvidence` contains connection identity, verification time, and tested outcomes. Produce async `ConnectionController::connect(config: ConnectionConfig, token: Option<SecretToken>, remember: bool) -> Result<ConnectionOutcome, AppError>`; `ConnectionOutcome` holds the client, readable state, `write_capability: Result<CompatibilityEvidence, SyncError>`, and an optional credential-storage warning. Add `CredentialError` to `domain/error.rs`. Store evidence is scoped to connection/configuration; a reconnect cannot reuse another space's queue or token.

- [ ] **1. Write failing connection cases.** `ignored_preconditions_disable_mutations` makes the fixture accept a stale guarded write and asserts no normal-note mutation becomes available. `probe_cleanup_is_path_scoped` asserts only its exact generated key is used. `keychain_denial_keeps_session_usable` tests a failing credential backend without placing the token in SQLite or logs. `switch_connection_keeps_queues_separate` asserts A's pending operations cannot run against B. `redirect_does_not_disclose_token` pins the cross-origin fixture request count to zero.
  For the fixture that ignores preconditions:
  ```rust
  assert!(outcome.write_capability.is_err());
  ```
- [ ] **2. Run red.** Run `cargo test --test connection_security --test compatibility_probe --locked`; use an in-memory credential fake, with real Keychain tests explicitly ignored unless requested on a developer Mac.
- [ ] **3. Implement connection and the compatibility probe.** Use security-framework's generic-password APIs, scoped by app service and canonical connection account; missing credentials are a normal state. On Keychain denial offer an in-memory session rather than silently persisting elsewhere. Test connection explains that verification uses a temporary isolated page. Generate `Quick Notes/Bulletnotes Probe/<uuid>.md` once: create-only A, matching write B, reject stale write/delete and create collision, read back B, then guarded-delete B. Timeouts fail closed and retain the exact owned probe key for guarded cleanup; no wildcard cleanup or unconditional mutation. Read capability alone must not enable write sync.
- [ ] **4. Verify deployed behavior.** Run both test targets and the same probe against an isolated official SilverBullet release, recording its release identity and results. Before enabling the user's endpoint, run the probe on its generated key only; do not modify an existing note or upgrade the server. If guards fail, keep local capture and show compatibility status. Record external credential/server prerequisites rather than treating them as passing tests.
- [ ] **5. Commit.** Commit `feat: add secure connection setup and write-safety verification`.

### Task 9: complete catalogue, progressive offline cache, and revision-safe sync

**Files:** Create `src/sync/{catalogue,worker,reconcile}.rs`, `tests/sync_lifecycle.rs`; modify store outbox/pages/search and coordinator/status bindings.

**Interfaces:** Produce `SyncHandle::start(StoreHandle, SilverBulletClient, Option<CompatibilityEvidence>) -> Result<SyncHandle, SyncError>`, `set_active(bool)`, `refresh()`, and async `shutdown() -> Result<(), SyncError>`; without matching mutation evidence it operates in read-only mode. Produce pure `reconcile(local: &PageSnapshot, remote: &RemoteRead) -> ReconcileDecision`, with Unchanged, InstallRemote(RemoteFile), KeepPending, Conflict(ConflictRecord), and RetainDeletedSnapshot. Define the decision and `ConflictRecord { id: Uuid, key: PageKey, base: Option<RemoteFile>, local: PageSnapshot, remote: RemoteRead }` in `domain/page.rs`. Extend the store with async `apply_remote(RemoteRead) -> Result<ReconcileDecision, StoreError>` and `record_catalogue(&ConnectionId, Vec<RemoteMetadata>, bool) -> Result<(), StoreError>`; the boolean means a fully successful catalogue, not a partial response. `AppEvent` adds CatalogueProgress, RemoteApplied, SyncStateChanged, and ConflictDetected tagged by connection/key/revision.

- [ ] **1. Write failing lifecycle tests.** `write_waits_for_local_commit_and_750ms` controls the clock. `old_ack_keeps_newer_revision_pending` sends revision 1, saves revision 2, then acknowledges 1 and asserts 2 remains pending with the advanced remote base. `timeout_after_success_reads_before_retry` drops the first response after applying the body; assert no duplicate create or unconditional write. `catalogue_failure_is_not_deletion` preserves known pages after an interrupted response. `partial_index_is_visible` asserts incomplete content search until all bodies arrive. `offline_uncached_open_is_explicit` preserves a selected uncached path with an explanation.
  After acknowledging revision one while revision two is committed:
  ```rust
  assert_eq!(pending_after_ack.local_revision, revision_two);
  ```
- [ ] **2. Run red.** Run `cargo test --test sync_lifecycle --locked` against the stateful fixture with paused time and deterministic random jitter.
- [ ] **3. Implement the workers and reconciliation.** Download at most four bodies concurrently; mutations are serialized per page. Claim durable pending operations, keep their payload immutable, and acknowledge only the sent revision. After unknown mutation outcomes, compare remote body/revision with sent and current local states before retrying. Never manufacture an ETag or overwrite based on timestamps. Remote changes to clean pages install transactionally; dirty changes/deletions become conflicts. Update catalogue/search completeness only after a complete validated listing.
- [ ] **4. Implement scheduling and status projection.** Use 30/60-second foreground/background polls, immediate activation/reconnect refresh, 750 ms write debounce, and exponential retries capped at 60 seconds with jitter. Authentication/permission/compatibility/conflict pauses require their specific resolution. Derive Saving, Saved locally, Synced, and Offline · saved locally from both local and remote revision state; a newer uncommitted buffer always overrides a stale synced indication. Keep unrelated pages progressing.
- [ ] **5. Verify green.** Run `cargo test --test sync_lifecycle --test protocol_contract --test store_recovery --locked`, then native two-client/offline/restart scenarios on an isolated space. Verify all pages remain searchable while background download/sync progresses and that per-page queues stay bounded under rapid edits.
- [ ] **6. Commit.** Commit `feat: synchronize the full space without losing concurrent edits`.

### Task 10: conflict review, recoverable deletion, and restoration

**Files:** Create `src/app/recovery.rs`, `ui/recovery-view.slint`, and `tests/conflict_recovery.rs`; modify reconcile/store recovery/outbox/coordinator.

**Interfaces:** Consume Task 9's `ConflictRecord`. Define `ConflictResolution { conflict_id: Uuid, expected_local: LocalRevision, choice: ResolutionChoice }` and `ResolutionChoice::{AcceptRemote, KeepLocal, Merge(String)}` in `domain/page.rs`. Produce store async methods `save_conflict(ConflictRecord) -> Result<(), StoreError>`, `resolve_conflict(ConflictResolution) -> Result<Option<SavedRevision>, StoreError>`, `move_to_trash(PageKey, LocalRevision) -> Result<TrashId, StoreError>`, `restore_trash(TrashId, Option<PagePath>) -> Result<PageKey, StoreError>`, and `empty_trash(Vec<TrashId>) -> Result<(), StoreError>`. Accepting a remote deletion returns no active saved revision; an optional restore destination is an explicitly chosen new path within the original connection. Extend `AppAction` with these exact recovery intents; never expose raw SQL or unconditional HTTP writes to the UI.

- [ ] **1. Write failing recovery cases.** `recreated_path_survives_delete_retry` changes the server page after a timed-out delete and asserts the new content remains. `remote_delete_is_not_empty_text` distinguishes Missing from an existing empty document. `resolution_races_back_to_review` changes the server again while applying a merge and expects another conflict. `trash_restore_is_create_only` preserves a replacement page at the old path. `trash_survives_restart` retains source, base, and pending operation. `empty_existing_remote_page_is_retained` proves empty-note navigation cannot remove an existing page.
  In the timed-out-delete/recreated-path case:
  ```rust
  assert_eq!(remote_body_after_retry, replacement_body);
  ```
- [ ] **2. Run red.** Run `cargo test --test conflict_recovery --locked` with the same fixture/storage helpers.
- [ ] **3. Implement recovery transactions.** Commit recovery copies before queuing deletion; a stale delete becomes conflict rather than trash success. Accept remote, keep local, and merge preserve the common base and compare against the newest remote revision. Recreating a deleted remote page and restoring trash use create-only writes. Path collisions require an explicit user choice of a new recovery path; do not rename or rewrite links automatically. Explicit trash purge names exact IDs and never runs on a timer.
- [ ] **4. Implement and verify the native review flow.** Show local/remote text with clear labels, a remote-deleted state, and review actions; preserve the user's buffer when leaving review. Run the conflict target and native conflict→merge→retry and delete→restart→restore flows. Ensure unrelated pages continue syncing.
- [ ] **5. Commit.** Commit `feat: preserve conflicts and recover deleted notes safely`.

## Milestone 3: macOS production release

### Task 11: native lifecycle, activation, settings persistence, and graceful shutdown

**Files:** Extend `src/platform/macos.rs`, app coordinator/main, `ui/app-window.slint`; create `tests/macos_lifecycle.rs` and lifecycle sections in `docs/validation/release-matrix.md`.

**Interfaces:** Produce `MacIntegration::attach(window: &slint::Window, settings: &AppSettings) -> Result<MacIntegration, PlatformError>`, `register_hotkey(&mut self, accelerator: &str) -> Result<(), PlatformError>`, `set_pinned(&self, bool) -> Result<(), PlatformError>`, `set_material(&self, bool) -> Result<(), PlatformError>`, `activate(&self) -> Result<(), PlatformError>`, and `hide_and_restore_previous_app(&self) -> Result<(), PlatformError>`. Produce `LifecycleAction::{Activate, Hide, CloseRequested, QuitRequested, SystemAppearanceChanged, AccessibilityPreferencesChanged}`. Reuse Task 4's instance lock; the owner receives a second-launch activation through documented app activation rather than allowing a second writer.

- [ ] **1. Write failing lifecycle cases.** `close_flushes_locally_without_waiting_for_network` blocks the fixture response but expects hide after local commit. `quit_with_failed_save_preserves_buffer` expects retry/copy/discard choice, not process exit. `hotkey_conflict_is_actionable` preserves the previous registration on a failed change. `second_launch_activates_owner` expects one writer. Native observations cover Spaces/full-screen apps, previous-app focus, pinning, menu-bar reopen, and quit.
  After QuitRequested receives a local save error:
  ```rust
  assert!(!quit_was_emitted);
  ```
- [ ] **2. Run red.** Run `cargo test --test macos_lifecycle --locked`; exercise the native activation cases from an ordinary signed development bundle, not just a terminal process.
- [ ] **3. Implement supported integration.** Use Slint's SystemTrayIcon and always-on-top facilities where they meet the gate; isolate necessary AppKit/NSWorkspace/NSVisualEffectView code behind the validated adapter. Keep hotkey registration and UI calls on the main thread, document ownership/threading for every unsafe FFI boundary, and unregister observers/hotkeys on teardown. Persist window position safely when monitors change. Honor OS availability and reduced transparency/motion immediately.
- [ ] **4. Verify green.** Run lifecycle tests plus store second-writer tests, then record actual full-screen/Spaces and VoiceOver observations. Crash after a local save and relaunch through the bundle; assert the page and caret are recovered. No network request may be required to hide or quit.
- [ ] **5. Commit.** Commit `feat: integrate macOS activation and safe app lifecycle`.

### Task 12: diagnostics, accessibility, and measured responsiveness

**Files:** Create `src/diagnostics.rs`, `tests/diagnostics.rs`, `benches/interaction.rs`, `scripts/check.sh`, and `docs/usage.md`; extend visual/native evidence files and manifest bench configuration.

**Interfaces:** Produce `diagnostics::init(log_dir: &Path) -> Result<DiagnosticGuard, DiagnosticError>`, `diagnostics::export(destination: &Path) -> Result<(), DiagnosticError>`, and a `DiagnosticEvent` enum limited to opaque operation IDs, classified outcomes, timings, and queue counts. Register a `harness = false` bench named `interaction`; its result records hardware/OS/build/renderer, fixture size, and p95 timings for warm activation, cached open, and cached search. No production event accepts note text, a path/title, token, or clipboard payload.

- [ ] **1. Write failing checks.** Inject sentinel note/token/path strings and assert none appears in generated diagnostics or exports. Assert seven-day log retention and export failure preserve existing data. Native matrix cases cover keyboard-only navigation, accessible icon names, VoiceOver text selection, high contrast, Reduce Motion, Reduce Transparency, and long search result lists. The benchmark initializes 10,000 generated pages and a 1 MiB note; performance thresholds are 150/100/100 ms p95 as specified.
  After exporting diagnostics from sentinel-bearing failures:
  ```rust
  assert!(!exported.contains("TOKEN_SENTINEL"));
  assert!(!exported.contains("NOTE_BODY_SENTINEL"));
  ```
- [ ] **2. Run red/baseline.** Run `cargo test --test diagnostics --locked`, `cargo bench --bench interaction --locked`, and the accessibility matrix. Record observed baselines and failures, never substitute mockup/browser numbers.
- [ ] **3. Implement diagnostics and address measured issues.** Use non-blocking local tracing with daily rolling files and seven-day retention; omit sensitive fields at event construction and redact errors before logging. Export only on user action. Fix demonstrated UI blocking, search/copy costs, focus errors, or paper metrics in their owning modules; do not preemptively introduce caches/frameworks. Add a regression case to the owner for each fixed behavior.
- [ ] **4. Run the reproducible checks.** `scripts/check.sh` runs `cargo fmt --check`, `cargo clippy --all-targets --all-features --locked -- -D warnings`, and `cargo test --lib --bins --tests --examples --all-features --locked`, excluding explicitly ignored native/manual cases. Benchmarks are compiled by Clippy but executed only through the named benchmark command, not routine tests. Run a current RustSec audit tool against Cargo.lock and assess reachability of each advisory before release; verify the tool version from its official source when installing it. Native/Keychain/manual checks have separately named commands and recorded evidence.
- [ ] **5. Verify and commit.** Rerun diagnostics tests, affected regressions, benchmark, and accessibility matrix. Commit `test: verify native usability and production diagnostics` only with actual results; unmet performance/accessibility targets remain release blockers.

### Task 13: signed macOS artifacts, clean installation, and upgrade recovery

**Files:** Create `scripts/{package-macos,verify-release}.sh`, `packaging/{Info.plist,entitlements.plist}`; complete `docs/{usage,validation/release-matrix}.md`; modify manifest release metadata and `.gitignore` for generated bundles/evidence outputs.

**Interfaces:** `scripts/package-macos.sh --version <semver> --output <directory>` produces Apple Silicon ARM64 `.app` archives only from locked release builds. `scripts/verify-release.sh <app-path>` verifies architecture, deployment target, signature, notarization staple, required resources, and launchability. Signing identity/team/notary credentials come from the user's release environment/Keychain, never repository files or shell-expanded secret values. Use Apple CLI tooling rather than an unverified packaging service. Bundle identifier starts as `lol.harflix.bulletnotes`; record any required identity change before distributing a signed build.

- [ ] **1. Define failing artifact checks.** The verifier rejects unsigned, incorrectly targeted, missing-resource, and unstapled distribution artifacts. The upgrade matrix starts with committed notes plus an unsynced edit, imports a custom theme, updates the app, and asserts source, queued operations, and settings survive. An injected migration failure must recover the original store from its pre-migration backup rather than creating an empty database.
  Capture the verifier process result for an unsigned fixture:
  ```rust
  assert!(!unsigned_verification.status.success());
  ```
- [ ] **2. Run red.** Run `scripts/verify-release.sh <development-app>` and confirm it rejects incomplete distribution requirements. Keep this expected failure distinct from a failed native application test.
- [ ] **3. Build and package.** Use `cargo build --release --locked --target aarch64-apple-darwin` with deployment target 14.0. Bundle only the required native binary/resources, sign with hardened runtime and reviewed minimum entitlements, notarize using `xcrun notarytool`, and staple with Apple's tooling. Do not disable certificate or signing checks for convenience. Credentials/account access are release prerequisites; without them report a verified development build and the exact remaining release work, not production completion.
- [ ] **4. Verify clean installations and compatibility.** Run `scripts/verify-release.sh` on each artifact; install on clean supported Apple Silicon macOS systems and record launch, Keychain prompts, offline editing, system appearance, and upgrade recovery. Verify the declared Rust 1.92 minimum with the locked graph or explicitly raise/document the minimum after review. Exercise sync against the isolated official SilverBullet release and the user's successfully probed endpoint without altering unrelated pages. Review dependency attribution/license requirements and include Slint attribution in About.
- [ ] **5. Final verification and commit.** Run `scripts/check.sh`, protocol/recovery suites, native release matrix, and the performance harness as needed for the final build. Commit `build: package and verify signed macOS releases`, retaining commands and measured evidence. Publish/upload/distribute only when requested; building and verifying artifacts does not itself publish them.

## Plan self-review and handoff

Coverage maps the spec's window/editor/search/theme sections to Tasks 1–6, store/sync/security/status sections to Tasks 4 and 7–10, and native/release requirements to Tasks 11–13. Every Review Focus item has named tests in its owning tasks. Runtime/library names in this plan are proposed interfaces to create, not claims that those files or functions already exist.

Task 1 is a genuine feasibility boundary. The user approved deferral of inline heading/link colors in revision A. A webview, AppKit editor, custom renderer, or reduced plain-text editing correctness remains unapproved. Native materials are deferred by user-approved revision B. Record and review any required architectural change before proceeding. Missing release credentials or access to an OS/hardware test matrix is an external prerequisite, not an excuse to mark that evidence passed.

After plan review, use the user's chosen execution method. Native execution is recommended because this is a single app whose tasks share revision/persistence contracts and the first native gate may revise later details. Subagent-driven execution is also available, with a fresh implementer/reviewer per task and a higher context cost. Whichever method is selected, make task commits and perform a final independent review before claiming the production release is complete.

## Official sources used for planning

- [Slint 1.18.1 release](https://github.com/slint-ui/slint/releases/tag/v1.18.1), [Slint Cargo features](https://github.com/slint-ui/slint/blob/v1.18.1/api/rs/slint/Cargo.toml), and [Rust ownership/threading API](https://docs.slint.dev/latest/docs/rust/slint/).
- [crates.io release metadata](https://crates.io/api/v1/crates/slint); other pins use the same official API with each crate name.
- [reqwest features/API](https://docs.rs/reqwest/0.13.5/reqwest/), [Tokio](https://docs.rs/tokio/1.53.3/tokio/), [rusqlite](https://docs.rs/rusqlite/0.40.2/rusqlite/), and [Jiff](https://docs.rs/jiff/0.2.38/jiff/).
- [SilverBullet HTTP API](https://docs.silverbullet.md/HTTP%20API) and [official repository](https://github.com/silverbulletmd/silverbullet).
- [SQLite durability pragmas](https://www.sqlite.org/pragma.html), [backup API](https://www.sqlite.org/backup.html), and [FTS5](https://www.sqlite.org/fts5.html).
- [security-framework Keychain APIs](https://docs.rs/security-framework/3.7.0/security_framework/passwords/), [objc2](https://docs.rs/objc2/0.6.5/objc2/), and [Apple notarization](https://developer.apple.com/documentation/security/notarizing-macos-software-before-distribution).
- [Rust 1.99.1 announcement](https://blog.rust-lang.org/2026/10/15/Rust-1.99.1/) and [standard file locking](https://doc.rust-lang.org/std/fs/struct.File.html#method.try_lock).

### Deployment audit correction, 2026-10-17

The original temporary probe folder began with a dot. Current SilverBullet rejects any dot-prefixed filesystem path component with HTTP 404, including mutations. The deployed endpoint reproduced that refusal; this was a probe-path mistake, not evidence that normal writes were disabled. Task 8 now uses a visible, generated `Quick Notes/Bulletnotes Probe/<uuid>.md` path. Its ownership, create-only request, guarded cleanup, and prohibition on existing-note mutations are unchanged. See [the sync readiness audit](../../validation/sync-readiness.md) for observed deployment checks. This spot-check does not complete Tasks 7–10 or replace their automated fixtures, timeout/recovery tests, or integration.

Outstanding native input/caret observations remain release requirements. They do not prevent independent implementation/testing of the HTTP client, guarded sync state machine, and full-space catalogue/cache. Finish those approved modules without waiting on unrelated UI polish; production integration remains subject to the full acceptance matrix.
