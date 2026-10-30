# Bulletnotes Brand Rollout Implementation Plan

> **For agentic workers:** Execute in the current session, using parallel work only for independent asset and site changes. Track checkable results below.

**Goal:** Apply the approved icon throughout Bulletnotes and make the site express its warm, tactile identity.

**Architecture:** A reproducible native export supplies existing app and packaging asset paths. The static site retains its gallery behavior and content while adopting paper, orange, silver, and an editorial layout.

**Tech Stack:** Swift/AppKit and iconutil for exports; existing Slint; static HTML/CSS/JS; agent-browser for visual checks. No new dependencies.

**Spec:** `docs/superpowers/specs/2026-10-31-brand-rollout-design.md`.

## Tasks

- [x] Export clean production PNGs, ICO and ICNS; update tray and runtime window references; verify formats and package checks.
- [x] Update README identity and site favicon/touch icon/header/footer/hero. Replace cinematic styling with warm editorial typography, paper surfaces, orange actions and silver details; preserve all gallery behavior.
- [x] Update branding and site documentation; distinguish rejected explorations from current production artwork.
- [x] Run appropriate native checks and browser verification at all required viewport sizes, all gallery combinations, 200% text, reduced motion, and no JavaScript. Inspect screenshots and resolve observed issues.
- [x] Review combined changes for scope, accessibility, and consistency. Deliver local results without publishing or committing.

## Verification results

- `scripts/check.sh`: formatting and clippy passed; 399 Rust tests passed, 6 ignored across 42 suites. The initial sandboxed run failed to bind local HTTP fixtures; the unchanged suite passed outside the sandbox.
- `python3 -m unittest discover -s tests -p test_package_desktop.py`: 13 passed.
- `node --check site/script.js`: passed. Prettier checks on site HTML/CSS passed.
- Native export validation: PNG sizes/alpha, ICO frames, ICNS round trip and deterministic exports passed. The approved master remains untouched.
- Browser: all 48 theme/paper combinations passed with matching images, captions and pressed states. No broken assets or editable fields. No horizontal overflow at 320, 390, 768, 1024 and 1440 px, or at 200% text at 320, 390 and 1440 px. Smallest supporting text is 14 px (28 px enlarged).
- Keyboard skip-link focus is visible. Reduced motion produces zero gallery image animations and disables smooth scrolling. Static content and previews remain readable with the script blocked.
- Axe WCAG A/AA: zero violations; gradient backgrounds required manual contrast checks. Worst-case tested orange button text is 4.72:1, muted text 4.78:1 and silver keyboard-cap text 10.78:1.
- Independent review: no material findings. Inspected desktop/mobile before-and-after screenshots and small-icon previews. Fixed a two-pixel mobile overflow at its rotated-image source.

No commit, push, publication or release was performed.
