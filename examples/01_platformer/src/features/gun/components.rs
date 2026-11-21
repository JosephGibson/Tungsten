//! The gun's components: the bullet its spawn payload fires, a shot's
//! muzzle flash, and the reload an empty magazine waits through.

use glam::Vec2;
use serde::{Deserialize, Serialize};

/// A bullet, the gun's spawn: the kit's projectile on its linear motion,
/// whose step 01 decides in `gun_hits`. Its prefab names it `bullet`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Bullet {
    /// The times it turned off iron.
    pub ricochets: u32,
    /// Its centre's path this step, from the move phase, before the kit's
    /// pass moves it back: the step's, never saved.
    #[serde(skip)]
    pub segment: Option<(Vec2, Vec2)>,
}

/// [`Bullet`]'s saved form: its ricochets; its segment is the step's.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BulletForm {
    pub ricochets: u32,
}

impl From<&Bullet> for BulletForm {
    fn from(bullet: &Bullet) -> Self {
        Self {
            ricochets: bullet.ricochets,
        }
    }
}

impl From<BulletForm> for Bullet {
    fn from(form: BulletForm) -> Self {
        Self {
            ricochets: form.ricochets,
            segment: None,
        }
    }
}

/// A shot's muzzle flash, its frames shown in turn.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Muzzle {
    pub age: f32,
}

/// The player's magazine coming back, from its last round or a `reload`
/// press: the seconds left until every round does, while no round fires.
/// Saved.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reload {
    pub left: f32,
}
