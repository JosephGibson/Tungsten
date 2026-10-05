# Assets

Where each asset type lives and what its manifest entry needs. [AGENTS.md](../AGENTS.md) links here; the table moved out of it with `D-106`. A new asset type adds its row here in the change that adds its manifest section.

Every asset file is listed in a `manifest.json` and named in game code by its registry ID. `just repo-check` checks that each file under an `assets/` folder is listed and each listed path exists; loaders validate what the files contain.

| Type | Location | Section | Required |
| --- | --- | --- | --- |
| Sprite | `assets/sprites/` | `sprites` | ID, filter `nearest`/`linear`; optional `normal_map`, `emissive_mask` |
| Animation | `assets/animations/` | `animations` | ID; sprite IDs must exist |
| Font | `assets/fonts/<Fam>/` | `fonts` | ID |
| Font family | manifest only | `font_families` | family ID, `faces` (`fonts` IDs) (`D-115`) |
| Fallback chain | manifest only | `font_fallback` | a list of family IDs, not a map (`D-115`) |
| Sound | `assets/sounds/` | `sounds` | ID; optional `looping`, `volume` |
| Shader | `assets/shaders/` | `shaders` | ID (`D-057`) |
| Material | manifest only | `materials` | `shader` ID, `uniform_defaults` (`D-058`) |
| Particle mesh | manifest only | `particle_meshes` | `vertices`, `indices` |

- Example-local assets: `examples/NN_name/assets/` with its own `manifest.json`. IDs are unique across loaded manifests; duplicates are fatal.
- Template-local assets: `templates/<name>/assets/` with its own `manifest.json`, the only root a game made from the template loads (`D-123`). Layer 1 loads each template manifest on its own, outside the examples' uniqueness merge, and `just repo-check` covers its files as it covers an example's.
- `font_fallback` chains concatenate in root order, each family keeping its first position. A family's faces and a chain's families may come from another root: references are checked on the merged graph (`D-089`).
- Font ID `engine_mono` is reserved for the engine font, `tungsten::ENGINE_FONT_ID`, which the HUD, the systems overlay and the inspector draw with (`D-123`). A manifest `fonts` entry under that ID replaces it for engine text; beside another face of the same family, weight and style with other bytes, the earlier face draws and a warning names both. Manifests are not checked against the reservation.
- Game code uses registry IDs; explicit scene loading follows `D-046`.

## Coverage exceptions

Asset coverage exceptions are explicit: complete font families and their inventory README; four vendored LYGIA helper fragments and their license; and example 03's explicitly loaded `scene.json` (`D-046`). A new exception needs review in the checker (`ASSET_EXCEPTIONS` in `scripts/check-repo.py`), not a broad ignored extension.
