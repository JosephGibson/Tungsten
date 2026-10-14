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
//! An item joins the kit when it is generic and has a user (the template, a
//! migrated example or the acceptance game), with rustdoc that carries an
//! example and a harness test. An item with no user by the 1.0 freeze moves
//! to 1.x.
//!
//! Every item is opt-in: a game adds the plugins it uses, after the
//! engine's (`App::add_plugin`), and `DefaultPlugins` holds none of them.
//! Each system runs under a public name, or under the name the game gives a
//! generic plugin, and its constraints on the engine's names and on the
//! other kit plugins apply only when those are registered, so leaving a
//! plugin out never fails startup. A system runs in `PostUpdate`, except
//! those that drive a body or count a window on the fixed-step clock: they
//! run in `FixedUpdate` before `physics_step`, in this order after the
//! history snapshot, and the trigger zones after the step (`D-158`).
//!
//! 1. [`HEALTH_TICK`]: invulnerability windows.
//! 2. A [`StateMachinePlugin`]'s system: per-entity states.
//! 3. [`PATH_FOLLOW`]: bodies along paths.
//! 4. [`PLATFORMER_CONTROLLER`] and [`TOP_DOWN_MOVER`]: the controllers.
//! 5. `physics_step`, then a [`TriggerZone`]'s system.
//!
//! A game's input system for a controller runs in `FixedUpdate` before it.
//!
//! - [`AnimationPlugin`]: an animated sprite, whose `AnimationState` writes
//!   its current frame into `Sprite.asset_id`.
//! - [`Lifetime`] and [`LifetimePlugin`]: an entity despawned once its time
//!   on the game clock has passed.
//! - [`DespawnOutside`] and [`DespawnOutsidePlugin`]: an entity despawned
//!   once it leaves a world-space box or the camera's view.
//! - [`SoundOnEvent`]: a manifest sound played once per event.
//! - [`Spawner`] and [`SpawnerPlugin`]: a prefab spawned at an entity's
//!   position once per interval of game time.
//! - [`cursor_to_world`]: the world point under the cursor.
//! - The transform hierarchy: [`spawn_child`], [`Parent`], [`Children`],
//!   [`LocalOffset`], [`despawn_recursive`] and [`HierarchyPlugin`], which
//!   moves children with their parents' positions after the physics sync.
//!   Physics bodies live on root entities (`D-132`).
//! - [`PlatformerController`] and [`PlatformerControllerPlugin`]: a
//!   platformer character on a kinematic body, with a ground check, coyote
//!   time, a jump buffer and variable jump height.
//! - [`TopDownMover`] and [`TopDownMoverPlugin`]: a top-down character on a
//!   kinematic body that slides along walls.
//! - [`Health`], [`damage`], [`Hit`], [`Damaged`], [`HitFlash`] and
//!   [`HealthPlugin`]: health with an invulnerability window and a hit
//!   flash on the `damage_flash` material ([`HIT_FLASH_MATERIAL`]).
//! - [`TriggerZone`] and [`TriggerEntered`]: a sensor of one game type
//!   entered by an entity of another.
//! - [`PathFollower`] and [`PathFollowerPlugin`]: a kinematic body moved
//!   along points, such as a moving platform.
//! - [`StateMachine`] and [`StateMachinePlugin`]: a per-entity state with
//!   enter, exit and update hooks.

#![deny(missing_docs)]

mod animation;
mod cursor;
mod despawn;
mod health;
mod hierarchy;
mod lifetime;
mod path;
mod platformer;
mod slide;
mod sound;
mod spawner;
mod state_machine;
mod top_down;
mod trigger;

pub use animation::{ANIMATION_TICK, AnimationPlugin};
pub use cursor::cursor_to_world;
pub use despawn::{DESPAWN_OUTSIDE, DespawnOutside, DespawnOutsidePlugin};
pub use health::{
    Damaged, HEALTH_TICK, HIT_FLASH_MATERIAL, HIT_FLASH_TICK, Health, HealthPlugin, Hit, HitFlash,
    damage,
};
pub use hierarchy::{
    Children, HierarchyPlugin, LocalOffset, Parent, TRANSFORM_PROPAGATE, TRANSFORM_PROPAGATE_LATE,
    despawn_recursive, spawn_child,
};
pub use lifetime::{LIFETIME_TICK, Lifetime, LifetimePlugin};
pub use path::{PATH_FOLLOW, PathFollower, PathFollowerPlugin};
pub use platformer::{PLATFORMER_CONTROLLER, PlatformerController, PlatformerControllerPlugin};
pub use sound::SoundOnEvent;
pub use spawner::{SPAWNER_TICK, Spawner, SpawnerPlugin};
pub use state_machine::{StateMachine, StateMachinePlugin};
pub use top_down::{TOP_DOWN_MOVER, TopDownMover, TopDownMoverPlugin};
pub use trigger::{TriggerEntered, TriggerZone};

/// Seconds a countdown treats as already over, so a window of whole steps
/// ends on its last step whatever the sum's rounding.
const TIME_EPSILON: f32 = 1e-5;

/// `left` counted down by `dt`, never below 0.
fn count_down(left: f32, dt: f32) -> f32 {
    let rest = left - dt;
    if rest > TIME_EPSILON { rest } else { 0.0 }
}
