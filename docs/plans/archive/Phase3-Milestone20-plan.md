---
status: draft
goal: Ship M20 Scene/State System. `StateStack` + `GameState` drive a `MainMenu -> Gameplay -> Pause -> Gameplay` flow through a single dispatcher system; scene-owned entities auto-despawn on state exit; `scene.json` spawns baseline entities data-driven.
non-goals:
  - UI widget library / menu layout engine
  - Save/load, persistent scene state
  - Scripted transitions other than action-triggered
  - Tween-driven scene fades (M24)
  - Scene hot reload for `scene.json`
  - Nested sub-states / hierarchical state machines
  - Change detection / system run-conditions
files-to-touch:
  - /home/joker/Projects/Tungsten/crates/tungsten-core/src/assets/scene.rs              (new)
  - /home/joker/Projects/Tungsten/crates/tungsten-core/src/assets/mod.rs                (exports)
  - /home/joker/Projects/Tungsten/crates/tungsten-core/src/lib.rs                       (re-exports)
  - /home/joker/Projects/Tungsten/crates/tungsten-core/src/input/action_map.rs          (default bindings)
  - /home/joker/Projects/Tungsten/crates/tungsten/src/state.rs                          (new)
  - /home/joker/Projects/Tungsten/crates/tungsten/src/lib.rs                            (exports)
  - /home/joker/Projects/Tungsten/crates/tungsten/src/app.rs                            (resource+dispatcher wiring)
  - /home/joker/Projects/Tungsten/crates/tungsten/src/asset_loader.rs                   (scene load/spawn helpers)
  - /home/joker/Projects/Tungsten/examples/04_scene_state/Cargo.toml                    (new)
  - /home/joker/Projects/Tungsten/examples/04_scene_state/src/main.rs                   (new)
  - /home/joker/Projects/Tungsten/examples/04_scene_state/src/states.rs                 (new)
  - /home/joker/Projects/Tungsten/examples/04_scene_state/assets/manifest.json          (new)
  - /home/joker/Projects/Tungsten/examples/04_scene_state/assets/scene.json             (new)
  - /home/joker/Projects/Tungsten/examples/04_scene_state/assets/quad.png               (new, copy from ex03 or 1x1)
  - /home/joker/Projects/Tungsten/Cargo.toml                                            (workspace member)
  - /home/joker/Projects/Tungsten/docs/LLM_INDEX.md                                     (subsystem rows)
  - /home/joker/Projects/Tungsten/AGENTS.md                                             (status line)
  - /home/joker/Projects/Tungsten/DECISIONS.md                                          (D-046 entry)
  - /home/joker/Projects/Tungsten/docs/DECISION_INDEX.md                                (D-046 row)
  - /home/joker/Projects/Tungsten/docs/plans/Phase3.md                                  (status + archive pointer)
---

# M20 — Scene/State System

## Core Types (target shape)

Umbrella crate owns the state machine (`Phase3.md` Core Objects table: `StateStack + GameState (tungsten)`). Core owns the scene data model (assets belong in `tungsten-core`).

```rust
// crates/tungsten/src/state.rs
pub type StateId = &'static str;

pub struct StateContext<'a> {
    pub world: &'a mut tungsten_core::World,
    pub state_id: StateId,
}

pub trait GameState: 'static {
    fn id(&self) -> StateId;
    fn on_enter(&mut self, ctx: &mut StateContext);
    fn on_exit(&mut self, ctx: &mut StateContext);
    fn on_pause(&mut self, _ctx: &mut StateContext) {}
    fn on_resume(&mut self, _ctx: &mut StateContext) {}
    fn update(&mut self, world: &mut tungsten_core::World);
}

enum StateCommand { Push(Box<dyn GameState>), Pop, Replace(Box<dyn GameState>) }

pub struct StateStack {
    stack: Vec<Box<dyn GameState>>,
    pending: Vec<StateCommand>,
}
impl StateStack {
    pub fn new() -> Self;
    pub fn request_push(&mut self, s: impl GameState);
    pub fn request_pop(&mut self);
    pub fn request_replace(&mut self, s: impl GameState);
    pub fn active_id(&self) -> Option<StateId>;
    pub fn depth(&self) -> usize;
}

/// Marker on spawned entities owned by a state. Removed with the entity.
#[derive(Debug, Clone, Copy)]
pub struct SceneEntity { pub state_id: StateId }

pub fn state_dispatcher_system(world: &mut tungsten_core::World);
pub fn despawn_scene_entities(world: &mut tungsten_core::World, state_id: StateId);
```

