# Getting started

Make a game with Tungsten from its template, `templates/basic`: a title screen, a player that the arrow keys or WASD move, and a pause. The game lives in a repository of its own and depends on the engine through a git tag (`D-123`). This is the guide's first draft; packaging a game for players joins it with `tungsten package`.

## What you need

- Rust through [rustup](https://rustup.rs). The template's `rust-toolchain.toml` pins the toolchain the engine is tested with, and rustup installs it on first use.
- A GPU with a current driver: Vulkan on Linux, DX12 or Vulkan on Windows. Those two are the tested hosts; Metal on macOS is untested ([known issues](known-issues.md)).
- On Linux, `pkg-config` and the ALSA headers (`libasound2-dev` on Debian and Ubuntu).
- Git.

## Start a game from the template

Pick a release tag from the [releases](https://github.com/JosephGibson/Tungsten/releases), here `vX.Y.Z`, and copy the template out of the engine's repository at that tag:

```bash
git clone --depth 1 --branch vX.Y.Z https://github.com/JosephGibson/Tungsten.git tungsten
cp -R tungsten/templates/basic my-game
cd my-game
```

`my-game` is the game's folder and its working directory: the game reads `tungsten.json`, `input.json` and `assets/` from there.

In `Cargo.toml`, point both engine dependencies at the same tag:

```toml
[dependencies]
anyhow = "1"
log = "0.4"
serde = { version = "1", features = ["derive"] }
tungsten = { git = "https://github.com/JosephGibson/Tungsten", tag = "vX.Y.Z" }

[dev-dependencies]
tungsten = { git = "https://github.com/JosephGibson/Tungsten", tag = "vX.Y.Z", features = ["testing"] }
```

Rename the game:

- the package in `Cargo.toml`: `name = "my-game"`;
- the crate in `src/main.rs` and `tests/game.rs`: `tungsten_template_basic` becomes `my_game`;
- the game in `tungsten.json`: `"game": { "id": "my-game" }`. The ID names the folder where the game keeps its player's settings and saves and writes its logs and crash reports: up to 64 ASCII letters, digits, `.`, `_` or `-`, starting with a letter or digit, and not a Windows device name such as `con`.

Add a development profile to `Cargo.toml`, so the engine and its dependencies build optimized while your own code builds fast:

```toml
[profile.dev.package."*"]
opt-level = 2
```

## Run and test

From the game's folder:

```bash
cargo run    # a debug build, with hot reload and the engine's HUD (F4 toggles it)
cargo test   # the game's tests, on the engine's headless harness
```

The first build compiles the engine and its dependencies, which takes a few minutes and more than a gigabyte of disk. In the game, Enter starts, the arrow keys or WASD move the player, P pauses, Backspace or P resumes, M mutes and Escape quits.

## Change it while it runs

A debug build watches `assets/` and `input.json`. With the game running, open `assets/sprites/player.png` in an image editor, change it and save: the player draws with the new image. Change a binding in `input.json`, say `move_up` to `KeyI`, and save: the next press reads the new key.

## Settings and saves

The engine keeps the player's settings in the game's user folder, which `game.id` names (`D-119`, `D-159`): `~/.config/<id>` on Linux, `%APPDATA%\<id>` on Windows and `~/Library/Application Support/<id>` on macOS, or `settings/` under `TUNGSTEN_USER_DIR` when that is set. Two files live there, both written by the game: `settings.json`, the display mode, resolution, vsync, UI scale and master volume the player set, and `input.json`, the bindings the player changed, laid over the shipped `input.json` action by action. A file there that is corrupt falls back to the defaults and stays beside them as `settings.json.bad` or `input.json.bad`; it never stops the game, while a broken shipped `tungsten.json` or `input.json` still does. Without a folder (smoke mode, or no `game.id`) nothing is saved.

The `Settings` resource holds the player's values, `None` for each one left at the shipped value. Change one, apply it, then save, as the template's `toggle_mute` does on M:

```rust
let settings = world.resource_mut::<Settings>();
let muted = settings.master_volume().is_some_and(|volume| volume <= 0.0);
settings.set_master_volume(if muted { None } else { Some(0.0) });
tungsten::apply_settings(world);
if let Err(err) = world.resource::<Settings>().save() {
    log::warn!("Settings not saved: {err}");
}
```

`apply_settings` applies them as a restart would: the display through a request the next frame takes, the UI scale to `ScaleFactor` and the master volume to the mixer. A value set back to `None` returns to `tungsten.json`'s, and the `TUNGSTEN_DISPLAY_*` overrides still win over both. A window the player resized or maximized keeps its size: the resolution goes out only when the set or shipped one changes (`D-169`). `ActionMap::replace_bindings_and_persist` writes a rebinding to the user folder's `input.json`, and only the actions that differ from the shipped file, so a game update's new bindings reach every action the player left alone.

Saves go in the user folder's `saves` (`~/.local/share/<id>/saves` on Linux, `saves` beside the settings on Windows and macOS), one file per slot, through the `SaveSlots` resource (`D-160`). A save is the game's own serde type with a schema number; bump the number when the type changes shape, and write `migrate` to rewrite an older save into the new shape. The template's `SaveGame` holds the player's position: gameplay continues from slot `autosave` and pause saves it.

```rust
#[derive(Serialize, Deserialize)]
pub struct SaveGame {
    pub player: [f32; 2],
}

impl SaveData for SaveGame {
    const SCHEMA: u32 = 1;
}

let start = match world.resource::<SaveSlots>().load::<SaveGame>(AUTOSAVE) {
    Ok(save) => save.player,
    Err(_) => PLAYER_START,
};
```

`save(slot, &data)`, `load`, `list` and `delete` return a `SaveError` rather than panic: `NotFound` for a slot never saved, `NewerSchema` for one a newer build wrote, `Corrupt`, `Migration`, `NoFolder` without a folder. A slot name is lowercase letters, digits, `.`, `_` and `-`. A slot that failed to load is the player's progress all the same: the template never overwrites one in the same run.

Tests set the folder in code, so they never read or write your own settings or saves: the template's `harness()` sets `config.game.user_folder = UserFolder::Off`, and a persistence test points `UserFolder::At(dir)` at a fresh temp folder and builds a second app on it, as `mute_survives_a_restart` and `continue_from_the_autosave` do.

## How the game is laid out

- `src/main.rs` loads `tungsten.json`, creates the app, names the manifest, turns on hot reload in debug builds, registers the game and runs it.
- `src/game.rs` is the game's plugin: its systems by stage, the setup its first frame needs, the hazard's touch, the camera's dead zone and its text. The player's movement, health, knockback and camera and the hazard's patrol are the kit's, named on their prefabs ([Composing with the kit](#composing-with-the-kit)).
- `src/states.rs` holds the title, gameplay and pause states on the engine's state stack.
- `src/components.rs` holds the game's components.
- `tests/game.rs` steps the game on the headless harness, with no window: a test per system and a snapshot of the resolved schedule.
- `assets/manifest.json` names every asset by an ID, and game code uses the IDs, never file paths ([assets](assets.md)).
- `assets/prefabs/` holds the prefab files, one per entity kind the game spawns by ID ([Queries and spawns](#queries-and-spawns)); `assets/data/tags.json` declares the tags its prefabs name, and `assets/shaders/damage_flash.wgsl` is the kit's hit-flash shader.
- `AGENTS.md` holds the repository's rules for agent sessions, and `CLAUDE.md` imports it.

## Systems and stages

A system is a function over the world, `fn(&mut World)`, registered under a name in one of five stages the engine runs in order each frame: `startup` (once, on the first frame), `pre_update` (the engine's hotkeys, its state dispatcher and the UI's input routing), `fixed_update` (once per fixed step: the physics snapshot and `physics_step`), `update` and `post_update` (the physics sync, particles, tweens, game feel and the camera). A game with a UI reads its gameplay input from `RoutedInput`, which `ActionMap` reads as it reads `InputState` but which holds only what the UI did not take, so a click on a button is not also a jump (`D-149`). The template's `GamePlugin` registers `setup` in `startup`, `toggle_mute` in `update` and `hazard_touch` in the kit's overlap phase; the player's movement is the kit's, on the fixed step ([Composing with the kit](#composing-with-the-kit)):

```rust
impl Plugin for GamePlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        schedule.add(Stage::Startup, system("setup", setup));
        schedule.add(Stage::Update, system("toggle_mute", toggle_mute));
        ensure_phases(schedule);
        schedule.add(
            Phase::Overlap.stage(),
            Phase::Overlap.join(system("hazard_touch", hazard_touch)),
        );
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

An entity kind the game places more than once, or that a level places, is a prefab: a file under `assets/prefabs/`, listed in the manifest's `prefabs`, with a scene entry's fields but no transform: its sprite, visibility, tag, one tween and its components by their registered names. The template's player is one: its sprite, the game's `player` marker and the kit's components ([Composing with the kit](#composing-with-the-kit)); `GamePlugin::build` registers `Player` as `player` before the manifest loads, which checks each prefab's components and the assets they name, and the kit's plugins register theirs. The template's `spawn_player`:

```rust
world
    .prepare_prefab(PLAYER_PREFAB, PLAYER_START)
    .expect("the player prefab loads with the manifest")
    // The state stack despawns it when gameplay leaves the stack.
    .with((SceneEntity { state_id: GAMEPLAY },))
    .spawn(world);
```

`prepare_prefab` builds the spawn against the world, with its top-left at the position, and `with` adds components in the same archetype move, here the state that owns the player; `world.spawn_prefab(id, position)` does both. A system queues the spawn for the frame's command flush instead: `let bolt = world.prepare_prefab("bolt", muzzle)?;`, then `world.resource_mut::<CommandBuffer>().spawn_prefab(bolt)`; queued from an entity with `CommandBuffer::spawn_prefab_from(entity, bolt)`, the spawn takes that entity's `SceneEntity` and leaves with its state, as the kit's `Spawner` spawns once per interval. In a Tiled map, an object whose class names a prefab spawns it at the object's top-left, with the object's custom properties written over the prefab's components by dotted path, such as `particle_emitter.config`, through `world.spawn_map_objects(map, origin)`; an object without a class is a marker the game reads from the map's `object_layers`.

`world.resource::<Time>()` returns a resource the engine always inserts and panics naming the type if it is missing; `get_resource` is the `Option` form for a resource a game may not have inserted.

## Composing with the kit

`tungsten::kit` holds the generic gameplay a game composes in data rather than writes: `game::register` adds every kit plugin before the game's own, `app.add_plugins(KitPlugins::set())`, and a prefab names the kit's components by their scene names. The template's player:

```json
{
  "sprite": { "asset_id": "player" },
  "components": {
    "player": null,
    "tags": ["team.player"],
    "action_input": { "move": ["move_left", "move_right", "move_up", "move_down"] },
    "top_down_mover": { "speed": 240 },
    "health": { "max": 5, "invulnerability": 0.5 },
    "hit_flash": { "color": [1, 1, 1, 1], "seconds": 0.12 },
    "knockback": { "seconds": 0.2 },
    "camera_target": null,
    "cues": { "damaged": [{ "shake": 0.25 }, { "label": { "font": "sans" } }] }
  }
}
```

- `tags` are dotted names the kit's items filter on (`team.player` carries `team`). `assets/data/tags.json`, listed under the manifest's `data.tags`, declares them, and a prefab or a zone filter that names an undeclared tag fails the load, naming the field.
- `action_input` reads the named actions into the entity's `Intent` each fixed step: a movement direction and named slots (`"slots": { "jump": "jump" }`) read as held and just pressed. The movers take the intent when the entity has one, so an AI that writes an `Intent` drives them the same way; a misspelt action fails the load naming its field.
- `top_down_mover` moves the entity at its speed in the intent's direction on the fixed step: on a kinematic `rigid_body` it slides along walls, without one it moves its `Position` freely, drawn through the physics sync as a body is. `platformer_controller` is the side-view counterpart, on a kinematic body.
- `health` takes hits through `tungsten::kit::damage(world, entity, amount)`, which lowers it, opens its invulnerability window, starts the `hit_flash` on the entity's sprite through the manifest's `damage_flash` material and sends a `Damaged` event.
- `camera_target` marks what the camera follows; the template gives the camera a dead zone, so the player roams the middle of the view before the camera moves.
- `cues` reacts in data to the events about the entity, on the fixed step that reads them: the player's `damaged` list shakes the camera and floats the hit's amount above it in the manifest face `sans`. Each event, `damaged`, `killed`, `zone_entered` (the zone's, with the entrant as the other entity) or `cooldown_elapsed`, takes a list of actions run in order: `prefab`, `counter`, `flag`, `despawn`, `sound`, `shake`, `squash`, `flash` and `label`. A reaction many prefabs share is a rule in the `cue_rules` data kind, `[{ "on": "killed", "filter": { "all": ["team.enemy"] }, "do": [{ "counter": { "name": "kills", "add": 1, "target": "game" } }] }]`, run after each entity's own cues; a game's own event joins through `CuesOnEvent`. The vocabulary stops at the rule's tag filter: a reaction that needs a condition is the game's Rust.
- `stats` holds named values that modifiers change, `{ "move_speed": 160 }`. The movers, health and cooldowns read the well-known ones (`move_speed`, `max_health`, `cooldown_rate`) when the entity has them; a game declares its own, with their bounds, in the `stats` data kind.
- `cooldown` repeats on the game clock and sends `cooldown_elapsed` each time, with an optional jitter drawn from the kit's seeded `KitRng`. `counters` and `flags` hold named counts and switches that cues write, on the entity or, with `"target": "game"`, in the World's own `Counters` and `Flags`. `spawn_children` spawns prefabs once as the entity's children, at offsets; they follow it and leave with its state.
- `KitTrace` records what the kit's items did, each event, cue action, spawn and despawn; with `KitDebug` on, the log console shows each entry as it lands.
- `knockback` takes a push from `tungsten::kit::knock_back(world, entity, velocity)`, falling to nothing over its seconds: a platformer controller is launched with its input locked, a dynamic body kicked once, anything else pushed along its motion, sliding along walls on a kinematic body. The template's hazard, `assets/prefabs/hazard.json`, patrols a line below the player by an `oscillator`; touching it calls `damage` and, on a hit that lands, `knock_back` away from it, in the game's `hazard_touch` until the kit's contact damage takes it into data.
- The motion items move an entity without a body or drive a body: `linear_motion` (a velocity and an acceleration), `oscillator` (a sine on the position, rotation or scale), `follow` (a target's point with a lag), `orbit` (a circle about a centre) and `steering` (seek, flee or arrive). Their targets are the spawn chain's origin, `{ "source": {} }`, the nearest tagged entity, `{ "nearest": { "all": ["team.player"], "range": 320 } }`, or a point. `speed_limit`, `wrap` and `stay_in_view` constrain the motion; `aim`, `facing`, `character_animator` (idle, move, rise and fall clips, by facing) and `depth_sort` present it. The rule that lets them compose: motion moves a root's `Position`, or a body's `Velocity`, before the physics step, one mover drives a kinematic body, the constraints correct after the step, and presentation writes once a frame; a game system that moves what the kit moves joins the same phase and never writes that entity's `Transform`.

Every kit system runs in one of seven phases, anchor systems in `fixed_update` and `post_update` ordered against the engine's: timers first, then intent, motion before `physics_step`, overlap after it, resolution, reactions, and presentation after the physics sync. A game system that belongs with the kit's joins a phase instead of naming an engine system: `ensure_phases(schedule)` in the plugin's `build`, then `schedule.add(Stage::FixedUpdate, Phase::Move.join(system("magnet", magnet)))`. The rustdoc of `tungsten::kit` lists every item by domain, and `api/tungsten-kit-components.txt` every scene component's fields with their types and defaults. The engine's genre fixtures, `crates/tungsten/tests/fixtures/kit/`, are the cookbook: a platformer, a top-down, a shmup and a grid folder of prefabs and data, each kit item in at least two of them, with the harness tests that drive them in `crates/tungsten/src/tests/kit_recipes/`.

When the game passes about three components or two features, move each feature into `src/features/<name>/`: `mod.rs` holds its `Plugin`, `components.rs` and `systems.rs` the rest; `game.rs` only adds the feature plugins, a component two features share goes in `src/shared.rs`, and a feature plugin registers its own prefab components, data kinds and systems, as a kit item does. The kit's scene names and data kinds are reserved: the registries fail startup on a duplicate.

## Drawing

The engine draws what the world holds: each entity with a `Sprite`, a `Transform` and `Visibility`, in `z_order`, over the render layers of every tilemap. Anything else the game draws, its own sprite batches, quads and text, it adds to the `Extracts` resource in its plugin's `build`, as the template's `GamePlugin` adds its text (`D-138`):

```rust
world.resource_mut::<Extracts>().add_text(text);
```

`add_sprites`, `add_quads` and `add_text` each take a function from `&World` to the frame's batches, quads or text sections, called once a frame after the stages. What it returns draws after the engine's own and after whatever was added before it: the engine's plugins first, then the game's, in the order they were added. Under the `gpu_depth` sort (`render.depth_sort`), an instance at `z_norm` 0 draws over everything before it, and another value is tested against the engine's sprites, which sit between 0 and 1. `replace_sprites`, or `app.set_extract_sprites(f)`, swaps the engine's sprites and tilemaps for the game's own function, and what plugins added still follows; replace only when the game draws its `Sprite` entities itself, as example 01 does for its parallax backdrop. `extract_tilemaps` and `extract_tilemap_layers` give such a function the engine's tiles.

## Time and timers

The engine keeps two clocks in the `Time` resource and advances both once a frame, before any stage runs (`D-129`). Real time is the frame's elapsed time, capped at 0.1 s so that a stall cannot throw bodies about. Game time is real time times a scale, 1 unless you set one, and stands still while the game clock is paused. A system of your own that moves or animates something reads the game clock's dt, `delta()`:

```rust
let dt = world.resource::<Time>().delta();
```

`real_delta()` is the real clock's dt, for anything that should keep running over a pause, such as a pause menu's own animation; the engine's screen transitions run on it. `elapsed()` and `real_elapsed()` sum each clock's dts, and `frame()` counts frames.

`pause()`, `resume()` and `set_scale(0.5)` take effect from the next frame: `world.get_resource_mut::<Time>().unwrap().pause()`. A paused clock freezes whatever moves by game time: physics, tweens, particles, game feel, the camera's follow and the game's own systems that read `delta()`. Input keeps arriving, so a pause screen still reads its keys. The template's pause state pauses the clock on enter and resumes it on exit, so the kit's fixed-step systems, which move its player, hold with it.

A `Timer` counts down a cooldown, a wave or a lifetime by whichever clock's dt you tick it with. `tick` returns how many times it finished during the tick, so a long frame cannot swallow a repeat:

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

`setup` inserts it with `world.insert_resource(WaveTimer(Timer::repeating(2.0)))`; `Timer::once(0.5)` finishes once and stays `finished()` until `reset()`.

The game clock is spent in fixed steps of 1/60 s, at most two a frame: `fixed_update` runs once per step, zero times in a short frame at 144 Hz, twice at 30 Hz, and a frame that owes more drops the rest, so a stall slows the game rather than piling up steps; a paused clock runs none. Inside `fixed_update`, `delta()` is the step; in the other stages it is the frame's game dt. Bodies are drawn between their last two steps: `physics_sync` writes each body's `Transform` at `prev + (cur - prev) * alpha`, where `alpha()` is the fraction of a step left over, so a body moves smoothly at any frame rate and is drawn up to one step behind its `Position`. An extract of your own that draws from `Position` does the same lerp with `PrevPosition` and `Time::alpha()`, and draws whatever follows the body at that point. `tungsten.json` sets the step:

```json
"time": { "fixed_step_hz": 60, "max_steps_per_frame": 2, "interpolate": true }
```

A rate below 30 or a bound of 0 fails at startup, and `"interpolate": false` draws every body at its `Position`.

## Read next

- The engine's API: `cargo doc -p tungsten --open`, from the game's folder.
- [Assets](assets.md): each asset type's folder, manifest section and fields.
- [Design](../DESIGN.md): how the engine works, subsystem by subsystem.
