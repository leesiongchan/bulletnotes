# AntiNote theme configuration

Compared the installed AntiNote 2.1.3 Appearance panel directly with Bulletnotes
on 21 October 2026. The Themes section uses its neutral settings surface, native
theme order, 250×100-point cards, four-point corners, 15-point monospace names,
22.5-point grid, eight 24-point swatches, and blue selected borders. Sun and moon
buttons use separate Light/Dark assignments. Wide windows retain the reference's
350-point column offset and 16-point row gap; narrow windows use one column.
Bulletnotes retains its own text, paper, shortcut, and import/export controls.

All 37 AntiNote palettes include the eight authored swatches. Older Bulletnotes
and imported files derive swatches from their existing palette. The optional
metadata remains compatible with schema version 1. Selecting a sun or moon saves
only that assignment and preserves appearance mode, the other assignment, paper,
font, and source content. Existing picker themes still apply immediately.

## Verification

- `scripts/check.sh`: formatting and strict all-target/all-feature Clippy passed;
  349 tests passed, zero failed, and 24 existing tests remained ignored.
- `theme_selection` covers every palette across four appearance cases, real
  keyboard card/button activation, independent assignment persistence, unknown
  IDs, failure rollback, stable focus models, and authored preview colors with
  Increase Contrast enabled.
- Native `ui_review --appearance-contract` verifies 164 selections and independent
  sun/moon assignments through production callbacks and temporary SQLite.
- Independent review prompted corrections to the responsive breakpoint and
  Increase Contrast control colors; both were rechecked.

Native generated-content captures were visually checked for alignment, readable
labels, swatches, grids, and selection states:

| Appearance | Capture |
| --- | --- |
| Dark settings, two columns | [Wide](theme-configuration/wide.png) |
| Narrow window | [Compact](theme-configuration/compact.png) |
| Light settings | [Light](theme-configuration/light.png) |
| Increase Contrast | [Contrast](theme-configuration/contrast.png) |

The contrast capture was taken after the native window settled; the fixture's
one-second capture timer can precede its first complete Metal frame. A separate
rendered regression also verifies that contrast mode retains authored swatches.
Reference AntiNote screenshots remain local and are not included in the repository.
