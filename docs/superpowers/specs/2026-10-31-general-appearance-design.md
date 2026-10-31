# General and Appearance settings

Scope confirmed by the user: focus on General and Appearance, apply reference
features only where they fit, introduce paper types such as Bone and Eggshell,
and call the existing pattern choice Paper style.

## Approach

Adapt AntiNote's categorized, grouped forms using existing Rust/Slint components.
A cosmetic change alone would omit the requested paper types; full AntiNote
parity would introduce unrelated product systems. Work stays in the existing
settings view and the harness-managed worktree.

## General

Provide a General tab with Start with a new note (off by default), the existing
global shortcut editor, and access to the existing SilverBullet connection view.
Starting with a new note applies only on application launch; existing notes and
their recovery state remain available. Keep all current OS integration behavior.

## Appearance

Provide an Appearance tab with grouped Text, Paper, Window, and Themes controls.
Rename the existing Plain/Dots/Ruled/Grid choice to Paper style.

Paper type is independent of theme colors and paper style: Smooth is the default
and removes grain, Bone has a fine subtle finish, Eggshell
has a stronger grain, and Linen has a woven finish. Explicit finishes work with
all palettes. This texture interpretation is an implementation choice.

Paper intensity offers Subtle, Clear (current default), and Bold, affecting the
pattern rather than text contrast. Window options control persistent bottom-bar
visibility (off by default), note count/date visibility, and newest-note indicator
(all information defaults on, preserving current behavior).

Retain Light/Dark/System, theme slot assignments, import/export and contrast
confirmation. The selected theme applies to every app-owned view through one resolved palette.
Saved legacy Theme default paper values load as Smooth. This supersedes the
original separate settings-palette and implicit paper-finish behavior.

## Constraints and acceptance

Use current pinned dependencies; no new services or runtime dependencies. Persist
new preferences through the existing serialized settings writer, load older
settings with defaults, and restore durable values after save failure. Never
replace note text or editor selection when applying preferences. Disable grain
under reduced transparency or increased contrast.

Controls must remain keyboard reachable at 400 × 440 and larger window sizes.
Verify real rendered finishes, startup behavior, persistence/reload, failed-save
rollback, and existing theme/shortcut/navigation contracts. Compare rendered
before/after settings and run scripts/check.sh. Native platform behavior beyond
the available machine must be reported as unverified.
