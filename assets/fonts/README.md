# Fonts

Base font assets for the project: sans-serif, serif, and monospace families. All three use SIL OFL 1.1, and each family directory includes its own `LICENSE.txt`.

| Role | Family | License |
| --- | --- | --- |
| Sans | Inter | SIL OFL 1.1 |
| Serif | Source Serif 4 | SIL OFL 1.1 |
| Mono | JetBrains Mono | SIL OFL 1.1 |

SIL OFL permits commercial use, modification, and embedding.

## Layout

```text
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

## Load Choice

Use the variable font (`*-Variable.ttf`) when the text stack supports it. One file covers weights `100–900`, the italic axis comes from the matching `-Italic-Variable.ttf`, this is the modern default, and it keeps the asset footprint small.

Rust crates known to handle variable fonts well:

- `cosmic-text` / `glyphon` (via `swash`) — full support
- `swash` — full support
- `ab_glyph` — supports variable fonts
- `fontdue` — supports variable fonts

Use the static instances in `static/` when your engine or text crate does not read variation axes, when you only need `1–2` weights and want the smallest possible asset footprint, or when you target a platform where variable font support is flaky.

Static instances were generated from the variable masters at standard weights Regular `400`, Medium `500`, SemiBold `600`, and Bold `700`. For Inter and Source Serif 4, the optical-size axis was pinned to `14` (text-optimized).

## Recommended Starting Set

Load these first if you want the minimum useful set:

- `Inter/static/Inter-Regular.ttf` — body UI text
- `Inter/static/Inter-Bold.ttf` — UI emphasis / headings
- `JetBrainsMono/static/JetBrainsMono-Regular.ttf` — code, debug overlays, console

Add the serif family and additional weights only when needed.

## Suggested Role Aliases

Alias by role, not family name. This keeps call sites stable if the family changes later.

```text
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

OFL does not require attribution in the shipped product, but you must include `LICENSE.txt` or `OFL.txt` alongside redistributed font files. Keeping the per-family `LICENSE.txt` files in this folder satisfies that requirement.
