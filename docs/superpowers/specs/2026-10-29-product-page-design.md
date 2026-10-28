# Bulletnotes product page

Build a GitHub Pages landing page that introduces Bulletnotes as a simple desktop scratchpad and directs visitors to development builds and build instructions.

## Direction

Use Paper's cream (#FCFAF4), ink (#353B32), and forest accent (#54705C), serif headlines, and real editor screenshots. Keep the existing visual identity while replacing promotional language with straightforward explanations of scratchpads, simplicity, offline saving, and optional SilverBullet sync.

The user specifically rejected the editable browser sampler and its invented editor frame/footer. All product previews must now be non-editable images from the app renderer. Palette swatches are selection controls, not representations of editor chrome.

## Experience

- Header: identity, Features/Themes links, and Get Bulletnotes.
- Hero: “A simple scratchpad.” beside an actual editor capture.
- Three concise benefits: capture, local work, SilverBullet sync.
- Screenshot gallery: twelve diverse bundled themes, each with plain/dotted/ruled/grid paper. Theme/pattern controls select one of 48 captures. Captions stay outside the editor image; no textarea, editable content, resize handle, or fabricated editor chrome.
- Three static theme screenshots show variety even without JavaScript.
- SilverBullet explanation, keyboard shortcuts, development download/build section, and footer.

## Constraints

Plain static HTML/CSS/JS, no runtime dependencies, no analytics or forms. Self-host fonts with licenses. Respect reduced motion, provide visible focus, accessible labels, and layouts down to 320px. Generate product screenshots using the real AppWindow and Slint Skia renderer without opening notes/settings/credentials/network; keep a reproducible capture example. Do not fabricate screenshots, reviews, adoption numbers, or production releases. No releases exist at implementation time. Source/build and release links must reflect this.

GitHub Pages workflow publishes only the site directory, with immutable action pins, minimal permissions, and no persisted checkout credentials. Keep app code untouched.

## Research

- https://stephango.com/flexoki — digital ink/paper and restrained legible palettes.
- https://ia.net/writer — writing-first product presentation and focused typography.
- https://bear.app/ — real product imagery and theme demonstrations.
- https://docs.github.com/en/pages/getting-started-with-github-pages/using-custom-workflows-with-github-pages — static artifact deployment.

## Verification

Inspect desktop and mobile screenshots, exercise all 48 image selections, confirm no editable fields remain, check image/caption agreement and screenshot provenance, audit accessibility, check local asset/anchor resolution and JavaScript syntax, and validate workflow configuration. Confirm remote publishing status before claiming the site is live.
