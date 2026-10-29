# Bulletnotes product page

A buildless product page for GitHub Pages. All fonts and images are local; there are no runtime dependencies, analytics, or external font requests. The theme gallery displays non-editable images rendered by the actual Bulletnotes editor. Its controls switch screenshots, not a browser recreation of the editor.

## Preview

From the repository root:

```sh
python3 -m http.server 4173 --directory site --bind 127.0.0.1
```

Open http://127.0.0.1:4173. Check desktop and mobile sizes, all 48 theme/paper combinations, image/caption agreement, keyboard focus, and reduced motion. The page must contain no editable fields. JavaScript is optional for reading the page; static theme screenshots remain visible without it.

Typography uses rem units: body copy and feature descriptions 18–20px, controls/navigation 16px, and supporting text at least 14px. Verify computed text sizes at 320px, 390px, 768px, 1024px, and 1440px, and test text enlargement to 200% without clipping or horizontal overflow. Product screenshots retain the app's original typography.

## Capture theme screenshots

`examples/capture_product_themes.rs` uses the real `AppWindow`, appearance mapping, Markdown renderer, and bundled palettes through the existing Slint Skia testing backend. It renders a fixed sample note at the app's default 505 × 552 size, without opening the store, credentials, native application, or network. Images have no added editor controls, labels, or decorations.

Generate the 48 RGBA snapshots, then convert them to PNG on macOS:

```sh
rustup run 1.99.1 cargo run --locked --example capture_product_themes -- /tmp/bulletnotes-product-captures
python3 - <<'PY'
from pathlib import Path
import subprocess
output = Path('site/assets/themes')
output.mkdir(exist_ok=True)
for source in Path('/tmp/bulletnotes-product-captures').glob('*.pam'):
    subprocess.run(['sips', '-s', 'format', 'png', str(source), '--out',
                    str(output / (source.stem + '.png'))], check=True)
PY
```

## Publish

Publishing is automatic: a push to `main` that changes `site/` or `.github/workflows/pages.yml` runs the **Product page** GitHub Actions workflow. Merging a pull request with those changes into `main` triggers the same workflow. It uploads `site/` and deploys it to https://leesiongchan.github.io/bulletnotes/.

To deploy manually, open **Actions → Product page → Run workflow** and select `main`. GitHub Pages uses **Settings → Pages → Source → GitHub Actions**.

The download section currently says no builds are published. When releases become available, update that statement in `index.html` alongside the download links.

## Design sources

The user-selected direction is dark cinematic: graphite, off-white, lime accents, and oversized DM Sans. A poster-style hero stages three actual app captures, feature rows explain the scratchpad, and the theme gallery changes its ambient color with the selected palette. A lime closing section provides the source and release links. Motion is limited to a brief entrance and a screenshot fade after a selection; reduced-motion preferences disable both. The hero and gallery show renderer captures. The twelve showcased themes are Paper, Graphite, Matcha, Rose, Tartan, Sanrio, Dracula, Nord, Catppuccin Mocha, Dot Matrix, Tokyo Drift, and Mononoke. Each has captures for plain, dotted, ruled, and grid paper. The swatch colors come directly from the bundled palette JSON files.

Research references:

- [DIA](https://www.dia.studio/): our reference for treating typography as the main graphic.
- [Nothing](https://nothing.tech/): our reference for bold contrast and product-first staging.
- [Lusion](https://lusion.co/): our reference for purposeful motion; this page uses a short entrance rather than a continuous effect.
- [Teenage Engineering](https://teenage.engineering/): our reference for a small, consistent visual vocabulary.
- [Linear](https://linear.app/): focused product hierarchy and clear sections.
- [Raycast](https://www.raycast.com/): concise positioning and prominent product visuals.
- [Things](https://culturedcode.com/things/): restrained native-app presentation.
- [W3C text resizing guidance](https://www.w3.org/WAI/WCAG22/Understanding/resize-text.html): support enlargement up to 200% without losing content or functionality.
- [Web Interface Guidelines](https://raw.githubusercontent.com/vercel-labs/web-interface-guidelines/main/command.md): semantic controls, visible focus, balanced headings, and reduced-motion support.
- [Flexoki](https://stephango.com/flexoki): digital ink and paper, legible warm palettes.
- [iA Writer](https://ia.net/writer): writing-first product presentation and typography.
- [Bear](https://bear.app/): real app imagery and appearance demonstrations.
- [GitHub Pages documentation](https://docs.github.com/en/pages/getting-started-with-github-pages/using-custom-workflows-with-github-pages): artifact-based static deployment.

DM Sans and DM Mono come from the [official Google Fonts repository](https://github.com/google/fonts). Their SIL Open Font Licenses are included in `assets/fonts/`.
