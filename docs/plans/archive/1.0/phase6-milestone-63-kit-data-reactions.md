# Phase 6 milestone 63: kit data and reactions (W13h)

- **status:** done
- **goal:** The kit catalogue's A ([w13-kit](../../1.0/w13-kit.md) §7): `Stats` with ordered modifiers, which the kit's movers, health and cooldowns read when present; cues, the one way to react to events in data (an entity's `cues`, the `cue_rules` kind and `CuesOnEvent<E>`), with A's actions, `min_interval` on sounds and the `label` action's timed world labels; `Cooldown`, `Counters`, `Flags`, `KitRng` and `SpawnChildren`; the event trace with `KitDebugPlugin`'s view; recipes in two genres for each item, and the template's cues.
- **non-goals:** The damage pipeline, the death policy, hit shapes, `roll` and `loot_tables` (W13j); effects, abilities, levels and the `effect` action (W13k); motion items (W13i); the cue events `spawned`, `expired`, `healed`, `hit`, `picked_up` and E's; a `label_styles` kind; a trace window or world-anchored trace; a `KitDebug` hotkey; nearest-name suggestions; a command flush per fixed step; the examples (W12b); the measured bench run (W13i); any change to core, render or physics.
- **files to touch:** Kit sources under `crates/tungsten-kit/src/`: new `foundation/{stats,rng,trace}.rs`, `react/{mod,cues,counters}.rs`, `lifecycle/{cooldown,children}.rs`, `present/labels.rs`; changed `lib.rs`, `foundation/{mod,describe,debug,plugins,phases,tags}.rs`, `lifecycle/{mod,lifetime,despawn,spawner}.rs`, `present/{mod,sound}.rs`, `transform/hierarchy.rs`, `damage/health.rs`, `motion/{top_down,platformer}.rs`, `overlap/zone.rs` (steps 1–2). Kit tests: new `src/tests/{stats,rng,trace,cues,counters,cooldown,children,labels}.rs`; changed `src/tests/{describe,inspect,phases,sound,debug,hierarchy,health,top_down,platformer,zone,lifetime,despawn,spawner}.rs` (steps 1–2). `crates/tungsten-kit/benches/kit.rs` (step 3). Umbrella: `crates/tungsten/src/tests/kit.rs` (steps 1–2), `crates/tungsten/src/tests/kit_recipes/{coverage,mod,platformer,top_down,shmup,grid,determinism}.rs` (steps 1, 3), fixtures under `crates/tungsten/tests/fixtures/kit/` (§2.9; steps 1, 3). Template: `templates/basic/tests/game.rs` (steps 1, 4), `templates/basic/assets/prefabs/player.json`, `templates/basic/AGENTS.md`, `templates/basic/README.md`, `docs/getting-started.md` (step 4). Snapshots: `api/tungsten-kit.txt`, `api/tungsten-kit-components.txt` (step 1, checked in 5). Records: `DECISIONS.md`, `docs/DECISION_INDEX.md`, `DESIGN.md`, `docs/LLM_INDEX.md`, `docs/plans/1.0/w04-api-freeze.md`, `docs/known-issues.md`, `docs/plans/1.0/w13-kit.md` (step 5). Release: `CHANGELOG.md`, `DESIGN.md`, `Cargo.toml`, `Cargo.lock`, the register row in `docs/plans/1.0/implementation-plan.md`, `docs/plans/1.0/README.md`'s four owned fields, this plan (step 6).
- **ordered steps:** (1) The API, the data forms and the state items. (2) Reactions: the cue system, labels and the trace. (3) Recipes, the kit hash and the bench. (4) The template and the guide. (5) Records. (6) Release 0.83.
- **done-when:** Every step's evidence row quoted, or the step marked skipped with its reason; workflow §5 closed (§8); the release checks pass after the cut, with the gates no step ran; the preflight prints the command block and the post-merge block.

Candidate W13h ([implementation plan](../../1.0/implementation-plan.md) §4 card, level C), milestone M63, release 0.83. Written 2026-10-16 at `28e802a` on branch `0.83`. Runs under the [tungsten-milestone skill](../../../../.claude/skills/tungsten-milestone/SKILL.md) and [workflow](../../1.0/workflow.md) §4–§6; this plan records only where it differs.

## Context digest

- **Why now.** W13h is the register's first open stop, after W13g (M62, 0.82, `D-172`). The catalogue orders F1, F2, F3, A, M, C, E; C's top-down recipe ("counts kills, bursts the enemy, shows the damage and rolls a drop through cues alone") and E's effects build on A's cues, counters and stats.
- **What it stands on.** `EventCursor` (`D-170`), spawn inheritance at the flush (`CommandBuffer::spawn_prefab_from`, `D-170`), the `data` section with `DataRegistry` and `register_data_kind_with_assets` (`D-171`), and F3's phases, `Tags`/`TagFilter`, `Source`, scene components, `KitDescriptions`, the recipe harness, the coverage test and the kit hash `0x2b64_8d77_6dff_9170` (`D-172`). Core's `WorldLabels` lasts one frame (`D-161`); `ShakeEvent` and `SquashEvent` are core's (`D-073`).
- **The card.** New kit modules for stats, cues, cooldowns, counters, flags, the random stream, child spawns, timed labels and the trace; `KitDebugPlugin`'s trace view; the template's cues; fixtures. Decisions: the stats model; the cue vocabulary and its boundary. New surface; new scene names and the `stats` and `cue_rules` kinds. Checks: recipes per item in two genres; the determinism hash unchanged or recorded; a cue-rule recipe counts kills and shows damage with no prefab field; smoke. Level C: an API sketch (§2) and a critique of step 1's API diff before the rest runs.
- **What it answers.** Catalogue Q3 (stat keys), Q13 (cue order) and Q16 (damage numbers), and the known issue's "a flush per step is a core question for W13h's plan" (`docs/known-issues.md:51`): not here (Q16 below).
- **Cost shape.** The umbrella's default `kit` feature compiles the kit into every example (`crates/tungsten/Cargo.toml:10`), but no example or perf workload adds a kit plugin, and `DefaultPlugins` registers no kit system, so no perf-suite row runs kit code and no capture runs; `DefaultPlugins` and the physics hashes must not move. 27 new public names, six scene names, two kinds, 13 recipes.

## 1. Audit

Read at `28e802a` (0.82). Paths under `crates/tungsten-kit/src/` unless named.

| # | Finding | Evidence |
| --- | --- | --- |
| A1 | `Damaged { entity, amount, killed }` carries no position or source; `damage` sends it from any stage when `HealthPlugin` registered the queue. A has no damage source, so a kill count must be the game's, not the killer's | `damage/health.rs:226-236`, `:262-286` |
| A2 | `ZoneEntered { zone, entity, position }` is sent in `kit_overlap` for an entrant the zone's filter passes; a bodiless entity never enters | `overlap/zone.rs:97-106`, `:184-209`, `:24-28` |
| A3 | `SoundOnEvent` reads through an `EventCursor` in `kit_present` and plays every event, with no interval | `present/sound.rs:150-191` |
| A4 | `WorldLabels` lasts one frame: the extract drains it each frame without checking the font; the App inserts it; a label takes a font ID, a size, an sRGB `[u8; 4]` colour, an offset in UI units and a scale | `tungsten-core/src/text.rs:474-497`, `:558-583`; `tungsten/src/extract.rs:175-238`; `tungsten/src/app.rs:308` |
| A5 | `shake_tick_system` and `squash_stretch_trigger_system` read `iter_current` in `post_update`, before `kit_present`: a shake sent in `kit_present` is never read, one sent in a fixed step is read the same frame | `tungsten/src/game_feel.rs:26-43`, `:53-60`; the post_update order in `templates/basic/tests/game.rs:105-107` |
| A6 | The spawner prepares a prefab and queues `spawn_prefab_from(spawner, prepared)`; a failure logs once | `lifecycle/spawner.rs:233-272` |
| A7 | Spawn inheritance applies only at the flush; a source despawned by an earlier command drops the spawn; `PreparedPrefab::with` adds a bundle; `CommandBuffer::call` runs in queue order at the flush | `tungsten-core/src/assets/prefab.rs:334-341`, `:746-750`; `tungsten-core/src/ecs/command_buffer.rs:249-257` |
| A8 | `spawn_child` is immediate and is the only setter of `Parent` (a private field); the propagation walks from roots through `Children`, so a child missing from its parent's list is never placed; the first pass already scans every `Parent` for dead parents | `transform/hierarchy.rs:30-35`, `:91-127`, `:278-326` |
| A9 | Inside a phase, systems run in registration order; `KitPlugins::set()` ends with `KitDebugPlugin`; the resolved stages with `DefaultPlugins` are pinned in the umbrella's and the template's tests | `foundation/phases.rs:39-47`, `:150-160`; `foundation/plugins.rs:56-73`; `tungsten/src/tests/kit.rs:34-40`; `templates/basic/tests/game.rs:102-108` |
| A10 | `KitDescriptions` holds components and one summary line per kind; `FieldType` has no signed integer and `Object` is literal text, so no nested shape is described or tested; the tests pin 13 descriptions, 1 kind, 15 numeric fields and the enum variant count | `foundation/describe.rs:231-268`, `:317-395`; `tests/describe.rs:69`, `:124-128`, `:203`, `:213-214`, `:230`; `tests/inspect.rs:62` |
| A11 | The coverage test pins 13 scene names and 1 kind; the determinism test hashes each entity's index, point, health and tags after four fixed scripts | `tungsten/src/tests/kit_recipes/coverage.rs:80-82`; `…/determinism.rs:15`, `:28-63`, `:65-110` |
| A12 | Grid's rogue and rats are bodiless, so no grid recipe can use a zone; platformer, top-down and shmup bodies are kinematic | fixtures `grid/prefabs/rogue.json`, `rat.json`; `top_down/prefabs/hero.json` |
| A13 | `WorldRngSeed` advances on every derive and particle emitters derive lazily at their first tick; core has `Pcg32` and `splitmix64` | `tungsten-core/src/assets/particle.rs:470-486`; `tungsten/src/particles.rs:116-128`; `tungsten-core/src/rng.rs:10-62` |
| A14 | `Time` has `frame()` and the frame's step count, no step index; `Timer` has no serde | `tungsten-core/src/time.rs:246`, `:335`, `:403-404` |
| A15 | `Health`, `TopDownMover` and `PlatformerController` keep `max`, `speed` and `run_speed` private, set only by constructors; `start_flash` is private | `damage/health.rs:78-83`, `:414`; `motion/top_down.rs:110-115`; `motion/platformer.rs:166-168` |
| A16 | A load check gets `&World` only: `SpawnContext::world` and an asset check's `&World`; a check's error is a `MissingAsset { field, kind, id }`, printed "unknown <kind> `<id>`" with the field | `tungsten-core/src/component_registry.rs:453`, `:464-472`, `:515` |
| A17 | Core's nearest-name helper is crate-private, so the kit cannot suggest names | `tungsten-core/src/assets/data.rs:1058` |
| A18 | `lifetime_tick` queues a finished lifetime's despawn again each step until the flush; `despawn_outside` queues per step | `lifecycle/lifetime.rs:137-155`; `lifecycle/despawn.rs:229-263` |
| A19 | `squash_stretch`'s triggers are `on_land`, `on_hit`, `on_pickup`, `manual`; `GameFeelPlugin` registers both event queues | `tungsten/src/asset_loader/scene.rs:322-330`; `tungsten/src/plugins.rs:206-207` |
| A20 | The harness exposes the frame's drawn world labels and audio commands | `tungsten/src/testing.rs:274`, `:329-332` |
| A21 | The template's player prefab has no cues; its font is `sans`; its schedule snapshot and damage test pin today's behaviour | `templates/basic/assets/prefabs/player.json`; `templates/basic/assets/manifest.json`; `templates/basic/tests/game.rs:102-108`, `:248-275` |
| A22 | The index is 5 bytes under its 12,288-byte budget; its kit row routes the whole kit folder | `docs/LLM_INDEX.md:14`; `scripts/check-agent-context.py:25` |
| A23 | `w13-kit.md:495-507` holds a pasted fragment (a heading that starts `## Follow-ups/{`, a copy of the catalogue's round-2 table and an `## Evidence log`), from 0.80 and 0.81; §"Critique and evidence" says the archived catalogue holds them | `docs/plans/1.0/w13-kit.md:458-460`, `:495-507` |
| A24 | The catalogue fixes A's shape loosely: §3's vocabulary and boundary, the A row, Q3, Q13, Q16, and the phase table's "stats" in `kit_resolve` | `docs/plans/1.0/w13-kit.md:99-107`, `:153-172`, `:268`, `:344`, `:354`, `:357` |

## 2. Design

### 2.1 API sketch

Every type keeps private fields behind constructors and `with_*` builders (`D-155`); data types derive `Serialize` and `Deserialize` in their data shape; events are `#[non_exhaustive]` with `new`. All re-exported at the kit root and in `prelude` (constants at the root only).

```rust
// foundation/stats.rs
pub const STATS_APPLY: &str = "stats_apply";
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Stats { /* entries sorted by name: base, bounds, value; modifiers in add order */ }
impl Stats {
    pub const MOVE_SPEED: &'static str = "move_speed";       // also MAX_HEALTH, DAMAGE,
    pub const WELL_KNOWN: [&'static str; 7];                 // COOLDOWN_RATE, AREA, PICKUP_RADIUS,
    pub fn new() -> Self;                                    // PROJECTILE_SPEED
    pub fn with(self, stat: &str, base: f32) -> Self;        // panics: bad name, not finite
    pub fn with_bounds(self, stat: &str, min: f32, max: f32) -> Self; // a game stat's clamp
    pub fn set_base(&mut self, stat: &str, base: f32);
    pub fn base(&self, stat: &str) -> Option<f32>;
    pub fn get(&self, stat: &str) -> Option<f32>;            // the current value, never computed on read
    pub fn add_modifier(&mut self, stat: &str, source: &str, modifier: Modifier);
    pub fn remove_modifiers(&mut self, source: &str) -> usize;
    pub fn iter(&self) -> impl Iterator<Item = (&str, f32)>;
}
#[non_exhaustive] #[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Modifier { Add(f32), Pct(f32), Mul(f32), Override(f32) }   // snake_case in data
#[derive(Debug, Default, Clone)] pub struct StatsPlugin { /* build declarations */ }
impl StatsPlugin { pub fn new() -> Self; pub fn with_stat(self, name: &str, min: f32, max: f32) -> Self; }

// foundation/rng.rs
#[derive(Debug, Clone)] pub struct KitRng { /* Pcg32, seed, draws */ }
impl KitRng {
    pub const DEFAULT_SEED: u64;
    pub fn seeded(seed: u64) -> Self;  pub fn reseed(&mut self, seed: u64);
    pub fn seed(&self) -> u64;  pub fn draws(&self) -> u64;
    pub fn next_u32(&mut self) -> u32;  pub fn unit(&mut self) -> f32;   // [0, 1)
    pub fn range(&mut self, lo: f32, hi: f32) -> f32;  pub fn index(&mut self, len: usize) -> usize;
}                                                       // Default = seeded(DEFAULT_SEED)

// foundation/trace.rs
pub const KIT_TRACE_STEP: &str = "kit_trace_step";
#[derive(Debug)] pub struct KitTrace { /* ring, capacity, steps, pushed */ }
impl KitTrace {
    pub const DEFAULT_CAPACITY: usize = 256;
    pub fn new(capacity: usize) -> Self;  pub fn capacity(&self) -> usize;  // 0 records nothing
    pub fn iter(&self) -> impl Iterator<Item = &TraceEntry>;               // oldest first
    pub fn about(&self, entity: Entity) -> impl Iterator<Item = &TraceEntry>; // entity or other
    pub fn dropped(&self) -> u64;  pub fn steps(&self) -> u64;  pub fn clear(&mut self);
}
#[non_exhaustive] #[derive(Debug, Clone, PartialEq)]
pub struct TraceEntry { pub frame: u64, pub step: u64, pub entity: Entity, pub kind: TraceKind }
#[non_exhaustive] #[derive(Debug, Clone, PartialEq)]
pub enum TraceKind {
    Event { name: &'static str, other: Option<Entity>, amount: Option<f32> },
    Cue { event: &'static str, action: &'static str, rule: Option<String> },
    Skipped { event: &'static str, action: &'static str, why: &'static str },
    Spawn { prefab: String, by: &'static str },
    SpawnFailed { prefab: String, by: &'static str, error: String },
    Despawn { by: &'static str },
}
// impl Display for TraceEntry: "frame 12 step 40: 7v0 damaged 3 (killed)"

// react/counters.rs: each type is a component and, for the game's own, a World resource
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Counters { /* sorted (name, i64) */ }
impl Counters { pub fn new() -> Self; pub fn with(self, name: &str, value: i64) -> Self;
    pub fn get(&self, name: &str) -> i64; pub fn add(&mut self, name: &str, by: i64) -> i64;
    pub fn set(&mut self, name: &str, value: i64); pub fn iter(&self) -> impl Iterator<Item = (&str, i64)>; }
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Flags { /* sorted names */ }
impl Flags { pub fn new() -> Self; pub fn with(self, name: &str) -> Self; pub fn has(&self, name: &str) -> bool;
    pub fn set(&mut self, name: &str, on: bool) -> bool; pub fn iter(&self) -> impl Iterator<Item = &str>; }
pub struct CountersPlugin;  // `counters`, `flags`; inserts the World's Counters and Flags

// lifecycle/cooldown.rs
pub const COOLDOWN_TICK: &str = "cooldown_tick";
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Cooldown { /* seconds, jitter, left, remaining, rate, fresh */ }
impl Cooldown { pub fn new(seconds: f32) -> Self; pub fn with_jitter(self, seconds: f32) -> Self;
    pub fn with_count(self, count: u32) -> Self; pub fn restart(&mut self);
    pub fn seconds(&self) -> f32; pub fn left(&self) -> f32; pub fn remaining(&self) -> Option<u32>; }
#[non_exhaustive] #[derive(Debug, Clone, Copy, PartialEq)]
pub struct CooldownElapsed { pub entity: Entity }        // CooldownElapsed::new(entity)
pub struct CooldownPlugin;

// lifecycle/children.rs
pub const SPAWN_CHILDREN: &str = "spawn_children";
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SpawnChildren { /* (prefab, offset) in order */ }
impl SpawnChildren { pub fn new() -> Self; pub fn with_child(self, prefab: impl Into<String>, offset: Vec2) -> Self;
    pub fn children(&self) -> impl Iterator<Item = (&str, Vec2)>; }
pub struct SpawnChildrenPlugin;

// react/cues.rs, present/labels.rs
pub const CUES: &str = "cues";  pub const LABEL_TICK: &str = "label_tick";
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Cues { /* (event, actions) */ }
impl Cues { pub fn new() -> Self; pub fn on(self, event: &str, actions: impl IntoIterator<Item = CueAction>) -> Self;
    pub fn actions(&self, event: &str) -> &[CueAction]; }
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CueAction { /* private kind */ }
impl CueAction {
    pub fn prefab(id: impl Into<String>) -> Self;  pub fn despawn() -> Self;
    pub fn counter_add(name: &str, by: i64) -> Self;  pub fn counter_set(name: &str, value: i64) -> Self;
    pub fn flag(name: &str, on: bool) -> Self;
    pub fn for_other(self) -> Self;  pub fn for_game(self) -> Self; // `target`: counter, flag; despawn other only
    pub fn sound(id: impl Into<String>) -> Self;  pub fn with_min_interval(self, seconds: f32) -> Self;
    pub fn shake(trauma: f32) -> Self;  pub fn squash(trigger: SquashTrigger) -> Self;
    pub fn flash() -> Self;  pub fn label(style: LabelStyle) -> Self;
    pub fn name(&self) -> &'static str;
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LabelStyle { /* font, size, color, seconds, rise, pop */ }
impl LabelStyle { pub fn new(font: impl Into<String>) -> Self; pub fn with_size(self, ui_units: f32) -> Self;
    pub fn with_color(self, linear: [f32; 4]) -> Self; pub fn with_seconds(self, s: f32) -> Self;
    pub fn with_rise(self, ui_units: f32) -> Self; pub fn with_pop(self, scale: f32) -> Self; }
#[non_exhaustive] #[derive(Debug, Clone, Copy, PartialEq)]
pub struct CueEvent { pub entity: Entity, pub position: Vec2, pub amount: Option<f32>, pub other: Option<Entity> }
impl CueEvent { pub fn new(entity: Entity, position: Vec2) -> Self;
    pub fn with_amount(self, amount: f32) -> Self; pub fn with_other(self, other: Entity) -> Self; }
pub struct CuesOnEvent<E> { /* name, map */ }
impl<E: 'static> CuesOnEvent<E> { pub fn new(name: &'static str, map: fn(&E) -> Option<CueEvent>) -> Self; }
pub struct CuesPlugin;

// present/sound.rs (added)
impl<E: 'static> SoundOnEvent<E> { pub fn with_min_interval(self, seconds: f32) -> Self; }

// foundation/describe.rs (added): FieldType::I64 { min: Option<i64>, max: Option<i64> },
// FieldType::Shape(&'static str); ComponentDescription::one_of(self) and is_one_of(&self);
// KitDescriptions::add_shape(description), shapes(), get_shape(name)
```

`CuesOnEvent<E>`, like `SoundOnEvent<E>`, takes a game type, so it stays out of `KitPlugins::set()`; it panics at build on a name that is not `[a-z][a-z0-9_]*`, is one of the kit's events or was registered before. `KitPlugins::set()` becomes: tags, source, hierarchy, animation, lifetime, despawn outside, spawner, health, path follower, the platformer controller, the top-down mover, zone, action input, the camera target, stats, counters, cooldown, spawn children, cues, then the debug drawing, last.

### 2.2 Data forms

```jsonc
"stats":          { "move_speed": 160, "armor": 2 }            // base values; names declared
"cues":           { "damaged": [{ "flash": {} }], "killed": [{ "prefab": "puff" }, { "despawn": {} }] }
"cooldown":       { "seconds": 0.5, "jitter": 0.25, "count": 3 }  // jitter 0, count null by default
"counters":       { "coins": 0 }
"flags":          ["door.locked"]
"spawn_children": [{ "prefab": "glow", "offset": [0, -12] }]    // offset [0, 0] by default
// data.stats entries (kind `stats`): { "armor": { "min": 0, "max": 10 }, "luck": {} }
// data.cue_rules entries (kind `cue_rules`): a list of rules
[{ "on": "killed", "filter": { "all": ["team.enemy"] }, "do": [{ "counter": { "name": "kills", "add": 1, "target": "game" } }] },
 { "on": "damaged", "filter": { "all": ["team.enemy"] }, "do": [{ "label": { "font": "sans" } }] }]
```

Each DTO is private, under `deny_unknown_fields` (except `cues`, whose keys may be a game's events), numbers checked at deserialization, every ID and name checked at load: stat names against the well-known stats, `StatsPlugin::with_stat` and the `stats` kind (a base outside its bounds fails); `cues` keys and rules' `on` against the registered events; tag filters with F3's `check_declared`; `prefab` and `spawn_children` prefab IDs with `World::check_prefab_id`, a `spawn_children` prefab with a `rigid_body` refused (bodies live on roots, `D-132`); `sound` IDs against the `SoundRegistry` when the World has one. A label's `font` names a manifest face: world labels shape through `section_attrs`, which resolves face IDs only (`tungsten-render/src/text/engine.rs:576`, `:626`, `:998`), so a family ID, which `WorldLabel`'s doc allows, draws in the fallback chain's head with a warning, a renderer limitation recorded on its own. The font is the one name not checked at load: core's `FontRegistry` is filled only at a window's load and lacks the engine font the renderer registers (`tungsten/src/asset_loader/mod.rs:135-167`), and the harness loads no font (`tungsten/src/testing.rs:92-101`); a known issue and a follow-up (Q24). A load error names the file, the field path and the kind (A16); suggestions wait for core (A17, Q18). Counter and flag names follow the tag-name rule (`[a-z][a-z0-9_]*` segments joined by dots) and need no declaration (Q5).

### 2.3 Item cards

| Item | Scene name, kind | Events | Phase and edges | Writes | Stats read | Genres (§2.9) | Rows; perf |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `Stats` | `stats`; kind `stats` | — | `stats_apply` in `kit_tick`, after `health_tick` by registration | `TopDownMover` speed, `PlatformerController` run speed, `Health` max and current, `Cooldown` rate, when they differ | — | platformer, top-down | each stat `value (base b, n mods)`; `stats_apply` bench row |
| `Cooldown` | `cooldown` | out `CooldownElapsed` | `cooldown_tick` in `kit_tick`, after `stats_apply` when present | itself, `KitRng` | `cooldown_rate` | top-down, shmup, grid | next, remaining, rate; `cooldown_tick` row |
| `Counters`, `Flags` | `counters`, `flags` | — | written by cues | — | — | top-down, grid; platformer, grid | name = value; names |
| `KitRng` | — | — | drawn by `cooldown_tick` in entity order | — | — | top-down, shmup (jitter) | — |
| `SpawnChildren` | `spawn_children` | — | `spawn_children` in `kit_react`, by registration after `spawner_tick` | the command buffer: children from the parent, adopted at the flush | — | platformer, shmup | the children's prefabs; one-shot, no row |
| `Cues`, rules, `CuesOnEvent<E>` | `cues`; kind `cue_rules` | in `Damaged`, `ZoneEntered`, `CooldownElapsed`, each `CuesOnEvent<E>`'s `E`; out `ShakeEvent`, `SquashEvent` | `cues` in `kit_react`; `label_tick` in `kit_present` | §2.4's actions | — | all four; kind top-down, shmup | events and action counts; `cues` row |
| `KitTrace` | — | reads none; the items push | `kit_trace_step` in `kit_react`, after `spawner_tick`, `spawn_children` and `cues` when present | itself | — | harness test | the log view (§2.6) |

Within-phase edges are only where a system reads another's output in the same step: `cooldown_tick` reads the rate `stats_apply` writes, and `kit_trace_step` counts the step the others trace in.

### 2.4 Cues: events, actions, order and the boundary

Events in A (§3's vocabulary, closed at this release; A adds `cooldown_elapsed`, its own, and maps the kit's existing events):

| Event | From | Subject | Other | Amount | Position |
| --- | --- | --- | --- | --- | --- |
| `damaged` | each `Damaged` | `entity` | — | `amount` | the subject's `Position`, else `Transform` |
| `killed` | a `Damaged` with `killed`, after its `damaged` | `entity` | — | `amount` | the same |
| `zone_entered` | `ZoneEntered` | the zone | the entrant | — | the event's `position` |
| `cooldown_elapsed` | `CooldownElapsed` | `entity` | — | — | the subject's point |
| a game's name | `CuesOnEvent<E>`'s map | `CueEvent.entity` | `other` | `amount` | `position` |

Actions (A's; `roll` is C's and `effect` E's):

| Action | Data | What it does | Checked at load |
| --- | --- | --- | --- |
| `prefab` | `{ "prefab": "puff" }` | spawns the prefab at the event's position from the subject, so it inherits its `SceneEntity` and `Source` | the ID |
| `counter` | `{ "counter": { "name": "kills", "add": 1, "target": "game" } }`, or `"set": 0` | adds to or sets a counter on the subject (default), the other or the World's `Counters`; a missing component is inserted | exactly one of `add`, `set`; `other` needs an event with one |
| `flag` | `{ "flag": { "name": "has_key", "set": true, "target": "other" } }` | sets or clears a flag, with `counter`'s targets | as `counter` |
| `despawn` | `{ "despawn": {} }`, `{ "despawn": { "target": "other" } }` | queues `despawn_recursive` after the step's spawns | `other` as above |
| `sound` | `{ "sound": { "id": "blip", "min_interval": 0.1 } }` | plays a manifest sound at its volume, at most once per `min_interval` game seconds per sound ID | the ID |
| `shake` | `{ "shake": 0.3 }` | sends `ShakeEvent { trauma_add }`, read by `shake_tick` the same frame (A5) | finite, at least 0 |
| `squash` | `{ "squash": "on_hit" }` | sends `SquashEvent` for the subject with that trigger | one of A19's |
| `flash` | `{ "flash": {} }` | starts the subject's `HitFlash`, as `damage` does | — |
| `label` | `{ "label": { "font": "sans", "size": 16, "color": [1, 1, 1, 1], "seconds": 0.8, "rise": 32, "pop": 1.5 } }` | the event's amount ("3", "1.5": whole, else one decimal) as a timed world label at the event's position: it rises `rise` UI units over `seconds` of game time, scales from `pop` to 1 over its first fifth and fades over its second half; the colour is linear, encoded to sRGB | an event with an amount; `font` required, a face ID, not checked (§2.2) |

The `cues` system, last in the fixed step:
1. Reads each source through its own `EventCursor`: the kit's in the table's order, then each `CuesOnEvent<E>` in build order, each queue in send order, making the step's occurrences. An occurrence whose subject is dead, or carries the crate's `Despawning` marker, is skipped and traced.
2. Runs every occurrence's entity cues, then every occurrence's matching rules (catalogue Q13): rules in entry-ID order, then file order; each list in written order. A rule matches on `on` and its filter over the subject's `Tags`.
3. Applies `counter` and `flag` at once; queues `prefab` spawns as it meets them; queues the step's despawns after every spawn and marks each target `Despawning` at once, so a frame's second step reacts to it no more. Presentation actions change no gameplay state.

The boundary (§3): no conditions, variables or sequences; the rule's filter is the one tag filter (actions take none); `label` shows the event's own amount. A reaction needing more is a game's Rust (§7).

### 2.5 The stats model

A stat's value is `((base + Σadd) × (1 + Σpct) × Πmul)`, replaced by the last-added `Override`, then clamped to its bounds, recomputed when a base or modifier changes, so `get` never computes. Bases and modifier values must be finite (constructors panic, data refuses); the sum and products run in `f64`, a zero factor makes the product 0 (so no `0 × ∞`), and the result is clamped to its bounds and to `±f32::MAX`, so every value an item takes is finite (the setters' finite contracts, `damage/health.rs:107`, `motion/top_down.rs:128`). Modifiers are keyed by a source string (an effect's ID in E); `remove_modifiers` drops all of a source. Bounds: the well-known stats `[0, ∞)`, `max_health` `[1, ∞)`; a game's from `with_stat`, the `stats` kind (the first declaration found, build declarations then entries in ID order) or `with_bounds`. `stats_apply` writes each well-known stat present into the entity's item when it differs (Q2): the movers' speeds (0 stops them), `Health`'s max with its current kept as a fraction of max (Q3: 5/5 at a max of 8 becomes 8/8; a dead one stays dead), `Cooldown`'s rate. Without `Stats`, or without that stat, an item runs on its own field, so no existing recipe moves. `damage`, `area`, `pickup_radius` and `projectile_speed` are declared now and read from C on.

### 2.6 The trace

`KitDebugPlugin` inserts `KitTrace` (256 entries, oldest dropped and counted) and `kit_trace_step`, which counts ended steps. An entry records `Time::frame()` and the steps ended before it: the step it was pushed in for a fixed-step system, the next one for a push outside the steps. Pushes: `damage` (`damaged`, `killed`), `zone_tick`, `cooldown_tick`, the `cues` system (each action run or skipped, each spawn queued or failed, each despawn), `spawn_children`, `spawner_tick` (spawns and failures), `lifetime_tick` (once, at the step its timer finishes) and `despawn_outside`. The view (Q14): while `KitDebug` is on, `kit_debug_draw` logs each new entry once at `info` under target `tungsten_kit::trace`, which the log console (F8) shows. A recipe that fails prints `trace.about(entity)`.

### 2.7 The resolved schedule

`DefaultPlugins` alone stays as `crates/tungsten/src/tests/plugins.rs` pins it. With `KitPlugins::set()` after it, the template's stages become:

```text
fixed_update: physics_prev_snapshot, kit_tick, lifetime_tick, health_tick, stats_apply, cooldown_tick, kit_intent, action_input, kit_move, path_follow, platformer_controller, top_down_mover, physics_step, kit_overlap, despawn_outside, zone_tick, kit_resolve, kit_react, spawner_tick, spawn_children, cues, kit_trace_step
post_update: physics_sync, transform_propagate, particle_count_refresh, particle_emit, particle_tick, tween_tick, squash_stretch_trigger, squash_stretch_tick, shake_tick, transform_propagate_late, kit_present, animation_tick, hit_flash_tick, camera_target, label_tick, kit_debug_draw, camera_update
```

The umbrella's `kit_app()` adds `phases` after `action_input`, `zone_entered` after `zone_tick` and `hit_sound` after `kit_debug_draw`, as today. The phase rustdoc table (`foundation/phases.rs:39-47`) gains stats and cooldowns under `Tick`, children and cues under `React`, labels under `Present`: the catalogue put stats in `kit_resolve`, but `kit_tick` gives a spawned entity its stats before its first move (Q2).

### 2.8 Descriptions

Shapes are named objects a field refers to with `FieldType::Shape`, rendered `shape <name>: …` between the components and the kinds, and held by the same tests as components (sample round trip, defaults, enum variants, numeric extremes). `cue_action` is `one_of`: an object with exactly one of its fields, which the tests probe one field at a time, alone. A's shapes: `cue_action` (fields `prefab`, `counter`, `flag`, `despawn`, `sound`, `shake`, `squash`, `flash`, `label`), `counter_action`, `flag_action`, `despawn_action`, `sound_action`, `flash_action`, `label_style`, `cue_rule`, `stat_bounds`, `child`. Components: `stats` (one field, the value itself), `cues` (one field per kit event, a list of `cue_action`, default `[]`), `cooldown`, `counters`, `flags`, `spawn_children` (a list of `child`). Kind lines: `stats` (an object of `stat_bounds` by name) and `cue_rules` (a list of `cue_rule`). A new test fails when a `Shape` names no described shape.

### 2.9 Fixtures and recipes

New fixture files (step 1, loaded by every recipe from step 1 on; behaviour asserted in step 3):

| Genre | Files | Items |
| --- | --- | --- |
| platformer | `data/stats.json` (`jump_boost`), `prefabs/runner.json`, `key.json`, `torch.json`, `glow.json` | `stats`, `flags` (runner, kinematic, with `action_input`); `cues` (key: a zone setting the runner's `has_key` and despawning); `spawn_children` (torch → glow) |
| top_down | `data/stats.json` (`armor`), `data/cue_rules/combat.json`, `prefabs/brute.json`, `bomb.json`, `blast.json`, `hopper.json`, `sounds/blip.ogg` | `stats`, `counters`, `cues` with a sound (brute); `cooldown` with `count` 1 and a `prefab` and `despawn` cue (bomb); `cooldown` with jitter (hopper); the rules |
| shmup | `data/cue_rules/score.json`, `prefabs/turret.json`, `gunship.json`, `wing.json`, `sounds/blip.ogg` | `cooldown` with jitter and `prefab`/`sound` cues (turret); `spawn_children` (gunship → two wings); a killed rule adding to the game's `score` and shaking |
| grid | `prefabs/delver.json`, `trapdoor.json`, `slug.json` | `counters` and a turn `cooldown` (delver); `flags` and a one-shot `cooldown` clearing `closed` (trapdoor); `killed` cues on a bodiless enemy (slug) |

`sounds/blip.ogg` is a copy of core's synthetic `crates/tungsten-core/tests/fixtures/audio/tone_22050_mono.ogg` (Q20); each manifest lists its new files. Coverage, two genres or more each: `stats` (platformer, top_down), `cues` (all), `cooldown` (top_down, shmup, grid), `counters` (top_down, grid), `flags` (platformer, grid), `spawn_children` (platformer, shmup); kinds `stats` (platformer, top_down), `cue_rules` (top_down, shmup). The existing prefabs stay as they are, and no rule matches what today's scripts do (no script damages anything), so the hash holds until step 3's script spawns the new prefabs.

Recipes (step 3, each at both of `DTS`):

| File | Recipe |
| --- | --- |
| `platformer.rs` | `the_runner_runs_at_its_move_speed_stat_with_its_max_health`; `the_key_sets_the_runners_flag_and_goes`; `the_torchs_glow_hangs_above_it_and_leaves_with_its_state` |
| `top_down.rs` | `a_cue_rule_counts_kills_and_shows_damage_with_no_prefab_field` (the card's: slimes, whose prefab names no cue, damaged and killed through `damage`; the game's `kills` counts each kill once and each hit's amount is drawn as a label); `the_brutes_speed_follows_a_modifier_and_its_hits_count_flash_and_sound`; `the_bomb_bursts_once_after_its_fuse`; `the_hoppers_jittered_hops_repeat_under_a_seed` |
| `shmup.rs` | `the_turret_fires_at_jittered_intervals_with_a_sound`; `the_gunships_wings_follow_it`; `a_score_rule_counts_each_kill_and_shakes_the_camera` |
| `grid.rs` | `the_delvers_turns_tick_on_its_cooldown`; `the_trapdoor_opens_once_its_timer_ends`; `the_slug_counts_its_death_and_goes` |

### 2.10 The template

The player prefab gains `"cues": { "damaged": [{ "shake": 0.25 }, { "label": { "font": "sans" } }] }` (Q22): a hit shakes the camera and floats its amount, with no game Rust and no sound, so no audible review. `tests/game.rs` gains `a_hit_shakes_the_camera_and_shows_its_damage`; `AGENTS.md` and the README name the new items; the guide's "Composing with the kit" gains cues, rules, stats, cooldowns, counters, flags and children.

### 2.11 The seam

The kit stays on core alone (`D-007`, `D-016`, `D-018`): it pushes core's `WorldLabels`, sends core's `ShakeEvent` and `SquashEvent`, plays through `AudioCommands`, draws with `Pcg32`; core names no kit type; no render type; no new dependency; no thread.

## 3. Steps

### Step 1: The API, the data forms and the state items

- **Files:** `crates/tungsten-kit/src/lib.rs`; `foundation/{mod,describe,debug,plugins,phases,tags,stats,rng,trace}.rs`; `react/{mod,cues,counters}.rs`; `lifecycle/{mod,cooldown,children}.rs`; `present/{mod,labels,sound}.rs`; `transform/hierarchy.rs`; `damage/health.rs`; `motion/{top_down,platformer}.rs`; `src/tests/{stats,rng,trace,cues,counters,cooldown,children,describe,inspect,phases,sound,debug,hierarchy,health,top_down,platformer}.rs`; `crates/tungsten/src/tests/kit.rs`; `crates/tungsten/src/tests/kit_recipes/coverage.rs`; §2.9's fixture files and the four manifests; `templates/basic/tests/game.rs` (the schedule string only); `api/tungsten-kit.txt`, `api/tungsten-kit-components.txt`.
- **Change:**
  1. Descriptions (§2.8): `FieldType::{I64, Shape}`, `one_of`, `add_shape`; render; the tests over shapes, the `one_of` probe and the shape-reference test; counts rewritten to what the new set describes.
  2. `Stats`, `StatsPlugin` (the `stats` component and kind, the declarations, the checks), `stats_apply` with crate-private setters in `health.rs`, `top_down.rs` and `platformer.rs` (their first callers).
  3. `KitRng`; `Cooldown`, `CooldownElapsed`, `CooldownPlugin` (inserts `KitRng` when absent): `cooldown_tick` counts `left` down by the step times the rate and sends one event per elapse. Without jitter a cooldown elapses as many times as whole cycles fit in the step's time; with jitter, each elapse draws the next cycle's extra in `[0, jitter]` before the rest of the step's time decides whether another elapse fits, the jittered cooldowns that reach 0 this step taking their catch-up in `Entity::id` order (the one collect in A), and a fresh one draws its first cycle's extra at its first step, in the same order. A cooldown elapses at most 16 times a step (`MAX_ELAPSES_PER_STEP`): the rest of that step's time is dropped, its next cycle starts whole, and the trace notes it, so a tiny `seconds` or a large `cooldown_rate` bounds the step's events at 16 per cooldown.
  4. `Counters`, `Flags`, `CountersPlugin`.
  5. `SpawnChildren`, `SpawnChildrenPlugin`: `spawn_children` prepares each child at the parent's `Transform.position` plus its offset, queues it with `spawn_prefab_from(parent, prepared.with((Parent, LocalOffset, Adopt)))`, then one `CommandBuffer::call` per parent that adopts every `Adopt`-marked child into its parent's `Children` at the flush (`hierarchy.rs` keeps the crate-private helpers, since `Parent`'s field is private, A8); it marks the component done at once, so a frame's second step queues nothing again, and removes it through the buffer.
  6. `KitTrace`, `TraceEntry`, `TraceKind` and the crate's push helper; `KitDebugPlugin` inserts the trace and registers `kit_trace_step`; `cooldown_tick` and `spawn_children` push (the helper's first callers).
  7. The cue API (§2.1, §2.2): `Cues`, `CueAction`, `LabelStyle`, `CueEvent`, `CuesOnEvent<E>` (registers `E`'s queue and its name; its reader lands in step 2), `CuesPlugin` (the `cues` component and the `cue_rules` kind with their DTOs and checks, the event-name registry, the `cues` and `label_tick` systems as empty bodies that step 2 fills); `SoundOnEvent::with_min_interval` (per sound ID, on real time).
  8. `KitPlugins::set()`, `lib.rs` (modules, re-exports, `prelude`, the root catalogue), the phase table; rustdoc with a doctest on every new public item.
  9. Tests: a unit test file per new module; the umbrella's harness tests `stats_drive_the_movers_health_and_cooldowns`, `a_cooldown_elapses_on_the_game_clock_and_its_jitter_repeats_under_a_seed`, `spawned_children_follow_their_parent_and_leave_with_its_state`, `sound_on_event_waits_out_its_min_interval`; the two schedule strings in `kit.rs` and the template's to §2.7; the coverage counts to 19 names and 3 kinds.
  10. §2.9's fixtures; `just api`, then the step 1 API critique (the skill's command, background), each finding checked and recorded in §9 as `C.<n>` and applied to step 1 where it holds.
- **Done-when:**
  - `cargo fmt --all -- --check` → exit 0; `just check` → exit 0.
  - `cargo test -p tungsten --lib --locked kit_recipes::determinism -- --nocapture` → passes, printing `kit recipes hash: 0x2b648d776dff9170`: A's plugins are inert without their components.
  - `cargo test -p tungsten --lib --locked kit_recipes::coverage` → `1 passed`; `cargo test -p tungsten --lib --locked kit_tests::kit_plugins_resolve_into_their_phases` → `1 passed`; each of the four new harness tests, run by its full path with `--exact`, → `1 passed`.
  - `cargo test -p tungsten-kit --lib --locked -- --exact <path>` → `1 passed` for each of `lifecycle::cooldown::tests::a_jittered_cooldown_draws_each_cycle_before_counting_the_next` (0.005 s left, `seconds` 0.005, a drawn extra of 0.02 and a 1/60 s step elapse once), `lifecycle::cooldown::tests::a_cooldown_elapses_at_most_sixteen_times_a_step` and `foundation::stats::tests::overflowing_sums_and_products_stay_finite_and_clamped` (a base of `3e38` doubled, nine `mul` modifiers of `1e38`, a zero factor among them).
  - `rtk proxy git --no-pager diff --numstat -- api/` → only `api/tungsten-kit.txt` and `api/tungsten-kit-components.txt`, the first with 0 lines removed; `rg -c '^component (stats|cues|cooldown|counters|flags|spawn_children):' api/tungsten-kit-components.txt` → 6; `rg -c '^kind (stats|cue_rules):' api/tungsten-kit-components.txt` → 2; `rg -c '^shape (cue_action|counter_action|flag_action|despawn_action|sound_action|flash_action|label_style|cue_rule|stat_bounds|child):' api/tungsten-kit-components.txt` → 10.
  - The API critique's footer and findings are in §9, or its error line as "not run".
- **Moves:** the umbrella's and the template's resolved-schedule strings; the components snapshot; the description, inspector and coverage counts. Not the kit hash.

### Step 2: Reactions: the cue system, labels and the trace

- **Files:** `crates/tungsten-kit/src/react/cues.rs`; `present/labels.rs`; `foundation/{trace,debug}.rs`; `damage/health.rs`; `overlap/zone.rs`; `lifecycle/{lifetime,despawn,spawner}.rs`; `src/tests/{cues,labels,trace,debug,health,zone,lifetime,despawn,spawner}.rs`; `crates/tungsten/src/tests/kit.rs`.
- **Change:** §2.4's system with `CuesOnEvent<E>`'s reader; the label tick (live labels in a crate resource, at most 512, the oldest dropped first; aged on the game clock, pushed every frame, frozen while paused); §2.6's pushes and view; `start_flash` crate-visible for `flash`. Harness tests: `cues_react_once_to_each_event_from_any_stage`, `entity_cues_run_before_rules_and_despawns_after_spawns`, `a_game_event_reaches_cues_through_cues_on_event`, `labels_rise_fade_and_hold_while_paused`, `the_trace_tells_why_an_entity_died`.
- **Done-when:**
  - `cargo fmt --all -- --check` → exit 0; `just check` → exit 0.
  - Each of the five harness tests, run by its full path with `--exact`, → `1 passed`.
  - `cargo test -p tungsten --lib --locked kit_recipes::determinism -- --nocapture` → passes at `0x2b648d776dff9170`.
  - `just api-check` → current: step 2 adds no public item.
- **Moves:** none.

### Step 3: Recipes, the kit hash and the bench

- **Files:** `crates/tungsten/src/tests/kit_recipes/{mod,platformer,top_down,shmup,grid,determinism}.rs`; the fixture files of §2.9 where a recipe needs a value changed; `crates/tungsten-kit/benches/kit.rs`.
- **Change:** §2.9's 13 recipes. The determinism script spawns A's new prefabs after the existing ones, so existing entities keep their indices, and calls `damage` at fixed frames; the hashed row adds each entity's counters, flags and stat values, and each genre hashes the World's `Counters` and `Flags`; the new pin goes in `KIT_RECIPES_HASH`. Bench rows `stats_apply`, `cooldown_tick` (no jitter) and `cues` (one `damaged` occurrence per entity per step, with an entity cue and one rule) at 1,000 and 10,000, run once in test mode; the measured run and bound are W13i's (`D-172`).
- **Done-when:**
  - `cargo fmt --all -- --check` → exit 0; `just check` → exit 0.
  - `rtk proxy cargo test -p tungsten --lib --locked kit_recipes:: -- --list` → lists each of §2.9's 13 recipes; `cargo test -p tungsten --lib --locked kit_recipes::` → `35 passed` (22 today and the 13).
  - `cargo test -p tungsten --lib --locked kit_recipes::determinism -- --nocapture` → passes, the printed hash equal to the new pin (recorded in the evidence log), both in-process runs equal.
  - `jq -e '.components | (has("cues") or has("counters")) | not' crates/tungsten/tests/fixtures/kit/top_down/prefabs/slime.json` → `true`.
  - `rtk proxy cargo test -p tungsten-kit --bench kit --locked` → exit 0: criterion runs each row once in test mode under `cargo test` (criterion 0.8.2's `lib.rs:958-964`), naming `stats_apply`, `cooldown_tick` and `cues` at 1000 and 10000; no timing capture.
  - `just physics-release` → passes: `0x088e_c07a_73c1_b168` and `0xaee2_72e0_1ffc_4e4c` unchanged.
- **Moves:** the kit hash, on purpose: the new value in the evidence log, decision 173's entry (step 5) and the `CHANGELOG.md` line.

### Step 4: The template and the guide

- **Files:** `templates/basic/assets/prefabs/player.json`, `templates/basic/tests/game.rs`, `templates/basic/AGENTS.md`, `templates/basic/README.md`, `docs/getting-started.md`.
- **Change:** §2.10. The test damages the player, steps one frame and finds a drawn label `1` in `sans` and a camera trauma above 0.
- **Done-when:**
  - `cargo fmt --all -- --check` → exit 0; `just check` → exit 0.
  - `cargo test -p tungsten-template-basic --locked -- --exact a_hit_shakes_the_camera_and_shows_its_damage` → `1 passed`.
  - `cargo test -p tungsten-core --test manifests --locked` → passes (layer 1 loads the template's manifest).
  - `jq -e '.components.cues.damaged | length == 2' templates/basic/assets/prefabs/player.json` → `true`; `rg -c '"cues"' docs/getting-started.md` → at least 1.
  - `just repo-check` → passes.
- **Moves:** none (the template's schedule moved in step 1).

### Step 5: Records

- **Files:** `DECISIONS.md`, `docs/DECISION_INDEX.md`, `DESIGN.md`, `docs/LLM_INDEX.md`, `docs/plans/1.0/w04-api-freeze.md`, `docs/known-issues.md`, `docs/plans/1.0/w13-kit.md`; `api/` checked only.
- **Change:**
  1. First, with tungsten-decision: decision 173 (§5), its index row at the end of "ECS / Runtime Flow", marker lines on `D-155`, `D-158` and `D-172` and their rows.
  2. `DESIGN.md`: `### Stats and Cues — M63` after the M62 section: the stats model, the cue vocabulary, order and boundary, labels, the trace, the new resolved stages and the kit hash.
  3. The index's kit row becomes `| Kit by domain: phases, tags, stats, cues, recipes, descriptions (`D-155`, `D-172`, `D-17x`) | …unchanged paths… |` with 173 for `17x`, within the budget (A22).
  4. The break ledger: one row of additions for the freeze (§2.1, the `api/` line counts) and one of behaviour changes (the set's new systems in a game's resolved schedule; the movers, health and cooldowns following `Stats` when present; `SoundOnEvent` unchanged without an interval).
  5. Known issues, the kit row: the flush sentence names the Phase 6 QA pass only (Q16); new limits: a label's font is a face ID, not checked at load (§2.2); a separate render row: world labels resolve face IDs only, though `WorldLabel`'s doc allows a family (`tungsten-render/src/text/engine.rs:998`); a child draws one frame at its queue point before its first propagation; a `prefab` cue on a subject another system despawned earlier in the frame spawns nothing (A7); labels outlive a state's exit by at most their `seconds`; a cue reacts one step later to an event sent outside the fixed step.
  6. `w13-kit.md`: the revision line ("W13h landed in 0.83 (M63, `D-17x`)…"), §2's phase table (stats and cooldowns in `kit_tick`), §12's Q3, Q13 and Q16 answered, the M50 follow-up on command-buffer children landed, A23's fragment deleted.
- **Done-when:**
  - `rg -c '^## D-17[3] — ' DECISIONS.md` → 1; `rg -c '^\*\*Amended by D-17[3]:\*\*' DECISIONS.md` → 3; `rg -c '^\| `D-17[3]` \|' docs/DECISION_INDEX.md` → 1; `rg -c 'Amended by `D-17[3]`' docs/DECISION_INDEX.md` → 3.
  - `rg -c '^### Stats and Cues — M63$' DESIGN.md` → 1; `rg -c 'W13h \(M63' docs/plans/1.0/w04-api-freeze.md` → 2.
  - `rg -c '^## ' docs/plans/1.0/w13-kit.md` → 19; `rg -c 'W13h landed in 0\.83 \(M63' docs/plans/1.0/w13-kit.md` → 1.
  - `just api-check` → current; `just ctx` → passes; `just repo-check` → passes; `git diff --check` → exit 0.
  - `just checks` → its recommendations in the evidence row, each run by a step or the release chain.
- **Moves:** none.

### Step 6: Release 0.83

- **Files:** `CHANGELOG.md`, `DESIGN.md`, `Cargo.toml`, `Cargo.lock`, the register row, the README's four owned Now fields (`roadmap.py summary`), this plan (moved to `docs/plans/archive/1.0/`)
- **Change:** close workflow §5 (§8) and this step's evidence row with `status: done`; then [tungsten-finalize](../../../../.claude/skills/tungsten-finalize/SKILL.md) and [tungsten-release](../../../../.claude/skills/tungsten-release/SKILL.md) ([releases](../../../releases.md)): the plan archived, `just release-cut 0.83.0` once, every release check once after it (no gate is left over: each step runs its own, and step 4's layer 1 runs inside `just check`), then the preflight with `--message`.
- **Done-when:** [releases](../../../releases.md) step 1's command chain → every check passes, quoted in the report (the plan is archived before the cut); `just release-preflight 0.83.0 --repo JosephGibson/Tungsten --message 'Update 0.83: M63 kit data and reactions'` → passes and prints the command block and the post-merge block.
- **Ends with:** the changed-file list, the command block, the post-merge block, then the plan prompt for W13i (the register's next row).

## 4. Open questions

| # | Question | Default |
| --- | --- | --- |
| Q1 | Stat keys (catalogue Q3: interned IDs) | Names, not interned IDs: a load's checks and spawns see `&World` only (A16), so no table could intern the `stats` kind's names; an entity holds few stats, sorted, read by binary search, and `stats_apply`'s bench row measures it |
| Q2 | How items take well-known stats | `stats_apply` in `kit_tick` writes them into the item's own field when they differ, so items' systems stay unchanged and an entity without `Stats` runs as today; `kit_tick`, not the catalogue's `kit_resolve`, so a spawn moves at its stat from its first step |
| Q3 | `Health` when `max_health` changes | Current keeps its fraction of max; `max_health` clamps at 1, so no modifier kills |
| Q4 | Modifier order and overrides | §2.5: the catalogue's order, the last-added override, then the clamp |
| Q5 | Where counters and flags live | An entity's components and the World's resources of the same types; `target` picks `self`, `other` or `game`; names follow the tag-name rule, undeclared, since data writes them and the game reads them |
| Q6 | Counter type | `i64`, added to or set |
| Q7 | A's cue events | `damaged`, `killed`, `zone_entered`, `cooldown_elapsed`; `spawned` and `expired` wait for an item that sends them (C's projectile; a `spawned` rule would need a marker on every spawn) |
| Q8 | `zone_entered`'s subject | The zone; the entrant is `other`, since the zone's filter already chose it |
| Q9 | Action targets | `counter`, `flag` and `despawn` take `target`; the rest act on the subject or its position; no action takes a filter |
| Q10 | Label styles (catalogue Q16) | Inline in the action, no kind: a cue rule is already shared data; a `label_styles` kind can come later as a `DataRef`, which still reads the inline form |
| Q11 | Order (catalogue Q13) | §2.4: entity cues before rules, each in event order and written order; despawns after spawns; a `Despawning` subject reacts no more that frame |
| Q12 | `min_interval`'s clock | Game time for cue sounds, which run on the fixed step; real time for `SoundOnEvent`, which plays over a pause; per sound ID |
| Q13 | `KitRng`'s seed | `DEFAULT_SEED`, reseeded by the game per run; drawing from `WorldRngSeed` would move every particle emitter's seed (A13); cooldown jitter is its first user |
| Q14 | The trace's view | The log console (F8), one `info` line per new entry while `KitDebug` is on; the ring records either way; a window or world view waits for a user |
| Q15 | The trace's step | The ended-step count `kit_trace_step` keeps, so entries order across a frame's steps |
| Q16 | A command flush per fixed step (`docs/known-issues.md:51`) | Not in A: it changes `App`'s frame loop, shared frame-loop code whose change captures every CPU row; the Phase 6 QA pass; A's recipes stay one-step-per-frame claims, and `Despawning` keeps a frame's second step from reacting twice |
| Q17 | Describing nested data | §2.8's shapes and `one_of`, so the action vocabulary's fields and defaults are under `just api-check` and the description tests (P7) |
| Q18 | Nearest-name suggestions (§5.2) | The error names the field and the kind; suggestions wait for core's helper to go public (A17), the M61 follow-up |
| Q19 | `CuesOnEvent<E>`'s checks | A game event is checked as one with an amount and an other; an event its map returns without them skips those actions, traced |
| Q20 | A sound in the fixtures | A copy of core's synthetic tone as `sounds/blip.ogg` in two fixtures, so the cookbook stays self-contained |
| Q21 | The kit hash | Unchanged through steps 1–2, then moved once by step 3's script and recorded |
| Q22 | The template's cues | The player's `damaged` shake and label in `sans`; no sound, no rule (one entity) |
| Q23 | A23's fragment | Step 5 deletes it: the archived catalogue holds the critique and the evidence |
| Q24 | Label fonts | A manifest face ID, since world labels resolve faces only (round 2); not checked at load, since core's `FontRegistry` exists only after a window loads fonts and lacks the engine font; two known issues, the kit's unchecked font and the renderer's unresolved family IDs, and a follow-up each |

Each question takes its stated default; the owner reviews the plan on their own time (`D-150`).

## 5. Decisions expected

- **173:** W13h landed (M63): `Stats` and its model (§2.5), `stats_apply` in `kit_tick`, the well-known stats; the cue vocabulary at A (§2.4's events and actions), its order and its boundary, `CuesOnEvent<E>`, the `label` action over `WorldLabels`; `Cooldown`, `Counters`, `Flags`, `KitRng`, `SpawnChildren` with children adopted at the flush; `KitTrace` and the log view; described shapes; the new resolved stages and the kit hash's new pin. Amends `D-155` (`SoundOnEvent`'s interval), `D-158` (the movers and health follow `Stats`) and `D-172` (the set, the debug plugin's trace, shapes in the components snapshot, the phase table, the hash). The next free ID at `28e802a`; checked again when step 5 runs.

## 6. Invariants

- **May move:** the kit hash `0x2b64_8d77_6dff_9170` (step 3, recorded); the umbrella's and the template's resolved-schedule strings (step 1); `api/tungsten-kit.txt` and `api/tungsten-kit-components.txt` by additions (step 1).
- **Must not move:** the determinism hash `0x088e_c07a_73c1_b168`, the pinned containment hash `0xaee2_72e0_1ffc_4e4c`, the row digests, `gpu-visual.png`, the post and transition regressions, `DefaultPlugins`' resolved schedule, `api/tungsten-core.txt`, `api/tungsten-render.txt`, `api/tungsten.txt`; the kit on core alone and core naming no kit type; no new third-party dependency (`D-015`); opt-in plugins; two background threads; prefab and scene files keep their fields (`D-153`, `D-157`).

## 7. Stop conditions

- A done-when check fails: restore the step's files from their copies, mark the step skipped, and continue with the steps that do not depend on it. Steps 2–5 depend on step 1; step 3 on step 2; the release needs steps 1–3 and 5, and may ship with step 4 skipped, its template change then a known issue and its gates (`just template-check` already in the chain) run by the release.
- The kit hash moves in step 1 or 2: an existing item changed behaviour; stop the step.
- `DefaultPlugins`' schedule, a physics hash or a snapshot other than the kit's moves: stop at the release.
- An item cannot pass recipes in two genres: leave it out of `KitPlugins::set()` and record it (w13-kit §14).
- The API critique finds a shape tied to one genre: redesign it in step 1 or drop the item.
- A cue needs a condition beyond a tag filter: the reaction is a game's Rust; change the recipe, not the vocabulary.
- A file outside a step's list needs a change: stop the step and record why.
- The index cannot take the kit row's new text within 12,288 bytes: keep the row as it is and record it.

## 8. What the milestone owes

| Workflow §5 line | Step | Note |
| --- | --- | --- |
| 1 Evidence | every | |
| 2 Gates by change | 1–4, release | The kit, umbrella and template tests through `just check` per step; layer 1 (4); `just physics-release` (3); no perf row: no perf workload adds a kit plugin and `DefaultPlugins` holds; no visual or motion review in A: no shipped scene can hit anything (the template's player takes hits only in its tests, and no example adds a kit plugin), and the template smoke runs three title frames (`scripts/template-check.sh:86`), so the harness asserts the label's per-frame rise, scale and alpha and the camera's trauma (`labels_rise_fade_and_hold_while_paused`, `a_hit_shakes_the_camera_and_shows_its_damage`), and the first scene with a hit, W13i's template hazard, takes the motion review (a follow-up); no audible review: no shipped scene plays a cue sound. The release runs `just check`, `just smoke`, `just template-check` and the rest once, after the cut |
| 3 Invariants | 1–3 | §6; the kit hash's move in 3 |
| 4 Decisions | 5 | 173, written before the docs that cite it |
| 5 API ([break ledger](../../1.0/w04-api-freeze.md#break-ledger), rustdoc, `just api` snapshot) | 1, 5 | `just api` in 1 with its critique; the ledger rows and `just api-check` in 5 |
| 6 Template and guide | 4 | The schedule string in 1 |
| 7 Routes (`docs/LLM_INDEX.md`, `just ctx`) | 5 | New files sit under the routed kit folder |
| 8 Design | 5 | `DESIGN.md`'s M63 section |
| 9 Records (known issues, backlog, gap log, register, workstream file, README's four owned Now fields) | 5, release | Known issues and `w13-kit.md` in 5; no backlog row (nothing cut), no gap row (`acceptance-game.md`'s log is empty); the register and the owned fields at the release, `python3 -B scripts/roadmap.py summary` supplying the fields |
| 10 Close (one `CHANGELOG.md` line, `status: done`, the plan archived) | release | The line carries the kit hash's new value; then the cut, the checks and the preflight; the session ends with both blocks and the next prompt |

Closed at the release, 2026-10-17: 1, every step's row below; 2, steps 1–5's gates and the release chain after the cut; 3, §6 held (the physics hashes in `just physics-release`, `DefaultPlugins`' schedule in `just check`, the core, render and umbrella snapshots unchanged, the kit on core alone with no new dependency, no new thread, prefab and scene files keeping their fields), the kit hash moved on purpose and recorded; 4, `D-173` before the docs; 5, the ledger's two rows, rustdoc on every new item, `just api` and `just api-check`; 6, the template and the guide in step 4; 7, the index's kit row, `just ctx`; 8, `DESIGN.md`'s M63 section; 9, known issues, `w13-kit.md`, the register row and the README's owned fields (`just release-cut`); 10, the `CHANGELOG.md` entry, this status and the archive.

## 9. Critique

Round 1, 2026-10-16, at `28e802a` with this plan uncommitted, §9 and the log cut with `sed`: the `critique` skill with `--repo`, gpt-6-astra, effort max, context 872,000, output cap 4,000, 441.3 s, 1,156,452 tokens in (1,044,352 cached), 17,572 out (12,079 reasoning), 46 commands, 300.1 KB output (largest 51.2 KB), 5h window 25% → 55%, week 62% → 67%. Six findings, each checked before any change; five accepted, one in part. Round 1 changed step 1's and step 3's done-when checks, so round 2 runs.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 1 | [MEDIUM] Cooldown catch-up counts expirations before their durations are known: a jittered cycle drawn after counting base periods lets three elapses through where one fits | Step 1 change 3 as written; the arithmetic of its example | Accepted | Step 1 change 3: each elapse draws the next cycle's extra before the step's remaining time decides whether another fits, jittered catch-ups in `Entity::id` order, a fresh cooldown's first draw at its first step; step 1's done-when gains `a_jittered_cooldown_draws_each_cycle_before_counting_the_next` with the critic's case |
| 2 | [MEDIUM] Valid parameters permit unbounded work per step: a 0.5 s cooldown at rate `1e9` sends about 33 million events a step, each pushed to a `Vec` | `crates/tungsten-core/src/ecs/event_queue.rs:95`; §2.5's unbounded `cooldown_rate` | Accepted | Step 1 change 3: at most 16 elapses per cooldown a step (`MAX_ELAPSES_PER_STEP`), the rest of the step dropped and traced; done-when gains `a_cooldown_elapses_at_most_sixteen_times_a_step`. The same unbounded catch-up in `spawner_tick` (`lifecycle/spawner.rs:251-258`) is a follow-up for the Phase 6 QA pass, outside A's files |
| 3 | [MEDIUM] Finite stat inputs can overflow to non-finite values that break `Health`'s and the movers' finite contracts | `damage/health.rs:107`; `motion/top_down.rs:128`; §2.5 | Accepted | §2.5: finite inputs, `f64` arithmetic, a zero factor short-circuits, the result clamped to its bounds and to `±f32::MAX`; done-when gains `overflowing_sums_and_products_stay_finite_and_clamped` |
| 4 | [MEDIUM] The label fixtures pass with an undeclared font: the harness loads no font and an unknown font only warns at render, so "every ID is checked" is unverified | `tungsten/src/testing.rs:92-101`; `tungsten-render/src/text/engine.rs:995-1005`; `tungsten/src/asset_loader/mod.rs:135-167` (core's `FontRegistry` holds faces, not families, filled before the prefabs at a window's load) | Accepted in part | §2.2, §2.4 and Q24: the plan no longer claims a label's font is checked; a check against faces alone would refuse valid family IDs, so the limit goes to known issues (step 5) and a follow-up asks for a core list of font and family IDs. Rejected: a declared fixture font, since the harness skips fonts and so would verify nothing |
| 5 | [MEDIUM] No check exercises the new visual behaviour: the matrix asks for visual or motion review of visible gameplay, and the template smoke runs three title frames | `docs/validation.md:25`; `scripts/template-check.sh:86`; `templates/basic/src/game.rs:65` | Accepted in part | §8 line 2 no longer rests on "no renderer change": no shipped scene can trigger a hit in A, so the harness asserts the label's per-frame values and the trauma, and the motion review goes to W13i's template hazard, the first running scene with a hit (a follow-up). Rejected: a new scene or debug hook to trigger hits now, which the card does not include |
| 6 | [LOW] The perf exemption's premise is false: the default `kit` feature compiles the kit into the bench | `crates/tungsten/Cargo.toml:10`; `examples/02_bench/Cargo.toml:9` | Accepted | Context digest and §8: no example or perf workload adds a kit plugin, and `DefaultPlugins` registers no kit system |

Round 2, 2026-10-16, on the revised plan, §9 and the log cut with `sed`, with a focus note naming round 1's changed sections and its two partial rejections: gpt-6-astra, effort xhigh, context 872,000, output cap 4,000, 144.0 s, 587,195 tokens in (514,176 cached), 5,133 out (2,857 reasoning), 13 commands, 192.6 KB output (largest 29.8 KB), 5h window 55% → 70%, week 67% → 69%. One finding, accepted. There is no third round.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 2.1 | [MEDIUM] Declared font families fall back too: world labels pass their ID into shaping, whose `section_attrs` resolves face IDs only, so a family ID draws in the fallback chain's head; the planned check would not fix that | `tungsten-render/src/text/engine.rs:576`, `:626`, `:995-1005` | Accepted | §2.2, §2.4's `label` row and Q24: a label's `font` is a face ID; step 5's known issues gain a separate render row; a follow-up for family resolution or a narrower `WorldLabel` doc |

Step 1 API critique, 2026-10-17, on the `api/` and template diff (the skill's command, `--raw --repo --force`), gpt-6-astra, effort xhigh, context 872,000, output cap 4,000, 104.6 s, 647,082 tokens in (562,560 cached), 4,177 out (2,005 reasoning), 14 commands, 211 KB output (largest 29.7 KB), 5h window 0% → 16%, week 69% → 72%. Two findings, each checked; one accepted, one rejected as the plan's own staging.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| C.1 | [HIGH] The snapshots advertise cue actions that never execute: `Reactor::run` reads occurrences and runs no action, `label_tick` is empty | `react/cues.rs` `Reactor::run`; `present/labels.rs` `label_tick`; step 1 change 7 ("the `cues` and `label_tick` systems as empty bodies that step 2 fills") | Rejected | Step 1's planned state: step 2 runs the actions and the labels and holds them with its five harness tests, before any snapshot ships |
| C.2 | [MEDIUM] `KitRng::range` returns infinity or NaN for finite, ordered ends: `hi - lo` overflows `f32` at `-f32::MAX..f32::MAX` | `foundation/rng.rs` `range` | Accepted | `range` interpolates in `f64` and clamps to `[lo, hi]`; `tests/rng.rs` gains `a_range_across_the_whole_f32_line_stays_finite`; no signature or snapshot change |

Not from the critic: step 3's bench check runs `cargo test --bench kit` (criterion's test mode) in place of `cargo bench … -- --test`, which the roadmap's state script counts as a timing capture (`.claude/skills/tungsten-next/state.sh:199`).

## Evidence log

| Step | Date | Verdict | Key numbers | Paths |
| --- | --- | --- | --- | --- |
| 1 | 2026-10-17 | Pass | `cargo fmt --all -- --check` exit 0; `just check` exit 0; `kit recipes hash: 0x2b648d776dff9170`, `1 passed`; `kit_recipes::coverage` `1 passed` (19 names, 3 kinds); `kit_tests::kit_plugins_resolve_into_their_phases` `1 passed`; the four harness tests by `--exact` `kit_tests::<name>`, each `1 passed`; the three kit unit tests by `--exact`, each `1 passed`; `api/` numstat `86 0 api/tungsten-kit-components.txt`, `1107 0 api/tungsten-kit.txt`, nothing else; the component, kind and shape greps `6`, `2`, `10`; the API critique in §9 (C.1 rejected, C.2 applied after the checks, its rng tests `7 passed`, `just check` rerun in step 2). Departures: `cues`'s body reads its sources in step 1 so the readers have a non-test caller (`dead_code` under `-D warnings`), with an `#[expect(dead_code)]` on the occurrence and the rule filter until step 2 runs the actions; `Stats::add_modifier` returns whether the stat is there; `Cooldown` gains `rate()` and `MAX_ELAPSES_PER_STEP` as an associated constant; the description tests skip a default the sample itself gives and probe whole numbers on the sample's required fields | `crates/tungsten-kit/src/{foundation,lifecycle,react,present}/`; `api/tungsten-kit*.txt`; fixtures under `crates/tungsten/tests/fixtures/kit/` |
| 2 | 2026-10-17 | Pass | `cargo fmt --all -- --check` exit 0; `just check` exit 0 (after C.2's change too); the five harness tests by `--exact` `kit_tests::<name>`, each `1 passed`; `kit recipes hash: 0x2b648d776dff9170`, `1 passed`; `just api-check` "Public API snapshots current: 4 crates and the kit's components", exit 0. Kit unit tests 304 passed. Departures: a `Cues` list sits behind an `Arc<[CueAction]>`, so a step takes an entity's or a rule's list without copying its actions; `despawn_outside` and `kit_debug_draw` became closures holding the per-frame trace dedupe and the view's place; the view logs only entries pushed while `KitDebug` is on | `crates/tungsten-kit/src/react/cues.rs`, `present/labels.rs`, `foundation/{trace,debug}.rs`, `damage/health.rs`, `overlap/zone.rs`, `lifecycle/{lifetime,despawn,spawner}.rs`; `crates/tungsten/src/tests/kit.rs` |
| 3 | 2026-10-17 | Pass | `cargo fmt --all -- --check` exit 0; `just check` exit 0 (a first run failed on `clippy::format_push_string` in `determinism.rs`, fixed with `write!`, rerun from the top); `kit_recipes:: -- --list` lists 35, the 13 among them; `kit_recipes::` `35 passed`; `kit recipes hash: 0xd2a064953ac2b9c1`, `1 passed`, both in-process runs equal and equal in two processes, the new pin `KIT_RECIPES_HASH = 0xd2a0_6495_3ac2_b9c1` (was `0x2b64_8d77_6dff_9170`); the slime `jq` check `true`; `cargo test -p tungsten-kit --bench kit` exit 0, naming `stats_apply`, `cooldown_tick` and `cues` at 1000 and 10000; `just physics-release` exit 0, `7 passed`, `1 passed`, `7 passed`, the two pinned hashes in their tests unchanged and core untouched. Departures: the brute prefab gains a sprite (a flash needs one, `damage/health.rs` `start_flash`); the determinism test asserts the script's cues ran (the runner's key, the kills, the score, the slug) before it hashes; the `cues` bench row checks once that every entity's cue and the rule ran | `crates/tungsten/src/tests/kit_recipes/`; `crates/tungsten/tests/fixtures/kit/top_down/prefabs/brute.json`; `crates/tungsten-kit/benches/kit.rs` |
| 4 | 2026-10-17 | Pass | `cargo fmt --all -- --check` exit 0; `just check` exit 0; `a_hit_shakes_the_camera_and_shows_its_damage` `1 passed` (the drawn labels `[("1", "sans")]`, the camera's trauma above 0); `cargo test -p tungsten-core --test manifests` `5 passed`; the player `jq` check `true`; `"cues"` lines in the guide: 1; `just repo-check` exit 0 | `templates/basic/{assets/prefabs/player.json,tests/game.rs,AGENTS.md,README.md}`; `docs/getting-started.md` |
| 5 | 2026-10-17 | Pass | `D-173` the next free ID (checked again: `rg` gives `D-172` last, no other session's entry); `^## D-173 — ` 1; `**Amended by D-173:**` markers 3 (on `D-155`, `D-158`, `D-172`); index row 1 (end of ECS / Runtime Flow), `Amended by \`D-173\`` 3; `### Stats and Cues — M63` 1; `W13h (M63` in the ledger 2; `w13-kit.md` `## ` headings 19 (A23's fragment gone), its revision line 1; the index's kit row rewritten within the budget (12,287 of 12,288 bytes, "scene components" dropped for "stats, cues"); `just api-check` current; `just ctx` exit 0; `just repo-check` exit 0; `git diff --check` exit 0. `just checks` recommends the kit, umbrella, template and layer 1 tests (run by `just check` in steps 1–4 and step 4's layer 1), `just ctx`, `just repo-check`, `just api-check`, `just check`, `git diff --check` (here), `just smoke` and `just template-check` (the release chain); its audio review is not run, since no shipped scene plays a cue sound (§8 line 2). `w13-kit.md` also carries M63's follow-ups, since the archived plan is not read | `DECISIONS.md`; `docs/DECISION_INDEX.md`; `DESIGN.md`; `docs/LLM_INDEX.md`; `docs/plans/1.0/{w04-api-freeze,w13-kit}.md`; `docs/known-issues.md` |
| 6 | 2026-10-17 | Closed | Workflow §5 closed (§8); `CHANGELOG.md`'s `[Unreleased]` entry, `DESIGN.md`'s status line, the register row (0.83, archive link); the plan archived before the cut, so `just release-cut 0.83.0`, the release checks and the preflight are quoted in the session's report | `CHANGELOG.md`; `DESIGN.md`; `docs/plans/1.0/implementation-plan.md`; `docs/plans/archive/1.0/` |

## Follow-ups

- A `KitDebug` hotkey or console command, so the trace view needs no game code (M62's open item).
- `spawned` and `expired` cue events, with C's projectile; `Lifetime` sending `expired` then.
- A `label_styles` kind, when a second style needs sharing across rules and prefabs (Q10).
- `KitRng` and `KitTrace` serde, when a save needs the stream's place.
- Nearest-name suggestions for the kit's names, with core's helper (Q18).
- A command flush per fixed step: the Phase 6 QA pass (Q16).
- A core list of the manifest's face IDs with the engine font, so a label's font is checked at load (Q24).
- Family IDs for world labels: resolve a family in `section_attrs` (`tungsten-render/src/text/engine.rs:998`) or narrow `WorldLabel`'s doc to faces; render-side, the Phase 6 QA pass.
- W13i: a motion review of the cue label and shake in the template's hazard, the first running scene with a hit (§8).
- `Spawner`'s interval has no lower bound and `spawner_tick` queues every spawn a long step owes (`lifecycle/spawner.rs:251-258`), as round 1's finding 2 found for cooldowns: the Phase 6 QA pass.
