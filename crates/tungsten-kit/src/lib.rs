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
//!   movers, health, cooldowns and hit items read, and the `stats` data
//!   kind that declares a game's. [`Curve`]: a piecewise-linear map, the
//!   `curves` kind that effects scale by and levels climb. [`KitRng`]: the
//!   kit's seeded random stream.
//!   [`KitTrace`], [`TraceEntry`] and [`TraceKind`]: what the items did,
//!   for a failing recipe or the debug view. [`Phase`], [`ensure_phases`]
//!   and [`KitPlugins`].
//! - **Intent.** [`ActionInput`], [`Intent`] and [`ActionInputPlugin`]:
//!   the action map read into what each entity means to do, which the
//!   movers take. [`StateMachine`] and [`StateMachinePlugin`]: a per-entity
//!   state with enter, exit and update hooks. [`cursor_to_world`]: the
//!   world point under the cursor. [`Aim`] and [`AimPlugin`]: the direction
//!   an entity aims in, from its intent, the cursor, its movement or a
//!   target. [`Ability`], [`Abilities`], [`use_ability`], [`AbilityUsed`]
//!   and [`AbilitiesPlugin`]: what an entity can do on a trigger (an
//!   action, an interval, or the `ability` cue action), at a cost, with a
//!   cooldown and charges, at a target: a spawn, an effect, an impulse or a
//!   hitbox window; the `abilities` data kind and scene component.
//! - **Motion.** [`PlatformerController`] and
//!   [`PlatformerControllerPlugin`]: a platformer character on a kinematic
//!   body, with a ground check, coyote time, a jump buffer and variable
//!   jump height. [`TopDownMover`] and [`TopDownMoverPlugin`]: a top-down
//!   character that slides along walls on a kinematic body, or moves its
//!   `Position` without one. [`PathFollower`] and [`PathFollowerPlugin`]: a
//!   kinematic body moved along points, such as a moving platform.
//!   [`LinearMotion`] and [`LinearMotionPlugin`]: a velocity an
//!   acceleration changes. [`Oscillator`] and [`OscillatorPlugin`]: a sine
//!   wave on the position, rotation or scale. [`Follow`] and
//!   [`FollowPlugin`]: a target's point plus an offset, with a lag.
//!   [`Orbit`] and [`OrbitPlugin`]: a circle about a centre. [`Steering`]
//!   and [`SteeringPlugin`]: seek, flee or arrive. [`Target`]: what those
//!   items aim at. [`Knockback`], [`knock_back`] and [`KnockbackPlugin`]: a
//!   push back from a hit. [`SpeedLimit`] and [`SpeedLimitPlugin`]: a cap on
//!   the speed. [`Wrap`] and [`WrapPlugin`]: back in from the far side of an
//!   area. [`StayInView`] and [`StayInViewPlugin`]: a box clamped inside an
//!   area. A mover writes a root's `Position`, or a body's `Velocity`,
//!   before the physics step, a kinematic body has one kit driver, the
//!   constraints correct after the step, and presentation writes once a
//!   frame, one kit writer per channel.
//! - **Lifecycle.** [`Lifetime`] and [`LifetimePlugin`]: an entity
//!   despawned once its time on the game clock has passed.
//!   [`DespawnOutside`] and [`DespawnOutsidePlugin`]: one despawned once it
//!   leaves a world-space box or the camera's view. [`Spawner`] and
//!   [`SpawnerPlugin`]: a prefab spawned at an entity's position once per
//!   interval of game time. [`Cooldown`], [`CooldownElapsed`] and
//!   [`CooldownPlugin`]: a repeating countdown with an optional jitter.
//!   [`SpawnChildren`] and [`SpawnChildrenPlugin`]: prefabs spawned once
//!   as an entity's children. [`KIT_DESPAWN`]: the kit's end-of-step
//!   despawn, which takes what the step's items ended after the step's
//!   spawns from them.
//! - **Overlap.** [`Zone`], [`ZoneEntered`] and [`ZonePlugin`]: a sensor
//!   entered by the entities whose tags pass its filter. [`TriggerZone`]
//!   and [`TriggerEntered`]: a sensor of one game type entered by an entity
//!   of another. [`HitShape`], [`Hitbox`], [`Hurtbox`], [`Struck`] and
//!   [`HitPlugin`]: hit shapes on the kit's grid, each hit tested over the
//!   step's motion, so a fast bolt skips nothing; a windowed hitbox hits
//!   while an ability or `arm` keeps it open. [`ContactDamage`],
//!   [`ContactTouch`] and [`ContactDamagePlugin`]: a hazard or an aura that
//!   hurts what it touches, again each interval. [`Projectile`],
//!   [`ProjectileExpired`], [`ProjectileBounced`], [`ProjectilePassed`],
//!   [`WallContact`] and [`ProjectilePlugin`]: a bolt that ends at its first
//!   hit, its first wall or its range, unless it pierces, turns at a wall in
//!   its bounce layers or passes one in its pass layers.
//!   [`Pickup`], [`PickedUp`] and [`PickupPlugin`]: collected once, drawn in
//!   by a magnet.
//! - **Damage.** [`Health`], [`damage`], [`heal`], [`Hit`], [`Damaged`],
//!   [`Healed`], [`HitFlash`] and [`HealthPlugin`]: health with an
//!   invulnerability window, a death policy and a hit flash on the
//!   `damage_flash` material ([`HIT_FLASH_MATERIAL`]). [`DamageRequest`]
//!   and [`HealRequest`]: the damage pipeline that hitboxes, contact damage,
//!   projectiles and a game's Rust share, whose requests carry a source,
//!   damage types, a position, a knockback and a crit flag and resolve at
//!   once or once a step in sorted order, through [`Resist`] and a
//!   [`Shield`] to health. A hit is a list of [`DamagePart`]s, each an
//!   amount with its damage types, which the game declares in the
//!   `damage_types` data kind, none named by the kit: each part goes
//!   through [`Conversion`], [`Penetration`] and the target's [`Resist`],
//!   with its modifiers, on its own, then the parts fall once through the
//!   shield and health, a [`DamagedPart`] out for each part and one
//!   [`Damaged`] for the hit, each carrying its [`DamageKinds`]. [`Effect`],
//!   [`Effects`], [`EffectRequest`], [`EffectApplied`], [`EffectExpired`]
//!   and [`EffectsPlugin`]: timed, infinite or instant changes an entity
//!   carries, the `effects` data kind and scene component: a periodic
//!   payload that passes the window, stacks, modifiers on its stats and
//!   resistances, granted and blocking tags, a scale curve and a
//!   transition at the end, which the cue action `effect` applies.
//!   [`AilmentRung`], [`AilmentThreshold`] and [`AilmentBuckets`]: the
//!   ladder on a `damage_types` definition that a target's bucket of that
//!   type climbs against its threshold, each rung an effect.
//! - **React.** [`Cues`], [`CueAction`], [`CueEvent`], [`CuesOnEvent`] and
//!   [`CuesPlugin`]: the one way to react to events in data, an entity's
//!   `cues` and the shared `cue_rules` kind, whose `effect` action applies
//!   an effect to the subject or the other entity. [`WeightedTable`]: the
//!   `loot_tables` kind that a cue's `roll` draws drops from. [`Counters`],
//!   [`Flags`] and [`CountersPlugin`]: named counts and switches, an
//!   entity's or the game's, that cues write. [`Level`], [`Offers`],
//!   [`LevelUp`] and [`LevelPlugin`]: a counter climbed along a curve,
//!   each level up an event and a draw of offers, an effect or an ability
//!   the game's choice applies; the `offers` data kind and the `level`
//!   scene component.
//! - **Present.** [`AnimationPlugin`]: an animated sprite, whose
//!   `AnimationState` writes its current frame into `Sprite.asset_id`.
//!   [`CharacterAnimator`], [`Clips`] and [`CharacterAnimatorPlugin`]: the
//!   clip a character's motion and facing pick. [`Facing`] and
//!   [`FacingPlugin`]: the way an entity faces, turning it when asked.
//!   [`DepthSort`] and [`DepthSortPlugin`]: a sprite sorted by how low it
//!   stands. [`CameraTarget`] and [`CameraTargetPlugin`]: the marker the
//!   camera follows. [`SoundOnEvent`]: a manifest sound played once per
//!   event. [`LabelStyle`] and [`float_label`]: how a cue's `label`, or a
//!   game's own code, floats an amount.
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

