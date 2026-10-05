# AGENTS.md

Rules for this game's repository; `CLAUDE.md` imports this file.

## Layout

- `tungsten.json`, `input.json` and `assets/` sit in the game's folder, its working directory: run `cargo run` and `cargo test` from there.
- `src/main.rs` builds the app. `src/game.rs` registers the game's systems, by hand and in the order they run, its text and its setup. `src/states.rs` holds the states, `src/components.rs` the components and `tests/` the tests.

## Rules

- Use the engine's public API, `tungsten::…`, and nothing else from its sources.
- Name assets by their ID in `assets/manifest.json`, never by file path (the engine's `D-009`).
- Read actions through `ActionMap`, never keys; bindings live in `input.json`.
- Register each system in `src/game.rs`, in order, and test it on the engine's headless harness (`tungsten::testing::Harness`) in `tests/`.
- Before a commit: `cargo test` and `cargo clippy --all-targets -- -D warnings`.

## Engine documentation

- The API: `cargo doc -p tungsten --open`.
- The getting-started guide: `docs/getting-started.md` at the engine tag in `Cargo.toml`, on <https://github.com/JosephGibson/Tungsten>.
