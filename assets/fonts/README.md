# Fonts

Base font assets for the project. Three families covering sans-serif, serif, and monospace.

| Role  | Family          | License        |
|-------|-----------------|----------------|
| Sans  | Inter           | SIL OFL 1.1    |
| Serif | Source Serif 4  | SIL OFL 1.1    |
| Mono  | JetBrains Mono  | SIL OFL 1.1    |

All three are under the SIL Open Font License — free for commercial use, modification, and embedding. The full license text lives in each family's `LICENSE.txt`.

## Layout

```
fonts/
├── Inter/
│   ├── Inter-Variable.ttf            # variable font (wght 100–900, opsz 14–32)
│   ├── Inter-Italic-Variable.ttf
│   ├── static/                       # pre-instanced static weights
│   │   ├── Inter-Regular.ttf         (400)
│   │   ├── Inter-Medium.ttf          (500)
│   │   ├── Inter-SemiBold.ttf        (600)
│   │   ├── Inter-Bold.ttf            (700)
│   │   └── *Italic.ttf
│   └── LICENSE.txt
├── SourceSerif4/   (same layout)
└── JetBrainsMono/  (same layout)
```

## Which file should I load?

**Use the variable font (`*-Variable.ttf`)** if your text stack supports it. One file gives you every weight from 100 to 900, plus the italic axis from the matching `-Italic-Variable.ttf`. This is the modern default and keeps your binary small.

Rust crates known to handle variable fonts well:
- `cosmic-text` / `glyphon` (via `swash`) — full support
- `swash` — full support
- `ab_glyph` — supports variable fonts
- `fontdue` — supports variable fonts

**Use the static instances in `static/`** if:
- Your engine or text crate doesn't read variation axes
- You only need 1–2 weights and want the smallest possible asset footprint
- You're targeting a platform where variable font support is flaky

The statics were generated from the variable masters at the standard weights (Regular 400, Medium 500, SemiBold 600, Bold 700). For Inter and Source Serif 4, the optical-size axis was pinned to 14 (text-optimized).

## Recommended starting set

If you just want to load the minimum and move on:

- `Inter/static/Inter-Regular.ttf` — body UI text
- `Inter/static/Inter-Bold.ttf` — UI emphasis / headings
- `JetBrainsMono/static/JetBrainsMono-Regular.ttf` — code, debug overlays, console

Add the serif and other weights when you actually need them.

## Suggested role aliases

When you wire these into your engine, alias them by role rather than family name so you can swap fonts later without touching call sites:

```
font.sans          → Inter
font.sans.bold     → Inter Bold
font.serif         → Source Serif 4
font.mono          → JetBrains Mono
```

## Sources

- Inter — https://github.com/rsms/inter
- Source Serif 4 — https://github.com/adobe-fonts/source-serif
- JetBrains Mono — https://github.com/JetBrains/JetBrainsMono

## Attribution

The OFL does not require attribution in your shipped product, but you must include the `LICENSE.txt` (or `OFL.txt`) alongside the font files if you redistribute them. Keeping the per-family `LICENSE.txt` files in this folder satisfies that.
