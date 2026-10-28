# Bulletnotes product page

Build a GitHub Pages landing page that introduces Bulletnotes as a simple desktop scratchpad and directs visitors to development builds and build instructions.

## Direction

Use a modern product-page layout with warm cream, forest ink, a dark green theme section, large sans-serif headlines, selective serif accents, and real editor screenshots. Remove stationery props, tiny numbered captions, decorative annotations, and repeated claims. Rewrite every section around capture, retrieval, personalization, and optional SilverBullet sync.

The user identified widespread undersized text. Body copy is 18–19px, feature descriptions 17px, navigation and controls 16px, and labels, status, captions, and supporting information at least 14px. Use rem units and support enlargement up to 200% without clipping or horizontal overflow at 320px. The 14px floor is this design's choice, not a WCAG minimum-size claim.

The user specifically rejected the editable browser sampler and its invented editor frame/footer. All product previews must now be non-editable images from the app renderer. Palette swatches are selection controls, not representations of editor chrome.

## Experience

- Header: identity, Overview/Themes/Guide links, and Get the app.
- Hero: “Quick notes. Kept simple.” beside an actual editor capture, with clear development and platform information.
- Three concise benefits: create a scratchpad, save locally, and find a note later.
- Screenshot gallery: twelve diverse bundled themes, each with plain/dotted/ruled/grid paper. Theme/pattern controls select one of 48 captures. Captions stay outside the editor image; no textarea, editable content, resize handle, or fabricated editor chrome.
- Three static theme screenshots show variety even without JavaScript.
- SilverBullet explanation, a readable three-shortcut guide, development availability and platform requirements, and footer.

## Constraints

Plain static HTML/CSS/JS, no runtime dependencies, no analytics or forms. Self-host fonts with licenses. Respect reduced motion, provide visible focus, accessible labels, and layouts down to 320px. Generate product screenshots using the real AppWindow and Slint Skia renderer without opening notes/settings/credentials/network; keep a reproducible capture example. Do not fabricate screenshots, reviews, adoption numbers, or production releases. No releases exist at implementation time. Source/build and release links must reflect this.

GitHub Pages workflow publishes only the site directory, with immutable action pins, minimal permissions, and no persisted checkout credentials. Keep app code untouched.

## Research

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
