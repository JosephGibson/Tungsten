# AGENTS.md

Rules for this game's repository; `CLAUDE.md` imports this file.

## Layout

- `tungsten.json`, `input.json` and `assets/` sit in the game's folder, its working directory: run `cargo run` and `cargo test` from there.
- `src/main.rs` builds the app. `src/game.rs` is the game's plugin: its systems by stage, the setup its first frame needs, and its text. `src/states.rs` holds the states, `src/components.rs` the components and `tests/` the tests.

## Rules

- Use the engine's public API, `tungsten::…`, and nothing else from its sources.
- Name assets by their ID in `assets/manifest.json`, never by file path (the engine's `D-009`).
- Read actions through `ActionMap`, never keys; bindings live in `input.json`.
- Draw through `Sprite` entities and tilemaps, which the engine draws by default. Add the game's own sprite batches, quads and text in its plugin's `build`, through `world.resource_mut::<Extracts>()` and `add_sprites`, `add_quads` or `add_text`: they draw after the defaults, in the order added. Replace a default (`replace_sprites`) only on purpose: the engine's `Sprite` entities and tilemaps stop drawing.
- Register each system in `src/game.rs`'s plugin under a name, in a stage: `update` by default; `fixed_update` before `physics_step` for a system that drives bodies or reads input for movement: it runs once per fixed step (1/60 s of game time, zero or more times a frame), so a press moves the body on the next step, `just_pressed` there answers with the presses since the last step, and it reads the events `fixed_update` sends, events from other stages in `update`; `post_update` with `before`/`after` against the engine's names in `tungsten::plugins` for one that runs around the physics sync, game feel or the camera. Systems in one stage run in the order they were added unless a constraint says otherwise; a typo in a constraint fails startup, which a harness test catches. Test each system on the engine's headless harness (`tungsten::testing::Harness`) in `tests/`.
- Read time from the `Time` resource: `delta()`, the game clock (scaled, zero while paused; the step inside `fixed_update`), for movement and animation, and `real_delta()` for anything that runs over a pause. Count cooldowns and lifetimes with `Timer`, ticked with the clock's dt. `DeltaTime` is deprecated.
- Read and write components through one query that names its data, `world.query::<(Entity, &A, Option<&B>)>()` or `query_mut_filtered::<&mut A, With<B>>()`, never a collected entity list and `get` per entity; spawn with `world.spawn_with((A, B, C))`, or `RigidBodyBundle::dynamic(..).with((..))` for a body, never a run of `insert`s; a teleport or respawn writes a body's `Position` and `PrevPosition` together, or it is drawn sliding from the old point; read a resource the engine inserts with `world.resource::<T>()`. The numbered `query2`/`query3_mut` functions are deprecated.
- Before a commit: `cargo test` and `cargo clippy --all-targets -- -D warnings`.

## Engine documentation

- The API: `cargo doc -p tungsten --open`.
- The getting-started guide: `docs/getting-started.md` at the engine tag in `Cargo.toml`, on <https://github.com/JosephGibson/Tungsten>.
