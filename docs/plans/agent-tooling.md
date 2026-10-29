# Agent tooling foundations (agent-tooling) — in progress

- **status:** in progress
- **goal:** Redo step 7.2: `tungsten run --script` (inputs at frames, assertions, screenshots, a JSON report), `tungsten render` (a scene, an area or a prefab to PNG with overlays, on the capture path) and `tungsten describe`, so asset review sheets and in-game captures come from the command line instead of a scratch crate. Sequence capture, a Later line that waits for 7.2's review sheets, lands with them.
- **non-goals:**
  - The later systems' surfaces (the redo's §8): `describe` of prefabs with resolved fields, `check` with suggestions, scaffolds, `explain`, save dumps, the UI tree dump and named screens (7.17, 7.20), rigs (7.18), a JSON Schema export, a resolved-schedule dump.
  - A command-line run with no window. The harness runs scripts in-process for tests (step 2); a headless CLI mode goes to Later if 7.13's evaluations want one.
  - A hidden or offscreen window: a script run's window shows, as a smoke run's does (Q1).
  - Benchmark rows and counter guards (7.3); 7.2 builds no system, so phase 7's per-step benchmark row does not apply.
  - Agent guidance and the template's `AGENTS.md` (7.16; the file has 1 byte of room, A14).
  - The shell: no file under `templates/basic/src/shell/` or 01's byte-held copy changes (`D-197`); each game registers its own hooks (Q11).
  - Image compositing: a sheet is one rendered frame with its items laid out in the world.
- **files to touch:**
  - Step 1: `crates/tungsten-core/src/script.rs` (new), `crates/tungsten-core/src/tests/script.rs` (new), `crates/tungsten-core/src/lib.rs`, `crates/tungsten-core/src/component_registry.rs`, `api/tungsten-core.txt`.
  - Step 2: `crates/tungsten/src/script.rs` (new), `crates/tungsten/src/tests/script.rs` (new), `crates/tungsten/src/lib.rs`, `crates/tungsten/src/app.rs`, `crates/tungsten/src/capture.rs`, `crates/tungsten/src/input_bridge.rs`, `crates/tungsten/src/testing.rs`, `crates/tungsten/src/logging.rs`, `crates/tungsten/src/state.rs` (a crate-private check for pending or held requests), `crates/tungsten/Cargo.toml`, `Cargo.lock`, `api/tungsten.txt`; the perf evidence in `perf-runs/<UTC>-agent-tooling/` (untracked).
  - Step 3: `crates/tungsten/src/app.rs`, `crates/tungsten/src/script.rs`, `crates/tungsten/src/user_dir.rs`, `crates/tungsten/src/tests/script.rs`, `crates/tungsten/src/tests/user_dir.rs`, `DESIGN.md`, `docs/LLM_INDEX.md`.
  - Step 4: `tools/cli/src/args.rs`, `tools/cli/src/main.rs`, `tools/cli/src/check.rs`, `tools/cli/src/run.rs` (new), `tools/cli/src/describe.rs` (new), `tools/cli/src/tests/args.rs`, `tools/cli/src/tests/run.rs` (new), `tools/cli/src/tests/describe.rs` (new), `tools/cli/tests/cli.rs`, `crates/tungsten/src/app.rs`, `crates/tungsten/src/describe.rs` (new), `crates/tungsten/src/tests/describe.rs` (new), `crates/tungsten/src/lib.rs`, `crates/tungsten-kit/src/foundation/describe.rs`, `api/tungsten-cli.txt`, `api/tungsten.txt`, `api/tungsten-kit.txt`, `scripts/template-check.sh`, `docs/getting-started.md`, `docs/validation.md`, `docs/LLM_INDEX.md`.
  - Step 5: `templates/basic/src/sample/script.rs` (new), `templates/basic/src/sample/mod.rs`, `templates/basic/tests/game/script.rs` (new), `templates/basic/tests/game/main.rs`, `examples/01_platformer/src/features/run/script.rs` (new), `examples/01_platformer/src/features/run/mod.rs`, `docs/getting-started.md`.
  - Step 6: `tools/cli/src/render.rs` (new), `tools/cli/src/tests/render.rs` (new), `tools/cli/src/args.rs`, `tools/cli/src/main.rs`, `tools/cli/tests/cli.rs`, `api/tungsten-cli.txt`, `docs/getting-started.md`, `examples/01_platformer/tools/README.md`, `docs/plans/roadmap-redo.md` (§6), `ROADMAP.md` (the sequence-capture Later line), `DECISIONS.md`, `docs/DECISION_INDEX.md`; the proof in `perf-runs/<UTC>-agent-tooling-sheet/` (untracked).
  - Step 7: this plan (deleted), `ROADMAP.md`, `docs/plans/phase-7.md` (its evidence row), the follow-ups' homes.
- **ordered steps:** (1) The script format and hooks in core. (2) The runner, on the frame body and the harness, with the perf gate. (3) Script mode in the window loop. (4) `tungsten run --script` and `tungsten describe`. (5) The template's and 01's script hooks. (6) `tungsten render` and the review-sheet proof. (7) Close.
- **done-when:** every step's done-when passes, quoted in the evidence log; `just check` passes and example 01 runs; a review sheet of 01 (a contact sheet, a loop and an area still) comes from `tungsten render` with no scratch crate (step 6); the plan deleted with its citations, phase 7's evidence row written, and `agent-tooling` out of Now.

Item `agent-tooling` in [ROADMAP.md](../../ROADMAP.md), from the [roadmap redo](roadmap-redo.md)'s 7.2 row and §8, and step 1 of the [phase 7 plan](phase-7.md) (§2.8: the main track's first step; the UI track's 7.17 starts after it). Written 2026-10-29 at `94cb8744`. Runs under the [tungsten-milestone skill](../../.claude/skills/tungsten-milestone/SKILL.md) and the [operating rules](../operating-rules.md); it differs in one way: the close runs only once steps 1–6 have all landed, since phase 7's step 1 needs all three commands and the sheet (`docs/plans/phase-7.md:244`).

## Context digest

