# Assets

Where each asset type lives and what its manifest entry needs. [AGENTS.md](../AGENTS.md) links here; the table moved out of it with `D-106`. A new asset type adds its row here in the change that adds its manifest section.

Every asset file is listed in a `manifest.json` and named in game code by its registry ID. `just repo-check` checks that each file under an `assets/` folder is listed and each listed path exists; loaders validate what the files contain.

| Type | Location | Section | Required |
| --- | --- | --- | --- |
| Sprite | `assets/sprites/` | `sprites` | ID, filter `nearest`/`linear`; optional `normal_map`, `emissive_mask` |
| Animation | `assets/animations/` | `animations` | ID; sprite IDs must exist |
| Font | `assets/fonts/<Fam>/` | `fonts` | ID |
| Sound | `assets/sounds/` | `sounds` | ID; optional `looping`, `volume` |
| Shader | `assets/shaders/` | `shaders` | ID (`D-057`) |
| Material | manifest only | `materials` | `shader` ID, `uniform_defaults` (`D-058`) |
| Particle mesh | manifest only | `particle_meshes` | `vertices`, `indices` |

- Example-local assets: `examples/NN_name/assets/` with its own `manifest.json`. IDs are unique across loaded manifests; duplicates are fatal.
- Game code uses registry IDs; explicit scene loading follows `D-046`.

## Coverage exceptions

Asset coverage exceptions are explicit: complete font families and their inventory README; four vendored LYGIA helper fragments and their license; and example 03's explicitly loaded `scene.json` (`D-046`). A new exception needs review in the checker (`ASSET_EXCEPTIONS` in `scripts/check-repo.py`), not a broad ignored extension.
