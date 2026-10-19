# Tungsten game template

The springboard a Tungsten game starts from: a shell of the screens every game needs, and a sample game to replace.

- **The shell** (`src/shell/`): a title with Continue, New Game (through a difficulty picker), Load, Settings and Quit; pause; game over with Retry; a save and load screen over an autosave and three slots; settings in tabs (Display with window mode, size, vsync and a frame-rate limit; Audio; Interface scales; Controls with keybindings; Gameplay options); a confirm dialog; toasts; all in one dark, square theme (`assets/ui/themes/shell.json`), driven by the keyboard, a pad or the mouse. The autosave is written when the game pauses, quits to the title or exits, so closing the window keeps the run.
- **The sample** (`src/sample/`): a player that the arrow keys or WASD move, a hazard that patrols below it and three gems to collect, with a HUD. The player, the hazard and the gems are prefabs of the engine kit's components (`assets/prefabs/`): movement, health, hit flash, knockback, the camera, the hazard's touch and slow, the gems' drift and count come from `tungsten::kit`, with no game Rust for them.
- **The seam**: the shell calls the game through six names in `src/game.rs` and `src/save.rs`, and nothing else.

Run it from this folder, where `tungsten.json`, `input.json` and `assets/` sit:

```bash
cargo run    # a debug build, with hot reload; F4 shows the engine's HUD
cargo test   # the game's tests on the engine's headless harness
```

In the game, the arrow keys or WASD move the player, P or Escape pauses and M mutes. In a debug build, edits under `assets/` and to `input.json` apply while the game runs.

To start a game of your own, copy this folder, delete `src/sample/` and `tests/game/sample.rs`, write your gameplay under `src/features/`, and point the seam at it: `game::GAMEPLAY`, `game::gameplay` and `game::BINDABLE_ACTIONS`, and `save::SaveGame`, `save::capture` and `save::summary`. Leave `src/shell/` as it is. The engine's [getting-started guide](https://github.com/JosephGibson/Tungsten/blob/main/docs/getting-started.md) walks through it; read it at the engine tag in `Cargo.toml`.
