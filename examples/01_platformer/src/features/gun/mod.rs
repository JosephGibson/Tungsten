//! The gun: `shoot` held fires the player's `gun` ability at its cooldown,
//! twelve rounds a magazine, all back [`RELOAD_SECS`] after the last; its
//! spawn payload fires a `bullet`, the kit's projectile, which 01 adopts as
//! it adopts the missile. The ability's charges count the rounds; 01 times
//! the reload and refills them, since the kit's tick starts an `all`
//! refill at the first round rather than the last, so the data's refill is
//! an hour, long past any reload. Each step, one decision a bullet: what its step met
//! first, the kit's hurtbox hit, its wall or range end, or a ball or iron
//! block, which the kit's pass never sees. A hit deals physical damage that
//! can crit; a ball takes a kick and ends the shot, a frozen one
//! shattering; iron turns it, twice at most. The HUD shows the magazine.

pub mod components;
pub mod systems;

use glam::Vec2;
use tungsten::core::{
    CommandBuffer, DataRegistry, EventCursor, Position, PrevPosition, Schedule, Stage, Transform,
    Visibility, World, system,
};
use tungsten::kit::{
    ABILITY_TICK, Abilities, Ability, LINEAR_MOTION, LinearMotion, PROJECTILE_TICK, Phase,
    Projectile, ensure_phases,
};
use tungsten::{Plugin, SceneEntity};

use crate::game::GAMEPLAY;
use crate::shared::SaveId;
use components::Bullet;

/// The action that fires, held to repeat at the cooldown.
pub const SHOOT: &str = "shoot";
/// The player's ability, and the prefab its payload fires.
pub const GUN: &str = "gun";
pub const BULLET_PREFAB: &str = "bullet";
/// A bullet's centre in its tracer's art, 32 × 3 along its flight, and its
/// hitbox's radius there.
pub const BULLET_CENTRE: Vec2 = Vec2::new(16.0, 1.5);
pub const BULLET_RADIUS: f32 = 2.0;
/// A hit's physical damage and push, px/s.
pub const SHOT_DAMAGE: f32 = 6.0;
pub const SHOT_PUSH: f32 = 120.0;
/// The kick a shot gives a ball along its flight, px/s over the ball's
/// mass.
pub const BALL_KICK: f32 = 900.0;
/// The most times a bullet turns off iron.
pub const MAX_RICOCHETS: u32 = 2;
/// A muzzle flash's frames and the seconds each shows.
pub const MUZZLE_FRAMES: [&str; 3] = ["ex10_muzzle_0", "ex10_muzzle_1", "ex10_muzzle_2"];
pub const MUZZLE_FRAME_SECS: f32 = 0.03;
/// How far along the aim the shot leaves the courier's centre, as the
/// ability's payload sets it.
pub const MUZZLE_OFFSET: f32 = 24.0;
/// The seconds after the last round until the magazine is whole again.
pub const RELOAD_SECS: f32 = 1.2;

/// The gun as a plugin. In the fixed step: in the intent phase after the
/// kit's ability tick, the reload and the trigger; in the move phase after the movers,
/// each bullet's path; in the overlap phase after the kit's projectile
/// pass and before the cascades' run, the hits. In `PreUpdate` before the
/// shell's save, the bullets the last flush landed taken in; in the present
/// phase the muzzle flashes.
pub struct GunPlugin;

impl Plugin for GunPlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        world.register_component::<Bullet>("bullet");
        ensure_phases(schedule);
        let shoot = system("gun_shoot", systems::shoot).after(ABILITY_TICK);
        schedule.add(Stage::FixedUpdate, Phase::Intent.join(shoot));
        let paths = system("bullet_paths", systems::paths).after(LINEAR_MOTION);
        schedule.add(Stage::FixedUpdate, Phase::Move.join(paths));
        let (mut struck, mut expired) = (EventCursor::new(), EventCursor::new());
        let hits = system("gun_hits", move |world: &mut World| {
            systems::hits(world, &mut struck, &mut expired);
        })
        .after(PROJECTILE_TICK)
        .before("cascade_run");
        schedule.add(Stage::FixedUpdate, Phase::Overlap.join(hits));
        // Before the shell's save captures them, so a save holds them whole.
        let adopt = system("bullet_adopt", systems::adopt).before("shell_save");
        schedule.add(Stage::PreUpdate, adopt);
        schedule.add(
            Stage::PostUpdate,
            Phase::Present.join(system("muzzle_flashes", systems::muzzles)),
        );
    }
}

/// The player's magazine: its rounds left and its most, from the `gun`
/// ability's charges; none without the ability.
#[must_use]
pub fn rounds(world: &World, player: tungsten::core::Entity) -> Option<(u32, u32)> {
    let (most, ..) = world
        .get_resource::<DataRegistry>()?
        .get::<Ability>(GUN)?
        .charges()?;
    let left = world.get::<Abilities>(player)?.charges(GUN).unwrap_or(most);
    Some((left, most))
}

/// A bullet's turn along `velocity`: its tracer's art lies along +x.
#[must_use]
pub fn heading(velocity: Vec2) -> f32 {
    velocity.y.atan2(velocity.x)
}

/// Fires a bullet with its centre at `centre` at `velocity`, from
/// `source`, credited and spared as the gun's payload would: spawned at the
/// frame's flush, its save ID, flight and look its own, so `bullet_adopt`
/// leaves it be. Its save ID.
pub fn fire(
    world: &mut World,
    centre: Vec2,
    velocity: Vec2,
    source: Option<tungsten::core::Entity>,
) -> Option<SaveId> {
    let id = world.resource_mut::<crate::shared::SaveIds>().take();
    let bullet = BulletShot {
        id,
        centre,
        motion: LinearMotion::new(velocity),
        projectile: None,
        ricochets: 0,
    };
    spawn(world, bullet, source).then_some(id)
}

/// A bullet to spawn: its save ID, its centre, its flight, its range so
/// far (a fresh one's when none) and the times it turned off iron.
#[derive(Debug, Clone)]
pub struct BulletShot {
    pub id: SaveId,
    pub centre: Vec2,
    pub motion: LinearMotion,
    pub projectile: Option<Projectile>,
    pub ricochets: u32,
}

/// Spawns `shot` at the frame's flush from `source`, so its source and its
/// emitter come with it, shown and turned along its flight. Whether its
/// prefab loaded.
pub fn spawn(world: &mut World, shot: BulletShot, source: Option<tungsten::core::Entity>) -> bool {
    let at = shot.centre - BULLET_CENTRE;
    let Ok(prepared) = world.prepare_prefab(BULLET_PREFAB, at.to_array()) else {
        return false;
    };
    let transform = Transform {
        position: at,
        rotation: heading(shot.motion.velocity()),
        scale: Vec2::ONE,
    };
    let mut prepared = prepared.with((
        transform,
        Position(at),
        PrevPosition(at),
        shot.id,
        shot.motion,
        Bullet {
            ricochets: shot.ricochets,
            segment: None,
        },
        Visibility { visible: true },
        SceneEntity { state_id: GAMEPLAY },
    ));
    if let Some(projectile) = shot.projectile {
        prepared = prepared.with((projectile,));
    }
    let commands = world.resource_mut::<CommandBuffer>();
    match source {
        Some(source) => {
            commands.spawn_prefab_from(source, prepared);
        }
        None => {
            commands.spawn_prefab(prepared);
        }
    }
    true
}
