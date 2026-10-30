# Bulletnotes identity

The approved app icon is a persimmon-orange handwritten **b**, with a shallow embossed finish and brushed-silver folded-note detail, on a warm eggshell-paper tile. Sparse fibers, natural inclusions, and subtle ivory variation make the paper individual. The angular brush terminal replaces the rejected capped bullet tip.

## Source and production files

- `bulletnotes-app-icon.png`: approved 1254 × 1254 generated raster master, preserved without modification.
- `bulletnotes-app-icon-{24,32,64}.png`: original design-review size samples.
- `bulletnotes-app-icon-review.png`: design-review comparison, with the approved version on the right.
- `../../assets/bulletnotes-logo.png`: clean 1024 px production icon for the app window, About window and README.
- `../../assets/native-preview-tray.svg`: simplified monochrome handwritten b for the small native tray.
- `../../site/assets/bulletnotes-logo.png`: 512 px website identity and share image; dedicated 32 px favicon and 180 px Apple touch icon live alongside it.
- `../../packaging/icons/`: 32, 128 and 256 px PNGs, Windows ICO, and macOS ICNS.

The approved master is a raster, not an editable vector. Production exports preserve its interior artwork and clip only the stray generated pixels outside the rounded tile. The tray is an intentionally simplified vector adaptation for legibility.

## Reproduce

On macOS with Swift and Apple's `iconutil` available, run from the repository root:

```sh
swift scripts/export-icons.swift
```

The export script needs no third-party dependency and recreates the application, website and packaging raster assets from the approved master. ICO contains 16, 24, 32, 48, 64 and 256 px PNG frames. ICNS contains native representations from 16 through 1024 px. The source master is not overwritten.

## Site design language

Use warm paper surfaces and dark warm ink; persimmon provides the focused accent. Headings use the locally hosted Instrument Serif, with italic accents to echo the mark's handwritten movement. DM Sans supports readable product copy and controls. Silver appears in quiet borders, keyboard caps and layered surfaces. Texture and depth support hierarchy and never obscure text or real app previews.

The icon finish was informed by Apple's Camera icon: coherent lighting, precise edge highlights, restrained relief and soft contact shadows. The site preserves this material contrast through matte paper, crisp orange actions and understated silver details.

## Verification

Production PNG dimensions and alpha, every ICO frame, and native ICNS round-trip representations were checked. Repeated export runs produced identical binary assets. Small icons were inspected on light and dark backgrounds. The site retains its actual editor screenshots, responsive text, keyboard focus, reduced-motion behavior and fourteen-theme/four-paper gallery.

## App design language

Bulletnotes and Bulletnotes Dark carry the identity into the native editor: eggshell or warm graphite paper, readable persimmon accents, silver-toned edges, and shallow control highlights. Settings uses the branded palette independently of custom note colors. Typography stays native, with Menlo for source editing.

The 128 × 128 transparent mask at `../../assets/paper/eggshell.png` was made with the built-in image-generation tool and downsampled with `sips`. Prompt brief: a seamless neutral grayscale alpha mask of opaque paper fibers and irregular grain on transparency, with no page, text, shadows, lighting, borders, or embedded paper color. It is tiled at logical scale and tinted at 0.5% in light mode and 1.5% in dark mode. High Contrast and Reduce Transparency suppress it and decorative depth. Imported palettes stay flat.

Capture synthetic app states without storage, credentials, or network:

```sh
rustup run 1.99.1 cargo run --locked --example capture_design_states -- /tmp/bulletnotes-design-captures
```

This renders editor, chrome, find, settings, browsing, connection, recovery, and About using the real Slint renderer, including minimum/default/wide and accessibility states. Product gallery captures use `capture_product_themes`, documented in the site README.

<details>
<summary>Final paper-mask prompt (built-in image edit)</summary>

Edit the paper-tooth texture just generated. Its alpha was too faint to survive downsampling. Make the texture mask much stronger: a seamless square with genuinely opaque charcoal micro-fibers and irregular dark grain occupying around half the area, remaining area fully transparent. At the generated 1024 pixel size use distinct 8–20 pixel wide dots and fibers with alpha near 255 inside them, no blurry washed-out alpha. This is a technical grayscale ALPHA MASK that the app will tint and reduce to 0.5% opacity itself. It must contain clearly visible opaque grain in the alpha channel even after reduction to 128x128. Uniform across edges, no page, text, shadow, lighting, border or embedded colored background.

The generated mask was downsampled to `assets/paper/eggshell.png`; the original generated output remains preserved.

</details>
