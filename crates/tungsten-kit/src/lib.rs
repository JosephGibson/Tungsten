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
//! Each system runs in `PostUpdate` under a public name, and its
//! constraints on the engine's names and on the other kit plugins apply
//! only when those are registered, so leaving a plugin out never fails
//! startup.
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

#![deny(missing_docs)]

mod animation;
mod cursor;
mod despawn;
mod hierarchy;
mod lifetime;
mod sound;
mod spawner;

pub use animation::{ANIMATION_TICK, AnimationPlugin};
pub use cursor::cursor_to_world;
pub use despawn::{DESPAWN_OUTSIDE, DespawnOutside, DespawnOutsidePlugin};
pub use hierarchy::{
    Children, HierarchyPlugin, LocalOffset, Parent, TRANSFORM_PROPAGATE, TRANSFORM_PROPAGATE_LATE,
    despawn_recursive, spawn_child,
};
pub use lifetime::{LIFETIME_TICK, Lifetime, LifetimePlugin};
pub use sound::SoundOnEvent;
pub use spawner::{SPAWNER_TICK, Spawner, SpawnerPlugin};
