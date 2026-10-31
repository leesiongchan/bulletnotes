# Universal Theme and Explicit Paper Types

> Use superpowers:executing-plans for inline execution. User requested removal of Theme default and universal, shared theme presentation.

**Goal:** Four explicit paper types and one selected palette across all app-owned views.

**Architecture:** Keep resolve_theme and palette as the sole palette path. Remove settings_palette, settings-theme and chrome-theme, and the duplicate settings preview palette. All consumers receive AppWindow.theme. About observes that palette and the selected paper type.

**Compatibility:** PaperType has Smooth (default), Bone, Eggshell and Linen. Serde accepts the legacy "theme" value as Smooth and serializes "smooth". Missing paper_type also becomes Smooth. No schema bump, data reset or credential changes.

**Verification:** Render all five views and About with Matcha/Graphite; check system/manual modes, high contrast, failed-write rollback, existing theme-card assignments, editor preservation, minimum-size keyboard access and legacy settings. Run scripts/check.sh. Inspect native on-screen captures, since GPU snapshots omit texture. Produce a distinct new test app without installing it or replacing earlier builds.

- [x] Observe failing universal-rendering and migration regressions.
- [x] Remove the fallback material and separate palette paths; reuse shared theme inputs.
- [x] Update existing contracts and documentation to the user's new requirements.
- [x] Run focused tests, final review, full checks, native visual verification and build verification.

No changes to native OS-owned dialogs or theme-gallery preview colors: those previews must show the palette they would select.

## Completion evidence

The legacy implicit finish and separate settings palette have been removed.
AppWindow.theme is the single resolved, accessibility-adjusted palette passed to
all app-owned view components. AppearancePanel now has one theme input for both
its controls and preview. About uses the same palette and selected finish.

Standard widget globals are per root window. The reusable ThemeContext component
synchronizes their color scheme for both AppWindow and AboutWindow, keeping the
required attribution widget readable. The connection credential preference now
reuses SettingToggle rather than a system-colored checkbox. Rendered regressions
cover both controls and the selected backgrounds across all five main views.

Migration and universal-view tests failed before the change and now pass. The
connection-control regression failed with zero selected-accent pixels before its
fix, and now passes with enabled/disabled click behavior. The About text regression
fails with the shared bridge disconnected and passes with it connected.

scripts/check.sh passed formatting, strict Clippy and the full test suite. Native
macOS window captures in target/universal-theme-review cover Matcha throughout
all app views and Graphite settings/connection/About. Physical 1× and Windows/Linux
native testing were unavailable. Temporary capture examples have been removed.

A new development app is at target/test-build-universal-theme/Bulletnotes.app.
Signature verification and its generated-content Metal installation check passed.
Previous test apps remain available; nothing was installed or published.