```rust
// crates/tungsten-core/src/assets/scene.rs
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct SceneData { pub entities: Vec<SceneEntry> }
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SceneEntry {
    pub transform: SceneTransform,
    pub sprite: Option<SceneSprite>,
    #[serde(default = "default_visible")] pub visible: bool,
    pub tag: Option<String>,
}
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct SceneTransform { pub position: [f32; 2], #[serde(default)] pub rotation: f32, #[serde(default = "one_scale")] pub scale: [f32; 2] }
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SceneSprite { pub asset_id: String, #[serde(default = "white")] pub color: [u8; 4], #[serde(default)] pub z_order: i32 }
impl SceneData { pub fn load(path: &std::path::Path) -> Result<Self, SceneError>; }
```

## Dispatcher Contract

- Registered as an engine-owned system in `App::new` immediately after `__display_input` (before user systems) so input-driven transitions land before gameplay reads them, and state-owned entities spawned via `CommandBuffer` inside `on_enter` flush in the same frame (per `D-039`).
- Each frame the dispatcher:
  1. Drains `StateStack.pending` in order; for each command, fires `on_exit` / `on_pause` / `on_enter` / `on_resume` per the matrix below. Auto-despawns every `SceneEntity` whose `state_id` equals the exiting state's `id()` **before** the user's `on_exit` runs — user impls may enqueue additional despawns/inserts but must not re-spawn scene entities under the exiting id.
  2. Calls `update(world)` on the top state only (no-op if stack empty).
  3. Writes `HudActiveState(stack.last().map(|s| s.id()).unwrap_or("").to_string())` into the world (M18's `state_provider` at `crates/tungsten/src/debug_hud.rs:243-255` consumes it; empty string suppresses the row).
- Transition matrix:
  - `push(new)`:   `old.on_pause` → push(new) → `new.on_enter`
  - `pop()`:       `old.on_exit`  → pop       → `next.on_resume` (if any)
  - `replace(new)`:`old.on_exit`  → replace   → `new.on_enter`
- `on_pause`/`on_resume` are **no-op defaults** on the `GameState` trait so `Pause` overlays `Gameplay` without despawning the gameplay scene.

### Transition Implementation Pattern (lift-and-restore)

Hooks need `&mut dyn GameState` + `&mut World` simultaneously, but the state lives inside a `StateStack` resource. Use this pattern (drop-in skeleton for `state_dispatcher_system`):

```rust
pub fn state_dispatcher_system(world: &mut World) {
    // 1) Take pending queue only; leave StateStack in world so hooks can
    //    call `world.get_resource_mut::<StateStack>()` to enqueue more.
    let pending: Vec<StateCommand> = match world.get_resource_mut::<StateStack>() {
        Some(s) => std::mem::take(&mut s.pending),
        None => return,
    };

    for cmd in pending {
        match cmd {
            StateCommand::Push(mut new_state) => {
                // on_pause: lift the old top out, call hook, put it back.
                if let Some(mut old) = pop_top(world) {
                    let old_id = old.id();
                    old.on_pause(&mut StateContext { world, state_id: old_id });
                    push_top(world, old);
                }
                let new_id = new_state.id();
                new_state.on_enter(&mut StateContext { world, state_id: new_id });
                push_top(world, new_state);
            }
            StateCommand::Pop => {
                if let Some(mut old) = pop_top(world) {
                    let old_id = old.id();
                    despawn_scene_entities(world, old_id); // before user on_exit
                    old.on_exit(&mut StateContext { world, state_id: old_id });
                    // drop `old`
                }
                if let Some(mut next) = pop_top(world) {
                    let next_id = next.id();
                    next.on_resume(&mut StateContext { world, state_id: next_id });
                    push_top(world, next);
                }
            }
            StateCommand::Replace(mut new_state) => {
                if let Some(mut old) = pop_top(world) {
                    let old_id = old.id();
                    despawn_scene_entities(world, old_id);
                    old.on_exit(&mut StateContext { world, state_id: old_id });
                }
                let new_id = new_state.id();
                new_state.on_enter(&mut StateContext { world, state_id: new_id });
                push_top(world, new_state);
            }
        }
    }

    // 2) update() on top state via the same lift-and-restore dance.
    if let Some(mut top) = pop_top(world) {
        top.update(world);
        push_top(world, top);
    }

    // 3) Mirror active id into HudActiveState.
    let active = world
        .get_resource::<StateStack>()
        .and_then(|s| s.stack.last().map(|t| t.id()))
        .unwrap_or("")
        .to_string();
    world.insert_resource(HudActiveState(active));
}

fn pop_top(world: &mut World) -> Option<Box<dyn GameState>> {
    world.get_resource_mut::<StateStack>()?.stack.pop()
}
fn push_top(world: &mut World, s: Box<dyn GameState>) {
    world.get_resource_mut::<StateStack>().expect("StateStack removed").stack.push(s);
}
```

- Entity-visibility rule: `on_enter` / `on_exit` enqueue into the `CommandBuffer` resource; the engine's post-systems flush (`app.rs:814-820` `RedrawRequested` flow) applies them before extract/render, so the first frame of a state already renders its new scene entities and the last frame of an exiting state already sees its `SceneEntity`s gone.

## Ordered Steps

1. `/home/joker/Projects/Tungsten/crates/tungsten-core/src/assets/scene.rs` — create file. Define `SceneData`, `SceneEntry`, `SceneTransform`, `SceneSprite`, `SceneError` (via `thiserror::Error` with `Io{path,source}`, `Parse{path,source}`). Implement `SceneData::load(&Path)` using `serde_json::from_str`. Defaults: `scale = [1.0, 1.0]`, `color = [255; 4]`, `visible = true`, `rotation = 0.0`. Unit tests: round-trip parse of a minimal fixture string; missing-optional fields get defaults; empty `entities` list valid.

2. `/home/joker/Projects/Tungsten/crates/tungsten-core/src/assets/mod.rs` — add `pub mod scene;` and `pub use scene::{SceneData, SceneEntry, SceneSprite, SceneTransform, SceneError};`.

3. `/home/joker/Projects/Tungsten/crates/tungsten-core/src/lib.rs` — append to the `pub use assets::{…}` list: `SceneData, SceneEntry, SceneError, SceneSprite, SceneTransform`.

4. `/home/joker/Projects/Tungsten/crates/tungsten/src/state.rs` — create file. Implement `StateId`, `SceneEntity`, `StateContext`, `GameState` (with default `on_pause`/`on_resume` returning `()`), private `StateCommand`, `StateStack` (fields: `pub(crate) stack: Vec<Box<dyn GameState>>`, `pub(crate) pending: Vec<StateCommand>` — crate-visible so `state_dispatcher_system` in the same module can mutate them; keep getters `active_id`/`depth` public-facing), `request_push`/`request_pop`/`request_replace` (each `Box::new`s its arg and pushes into `pending`), `state_dispatcher_system` (see pseudocode under "Transition Implementation Pattern" above; use the `pop_top`/`push_top` helpers verbatim), and `despawn_scene_entities`. `despawn_scene_entities` walks `world.query::<SceneEntity>()`, collects `Entity`s whose `state_id == id`, and enqueues `CommandBuffer::despawn` for each via `world.get_resource_mut::<CommandBuffer>().expect("CommandBuffer resource missing")`.

   Add `#[cfg(test)] mod tests;` at the bottom, and create adjacent `crates/tungsten/src/state_tests.rs` (or inline `mod tests { … }`) containing the unit tests listed under **Done-When Checks**. All test helpers must run the engine's flush cycle between `request_*` and any query: `let buf = world.remove_resource::<CommandBuffer>().unwrap(); world.flush(buf); world.insert_resource(CommandBuffer::new());`. Without this, `SceneEntity` despawns enqueued by the dispatcher remain pending and the cleanup assertions will fail.

   Test-only shim: define a `TestState { id: &'static str, hooks_fired: Rc<RefCell<Vec<&'static str>>> }` that records its hook invocations as strings (`"menu:on_enter"`, etc.) and, in its `on_enter`, optionally enqueues a tagged `SceneEntity { state_id: self.id }` spawn via `CommandBuffer`. Tests inspect `hooks_fired` order and the post-flush entity set.

5. `/home/joker/Projects/Tungsten/crates/tungsten/src/lib.rs` — add `pub mod state;` and `pub use state::{GameState, SceneEntity, StateContext, StateId, StateStack, state_dispatcher_system, despawn_scene_entities};`.

6. `/home/joker/Projects/Tungsten/crates/tungsten/src/app.rs` — in `App::new`:
   - After the existing `world.insert_resource(DebugHud::new());` at `app.rs:146`, add `world.insert_resource(StateStack::new());` and `world.insert_resource(HudActiveState::default());` (`HudActiveState` is the placeholder the M18 `state_provider` consumes; it's safe to insert unconditionally — `state_provider` suppresses the row when the inner string is empty).
   - After the two existing `add_engine_system` calls at `app.rs:186-187` (`__hud_toggle`, `__display_input`), append `app.add_engine_system("__state_dispatcher", state_dispatcher_system);`.
   - Add `use crate::state::{state_dispatcher_system, StateStack};` alongside the existing `use crate::debug_hud::…` block. Do NOT edit the frame-order comment block — the dispatcher is just another system; frame order is unchanged.

7. `/home/joker/Projects/Tungsten/crates/tungsten/src/asset_loader.rs` — add:
   - `pub fn load_scene(path: &Path) -> anyhow::Result<SceneData>` — thin wrapper over `SceneData::load` that maps `SceneError` into `anyhow::Error` (use `?` with `anyhow::Context` to include the path).
   - `pub fn spawn_scene(world: &mut World, data: &SceneData, state_id: StateId)` — iterates `data.entities`. For each entry: `let buf = world.get_resource_mut::<CommandBuffer>().expect("CommandBuffer resource missing"); let e = buf.spawn();` then `buf.insert_pending(e, Transform { position: Vec2::from(entry.transform.position), rotation: entry.transform.rotation, scale: Vec2::from(entry.transform.scale) });` — and conditionally insert `Sprite` / `Tag`, always insert `Visibility { visible: entry.visible }` and `SceneEntity { state_id }`. Do **not** validate `sprite.asset_id` against `AssetRegistry` at spawn time — missing sprite IDs fall through to the sprite-extract default behaviour (a log warning), matching how `TilemapInstance` treats unresolved tile ids. Asset-id validation is an explicit non-goal for M20.

   Imports: `crate::state::{SceneEntity, StateId}`, `tungsten_core::{Sprite, Tag, Transform, Visibility, CommandBuffer, SceneData}`, `glam::Vec2`.

8. `/home/joker/Projects/Tungsten/crates/tungsten-core/src/input/action_map.rs` — extend `default_map()` with three engine-neutral defaults so the example works out-of-the-box without an edited `input.json`:
   - `"state_start"` → `Binding::Key { code: KeyCode::Enter }`
   - `"state_pause"` → `Binding::Key { code: KeyCode::KeyP }`
   - `"state_back"`  → `Binding::Key { code: KeyCode::Backspace }`

   Update the existing `default_map_has_platformer_and_engine_actions` test to include the three new action names.

9. `/home/joker/Projects/Tungsten/examples/04_scene_state/Cargo.toml` — create. Mirror `examples/03_component_sprites/Cargo.toml` byte-for-byte except the name:
   ```toml
   [package]
   name = "example-04-scene-state"
   version.workspace = true
   edition = "2021"
   publish = false

   [dependencies]
   tungsten = { workspace = true }
   tungsten-core = { workspace = true }
   tungsten-render = { workspace = true }
   glam = { workspace = true }
   anyhow = { workspace = true }
   log = { workspace = true }
   env_logger = { workspace = true }
   ```
   (No `[[bin]]` stanza — Cargo picks up `src/main.rs` by default, matching ex03.)

10. `/home/joker/Projects/Tungsten/examples/04_scene_state/assets/quad.png` — reuse the existing `examples/03_component_sprites/assets/ex03_quad.png` asset (copy to this path) to avoid adding new art.

11. `/home/joker/Projects/Tungsten/examples/04_scene_state/assets/manifest.json` — register a single sprite, e.g.:
    ```json
    { "sprites": [ { "id": "ex04_quad", "path": "quad.png", "filter": "nearest" } ] }
    ```

12. `/home/joker/Projects/Tungsten/examples/04_scene_state/assets/scene.json` — author 3 `SceneEntry`s (two sprites + one tagged origin marker) to validate `spawn_scene`, e.g.:
    ```json
    { "entities": [
      { "transform": { "position": [-64, 0] }, "sprite": { "asset_id": "ex04_quad", "color": [255,120,120,255], "z_order": 0 } },
      { "transform": { "position": [ 64, 0], "rotation": 0.5 }, "sprite": { "asset_id": "ex04_quad", "color": [120,120,255,255], "z_order": 0 } },
      { "transform": { "position": [0, -80], "scale": [2.0, 0.25] }, "sprite": { "asset_id": "ex04_quad", "color": [200,200,200,255], "z_order": 0 }, "tag": "gameplay_marker" }
    ] }
    ```

13. `/home/joker/Projects/Tungsten/examples/04_scene_state/src/states.rs` — create. Each state distinguishes itself visually with a **sprite quad** (not text) so the default sprite extract path is sufficient — no custom `extract_text` plumbing is required for the demo.

    Define three types implementing `GameState`:
    - `#[derive(Default)] struct MainMenuState;`
      - `id() -> "menu"`
      - `on_enter(ctx)`: via `CommandBuffer` spawn one entity with `Transform::from_position(Vec2::ZERO)`, `Sprite { asset_id: "ex04_quad".into(), color: [200, 80, 80, 255], z_order: 0 }`, `Visibility { visible: true }`, `Tag::new("menu_marker")`, `SceneEntity { state_id: "menu" }`.
      - `on_exit`: no-op (dispatcher already auto-despawns `SceneEntity`s).
      - `update(world)`: if `ActionMap::just_pressed(input, "state_start")`, `world.get_resource_mut::<StateStack>().unwrap().request_replace(GameplayState::new("examples/04_scene_state/assets/scene.json"))`.
    - `struct GameplayState { scene_path: PathBuf }` with `pub fn new(path: impl Into<PathBuf>) -> Self`.
      - `id() -> "gameplay"`
      - `on_enter(ctx)`: `let scene = SceneData::load(&self.scene_path).expect("scene.json missing"); asset_loader::spawn_scene(ctx.world, &scene, "gameplay");`
      - `update(world)`: `state_pause` ⇒ `request_push(PauseState::default())`; `state_back` ⇒ `request_replace(MainMenuState::default())`.
    - `#[derive(Default)] struct PauseState;`
      - `id() -> "pause"`
      - `on_enter(ctx)`: spawn one overlay quad: `Transform` at `Vec2::new(0.0, -40.0)`, `Sprite { asset_id: "ex04_quad".into(), color: [40, 40, 40, 200], z_order: 100 }`, `Visibility { visible: true }`, `Tag::new("pause_overlay")`, `SceneEntity { state_id: "pause" }`.
      - `update(world)`: `state_pause` ⇒ `request_pop`.

    Gameplay entities persist across the push-Pause / pop-Pause sequence because Pause only triggers `on_pause` on Gameplay (no auto-despawn), and only `pause`-tagged `SceneEntity`s are cleaned on Pause's `on_exit`.

14. `/home/joker/Projects/Tungsten/examples/04_scene_state/src/main.rs` — `fn main() -> anyhow::Result<()>`:
    - `env_logger::init()`
    - `let config = tungsten::core::Config::load("tungsten.json")?;`
    - `let mut app = tungsten::App::new(config)?;`
    - `app.on_startup(|world, renderer| { … })`:
        - Load the **workspace-root** shared manifest for fonts (`mono` is required by the HUD): `let root = ResolvedManifest::load("assets/manifest.json").expect("root manifest"); asset_loader::load_fonts(&root, world, renderer).expect("fonts");` — do NOT call `asset_loader::load_all` (it overwrites registries that the local sprite load below populates).
        - Load the local sprite manifest: `let local = ResolvedManifest::load("examples/04_scene_state/assets/manifest.json").expect("local manifest"); asset_loader::load_sprites(&local, world, renderer).expect("sprites");`
        - Enable the HUD: `world.get_resource_mut::<DebugHud>().unwrap().enabled = true;`
        - Push the initial state: `world.get_resource_mut::<StateStack>().unwrap().request_push(MainMenuState::default());` — it lands on the first dispatcher tick.
    - `app.run()`

    No custom `set_extract_*` calls: the engine's default sprite extract (installed in `App::run -> install_default_extracts`) picks up every `Sprite` + `Transform` + `Visibility` combo, including the scene-spawned quads.

15. `/home/joker/Projects/Tungsten/Cargo.toml` — append `"examples/04_scene_state"` to the `[workspace] members` list.

16. `/home/joker/Projects/Tungsten/DECISIONS.md` — add `D-046 — M20 scene/state system` before the "When to open full DECISIONS" section. Cover: (a) `StateStack`+`GameState` live in umbrella per Phase 3 Core Objects table, with core hosting `SceneData` since asset parsing belongs in `tungsten-core`; (b) single `state_dispatcher_system` registered as engine-owned before user systems (risk mitigation from `Phase3.md` M20 risk: "runtime system-list churn; prefer a single dispatcher system"); (c) scene-owned entity cleanup uses a `SceneEntity(state_id)` marker despawned through `CommandBuffer` in the transition, inheriting `D-039` visibility rules; (d) transition matrix `push/pop/replace` with `on_pause`/`on_resume` default no-ops so Pause doesn't tear down Gameplay; (e) `scene.json` is a minimal schema reusing `D-042` components (`Transform`, `Sprite`, `Visibility`, `Tag`) and validated at load via the same `AssetRegistry` path the manifest uses for sprite IDs; (f) action defaults for `state_start`/`state_pause`/`state_back` are added to `ActionMap::default_map()` so examples work with no `input.json` edits while keeping engine-owned keys distinct from the `engine_*` set.

17. `/home/joker/Projects/Tungsten/docs/DECISION_INDEX.md` — add a row under "ECS / Runtime Flow":
    `| D-046 | Scene/state system: single dispatcher, SceneEntity auto-cleanup via CommandBuffer, scene.json reuses M15 components. |`

18. `/home/joker/Projects/Tungsten/docs/LLM_INDEX.md` — in "Subsystem Map" add:
    `| Scene/state stack (M20) | [crates/tungsten/src/state.rs](...), [crates/tungsten-core/src/assets/scene.rs](...), [examples/04_scene_state/](...) |`
    In "Task Map" add a row for "Change state transitions, scene spawn, or SceneEntity cleanup" pointing to `crates/tungsten/src/state.rs`, `crates/tungsten/src/asset_loader.rs`, and `D-046`.

19. `/home/joker/Projects/Tungsten/AGENTS.md` — update the "What Tungsten Is" paragraph: replace `Phase 3 Milestone 19 complete` with `Phase 3 Milestone 20 complete`, append `a state/scene dispatcher (StateStack + GameState + scene.json) with scene-owned entity auto-cleanup,` to the capabilities list.

20. `/home/joker/Projects/Tungsten/docs/plans/Phase3.md` — in "Current Status" change `v0.16.0` → `v0.17.0`, set `Completed milestones` to include `M20 — Scene/State System`, move `Next recommended milestone` to `M21 — Debug Tooling`. In the M20 section add the header line `> **Status: complete** (`v0.17.0`, `<fill date when merged>`)` and a pointer `Detailed implementation plan at [`docs/plans/Phase3-Milestone20-scene-state-system.md`](Phase3-Milestone20-scene-state-system.md).`. Check off the Phase 3 Done-When item `Multi-screen game loop (menu / gameplay / pause) ships without custom extract plumbing`.

21. Run validation in order:
    - `cargo fmt --all`
    - `cargo test --workspace` — must pass, including new unit tests in `state.rs`, `scene.rs`, and the amended `default_map_has_platformer_and_engine_actions` test.
    - `cargo build --workspace` — confirms `example-04-scene-state` compiles.
    - `./scripts/smoke-examples.sh` — all four examples (01, 02, 03, 04) boot and exit cleanly under `TUNGSTEN_SMOKE_FRAMES=3`.

## Done-When Checks

- `cargo test --workspace` green, with at least these new tests passing:
  - `tungsten_core::assets::scene::tests::load_parses_minimal_fixture`
  - `tungsten_core::assets::scene::tests::defaults_fill_missing_fields`
  - `tungsten::state::tests::push_fires_on_pause_then_on_enter`
  - `tungsten::state::tests::pop_fires_on_exit_then_on_resume`
  - `tungsten::state::tests::replace_fires_on_exit_then_on_enter`
  - `tungsten::state::tests::scene_entities_despawn_on_exit_through_command_buffer` — **must** insert a `CommandBuffer` into the test `World`, call the dispatcher, then run `let buf = world.remove_resource::<CommandBuffer>().unwrap(); world.flush(buf); world.insert_resource(CommandBuffer::new());` before asserting the entity count drops to zero (otherwise the despawn is still queued).
  - `tungsten::state::tests::push_does_not_despawn_paused_states_scene_entities` — regression guard: `Gameplay` pushes `Pause`, then asserts that `SceneEntity { state_id: "gameplay" }` rows still exist after flush.
  - `tungsten::state::tests::update_only_runs_on_top_state` — after push, only the new top's `update` fires for the next dispatcher tick; the paused state records no `update` call.
  - `tungsten::state::tests::hud_active_state_mirrors_top_state_id`
  - `tungsten::state::tests::hud_active_state_cleared_when_stack_empty` — after popping the last state, `HudActiveState.0` is empty and the HUD `state` row is suppressed by `state_provider`.
  - `tungsten_core::input::action_map::tests::default_map_has_platformer_and_engine_actions` (amended with the three new `state_*` actions)
- `./scripts/smoke-examples.sh` passes for `01_platformer`, `02_sprite_stress`, `03_component_sprites`, `04_scene_state` (with default `TUNGSTEN_SMOKE_FRAMES=3`).
- Manual run `cargo run -p example-04-scene-state`:
  - Frame 1 shows the red menu marker quad; HUD `state` row reads `menu`.
  - `Enter` → `replace` transition: menu quad despawns, three scene quads from `scene.json` appear; HUD `state` reads `gameplay`.
  - `P` → `push Pause`: scene quads remain, grey overlay quad appears; HUD `state` reads `pause`.
  - `P` again → `pop`: overlay quad despawns, scene quads remain unchanged (no re-entry flicker); HUD `state` reads `gameplay`.
  - `Backspace` → `replace Menu`: scene quads despawn, red menu marker reappears; HUD `state` reads `menu`.
- `docs/plans/Phase3.md` M20 row shows `Status: complete` and the Phase 3 Done-When `Multi-screen game loop` item is checked.
- `DECISIONS.md` contains a `D-046` entry and `docs/DECISION_INDEX.md` lists the matching row (the workspace coverage test in `tungsten_core` enforces this pairing — it must stay green).
