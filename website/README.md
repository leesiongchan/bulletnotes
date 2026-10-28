# Bulletnotes product page

A buildless product page for GitHub Pages. All fonts and images are local; there are no runtime dependencies, analytics, or external font requests. The paper sampler is a website demonstration, not the desktop app; text lives only in the current page and is never saved or sent.

## Preview

From the repository root:

```sh
python3 -m http.server 4173 --directory website --bind 127.0.0.1
```

Open http://127.0.0.1:4173. Check desktop and mobile sizes, all theme/paper buttons, editable preview, keyboard focus, and reduced motion. JavaScript is optional for reading the page.

## Publish

Enable **Settings → Pages → Source → GitHub Actions** for this repository. Push changes to `website/` or `.github/workflows/pages.yml` on `main`, or run the **Product page** workflow manually. The workflow deploys only this directory to https://leesiongchan.github.io/bulletnotes/.

The download section currently says no builds are published. When releases become available, update that statement in `index.html` alongside the download links.

## Design sources

The layout takes its colors and paper metaphor from Bulletnotes' bundled Paper theme and existing app icon. The hero shows an actual repository screenshot. The interactive sampler uses the bundled Paper, Graphite, Matcha, and Rose palette colors.

Research references:

- [Flexoki](https://stephango.com/flexoki): digital ink and paper, legible warm palettes.
- [iA Writer](https://ia.net/writer): writing-first product presentation and typography.
- [Bear](https://bear.app/): real app imagery and appearance demonstrations.
- [GitHub Pages documentation](https://docs.github.com/en/pages/getting-started-with-github-pages/using-custom-workflows-with-github-pages): artifact-based static deployment.

Instrument Serif, DM Sans, and DM Mono come from the [official Google Fonts repository](https://github.com/google/fonts). Their SIL Open Font Licenses are included in `assets/fonts/`.
