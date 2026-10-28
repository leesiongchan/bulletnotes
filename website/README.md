# Bulletnotes product page

A buildless product page for GitHub Pages. All fonts and images are local; there are no runtime dependencies, analytics, or external font requests. The theme gallery displays non-editable images rendered by the actual Bulletnotes editor. Its controls switch screenshots, not a browser recreation of the editor.

## Preview

From the repository root:

```sh
python3 -m http.server 4173 --directory website --bind 127.0.0.1
```

Open http://127.0.0.1:4173. Check desktop and mobile sizes, all 48 theme/paper combinations, image/caption agreement, keyboard focus, and reduced motion. The page must contain no editable fields. JavaScript is optional for reading the page; static theme screenshots remain visible without it.

## Capture theme screenshots

`examples/capture_product_themes.rs` uses the real `AppWindow`, appearance mapping, Markdown renderer, and bundled palettes through the existing Slint Skia testing backend. It renders a fixed sample note at the app's default 505 × 552 size, without opening the store, credentials, native application, or network. Images have no added editor controls, labels, or decorations.

Generate the 48 RGBA snapshots, then convert them to PNG on macOS:

```sh
rustup run 1.99.1 cargo run --locked --example capture_product_themes -- /tmp/bulletnotes-product-captures
python3 - <<'PY'
from pathlib import Path
import subprocess
output = Path('website/assets/themes')
output.mkdir(exist_ok=True)
for source in Path('/tmp/bulletnotes-product-captures').glob('*.pam'):
    subprocess.run(['sips', '-s', 'format', 'png', str(source), '--out',
                    str(output / (source.stem + '.png'))], check=True)
PY
```

## Publish

Enable **Settings → Pages → Source → GitHub Actions** for this repository. Push changes to `website/` or `.github/workflows/pages.yml` on `main`, or run the **Product page** workflow manually. The workflow deploys only this directory to https://leesiongchan.github.io/bulletnotes/.

The download section currently says no builds are published. When releases become available, update that statement in `index.html` alongside the download links.

## Design sources

The layout takes its colors and paper metaphor from Bulletnotes' bundled Paper theme and existing app icon. The hero and gallery show renderer captures. The twelve themes are Paper, Graphite, Matcha, Rose, Tartan, Sanrio, Dracula, Nord, Catppuccin Mocha, Dot Matrix, Tokyo Drift, and Mononoke. Each has captures for plain, dotted, ruled, and grid paper. The swatch colors come directly from the bundled palette JSON files.

Research references:

- [Flexoki](https://stephango.com/flexoki): digital ink and paper, legible warm palettes.
- [iA Writer](https://ia.net/writer): writing-first product presentation and typography.
- [Bear](https://bear.app/): real app imagery and appearance demonstrations.
- [GitHub Pages documentation](https://docs.github.com/en/pages/getting-started-with-github-pages/using-custom-workflows-with-github-pages): artifact-based static deployment.

Instrument Serif, DM Sans, and DM Mono come from the [official Google Fonts repository](https://github.com/google/fonts). Their SIL Open Font Licenses are included in `assets/fonts/`.
