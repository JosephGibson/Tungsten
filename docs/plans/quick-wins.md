# Quick wins

Step 7.1 of the [roadmap redo](roadmap-redo.md) (`D-215`): the cheap changes that cut the cost of every later change, before the analysis pass scopes the rest of Phase 7.

- **status:** in progress
- **goal:** W4b's deprecated surface gone; one integration-test binary per crate; a named list of small known-issue fixes; gamepads read again, with a radial dead zone and a sensitivity setting (P7-12); example 01's map objects keep authored IDs after one last save schema bump (P7-9); a tileset of only the used tiles in a stable order, written compactly; 01's asset families generated on their own, accepted ones locked, and bodies, hurtboxes and anchors as data beside their sprites. Measured: an enemy added to the map needs no schema bump, a sprite change outside the terrain touches no level file, and a terrain change moves only the props `D-214`'s placement sets on it.
- **non-goals:**
  - 7.9's generic saves (components registered as saved, per-area changes, entity references remapped by stable IDs) and its format change; 7.1 keeps 01's numeric save format.
  - 7.12's level format, commands and CLI; Tiled object `id`s stay generator-numbered and unused by 01.
  - Drawing or changing art: steps 8 and 9 must leave every sprite and sound byte-identical, and step 7 changes only which tile variant sits where (§7).
  - The template's generator (`templates/basic/tools/`), its settings screen, the pad's aim on the right stick (backlog row "The pad's aim"), rumble (P8-14), per-player maps.
  - The known issues outside §3's list (§3 names why each is left).
- **files to touch:** per step, in §1–§9. Records: this file, `ROADMAP.md`'s `quick-wins` line, `docs/known-issues.md` (each fixed entry out in its fix's commit), `api/*.txt`, `DESIGN.md`, `docs/LLM_INDEX.md`, `docs/validation.md`, `docs/assets.md`, `examples/01_platformer/tools/README.md`, `DECISIONS.md` and `docs/DECISION_INDEX.md` (three entries: §4, §6, §8).
- **ordered steps:**
  1. The deprecated surface removed (§1).
  2. One integration-test binary per crate (§2).
  3. Small known-issue fixes, one commit each (§3).
  4. The `gilrs` backend re-landed with its licence texts and decision (§4).
  5. A radial stick dead zone and a sensitivity setting (§5).
  6. 01's map objects keep authored IDs; schema 23 and its migration (§6).
  7. The tileset: used tiles only, stable IDs, compact rows; variation seeded per platform (§7).
  8. Generation per family, writes only on change, locks (§8).
  9. Bodies, hurtboxes and anchors as generated data beside their sprites (§9).
  10. Close: records, `just check`, `ROADMAP.md`, this plan deleted (§10).
- **done-when:**
  - Every step's own checks pass, quoted in the evidence log.
  - `just check` passes at the close; `just smoke` passes (GPU, local), with 01 started and played to its first area by the session.
  - The three measured tests pass: `an_enemy_added_to_the_map_needs_no_schema_bump` in `tests/game/saves.rs` (`cargo test -p example-01-platformer --locked`, §6), and `test_a_sprite_outside_the_terrain_touches_no_level_file` and `test_a_terrain_change_moves_only_its_props` in `tools/test_generate.py` (`just level-check`, §7).
  - `ROADMAP.md` drops `quick-wins` from Now; this file and its citations are gone.

## Context digest

