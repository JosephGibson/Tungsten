# Tungsten game template

The springboard a Tungsten game starts from: a shell of the screens every game needs, and a sample game to replace.

- **The shell** (`src/shell/`): a title with Continue, New Game (through a difficulty picker), Load, Settings and Quit; pause; a game over that reads "You died" in red, with its own sting and music, and Retry; a save and load screen over an autosave and ten slots; settings in tabs (Display with window mode and the monitor's sizes behind an Apply, vsync and a frame-rate limit; Audio at 80 % as shipped; Interface scales; Controls with keybindings; Gameplay options); a confirm dialog; toasts; all in one dark, square theme (`assets/ui/themes/shell.json`) with a serif title (Source Serif 4), Inter for text and the engine's mono for numbers, menu sounds, and a UI that keeps its layout at any window size and scale, driven by the keyboard, a pad or the mouse. The autosave is written when the game pauses, quits to the title or exits, so closing the window keeps the run; quitting from a menu asks first.
- **The sample** (`src/sample/`): a player that the arrow keys or WASD move, easing in and out, and that shoots bolts at the cursor; four red enemies that move, take three hits and come back, among them a hazard below the player whose touch slows; a target dummy, three gems and two pickups, one healing, one a burst of speed; a HUD and two music loops. The player, the bolt, the enemies, the dummy, the gems and the pickups are prefabs of the engine kit's components (`assets/prefabs/`): movement, aim and the shot, health, hit flash, knockback, the camera, the enemies' motion, touch and slow, the dummy's mend, the gems' drift and count and the pickups' effects come from `tungsten::kit`; the sample's Rust spawns them and brings a fallen enemy back.
- **The seam**: the shell calls the game through seven names in `src/game.rs` and `src/save.rs`, and nothing else.
- **The sounds and sprites** (`assets/sounds/`, `assets/sprites/`): made by `tools/sounds.py` and `tools/sprites.py`, which `tools/README.md` describes, all but M36's `player.png`; each script's `--check` compares the files with a fresh render.

Run it from this folder, where `tungsten.json`, `input.json` and `assets/` sit:

```bash
cargo run    # a debug build, with hot reload; F4 shows the engine's HUD
cargo test   # the game's tests on the engine's headless harness
```

In the game, the arrow keys or WASD move the player, the left mouse button shoots toward the cursor, and P or Escape pauses. In a debug build, edits under `assets/` and to `input.json` apply while the game runs.

To start a game of your own, run `tungsten new <name>`, the engine's command line, which the engine's [getting-started guide](https://github.com/JosephGibson/Tungsten/blob/main/docs/getting-started.md) installs: it copies this folder under the new name, with the engine pinned to its release tag. Then delete `src/sample/` and `tests/game/sample.rs`, write your gameplay under `src/features/`, and point the seam at it: `game::GAMEPLAY`, `game::gameplay`, `game::BINDABLE_ACTIONS` and `game::difficulty_note`, and `save::SaveGame`, `save::capture` and `save::summary`. Leave `src/shell/` as it is. The guide walks through it; read it at the engine tag in `Cargo.toml`.