- **Why now.** Every later step's checks, 7.13's evaluations and the asset sessions (the redo's §6) stand on these commands. A review round today builds a scratch driver crate under `target/` per session, launches the game once per captured frame (about 1.4 s each; `e01flask`'s three rounds took about 1,400 frames, some 35 minutes) and cleans up by fingerprint hash.
- **What it stands on.** `tungsten check`'s handshake (`D-182`): the CLI, on core alone, runs `cargo run` in the game's folder with `TUNGSTEN_CHECK` naming a file, and `App::run` does the work in the game's own build. 7.2 adds `run --script`, `render` and `describe` the same way. The capture path (`D-047`, `D-142`): a capture belongs to its frame and its result comes back. Smoke mode's 60 Hz dt and dropped window input. The headless harness (`D-110`). Explicit scene loading (`D-046`).
- **Decisions cited:** `D-182`, `D-183` (the help snapshot), `D-142`, `D-047`, `D-110`, `D-046`, `D-153` (registered components, field paths), `D-159` (`UserFolder` in code), `D-181`/`D-199` (the shell's budget), `D-197` (01 from its folder, its shell byte-held), `D-070` (GPU checks stay local), `D-015` (no new crate: the umbrella takes `serde_json`, already in its graph through core).
- **What later steps expect.** 7.13 runs §8's five tasks as scripts and counts friction; 7.17 and 7.20 render a named screen on this path and extend `describe`; 7.18 describes its rigs; art sessions take contact sheets, loops and stills from `render`.

## 1. Audit

Read at `94cb8744`. `perf-runs/` is untracked; its driver is cited as the owner's local copy.

| # | Finding | Evidence |
| --- | --- | --- |
| A1 | The CLI has `new`, `check` and `package` and parses by hand; any argument starting with `-` other than `-h`/`-V` is an unknown option, so no command takes options yet | `tools/cli/src/args.rs:31-43`, `:49-66`; `tools/cli/src/main.rs:39-51` |
| A2 | `check`'s handshake: `cargo run --quiet` in the game's folder with `TUNGSTEN_CHECK=<report>`; `App::run` sees it first and runs `run_check` with no window, renderer or audio; the version gate holds the game to the CLI's engine | `tools/cli/src/check.rs:22`, `:88-105`; `tools/cli/src/cargo.rs:43-66`; `crates/tungsten/src/app.rs:604-627`, `:629-656`, `:2114` |
| A3 | The capture is one frame: `TUNGSTEN_CAPTURE_FRAME`/`PATH`/`DIRECT`, armed through `armed_for(frame)` for that frame alone; render takes any frame's path (`capture_frame(&path)`), so several frames need only the umbrella | `crates/tungsten/src/capture.rs:24-38`, `:62-64`; `crates/tungsten/src/app.rs:1557-1567`; `crates/tungsten-render/src/screenshot.rs:47`, `:60` |
| A4 | Smoke mode pins dt at 60 Hz and drops keyboard, mouse and cursor events, but focus still applies; smoke resolves no user folder, and outside smoke code's `UserFolder::At` wins over `TUNGSTEN_USER_DIR` | `crates/tungsten/src/app.rs:58-61`, `:2446-2454`, `:2466`; `crates/tungsten/src/user_dir.rs:68-81` |
| A5 | The harness presses an action's first binding (key, button, wheel, pad) and moves the cursor; that code is in `testing.rs`, compiled only under `test` or the `testing` feature | `crates/tungsten/src/testing.rs:206-251`; `crates/tungsten/src/lib.rs:29-30` |
| A6 | The frame body's order: gamepads, display request, time, update, cursor, post-update, flush, events, hot reload, debug views, UI layout, extract, render, recycle, audio | `crates/tungsten/src/app.rs:1745-1790` |
| A7 | `Plugin::build` gets the `Schedule` and the `World`, so a hook registry plugins and games add to is a `World` resource, and core's for the kit to reach it | `crates/tungsten-core/src/schedule.rs:541-543` |
| A8 | Scene components are named in `ComponentRegistry` (name → registration, type → name) with no count by name; the kit's `KitDescriptions` renders the components snapshot as text, through the kit's `components` example | `crates/tungsten-core/src/component_registry.rs:111-118`; `crates/tungsten-kit/src/foundation/describe.rs:365`, `:467`; `scripts/public-api.sh:73-74` |
| A9 | The camera update takes a `CameraState.zoom` that differs from its last output as the new base zoom, so a driver writing the state's zoom draws at base² on later frames | `crates/tungsten/src/camera.rs:72`, `:118`; `crates/tungsten-core/src/camera.rs:194-201` |
| A10 | Overlays are resources with an `enabled` flag (physics) or the kit's `KitDebug`; window input can't reach their F-keys in a script run | `crates/tungsten/src/physics_debug.rs:11-13`; `crates/tungsten-kit/src/foundation/debug.rs:80` |
| A11 | What a stage and a sheet need exists: state requests and scene-owned cleanup, scene load and spawn under a state, prefab spawn by ID, world labels, `AmbientLight`, `Position`/`PrevPosition`/`Velocity` | `crates/tungsten/src/state.rs:171-187`, `:281`; `crates/tungsten/src/asset_loader/scene.rs:26`, `:44`; `crates/tungsten-core/src/assets/prefab.rs:494`; `crates/tungsten-core/src/text.rs:474`; `crates/tungsten-core/src/lighting.rs:15`; `crates/tungsten-core/src/physics/components.rs:10`, `:31`, `:35` |
| A12 | The shell pushes the title at `Startup`; the shell is 2,790 code lines (`tokei`) against `D-199`'s 2,900, and 01's copy is held to it byte for byte | `templates/basic/src/shell/mod.rs:260`, `:271-275`; `D-197` |
| A13 | The umbrella has `serde` and no `serde_json`; core, the kit and the CLI have it | `crates/tungsten/Cargo.toml:21-35`; `tools/cli/Cargo.toml` |
| A14 | Doc budgets: the LLM index is 12,285 of its 12,288 bytes; the template's `AGENTS.md` 12,287 | `wc -c docs/LLM_INDEX.md templates/basic/AGENTS.md` |
| A15 | Today's driver (e01hollow): a scratch crate's system holds the player at a corner, presses bindings, aims through the camera, holds the camera and scripts a scene by gameplay frame; `loop.sh` launches the game once per frame of a loop | `perf-runs/20261027-e01hollow-cave-review/tools/scenes.rs:88`, `:106`, `:122`, `:130`, `:246`; `…/tools/loop.sh` |
| A16 | Sequence capture waits in Later "with redo 7.2's review sheets"; the redo's §6 says each frame is a launch until it lands | `ROADMAP.md:22`; `docs/plans/roadmap-redo.md:415` |
| A17 | `tungsten new` embeds every file under `templates/basic` by walking it, so a new template file ships with no list to update | `tools/cli/build.rs:56-68` |
| A18 | `Startup` runs inside the update stage on the first frame only, before `PreUpdate`, where the dispatcher applies state requests; the shell's title push is queued there | `crates/tungsten/src/app.rs:1305-1311`; `crates/tungsten/src/plugins.rs:120`; `templates/basic/src/shell/mod.rs:274` |
| A19 | `physics_sync` copies `Position` (or its interpolation from `PrevPosition`) into `Transform` in `PostUpdate`, and the sprite extract reads `Transform` | `crates/tungsten-core/src/physics/plugin.rs:52`, `:95`; `crates/tungsten/src/sprite_extract.rs:390-392` |
| A20 | `App::with_plugins` loads the player's settings from the user folder at construction, and a damaged file is quarantined there; only check and smoke runs resolve no folder | `crates/tungsten/src/app.rs:185-200`, `:231`; `crates/tungsten-core/src/settings.rs:171` |
| A21 | A run's fatal error (a failed frame, setup) is kept apart from the capture's error, the fatal one returned first | `crates/tungsten/src/app.rs:658-663` |
| A22 | The dispatcher holds a command that would exit a state entered in the same dispatch, with every command after it, for the next frame; `Replace` takes the top state only, so the shell's `goto` pops down to the bottom state before replacing it | `crates/tungsten/src/state.rs:307-330`, `:355-365`, `:548`; `templates/basic/src/shell/mod.rs:404-407` |
| A23 | An engine exit ends the frame body after `Update`, before post-update, extract and render | `crates/tungsten/src/app.rs:1757-1759` |
| A24 | The template's `src/` is 3,299 code lines against the 3,400 its `AGENTS.md` sets (`D-199`) | `tokei templates/basic/src`; `templates/basic/AGENTS.md` (Layout) |

## 2. Design

### 2.1 Three handshakes

Each is the check's (A2): the CLI validates its input on core alone, then runs `cargo run --quiet` (with `--release` when asked) in the game's folder with variables naming files, and `App::run` reads them before anything else. The variables are the command line's, not a promised interface, as `TUNGSTEN_CHECK` is.

| Command | Variables | `App::run` |
| --- | --- | --- |
| `run --script <file>` | `TUNGSTEN_SCRIPT`, `TUNGSTEN_REPORT`, `TUNGSTEN_USER_DIR` | The window loop in script mode (§2.5); writes the report on every ending |
| `render …` | The same, the script generated (§2.6) | As `run` |
| `describe` | `TUNGSTEN_DESCRIBE` | No window: builds the plugins, loads every file as `check` does, writes the description (§2.7) |

### 2.2 The script (core, `tungsten_core::script`)

JSON, version 1, unknown keys refused, errors with their field path (`serde_path_to_error`, core's since `D-153`), so the CLI rejects a bad script before it builds anything. Top level: `version`, `frames` (the last frame; the run ends after it), optional `resolution` (`[w, h]`, the window's and the captures' size) and `steps`.

A step has `frame` (counting from 1, every frame body, skipped renders included, as `D-142`'s capture counts), optional `until` and `every` (default 1) that repeat it, and one verb:

| Verb | Value | Applies |
| --- | --- | --- |
| `press`, `release` | an action name | Before the frame's `PreUpdate`: its first binding, as the harness does (A5) |
| `cursor` | `[x, y]` in window pixels, or `{"world": [x, y]}` mapped through the game's camera | Before `PreUpdate` |
| `action` | a name, with `args` | Before `PreUpdate`: a registered `fn(&mut World, &Value) -> Result<(), String>` |
| `spawn` | `{"prefab": id, "at": [x, y]}` | Before `PreUpdate`; owned by the stage when there is one |
| `scene` | a scene file, relative to the game's folder | Before `PreUpdate`, spawned under the stage |
| `stage` | `{}`, or `{"ambient": [r, g, b]}` | Held from its frame: on each frame whose dispatch has no state request pending or held (A22), a stack that isn't the engine's empty `ScriptStage` alone gets a pop for each state above the bottom, then, unless the bottom is `ScriptStage`, a replace of the bottom with it, the shell's own `goto` shape; so the game's states exit, their entities go, a state the game pushes later (the shell's title at `Startup`, A18) goes too, and a stage once settled is never replaced again. What a stage owns spawns once `ScriptStage` is active |
| `sheet` | `{"items": ["prefab:goblin", "sprite:icon_fire", "animation:wisp_idle", "particles:spark"], "columns"?, "cell"?, "labels"?}` | Stages if needed, spawns each item in a grid cell (from the items' sprite sizes, else `cell`, default 64 × 64) captioned by a `WorldLabel`, holds each at its cell every frame, and fits the camera to the grid; a bare item is a prefab |
| `camera` | `{"center": [x, y], "zoom": z}`, `{"center", "view_height": h}`, `{"fit": "staged"}` or `null` | Held until `null`, drawn at extract only (§2.4) |
| `overlay` | `{"physics": bool, "kit": bool}` | Sets `PhysicsDebugOverlay.enabled` and `KitDebug` (A10) |
| `ui` | a bool | `false` paints no UI root, game or debug |
| `capture` | a `.png` path; with `until`, a folder of `NNNN.png` named by frame | Arms the frame's render (§2.5) |
| `probe` | a name, with `args` and `path` | After the frame: its value into the report |
| `assert` | `{"probe", "args"?, "path"?, "op": "==", "value", "tolerance"?}`, `op` one of `==`, `!=`, `<`, `<=`, `>`, `>=`, `~=` | After the frame; `~=` compares numbers and arrays of numbers within `tolerance` |
| `exit` | `{}` | Ends the run after the frame |

A `path` is a JSON pointer (RFC 6901, `serde_json`'s `Value::pointer`) into the probe's value, so `{"probe": "player", "path": "/position", "op": "~="}` compares one field; a pointer that finds nothing fails the assert.

Probes and actions live in `ScriptHooks`, a core resource with a one-line doc per entry; a name registered twice panics, as `KitDescriptions` does. Engine probes: `frame`, `entities`, `count` (`{"component": name}`, through a new `ComponentRegistry::count`), `camera` (core: the game's camera, or with `{"drawn": true}` the override in force); `states` and `log` (`{errors, warnings}`, the umbrella's). A game or plugin adds its own in `build` (A7).

The report (version 1): `script`, `frames` run, `ended` (`frames`, `exit`, `game_exit` or `failed`), `error`, `passed`, `captures` (frame, path, written or its error), `probes` (frame, name, value), `asserts` (frame, probe, op, value, actual, passed), `failed_steps` (frame, index, verb, error: an action's error, an unknown action, probe or prefab), `unreached` (every scheduled occurrence the run never settled: the steps after its end, and the exit frame's own after-render steps, since an engine exit returns before render, A23) and `log` (counts and the first five error lines). `Report::passed` is computed in core: the run ended in `frames`, `exit` or `game_exit`, never `failed` (a fatal error, A21), with every assert, capture and step passing and nothing `unreached`, so a game that exits before an assert fails the run. Log lines only count (Q3).

### 2.3 Where in the frame

1. After `Startup` (frame 1 only, A18) and before `PreUpdate`: inputs, cursor, actions, spawns, scenes, the stage's requests, sheet, overlay and `ui`, in step order. On frame 1 the game's `Startup` requests are queued first, so the stage's come after them.
2. `PreUpdate`, the fixed steps and `Update` as today.
3. After `Update`, before `PostUpdate`: the sheet's holds (`Position` and `PrevPosition` at the cell, `Velocity` zero), so `physics_sync` writes the cell into `Transform`, interpolated or not, and the extract draws it there (A19).
4. `PostUpdate`, the flush and the events as today; then, before the UI layout, the camera override swapped in.
5. Render, with the frame's capture armed.
6. After render, before audio: the game's camera put back (§2.4), the capture settled, then probes and asserts read the World.

The runner is one type driven from `run_frame` and the update stage by an `Option` (`None` outside a script), so the harness and the window run the same steps. Captures generalize `Capture` to a frame → path map; `TUNGSTEN_CAPTURE_*` keeps its one frame and is ignored under a script, so smoke and the visual tests stay on it unchanged.

### 2.4 The camera override

The override draws without reaching the game: the runner keeps the game's `CameraState`, writes the override before the UI layout and extract, and puts the game's back right after render, before audio and on every ending, so the camera update never takes the override as its base (A9) and the World holds the game's camera whenever a run returns. `cursor.world` maps through the game's camera, which is what the game reads to aim.

### 2.5 Script mode in the window loop

As smoke: dt pinned at 60 Hz and the window's keyboard, mouse and cursor events dropped. Also: `Focused(false)` dropped, so a game that pauses unfocused plays on; no watcher and no gamepad backend; the window sized by `resolution`; the user folder from `TUNGSTEN_USER_DIR` alone, ignoring code's `UserFolder` (A4), and none without it, so a script never touches the player's folder. The run ends after `frames`, on `exit`, on the game's own exit or on a fatal error (a failed render, `D-142`; A21), and writes the report every time, with the fatal error as `error`; the process exits 0 when `Report::passed`. A skipped frame fails its capture as `D-142` says.

### 2.6 The command line

```
tungsten run --script <file> [--report <file>] [--user-dir <dir>] [--release]
tungsten render (--sheet <item>[,<item>…] | --scene <file> | --at <x>,<y>)
                [--after <action>[=<json>]]… [--zoom <z> | --view <height>]
                [--size <w>x<h>] [--overlay physics,kit] [--no-ui]
                [--frame <n> | --loop <from>..<to>/<every>] [--save-script <file>]
                [--release] -o <path>
tungsten describe [<name>] [--json]
```

- **`run`** parses the script with core's parser (an error exits 1 with its field path, before any build), runs the game with a report at `--report` or in the temp folder, and a user folder at `--user-dir` or a fresh temp folder it removes after. It prints each failure (a failed assert with its value and the actual), then `tungsten run: <frames> frames, <n> captures, <n> asserts: passed` or `failed`, and exits 0 or 1; a usage error exits 2 (`D-182`).
- **`render`** writes a script and runs it as `run` does: `--sheet` stages and lays out a sheet; `--scene` stages, loads the scene and fits the camera; `--at` centres the camera on a point of the game's own world, after the `--after` actions at frames 2, 3, …, so an area is `--after start_run --at <x>,<y>` in a game that registers `start_run`. `--view` sets the view height in world pixels. One capture at `--frame` (default 60, one second), or a loop of `--loop`'s frames into the folder `-o` names; `--size` defaults to 1280 × 720. `--save-script` keeps the generated script to edit and rerun with `run`.
- **`describe`** prints the game's components (the registry's names, with the kit's fields where the kit describes them), shapes and data kinds, the script hooks with their docs, and the manifest's IDs by section; `<name>` keeps the entries so named, `--json` prints the file.

### 2.7 `describe`'s engine side

`TUNGSTEN_DESCRIBE`: `App::with_plugins` treats it as a check run (no user folder, so no settings read or quarantined, A20; warnings and errors only in the log), and `App::run` resolves the schedule, loads the manifest roots' files into the World as `run_check` does (its errors listed, not fatal), and writes JSON from the World: `ComponentRegistry`'s names, `KitDescriptions` (a new `to_json`, under the umbrella's `kit` feature), `ScriptHooks`, and the asset registry's IDs per section. `api/tungsten-kit-components.txt` stays the kit's text snapshot, unchanged.

### 2.8 The games' hooks

Each game's own module, registered from its plugin, outside the shell (Q11): the template's `sample/script.rs` and 01's `features/run/script.rs`. Actions: `start_run` (`{"difficulty"?}`, through `shell::start_run`) and `place_player` (`{"at": [x, y]}` in world pixels, the body's point; held with `until`). Probes: `player` (`{position, health}`), and 01's `area` (the area the player stands in). The template's module is sample code, deleted with the sample, and shows a game how to add its own.

### 2.9 Decision

One entry, the next free ID (`D-224` is the last at `94cb8744`; recheck at commit, with the tungsten-decision skill), in step 6's commit with its index row: the three commands and their handshakes; the script and report formats at version 1, a field added or changed bumping the version; a capture per script frame and loops (amends `D-142`'s one-frame capture, whose variables stand); the camera override drawn at extract only; script mode's pins and its user-folder rule; the umbrella's `serde_json`, already in its graph (no `D-015` rule needed). If step 6 is skipped, the entry waits with the plan, which stays open (step 7).

## 3. Steps

### Step 1: The script format and hooks in core

- **Files:** as listed for step 1.
- **Change:** §2.2's types (`Script`, `Step`, the verbs, `Report`) with `Script::parse` and serialization; the expansion of `until`/`every` into a frame-ordered list, steps of one frame in file order; `Assert::check`; `ScriptHooks` with `add_action`, `add_probe`, lookups and docs; the core probes; `ComponentRegistry::count(name, &World)`, a counting function stored at registration. Rustdoc on every public item; `just api`.
- **Done-when:** `cargo test -p tungsten-core --locked --lib -- script component_registry` → passes, with tests that an unknown key fails naming its path (`steps[2].captur`), version 2 fails, `until`/`every` expand to the listed frames, each op holds on numbers, arrays and strings and `~=` on its tolerance, a `path` selects a field and a missing one fails, `count` counts a registered component by name, and `Report::passed` is false for a `failed` ending with no capture, for a `game_exit` with an assert `unreached`, and true for a clean `frames` ending; `just api-check` after `just api` → passes, `api/tungsten-core.txt`'s diff adding only `script` items and `ComponentRegistry::count`; `cargo clippy -p tungsten-core --all-targets --locked -- -D warnings` → clean.
- **Commit:** `feat(core): add the script format and script hooks`
- **Moves:** `api/tungsten-core.txt`.

### Step 2: The runner, on the frame body and the harness

- **Files:** as listed for step 2.
- **Change:** the binding press and release move from `testing.rs` to `input_bridge.rs`, the harness calling them. `script.rs`: the runner over §2.3's points, the `ScriptStage` state, the sheet's layout and holds, §2.4's camera swap, captures through the generalized `Capture`, probes and asserts, the report; `ScriptHooks` inserted at `App::new` with the umbrella's probes; the log counts from the logger. `run_frame` and the update stage call the runner at §2.3's points, each under one `Option` test. `Harness::run_script(&Script) -> Report` runs the same runner on the harness's frames, each capture recorded as not written (headless). The umbrella takes the workspace's `serde_json`.
- **Done-when:**
  - `cargo test -p tungsten --locked --lib -- script testing` → passes, with harness tests that: a press and a release reach `ActionMap` as edges on their frames; an action's error and an unknown action fail their steps and the run; a failed assert reports its actual value; `until`/`every` probes land on their frames; a frame-1 `stage` under a test plugin whose `Startup` pushes a state, as the shell does, leaves the stack holding `ScriptStage` alone by frame 3 and the pushed state's entities gone, then a sheet spawned under it keeps its entities to frame 60 with `ScriptStage` entered once and never exited; a `stage` over a two-state stack (a gameplay state under a covering one) leaves `ScriptStage` alone and both states' entities gone; an `engine_exit` press with a failing assert on the same frame ends the run with that assert `unreached` and the run failed; a sheet of three moving prefabs keeps each at its cell in `Harness::draw`'s sprites after 60 frames, with interpolation on and off; the game's `CameraState` reads the same after a run of 10 overridden frames returns as with no override; `ui: false` leaves the frame's paint list empty; the existing harness tests pass unchanged.
  - `just deps` → passes; `cargo tree -p tungsten --locked -e normal -i serde_json` → lists the umbrella's own edge, with no new package in `Cargo.lock` (`git diff --unified=1 Cargo.lock` shows only `tungsten`'s dependency list). `just api-check` after `just api` → passes.
  - Perf, local, before the commit: `git archive` of step 1's commit under `target/`; base and candidate `just perf suite --repeat 5` twice each in one sitting with nothing beside it. First the base's A/A compare → no owned metric `regressed` or `improved` beyond the profiling workflow's named A/A exceptions (its Compare §8); otherwise the sitting runs once more before any candidate is judged. Then candidate against base → no owned metric `regressed`, or each one that is carries the regression policy's placement evidence (the same verdict from the base plus a function no frame calls), recorded with both compares. With no script each frame pays two `Option` tests, so a real cost is a mistake. This is the plan's one frame-loop change: steps 3–6 add no per-frame work.
- **Commit:** `feat(tungsten): run scripts on the frame body and the harness`
- **Moves:** `api/tungsten.txt`; `Cargo.lock`'s `tungsten` entry.

### Step 3: Script mode in the window loop

- **Files:** as listed for step 3.
- **Change:** `App::with_plugins` reads `TUNGSTEN_SCRIPT` (a bad script is the run's error, logged), applies `resolution`, resolves the user folder by §2.5's rule; `App::run` starts no watcher or pad backend under it; the window event handler drops what §2.5 drops; the report is written on every ending. `DESIGN.md`: a paragraph on script mode beside the M70 section, and the opening paragraph names the commands once all land (step 6). The LLM index's debug-overlays row gains `tungsten/script.rs`, within its cap (A14).
- **Done-when:**
  - `cargo test -p tungsten --locked --lib -- user_dir script` → passes, with a test that a script run's folder is `TUNGSTEN_USER_DIR`'s over code's `UserFolder::At`, and none without it.
  - GPU, local: in `templates/basic`, `TUNGSTEN_SCRIPT=<fixture> TUNGSTEN_REPORT=<r> cargo run --quiet` with a fixture of 45 frames (a press, captures at frame 10 and at 20, 30 and 40 by `every`, an assert on `entities`) → exit 0; `jq -r '.passed, .ended, (.captures | length)' <r>` → `true`, `frames`, `4`; four PNGs at the fixture's resolution. The same fixture with an assert that fails → exit 1 and `.passed` false.
  - `just smoke` → passes; `just visual` → passes with no reference changed.
  - No per-frame work added: `git diff --unified=1 HEAD -- crates/tungsten/src/app.rs` before the commit touches construction, `run` and the window event arms only, never `run_frame` or a `stage_*` function; a change there runs step 2's sitting again.
- **Commit:** `feat(tungsten): run a script in the window loop`
- **Moves:** none.

### Step 4: `tungsten run --script` and `tungsten describe`

- **Files:** as listed for step 4.
- **Change:** `args.rs` parses options per command; `run.rs` (§2.6) and `describe.rs` share `check.rs`'s folder check and `cargo run` launch; `App::run` gains `TUNGSTEN_DESCRIBE` (§2.7) and `describe.rs` its JSON; the kit's `KitDescriptions::to_json`. `scripts/template-check.sh` runs `tungsten describe` and a 30-frame `tungsten run --script` with one capture and one assert on the outside copy. `docs/getting-started.md` gains "Script, render and describe" after "Check and package"; `docs/validation.md`'s command-line row names the new tests and the local GPU run; the LLM index's CLI row names the new files.
- **Done-when:** `cargo test -p tungsten-cli --locked` → passes, with tests that each command's options parse and an unknown or misplaced one exits 2, a bad script exits 1 with its field path and no `cargo` run, and the summary lines format a report's failures; `cargo test -p tungsten --locked --lib -- describe` → passes, the template's World describing its registered components, the kit's fields, the script hooks with docs and the manifest's IDs by section, and a describe run with `TUNGSTEN_USER_DIR` over a damaged `settings.json` resolving no folder, the file left unmoved. With `T=$PWD/target/debug/tungsten` after `cargo build -p tungsten-cli`: `(cd templates/basic && $T describe --json) | jq -e '(.components | length) > 0 and (.script.probes | map(.name) | index("entities")) != null'` → true. GPU, local: `(cd examples/01_platformer && $T run --script <fixture>)` → exit 0 and a last line ending `passed`; a fixture with a failing assert → exit 1 naming the assert, its value and the actual. `just template-check` → passes with its new lines. `just api-check` after `just api` → passes.
- **Commit:** `feat(cli): add tungsten run --script and tungsten describe`
- **Moves:** `api/tungsten-cli.txt`, `api/tungsten.txt`, `api/tungsten-kit.txt`.

### Step 5: The template's and 01's script hooks

- **Files:** as listed for step 5.
- **Change:** §2.8's actions and probes in each game, registered from the sample's plugin and 01's run plugin. `docs/getting-started.md`'s list of what a game deletes with the sample gains `sample/script.rs`. A template harness test runs a script through `Harness::run_script`.
- **Done-when:** `cargo test -p tungsten-template-basic --locked` → passes, with a test whose script starts a run at frame 2, holds the player at a point from frame 30 to 90 and asserts `player` at `/position` there at frame 90 within 0.5, and one whose frame-1 `sheet` under the real shell leaves the stack holding `ScriptStage` alone by frame 3 with the sheet's items spawned; `cargo test -p tungsten-cli --locked` → passes (the embedded template); `cargo test -p example-01-platformer --locked --no-run` → builds; GPU, local: `(cd examples/01_platformer && $T run --script <fixture>)`, starting a run and asserting `area` at a placed point → exit 0; `(cd templates/basic && $T describe start_run)` → prints the action and its doc; `just repo-check` → passes (the shells unchanged); `tokei templates/basic/src --output json | jq '.Rust.code'` → at most 3,400 (A24).
- **Commit:** `feat(template): register the sample's script hooks`, and 01's in the same commit
- **Moves:** none.

### Step 6: `tungsten render` and the review-sheet proof

- **Files:** as listed for step 6.
- **Change:** `render.rs` builds §2.6's script from its flags and runs it through `run`; the help and its snapshot; `docs/getting-started.md`'s section gains `render`; 01's tools README replaces "make a contact sheet in temporary output" with `render --sheet`, `--loop` and an area still, with one review script inline; the redo's §6 drops "until sequence capture lands" and says loops are one run; the Later line for sequence capture goes; the decision entry (§2.9). Then the proof, a lean review sheet (the owner's rule, 2026-10-28): a contact sheet of 01's six foes, a loop of one animated effect at 20 fps, and an area still with the physics overlay.
- **Done-when:** `cargo test -p tungsten-cli --locked` → passes, with a golden script per flag set and an error per bad flag. GPU, local, in `examples/01_platformer`: `$T render --sheet <the six foes> -o <dir>/foes.png` → exit 0 and a 1280 × 720 PNG; `$T render --sheet <an effect> --loop 2..61/3 -o <dir>/loop/` → 20 PNGs whose SHA-256 sums are not all equal; `$T render --after start_run --at <x>,<y> --view 288 --overlay physics --frame 120 -o <dir>/area.png` → exit 0; `fd -g Cargo.toml target --max-depth 3` → none (no scratch crate); the three opened and checked by eye, and linked in the evidence row. `just api-check` → passes; `just repo-check` → passes (the decision's index row).
- **Commit:** `feat(cli): add tungsten render`
- **Moves:** `api/tungsten-cli.txt`.

### Step 7: Close

- **Files:** this plan (deleted), the files that cite it (`ROADMAP.md`), `docs/plans/phase-7.md`, the follow-ups' homes.
- **Runs only** once steps 1–6 have all landed. With one skipped, the plan stays with its status `in progress`, `agent-tooling` stays in Now, the evidence log names the step and its reason, and the session ends with the run prompt for the skipped step, not 7.3's.
- **Change:** run `just check` and 01; move open follow-ups to known issues or Later; write phase 7's evidence row for step 1 (the commit range and this plan's done-when quoted); delete this plan and its citations; take `agent-tooling` out of Now and say in phase 7's Now line that 7.3 is next.
- **Done-when:** `just check` → passes; `git diff --check` → clean; `cd examples/01_platformer && cargo run` → plays to its first area; `just repo-check` → passes; `rg -n 'agent-tooling' ROADMAP.md docs/plans/phase-7.md` → only phase 7's evidence row.
- **Ends with:** the commits made, the evidence quoted, what the owner should try (`tungsten render --sheet` and a loop in 01, and `tungsten describe` in the template), and the next prompt: 7.3's plan, from tungsten-next's `plan` row with the phase 7 plan and the redo's 7.3 row and §5.8 as its sources. The UI track (7.17) may start in its own worktree now (phase 7's §2.8).

## 4. Open questions

| # | Question | Default |
| --- | --- | --- |
| Q1 | A hidden window for script runs? | No: a hidden or unmapped surface may never present (Wayland), which fails every capture by `D-142`; the window shows, focus loss is dropped, and a hidden window goes to Later if runs on the owner's desktop get in the way |
| Q2 | JSON or a line-based script language? | JSON: no parser to write, `serde_path_to_error`'s field paths, and the CLI can validate it on core alone |
| Q3 | Does an error-level log line fail a run? | No: the report counts them, and `assert` on `log` makes it a check |
| Q4 | Frame numbering | Frame bodies from 1, skipped renders included, as `D-142` counts |
| Q5 | `render`'s capture frame | 60 by default; a render after `--after start_run` raises it past the game's curtain, as step 6's area still does |
| Q6 | Where 01's review scripts live | In each art session's evidence folder, as today's drivers do; 01's tools README holds one inline as the pattern, so no committed script rots with the level |
| Q7 | Scripts in release builds | Honoured in every build, as `TUNGSTEN_CHECK` is (`package` checks the release binary) |
| Q8 | A command-line run with no window | Later, if 7.13's evaluations need one; tests use `Harness::run_script` |
| Q9 | Hooks in the shell or the games | The games': the shell has 110 lines of `D-199`'s room and a byte-held copy in 01 |
| Q10 | `stage` in a game whose systems need its own states | The game registers a `stage` action of its own and `render` calls it when `describe` lists it; neither game here needs one (step 5 checks) |

Each question takes its stated default; the owner reviews the plan on their own time.

## 5. Invariants

- **May move:** `api/tungsten-core.txt` (step 1), `api/tungsten.txt` (2, 4), `api/tungsten-kit.txt` (4), `api/tungsten-cli.txt` (4, 6), `Cargo.lock`'s `tungsten` entry (2), each diff recorded in the evidence log.
- **Must not move:** the determinism hash, the pinned containment hash, the row digests, `gpu-visual.png`, the post and transition regressions and every visual reference (the one-frame `TUNGSTEN_CAPTURE_*` path is unchanged); `tungsten check`'s and `package`'s output; every file under both shells; 01's save schema 23 and its save tests; `api/tungsten-kit-components.txt`; the doc budgets (A14); the template's `src/` within 3,400 code lines (A24).

## 6. Stop conditions

- A done-when check fails: restore the step's files from the last commit, mark the step skipped, and continue with steps that don't depend on it (4–6 depend on 2 and 3; 6 on 4 and 5). The close then waits (step 7).
- Step 2's base A/A reads unstable twice: the step's perf check is recorded as not run with both compares and step 2 waits uncommitted for a quiet sitting; steps 3–6 wait with it.
- Step 2's candidate reads an owned metric `regressed` without placement evidence: a real cost moves the runner's tests behind one cold call and the sitting runs once more; a second real regression stops step 2 before its commit, recorded with both compares, and steps 3–6 wait with it.
- Captures fail because the window's frames skip (an occluded or unmapped window): record the skips and the backend, and stop the GPU checks of that step, not the CPU ones.
- A change needs a shell file, the template's `AGENTS.md` or a new crate: stop that part and record it as a follow-up.
- A game's systems fail under `stage`: Q10's game action, in step 5's files, recorded.
- The LLM index won't fit its cap: tighten the touched rows' words, never drop a path.

## 7. Critique

Round 1: the `critique` skill with `--repo`, `gpt-6-astra` at xhigh, 2026-10-29, reading `8a7346a1`. Footer: `375.7 s, tokens in 909870 (cached 822144), out 9274 (reasoning 4844), commands 33, output 365.4 KB (largest 43 KB), usage 5h 42% → 63%, week 19% → 22%`. Eight findings, all accepted; they changed done-when checks, the perf gate's step and the close, so a second round runs.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 1 | [HIGH] A frame-1 `stage` runs before the shell creates its title state | `Startup` runs in the update stage before `PreUpdate` (`crates/tungsten/src/app.rs:1305-1311`); the shell queues the title there (`templates/basic/src/shell/mod.rs:274`) | accepted | A18; §2.2: `stage` is held each frame until the stack holds `ScriptStage` alone; §2.3: steps apply after `Startup`; steps 2 and 5 test a frame-1 stage under a `Startup` push and the real shell |
| 2 | [HIGH] The success predicate does not account for fatal errors or unreached assertions | The fatal error is kept apart from the capture's (`crates/tungsten/src/app.rs:658-663`); §2.2 had no rule for steps after a game's exit | accepted | A21; §2.2: `Report::passed` in core needs a non-`failed` ending and nothing `unreached`; §2.5 reports the fatal error; step 1 tests both cases |
| 3 | [MEDIUM] Sheet holds happen too late to control the rendered positions | `physics_sync` writes `Transform` in `PostUpdate` (`crates/tungsten-core/src/physics/plugin.rs:52`, `:95`), and the extract reads `Transform` (`crates/tungsten/src/sprite_extract.rs:390-392`) | accepted | A19; §2.3: holds after `Update`, before `PostUpdate`; step 2 checks the drawn sprites of moving prefabs, interpolation on and off |
| 4 | [MEDIUM] The camera remains overridden when the final frame returns | §2.4 restored at the next frame's start, which the last frame lacks | accepted | §2.3 and §2.4: the game's camera goes back right after render and on every ending; the `camera` probe reads the game's, `drawn` the override; step 2's test reads it after the run returns |
| 5 | [MEDIUM] The required player-position assertion cannot be expressed as specified | §2.2's `~=` took numbers and arrays; §2.8's `player` is an object | accepted | §2.2: `path`, a JSON pointer into a probe's value; step 5 asserts `/position` |
| 6 | [MEDIUM] `describe` is intercepted after player settings have already been loaded | Settings load at construction from the user folder (`crates/tungsten/src/app.rs:231`) and a damaged file is quarantined (`crates/tungsten-core/src/settings.rs:171`); only check and smoke skip the folder (`:185-200`) | accepted | A20; §2.7: describe counts as a check run at construction; step 4 tests a damaged settings file left unmoved |
| 7 | [MEDIUM] The combined performance gate cannot roll back everything it measures | The gate sat in step 3 while step 2's runner was already committed | accepted | The sitting moves into step 2, before its commit, as the plan's one frame-loop change; step 3 checks it adds no per-frame work; §6 stops step 2 before its commit |
| 8 | [MEDIUM] The closing step advances the roadmap even when `render` is skipped | Phase 7's step 1 needs all three commands and the sheet (`docs/plans/phase-7.md:244`) | accepted | The header's difference, step 7's "runs only" and §6: the close waits for steps 1–6, the plan and Now line stay, and the hand-off reruns the skipped step; §2.9 waits with it |

Round 2: the same reviewer at xhigh, 2026-10-29, reading `36b9a397` with the Critique section and the log cut and a focus note on round 1's changes. Footer: `382.5 s, tokens in 953740 (cached 874624), out 8445 (reasoning 4953), commands 20, output 313.4 KB (largest 51.1 KB), usage 5h 63% → 84%, week 22% → 25%`. Four findings, three repairs of round-1 fixes, all accepted; no third round, and none left open.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 2.1 | [HIGH] Repeating the stage request can replace the newly entered stage again | The dispatcher holds a command exiting a state entered in the same dispatch, and every command after it, for the next frame (`crates/tungsten/src/state.rs:307-330`, `:355-365`) | accepted | A22; §2.2: the stage requests only on a frame with nothing pending or held, and never replaces a settled `ScriptStage`; step 2 checks the sheet's entities live to frame 60 with one entry and no exit |
| 2.2 | [MEDIUM] Replacing an absent stage does not clear a multi-state stack | `Replace` takes the top only (`crates/tungsten/src/state.rs:548`); the shell pops to the bottom first (`templates/basic/src/shell/mod.rs:404-407`) | accepted | §2.2: pops above the bottom, then a replace of the bottom, the shell's shape; step 2 tests a two-state stack |
| 2.3 | [MEDIUM] The success predicate leaves unfinished checks on the exit frame ambiguous | An engine exit returns after `Update`, before render and the after-render steps (`crates/tungsten/src/app.rs:1757-1759`) | accepted | A23; §2.2: `unreached` counts every occurrence the run never settled, the exit frame's included; step 2 tests an exit with a failing assert on its frame |
| 2.4 | [MEDIUM] The performance gate excuses regressions by metric name, regardless of magnitude | The A/A check must read no `regressed` or `improved` beyond its named exceptions before verdicts count (`docs/perf/profiling-workflow.md:167`), and a regression's placement needs the padded-baseline evidence (`:212`) | accepted | Step 2's perf check judges the base A/A first, reruns an unstable sitting once, and accepts a candidate regression only with placement evidence; §6 records a twice-unstable A/A as not run |

Checking round 2's findings turned up one more limit, the critic's neither: the template's `src/` has 101 lines left of its 3,400 (A24), now a step-5 check and an invariant.

## Evidence log

| Step | Date | Verdict | Key numbers | Paths |
| --- | --- | --- | --- | --- |
| 1 | 2026-10-29 | passed | `cargo test -p tungsten-core --locked --lib -- script component_registry` → 31 passed (12 `script`, 19 `component_registry`, `count_counts_a_component_registered_as_itself` new); doc tests 5 passed; `api/tungsten-core.txt` +673 −0, every line under `tungsten_core::script`, `World::add_script_action`/`add_script_probe` (the hooks' bare-World door, as `register_component`'s) or `ComponentRegistry::count`; `just api-check` → current; clippy `-D warnings` → clean. Deviations: `count` is `None` for a name registered with a definition (`register_with`, every kit and engine component but `register::<T>`'s), since the registry knows only the definition's type, and the `count` probe fails such a step naming it (F2); the report's `passed` is a field set by `Report::finish`, the verdict computed there | `crates/tungsten-core/src/script.rs`, `crates/tungsten-core/src/tests/script.rs` |
| 2 | 2026-10-29 | passed | `cargo test -p tungsten --locked --lib -- script testing` → 74 passed (11 new in `script::tests`, one per done-when clause, plus a headless capture recorded as not written), the existing harness tests unchanged; the full umbrella lib 733 passed; clippy `-D warnings` clean with default features, none, and `testing` alone; `just deps` → advisories, bans, licenses, sources ok; `cargo tree -p tungsten --locked -e normal -i serde_json` → `tungsten` listed directly; `Cargo.lock` +1 line, `serde_json` in `tungsten`'s list, no new package; `api/tungsten.txt` +1 (`Harness::run_script`); `just api-check` → current. Perf (`suite --repeat 5`, base `git archive 2ecd7d41`, prebuilt binaries, one background job each): sitting 1's base A/A read `gpu` `render_encode` p95 `regressed` on one binary (1.892 → 2.098 ms), outside §8's exceptions, so the sitting ran again; sitting 2's base A/A 0 `regressed`/`improved`, but direct pair B read `ecs` `follow` p50 1.618 → 1.790 (the per-run mode: both binaries at 1.57 and 1.84) and `churn` `flush` p50 2.474 → 2.582 `regressed`, with no placement control (`flush_reusing` at 16 mod 64 in both, its callees moved 16 or 32 by region, so no pad reproduces them); by §6 the runner's four call points went cold (`#[cold]`, `#[inline(never)]`) and sitting 3 ran: base A/A 0 `regressed`/`improved`, direct A 0 `regressed` (40 unchanged, 18 noisy), direct B 0 `regressed` (44, 14), candidate A/A 0, every row's digests matching; `flush` p50 Δ −0.008 and +0.030 ms (τ 0.074), `follow` −0.094 and +0.032 (τ 0.054), `integrated` `total` p50 −0.106 and −0.040 (τ 0.23); `just smoke` and `just visual` pass after the cold build. Deviations: four `None` tests a frame, not two (the camera swap needs a point after `PostUpdate` and the holds one before it); `ui: false` hides the `UiTree` from layout to render and `run_frame` clears its retained paint list; the log tally rides the console channel's sink (`logging.rs`), so `tests/logging.rs` is untouched; an explicit sheet `cell` wins over sprite sizes; the runner's constructor, `ended` and `finish` carry `allow(dead_code)` outside `testing` until step 3 calls them | `crates/tungsten/src/script.rs`, `crates/tungsten/src/tests/script.rs`; `perf-runs/20261029-agent-tooling/` (`summary.md`, `summary-r2.md`, `summary-r3.md`, `nm/`, `scripts/`) |
| 3 | 2026-10-29 | passed | `cargo test -p tungsten --locked --lib -- user_dir script` → 28 passed, with `a_script_runs_folder_is_the_environments_over_codes_and_none_without_it` (`TUNGSTEN_USER_DIR` over `UserFolder::At`; none without it, set empty or with an invalid id) and `a_fatal_error_ends_the_report_failed_and_the_report_round_trips`; the umbrella lib 735 passed; clippy clean. GPU, in `templates/basic`, a 45-frame fixture at 640 × 360 (a `move_right` press and release, a capture at 10 and at 20, 30, 40 by `every`, an `entities` assert): exit 0, `jq -r '.passed, .ended, (.captures \| length)'` → `true`, `frames`, `4`, four 640 × 360 PNGs (the title, "Saving is off": no user folder); the same with a failing assert → exit 1, `.passed` false. A first draft asserted `entities > 0` and failed: the title holds no ECS entities, so the fixture asserts `== 0`. `just smoke` → passes; `just visual` → 5 suites ok, no reference changed; `just repo-check` → passes. `git diff --unified=1 HEAD -- crates/tungsten/src/app.rs`: construction (`with_plugins` now wraps `build`, so a setup error writes a failed report), `run` (split into `run_window` and `finish_script`), `FrameClock`'s doc, two env helpers and the window event arms; never `run_frame` or a `stage_*` function. Deviations: dt is pinned by `FrameClock::Pinned` from the redraw arm; `capture.rs` regains `Capture::empty` and `logging.rs` loses step 2's `allow(dead_code)`, files outside the step's list; the LLM index reached its cap exactly (12,288 B) with "the doc map" in its intro, a row outside the touched ones | `templates/basic` fixtures, reports and shots in `perf-runs/20261029-agent-tooling/step3/` |
| 4 | 2026-10-29 | passed | `cargo test -p tungsten-cli --locked` → 53 + 6 passed (options per command, an unknown or misplaced one a usage error and exit 2 end to end, a bad script exit 1 with `steps[0].captur` and no `target/` made, the failure and summary lines, the name filter and text form); `cargo test -p tungsten --locked --lib -- describe` → 3 passed (the kit World's components with the kit's fields, `rigid_body` bare, the probes with docs, a test action, the manifest's sprites and prefabs; a bad file listed, not fatal; under `TUNGSTEN_DESCRIBE` with `TUNGSTEN_USER_DIR` over a damaged `settings.json`, no folder and the file unmoved, while the same build without it quarantines it); the umbrella lib 738 passed. `(cd templates/basic && $T describe --json) \| jq -e '…'` → `true` (52 components, 40 shapes, 9 kinds, 6 probes, 11 sprites, 11 prefabs, 0 errors). GPU, `(cd examples/01_platformer && $T run --script …)`: a 30-frame fixture at 960 × 540 → exit 0, last line `tungsten run: 30 frames, 1 capture, 1 assert: passed`; its failing twin → exit 1, `frame 30: assert frame == 31 failed: actual 30`. `just template-check` → every line OK, with `Describe (components, the entities probe): OK` and `Run --script (30 frames, one capture, one assert): OK`. `just api` → `api/tungsten-cli.txt` (the help), `api/tungsten-kit.txt` +1 (`KitDescriptions::to_json`), `api/tungsten.txt` unchanged; `just api-check` → current; clippy clean. Deviations: `App`'s construction takes an env lookup (`App::build`, crate-private), so the describe test sets no variable; the game's stdout goes to stderr under `describe`, so `--json` pipes clean; `describe <name>` matching nothing exits 1; the LLM index stays at 12,288 B by "Template: shell, sample, seam; `tungsten` CLI", "Overlays, captures" and "Frame order, winit loop, smoke" | fixtures, outputs, the title capture and the template's description in `perf-runs/20261029-agent-tooling/step4/`; `template-check-step4.log` |

## Follow-ups

- **F2.** `ComponentRegistry::count` and the `count` probe cover components registered as themselves (`register::<T>`); a definition's (`register_with`, the kit's and the engine's) needs its build's spawned type recorded at registration, a `Bundle` or registry API addition. Until then a game probe counts them.
- **F1.** The agent memory's scratch-driver recipes (in-game shots, cleanup by fingerprint hash) are replaced by `render` once step 6 lands; H2 (7.4) prunes agent memory.
