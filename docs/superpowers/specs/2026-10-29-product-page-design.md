# Bulletnotes product page

Build a GitHub Pages landing page that introduces Bulletnotes as a simple desktop scratchpad and directs visitors to development builds and build instructions.

## Direction

The user selected a dark cinematic direction: graphite (#101411), off-white (#F1F4EB), electric lime (#D0FF71), oversized DM Sans typography, and deliberate motion. Replace the conservative cream/serif/card composition with a centered poster hero (“Write it.”), three actual app captures on a spotlight stage, border-separated feature rows, a central theme gallery with neighboring images, and a full-width lime closing section. Keep the design simple: no WebGL, particles, custom cursor, scroll hijacking, autoplay theme cycling, or new runtime dependency.

Theme selection updates the real editor screenshot and a subtle surrounding glow derived from the selected palette. A brief image entrance runs on image load only when reduced motion is not requested. Page-load typography has one short entrance animation; all content is visible without JavaScript.

The user identified widespread undersized text. Body copy is 18–19px, feature descriptions 17px, navigation and controls 16px, and labels, status, captions, and supporting information at least 14px. Use rem units and support enlargement up to 200% without clipping or horizontal overflow at 320px. The 14px floor is this design's choice, not a WCAG minimum-size claim.

The user specifically rejected the editable browser sampler and its invented editor frame/footer. All product previews must now be non-editable images from the app renderer. Palette swatches are selection controls, not representations of editor chrome.

## Experience

- Header: identity, Features/Themes links, and Get the app.
- Hero: oversized “Write it.” centered above real Graphite, Paper, and Dot Matrix captures, with readable supporting copy and a clear development badge.
- Feature rows: create a scratchpad, save locally, and find a note later.
- Screenshot gallery: twelve diverse bundled themes, each with plain/dotted/ruled/grid paper. Theme/pattern controls select one of 48 captures. Captions stay outside the editor image; no textarea, editable content, resize handle, or fabricated editor chrome.
- Neighboring static editor previews show variety even without JavaScript.
- SilverBullet explanation, a readable three-shortcut guide, development availability and platform requirements, and footer.

## Constraints

Plain static HTML/CSS/JS, no runtime dependencies, no analytics or forms. Self-host fonts with licenses. Respect reduced motion, provide visible focus, accessible labels, and layouts down to 320px. Generate product screenshots using the real AppWindow and Slint Skia renderer without opening notes/settings/credentials/network; keep a reproducible capture example. Do not fabricate screenshots, reviews, adoption numbers, or production releases. No releases exist at implementation time. Source/build and release links must reflect this.

GitHub Pages workflow publishes only the site directory, with immutable action pins, minimal permissions, and no persisted checkout credentials. Keep app code untouched.

## Research

The chosen art direction is an interpretation of these references, not a reproduction:

- https://www.dia.studio/ — typography as the primary graphic and stripped-back navigation.
- https://nothing.tech/ — high contrast, technical labels, and product-first staging.
- https://lusion.co/ — a focal scene and purposeful motion; borrow restraint and staging without introducing 3D infrastructure.
- https://teenage.engineering/ — expressive product presentation with a small visual vocabulary.

- https://stephango.com/flexoki — digital ink/paper and restrained legible palettes.
- https://ia.net/writer — writing-first product presentation and focused typography.
- https://bear.app/ — real product imagery and theme demonstrations.
- https://linear.app/ — clear product hierarchy and focused section structure.
- https://www.raycast.com/ — concise positioning and prominent product presentation.
- https://culturedcode.com/things/ — restrained presentation of a native productivity app.
- https://www.w3.org/WAI/WCAG22/Understanding/resize-text.html — support text enlargement to 200%.
- https://raw.githubusercontent.com/vercel-labs/web-interface-guidelines/main/command.md — semantic controls, focus, typography, reduced motion, and image handling.
- https://docs.github.com/en/pages/getting-started-with-github-pages/using-custom-workflows-with-github-pages — static artifact deployment.

## Verification

Inspect desktop and mobile screenshots, exercise all 48 image selections, confirm no editable fields remain, check image/caption agreement, measure the 14px text floor at multiple viewport sizes, verify 200% text enlargement, audit accessibility, check local asset/anchor resolution and JavaScript syntax, and validate workflow configuration. Confirm remote publishing status before claiming the site is live.
