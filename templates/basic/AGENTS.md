# AGENTS.md

Rules for this game's repository; `CLAUDE.md` imports this file.

## Layout

- `tungsten.json`, `input.json` and `assets/` sit in the game's folder, its working directory: run `cargo run` and `cargo test` from there.
- `src/main.rs` builds the app. `src/game.rs` is the game's plugin: its systems by stage, the setup its first frame needs, and its text. `src/states.rs` holds the states, `src/components.rs` the components and `tests/` the tests.

## Rules

- Use the engine's public API, `tungsten::…`, and nothing else from its sources.
- Name assets by their ID in `assets/manifest.json`, never by file path (the engine's `D-009`).
- Read actions through `ActionMap`, never keys; bindings live in `input.json`.
- Register each system in `src/game.rs`'s plugin under a name, in a stage: `update` by default; `fixed_update` before `physics_step` for a system that drives bodies or reads input for movement, so a press moves the body on the frame of the press; `post_update` with `before`/`after` against the engine's names in `tungsten::plugins` for one that runs around the physics sync, game feel or the camera. Systems in one stage run in the order they were added unless a constraint says otherwise; a typo in a constraint fails startup, which a harness test catches. Test each system on the engine's headless harness (`tungsten::testing::Harness`) in `tests/`.
- Before a commit: `cargo test` and `cargo clippy --all-targets -- -D warnings`.

## Engine documentation

- The API: `cargo doc -p tungsten --open`.
- The getting-started guide: `docs/getting-started.md` at the engine tag in `Cargo.toml`, on <https://github.com/JosephGibson/Tungsten>.
