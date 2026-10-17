# Tungsten game template

A small game laid out the way a Tungsten game is: a title screen, a player that the arrow keys or WASD move, a hazard that patrols below it, and a pause. The player is a prefab of the engine kit's components (`assets/prefabs/player.json`): its movement, health, hit flash, knockback, the camera that follows it and a hit's camera shake and floating damage number come from `tungsten::kit`, with no game code for them. The hazard (`assets/prefabs/hazard.json`) swings along its line and wobbles by the kit's oscillator; touching it costs a hit and throws the player back, the one piece of game Rust in `src/game.rs` (`hazard_touch`), and the hit that empties the player's health starts gameplay over. The camera holds while the player roams the middle of the view and follows once it leaves, so the world holds more than the player: the hazard's line shows the movement against it.

Run it from this folder, where `tungsten.json`, `input.json` and `assets/` sit:

```bash
cargo run    # a debug build: hot reload and the engine's HUD (F4 toggles it)
cargo test   # the game's tests on the engine's headless harness
```

In a debug build, edits under `assets/` and to `input.json` apply while the game runs. Enter starts, P pauses, Backspace or P resumes and Escape quits.

The engine's [getting-started guide](https://github.com/JosephGibson/Tungsten/blob/main/docs/getting-started.md) shows how to make a game of your own from this template; read it at the engine tag in `Cargo.toml`.
