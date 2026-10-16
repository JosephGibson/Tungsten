# Tungsten game template

A small game laid out the way a Tungsten game is: a title screen, a player that the arrow keys or WASD move, and a pause. The player is a prefab of the engine kit's components (`assets/prefabs/player.json`): its movement, health, hit flash and the camera that follows it come from `tungsten::kit`, with no game code for them. The camera holds while the player roams the middle of the view and follows once it leaves; with nothing else in the world, that is what shows the movement.

Run it from this folder, where `tungsten.json`, `input.json` and `assets/` sit:

```bash
cargo run    # a debug build: hot reload and the engine's HUD (F4 toggles it)
cargo test   # the game's tests on the engine's headless harness
```

In a debug build, edits under `assets/` and to `input.json` apply while the game runs. Enter starts, P pauses, Backspace or P resumes and Escape quits.

The engine's [getting-started guide](https://github.com/JosephGibson/Tungsten/blob/main/docs/getting-started.md) shows how to make a game of your own from this template; read it at the engine tag in `Cargo.toml`.
