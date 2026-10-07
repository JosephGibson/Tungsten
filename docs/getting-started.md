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
tungsten = { git = "https://github.com/JosephGibson/Tungsten", tag = "vX.Y.Z" }

[dev-dependencies]
tungsten = { git = "https://github.com/JosephGibson/Tungsten", tag = "vX.Y.Z", features = ["testing"] }
```

Rename the game:

- the package in `Cargo.toml`: `name = "my-game"`;
- the crate in `src/main.rs` and `tests/game.rs`: `tungsten_template_basic` becomes `my_game`;
- the game in `tungsten.json`: `"game": { "id": "my-game" }`. The ID names the folder where the game writes its logs and crash reports: up to 64 ASCII letters, digits, `.`, `_` or `-`, starting with a letter or digit, and not a Windows device name such as `con`.

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

The first build compiles the engine and its dependencies, which takes a few minutes and more than a gigabyte of disk. In the game, Enter starts, the arrow keys or WASD move the player, P pauses, Backspace or P resumes and Escape quits.

## Change it while it runs

A debug build watches `assets/` and `input.json`. With the game running, open `assets/sprites/player.png` in an image editor, change it and save: the player draws with the new image. Change a binding in `input.json`, say `move_up` to `KeyI`, and save: the next press reads the new key.

## How the game is laid out

- `src/main.rs` loads `tungsten.json`, creates the app, names the manifest, turns on hot reload in debug builds, registers the game and runs it.
- `src/game.rs` is the game's plugin: its systems by stage, the setup its first frame needs, and its text.
- `src/states.rs` holds the title, gameplay and pause states on the engine's state stack.
- `src/components.rs` holds the game's components.
- `tests/game.rs` steps the game on the headless harness, with no window: a test per system and a snapshot of the resolved schedule.
- `assets/manifest.json` names every asset by an ID, and game code uses the IDs, never file paths ([assets](assets.md)).
- `AGENTS.md` holds the repository's rules for agent sessions, and `CLAUDE.md` imports it.

## Systems and stages

A system is a function over the world, `fn(&mut World)`, registered under a name in one of five stages the engine runs in order each frame: `startup` (once, on the first frame), `pre_update` (the engine's hotkeys and its state dispatcher), `fixed_update` (once per fixed step: the physics snapshot and `physics_step`), `update` and `post_update` (the physics sync, particles, tweens, game feel and the camera). The template's `GamePlugin` registers `setup` in `startup` and `player_movement` in `update`:

```rust
impl Plugin for GamePlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        schedule.add(Stage::Startup, system("setup", setup));
        schedule.add(Stage::Update, system("player_movement", player_movement));
    }
}
```

Where a system goes:

- `update` by default. Systems in one stage run in the order they were added.
- `fixed_update`, before `physics_step`, for a system that drives bodies or reads input for movement: `schedule.add(Stage::FixedUpdate, system("jump", jump).before(PHYSICS_STEP))`, with the name from `tungsten::plugins`. The stage runs once per fixed step, zero, one or two times a frame ([Time and timers](#time-and-timers)), so a body moves the same at any frame rate and a press moves it on the next step. Inside it `just_pressed` and `ActionMap::just_pressed` answer with the presses since the last step, so each press reaches exactly one step, and an event queue's readers see each step's events once. A fixed system reads the events `fixed_update` sends through `iter_current`, after their sender, and those `update` and `post_update` send through `iter`: a frame that runs no step holds them for the next one, except while the game is paused (`D-137`, `D-139`).
- `post_update` for a system that reads the physics sync's `Transform` (the body's drawn point) or sets up the camera, ordered against the engine's names in `tungsten::plugins`: `.after(PHYSICS_SYNC).before(PARTICLE_COUNT_REFRESH)` runs before every other engine system there, `.before(CAMERA_UPDATE)` last but for the camera.

A constraint names a system in the same stage. A typo, a duplicate name or a cycle fails at startup with a message naming the systems, and `Harness::new` panics the same way, so a test catches it. `App::new` installs the engine's `DefaultPlugins`; a game that replaces an engine feature builds its app from the set without that plugin, `App::with_plugins(config, DefaultPlugins::set().without::<CameraPlugin>())`, and `app.add_system_to(stage, desc)` registers one system outside a plugin.

## Queries and spawns

A system reads and writes components through one query that names its data as a type: `&A` reads a component, `&mut A` writes one, `Option<&A>` reads one the entity may lack, and `Entity` is the entity itself, alone or in a tuple of up to eight. A filter as the second type parameter, `With<T>`, `Without<T>` or a tuple of them, narrows the rows by components the body never reads. The template's `player_movement` moves every entity that carries `Player` (`D-130`):

```rust
let step = PLAYER_SPEED * world.resource::<Time>().delta();
for transform in world.query_mut_filtered::<&mut Transform, With<Player>>() {
    transform.position.x += dx * step;
    transform.position.y += dy * step;
}
```

`world.query::<(Entity, &Transform, Option<&Sprite>)>()` reads; `query_mut` writes, and panics at the call if two items name one component. Write in place through the query rather than collecting entities and calling `get_mut` on each. The engine's `query2`, `query3_mut` and the other numbered functions are deprecated and go at the API freeze; each one's deprecation note gives its tuple form.

A spawn gives the entity every component at once, in one archetype move, through a tuple of up to sixteen components. The template's `spawn_player`:

```rust
world.spawn_with((
    Player,
    transform,
    sprite,
    Visibility::default(),
    SceneEntity { state_id: GAMEPLAY },
));
```

A tuple's elements are components, never bundles; join a bundle with `with`: `RigidBodyBundle::dynamic(position, collider).with((Player, transform))` spawns a body with its `Position`, `PrevPosition` (the history it is drawn from), `Velocity`, `Collider` and `RigidBody` and the game's components beside them. A system without `&mut World` records the same through `CommandBuffer::spawn_with`. A body that jumps to a new place, a respawn or a portal, writes `Position` and `PrevPosition` together; otherwise it is drawn sliding from the old point for a step.

`world.resource::<Time>()` returns a resource the engine always inserts and panics naming the type if it is missing; `get_resource` is the `Option` form for a resource a game may not have inserted.

## Drawing

The engine draws what the world holds: each entity with a `Sprite`, a `Transform` and `Visibility`, in `z_order`, over the render layers of every tilemap. Anything else the game draws, its own sprite batches, quads and text, it adds to the `Extracts` resource in its plugin's `build`, as the template's `GamePlugin` adds its text (`D-138`):

```rust
world.resource_mut::<Extracts>().add_text(text);
```

`add_sprites`, `add_quads` and `add_text` each take a function from `&World` to the frame's batches, quads or text sections, called once a frame after the stages. What it returns draws after the engine's own and after whatever was added before it: the engine's plugins first, then the game's, in the order they were added. Under the `gpu_depth` sort (`render.depth_sort`), an instance at `z_norm` 0 draws over everything before it, and another value is tested against the engine's sprites, which sit between 0 and 1. `replace_sprites`, or `app.set_extract_sprites(f)`, swaps the engine's sprites and tilemaps for the game's own function, and what plugins added still follows; replace only when the game draws its `Sprite` entities itself, as example 01 does for its parallax backdrop. `extract_tilemaps` and `extract_tilemap_layers` give such a function the engine's tiles.

## Time and timers

The engine keeps two clocks in the `Time` resource and advances both once a frame, before any stage runs (`D-129`). Real time is the frame's elapsed time, capped at 0.1 s so that a stall cannot throw bodies about. Game time is real time times a scale, 1 unless you set one, and stands still while the game clock is paused. A system that moves or animates something reads the game clock's dt, `delta()`, as the template's `player_movement` does:

```rust
let dt = world.resource::<Time>().delta();
```

`real_delta()` is the real clock's dt, for anything that should keep running over a pause, such as a pause menu's own animation; the engine's screen transitions run on it. `elapsed()` and `real_elapsed()` sum each clock's dts, and `frame()` counts frames.

`pause()`, `resume()` and `set_scale(0.5)` take effect from the next frame: `world.get_resource_mut::<Time>().unwrap().pause()`. A paused clock freezes whatever moves by game time: physics, tweens, particles, game feel, the camera's follow and the game's own systems that read `delta()`. Input keeps arriving, so a pause screen still reads its keys. The template's pause state stops movement through the state stack instead, since nothing else in it moves.

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
