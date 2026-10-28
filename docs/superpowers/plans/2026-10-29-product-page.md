# Bulletnotes Product Page Implementation Plan

> **For agentic workers:** Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Correct the product page with accurate non-editable editor captures, straightforward scratchpad copy, and broader theme coverage.

**Architecture:** A buildless site in `website/`, with relative asset URLs. A dedicated GitHub Actions workflow uploads this directory and deploys it to Pages.

**Tech Stack:** HTML, CSS, vanilla JavaScript, self-hosted Google Fonts.

**Spec:** `docs/superpowers/specs/2026-10-29-product-page-design.md`

## Global Constraints

Use the app's real palettes and imagery; state development status. No framework, app-code edits, tracking, invented releases, or product claims unsupported by the repository.

## Review Focus

Mobile overflow, no-JavaScript readability, keyboard operation, reduced motion, and GitHub project-subpath asset resolution.

### Task 1: Product page

- [x] Create `website/index.html`, `website/styles.css`, `website/script.js`, and licensed font/image assets.
- [x] Capture twelve themes × four paper styles with the actual app renderer, using `examples/capture_product_themes.rs` without opening user data.
- [x] Replace the editable sampler and fabricated editor decoration with an image gallery. Rewrite the copy around scratchpads and simplicity.
- [x] Verify all 48 screenshot selections, no editable fields, desktop/mobile layouts, focus, reduced motion, and no-JavaScript behavior.

### Task 2: GitHub Pages

- [x] Create `.github/workflows/pages.yml` with verified immutable action revisions, Pages environment, least permissions, and site-only artifact.
- [x] Add preview/deployment guidance in `website/README.md` and a product-page link in the repository README.
- [x] Verify links/assets, JavaScript syntax, screenshot-helper formatting/lints, workflow validity, and publishing access; report the actual deployment state.

## Verification evidence

- Before the correction, the live page reproduced the reported issue: one editable textarea with vertical resizing and a fabricated frame/footer.
- Built and ran the isolated capture example; all 48 outputs are distinct 505 × 552 images from the actual app renderer.
- Browser verification passed every theme/paper selection, image/caption agreement, selected state, and zero editable fields.
- Visually checked desktop and mobile editor captures; 320px layout has no horizontal overflow. Keyboard activation, reduced motion, and script-blocked fallback passed.
- WCAG A/AA axe audit reported zero violations; JavaScript syntax, local assets/anchors, Rust formatting, capture-helper clippy, Prettier, and Pages actionlint passed.
- All 18 relevant Markdown, marker, and theme-selection tests passed.
- Independent review found one inaccurate Tartan alt description; corrected it to match the capture. No material findings remain.
- Remote deployment is verified separately after publication.
