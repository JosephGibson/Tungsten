# Tungsten game template

A small game laid out the way a Tungsten game is: a title screen, a player that the arrow keys or WASD move, and a pause.

Run it from this folder, where `tungsten.json`, `input.json` and `assets/` sit:

```bash
cargo run    # a debug build: hot reload and the engine's HUD (F4 toggles it)
cargo test   # the game's tests on the engine's headless harness
```

In a debug build, edits under `assets/` and to `input.json` apply while the game runs. Enter starts, P pauses, Backspace or P resumes and Escape quits.

The engine's [getting-started guide](https://github.com/JosephGibson/Tungsten/blob/main/docs/getting-started.md) shows how to make a game of your own from this template; read it at the engine tag in `Cargo.toml`.
