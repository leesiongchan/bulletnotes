# AntiNote editor and chrome study

Inspected the installed Antinote 2.1.3 on macOS on 2026-10-31. Opened
the app, inspected accessibility controls and menus, and captured the existing
note with its header hidden and revealed. No note text or preferences were
intentionally changed. This is research, not an approved implementation plan.

## Observed reference

- The hidden header leaves the paper uninterrupted. Revealing it shows native
  traffic lights, Settings and Search on the left, and New Note and Pin on the
  right. Accessibility reports 32 × 32 point buttons.
- Icons use fine, rounded warm-ink strokes, generous internal space and faint
  circular edges. Settings uses sliders, Search an orbital magnifier, New Note
  an outlined note with a diagonal pen, and Pin a stylized upright pin.
- The visible note uses a monospaced face, comfortable line spacing, warm ink,
  a restrained orange heading and a light neutral paper background. The exact
  font family was not established by this inspection.
- An existing fenced JavaScript block retains its literal delimiters, mutes the
  fence and language label, and has a pale background extending across the
  writing area. The block's sample text does not establish token highlighting.
- AntiNote's official [1.1.7 release notes](https://antinote.io/updates/antinote-1-1-7)
  document language highlighting for fenced blocks and a whole-note `code:` mode.
- The hidden footer leaves a small orange dot. A later revealed-footer capture showed one row: Trash, a centered date,
  older/newer/newest navigation, and an archive action, within a faint rounded
  capsule. Controls have faint circular outlines; the writing surface continues
  behind them. Bulletnotes uses this alignment while keeping its sync status separate.

## Verified Bulletnotes causes and constraints

- `LocalTray` in `ui/app-window.slint` explicitly sets the menu bar title to `BN`.
  The pinned Slint fork exposes a tray `color-scheme` property and observes the
  status item's effective macOS appearance. Use that system signal for the icon,
  independently of the selected note theme.
- `ThemeScrollbar` is visible whenever content exceeds the viewport. It has no
  idle timer or fade. The shared component serves the editor and settings;
  standalone instances also appear in page and recovery lists.
- The bottom-right dot is explicitly labeled “Newest scratchpad” and is gated by
  the latest-note preference. It currently contains no sync information.
- Sync presentation already separates local save state, remote state and matching
  remote base. `sync_state` returns pending or synced after checking errors,
  permissions and revision-specific facts, but does not currently emit `Syncing`.
  Active-transfer evidence must be traced before implementing a blinking state.
  Local-only, queued, offline, read-only, authentication and conflicts need honest
  labels; a local save alone must never turn the indicator green.
- The native editor already parses fenced and indented Markdown code and emits
  source-code decorations. Fences are muted; there is no language tokenization.
  Existing tests ensure Markdown headings and links inside code stay literal.
- Note text is currently Menlo at 15 px, with paper-derived line height and
  45 px side margins. Preserve source, Unicode selection, undo and caret behavior
  when refining typography or decorations.
- Circular `ActionButton` controls currently have an opaque paper fill and
  drop shadows. Icon assets mix 16 and 24 unit canvases and stroke weights.
- The approved identity is warm eggshell, persimmon, silver and native typography.
  Keep the approved app icon; redesign interface glyphs as code-native SVGs.
- Existing General/Appearance preferences include footer visibility and a
  newest-note indicator. Replacing that dot must also reconcile its setting and
  documentation rather than leave a misleading preference behind.

## Recommended scope for approval

Retain Rust/Slint and the native source editor. Apply all seven QOL requests:
an icon-only system-aware tray, shared idle-fading scrollbars, an accessible
SilverBullet status indicator, quieter paper/ink styling and typography, shaded
language-highlighted code blocks, lighter hover-revealed bars, and a consistent
fine-stroke SVG icon set. Preserve custom themes, paper choices, keyboard focus,
reduced-motion/high-contrast behavior, editor state and sync safety.

The smaller alternative defers language highlighting while implementing code
block styling and the other UI changes. A replacement editor is not recommended:
it introduces editing-contract risk without addressing the requested polish.

After scope approval, write the design and implementation plan. Verify status
mapping against actual worker facts, scrollbar idle/hover/drag behavior, code
fences with language/unknown-language/Unicode input, unchanged source and undo,
and rendered light/dark layouts at 400 × 440 and larger sizes. Compare native
screenshots and run `scripts/check.sh`. Report untested live SilverBullet and
non-macOS behavior separately.
