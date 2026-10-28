# Bulletnotes Product Page Implementation Plan

> **For agentic workers:** Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Create and verify a distinctive product page ready for GitHub Pages.

**Architecture:** A buildless site in `website/`, with relative asset URLs. A dedicated GitHub Actions workflow uploads this directory and deploys it to Pages.

**Tech Stack:** HTML, CSS, vanilla JavaScript, self-hosted Google Fonts.

**Spec:** `docs/superpowers/specs/2026-10-29-product-page-design.md`

## Global Constraints

Use the app's real palettes and imagery; state development status. No framework, app-code edits, tracking, invented releases, or product claims unsupported by the repository.

## Review Focus

Mobile overflow, no-JavaScript readability, keyboard operation, reduced motion, and GitHub project-subpath asset resolution.

### Task 1: Product page

- [x] Create `website/index.html`, `website/styles.css`, `website/script.js`, and licensed font/image assets.
- [x] Verify desktop/mobile screenshots, theme and paper controls, editable note, focus, reduced motion, and no-JavaScript behavior in the browser.

### Task 2: GitHub Pages

- [x] Create `.github/workflows/pages.yml` with verified immutable action revisions, Pages environment, least permissions, and site-only artifact.
- [x] Add preview/deployment guidance in `website/README.md` and a product-page link in the repository README.
- [x] Verify links/assets, JavaScript syntax, workflow validity, and publishing access; report the actual deployment state.

## Verification evidence

- Browser screenshots inspected at 1440px desktop and 390px mobile; 320px overflow and keyboard activation checked.
- All 16 theme/paper combinations preserve preview text and update selected states and the live caption.
- WCAG A/AA axe audit: zero violations; patterned-background contrast manually checked against palette values.
- Reduced-motion CSS, script-blocked fallback, all relative assets/anchors, JavaScript syntax, Prettier, and actionlint checked.
- Independent static review found no material defects; reviewer browser access was unavailable, so visual QA was performed by the implementer.
- Remote deployment state is checked separately after publication.
