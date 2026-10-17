//! The kit: generic 2D gameplay components and systems that a game, the
//! template and the examples take instead of writing their own copies.
//!
//! The kit depends on `tungsten-core` alone, never on the renderer or the
//! umbrella: each item is a component, a function over a
//! [`World`](tungsten_core::World) or a [`Plugin`](tungsten_core::Plugin)
//! that registers a system through the [`Schedule`](tungsten_core::Schedule).
//! It writes the core components the default extract already draws
//! (`Transform`, `Sprite`), so it adds no render type and no extract
//! contribution (`D-007`, `D-016`). The umbrella re-exports it as
//! `tungsten::kit` behind its default `kit` feature, so a game keeps one
//! dependency, and the kit's API is inside the 1.0 promise through that
//! re-export (`D-103`).
//!
//! An item joins the kit when it is generic, names no game type and passes
//! a recipe (a fixture of prefabs and data plus a harness test, with no
//! game Rust) in two of the four reference genres: platformer, top-down,
//! shmup and grid (`D-155`, as the kit foundations amend it). Each item has
//! a scene component under its bare name, rustdoc with an example, a unit
//! test and a harness test.
//!
//! # Composing a game
//!
//! [`KitPlugins::set()`] is every kit plugin that takes no game type, which
//! a game builds after the engine's `DefaultPlugins`
//! (`app.add_plugins(KitPlugins::set())`); `without` drops one.
//! [`SoundOnEvent`], [`TriggerZone`], [`StateMachinePlugin`] and
//! [`CuesOnEvent`] take a game type, so a game adds them itself. [`prelude`] imports the plugins,
//! components, events and helpers in one line. Each plugin registers its
//! scene component, so a prefab composes the kit with no Rust:
//!
//! ```json
//! {
//!   "sprite": { "asset_id": "hero" },
//!   "components": {
//!     "tags": ["team.player"],
//!     "action_input": { "move": ["move_left", "move_right", "move_up", "move_down"] },
//!     "top_down_mover": { "speed": 240 },
//!     "health": { "max": 5, "invulnerability": 0.5 },
//!     "hit_flash": { "seconds": 0.12 },
//!     "camera_target": null,
//!     "cues": { "damaged": [{ "shake": 0.25 }, { "label": { "font": "sans" } }] }
//!   }
//! }
//! ```
//!
//! Every value type keeps private fields behind a constructor and
//! `with_*` builders; a type that is data derives `Serialize` and
//! `Deserialize`, so a game's save can hold it.
//!
//! # Phases
//!
//! Every kit system joins one of seven phases, each an anchor system the
//! kit's plugins add once ([`ensure_phases`]) and order against the
//! engine's names: [`Phase::Tick`], [`Phase::Intent`], [`Phase::Move`],
//! [`Phase::Overlap`], [`Phase::Resolve`] and [`Phase::React`] in
//! `FixedUpdate`, around `physics_step`, and [`Phase::Present`] in
//! `PostUpdate`, after the physics sync and before the camera. Timers,
//! lifetimes, spawns and motion run on the fixed step, so a paused clock
//! holds them and a hit sequence lands the same at any frame rate; the
//! animation, the hit flash, the camera target and sounds run once a frame.
//! A game's system joins a phase with [`Phase::join`]; the [`Phase`] table
//! says what runs where.
//!
//! # Catalogue
//!
//! - **Foundation.** [`Tags`], [`TagFilter`] and [`TagsPlugin`]: dotted
//!   names on an entity, the filters items match on and the `tags` data
//!   kind that declares them. [`Source`] and [`SourcePlugin`]: the spawn
//!   chain's origin, derived at the flush. [`Stats`], [`Modifier`] and
//!   [`StatsPlugin`]: named values with ordered modifiers, which the
//!   movers, health and cooldowns read, and the `stats` data kind that
//!   declares a game's. [`KitRng`]: the kit's seeded random stream.
//!   [`KitTrace`], [`TraceEntry`] and [`TraceKind`]: what the items did,
//!   for a failing recipe or the debug view. [`Phase`], [`ensure_phases`]
//!   and [`KitPlugins`].
//! - **Intent.** [`ActionInput`], [`Intent`] and [`ActionInputPlugin`]:
//!   the action map read into what each entity means to do, which the
//!   movers take. [`StateMachine`] and [`StateMachinePlugin`]: a per-entity
//!   state with enter, exit and update hooks. [`cursor_to_world`]: the
//!   world point under the cursor.
//! - **Motion.** [`PlatformerController`] and
//!   [`PlatformerControllerPlugin`]: a platformer character on a kinematic
//!   body, with a ground check, coyote time, a jump buffer and variable
//!   jump height. [`TopDownMover`] and [`TopDownMoverPlugin`]: a top-down
//!   character that slides along walls on a kinematic body, or moves its
//!   `Position` without one. [`PathFollower`] and [`PathFollowerPlugin`]: a
//!   kinematic body moved along points, such as a moving platform.
//! - **Lifecycle.** [`Lifetime`] and [`LifetimePlugin`]: an entity
//!   despawned once its time on the game clock has passed.
//!   [`DespawnOutside`] and [`DespawnOutsidePlugin`]: one despawned once it
//!   leaves a world-space box or the camera's view. [`Spawner`] and
//!   [`SpawnerPlugin`]: a prefab spawned at an entity's position once per
//!   interval of game time. [`Cooldown`], [`CooldownElapsed`] and
//!   [`CooldownPlugin`]: a repeating countdown with an optional jitter.
//!   [`SpawnChildren`] and [`SpawnChildrenPlugin`]: prefabs spawned once
//!   as an entity's children.
//! - **Overlap.** [`Zone`], [`ZoneEntered`] and [`ZonePlugin`]: a sensor
//!   entered by the entities whose tags pass its filter. [`TriggerZone`]
//!   and [`TriggerEntered`]: a sensor of one game type entered by an entity
//!   of another.
//! - **Damage.** [`Health`], [`damage`], [`Hit`], [`Damaged`], [`HitFlash`]
//!   and [`HealthPlugin`]: health with an invulnerability window and a hit
//!   flash on the `damage_flash` material ([`HIT_FLASH_MATERIAL`]).
//! - **React.** [`Cues`], [`CueAction`], [`CueEvent`], [`CuesOnEvent`] and
//!   [`CuesPlugin`]: the one way to react to events in data, an entity's
//!   `cues` and the shared `cue_rules` kind. [`Counters`], [`Flags`] and
//!   [`CountersPlugin`]: named counts and switches, an entity's or the
//!   game's, that cues write.
//! - **Present.** [`AnimationPlugin`]: an animated sprite, whose
//!   `AnimationState` writes its current frame into `Sprite.asset_id`.
//!   [`CameraTarget`] and [`CameraTargetPlugin`]: the marker the camera
//!   follows. [`SoundOnEvent`]: a manifest sound played once per event.
//!   [`LabelStyle`]: how a cue's `label` floats an amount.
//! - **Transform.** [`spawn_child`], [`Parent`], [`Children`],
//!   [`LocalOffset`], [`despawn_recursive`] and [`HierarchyPlugin`]: the
//!   hierarchy that moves children with their parents' positions after the
//!   physics sync. Physics bodies live on root entities (`D-132`).
//!
//! # Seeing the kit
//!
//! Every kit component has inspector rows, which the umbrella's inspector
//! prints for a picked entity. [`KitDebugPlugin`] draws each item with a
//! shape through core's `DebugDraw` while the [`KitDebug`] resource is on.
//! Each scene component has a [`ComponentDescription`] in the
//! [`KitDescriptions`] resource, as does each shape its fields refer to:
//! its fields with their types and defaults, its events and the genres
//! that use it, rendered into
//! `api/tungsten-kit-components.txt` by `just api` and compared by
//! `just api-check`, so a changed default or a renamed field is a visible
//! diff (`D-107`). A test holds each description to its DTO: the sample
//! round-trips to the described defaults, every enum variant deserializes
//! and every numeric field refuses what lies outside its bounds.
//!
//! The engine's genre fixtures (`crates/tungsten/tests/fixtures/kit/`) are
//! the cookbook: one folder of prefabs and data per genre, each item in at
//! least two of them.

