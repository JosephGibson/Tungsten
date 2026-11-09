# Seeded generation (P7-16) — in progress

- **status:** in progress
- **goal:** 7.12's second plan, P7-16 from the redo's 7.12 row and §5.1: room generators that build rooms in memory from a seed and the templates they name, registered and spawned through the loader with IDs from the seed; saves of a generated area as its seed plus its changes, naming the generator's version and each template's, with a reproducible old version, a migration or a refused restore when they changed; `tungsten check` and `tungsten level check` over many seeds headless; a registry `remove` for rooms and maps (A6). With them, 7.12's two open rows that level-format's Step 2 left: a prefab's `scale` (S-129) and an instance's sprite variants (S-58).
- **non-goals:** moving a named entity out of its room's area (a room is removed only once none lives), a save-wide rename map for migrations, and the kit's tile statuses outliving a room spawned again at its origin within a step (§2.1's known limit); authoring-time generation (generated objects that carry their generator in a room file, a hand edit detaching one) and the editor, which are `level-editor`'s (Q1); leaving a room mid-run with its record kept, for streamed worlds (Q7); generators in `describe` (Q6); a CLI verb that renders a generated seed (Q9); 01's level on rooms (7.13); the `tungsten level apply <room> -` defect (`docs/known-issues.md`); a data-only generator format.
- **files to touch:** Step 1: `crates/tungsten-core/src/assets/{tilemap.rs,room/spawn.rs}`, `crates/tungsten-core/src/save/area.rs` (`AreaError::Held`), `crates/tungsten-core/src/tests/assets/{tilemap.rs,room.rs}`, `api/tungsten-core.txt`. Step 2: `crates/tungsten-core/src/assets/room/{generate.rs (new),mod.rs,spawn.rs,patch.rs}`, `crates/tungsten-core/src/assets/mod.rs`, `crates/tungsten-core/src/lib.rs`, `crates/tungsten-core/src/tests/assets/room_generate.rs` (new), `crates/tungsten/src/{app.rs,asset_loader/room.rs}`, `crates/tungsten/src/tests/rooms.rs`, `api/tungsten-core.txt`, `api/tungsten.txt`. Step 3: `crates/tungsten-core/src/save/{world.rs,area.rs}`, `crates/tungsten-core/src/save.rs`, `crates/tungsten-core/src/assets/room/generate.rs`, `crates/tungsten-core/src/tests/save/world.rs`, `crates/tungsten-core/src/tests/save/generated_v1.json` (new), `api/tungsten-core.txt`. Step 4: `crates/tungsten/src/{app.rs,asset_loader/room.rs}`, `crates/tungsten/src/tests/{app.rs,rooms.rs}`, `tools/cli/src/{args.rs,check.rs,level.rs,main.rs,package.rs}`, `tools/cli/src/tests/{args.rs,check.rs,level.rs}`, `api/tungsten-cli.txt`, `docs/getting-started.md`. Step 5: `crates/tungsten-core/src/script.rs`, `crates/tungsten-core/src/tests/script.rs`, `crates/tungsten/src/script.rs`, `crates/tungsten/src/tests/script.rs`, `api/tungsten-core.txt`, `docs/getting-started.md`. Step 6: `crates/tungsten-core/src/{assets/prefab.rs,assets/scene.rs,assets/room/rules.rs,components.rs,ecs/world.rs,save/id.rs,save/world.rs}`, `crates/tungsten-core/src/tests/{assets/prefab.rs,assets/room_generate.rs,assets/room_rules.rs,save/id.rs,save/world.rs}`, `crates/tungsten/src/asset_loader/{check.rs,mod.rs}`, `crates/tungsten/src/tests/{asset_loader.rs,check.rs}`, `docs/assets.md`, `api/tungsten-core.txt`. Step 7: `examples/02_bench/src/{generate.rs (new),main.rs}`, `examples/02_bench/src/tests/{guards.rs,knobs.rs}`, `examples/02_bench/src/tests/guards/generate.json` (new), `scripts/perf-rows.json`, `docs/perf/benchmarks.md`. Step 8: `DECISIONS.md`, `docs/DECISION_INDEX.md`, `DESIGN.md`, `docs/known-issues.md`. Every step: this plan's evidence log. Step 9: this plan (deleted), `ROADMAP.md`, `docs/plans/phase-7.md`, `docs/known-issues.md`.
- **ordered steps:** (1) remove a room or a map; (2) generators; (3) saves as the seed plus changes; (4) check over seeds; (5) the script's `generate` step; (6) a prefab's scale and sprite variants; (7) 02's `generate` row and guard; (8) records; (9) close.
- **done-when:** every step's done-when passes, quoted in the evidence log; the tests pass and the demo (§3 Step 9) works; the plan deleted with its citations and the item out of Now.

Item `level-generation` in [ROADMAP.md](../../ROADMAP.md), from [phase 7](phase-7.md)'s Step 12 and the [roadmap redo](roadmap-redo.md)'s 7.12 row and §5.1. Written 2026-11-09 at `1200a98b`. Runs under the [tungsten-plan skill](../../.claude/skills/tungsten-plan/SKILL.md) and the [operating rules](../operating-rules.md); this plan records only where it differs.

## Context digest

Level-format (`D-264`) gave rooms: one JSON file each, compiled by `Room::compile` to 7.8's runtime terrain, registered by `World::insert_room` in a `RoomRegistry` and the `TilemapRegistry`, spawned once at a time by `World::spawn_room` with each object named `<room>.<id>` in area `<room>`, so its save ID is `SaveId::authored` of that name (`D-222`, `D-230`). A save already writes a named entity as changes over its fresh spawn, so a room that comes back the same from a seed saves as that seed plus its changes, once the save names the seed and what made the room. Nothing removes a room or a map: the paper design's SF22 cloned eight maps a run and kept them all (P7-16's "No map removal"), and its SF27 rebuilt rooms from a seed by hand. The check over seeds is the paper design's first weekly task ("it must generate cleanly in 500 seeds").

Seams it stands on: the content check runs in the game's own build behind `TUNGSTEN_CHECK` (`D-182`), after the plugins build, so generators a plugin registers are there; the level rules (`World::room_findings`, the kit's `route_findings`) read any registered room; the commands (`Room::apply`) are what a generator edits a template copy with. Core has `Pcg32` (`rng.rs:17`); `KitRng` is the kit's and core may not use it.

Level-format's Step 2 stopped on `ecs/world.rs`'s flush naming, outside its list; Step 6 here lists it and carries level-format's §2.9 design for `scale` and `variants` unchanged. Later plans expect: `level-editor` builds on the same commands and takes Q1; 7.13 moves 01's level onto rooms.

## 1. Audit

Read at `1200a98b`.

| # | Finding | Evidence |
| --- | --- | --- |
| A1 | No registry removes anything: `TilemapRegistry` has `insert`, `revert`, `get`, `ids`; `RoomRegistry` has no removal; A6's check prints nothing | `crates/tungsten-core/src/assets/tilemap.rs:1373`, `:1391`; `assets/room/spawn.rs:32`; `rg -n 'pub fn remove' crates/tungsten-core/src/assets/` → exit 1 |
| A2 | A map's readers skip an ID the registry lacks: the physics gather rebuilds without it, the extract warns once and draws nothing, the kit's tile statuses keep only live placements | `crates/tungsten-core/src/physics/step/gather.rs:1221`, `:1360`; `crates/tungsten/src/tilemap_extract.rs:151`; `crates/tungsten-kit/src/chemistry/tiles.rs:640` |
| A3 | A map's history fingerprint is its content's (`MapHistory::start`); a cursor over equal content reads `Unchanged`, other content `Replaced` | `crates/tungsten-core/src/assets/terrain.rs:139`, `:547` |
| A4 | `spawn_room` refuses while the room's last instance lives (`AreaError::Live`); its objects are not the instance's children; `insert_room` refuses a tilemap's ID that no room registered | `assets/room/spawn.rs:326`, `:345`, `:284` |
| A5 | A loaded area stays loaded: `SaveState.loaded` grows at each spawn and nothing takes an area out; a capture writes every named entity whose entity is dead as `gone` in its area | `crates/tungsten-core/src/save/id.rs:92`; `save/area.rs:300`; `save/world.rs:251` |
| A6 | Names are global and a dead entity's name is free (`check_area_names`); a run's spawn takes the run's next ID at the flush (`saved_spawn`), a replacement its source's (`replaced_identity`) | `save/area.rs:253`; `save/id.rs:307`, `:324`; `crates/tungsten-core/src/ecs/world.rs:226`, `:245` |
| A7 | `AreaSave` holds `named`, `gone`, `spawns`, `tiles`; a restore plans each loaded area, keeps the rest (migrated by component versions) for `enter_area`; it checks everything before changing anything | `save/world.rs:75`, `:484`, `:524`, `:587`, `:861` |
| A8 | The content check loads files in a fixed order, rooms at 9, their objects and tileset sprites at 10, level rules at 10b; `run_check` writes `<file>\t<message>` lines; describe calls `check_files` too | `crates/tungsten/src/asset_loader/check.rs:47`, `:204`, `:246`; `crates/tungsten/src/app.rs:1210`; `crates/tungsten/src/describe.rs:41` |
| A9 | The level rules take any registered room by ID: core's spawns, supports, links and budgets, then the kit's reach | `assets/room/rules.rs:475`; `crates/tungsten-kit/src/motion/reach.rs:54`; `crates/tungsten/src/asset_loader/room.rs:122` |
| A10 | The CLI's content stage sets `TUNGSTEN_CHECK` alone; `check`, `level check` and `package` call it; `Command::Check` carries nothing; the report parser reads every line as a finding | `tools/cli/src/check.rs:97`, `:148`; `tools/cli/src/level.rs:719`; `tools/cli/src/package.rs:85`; `tools/cli/src/args.rs:263` |
| A11 | A manifest reload warns `room '<id>' removed — keeping stale` for every registered room the manifest lacks | `crates/tungsten/src/asset_loader/room.rs:250` |
| A12 | The World side of a batch builds its `RoomContext` from a private `ContextParts` (tileset, physics layers, prefab IDs) | `assets/room/patch.rs:600` |
| A13 | Room commands and room entries are `#[non_exhaustive]`: game code builds commands with `RoomCommand::parse_batch` and edits a room's `pub` fields or clones entries | `assets/room/command.rs:41`, `:225`; `assets/room/mod.rs:205` |
| A14 | The script's `room` verb spawns a `rooms` ID at the origin under the stage and does not repeat | `crates/tungsten-core/src/script.rs:19`; `crates/tungsten/src/script.rs:784` |
| A15 | A duplicate script probe panics naming itself; that is the registry convention a generator ID follows | `crates/tungsten-core/src/script.rs:1539` |
| A16 | `PrefabData` has no scale (the spawn's `Transform` has scale 1); `SceneSprite` has no variants and one literal outside core | `crates/tungsten-core/src/assets/prefab.rs:77`, `:582`; `assets/scene.rs:265`; `crates/tungsten/src/tests/asset_loader.rs:223` |
| A17 | The save oracle (7.21) captures and restores seeded worlds under `save::oracle::` | `crates/tungsten-core/src/save.rs:522` |
| A18 | 02 holds fourteen rows (`every_registry_row_resolves_to_its_row`); its bench module is `r#gen`, so a new module avoids the `gen` keyword | `examples/02_bench/src/tests/knobs.rs:84`; `examples/02_bench/src/rooms.rs:42` |
| A19 | `docs/LLM_INDEX.md` sits at 12,264 of its 12,288 bytes and `templates/basic/AGENTS.md` at 12,281; the index's rooms row names `core/assets/room/`, which covers a new file there | `wc -c`; `docs/LLM_INDEX.md:41` |

## 2. Design

### 2.1 Removal (Step 1, A6)

- `TilemapRegistry::remove(&mut self, id) -> Option<TilemapData>` drops the map, its copy as inserted, its history and every path naming it; `clock` stays. Readers already skip a missing ID (A2), and a map inserted again under the ID reads by its content's fingerprint (A3).
- `World::remove_room(&mut self, id) -> Result<Room, AreaError>`: `AreaError::UnknownRoom` for a room the registry lacks; `AreaError::Live` while its last spawned instance lives (A4's rule); `AreaError::Held { room, name }` while any entity named in its area lives (the first by name), since a named entity saves only as changes over the spawn its room makes (`save/world.rs:254`, `save/id.rs:278`) and would have no room to come back from. Otherwise it removes the room's entry (document, compiled form, path, live record, generation), its map from the `TilemapRegistry`, its `RoomJournal` batches (`clear`), and its area from the `SaveState`: `loaded`, `kept` and every named entry in the area. Returns the document. Nothing despawns: the game despawns the room's entities first. References waiting on a forgotten kept area stay waiting, as references into an area never entered do.
- A removed manifest room is no room until a manifest reload loads it again.
- Known limit, not changed here: the kit's tile statuses key a cell by its map's ID and origin and drop those of gone placements at its next chemistry step (`crates/tungsten-kit/src/chemistry/tiles.rs:640`), so a room despawned and spawned again at its origin before that step keeps the old run's statuses. It holds today for any room or area spawned again so; a removal and a same-prefix generation in one frame reach it too. Step 8 records it in known issues with its fix, an epoch the registry gives a map at each insert or revert that the kit's cells carry.

### 2.2 Generators (Step 2)

A new `assets/room/generate.rs`, re-exported beside the room types.

```rust
pub trait RoomGenerator: 'static {
    /// The version the same seed gives the same rooms at; bump it when it would not.
    fn version(&self) -> u32;
    /// Whether it can still generate at `version`; by default only its own.
    fn reproduces(&self, version: u32) -> bool { version == self.version() }
    /// Adds the rooms `generation`'s seed gives at `generation.version()`.
    fn generate(&self, generation: &mut Generate<'_>) -> Result<(), String>;
    /// Rewrites a saved area made under `from` for its room's generation `now`
    /// (`None`: an authored room, or none); false refuses the restore (§2.3).
    fn migrate(&self, from: &Generation, now: Option<&Generation>, area: &mut Value) -> bool { false }
}
```

- `World::register_generator(id, generator)` holds it in a `RoomGenerators` resource by ID (`[a-z][a-z0-9_]*`); a second under one ID panics naming it (A15). `App::register_generator` is the plugin's one-line form, as `App::register_saved` is.
- `World::generate_rooms(generator, seed, prefix) -> Result<GeneratedRooms, GenerateError>`, prepared whole so an error registers nothing: the generator is found (else `UnknownGenerator`); `generate` runs with a `Generate` context, a template it asks for that is not a registered room, or is a generated one, failing it (`GenerateError::Template`); a generation that adds no room fails (`Empty`), so every generation registers a room that carries its record; each room it added gets the ID `<prefix>_<local>`, refused when the `RoomRegistry` or `TilemapRegistry` holds it (`Taken`), so a run's rooms must be removed before its prefix generates again; each compiles against its tileset and the World's physics layers as `insert_room` compiles (`Room { room, source: RoomError }`); the object names `<room>.<id>` across the generation must take distinct `SaveId::authored` values (`Clash`, naming both); then each registers as `insert_room` registers it, with its `Generation`. A generator's `Err` is `GenerateError::Failed { generator, seed, message }`. Returns each room's ID and origin, in the order added.
- `Generate<'_>` borrows the World immutably: `seed()`, `version()` (the version to generate at), `prefix()`, `rng()` (a `Pcg32::seeded(seed)`, A1's core generator), `template(id)` (a clone of a registered room's document, recorded with its hash; an unknown or generated room is an `Err`), `id(local)` (`<prefix>_<local>`, for a link's `to`), `apply(local, &mut room, &[RoomCommand])` (`Room::apply` against the room's tileset, the physics layers and the prefab IDs, through `patch.rs`'s `ContextParts` made `pub(super)`, A12), and `add(local, room, origin)` (`local` an ID, unique in the generation; `origin` in pixels, where the game spawns it). A generator copies a template, edits it by commands or its `pub` fields (A13) and adds it; links between its rooms name `id(local)`.
- `Generation` (serialized, `#[non_exhaustive]`): `generator`, `version`, `seed` (`u64`), `prefix`, `templates` (each template the generation read, by ID, to the 64-bit FNV-1a of its `to_canonical()` text as 16 hex digits, sorted). `RoomRegistry::generation(id)` reads a room's; a room `insert_room` registers has none.
- IDs from the seed: the same generator, version, templates, seed and prefix give the same room IDs, documents and object names, so the same save IDs. The seed is not in the ID (Q5); the `Generation` record is what a restore compares.
- A manifest reload skips its "removed — keeping stale" warning for a generated room (A11). A tileset reload patches a generated room as any room on it; a template's reload changes the next generation's hash and leaves rooms already generated as they are.

### 2.3 Saves as the seed plus changes (Step 3)

- `AreaSave` gains `generated: Option<Generation>`, read by default and written only when present, so a save with no generated area keeps its bytes (02's `saves` row). A capture writes it for each loaded area whose room has a generation; a kept area carries its own.
- `WorldSave::generations()` lists each generated area with its record. `save::regenerate(world, &save) -> Result<Vec<GeneratedRooms>, GenerateError>` registers every generation the save names, one call per distinct generator, seed and prefix, prepared whole: at the saved version when the generator `reproduces` it and every saved template hash matches the template now, else at the current version; a generation whose rooms the World already holds under an equal record is skipped, under another one is `Taken`; an unregistered generator is `UnknownGenerator`. A game restores a generated run by `regenerate`, `spawn_room` for the rooms it shows, then `restore_world`, as for any area.
- `restore_world` matches every saved area that names a generation, loaded or not, before anything changes, so nothing is left to check at `enter_area`: `now` is the generation of the World's room of the area's ID when one is registered (`None` for an authored one); with no such room, it is the World's generation of the same generator, seed and prefix, the room one that generation no longer makes, or `None` when the World holds none, as when the game did not `regenerate` it. Equal to the saved record, the area is planned (loaded) or kept (not) as now. Otherwise the record's components are first migrated to the registry's versions, as `keep_area` migrates a kept area's (`save/world.rs:861`); then the saved generator's `migrate(from, now, &mut area)` runs on that JSON without its `generated` field; true plans or keeps the rewritten record under `now` (an ordinary area's record with `None`; one that no longer reads is a `RestoreError`); false, or a generator the World lacks, refuses the restore with `RestoreError::Generation { area, saved, now }` and nothing changed (Q3). So a loaded and a kept copy of one area reach the migration as the same JSON, and a refused save never leaves its saved resources (the kit's tile statuses among them) over other rooms. A kept area's room cannot change before its entry, since replacing it takes `remove_room`, which forgets the kept record (§2.1). An area whose save names no generation restores as before, generated room or not, since names decide (`D-230`).
- A migration rewrites values and drops records; it neither renames nor moves terrain. It sees the area's record alone, while saved resources load as saved (the kit's tile statuses by map cell, `crates/tungsten-kit/src/chemistry/tiles.rs:149`), so a version whose rooms move terrain cells or object IDs refuses its old saves rather than migrating them; the trait's doc says so. A record naming an object the new generation does not make drops with a report line, as an unknown named record does, and references to a renamed object would drop or wait (`save/world.rs:1144`); a save-wide rename map is a Later line.
- The test save from before a change: `tests/save/generated_v1.json`, captured once from a generator at version 1 over a template at a known hash, committed; the tests load it under the generator at version 2 (refused, then reproduced, then migrated) and under a changed template (refused).

### 2.4 Check over seeds (Step 4)

- `asset_loader/room.rs` gains `check_generators(world, seeds, error, note)`: each registered generator, sorted, at each seed of the range: `generate_rooms(id, seed, id)` (an error is a finding), then each room's objects against the prefabs (`check_map_objects`), core's level rules (`room_findings(id, &["light"])`) and, with the kit, `route_findings`; then `remove_room` on each, which none spawned. A finding's file is `generator <id>`; equal messages merge into one line naming the seeds, the first five then the count (`seeds 3, 17, 40, 41, 66 and 12 more: room `chain_r2`: objects[1] (`crate_3`): ...`). One note a generator: ``generator `chain` (version 1): seeds 0+500, 1,500 rooms``.
- `App::run_check` runs it after `check_files`, only when the World holds a generator, so a game without one checks and prints exactly as before; `describe` stays on `check_files` alone (A8). The seeds come from `TUNGSTEN_CHECK_SEEDS`, `<first>+<count>`, the seeds `first.wrapping_add(i)` for `i < count`; unset, `0+16` (Q2). Notes go into the report as `#\t<note>` lines.
- The CLI: `tungsten check` and `tungsten level check [<room>...]` take `--seeds <count>` (from 0) or `--seed <seed>` (one); `content_stage` sets the variable from them, and `package` passes none. `parse_report` reads `#\t` lines as notes; `check` and `level check` print the notes before the findings, and with `--seeds` or `--seed` given and no note back, `no room generator registered`, so a sweep that did not run never reads as a clean one. The engine reports an unreadable `TUNGSTEN_CHECK_SEEDS` as one finding with no file. `level check` prints the named rooms' findings (every room's with none named) and every generator's. `api/tungsten-cli.txt` from `just api`.

### 2.5 The script's `generate` step (Step 5)

`{"generator": id, "seed": n, "prefix"?: id}`: `generate_rooms` with the generator's ID as the default prefix, then each room spawned at its origin under the stage, as `room` stages one (A14); it does not repeat, and an error fails the script with its text. The `place` probe reads a generated room's places as any live room's. The verb table in `script.rs`'s doc and `docs/getting-started.md` gain the row.

### 2.6 A prefab's scale and sprite variants (Step 6, S-129, S-58)

Level-format's §2.9, unchanged: a prefab's `scale`, `[x, y]` each finite and above 0, default `[1, 1]`, is the spawn's `Transform.scale`. `sprite.variants` lists sprite IDs, checked as `asset_id` is; the pool is the sprite and its variants, held in a `SpriteVariants` component only on a spawn whose prefab has some. A spawn shows one, picked by its save identity when it has one (the ID's FNV hash modulo the pool) and else by its spawn position's whole pixels; `save::name`, the flush's naming (`derive_from`, `replace_from`, A6) and a restore pick again for an entity holding `SpriteVariants`, so a named or saved spawn shows the same variant after a load with nothing saved. A room's object, generated ones included, is named before it is drawn, so a seed shows the same variants each time. The support rule's drawn box (`rules.rs:450`, today the base sprite's size and anchor unscaled) takes the sprite the object's name picks and its size and anchor times the prefab's `scale`, as the extract draws them (`crates/tungsten/src/sprite_extract.rs:822`), so the check over seeds judges what a run shows. The pick writes `Sprite.asset_id` in place: no archetype move and no allocation, so 01, whose prefabs name no variants, pays one `has` check a derived spawn.

### 2.7 02's `generate` row (Step 7)

`examples/02_bench/src/generate.rs` (not `gen`, A18), on `rooms.rs`'s shape: a template room built in code and a generator registered in code that adds `rooms` rooms (knob, default 8, SF22's chain) side by side, each the template with `objects` circles (knob, default 24) at cells from its `rng()`. Each frame despawns the last run's instances and objects, removes its rooms, generates the next seed's and spawns them, so a frame pays a run's whole churn; runs alternate two prefixes, so a removal that left a run's areas behind would show. The art layers draw through the engine's tilemap extract and the bodies through the lean view. The guard pins its counters at `min`, and a guard test holds, after the guard's frames, the registries at the template plus the live run's rooms and `SaveState` loading and keeping none of the other prefix's areas: the leak P7-16 names, which `Taken` would otherwise turn into a failed frame.

## 3. Steps

Every code step's done-when includes `cargo fmt --all -- --check` → exit 0 and `just lint` → exit 0; a step that moves a public item includes `just api` then `just api-check` → exit 0. Steps 1–3 and 6, which change core's spawn, registry and save paths (preflight's P1, P2), also include the pins: `cargo test -p example-02-bench --locked guards_` → exit 0 (02's fourteen guards); `cargo test -p example-01-platformer --locked --test game guards::the_full_pit_holds_its_counts` → exit 0. Steps 2 and 4, which change `app.rs` (P3), include `cargo test -p tungsten-template-basic --locked --test game the_schedule_matches_the_snapshot` → exit 0.

### Step 1: Remove a room or a map

- **Files:** as listed.
- **Change:** §2.1.
- **Done-when:** `cargo test -p tungsten-core --lib --locked -- assets::tilemap:: assets::room::tests::` → exit 0 with `remove_drops_a_map_its_copy_as_inserted_and_its_paths`, `remove_of_an_unknown_map_is_none`, `remove_room_refuses_a_live_room`, `remove_room_refuses_while_an_entity_named_in_it_lives`, `remove_room_drops_the_room_its_map_and_its_journal`, `remove_room_forgets_its_areas_names` and `a_removed_room_inserts_and_spawns_again`; `rg -n 'pub fn remove' crates/tungsten-core/src/assets/` → `tilemap.rs`'s `remove` and `room/spawn.rs`'s `remove_room`.
- **Commit:** `feat(assets): remove a room or a map from its registry`
- **Moves:** `api/tungsten-core.txt`.

### Step 2: Generators

- **Files:** as listed.
- **Change:** §2.2.
- **Done-when:** `cargo test -p tungsten-core --lib --locked assets::room::generate::` → exit 0 with `a_seed_gives_the_same_rooms_and_names_each_time`, `generate_rooms_registers_each_room_with_its_generation`, `a_generation_records_each_template_it_read`, `a_template_that_is_not_registered_fails_the_generation`, `a_generation_that_adds_no_room_fails`, `a_room_that_does_not_compile_registers_nothing`, `an_id_a_room_or_map_holds_is_taken`, `two_names_sharing_a_save_id_fail_the_generation` (a colliding pair the test names, found once by a birthday search), `a_generators_error_names_it_and_its_seed`, `generated_rooms_link_by_their_ids` and `a_second_generator_under_one_id_panics`; `cargo test -p tungsten --lib --locked asset_loader::room::tests::` → exit 0 with `app_register_generator_reaches_the_world` and `a_manifest_reload_keeps_generated_rooms_without_a_warning`.
- **Commit:** `feat(assets): generate rooms from a seed`
- **Moves:** `api/tungsten-core.txt`, `api/tungsten.txt`.

### Step 3: Saves as the seed plus changes

- **Files:** as listed.
- **Change:** §2.3; the fixture save written by a one-off run of the step's own test helper, then committed.
- **Done-when:** `cargo test -p tungsten-core --lib --locked save::` → exit 0, with `a_capture_writes_each_generated_areas_generation`, `a_generated_run_restores_from_its_seed_and_its_changes`, `regenerate_registers_the_saves_generations_and_nothing_on_an_error`, `a_save_from_before_a_generator_change_is_refused`, `a_generator_that_reproduces_its_old_version_keeps_the_changes`, `a_generators_migration_rewrites_the_saved_changes`, `a_template_change_is_refused_without_a_migration`, `a_room_the_new_version_no_longer_makes_reaches_the_migration`, `a_kept_and_a_loaded_copy_of_one_area_migrate_alike`, `a_migration_takes_the_matched_rooms_generation`, `a_save_naming_a_generation_the_world_did_not_regenerate_is_refused`, `a_kept_generated_area_enters_with_its_matched_record` and `an_area_with_no_generation_restores_as_before` among them, and `save::oracle::` passing; 02's `guards_saves` among the pins (its bytes unchanged).
- **Commit:** `feat(save): save generated rooms as their seed and changes`
- **Moves:** `api/tungsten-core.txt`.

### Step 4: Check over seeds

- **Files:** as listed.
- **Change:** §2.4.
- **Done-when:** `cargo test -p tungsten --lib --locked -- asset_loader::room::tests:: app::tests::run_check` → exit 0 with `check_generators_sweeps_every_generator_over_its_seeds`, `a_finding_at_many_seeds_is_one_line_naming_them`, `a_generators_error_is_a_finding_at_its_seed`, `the_sweep_leaves_no_generated_room_registered`, `run_check_reads_the_seeds_variable` and `run_check_without_a_generator_writes_what_it_wrote_before`; `cargo test -p tungsten-cli --locked` → exit 0 with `check_takes_seeds_or_one_seed`, `level_check_takes_seeds_or_one_seed`, `a_report_reads_notes_apart_from_findings`, `level_check_prints_every_generators_findings` and `seeds_with_no_note_say_no_generator_ran`; `just api-check` → exit 0.
- **Commit:** `feat(cli): check room generators over many seeds`
- **Moves:** `api/tungsten-cli.txt`.

### Step 5: The script's `generate` step

- **Files:** as listed.
- **Change:** §2.5.
- **Done-when:** `cargo test -p tungsten-core --lib --locked script::` → exit 0 with `a_generate_step_reads_and_refuses_a_repeat`; `cargo test -p tungsten --lib --locked script::` → exit 0 with `a_generate_step_spawns_each_room_at_its_origin` and `a_generate_step_that_fails_fails_the_script`; `just script-test` → exit 0.
- **Commit:** `feat(script): add the generate step`
- **Moves:** `api/tungsten-core.txt`.

### Step 6: A prefab's scale and sprite variants

- **Files:** as listed.
- **Change:** §2.6; the umbrella test's `SceneSprite` literal gains the field; `docs/assets.md`'s prefab row.
- **Done-when:** `cargo test -p tungsten-core --lib --locked -- assets::prefab:: assets::room::generate:: assets::room::rules:: save::id:: save::world::` → exit 0 with `a_prefabs_scale_sets_the_spawns_transform`, `a_scaled_prop_is_judged_by_its_drawn_box`, `a_props_variant_is_judged_by_its_own_sprite`, `a_scale_not_above_zero_fails_the_load`, `a_prefabs_variant_comes_from_its_save_id`, `an_unnamed_spawn_picks_by_its_position`, `an_unknown_variant_fails_the_load`, `a_seed_shows_the_same_variants_each_time`, `naming_an_entity_picks_its_variant_again` and `a_restored_spawn_shows_its_variant_in_a_churned_world`; `cargo test -p tungsten --lib --locked asset_loader::` → exit 0 with `a_prefab_variant_naming_no_sprite_is_a_check_error`; `cargo test -p tungsten-core --lib --locked ecs::command_buffer::` → exit 0 (the `Command` size pin, preflight's R4); `cargo test -p tungsten --lib --locked -- views_guard_ each_owner_registers_its_scene_components` → exit 0 (the umbrella's view guards, P2, and App's scene names, P7, none moved).
- **Commit:** `feat(assets): add a prefab scale and sprite variants`
- **Moves:** `api/tungsten-core.txt`; no save bytes (variants save nothing).

### Step 7: 02's `generate` row and guard

- **Files:** as listed.
- **Change:** §2.7; the row-count pin 14 → 15, forced by the row.
- **Done-when:** `cargo test -p example-02-bench --locked` → exit 0 with `guards_generate` and `the_generate_row_keeps_only_the_live_runs_rooms_and_areas`; `just perf-test` → exit 0; `just perf run generate --repeat 1` → valid, its digest in the evidence log, run with the tree still.
- **Commit:** `test(bench): add the generate row and its guard`
- **Moves:** 02's row count; the new `generate.json` guard.

### Step 8: Records

- **Files:** as listed.
- **Change:** through the tungsten-decision skill, `D-26[6]` (next free at the plan's commit; recheck before writing) for §2.1–§2.7, amending `D-182` (the check's seeds and notes), `D-225` (the `generate` step), `D-230` (a generated area's record, `regenerate`, the forgotten areas) and `D-264` (removal, generators, the open `scale` and `variants` closed), each with its marker line and index row; DESIGN.md's rooms paragraph gains generation and removal; known issues gain §2.1's known limit with its fix and what the steps found.
- **Done-when:** `rg -c '^## D-26[6] — ' DECISIONS.md` → 1; ``rg -c 'Amended by `D-26[6]`' DECISIONS.md`` → 4; `just repo-check` → exit 0 (it holds each entry to its index row).
- **Commit:** `docs(decisions): record seeded generation and room removal`
- **Moves:** none.

### Step 9: Close

- **Files:** this plan (deleted), `ROADMAP.md`, `docs/plans/phase-7.md`, `docs/known-issues.md`.
- **Change:** the checks and the demo; phase 7's P7-16, S-58 and S-129 rows `landed` with their commits and Step 12's part-2 note; follow-ups to known issues or Later (Q6, Q7, Q9, moving a named entity between areas and a save-wide rename map as Later lines); the plan and its citations deleted; `level-generation` out of Now and phase 7's line naming `level-editor` next.
- **Done-when:** `just check` → exit 0; `just physics-release` → 16 passed, the hashes unchanged; `just api-check`, `just docs-ratchet`, `just perf-test`, `just script-test`, `just ctx`, `just repo-check` and `git diff --check` → exit 0; `just smoke` → every section passed; `just visual` → exit 0, no golden moved; `just template-check` → exit 0, its output as before (no generator in the template); the demo: in a `tungsten new` copy outside the repository pinned to this checkout, with the rooms fixture's tileset, sprites, prefabs, physics layer names and `hall` as a template, and a `chain` generator in its `src/` (three rooms from `hall` in a row, doors linked by `id`, crates at seeded cells) registered in its plugin: `tungsten level check --seeds 500` → the note ``generator `chain` (version 1): seeds 0+500, 1,500 rooms`` and `no room findings`; the generator made to float a crate at one seed in ten → one finding line naming those seeds, then put back; `tungsten run --script` (an empty stage, `generate` `chain` at seed 7, `place` asserted on `chain_r1.door_east`, a capture) → passed; phase 7's row check (`rg -n '^\| (P7-16|S-58|S-129) \| [^|]+ \| 7\.12 \| open \|' docs/plans/phase-7.md; test $? -eq 1` → exit 0).
- **Ends with:** the commits, the evidence, what the owner should try, the next prompt (`level-editor`'s plan).

## 4. Open questions

| # | Question | Default |
| --- | --- | --- |
| Q1 | §5.1's authoring-time output (generated objects carrying their generator in a room file; a hand edit detaching one) | `level-editor`'s: it is a command front-end on the editor's commands; this plan's generators build rooms in memory only |
| Q2 | Seeds a plain `tungsten check` or `package` sweeps | 16 (`0+16`); `--seeds` and `--seed` set it |
| Q3 | A generated area whose generation changed with no migration | the restore refused (`RestoreError::Generation`), nothing changed: a dropped area would leave the save's resources, the kit's tile statuses among them, over rooms they no longer fit; the game decides what an old run becomes |
| Q4 | A template's version | the FNV-1a of its canonical form: any edit counts, since it may change what a seed draws |
| Q5 | A generated room's ID | `<prefix>_<local>`, the caller's prefix (the generator's ID in the check and the script); no seed in it, the `Generation` record compared instead |
| Q6 | Generators in `tungsten describe` | not here (`describe` stays at version 3); the check's notes name them; a Later line |
| Q7 | A room left mid-run with its record kept, for streamed worlds | a Later line: a run's rooms stay spawned together, as SF22's are |
| Q8 | Step 6 stops again | S-58 and S-129 move to `level-editor` with the stop's reason |
| Q9 | Rendering a generated seed from the command line | a script with `generate` and `capture`; no `level render` option |
| Q10 | Holding a migration to rooms that keep their terrain cells and object IDs (round 2's finding 2) | the trait's doc states it and Step 3's migration test keeps both; no check, since the restore cannot read what the kit's saved tile statuses mean |

## 5. Invariants

- **May move:** `api/tungsten-core.txt` (Steps 1–3, 5, 6), `api/tungsten.txt` (Step 2), `api/tungsten-cli.txt` (Step 4); 02's row count 14 → 15 and its new `generate` guard (Step 7), each new value in the evidence log.
- **Must not move:** the physics hashes; the kit's determinism pin; 01's `pit` pin; 02's fourteen pins, the `saves` row's bytes included; `describe`'s version 3; a game without a generator checks, describes and packages with the same output (the template check's seeded errors and their summary); the save oracle's default run.

## 6. Stop conditions

- A done-when check fails: restore the step's files from the last commit, mark the step skipped, and continue with steps that don't depend on it. Steps 3–5 and 7 depend on Step 2; Step 7 on Step 1.
- A file outside the step's list: a pinned name, message or count the step's own change forces joins the step with its reason in the evidence log; any other reach stops the step.
- A pin in §5's must-not-move list moves: the step stops.

## 7. Critique

Round 1: gpt-6-astra, 2026-11-09, read the draft at `1200a98b`. `critique: model gpt-6-astra, effort xhigh, context 872000, mode repo (output cap 4000), 501.7 s, tokens in 1243028 (cached 1150720), out 11868 (reasoning 6280), commands 31, output 373.8 KB (largest 41.5 KB), usage 5h 0% → 27%, week 89% → 93%`. Six findings, four high or medium on mechanisms, so a second round.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 1 | High: removing a room while an entity named in it lives leaves that entity unrestorable, since a named entity saves as changes over the spawn its room makes | `save/world.rs:254` (patches over the baseline), `save/id.rs:278` (`Saved` with no prefab) | Accepted | §2.1: `remove_room` refuses with `AreaError::Held` while any entity named in its area lives and then forgets every named entry; Step 1's tests; moving a named entity between areas is a non-goal and a Later line |
| 2 | High: a dropped area leaves the kit's saved tile statuses, keyed by map ID and origin, over a different generation; the same after a removal and a same-prefix generation between chemistry steps | `crates/tungsten-kit/src/chemistry/tiles.rs:149` (`CellSave` by map and origin), `:640` (pruned to live placements at the step), `crates/tungsten-kit/src/chemistry/mod.rs:261`, `save/world.rs:544` (resources restored whole) | Accepted in part | The restore case goes: a changed generation without a migration refuses the restore (§2.3, Q3), so no resource lands over other rooms. The in-World case is older than this plan (any room despawned and spawned again at its origin within a step reaches it) and its fix is the kit's: §2.1 states it, and Step 8 records it in known issues with the fix, an epoch per map insert or revert that the kit's cells carry |
| 3 | Medium: a migration that renames an object leaves references to its old save ID in other areas and resources | `save/id.rs:32` (the ID is the name's hash), `save/world.rs:1144` (unresolved references drop or wait) | Accepted | §2.3: a migration rewrites values and drops records and does not rename; Step 3's migration test rewrites values; a save-wide rename map is a non-goal and a Later line |
| 4 | Medium: an area whose room a new version no longer makes is kept and never reaches the migration or the drop; a room that became authored has no `to` | `save/world.rs:525` (an unloaded area is kept), `:319` (a capture writes kept areas back) | Accepted | §2.3: the restore matches every saved generated area, loaded or not, by its room's ID or by its generator, seed and prefix; `migrate` takes `to: Option<&Generation>`; Step 3 adds `a_room_the_new_version_no_longer_makes_reaches_the_migration` |
| 5 | Medium: a loaded area's migration would see components at their saved versions, a kept area's at the registry's | `save/world.rs:883` (`keep_area` migrates components), `:764` (planning migrates a loaded area's) | Accepted | §2.3: components migrate to the registry's versions first on both paths, then the generator's migration; Step 3 adds `a_kept_and_a_loaded_copy_of_one_area_migrate_alike` |
| 6 | Medium: the support rule measures the base sprite unscaled, so the check over seeds misjudges scaled props and variants | `assets/room/rules.rs:450` (`drawn_box`: base sprite, its size and anchor), `crates/tungsten/src/sprite_extract.rs:822` (size times `Transform.scale`) | Accepted | §2.6: the drawn box takes the variant the object's name picks and the prefab's scale; Step 6 lists `rules.rs` and its tests, `a_scaled_prop_is_judged_by_its_drawn_box`, `a_props_variant_is_judged_by_its_own_sprite` |

Round 2: gpt-6-astra, 2026-11-09, read the plan without §7 at `1200a98b`, with a focus note naming round 1's changes and the rejected part of finding 2. `critique: model gpt-6-astra, effort xhigh, context 872000, mode repo (output cap 4000), 353.3 s, tokens in 506293 (cached 434176), out 9193 (reasoning 5888), commands 19, output 193.7 KB (largest 22.9 KB), usage 5h 27% → 43%, week 93% → 96%`. Five findings; no third round, the open point is Q10.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 2.1 | High: a new version that makes no room leaves neither an ID nor a group to match, so the old areas are kept unchecked | §2.2 as written: the generation record rides only on rooms | Accepted | §2.2: a generation that adds no room fails (`GenerateError::Empty`); Step 2's `a_generation_that_adds_no_room_fails` |
| 2.2 | Medium: a migration sees one area's record while the kit's tile statuses load from a resource by map cell, so a migration that moves terrain leaves statuses on old cells | `save/world.rs:38` (resources apart from areas), `crates/tungsten-kit/src/chemistry/tiles.rs:149`, `:1017` (cells restored as saved) | Accepted as a restriction | §2.3: a migration neither renames nor moves terrain, and a version that does refuses its old saves; the trait's doc says so; Q10 for the unenforced part |
| 2.3 | Medium: `to` came from the saved group even when a room of the area's ID exists under another seed or is authored | §2.3 as written | Accepted | §2.3: `now` is the matched room's generation when the room is registered (`None` for an authored one), the group's only when it is not; `migrate`'s parameter renamed `now`; Step 3's `a_migration_takes_the_matched_rooms_generation` |
| 2.4 | Medium: the check deferred to `enter_area` runs after the restore placed the resources, and a refused entry leaves the area loaded, so a capture drops its kept record | `save/world.rs:996` (resources applied at the restore), `assets/room/spawn.rs:355` (`load_area_map` marks the area loaded), `save/world.rs:320` (kept areas written only while not loaded) | Accepted | §2.3: every saved generated area is matched at `restore_world`, a generation the World did not regenerate reaching `migrate` with `None` and so refused by default; nothing is checked at entry, since a kept area's room changes only through `remove_room`, which forgets the kept record; Step 3's `a_save_naming_a_generation_the_world_did_not_regenerate_is_refused` and `a_kept_generated_area_enters_with_its_matched_record` replace the entry check's test |
| 2.5 | Medium: Step 1 adds `AreaError::Held` without listing `save/area.rs`, where `AreaError` lives | `save/area.rs:182` | Accepted | Step 1's files list `crates/tungsten-core/src/save/area.rs` |

## Evidence log

| Step | Date | Verdict | Key numbers | Paths |
| --- | --- | --- | --- | --- |
| 1 | 2026-11-09 | passed | `cargo test -p tungsten-core --lib --locked -- assets::tilemap:: assets::room::tests::` → exit 0, 52 passed, the seven named among them (a `--list` run selects each); `rg -n 'pub fn remove' crates/tungsten-core/src/assets/` → `room/spawn.rs:403` (`remove_room`) and `tilemap.rs:1405` (`remove`); fmt, `just lint`, `just api` then `just api-check` → exit 0 (`api/tungsten-core.txt` +9: `TilemapRegistry::remove` at its three paths, `World::remove_room` at two, `AreaError::Held` and its two fields); 02's `guards_` → 14 passed; 01's `guards::the_full_pit_holds_its_counts` → 1 passed. Departure: `AreaError::Live`'s message ends "despawn its instance first", since `remove_room` returns it too (no test pinned the old text) | `perf-runs/20261109-level-generation/step1/` |
| 2 | 2026-11-09 | passed | `cargo test -p tungsten-core --lib --locked assets::room::generate::` → exit 0, 11 passed, the eleven named (the colliding pair `clash_a.jnzvero` and `clash_b.xysfyis`, from a search over random object IDs, since numeric suffixes gave none in 200,000 × 200,000); `cargo test -p tungsten --lib --locked asset_loader::room::tests::` → 14 passed with the two named; fmt, `just lint`, `just api` then `just api-check` → exit 0 (`api/tungsten-core.txt` +393, `api/tungsten.txt` +2, none removed); 02's `guards_` → 14 passed; 01's pit → 1 passed; the template's `the_schedule_matches_the_snapshot` → 1 passed; core's and the umbrella's lib suites 1,631 and 1,059 passed. Departures: `GenerateError::Prefix` for a prefix that is no ID; `Generate::apply` returns `Result<(), String>` (the inverse dropped); a refused `template` or a wrong `add` (a local that is no ID or repeats) fails the generation even when the generator drops the error; the reload's warnings come from `stale_warnings`, which the test reads, since `App::new`'s `env_logger` would race a capturing logger in the umbrella's test binary | `perf-runs/20261109-level-generation/step2/` |

## Follow-ups
