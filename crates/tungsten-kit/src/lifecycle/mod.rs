//! Lifecycle: lifetimes, despawn outside a box or the view, spawners,
//! cooldowns, child spawns, caps on a group's live instances and the kit's
//! end-of-step despawn.

pub(crate) mod cap;
pub(crate) mod children;
pub(crate) mod cooldown;
pub(crate) mod despawn;
pub(crate) mod ended;
pub(crate) mod lifetime;
pub(crate) mod spawner;
