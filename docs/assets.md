# Assets

Where each asset type lives and what its manifest entry needs. [AGENTS.md](../AGENTS.md) links here; the table moved out of it with `D-106`. A new asset type adds its row here in the change that adds its manifest section.

Every asset file is listed in a `manifest.json` and named in game code by its registry ID. `just repo-check` checks that each file under an `assets/` folder in this repository is listed and each listed path exists; loaders validate what the files contain. In a game's own repository, `tungsten check` loads every listed file and every unlisted `.json` file under the manifest's folder as a scene, so an unlisted image or sound goes unreported, and `tungsten package` ships it.

| Type | Location | Section | Required |
| --- | --- | --- | --- |
| Sprite | `assets/sprites/` | `sprites` | ID, filter `nearest`/`linear`; optional `normal_map`, `emissive_mask`, `nine_slice` (`[left, top, right, bottom]` in source pixels, for UI images; `D-148`) |
| Animation | `assets/animations/` | `animations` | ID; sprite IDs must exist |
| Font | `assets/fonts/<Fam>/` | `fonts` | ID |
| Font family | manifest only | `font_families` | family ID, `faces` (`fonts` IDs) (`D-115`) |
| Fallback chain | manifest only | `font_fallback` | a list of family IDs, not a map (`D-115`) |
| Sound | `assets/sounds/` | `sounds` | ID; optional `looping`, `volume` (finite, at least 0), `bus` (`"music"` or `"effects"`, effects by default) and `max_voices` (1 to 64, 64 by default); a sample rate of 1,000–384,000 Hz |
| Tilemap | `assets/tilemaps/` | `tilemaps` | ID; a Tiled `.tmj` with one embedded tileset whose tiles name sprite IDs in a `sprite_id` property; tile layers (a `kind` property `collision` for collision) and object layers, whose object classes name prefabs (`D-032`, `D-157`) |
| Shader | `assets/shaders/` | `shaders` | ID (`D-057`) |
| Material | manifest only | `materials` | `shader` ID, `uniform_defaults` (`D-058`) |
| Particle mesh | manifest only | `particle_meshes` | `vertices`, `indices` |
| UI theme | `assets/ui/themes/` | `ui_themes` | ID, `path`; a JSON object of optional fields over the built-in theme (`tokens`, `kinds`), colours `#rrggbb` or `#rrggbbaa`, each `line_height` finite and above 0; reloaded live and re-applied to the tree following the ID (`D-148`) |
| Prefab | `assets/prefabs/` | `prefabs` | ID, `path`; a scene entry's fields without `transform`: `sprite`, `visible`, `tag`, at most one tween and `components` by registered name, unknown keys failing; checked at load against the World (`D-157`), with the data entries (`D-171`) |
| Data entry | `assets/data/`, a folder per kind by convention (`assets/data/effects/`) | `data` | kinds, each an object of entries by ID, each with `path`: `"data": { "<kind>": { "<id>": { "path": … } } }`; a JSON file of the type a plugin registered for the kind, a key repeated at any depth failing; loaded with the prefabs in two phases and checked against the World, so a prefab or an entry may name any other; reloaded live (`D-171`) |

- Example-local assets: `examples/NN_name/assets/` with its own `manifest.json`. IDs are unique across loaded manifests; duplicates are fatal.
- A data entry's ID is unique within its kind, across loaded manifests: two kinds, or a kind and `prefabs`, may share an ID. One file backs at most one prefab or data entry, since an edit reloads one (`D-171`).
- Template-local assets: `templates/<name>/assets/` with its own `manifest.json`, the only root a game made from the template loads (`D-123`). Layer 1 loads each template manifest on its own, outside the examples' uniqueness merge, and `just repo-check` covers its files as it covers an example's.
- `font_fallback` chains concatenate in root order, each family keeping its first position. A family's faces and a chain's families may come from another root: references are checked on the merged graph (`D-089`).
- Font ID `engine_mono` is reserved for the engine font, `tungsten::ENGINE_FONT_ID`, which the HUD, the systems overlay and the inspector draw with (`D-123`), and `engine_mono_semibold` for its SemiBold face, the family's weight 600 for the debug views' titles and headers (`D-168`). A manifest `fonts` entry under `engine_mono` replaces the Regular face for engine text; beside another face of the same family, weight and style with other bytes, the earlier face draws and a warning names both. Manifests are not checked against the reservation.
- Game code uses registry IDs; explicit scene loading follows `D-046`.

## Coverage exceptions

Asset coverage exceptions are explicit: complete font families and their inventory README; four vendored LYGIA helper fragments and their license; and the fixtures crate's explicitly loaded `scene.json` (`D-046`). A new exception needs review in the checker (`ASSET_EXCEPTIONS` in `scripts/check-repo.py`), not a broad ignored extension.