pub use damage::ailments::{
    AILMENT_FEED, AILMENT_TICK, AilmentBuckets, AilmentRung, AilmentThreshold,
};
pub use damage::defense::{Resist, SHIELD_TICK, Shield};
pub use damage::effects::{
    Applied, EFFECT_RESOLVE, EFFECT_TICK, Effect, EffectApplied, EffectExpired, EffectInstance,
    EffectRequest, Effects, EffectsPlugin, Expire, Refresh, Scaled, end_effect,
};
pub use damage::health::{
    Damaged, DamagedPart, HEALTH_TICK, HIT_FLASH_MATERIAL, HIT_FLASH_TICK, Health, HealthPlugin,
    Hit, HitFlash, damage,
};
pub use damage::kinds::{Conversion, DamageKinds, Penetration};
pub use damage::pipeline::{DAMAGE_RESOLVE, DamagePart, DamageRequest, HealRequest, Healed, heal};
pub use foundation::curve::Curve;
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
pub use intent::ability::{
    ABILITY_TICK, Abilities, AbilitiesPlugin, Ability, AbilityTarget, AbilityUsed, Payload,
    Trigger, Used, use_ability,
};
pub use intent::action_input::{ACTION_INPUT, ActionInput, ActionInputPlugin, Intent};
pub use intent::aim::{AIM, Aim, AimPlugin};
pub use intent::cursor::cursor_to_world;
pub use intent::state_machine::{StateMachine, StateMachinePlugin};
pub use lifecycle::children::{SPAWN_CHILDREN, SpawnChildren, SpawnChildrenPlugin};
pub use lifecycle::cooldown::{COOLDOWN_TICK, Cooldown, CooldownElapsed, CooldownPlugin};
pub use lifecycle::despawn::{DESPAWN_OUTSIDE, DespawnOutside, DespawnOutsidePlugin};
pub use lifecycle::ended::KIT_DESPAWN;
pub use lifecycle::lifetime::{LIFETIME_TICK, Lifetime, LifetimePlugin};
pub use lifecycle::spawner::{SPAWNER_TICK, Spawner, SpawnerPlugin};
pub use motion::follow::{FOLLOW, Follow, FollowPlugin};
pub use motion::knockback::{KNOCKBACK, Knockback, KnockbackPlugin, knock_back};
pub use motion::linear::{LINEAR_MOTION, LinearMotion, LinearMotionPlugin};
pub use motion::orbit::{ORBIT, Orbit, OrbitPlugin};
pub use motion::oscillator::{OSCILLATOR, OSCILLATOR_PRESENT, Oscillator, OscillatorPlugin};
pub use motion::path::{PATH_FOLLOW, PathFollower, PathFollowerPlugin};
pub use motion::platformer::{
    PLATFORMER_CONTROLLER, PlatformerController, PlatformerControllerPlugin,
};
pub use motion::speed_limit::{SPEED_LIMIT, SPEED_LIMIT_SETTLE, SpeedLimit, SpeedLimitPlugin};
pub use motion::stay_in_view::{STAY_IN_VIEW, StayInView, StayInViewPlugin};
pub use motion::steering::{STEERING, Steering, SteeringPlugin};
pub use motion::target::Target;
pub use motion::top_down::{TOP_DOWN_MOVER, TopDownMover, TopDownMoverPlugin};
pub use motion::wrap::{WRAP, Wrap, WrapPlugin};
pub use overlap::contact::{CONTACT_DAMAGE, ContactDamage, ContactDamagePlugin, ContactTouch};
pub use overlap::hit::{HIT_GRID, HITBOX_TICK, HitPlugin, HitShape, Hitbox, Hurtbox, Struck};
pub use overlap::pickup::{PICKUP_MAGNET, PICKUP_TICK, PickedUp, Pickup, PickupPlugin};
pub use overlap::projectile::{
    PROJECTILE_TICK, Projectile, ProjectileBounced, ProjectileExpired, ProjectilePassed,
    ProjectilePlugin, WallContact,
};
pub use overlap::trigger::{TriggerEntered, TriggerZone};
pub use overlap::zone::{ZONE_TICK, Zone, ZoneEntered, ZonePlugin};
pub use present::animation::{ANIMATION_TICK, AnimationPlugin};
pub use present::camera::{CAMERA_TARGET, CameraTarget, CameraTargetPlugin};
pub use present::character::{
    CHARACTER_ANIMATOR, CharacterAnimator, CharacterAnimatorPlugin, Clips,
};
pub use present::depth::{DEPTH_SORT, DepthSort, DepthSortPlugin};
pub use present::facing::{FACING, Facing, FacingPlugin};
pub use present::labels::{LABEL_TICK, LabelStyle, float_label};
pub use present::sound::SoundOnEvent;
pub use react::counters::{Counters, CountersPlugin, Flags};
pub use react::cues::{CUES, CueAction, CueEvent, Cues, CuesOnEvent, CuesPlugin};
pub use react::level::{LEVEL_TICK, Level, LevelPlugin, LevelUp, Offer, Offers};
pub use react::loot::WeightedTable;
pub use transform::hierarchy::{
    Children, HierarchyPlugin, LocalOffset, Parent, TRANSFORM_PROPAGATE, TRANSFORM_PROPAGATE_LATE,
    despawn_recursive, spawn_child,
};

