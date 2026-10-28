# Bulletnotes Product Page Implementation Plan

> **For agentic workers:** Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Redesign the product page with new copy, a modern layout, and consistently readable typography while preserving accurate editor imagery.

**Architecture:** A buildless site in `website/`, with relative asset URLs. A dedicated GitHub Actions workflow uploads this directory and deploys it to Pages.

**Tech Stack:** HTML, CSS, vanilla JavaScript, self-hosted Google Fonts.

**Spec:** `docs/superpowers/specs/2026-10-29-product-page-design.md`

## Global Constraints

Use the app's real palettes and imagery; state development status. No framework, app-code edits, tracking, invented releases, or product claims unsupported by the repository.

## Review Focus

Minimum 14px meaningful text; readable 16px controls; mobile/200% text enlargement without overlap or overflow; accurate copy; no-JavaScript behavior; all 48 screenshot selections; keyboard operation; reduced motion; GitHub project-subpath asset resolution.

### Current redesign

- [x] Research modern product pages and current interface/resize guidance.
- [x] Rewrite the page hierarchy and copy; replace the stationery treatment with a simpler product presentation.
- [x] Replace tiny text rules with rem-based 18px body, 16px controls, and a 14px supporting-text floor.
- [x] Measure text sizes and check desktop/mobile layouts, 200% text enlargement, keyboard controls, all 48 previews, no-JavaScript fallback, and accessibility.
- [ ] Publish and verify the live page; independent review is complete.

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

The entries below document the earlier correction. Record the current redesign checks separately after verification.

- Before the correction, the live page reproduced the reported issue: one editable textarea with vertical resizing and a fabricated frame/footer.
- Built and ran the isolated capture example; all 48 outputs are distinct 505 × 552 images from the actual app renderer.
- Browser verification passed every theme/paper selection, image/caption agreement, selected state, and zero editable fields.
- Visually checked desktop and mobile editor captures; 320px layout has no horizontal overflow. Keyboard activation, reduced motion, and script-blocked fallback passed.
- WCAG A/AA axe audit reported zero violations; JavaScript syntax, local assets/anchors, Rust formatting, capture-helper clippy, Prettier, and Pages actionlint passed.
- All 18 relevant Markdown, marker, and theme-selection tests passed.
- Independent review found one inaccurate Tartan alt description; corrected it to match the capture. No material findings remain.
- Remote deployment is verified separately after publication.

## Modern redesign verification

- Measured 104–108 visible text elements at 320, 390, 768, 1024, and 1440px: no text below 14px, horizontal overflow, header overlap, or clipped controls.
- 200% text enlargement passes at both 320 and 1440px.
- All 48 screenshot selections retain image/caption agreement and a single selected theme/paper state; no editable fields.
- Actual JavaScript-disabled browser shows fallback content and static previews with inactive controls hidden. Browser launch flags must be repeated across commands to retain that session configuration.
- Keyboard selection passed. Accessibility audit: zero WCAG A/AA violations. Local assets, anchor links, JavaScript syntax, Prettier, and actionlint passed.
- Independent review found no material copy or interface issues. Removed duplicate/empty CSS created while exploring hero layouts.
