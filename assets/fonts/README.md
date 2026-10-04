# Fonts

Base font assets for the project: sans-serif, serif, and monospace families. All three use SIL OFL 1.1, and each family directory includes its own `LICENSE.txt`.

| Role | Family | License |
| --- | --- | --- |
| Sans | Inter | SIL OFL 1.1 |
| Serif | Source Serif 4 | SIL OFL 1.1 |
| Mono | JetBrains Mono | SIL OFL 1.1 |

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

## Registered font IDs

The shared [manifest](../manifest.json) currently loads these static faces:

| ID | Path relative to `assets/` | Use |
| --- | --- | --- |
| `sans` | `fonts/Inter/static/Inter-Regular.ttf` | Body/UI text |
| `sans_bold` | `fonts/Inter/static/Inter-Bold.ttf` | Emphasis/headings |
| `mono` | `fonts/JetBrainsMono/static/JetBrainsMono-Regular.ttf` | Debug text |

Use those IDs in `TextSection.font_id`; other inventory files are not loaded automatically. Add a unique font entry to the appropriate manifest before using another face. Complete family directories, including unused weights and licenses, are permitted asset-coverage exceptions.

## Families and the fallback chain

The shared manifest groups the faces into families (`font_families`, `D-115`) and orders the families into the fallback chain (`font_fallback`):

| Family ID | Faces | Chain position |
| --- | --- | --- |
| `sans` | `sans` (400), `sans_bold` (700) | 1 |
| `mono` | `mono` (400) | 2 |

A text style names a family and a weight; the family resolves the weight to one of its faces. A glyph that the style's family lacks is taken from the chain's families in order. Add a face to a family by listing its `fonts` ID under `faces`.

The inventory includes upright/italic variable masters and static Regular (400), Medium (500), SemiBold (600) and Bold (700) faces. Inter and Source Serif 4 static optical size was pinned to 14 during authoring. Tungsten's current text API chooses the stored face's family, weight and style; it exposes no arbitrary variation-axis controls.

## Sources

- [Inter](https://github.com/rsms/inter)
- [Source Serif 4](https://github.com/adobe-fonts/source-serif)
- [JetBrains Mono](https://github.com/JetBrains/JetBrainsMono)

## Attribution

License texts remain with each family: [Inter](Inter/LICENSE.txt), [Source Serif 4](SourceSerif4/LICENSE.txt), [JetBrains Mono](JetBrainsMono/LICENSE.txt). Preserve them when redistributing these font files.
