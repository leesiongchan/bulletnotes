# Bulletnotes identity

The approved app icon is a persimmon-orange handwritten **b**, with a shallow embossed finish and brushed-silver folded-note detail, on a warm eggshell-paper tile. Sparse fibers, natural inclusions, and subtle ivory variation make the paper individual.

## Source and production files

- `bulletnotes-app-icon.png`: approved 1254 × 1254 generated raster master, preserved without modification.
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

## App design language

Bulletnotes and Bulletnotes Dark carry the identity into the native editor: eggshell or warm graphite paper, readable persimmon accents, silver-toned edges, and shallow control highlights. Settings uses the branded palette independently of custom note colors. Typography stays native, with Menlo for source editing.

The 128 × 128 transparent mask at `../../assets/paper/eggshell.png` is tiled at logical scale and tinted at 0.5% in light mode and 1.5% in dark mode. High Contrast and Reduce Transparency suppress it and decorative depth. Imported palettes stay flat.
