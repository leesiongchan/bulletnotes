# Bulletnotes product page

Build a GitHub Pages landing page that introduces the native SilverBullet notebook and directs visitors to development builds and build instructions.

## Direction

Proposed direction: an editorial field notebook, using Paper's cream (#FCFAF4), ink (#353B32), and forest accent (#54705C), oversize serif headlines, small monospaced annotations, layered real screenshots, and fine notebook rules. The alternate desktop scene would emphasize floating windows and motion; the notebook direction better reflects the existing icon and paper controls. This is the implementation's design choice, not a recorded user preference.

## Experience

- Header: identity, feature/appearance links, and Get Bulletnotes.
- Hero: “A little space to think.” beside real app imagery and paper annotations.
- Three concise benefits: capture, local work, SilverBullet sync.
- Interactive appearance sampler: Paper/Graphite/Matcha/Rose palettes and plain/dotted/ruled/grid paper. Clearly labeled website preview; edits stay in the current tab.
- SilverBullet explanation, keyboard shortcuts, development download/build section, and footer.

## Constraints

Plain static HTML/CSS/JS, no runtime dependencies, no analytics or forms. Self-host fonts with licenses. Respect reduced motion, provide visible focus, accessible labels, and layouts down to 320px. Use existing screenshots and icon; do not fabricate product screenshots, reviews, adoption numbers, or production releases. No releases exist at implementation time. Source/build and release links must reflect this.

GitHub Pages workflow publishes only the site directory, with immutable action pins, minimal permissions, and no persisted checkout credentials. Keep app code untouched.

## Research

- https://stephango.com/flexoki — digital ink/paper and restrained legible palettes.
- https://ia.net/writer — writing-first product presentation and focused typography.
- https://bear.app/ — real product imagery and theme demonstrations.
- https://docs.github.com/en/pages/getting-started-with-github-pages/using-custom-workflows-with-github-pages — static artifact deployment.

## Verification

Inspect desktop and mobile screenshots, exercise every sampler control and editable note, audit accessibility, check local asset/anchor resolution and JavaScript syntax, and validate workflow configuration. Confirm remote publishing status before claiming the site is live.