#![deny(missing_docs)]

mod damage;
mod foundation;
mod intent;
mod lifecycle;
mod motion;
mod overlap;
mod present;
mod react;
mod transform;

pub use damage::health::{
    Damaged, HEALTH_TICK, HIT_FLASH_MATERIAL, HIT_FLASH_TICK, Health, HealthPlugin, Hit, HitFlash,
    damage,
};
pub use foundation::debug::{KIT_DEBUG_DRAW, KitDebug, KitDebugPlugin};
pub use foundation::describe::{
    Bound, ComponentDescription, FieldDescription, FieldType, KitDescriptions,
};
pub use foundation::phases::{
    KIT_INTENT, KIT_MOVE, KIT_OVERLAP, KIT_PRESENT, KIT_REACT, KIT_RESOLVE, KIT_TICK, Phase,
    ensure_phases,
};
pub use foundation::plugins::KitPlugins;
pub use foundation::rng::KitRng;
pub use foundation::source::{Source, SourcePlugin};
pub use foundation::stats::{Modifier, STATS_APPLY, Stats, StatsPlugin};
pub use foundation::tags::{TagFilter, Tags, TagsPlugin};
pub use foundation::trace::{KIT_TRACE_STEP, KitTrace, TraceEntry, TraceKind};
pub use intent::action_input::{ACTION_INPUT, ActionInput, ActionInputPlugin, Intent};
pub use intent::cursor::cursor_to_world;
pub use intent::state_machine::{StateMachine, StateMachinePlugin};
pub use lifecycle::children::{SPAWN_CHILDREN, SpawnChildren, SpawnChildrenPlugin};
pub use lifecycle::cooldown::{COOLDOWN_TICK, Cooldown, CooldownElapsed, CooldownPlugin};
pub use lifecycle::despawn::{DESPAWN_OUTSIDE, DespawnOutside, DespawnOutsidePlugin};
pub use lifecycle::lifetime::{LIFETIME_TICK, Lifetime, LifetimePlugin};
pub use lifecycle::spawner::{SPAWNER_TICK, Spawner, SpawnerPlugin};
pub use motion::path::{PATH_FOLLOW, PathFollower, PathFollowerPlugin};
pub use motion::platformer::{
    PLATFORMER_CONTROLLER, PlatformerController, PlatformerControllerPlugin,
};
pub use motion::top_down::{TOP_DOWN_MOVER, TopDownMover, TopDownMoverPlugin};
pub use overlap::trigger::{TriggerEntered, TriggerZone};
pub use overlap::zone::{ZONE_TICK, Zone, ZoneEntered, ZonePlugin};
pub use present::animation::{ANIMATION_TICK, AnimationPlugin};
pub use present::camera::{CAMERA_TARGET, CameraTarget, CameraTargetPlugin};
pub use present::labels::{LABEL_TICK, LabelStyle};
pub use present::sound::SoundOnEvent;
pub use react::counters::{Counters, CountersPlugin, Flags};
pub use react::cues::{CUES, CueAction, CueEvent, Cues, CuesOnEvent, CuesPlugin};
pub use transform::hierarchy::{
    Children, HierarchyPlugin, LocalOffset, Parent, TRANSFORM_PROPAGATE, TRANSFORM_PROPAGATE_LATE,
    despawn_recursive, spawn_child,
};