- Run from the 0.112 branch beside `arpg-design` (7.19, paper only). That session may take decision IDs: recheck `rg -n '^## D-' DECISIONS.md | tail -1` and `git log -1` right before committing any entry.
- Per commit: `cargo fmt --all -- --check`, `cargo build --workspace --all-targets --locked`, `cargo clippy -p <crate> --all-targets --locked -- -D warnings` for each touched crate, then the step's tests. Conventional Commits, one line, staged by path, then `git push origin 0.112`.
- Example 01's whole `assets/` tree is generator output (`examples/01_platformer/tools/generate.py`); `--check` byte-compares 6,284 owned files and refuses strays. The generator needs Pillow: `python3 -m venv <scratchpad>/level-venv && <scratchpad>/level-venv/bin/pip install -r examples/01_platformer/tools/requirements.txt`, then `PATH=<scratchpad>/level-venv/bin:$PATH just level-check`; delete the venv after.
- 01's save is schema 22 (`src/save.rs:320`). Map objects take `SaveId`s by spawn order (`level/mod.rs:207-216`, `saved()` at `:266-274`): lava 0–2, decks 3–7, dummy 8, enemies 9–44, orbs 45–52, geysers 53–55, player 56, then a fresh run's iron, crates and kegs (`props::map_ids`, `props/mod.rs:467-478`). `restore` matches by ID against a fresh spawn (`save.rs:2199-2333`). `tools/level.json` is hand-authored; `level.tmj` (204k lines) is generated.
- The tileset is every non-backdrop sprite, sorted (2,710 entries, 676 used; `level.py:561-562`); the engine already loads sparse tile IDs (`core/assets/tilemap.rs:370-394`).
- Bodies and hurtboxes live in Rust consts, prefabs and generator constants at once (§9's table).

## 1. The deprecated surface

Remove, with every caller and test:

- The eleven arity-named queries in `core/ecs/world.rs:413-720`: `query_entities`, `query2`, `query2_entities`, `query3`, `query2_opt2`, `query3_opt2`, `query2_opt2_mut`, `query2_mut`, `query3_mut`, `query3_mut_without`, `query3_entities`. Their tests in `core/tests/ecs/world.rs` go where a tuple-form test already covers the shape, and are ported to the tuple form where none does.
- `DeltaTime` (`core/time.rs:37`, `lib.rs:94`) and the app's writes (`tungsten/app.rs:41-43`, `:293`, `:1141`, `:1235`, with their doc lines).
- `InspectorState::new_with_defaults`, `register` and `registered_len` (`tungsten/inspector.rs:141-176`); the state's own `registered` rows, `push_row` and anything else left without a reader once they go, so the overlay reads the `InspectRegistry` resource alone.
- `LightUbo::byte_size` (`render/lighting.rs:66`).
- `display.scale_mode` and `ScaleMode`: the field in both display structs, the parser (`core/display.rs:49-60`, `:360`, `:416`, `:455`, `:494`, `:546`), the umbrella's delta and telemetry fields (`tungsten/display.rs:20`, `:35`, `app.rs:926`, `telemetry.rs:69`, `:82`), the key in the root `tungsten.json` and `DESIGN.md`'s config sample (`:136`). A file that still sets the key loads as with any other unknown display key.
- `tuple_query_bench`'s pairs whose `arity` side calls a removed query (`core/benches/tuple_query_bench.rs`), since the `ecs` bench runs the same shapes on the tuple forms; the slice-form pairs (`single_mut`, `single_read`, `entity_mut`) stay. If no pair is left the bench and its `Cargo.toml` entry go.
- The 32 `#[allow(deprecated)]` sites.

**Files:** the ones named above, `core/tests/display.rs`, `core/src/tests/display.rs`, `core/src/tests/config.rs`, `tungsten/src/tests/{app,inspector,fixed_step,display}.rs`, `examples/02_bench/src/particles.rs` (a doc line), `crates/tungsten-core/Cargo.toml` if the bench goes, `api/*.txt` (`just api`), `docs/known-issues.md` (the follow-ups on the deprecated surface, `display.scale_mode` and `LightUbo::byte_size`), and any maintained doc that names a removed item (`rg -l 'DeltaTime|scale_mode|ScaleMode|byte_size|new_with_defaults|registered_len|query2|query3' -t md`, records excluded).

**Checks:** `rg -q '#\[deprecated' crates; test $? -eq 1` → 0; `rg -q 'allow\(deprecated\)' crates examples templates; test $? -eq 1` → 0; `cargo test -p tungsten-core -p tungsten -p tungsten-render --locked` → 0; `just api` then `git diff --numstat api/` shows deletions only; `just docs-ratchet` → 0.

## 2. One integration-test binary per crate

R7's compile-time item: each `tests/*.rs` is a binary that links the crate again. Core's eight, the umbrella's four and the fixtures' four become one binary each, as 01 and the template already are (`tests/game/main.rs`): `tests/integration/main.rs` with one `mod` per old file, the files moved unchanged under `tests/integration/`. Render's and the bench's single binary stay.

- Only core's `display.rs` sets environment variables, under its own `ENV_LOCK`, and no other integration test reads `Config` or those variables, so the merged binary needs no shared lock. A test that reads the environment later must take that lock; `main.rs` says so.
- Callers by binary name move to `--test integration` with module filters after `--`: `justfile:34` (`physics-release`), `:47-49` (`visual`), `:109` (`repo-check`), `docs/validation.md:18`, `scripts/check-repo.py:497`, `:507`, `:538`, `scripts/test-check-repo.py:431`, `:491-492`, the three physics files' run lines (`physics_determinism.rs:13`, `physics_containment.rs:26`, `physics_tunneling.rs:6`). The step greps every old name with `--test` before committing.

**Checks:** test counts equal the baseline taken at `b941c72` (core 41: audio_decode 8, composition 5, decision_index 2, display 6, manifests 5, physics_containment 7, physics_determinism 1, physics_tunneling 7; umbrella 27: asset_smoke 3, atlas_integration 2, camera 7, particles 15; fixtures 19: post_regression 5, screens 7, transition_regression 1, ui_regression 6), by `cargo test -q -p <pkg> --test integration --locked -- --list | rg -c ': test$'`; `just physics-release` → 0; `just repo-check` → 0; `just script-test` → 0. Indicative: the wall time of `cargo test -p tungsten-core --no-run --locked` after touching `core/src/lib.rs`, before and after, in the evidence log.

## 3. Small known-issue fixes

One commit each, each with the test named, each removing its `docs/known-issues.md` entry:

| # | Fix | Files | Test |
| --- | --- | --- | --- |
| K1 | A `PingPong` tween reflects its overshoot at a turn, so its period stops drifting | `tungsten/tweens.rs`, `tungsten/src/tests/tweens.rs` | the clamp-pinning test turned into one asserting the reflected value and an undrifted period over many turns |
| K2 | `charges { all: true }` starts and counts its refill only once no charge is left, as a use does | `kit/intent/ability.rs`, `kit/src/tests/ability.rs` | charges spent in separate steps refill `refill` seconds after the last |
| K3 | A whole resist (`resist` ≥ 1) neither launches nor locks a platformer controller, nor stores a push | `kit/motion/knockback.rs`, its tests | a resist-1 controller keeps its velocity and input after `knock_back` |
| K4 | `render.max_frame_latency = 0` warns and falls back at `Config::load`, as `display.max_frame_latency = 0` does | `core/config.rs`, `core/src/tests/config.rs` | a config with 0 loads with one warning and `None` |
| K5 | The manifest tests' helper removes its temp directories | `core/src/tests/assets/manifest.rs` | the helper's directory is gone after its guard drops |
| K6 | `just checks` recommends `just script-test` for `templates/basic/tools/*.py` | `scripts/check-repo.py`, `scripts/test-check-repo.py` | a case for a template tool path |
| K7 | Records only: the bloom shader-ID ordering note beside the seeding (`render/renderer.rs:338`), `D-041`'s missing `Amended by D-096` marker line (and its index row), `assets/fonts/README.md`'s SemiBold copy | those three | `just repo-check`; `cargo test -p tungsten-core --test integration --locked -- decision_index` |

Left, with why: the minimised window's spin (P2, the app's pacing, wants a display check of its own); F9 and F11 into `Settings` (a policy choice); the macOS mouse buttons and `notify` features (W7b, no host); the command buffer's identity, `has` through the slot and the perf entries (Phase 7's CPU and GPU passes); the label's ulp wrap (7.17 reworks text fitting); 01's hazard touches, bat alert and ricochet (01 stops); `.claude/settings.json`'s stale commands and the template's licence (the owner's); the SMAA port (GPU pass).

**Checks per fix:** its test, the crate's tests, clippy, fmt, the workspace build.

## 4. The gamepad backend

Re-land M54 step 5 from `perf-runs/20261014-m54-labels-gamepad/step5-skipped.patch` (856 lines, against the 2026-10-14 tree; `git apply` is denied, so hunk by hand or `patch -p1 --dry-run` first). Core's pad input, bindings and UI routing landed in 0.74 without a decision; this step's entry records both halves.

- `gilrs = "0.11"` in the workspace (`D-015` rule 1), optional in `crates/tungsten` behind a default `gamepad` feature; the crate-private backend in `input_bridge.rs` built with `GilrsBuilder::new().with_force_feedback(false)`, so one backend thread (the force-feedback server is the second, `gilrs-0.11.2/src/ff/server.rs:290`); `app.rs` polls it on the main thread each frame.
- Licence texts: `licenses/gilrs/LICENSE-APACHE` and `LICENSE-MIT` from `gitlab.com/gilrs-project/gilrs` at `07e286e24b046cf39e5c367daa2770b805a64692` (fetched raw, `curl -o`, not through a summarizer), a `texts` entry for `gilrs` 0.11.2 and `gilrs-core` 0.6.8 in `licenses/clarifications.json`, and the patch's SDL_GameControllerDB entry and file. Every other crate new in `Cargo.lock` is checked the same way: `fd -d 1 -i 'licen|copying' ~/.cargo/registry/src/*/<crate>-<ver>/`.
- Thread audit before the decision: `rg -n 'thread::|\.spawn\(' ` over `gilrs` and `gilrs-core`'s registry copies, naming each thread the default builder would start.
- Records as the patch has them, renumbered: the decision (the next free ID; the patch's `D-162` is taken) with marker lines on `D-045`, `D-131`, `D-144` and `D-149` and their index rows, the patch's "Breaks for the freeze" line dropped (`D-215`); `AGENTS.md`'s thread rule (three threads); `DESIGN.md`'s rule-1 table and frame loop; `libudev-dev` in `.github/workflows/ci.yml` and `release.yml`; `docs/getting-started.md`; `scripts/release.py`'s Linux line; `docs/known-issues.md`'s backend entry out.

**Checks:** `cargo build -p tungsten --no-default-features --features kit --locked` → 0 (no `gilrs`); the patch's `tests/input_bridge.rs` cases → 0; `just deps` → 0; `just notices` → 0; `just script-test` → 0 (workflow lint); `cargo test -p tungsten-core --test integration --locked -- decision_index` → 0. With a pad attached, the template's menus move on the d-pad and the stick (otherwise reported as not run, an owner check).

## 5. Stick dead zone and sensitivity

`ActionMap::value` reads a stick axis rescaled from `STICK_RELEASE` per axis (`core/input/action_map.rs:557-567`). A radial dead zone takes both axes of the stick together: below the dead zone the stick reads 0; above it the vector is rescaled from the dead zone to 1 along its own direction, times the sensitivity, its length clamped to 1. The digital presses keep `STICK_PRESS` and `STICK_RELEASE`. gilrs's own per-axis deadzone filter is turned off in the builder so the two do not stack, keeping its jitter and d-pad filters.

- `InputState` holds a `StickResponse { dead_zone, sensitivity }` (defaults 0.15 and 1.0, clamped to 0..0.9 and 0.25..2.0) behind a setter, so `ActionMap::value(&self, input, action)` (`action_map.rs:561`) reads it from the `input` it already takes, and the kit's `axis_value` (`kit/intent/action_input.rs:186`) needs no change.
- Core's `Settings` (`core/settings.rs:99`) gains `stick_dead_zone` and `stick_sensitivity`; an older settings file without them loads the defaults. At startup `App::new` applies the settings (`Config::apply_settings`, `tungsten/app.rs:248`) before it inserts `InputState` (`:294`), so it sets the response on that `InputState` from the loaded settings; at run time `tungsten::apply_settings` (`tungsten/settings.rs:39`) writes it to the live one.

**Files:** `core/input.rs`, `core/input/action_map.rs`, `core/settings.rs`, core's input and settings tests, `tungsten/app.rs`, `tungsten/settings.rs`, the umbrella's settings tests, `tungsten/input_bridge.rs` (the filter), `api/*.txt`.

**Checks:** core tests for a diagonal under the dead zone reading 0 on both axes (a per-axis zone passes it), a diagonal past it rescaled along its direction, sensitivity 2 clamping to length 1; a settings file without the fields. Umbrella tests reading an action's `value` from a held stick after `apply_settings` changes the response, and after an `App` starts on a user folder whose settings file sets it. A settings field is a format change, so no decision unless the step finds one constrains later work.

## 6. Authored map IDs and schema 23

P7-9: map objects key by authored names, so the save stops depending on spawn order.

- **Names.** Every saved map object gets a unique authored name in `tools/level.json`: the enemies, dummy and pickups already have one; the three lava hazards, five decks and three geysers get one (`lava_<place>`, `deck_<place>`, `geyser_<place>`); `level.py`'s `build_objects` carries each into the object's `name`, and `validate_level` refuses an empty or duplicate name in a saved layer.
- **IDs.** Only the objects `level::spawn` numbers today (`saved()`, `level/mod.rs:266-274`: lava, decks, the dummy, enemies, orbs, geysers) are matched against a fresh spawn at a load, so only they take authored IDs: a `SaveId` derived from the name, 32-bit FNV-1a with the top bit set. The player takes the ID of the name `player`. Runtime spawns count from 0 with the top bit clear, and `SaveIds::take` refuses to reach it. `level::spawn` reads each object's name beside its entity (`spawn_map_objects` returns them in object order), and refuses two saved objects whose IDs collide.
- **Marker props stay runtime spawns.** A fresh run's iron, crates, kegs and stones take counter IDs, as today; a load restores them from the save and skips the markers. Marker names keep naming kinds (`level.py:297`, `level::markers` at `level/mod.rs:317`). The map's own crates and kegs, which the summon cap leaves alone (`props::place`), are told today by `props::map_ids`'s range off the player's ID (`props/mod.rs:467-499`), which hashed player IDs break: a saved `map` flag on the prop replaces the range, set where a fresh run places them.
- **Age order.** `toys::place_iron` (`toys/mod.rs:416`) and `props::place` evict the lowest `SaveId` as the oldest. Neither touches a `level::spawn` object and marker props keep counter IDs, so both orders hold; the step greps every other `SaveId` sort or comparison (`rg -n 'SaveId' src | rg 'sort|min_by|max_by|cmp'`) and keeps each policy.
- **Load.** `restore` keeps matching by ID. A map object the save does not name keeps its fresh state (an added enemy appears at its home); a saved map ID with no live object follows today's respawn path. The step confirms both in `restore` (`save.rs:2199-2333`) before relying on them.
- **Migration.** `SCHEMA` 23; `to_23` maps schema 22's IDs 0–56 (lava, decks, the dummy, the 36 enemies, 8 orbs, 3 geysers in spawn order, then the player) to the new ones through a frozen table of schema 22's names, through `map_ids` (`save.rs:1202-1270`, `/black_hole/id` and the hole list included). Every ID from 57 up is a runtime ID in a schema 22 save, the fresh run's marker props and older saves' runtime spawns alike (the schema 11 fixture's iron 42 reaches 62 through `to_12`, `to_19` and `to_22`), and keeps its number, as does `next_id`. `to_23` sets the `map` flag on crates and kegs inside schema 22's `props::map_ids` range, frozen as constants; a save born before schema 12 has no crates or kegs, so its shifted runtime iron cannot take the flag. Every older arm ends in `to_23`. The `as_schema_*` test rewriters start from a current capture, so each first inverts `to_23`, or an older fixture shifts twice.
- **Tests moved:** `tests/game/saves.rs` (the schema 9 layout pin, the goblins, geysers and bat c tests, `schema_*_id` helpers, `map_ids` helper), `dummy.rs:22-49`, `enemies.rs:2924-2949`, `props.rs:530-541`.
- **Decision:** one entry for authored map IDs: unique names in a level's saved layers, save IDs derived from them, runtime IDs below the top bit; 7.12's format keeps the names.

**Files:** `src/save.rs`, `src/shared.rs`, `src/features/level/mod.rs`, `src/features/run/mod.rs`, `src/features/props/mod.rs`, `src/features/toys/mod.rs` (if the audit finds a policy to keep), `src/features/enemies/mod.rs`, `src/features/dummy/mod.rs`, `src/features/drops.rs`, `src/features/player/mod.rs`, `tools/level.json`, `tools/level.py`, `assets/tilemaps/level.tmj` (regenerated), the tests above, `tools/README.md`.

**Checks:** `an_enemy_added_to_the_map_needs_no_schema_bump`: a capture from the shipped map loads into a map built with one more enemy (the `TilemapData` edited in the test); every saved object comes back by ID, the new enemy stands at its home, and `SCHEMA` is unchanged. Every fixture `schema8.json`–`schema13.json` loads to the state its tests pin, the schema 11 fixture's iron 42 keeps runtime ID 62 and no `map` flag, and every `as_schema_*` test passes. A summon past the crate cap removes the oldest summoned crate and no map crate, in a fresh run and in a migrated schema 22 capture. `just level-check` → 0.

## 7. The tileset

- **Used tiles only, stable IDs.** `tools/tileset.json`, written by the generator beside `outputs.json`: `{"next": N, "ids": {sprite: id}}`, IDs only ever added. `build_level` gives each sprite the map uses its ID (a new one takes `next`), writes only the used ones into the embedded tileset, sparse, and `--check` fails when the file would change. The first run numbers today's 676 used sprites in sorted order: one last renumbering of every cell.
- **Compact.** `level.tmj`'s tile layers write one map row per line, and each object on one line; keys stay sorted. The engine reads it unchanged (`serde_json`).
- **Validators.** `validate_level`'s contiguous-ID and `tilecount` checks (`level.py:616`, `:622`), `placement.py:297` and `test_generate.py:50`, `:80` read the tileset's IDs instead of `id + 1`.
- **`opaque_tiles`** lists only used sprites (`generate.py:830-834`).
- **Local variation.** `terrain_tiles` and `background_tiles` draw from one stream (`level.py:564`), so a change to one platform re-rolls every later variant, and props settled on the drawn art (`placement.settle`) move with them. Each platform's terrain takes its own stream, `rng_for('terrain-variation-' + name)`, as `variants: own` platforms already do (`level.py:381`). `background_tiles` decides which cells draw from the terrain's occupancy and roofs (`level.py:446`, `:464`, `:473`), so a per-platform stream would still shift with a neighbour's change: each background cell seeds its own draw from its position (`rng_for(f'background-{x}-{y}')`). This re-rolls today's variants once: the session renders before and after stills of three areas for the owner's next playtest, and the owner may ask for the old picks back on named platforms (Q2).

**Files:** `tools/level.py`, `tools/placement.py`, `tools/generate.py`, `tools/test_generate.py`, `tools/tileset.json` (new), `tools/README.md`, `assets/tilemaps/level.tmj`, `assets/data/opaque_tiles/level.json`, `tools/outputs.json`.

**Checks:** `test_a_sprite_outside_the_terrain_touches_no_level_file`: `build_level` with one extra unused sprite, and with one enemy frame's pixels changed, writes `level.tmj` and `opaque_tiles` byte-identical. `test_a_terrain_change_moves_only_its_props`: one platform widened by a tile in a copy of `level.json`, run twice, on an open-air platform and on an indoor one whose widening changes which background cells draw; every object but the props `prepare` sets on that platform keeps its position and sprite (compared without Tiled `id`s), and tile cells change only within the platform's bounds widened by one cell, in every layer. `just level-check` → 0; `cargo test -p example-01-platformer --locked` → 0; `wc -l assets/tilemaps/level.tmj` under 60,000.

## 8. Families and locks

- **Write on change.** Write mode leaves a file whose bytes are unchanged untouched, so the hot-reload watcher and `git status` see only real changes.
- **Families.** A table in `generate.py` names each family (the player, each enemy by its `ENEMY_CLIPS` prefix, the dummy, balls, stones, projectiles, props, scenery, terrain, backdrops, effects, tomes, sounds, music, level) with its build function and the output paths it owns. `generate.py --family NAME` (repeatable) builds only those families and their post-steps (mirroring, sprays, normal and emissive maps, `validate_art` on their sprites), writes their files, and rewrites `manifest.json` and `outputs.json` only if their entries changed. A full run stays the default and `--check` stays whole.
- **Derived outputs.** An output drawn from another family's code belongs to the family whose code draws it: the troll's icons call `troll_canvas` (`effects.py:637`, `:643`, through the icon builder at `:712`), so they are the troll's, and the step lists every such call before the split (`rg -n` for each actor's canvas and pose functions across `effects.py`, `scenery.py` and `polish_art.py`), the ice and frost fits (`check_ice_fits`, `generate.py:716-734`) included. Where an output must stay in another family, that family declares the dependency and `--family X` rebuilds every family that declares X, under each one's lock.
- **Locks.** `tools/locks.json`: per family, the date and each owned file's SHA-256. Any run that would change a locked file fails, naming the family and files; `--lock NAME` records a family's current hashes, `--unlock NAME` removes its entry (the owner's call). `--check` verifies every locked file's hash.
- **Which are locked (Q1):** every family whose art the owner has accepted, which is all of 01's today; the level family is source-driven and never locked.
- **Decision:** one entry for the asset-session lock: accepted families' hashes in `tools/locks.json`, enforced by the generator; an unlock is the owner's.

**Files:** `tools/generate.py`, `tools/art.py`, `tools/sfx.py` (the family split), `tools/locks.json` (new), `tools/test_generate.py`, `tools/README.md`, `docs/assets.md`, `docs/operating-rules.md`'s Assets line if its wording no longer fits.

**Checks:** the refactor leaves all 6,284 outputs byte-identical (`--check` → 0 before and after); a test generating each family alone writes only its paths and matches a full run's bytes; a test that patches the troll's drawing (one pixel in `troll_canvas`, in memory) and runs `--family troll` changes exactly the files a full run changes, icons included; a test changing a locked file's bytes fails the run; `--family troll`'s wall time against a full run's, in the log.

## 9. Bodies, hurtboxes and anchors as data

Today the same numbers live in Rust consts, prefabs and generator constants:

| Item | Rust | Prefab | Generator |
| --- | --- | --- | --- |
| Player body | `BODY_OFFSET` (48,65), `PLAYER_HALF` (20,28), `player/mod.rs:32-33`; uses in `player/systems.rs:414`, `spells/systems.rs:367`, `gun/systems.rs:162`, `combat/draw.rs:132`, `run/mod.rs:85` | `content/prefabs/player.json:6`, `:9`, `:11` | `actors.py:28-35`, `art.py:92`, `:96`, `:252` |
| Enemy bodies | `KindInfo` `frame`, `half`, `centre`, `eye` (`enemies/mod.rs:139-272`) | each enemy's prefab | `effects.HURTBOXES` (`effects.py:761-769`), `actors.py:1343-1344`, `TrollPose` |
| Anchors | troll `FIST`, `HANDS` (`troll.rs:94`, `:111`), goblin `HAND` (`goblin.rs:63`), crawler `SNOWBALL_FROM` (`crawler.rs:86`), beetle `PLOUGH_AT` (`beetle.rs:55`) | — | the poses that draw them |

- The generator writes `assets/data/bodies/<actor>.json` per actor (the player, each enemy, the dummy): frame size, body half extents and centre, hurtbox, eye and named anchors, from the constants that draw the frames, with left-facing values mirrored as `mirror_anchors` does. 01 registers a `bodies` data kind and reads these in place of the consts; `check_ice_fits` and a new check hold every prefab's rigid body and hurtbox to its actor's file, so a prefab that disagrees fails `--check`.
- The lantern's anchors stay where they are; the prefab's duplicate glow offset (`player.json:28`) is read from them.
- `save.rs`'s historical `S12_BODY_OFFSET` stays a const (a migration's frozen value).
- Smallest version: actors only. Props' and toys' boxes (`STONE_*`, `CRATE_*`, `KEG_*`, `MUZZLE_OFFSET`, `SPRAY_MUZZLE`, `BOMB_TIP`, `SCORCH_ANCHOR`) follow in a later 01 stop, as a Later line.

**Files:** `tools/generate.py`, `tools/actors.py`, `tools/effects.py`, `tools/test_generate.py`, `tools/README.md`, `assets/data/bodies/*.json` and `manifest.json` (generated), `src/features/player/{mod,systems}.rs`, `src/features/enemies/{mod,systems,draw,troll,goblin,crawler,beetle}.rs`, `src/features/dummy/mod.rs`, `src/features/combat/draw.rs`, `src/features/spells/systems.rs`, `src/features/gun/systems.rs`, `src/features/run/mod.rs`, their tests, `docs/assets.md`.

**Checks:** every sprite byte-identical (`--check` before the data files land shows only the new `bodies` files and manifest entries); `rg -q 'const (BODY_OFFSET|PLAYER_HALF|FIST|HANDS|HAND|SNOWBALL_FROM|PLOUGH_AT)\b' examples/01_platformer/src; test $? -eq 1` → 0; a test moving one actor's body in its data file moves the spawned body with no Rust change; `cargo test -p example-01-platformer --locked` → 0; `just level-check` → 0.

## 10. Close

`docs/known-issues.md`, `docs/LLM_INDEX.md`, `docs/validation.md`, `docs/assets.md` and the tools README current; a Later line for §9's props and toys; `just check` → 0; `just smoke` → 0; 01 played to its first area; `ROADMAP.md`'s `quick-wins` line out of Now; this file deleted with its citations (`rg -l 'quick-wins.md'`).

## Session order

All at xhigh (code; no art is drawn). Each session reads this file whole, then only its steps' files.

1. 2026-10-28: this plan and its critique round; steps 1–3.
2. Critique round 2 on §5–§8 (below), then steps 4–5.
3. Step 6.
4. Step 7.
5. Step 8.
6. Steps 9–10.

## Questions, each with a default

- **Q1** Which families lock at step 8? Default: all of 01's art and sound families, since every example stop closed with the owner's acceptance; the level is never locked.
- **Q2** Step 7 re-rolls tile variants once. Default: accepted unless the owner names platforms at the next playtest, whose old picks then come back as `variants` entries.
- **Q3** Save IDs by hash or by a committed table? Default: the hash; a table is one more file to keep in step.
- **Q4** A settings-screen row for the stick? Default: no; the template's screen takes it with 7.11 or 7.17.

## Stop conditions

- A step whose stated byte-identical outputs differ: stop, report the files, change nothing to force a match.
- `just notices` fails after §4's texts: name the crate and its missing file; do not skip the backend.
- A schema fixture that no longer loads to the state its test pins: stop at step 6.

## Critique

- **Round 1** (2026-10-28, `/critique --repo` at xhigh on the draft; 30 commands, 480 s, the 5h window 41% → 79%): six findings, all checked against the tree and applied. (1) The frozen migration table took IDs from 57 up, where older saves' shifted runtime spawns sit, as map props: marker props now stay runtime IDs and the table stops at 56 (§6). (2) Marker names name kinds, so they cannot be identities: same fix. (3) Iron and props evict by lowest `SaveId`: §6's age-order audit, and a saved `map` flag in place of `props::map_ids`'s range, which the player's hashed ID would break. (4) `ActionMap::value` cannot read a World resource and the settings apply before `InputState` exists: the response lives in `InputState`, set at startup and by `apply_settings` (§5). (5) Background cells draw from terrain occupancy, so per-platform streams alone leak: background seeds per cell, and the locality test runs on an indoor platform (§7). (6) Actor-derived icons cross family lines: derived outputs belong to the drawing family, with declared dependencies and a patched-actor test (§8).
- **Round 2** is owed, since round 1 changed mechanisms in §5–§8; the window stood at 79% after round 1, so it runs at the start of session 2, with a focus file naming those sections, before step 5.

## Evidence log

| Step | Verdict | Numbers, paths |
| --- | --- | --- |
