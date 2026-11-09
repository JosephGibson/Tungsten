# Getting started

Make a game with Tungsten from its template, `templates/basic`: a shell of the screens every game needs (a title, pause, game over, save and load, tabbed settings and keybindings) around a sample game you replace, a player that the arrow keys or WASD move and that shoots, red enemies and three gems. The game lives in a repository of its own and depends on the engine through a git tag (`D-123`). This is the guide's first draft. The engine's command line, `tungsten`, starts the game, checks its files and packages it for players ([Check and package](#check-and-package)).

## What you need

- Rust through [rustup](https://rustup.rs). The template's `rust-toolchain.toml` pins the toolchain the engine is tested with, and rustup installs it on first use.
- A GPU with a current driver: Vulkan on Linux, DX12 or Vulkan on Windows. Those two are the tested hosts; Metal on macOS is untested ([known issues](known-issues.md)).
- On Linux, `pkg-config`, the ALSA headers and libudev for gamepads (`libasound2-dev` and `libudev-dev` on Debian and Ubuntu).
- Git.
- To package a game, a Linux x86-64 host with GNU `tar` and binutils' `objcopy`.

## Start a game from the template

Pick a release tag from the [releases](https://github.com/JosephGibson/Tungsten/releases), here `vX.Y.Z`, and install that release's command line, then make the game:

```bash
cargo install --locked --git https://github.com/JosephGibson/Tungsten --tag vX.Y.Z tungsten-cli
tungsten new my-game
cd my-game
cargo fmt   # new renames the template's crate in its sources; this reorders their imports
git init
```

`tungsten new` makes the folder from the template the command line carries, so the game starts on the engine you installed it with (`D-182`). The folder's name is the game's: the package in `Cargo.toml`, the crate `my_game` in `src/main.rs` and the files of `tests/game/`, the game ID in `tungsten.json` and the window's title. A name is 1 to 64 lowercase letters, digits, `-` and `_`, starting with a letter, and neither a Rust keyword nor a name Cargo or the engine already uses (`test`, `tungsten`, `serde`); `new` never writes into a folder that exists. The game ID names the folder where the game keeps its player's settings and saves and writes its logs and crash reports.

`my-game` is the game's folder and its working directory: the game reads `tungsten.json`, `input.json` and `assets/` from there. `new` points both engine dependencies in `Cargo.toml` at the tag, `tungsten = { git = "https://github.com/JosephGibson/Tungsten", tag = "vX.Y.Z" }`, adds the engine's build profiles, which build the engine and its dependencies optimized while your own code builds fast and keep line tables in release builds, and writes the engine's `Cargo.lock`, so the first build uses the versions the engine was tested with. `.gitignore` leaves out `target/` and `dist/`.

The title screen shows the game's name, so the title's pixel test, `tests/game/visual.rs`, which `cargo test` skips unless `TUNGSTEN_VISUAL_REGRESSION=1` is set, fails against the template's `tests/fixtures/title.png` until you capture yours, on the machine that runs the test, in a window:

```bash
TUNGSTEN_SMOKE_FRAMES=24 TUNGSTEN_CAPTURE_FRAME=20 TUNGSTEN_CAPTURE_RESOLUTION=1280x720 \
  TUNGSTEN_CAPTURE_PATH="$PWD/tests/fixtures/title.png" cargo run
```

Then make the game yours (`D-181`). Delete `src/sample/`, its script hooks in `sample/script.rs` with it, and its tests, `tests/game/sample.rs` and `tests/game/script.rs`, write your gameplay under `src/features/<name>/` ([Composing with the kit](#composing-with-the-kit)), and point the seam at it, the six names the shell calls and nothing else:

- in `src/game.rs`, `GAMEPLAY`, your gameplay state's ID, which pause opens over and losing focus pauses; `gameplay(start)`, the state the shell puts on the stack for New Game, Continue, Load and Retry, with the run's difficulty and, to continue one, its save; and `difficulty_note(difficulty)`, the note on each difficulty's card in the New Game picker, which the sample fills with its max health: short facts joined by " · ", each a line of its own on the card, as short as `8 health`, which `tests/game/fit.rs` checks at 200 % text;
- in `src/save.rs`, `SaveGame`, what a save holds beside the shell's `meta`; `capture`, the live run as a save, `None` when there is nothing to save; and `summary`, a save's line on its slot row.

Leave `src/shell/` as the template ships it, so a later template's shell can replace it whole. Your gameplay state calls `shell::game_over(world)` when the player dies, asks for an autosave at a checkpoint with `shell::request_autosave(world)`, and reads the run's difficulty through `shell::run(world)`; `save::capture` puts `shell::run_meta(world)`, the run's part of a save, in the `SaveGame` it returns. The sample also uses the shell's `play`, `top`, `stack` and `enter`, `options::GameOptions` and the screens' IDs, and builds its HUD from the engine's `tungsten::core::ui::widgets`; a later shell may change the shell's names, so a game that uses them checks them when it takes one.

The sample's assets go with it, each file with its manifest entry: a `.json` file the manifest no longer names loads as a scene and fails `tungsten check`, while an unlisted image or sound passes unreported and ships in the players' archive. That is every prefab you do not reuse, `assets/data/effects/`, `assets/data/abilities/` and `assets/data/cue_rules/labels.json`, `assets/animations/` and `assets/particles/`, the sample's sounds (`shoot`, `hit`, `hurt`, `pickup`, `music_run` and `music_pause`) and the sprites `tools/sprites.py` draws, with their entries in `tools/sounds.py` and that script (its `gem()` stays while `tools/icon.py` draws the icon from it), and the `shoot` binding and label in `input.json`, whose `labels` list the actions the Keybindings screen shows, each with its row's label. `player.json` names the sample's `player` marker, which only `SamplePlugin::build` registers, so a game that keeps the prefab registers a marker of its own under that name or drops the line. The shell's tests read the sample wherever they import from its `sample` module: the helpers in `tests/game/main.rs` (`player`, `player_position`, `player_entity`, `release_and_settle`, on `SampleRun` and `Player`), and `SampleRun`, `Hud`, `PLAYER_START`, `GEM_STARTS`, `HAZARD_START` and `CAMERA_SHAKE` in `shell.rs`, `saves.rs`, `settings.rs` and `keys.rs`, with a few of its numbers (the max health of each difficulty, a slot's summary line): point those at your gameplay. The deleted `sample.rs` also held the resolved schedule's snapshot and the manifest check, `data_files_load`, which a game writes again for its own. Rewrite the lines of `README.md` and `AGENTS.md` that describe the sample, and count `SaveGame::SCHEMA` on from the sample's 4, so a save the sample wrote while you tried it reads as damaged rather than as a newer version's.

## Run and test

From the game's folder:

```bash
cargo run    # a debug build, with hot reload and the engine's HUD (F4 toggles it)
cargo test   # the game's tests, on the engine's headless harness
```

The first build compiles the engine and its dependencies, which takes a few minutes and more than a gigabyte of disk. The menus take the keyboard, a pad or the mouse. In the game, the arrow keys or WASD move the player, the left mouse button shoots toward the cursor, P or Escape pauses, and Quit Game on the pause menu or Quit on the title ends the run after a question, autosaving it first while the Autosave option is on; the window's close button ends it at once, autosaving it too.

The UI is laid out at the window's height over 720 when that is larger than the OS scale, so a 4K window shows the menus as large against it as a 720p window at 100 % OS scale does, and never in less than 731 × 411 UI units, whatever the player's UI and text scales (`D-184`); `tests/game/fit.rs` checks every screen at six windows and scale extremes. Until the player sets a UI scale, the shell applies 75 % (`theme::UI_SCALE`, `D-199`) and leaves the setting unset, so the Interface tab's Reset returns to it. `tools/sounds.py` synthesizes every sound in `assets/sounds/`, with the standard library alone: change a sound there and run it again; `--check` compares the files with a fresh render.

## Change it while it runs

A debug build watches `assets/` and `input.json`. With the game running, open `assets/sprites/player.png` in an image editor, change it and save: the player draws with the new image. Change a binding in `input.json`, say `move_up` to `KeyI`, and save: the next press reads the new key. Save a room file, or its tileset, and the live room changes in place: its edited cells, and its objects despawned, spawned or spawned again where the file changed them; a file that fails to load keeps the room as it was. A binding's `kind` is `key` (with a `code`), `mouse` (a `button`), `scroll` (a `direction`), `gamepad_button` (a `button`) or `gamepad_axis` (an `axis` and a `direction`), and the rustdoc of `tungsten::core::input::GamepadButton` lists the pad's buttons by position, `South` the bottom face button.

## Check and package

`tungsten check`, from the game's folder, loads every file the game reads with the engine's own loaders and reports every error, each with its file, where the game's own start stops at the first. It loads `tungsten.json` and `input.json` first; an error in either ends the check there, since the game cannot start without them. Then it runs the game's own build, `cargo run`, with `TUNGSTEN_CHECK` set, so every component, data kind and prefab check your plugins register applies: the game opens no window and runs no frame, but loads its manifests, every sprite with its normal map and emissive mask, the animations, UI themes, screen files, sounds, tilemaps, particle configs, data entries, prefabs and each map's objects, and checks every reference between them: a screen file's bindings, row providers, mounts and actions against the names your plugins register in `UiBindings`, its text styles and sizes against the themes, and each screen it pushes against the screens a game registers and the files a data screen opens, each error naming its field (`ui screen 'pause': root.children[2].bind.text: no text binding 'titel'`). Every `.json` file under the manifest's folder that the manifest does not name loads as a scene, as `load_scene` would load it, so a misspelt key there fails too. Every path stays inside the game's folder: an entry that names a file outside it is an error. The check cannot see what lives only in Rust: IDs written in code, sprites or fonts a game registers there, WGSL shaders, font files and the schedule, which a test catches through `Harness::new`. A game that registers room generators (`App::register_generator`) has each one swept over seeds, 0 to 15 by default, 0 to `<count>` − 1 with `--seeds <count>`, or one with `--seed <seed>`: each seed's rooms are generated, checked as a listed room is and removed again, a finding is filed under `generator <id>` with the seeds that gave it, and a line per generator names its seeds and the rooms it made; given `--seeds` or `--seed` with no generator registered, the check says `no room generator registered`. It prints one `<file>: <message>` line per error and a summary, and exits 0 when there is none, 1 when there are errors, and 2 on a usage mistake. The command line's version must be the game's engine version; otherwise it names the `cargo install` line for the game's.

`tungsten package`, on Linux x86-64, builds the game for release and writes two archives to `dist/`: `<name>-<version>-x86_64-unknown-linux-gnu.tar.gz` for players, holding the binary, `tungsten.json`, `input.json`, `assets/`, the game's own `LICENSE*`, `COPYING*` and `NOTICE*` files and the third-party notices, and `<name>-<version>-x86_64-unknown-linux-gnu-debug.tar.gz`, the binary's debug file under the same top folder, so it extracts over the first for a crash's backtrace (`D-120`). It builds for the portable x86-64 baseline with `RUSTFLAGS="-C target-cpu=x86-64 -C link-arg=-Wl,--build-id"`, which replaces the flags of every Cargo config file, so a game's or your own config flags, such as a linker or `target-cpu=native`, do not apply; a `RUSTFLAGS` already set in the shell stops it. The debug file needs line tables, which `debug = 1` under `[profile.release]` keeps, as `new` writes it. Before it archives anything, the staged binary checks the staged files as the player will run them, so a file the manifest names outside `assets/` fails here rather than on the player's machine. The notices list every crate the binary links under a license it ships a text of, by the rules of the engine's own archives ([license notices](releases.md#license-notices)); a crate whose package has no matching text stops the package, naming it, until the game's own `licenses/clarifications.json` supplies the text, in the engine's format. Packaging on Windows and macOS hosts comes later.

## Script, render and describe

`tungsten run --script <file>`, from the game's folder, runs the game in its window from a JSON script and reports what happened. The script names the last frame, an optional `resolution` and its steps; each step has a `frame`, counting frame bodies from 1, an optional `until` and `every` that repeat it, and one verb: `press` or `release` an action, move the `cursor` (`[x, y]` in window pixels, or `{"world": [x, y]}` through the game's camera), call an `action` or read a `probe` the game registered (its arguments in the step's `args`), `spawn` a prefab, load a `scene`, open a `screen` (a `ui_screens` file by its ID, or a screen the game registered by name; a file whose root is not modal on the popup layer fails the step with the reason), clear the game's states for an empty `stage`, spawn a `room` there by its `rooms` ID, `generate` rooms there from a registered generator at a seed (`{"generator": "chain", "seed": 7}`, each at its origin, their IDs prefixed with the generator's or a `prefix` given), lay items out on a `sheet`, draw with another `camera`, turn an `overlay` on, hide the `ui`, `capture` the frame to a PNG (with `until`, a folder of `NNNN.png` frames), `assert` a probe's value and `exit`. A `path` is a JSON pointer into a probe's value. An `assert` names a `probe`, with its `args` and a `path` when it needs them, an `op` and a `value`: `==` and `!=` compare any JSON values, `<`, `<=`, `>` and `>=` two numbers or two strings, and `~=` two numbers or two arrays of numbers of one length, within its `tolerance`, 1e-6 when absent. The rustdoc of `tungsten::core::script` lists every verb's value.

```json
{
  "version": 1,
  "frames": 120,
  "resolution": [1280, 720],
  "steps": [
    { "frame": 2, "action": "start_run" },
    { "frame": 30, "until": 90, "every": 30, "capture": "shots/run" },
    { "frame": 120, "assert": { "probe": "player", "path": "/health/0", "op": ">", "value": 0 } }
  ]
}
```

The command line checks the script before it builds anything, naming the field that fails (`steps[2].captur`), then runs the game's own build with `TUNGSTEN_SCRIPT` set: the window opens at the script's resolution, dt is pinned at 60 Hz, the keyboard, mouse and focus loss are ignored, and the user folder is `--user-dir`'s or a fresh temporary one, so a script never touches the player's saves. It prints each failed assert with its value and the actual, each capture not written and each failed or unreached step, then `tungsten run: <frames> frames, <n> captures, <n> asserts: passed` or `failed`, and exits 0 when the script passed and 1 otherwise. `--report <file>` keeps the JSON report; `--release` runs the release build. Every engine has the probes `frame`, `entities`, `count`, `camera`, `place` (`{"room", "id"}`: a live room's place in world pixels), `states` and `log`; a game adds its own actions and probes in a plugin's `build` with `world.add_script_action` and `world.add_script_probe`, each with a one-line doc and a closure that takes the step's `args` and, for a probe, returns the value it reads, both `serde_json`'s `Value`, re-exported as `tungsten::core::save::Value`, so a game needs no JSON crate of its own: `Value::from`, `.into()` or `collect()` over `(name, value)` pairs builds one. A test runs the same script headless with `Harness::run_script`, where a capture is recorded as not written and fails the report, as an unreached step does, so a test drops the script's captures or reads the report's `asserts`.

`tungsten render` writes a script from its flags and runs it as `run` does, for the shots a review takes. It takes exactly one of `--sheet`, `--scene`, `--at` and `--screen`; a shot of a run through the game's own camera is a `run --script` with a `capture` step. `--sheet goblin,sprite:icon_fire,animation:wisp_idle,particles:spark` clears the game's states for an empty stage and lays the items out in a captioned grid, the camera fitted to it: a bare ID is a prefab, and each item is held at its cell. `--scene <file>` stages a scene file the same way. `--screen <id>` opens a screen file or a registered screen on the frame after the `--after` actions, so `tungsten render --screen pause -o pause.png` shoots the pause menu from its file alone, its bindings showing what they read with no run behind them. `--at <x>,<y>` centres the camera on a point of the game's own world, usually after `--after <action>[=<json>]` actions at frames 2, 3 and on, with `--zoom` or `--view <height>` in world pixels: `tungsten render --after start_run --at 640,2240 --view 288 --overlay physics --frame 120 -o area.png` starts a run and shoots its first area with the physics overlay. One PNG is captured at `--frame` (60 by default, one second in), or `--loop <from>..<to>/<every>` writes every `<every>`th frame into the folder `-o` names, a 20 fps loop at `/3`, in one run. `--size` sets the window and the PNGs (1280x720 by default), `--no-ui` hides the UI, and `--save-script <file>` keeps the script to edit and run with `run`.

`tungsten describe` prints what the game's World holds once its files load: its registered components, with the kit's fields where the kit describes them, the kit's shapes and data kinds, the script hooks with their docs and the manifest's IDs by section, the names a screen file may use that the game registers (its bindings, row providers, mounts, actions and screens by kind) and each screen file with its keys and the names it uses, each prefab with its fields resolved, a variant's base under them, then any error the load found. `tungsten describe <name>` prints only the entries named `<name>`, so `tungsten describe goblin_red` prints that prefab whole; `--json` prints the description as JSON, at version 3.

`tungsten schema ui_screen`, `ui_theme`, `room` and `tileset` print the JSON Schema (draft 2020-12) of a screen, theme, room or tileset file from the command line's own engine, and `-o <file>` writes it, so an editor validates and completes those files as you type. It runs anywhere, the game's folder or not.

`tungsten level` works on the game's rooms, each named by its `rooms` ID or its file. `apply <room> <batch>` applies a batch of commands, a JSON array from a file or `-` for stdin, each one of `place`, `move`, `set`, `delete`, `duplicate`, `paint`, `erase`, `link` and `rename` (`[{ "move": { "id": "crate_1", "by": [1, 0] } }]`): every command is checked against the room as the earlier ones leave it, and nothing is written unless all pass, each error naming its command and field, as `commands[1].move.id`. It writes the room in its canonical form and appends the batch with the one that undoes it to `target/tungsten/journal/<room>.jsonl`; `--inverse <file>` also writes that batch, and `apply <room> --undo` applies the newest one and drops it, refusing a file edited since. `query <room> <id>` prints an entry and `query <room> --at <col>,<row>` what lies at a cell: each layer's tile, the shapes over it, the objects in it and the places holding it. `describe [<room>]` lists each room's layers and named things by kind, with its links. These three read the files with the command line's own engine, so they need no build. `check [<room>...]` runs the game's check and prints those rooms' findings (every room's with none named) and every room generator's, with `--seeds` and `--seed` as `check` takes them, and `render <room> -o <png>` stages the room on an empty stage and captures it, with `--overlay`, `--size`, `--frame` and `--release` as `render` takes them.

## Settings and saves

The engine keeps the player's settings in the game's user folder, which `game.id` names (`D-119`, `D-159`): `~/.config/<id>` on Linux, `%APPDATA%\<id>` on Windows and `~/Library/Application Support/<id>` on macOS, or `settings/` under `TUNGSTEN_USER_DIR` when that is set. Two files live there, both written by the game: `settings.json`, the display mode, resolution, vsync, frame-rate limit, UI and text scales, master volume, bus volumes and the sticks' dead zone and sensitivity the player set, and `input.json`, the bindings the player changed, laid over the shipped `input.json` action by action. A file there that is corrupt falls back to the defaults and stays beside them as `settings.json.bad` or `input.json.bad`; it never stops the game, while a broken shipped `tungsten.json` or `input.json` still does. Without a folder (smoke mode, or no `game.id`) nothing is saved.

The `Settings` resource holds the player's values, `None` for each one left at the shipped value. Change one, apply it, then save, as the shell's Audio tab does with a volume:

```rust
let settings = world.resource_mut::<Settings>();
settings.set_bus_volume(AudioBus::Music, Some(0.5));
shell::apply_settings(world);
if let Err(err) = world.resource::<Settings>().save() {
    log::warn!("Settings not saved: {err}");
}
```

`tungsten::apply_settings` applies them as a restart would: the display through a request the next frame takes, the UI scale to `ScaleFactor` and the volumes to the mixer, an unset volume at the one `tungsten.json`'s `audio` section ships (the template ships 80 %, `D-184`); the shell's `shell::apply_settings` calls it and keeps the game muted while its window has no focus, when the player chose that. The shell's Settings screen changes these values, applies each change at once and saves when it closes, or in the exit's last frame while it is open, where a question still on screen takes its safe answer first, so a window mode not yet kept reverts; a window mode or size waits for Apply, then asks to keep it and reverts after 10 seconds. The engine's F11 and F9 keys toggle fullscreen and vsync, and the shell writes each press into `Settings` and saves it, unless the Settings screen is open, which saves when it closes; the Display rows read it when Settings next opens, and the next apply or start keeps it. Its gameplay options, screen shake, damage numbers, autosaves, Bloom and pausing or muting when the window loses focus, are `GameOptions`, kept in a store of their own under the user folder's `options`. The shell pauses, mutes, writes no autosave while Autosave is off and takes the game's Bloom out while Bloom is off ([Drawing](#drawing)); the game acts on the rest: Screen shake sets the `CameraController`'s `shake_scale`, and the sample's `sample_frame` tags what has `health` with `show.damage_numbers` while Damage numbers is on and quiets the run's music under pause and game over, which a new game does for its own. A value set back to `None` returns to `tungsten.json`'s, and the `TUNGSTEN_DISPLAY_*` overrides still win over both. A window the player resized or maximized keeps its size: the resolution goes out only when the set or shipped one changes (`D-169`). `ActionMap::replace_bindings_and_persist` writes a rebinding to the user folder's `input.json`, and only the actions that differ from the shipped file, so a game update's new bindings reach every action the player left alone.

Saves go in the user folder's `saves` (`~/.local/share/<id>/saves` on Linux, `saves` beside the settings on Windows and macOS), one file per slot, through the `SaveSlots` resource (`D-160`). A save is the game's own serde type with a schema number; bump the number when the type changes shape, and write `migrate_step`, which rewrites a save from one schema to the next, so an older save takes each step in turn. The shell saves your `SaveGame` to three autosaves, `autosave_1` to `autosave_3`, and ten slots, `slot_1` to `slot_10`, which the player saves to from the pause menu. An autosave is written when the game pauses, quits to the title or exits, and when a run starts or the game asks through `shell::request_autosave`, at the start of the next frame whose `capture` returns a save: to the slot the run's last autosave wrote while no fixed step has run since, so a pause and the quit after it take one slot, else to the first empty autosave, else over one an older version wrote under a schema this build's `migrate_step` refuses, else over the oldest, and over a damaged one only when no other can be written; never over a newer version's (`D-204`). The Gameplay tab's Autosave option, on by default, stops them, and New Game asks first while the option is on and an autosave holds a save. The shell's start renames an `autosave` file an earlier shell wrote to a free autosave slot. Continue loads the newest slot that reads and names it with the save's age; it and the Load screen ask first when an older version of the game wrote the save, comparing the versions' leading numbers (`shell::older`), and the slot's row names that version. The sample's `SaveGame` holds the shell's `meta` (the game's version, the difficulty and the playtime), the slot row's health and gem count, and `world`, the run itself, whose spawners keep how long each fallen enemy has been down; the sample's schemas 1 to 3 held no World and 4 its own timers for the fallen enemies, so the default `migrate_step` refuses them and their slots read as an older version's:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveGame {
    pub meta: RunMeta,
    pub health: Health,
    pub gems: i64,
    pub world: WorldSave,
}

impl SaveData for SaveGame {
    const SCHEMA: u32 = 5;
}

/// The live run as a save, its World through `capture_world`; `None` with
/// no run, no player or a dead one.
pub fn capture(world: &World) -> Option<SaveGame>
```

The run is saved by name. Gameplay names each entity a save keeps with `save::name(world, entity, area, name)` once the run's setup is done, before anything random or the run's play touches it: the sample names its player globally and the rest in the area `sample`. A component saves once registered, `app.register_saved::<T>(name)` in `main.rs` for the game's own; the engine's and the kit's register themselves. `save::capture_world` writes each named entity's components as changes over its fresh spawn, each spawn a named entity made (a bolt, a dropped coin) whole, the named entities gone and the registered resources, at a frame boundary. A continued run spawns as a new one, names it, then `save::restore_world(world, &save.world, &(SceneEntity { .. },))` lays the save over it, finding each reference again by name, whatever indices the new run's entities took. A level from a Tiled map spawns with `world.spawn_area(map, origin)`, which names its named objects and keeps its tile edits in the save.

`save(slot, &data)`, `load`, `list` and `delete` return a `SaveError` rather than panic: `NotFound` for a slot never saved, `NewerSchema` for one a newer build wrote, `Corrupt`, `Migration`, `NoFolder` without a folder. A slot name is lowercase letters, digits, `.`, `_` and `-`. A slot that failed to load is the player's progress all the same: the shell shows it as saved by a newer version or damaged, never loads or overwrites it until the game restarts, and lets the player delete it after a question.

Tests set the folder in code, so they never read or write your own settings or saves: the template's `harness()` sets `config.game.user_folder = UserFolder::Off`, and a persistence test points `UserFolder::At(dir)` at a fresh temp folder and builds a second app on it, as `back_saves_settings_and_options_across_a_restart` and `continue_restores_position_health_gems_and_count` do.

## How the game is laid out

- `src/main.rs` loads `tungsten.json`, creates the app, names the manifest, turns on hot reload in debug builds, turns the engine's Escape-to-quit off, registers the game and runs it.
- `src/game.rs` registers the kit, the shell and the game, and holds the seam's three game names; `src/save.rs` holds the other three, `SaveGame`, `capture` and `summary`.
- `src/shell/` holds the screens every game keeps, each a state on the engine's state stack with a UI root it owns: the title, pause, game over, save and load, settings, keybindings, the confirm dialog and toasts, with their theme, built from `tungsten::core::ui::widgets` and the screen files in `assets/ui/screens/`, handling each control by its key.
- `src/sample/` is the sample game: its plugin and gameplay state, its components, its HUD, its music and its script hooks (`script.rs`: `reload_run` and the `player` probe, beside the shell's `start_run` and the kit's `place` and `tagged`), the pattern for a game's own. The player's eased movement, aim and shot, health, knockback and camera, the enemies' motion and touch, the dummy's mend, the gems and the pickups are the kit's, named on their prefabs ([Composing with the kit](#composing-with-the-kit)).
- `tests/game/` steps the game on the headless harness, with no window: a module each for the sample, the shell, the saves, the settings, the keybindings, every screen's fit at six windows and scale extremes, the screen files, and the title's pixels; `sample` holds the snapshot of the resolved schedule.
- `assets/manifest.json` names every asset by an ID, and game code uses the IDs, never file paths ([assets](assets.md)).
- `assets/prefabs/` holds the prefab files, one per entity kind the game spawns by ID ([Queries and spawns](#queries-and-spawns)); `assets/data/tags.json` declares the tags its prefabs name, `assets/data/effects/` and `assets/data/abilities/` hold the effects the prefabs and a continued run name and the shot, `assets/data/cue_rules/labels.json` floats a hit's damage, `assets/animations/enemy_spin.json` turns the enemies, `assets/particles/spark.json` is a bolt's impact, `assets/ui/themes/shell.json` is the shell's theme, `assets/shaders/damage_flash.wgsl` is the kit's hit-flash shader, and `assets/sounds/` and `assets/sprites/` hold what `tools/sounds.py` and `tools/sprites.py` make, M36's `player.png` aside.
- `windows/` holds the game's Windows icon (`D-199`), which `tools/icon.py` draws from `tools/sprites.py`'s gem on the theme's background and writes as `icon.ico` and `icon.res`, and `build.rs` links `icon.res` into the executables of a Windows MSVC build; other targets link nothing. Change the icon in `icon.py` and run it again, `--check` comparing; a game that drops the gem from `sprites.py` draws its own there. Files outside `assets/` are not assets: they take no manifest entry and the players' archive leaves them out.
- `AGENTS.md` holds the repository's rules for agent sessions, and `CLAUDE.md` imports it.

## Systems and stages

A system is a function over the world, `fn(&mut World)`, registered under a name in one of five stages the engine runs in order each frame: `startup` (once, on the first frame), `pre_update` (the engine's hotkeys, its state dispatcher and the UI's input routing), `fixed_update` (once per fixed step: the physics snapshot and `physics_step`), `update` and `post_update` (the physics sync, particles, tweens, game feel and the camera). A game with a UI reads its gameplay input from `RoutedInput`, which `ActionMap` reads as it reads `InputState` but which holds only what the UI did not take, so a click on a button is not also a jump (`D-149`). The shell's plugin registers `shell_start` in `startup`, `shell_frame` and `shell_flow` in `update` and `shell_checkpoint` in `fixed_update`; the sample's registers its component and one system; the player's movement, the hazard's touch and the gems are the kit's, on the fixed step ([Composing with the kit](#composing-with-the-kit)):

```rust
impl Plugin for SamplePlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        world.register_component::<Player>("player");
        schedule.add(Stage::Update, system("sample_frame", frame));
    }
}
```

Where a system goes:

- `update` by default. Systems in one stage run in the order they were added.
- `fixed_update`, before `physics_step`, for a system that drives bodies or reads input for movement: `schedule.add(Stage::FixedUpdate, system("jump", jump).before(PHYSICS_STEP))`, with the name from `tungsten::plugins`. The stage runs once per fixed step, zero, one or two times a frame ([Time and timers](#time-and-timers)), so a body moves the same at any frame rate and a press moves it on the next step. Inside it `just_pressed` and `ActionMap::just_pressed` answer with the presses since the last step, so each press reaches exactly one step and one made while the game is paused, or its clock's scale is 0, reaches none (`D-146`); an event queue's readers see each step's events once. A fixed system reads the events `fixed_update` sends through `iter_current`, after their sender, and those `update` and `post_update` send through `iter`: a frame that runs no step holds them for the next one, except while the game is paused (`D-137`, `D-139`).
- `post_update` for a system that reads the physics sync's `Transform` (the body's drawn point) or sets up the camera, ordered against the engine's names in `tungsten::plugins`: `.after(PHYSICS_SYNC).before(PARTICLE_COUNT_REFRESH)` runs before every other engine system there, `.before(CAMERA_UPDATE)` last but for the camera.

A constraint names a system in the same stage. A typo, a duplicate name or a cycle fails at startup with a message naming the systems, and `Harness::new` panics the same way, so a test catches it. `App::new` installs the engine's `DefaultPlugins`; a game that replaces an engine feature builds its app from the set without that plugin, `App::with_plugins(config, DefaultPlugins::set().without::<CameraPlugin>())`, and `app.add_system_to(stage, desc)` registers one system outside a plugin.

## Queries and spawns

A system reads and writes components through one query that names its data as a type: `&A` reads a component, `&mut A` writes one, `Option<&A>` reads one the entity may lack, and `Entity` is the entity itself, alone or in a tuple of up to eight. A filter as the second type parameter, `With<T>`, `Without<T>` or a tuple of them, narrows the rows by components the body never reads. A game system reads the kit's components the same way: here, whether the player, found by the game's `Player` marker, is down to its last hit (`D-130`):

```rust
let last_hit = world
    .query_filtered::<&Health, With<Player>>()
    .any(|health| health.current() <= 1.0);
```

`world.query::<(Entity, &Transform, Option<&Sprite>)>()` reads; `query_mut` writes, and panics at the call if two items name one component. Write in place through the query rather than collecting entities and calling `get_mut` on each. The engine's `query2`, `query3_mut` and the other numbered functions are deprecated and go at the API freeze; each one's deprecation note gives its tuple form.

A spawn gives the entity every component at once, in one archetype move, through a tuple of up to sixteen components: `world.spawn_with((Player, transform, sprite, Visibility::default()))`. A tuple's elements are components, never bundles; join a bundle with `with`: `RigidBodyBundle::dynamic(position, collider).with((Player, transform))` spawns a body with its `Position`, `PrevPosition` (the history it is drawn from), `Velocity`, `Collider` and `RigidBody` and the game's components beside them. A system without `&mut World` records the same through `CommandBuffer::spawn_with`. A body that jumps to a new place, a respawn or a portal, writes `Position` and `PrevPosition` together; otherwise it is drawn sliding from the old point for a step.

An entity kind the game places more than once, or that a level places, is a prefab: a file under `assets/prefabs/`, listed in the manifest's `prefabs`, with a scene entry's fields but no transform: its sprite, visibility, tag, one tween and its components by their registered names. The template's player is one: its sprite, the game's `player` marker and the kit's components ([Composing with the kit](#composing-with-the-kit)); `SamplePlugin::build` registers `Player` as `player` before the manifest loads, which checks each prefab's components and the assets they name, and the kit's plugins register theirs. The sample's gameplay state spawns its player:

```rust
world
    .prepare_prefab(PLAYER_PREFAB, PLAYER_START)
    .expect("the manifest's prefabs load with it")
    // The state stack despawns it when gameplay leaves the stack.
    .with((SceneEntity { state_id: GAMEPLAY },))
    .spawn(world);
```

The state keeps the handles of what it spawned in a `SampleRun` resource, and its death check, its HUD and `save::capture` read those: when a run replaces another, the old run's entities are despawned at the frame's command flush, so for the rest of that frame a query would still see them.

`prepare_prefab` builds the spawn against the world, with its top-left at the position, and `with` adds components in the same archetype move, here the state that owns the player; `world.spawn_prefab(id, position)` does both. A system queues the spawn for the frame's command flush instead: `let bolt = world.prepare_prefab("bolt", muzzle)?;`, then `world.resource_mut::<CommandBuffer>().spawn_prefab(bolt)`; queued from an entity with `CommandBuffer::spawn_prefab_from(entity, bolt)`, the spawn takes that entity's `SceneEntity` and leaves with its state, as the kit's `Spawner` spawns once per interval. In a Tiled map, an object whose class names a prefab spawns it at the object's top-left, with the object's custom properties written over the prefab's components by dotted path, such as `particle_emitter.config`, through `world.spawn_map_objects(map, origin)`; an object without a class is a marker the game reads from the map's `object_layers`. A level in the engine's own format is a room: a file under `assets/rooms/` listed in the manifest's `rooms`, its tiles from a file under `tilesets`, with its terrain as shapes painted into its layers, its objects by prefab and its places by name ([the asset formats](assets.md)). `world.spawn_room(id, origin, &(SceneEntity { state_id: GAMEPLAY },))` spawns it, each object named `<room>.<id>` for saves, and returns its objects by ID and its places in world pixels; `tungsten check` holds a room to the level rules (an object spawned inside the terrain, a prop with nothing under it, a route its prefab cannot travel), and `tungsten level` edits one ([Script, render and describe](#script-render-and-describe)). A level laid out without either is a scene file, `{ "entities": [...] }`, each entry a prefab's fields with a `transform`, which loads by its path (`D-046`): `tungsten::asset_loader::load_scene(world, path)` checks it against the registered components and `spawn_scene(world, &scene, state_id)` spawns it, owned by that state.

`world.resource::<Time>()` returns a resource the engine always inserts and panics naming the type if it is missing; `get_resource` is the `Option` form for a resource a game may not have inserted.

## Composing with the kit

`tungsten::kit` holds the generic gameplay a game composes in data rather than writes: `game::register` adds every kit plugin before the game's own, `app.add_plugins(KitPlugins::set())`, and a prefab names the kit's components by their scene names. The template's player:

```json
{
  "sprite": { "asset_id": "player" },
  "components": {
    "player": null,
    "tags": ["team.player"],
    "rigid_body": { "kind": "kinematic", "shape": { "kind": "aabb", "half_extents": [12, 12] }, "offset": [16, 16] },
    "action_input": {
      "move": ["move_left", "move_right", "move_up", "move_down"],
      "slots": { "shoot": "shoot" }
    },
    "top_down_mover": { "speed": 240, "acceleration": 1800, "deceleration": 2400 },
    "aim": { "from": "cursor", "offset": [16, 16] },
    "abilities": ["shot"],
    "health": { "max": 5, "invulnerability": 0.5 },
    "hurtbox": { "shape": { "circle": 12 }, "offset": [16, 16] },
    "hit_flash": { "color": [1, 1, 1, 1], "seconds": 0.12 },
    "knockback": { "seconds": 0.2 },
    "stats": { "pickup_radius": 96, "move_speed": 240, "max_health": 5 },
    "camera_rig": { "follow": { "dead_zone": [640, 360] }, "view_height": 720 },
    "cues": {
      "damaged": [{ "shake": 0.6 }, { "sound": { "id": "hurt" } }],
      "ability_used": [{ "sound": { "id": "shoot" } }]
    }
  }
}
```

- `tags` are dotted names the kit's items filter on (`team.player` carries `team`). `assets/data/tags.json`, listed under the manifest's `data.tags`, declares them, and a prefab or a zone filter that names an undeclared tag fails the load, naming the field.
- `action_input` reads the named actions into the entity's `Intent` each fixed step: a movement direction and named slots (`"slots": { "jump": "jump" }`) read as held and just pressed. The movers take the intent when the entity has one, so an AI that writes an `Intent` drives them the same way; a misspelt action fails the load naming its field.
- `top_down_mover` moves the entity at its speed in the intent's direction on the fixed step, easing toward it at its `acceleration` and to a stop at its `deceleration` when they are set: on a kinematic `rigid_body` it slides along walls, without one it moves its `Position` freely, drawn through the physics sync as a body is. `platformer_controller` is the side-view counterpart, on a kinematic body.
- `health` takes hits through the damage pipeline: `tungsten::kit::damage(world, entity, amount)` at once, or a `DamageRequest` carrying a source, damage tags (`damage.fire`), a position, a knockback and a crit flag, applied at once or queued for the step's `damage_resolve`, which resolves the step's requests in a fixed order. A hit passes the entity's `resist` (a factor per damage tag, 0 for immunity) and its `shield` (taken first, recharging after a delay without a hit), then lowers the health, opens its invulnerability window, starts the `hit_flash` on the entity's sprite through the manifest's `damage_flash` material and sends a `Damaged` event; `tungsten::kit::heal` restores it. A game declares its own damage types in the `damage_types` data kind, `{ "physical": { "label_style": { "font": "sans" } }, "physical.slash": {} }`, each key a declared tag and each field inherited by the types under it. A `hitbox`'s or a `contact_damage`'s `damage` may then map types to amounts, `{ "physical.slash": 8, "elemental.fire": 4 }`, and each part resolves on its own: `convert` (`[{ "from": "physical.slash", "to": "elemental.cold", "pct": 0.5 }]`) turns shares into other types, `pen` (`{ "elemental.fire": 0.25 }`) raises the matching resistances below 1, the `conversion` and `penetration` components lend the attacker's own after the hit's, and `resist`, whose factors take modifiers in Rust, multiplies each part; the parts' total then falls once, a `DamagedPart` going out for each part before the hit's `Damaged`. A dead entity stays until something despawns it, unless its `health` names `despawn_on_death`, the seconds after the kill at which the kit despawns it, its `killed` cues having run.
- `hurtbox` is what the kit's hits meet. A `hitbox` hits each hurtbox its shape meets once per activation, a `contact_damage` hurts what it touches again each interval, and a `projectile` ends its hitbox's flight at its first hit, its first wall or its range, unless it pierces or bounces; each hit is tested over the step's motion, so a fast bolt skips nothing, spares the entity that fired it, and queues its damage with its push. A shape centres on its entity's point, while the entity's sprite draws from that point right and down, so a prefab offsets each shape by half its sprite: the template's sprites are 32 px squares, and its shapes, its bodies and the player's aim take `"offset": [16, 16]`. The player, the enemies and the dummy each carry a `rigid_body` too (`kinematic` for what moves, `static` for the dummy), so the player's mover stops against them; F1 outlines every body. A `rigid_body` also takes `layers` and `mask`, lists of layer numbers 0 to 31, and two bodies touch only when each one's mask holds a layer the other is in (`D-178`); the rustdoc of `tungsten::core::component_registry` lists the fields of the engine's own components, `rigid_body`, `particle_emitter`, `tilemap`, `light`, `parallax_layer` and `squash_stretch`, as `api/tungsten-kit-components.txt` lists the kit's. The template's hazard, `assets/prefabs/hazard.json`, patrols a line below the player by an `oscillator` and hurts the player's `hurtbox` through its `contact_damage`, once per half second while they touch, pushing the player away; it and the other red enemies take the player's bolts on a `hurtbox` of their own and fall after three. A `pickup` goes once to the first hurtbox its filter passes, drawn in by its magnet toward a collector whose `pickup_radius` stat reaches it: the template's three gems, `assets/prefabs/gem.json`, drift to the player and count `gems` on the game's `Counters` through their `picked_up` cue.
- `camera_rig` puts a camera rig on the entity: the player's follows its own point with a dead zone of 640 × 360 in a view 720 world units tall at any window, so the player roams the middle of the view before the camera moves and a taller window shows the same play space larger. The highest live rig wins, a tie going to the latest raised, and a rig's `blend` eases the view to it when it starts winning; `fixed` and `scripted` rigs hold a centre, a corner, a box or a point a game moves, `bounds` keeps a rig's view inside a box, a `camera_zone` beside a `zone` raises its rig while something is inside, and the `camera` cue action raises or releases rigs by their `id`. `camera_target` is the follow's shorthand: the camera controller's base rig, under every entity's, follows the lowest-indexed one. The camera writes `CameraState`, its top-left in world units and its zoom, each frame from the winning rig and never reads it back; the sample's gameplay state starts each run centred on its play space with `CameraController::cut_to`, and the shake is added on top.
- `cues` reacts in data to the events about the entity, on the fixed step that reads them: the player's `damaged` list shakes the camera, and the sample's `labels` rule in the `cue_rules` kind floats the hit's amount above it in the engine's mono face, `engine_mono`, but only while the game's flag `show.damage_numbers` is set (the rule's `flags` filter, over the kit's `Flags` resource), which the sample sets from the Damage numbers option. Each event, `damaged_part` (one per part of a hit, before its `damaged`), `damaged` and `killed` (with the hit's source as the other entity), `zone_entered` (the zone's, with the entrant as the other entity), `cooldown_elapsed`, `hit` (the hitting entity's, with its target as the other), `healed`, `picked_up` (the pickup's, with its collector as the other), `expired` (a projectile's end), `effect_applied` and `effect_expired` (an effect's, with its source as the other), `ability_used` (with its target as the other) or `level_up`, takes a list of actions run in order: `prefab`, `counter`, `flag`, `despawn`, `sound`, `shake`, `squash`, `flash`, `label`, `roll`, which draws from a weighted table of the `loot_tables` data kind and spawns what it draws, a drop, `effect`, which applies an `effects` entry to the subject or the other entity, and `ability`, which uses or refills one of theirs. A reaction many prefabs share is a rule in the `cue_rules` data kind, `[{ "on": "killed", "filter": { "all": ["team.enemy"] }, "do": [{ "counter": { "name": "kills", "add": 1, "target": "game" } }] }]`, run after each entity's own cues; a game's own event joins through `CuesOnEvent`. On the damage events a rule may add a `kind` filter over the hit's damage types, `{ "any": ["elemental"] }`, and a `damaged_part` cue's `{ "label": {} }` floats the part in its type's `label_style`. The vocabulary stops at the rule's tag and flag filters and, on the damage events, its kind filter: a reaction that needs a condition is the game's Rust.
- `stats` holds named values that modifiers change, `{ "move_speed": 160 }`. The movers, health, cooldowns and hit items read the well-known ones (`move_speed`, `max_health`, `cooldown_rate`, `damage`) when the entity has them, and a pickup's magnet its collector's `pickup_radius`; a game declares its own, with their bounds, in the `stats` data kind. The sample's player carries `move_speed` so an effect's modifier can slow it, a mover without the stat running on its own field, and `max_health`, whose base the gameplay state sets for the difficulty before the first step: 8, 5 or 3.
- `effects` lists the `effects` data kind's entries the entity starts with, and the kit keeps the instances it takes later: an effect is a change an entity carries for its `duration` (left out for one that lasts, 0 for one that lands and ends at once), with a `damage` or `heal` payload at each `period` (periodic damage passes the target's invulnerability window and starts none), `modifiers` on its stats and `resist` factors per stack, `grant` tags while it runs, `blocked_by` tags that refuse it, a `scale` curve from the `curves` kind, a `stack` policy (a limit, what a repeat does to the time left, whether the end drops one stack or all) and an `on_expire` effect for the end. The template's hazard applies one through its `hit` cue, `"cues": { "hit": [{ "effect": { "id": "slowed", "target": "other" } }] }`, and `assets/data/effects/slowed.json` halves the player's `move_speed` for a second and a half: `{ "duration": 1.5, "modifiers": [{ "stat": "move_speed", "modifier": { "pct": -0.5 } }] }`. A continued run's player spawns under another, `spawn_grace.json`, `{ "duration": 2.5, "grant": ["status.spawning"] }`, and every enemy's `contact_damage` filter leaves it out, `{ "all": ["team.player"], "none": ["status.spawning"] }`, so a save made in an enemy's reach does not die again as it loads; a new run starts clear of every reach and takes none. A game's Rust builds an `EffectRequest` and queues it for the step's `effect_resolve`, or applies it at once; `EffectApplied` and `EffectExpired` go out, and `Effects` on the entity lists what runs.
- `abilities` lists the `abilities` data kind's entries the entity owns, by ID or `#<index>` for one written in place: each has a `trigger` (an intent slot pressed, an interval, or manual), a `cost` off a stat's base, a `cooldown` counted at the entity's `cooldown_rate`, `charges` that refill by time, a `target` (`self`, `aim`, the `nearest` live entity a filter passes within a range, or a `random` one) and a `payload` list run in order: `spawn` a prefab along the direction at a speed, apply an `effect` to the target, an `impulse` through the entity's `knockback`, or a `hitbox` window that arms the entity's own hitbox for its seconds. `tungsten::kit::use_ability(world, entity, name)` uses one from Rust, the `ability` cue action from data, and `AbilityUsed` goes out; a HUD reads one by name from the entity's `Abilities`, through `ready`, `cooldown_left`, `charges` and `refill_left`.
- `level` climbs a counter along a curve from the `curves` kind, `{ "counter": "xp", "curve": "hero_xp", "offers": "hero_offers" }`: each level up sends `LevelUp` and draws the `offers` kind's `count` distinct entries by weight, an effect or an ability each, which a game shows and `Level::choose`s; the next tick applies the choice.
- `ailment_threshold` sets how much a target's buckets must hold before the `ailments` ladders on its `damage_types` entries reach: each landed part of a type fills the target's bucket of that type (its nearest ancestor's when the type sets no ladder), and the kit climbs the rungs from the top down, applying the first whose `needs` fraction of the threshold, `requires` tags and `chance` pass, scaled by the fill over the threshold, then empties the bucket; the buckets drain over the first `needs` rung's `window`. A rung without `needs` applies on any hit, so a type without a threshold still poisons.
- `cooldown` repeats on the game clock and sends `cooldown_elapsed` each time, with an optional jitter drawn from the kit's seeded `KitRng`. A game draws its own random numbers from the same resource (`unit`, `range`, `index`), so a run under one seed repeats, and reseeds it when a run starts, from a seed it can keep in its save. `counters` and `flags` hold named counts and switches that cues write, on the entity or, with `"target": "game"`, in the World's own `Counters` and `Flags`. `spawn_children` spawns prefabs once as the entity's children, at offsets; they follow it and leave with its state.
- `KitTrace` records what the kit's items did, each event, cue action, spawn and despawn; with `KitDebug` on, the log console shows each entry as it lands.
- `knockback` takes a push from `tungsten::kit::knock_back(world, entity, velocity)`, falling to nothing over its seconds: a platformer controller is launched with its input locked, a dynamic body kicked once, anything else pushed along its motion, sliding along walls on a kinematic body. A hit's request carries its own push, which goes through `knock_back` when it lands.
- The motion items move an entity without a body or drive a body: `linear_motion` (a velocity and an acceleration), `oscillator` (a sine on the position, rotation or scale), `follow` (a target's point with a lag), `orbit` (a circle about a centre) and `steering` (seek, flee or arrive; beside a `top_down_mover` it drives the mover, so on a kinematic body it slides along walls). Their targets are the spawn chain's origin, `{ "source": {} }`, the nearest tagged entity, `{ "nearest": { "all": ["team.player"], "range": 320 } }`, or a point. `speed_limit`, `wrap` and `stay_in_view` constrain the motion; `aim`, `facing`, `character_animator` (idle, move, rise and fall clips, by facing) and `depth_sort` present it. The rule that lets them compose: motion moves a root's `Position`, or a body's `Velocity`, before the physics step, one mover drives a kinematic body, the constraints correct after the step, and presentation writes once a frame; a game system that moves what the kit moves joins the same phase and never writes that entity's `Transform`.

Every kit system runs in one of seven phases, anchor systems in `fixed_update` and `post_update` ordered against the engine's: timers first, then intent, motion before `physics_step`, overlap after it, resolution, reactions, and presentation after the physics sync. A game system that belongs with the kit's joins a phase instead of naming an engine system: `ensure_phases(schedule)` in the plugin's `build`, then `schedule.add(Stage::FixedUpdate, Phase::Move.join(system("magnet", magnet)))`. The rustdoc of `tungsten::kit` lists every item by domain, and `api/tungsten-kit-components.txt` every scene component's fields with their types and defaults. The engine's genre fixtures, `crates/tungsten/tests/fixtures/kit/` in its repository at the tag in your `Cargo.toml`, are the cookbook: a platformer, a top-down, an RPG, a shooter and an action RPG folder of prefabs and data, each kit item in at least two of them, with the harness tests that drive them in `crates/tungsten/src/tests/kit_recipes/`; the action RPG folder holds a skill sheet, its casters' stat lines and a target, and a real-time loop's recipes (a mover, a hunting foe, a melee window and drops) are the top-down folder's.

Each feature lives in `src/features/<name>/` from the start: `mod.rs` holds its `Plugin`, its `GameState` and constants, `components.rs` its components and resources and `systems.rs` its systems, a part the size of the sample's HUD taking a file of its own; `game.rs` only adds the feature plugins, a component two features share goes in `src/shared.rs`, and a feature plugin registers its own prefab components, data kinds and systems, as a kit item does. The kit's scene names and data kinds are reserved: the registries fail startup on a duplicate.

## Drawing

The engine draws what the world holds: each entity with a `Sprite`, a `Transform` and `Visibility`, in `z_order`, over the render layers of every tilemap. The shell's screens and the sample's HUD are roots in the engine's UI tree. Anything else the game draws, its own sprite batches, quads and text, it adds to the `Extracts` resource in its plugin's `build` (`D-138`):

```rust
world.resource_mut::<Extracts>().add_text(text);
```

`add_sprites`, `add_quads` and `add_text` each take a function from `&World` to the frame's batches, quads or text sections, called once a frame after the stages. What it returns draws after the engine's own and after whatever was added before it: the engine's plugins first, then the game's, in the order they were added. Under the `gpu_depth` sort (`render.depth_sort`), an instance at `z_norm` 0 draws over everything before it, and another value is tested against the engine's sprites, which sit between 0 and 1. `replace_sprites`, or `app.set_extract_sprites(f)`, swaps the engine's sprites and tilemaps for the game's own function, and what plugins added still follows; replace only when the game draws its `Sprite` entities itself, as example 01 does for its parallax backdrop. `extract_tilemaps` and `extract_tilemap_layers` give such a function the engine's tiles. A sprite drawn with a shader of the game's own takes a manifest `materials` entry, whose `shader` names a `shaders` file ([assets](assets.md)): `world.resource::<MaterialRegistry>().get(id)` gives its handle and `Sprite::with_material` sets it.

The `PostStack` resource lists the passes drawn over the finished frame, Bloom among them. A run builds its stack whole: the sample's gameplay state inserts its own as it enters, one Bloom pass at `sample::BLOOM`, and the default as it leaves, and a game that changes its stack mid-run writes all of it again. The shell edits the stack once a frame (`D-199`): while the Display tab's Bloom option is off it takes the game's Bloom passes out and puts them back at their places when it comes on, game over adds its pixelation and dimming at the end, and a change of run gives the game back the stack as it built it.

## Time and timers

The engine keeps two clocks in the `Time` resource and advances both once a frame, before any stage runs (`D-129`). Real time is the frame's elapsed time, capped at 0.1 s so that a stall cannot throw bodies about. Game time is real time times a scale, 1 unless you set one, and stands still while the game clock is paused. A system of your own that moves or animates something reads the game clock's dt, `delta()`:

```rust
let dt = world.resource::<Time>().delta();
```

`real_delta()` is the real clock's dt, for anything that should keep running over a pause, such as a pause menu's own animation; the engine's screen transitions run on it. `elapsed()` and `real_elapsed()` sum each clock's dts, and `frame()` counts frames.

`pause()`, `resume()` and `set_scale(0.5)` take effect from the next frame: `world.get_resource_mut::<Time>().unwrap().pause()`. A paused clock freezes whatever moves by game time: physics, tweens, particles, game feel, the camera's follow and the game's own systems that read `delta()`. Input keeps arriving, so a pause screen still reads its keys. The sample's gameplay state pauses the clock while a screen covers it and resumes it when uncovered, so the kit's fixed-step systems, which move its player, hold under pause and game over, while the shell's toasts and menu animations run on the real clock.

A `Timer` counts down a cooldown, a wave or a lifetime by whichever clock's dt you tick it with, as the kit's spawners count down to a fallen enemy's return on the game clock and the shell's toasts on the real one. `tick` returns how many times it finished during the tick, so a long frame cannot swallow a repeat:

```rust
/// Starts a wave every two seconds of game time.
struct WaveTimer(Timer);

fn waves(world: &mut World) {
    let dt = world.resource::<Time>().delta();
    let due = world
        .get_resource_mut::<WaveTimer>()
        .map_or(0, |timer| timer.0.tick(dt));
    for _ in 0..due {
        spawn_wave(world);
    }
}
```

`setup` inserts it with `world.insert_resource(WaveTimer(Timer::repeating(2.0)))`; `Timer::once(0.5)` finishes once and stays `finished()` until `reset()`. A timer that spawns or scores runs in `update` on `delta()`, as here; one whose outcome must line up with the kit's hits and motion runs in `fixed_update`, where the kit counts its own.

The game clock is spent in fixed steps of 1/60 s, at most two a frame: `fixed_update` runs once per step, zero times in a short frame at 144 Hz, twice at 30 Hz, and a frame that owes more drops the rest, so a stall slows the game rather than piling up steps; a paused clock runs none. Inside `fixed_update`, `delta()` is the step; in the other stages it is the frame's game dt. Bodies are drawn between their last two steps: `physics_sync` writes each body's `Transform` at `prev + (cur - prev) * alpha`, where `alpha()` is the fraction of a step left over, so a body moves smoothly at any frame rate and is drawn up to one step behind its `Position`. An extract of your own that draws from `Position` does the same lerp with `PrevPosition` and `Time::alpha()`, and draws whatever follows the body at that point. `tungsten.json` sets the step:

```json
"time": { "fixed_step_hz": 60, "max_steps_per_frame": 2, "interpolate": true }
```

A rate below 30 or a bound of 0 fails at startup, and `"interpolate": false` draws every body at its `Position`.

Two more sections set the World's physics and particles at startup, which a plugin may set again after: `physics`, the gravity every dynamic body falls by (px/s², y down, none by default) and the broadphase's cell (32 px), and `particles`, the most particles alive at once (10,000):

```json
"physics": { "gravity": [0, 900], "broadphase_cell": 32 },
"particles": { "budget": 10000 }
```

A cell that is not above 0 fails at startup.

## Read next

- The engine's API: `cargo doc -p tungsten -p tungsten-core -p tungsten-kit --no-deps --open`, from the game's folder; `tungsten::core` and `tungsten::kit` are those two crates, re-exported.
- [Assets](assets.md): each asset type's folder, manifest section and fields.
- [Design](../DESIGN.md): how the engine works, subsystem by subsystem.