/// The plugins, components, events and helpers in one import:
/// `use tungsten::kit::prelude::*;`. The system-name constants stay at the
/// root.
pub mod prelude {
    pub use crate::{
        ActionInput, ActionInputPlugin, AnimationPlugin, CameraTarget, CameraTargetPlugin,
        Children, Cooldown, CooldownElapsed, CooldownPlugin, Counters, CountersPlugin, CueAction,
        CueEvent, Cues, CuesOnEvent, CuesPlugin, Damaged, DespawnOutside, DespawnOutsidePlugin,
        Flags, Health, HealthPlugin, HierarchyPlugin, Hit, HitFlash, Intent, KitDebug,
        KitDebugPlugin, KitPlugins, KitRng, KitTrace, LabelStyle, Lifetime, LifetimePlugin,
        LocalOffset, Modifier, Parent, PathFollower, PathFollowerPlugin, Phase,
        PlatformerController, PlatformerControllerPlugin, SoundOnEvent, Source, SourcePlugin,
        SpawnChildren, SpawnChildrenPlugin, Spawner, SpawnerPlugin, StateMachine,
        StateMachinePlugin, Stats, StatsPlugin, TagFilter, Tags, TagsPlugin, TopDownMover,
        TopDownMoverPlugin, TraceEntry, TraceKind, TriggerEntered, TriggerZone, Zone, ZoneEntered,
        ZonePlugin, cursor_to_world, damage, despawn_recursive, ensure_phases, spawn_child,
    };
}

/// Seconds a countdown treats as already over, so a window of whole steps
/// ends on its last step whatever the sum's rounding.
const TIME_EPSILON: f32 = 1e-5;

/// `left` counted down by `dt`, never below 0.
fn count_down(left: f32, dt: f32) -> f32 {
    let rest = left - dt;
    if rest > TIME_EPSILON { rest } else { 0.0 }
}