/// The plugins, components, events and helpers in one import:
/// `use tungsten::kit::prelude::*;`. The system-name constants stay at the
/// root.
pub mod prelude {
    pub use crate::{
        Abilities, AbilitiesPlugin, Ability, AbilityTarget, AbilityUsed, ActionInput,
        ActionInputPlugin, AilmentBuckets, AilmentRung, AilmentThreshold, Aim, AimPlugin,
        AnimationPlugin, Applied, CameraTarget, CameraTargetPlugin, CharacterAnimator,
        CharacterAnimatorPlugin, Children, Clips, ContactDamage, ContactDamagePlugin, Conversion,
        Cooldown, CooldownElapsed, CooldownPlugin, Counters, CountersPlugin, CueAction, CueEvent,
        Cues, CuesOnEvent, CuesPlugin, Curve, DamageKinds, DamagePart, DamageRequest, Damaged,
        DamagedPart, DepthSort, DepthSortPlugin, DespawnOutside, DespawnOutsidePlugin, Effect,
        EffectApplied, EffectExpired, EffectInstance, EffectRequest, Effects, EffectsPlugin,
        Expire, Facing, FacingPlugin, Flags, Follow, FollowPlugin, HealRequest, Healed, Health,
        HealthPlugin, HierarchyPlugin, Hit, HitFlash, HitPlugin, HitShape, Hitbox, Hurtbox, Intent,
        KitDebug, KitDebugPlugin, KitPlugins, KitRng, KitTrace, Knockback, KnockbackPlugin,
        LabelStyle, Level, LevelPlugin, LevelUp, Lifetime, LifetimePlugin, LinearMotion,
        LinearMotionPlugin, LocalOffset, Modifier, Offer, Offers, Orbit, OrbitPlugin, Oscillator,
        OscillatorPlugin, Parent, PathFollower, PathFollowerPlugin, Payload, Penetration, Phase,
        PickedUp, Pickup, PickupPlugin, PlatformerController, PlatformerControllerPlugin,
        Projectile, ProjectileBounced, ProjectileExpired, ProjectilePassed, ProjectilePlugin,
        Refresh, Resist, Scaled, Shield, SoundOnEvent, Source, SourcePlugin, SpawnChildren,
        SpawnChildrenPlugin, Spawner, SpawnerPlugin, SpeedLimit, SpeedLimitPlugin, StateMachine,
        StateMachinePlugin, Stats, StatsPlugin, StayInView, StayInViewPlugin, Steering,
        SteeringPlugin, Struck, TagFilter, Tags, TagsPlugin, Target, TopDownMover,
        TopDownMoverPlugin, TraceEntry, TraceKind, Trigger, TriggerEntered, TriggerZone, Used,
        WallContact, WeightedTable, Wrap, WrapPlugin, Zone, ZoneEntered, ZonePlugin,
        cursor_to_world, damage, despawn_recursive, end_effect, ensure_phases, float_label, heal,
        knock_back, spawn_child, use_ability,
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
