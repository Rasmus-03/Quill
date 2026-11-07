# macOS DMG window

What Finder shows when the DMG opens: a 660 × 400 pt background with the app icon and the
`Applications` link side by side. `package.sh` copies these files into the image; nothing here is
generated at build time, so the DMG still builds with `hdiutil makehybrid` (no mounted device, no
Finder scripting on CI, nothing extra installed in the signing job).

| File | What |
|---|---|
| `background.svg` | Source of the background: the app icon (`assets/app-icon/quill.svg`, linked, not copied) cropped as a cover on the Quill colour field (`#3b5bdb`), Ink and Paper. Its text is outlined, so rendering it needs no fonts: the wordmark is `Quill` in Inter SemiBold at 30 pt (cap height 21.83, baseline y 72, ink starting at x 266.73), the two labels are Inter and JetBrains Mono. |
| `background.tiff` | The background at 1x (660 × 400 px, 72 dpi) and 2x (1320 × 800 px, 144 dpi) in one HiDPI TIFF (16-colour palette, Deflate, sRGB). Goes to `.background/background.tiff`. |
| `dmg-layout.DS_Store` | Finder's view settings for the volume: window size, icon size 128, Quill.app at (326, 205), `Applications` at (574, 205), and the background. Goes to `.DS_Store` in the image; named so it isn't mistaken for (or ignored like) a Finder-generated `.DS_Store`. |
| `generate.py` | Writes `background.tiff` and `dmg-layout.DS_Store` from the SVG and the layout above. |

## The volume name has no version

The mounted volume is called `Quill`, not `Quill <version>`. The layout points at the background
through an alias that includes the volume name, and Finder resolves it by that name: with a
versioned name the window keeps its size and icon positions but shows no background (tested).
The DMG file name (`quill-<version>-macos-<arch>.dmg`) still carries the version.

## Rules

- **Finder draws the icon labels in black in light and dark mode** when a window has a background,
  so the area under both icons stays light (Paper).
- **Nothing goes inside the icon boxes:** artwork keeps 10 pt clear of each 128 pt icon box and of
  the label strip under it.

## Regenerate

Edit `background.svg` (or the layout constants in `generate.py`), then run, on any OS:

```sh
pip install 'pillow>=12' ds_store==1.3.3 mac_alias==2.2.3
python3 packaging/macos/dmg/generate.py   # needs resvg on PATH
```

Only the layout changed? (the volume or the app's name) The `.DS_Store` carries the volume name
and the icon's name, so it has to be regenerated too — and that needs no resvg and no fonts:

```sh
python3 packaging/macos/dmg/generate.py --only-ds-store
```

Both outputs are byte-for-byte reproducible and carry nothing from the machine that ran it: the
SVG renders without any fonts, and `dmg-layout.DS_Store` is written from scratch, its background
alias holding only the volume name and `/.background/background.tiff`. If you change the text in
the SVG, keep it outlined rather than retyped: the wordmark's `#text` path is Inter SemiBold at
30 pt, drawn from `assets/fonts/Inter-SemiBold.ttf` with HarfBuzz and fontTools
(`glyphSet` → `SVGPathPen`), placed by ink-left edge and baseline. Outlined text is what makes
the SVG render identically on a machine without fonts, and it is what keeps `background.tiff`
reproducible.

This follows VectorCraft's `packaging/macos/dmg/` (storytold/vectorcraft#493).
