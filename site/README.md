# Bulletnotes product page

A buildless product page for GitHub Pages. All fonts and images are local; there are no runtime dependencies, analytics, or external font requests. The theme gallery displays non-editable images rendered by the actual Bulletnotes editor. Its controls switch screenshots, not a browser recreation of the editor.

## Preview

From the repository root:

```sh
python3 -m http.server 4173 --directory site --bind 127.0.0.1
```

Open http://127.0.0.1:4173. Check desktop and mobile sizes, all 56 theme/paper combinations, image/caption agreement, keyboard focus, and reduced motion. The page must contain no editable fields. JavaScript is optional for reading the page; static theme screenshots remain visible without it.

Typography uses rem units: body copy and feature descriptions 18–20px, controls/navigation 16px, and supporting text at least 14px. Verify computed text sizes at 320px, 390px, 768px, 1024px, and 1440px, and test text enlargement to 200% without clipping or horizontal overflow. Product screenshots retain the app's original typography.

## Publish

Publishing is automatic: a push to `main` that changes `site/` or `.github/workflows/pages.yml` runs the **Product page** GitHub Actions workflow. Merging a pull request with those changes into `main` triggers the same workflow. It uploads `site/` and deploys it to https://leesiongchan.github.io/bulletnotes/.

To deploy manually, open **Actions → Product page → Run workflow** and select `main`. GitHub Pages uses **Settings → Pages → Source → GitHub Actions**.

The download section currently says no builds are published. When releases become available, update that statement in `index.html` alongside the download links.

## Design sources

The site follows the approved app icon: warm eggshell paper, persimmon orange, brushed silver, and restrained depth. Instrument Serif gives headings a personal editorial voice; DM Sans keeps body copy and controls clear. A subtle paper tooth stays behind the content, orange buttons carry a shallow relief, and silver borders and keyboard caps echo the logo's folded-metal detail. The two-column hero uses actual Bulletnotes, Bulletnotes Dark, and Rose editor captures; the theme gallery showcases fourteen themes and four paper styles. App screenshots keep their original palettes and typography.

The header, hero and footer use the approved logo, with dedicated favicon and Apple touch exports. Brand image and stylesheet URLs include a version marker so existing browser caches do not retain the old green icon or cinematic styling. Reduced-motion preferences suppress transitions and gallery image animation. Content and static screenshots remain available without JavaScript.

The production icons are generated from `docs/branding/bulletnotes-app-icon.png` by `scripts/export-icons.swift`. See [branding documentation](../docs/branding/README.md) for source preservation, exterior alpha cleanup, PNG sizes and package formats.

Design references:

- [Approved Bulletnotes identity](../docs/branding/README.md): the primary reference for paper, orange, silver, handwriting and material contrast.
- [Guild of Food Writers](https://www.gfw.co.uk/): shared contours and a compact visual idea.
- [Granola](https://jobs.granola.so/blog/a-new-look-for-granola): warmth and deliberate human character.
- [Papier](https://raggededge.com/partnerships/papier): expressive restraint and editorial craft.
- [Apple Camera](https://apps.apple.com/us/app/camera/id1584216193): coherent lighting, fine edge highlights and restrained layered depth in the approved icon.
- [W3C text resizing guidance](https://www.w3.org/WAI/WCAG22/Understanding/resize-text.html): support enlargement up to 200% without losing content or functionality.
- [Web Interface Guidelines](https://raw.githubusercontent.com/vercel-labs/web-interface-guidelines/main/command.md): semantic controls, visible focus, balanced headings and reduced-motion support.
- [GitHub Pages documentation](https://docs.github.com/en/pages/getting-started-with-github-pages/using-custom-workflows-with-github-pages): artifact-based static deployment.

Instrument Serif, DM Sans and DM Mono come from the [official Google Fonts repository](https://github.com/google/fonts). Their SIL Open Font Licenses are included in `assets/fonts/`.
