//! The gun: `shoot` held fires the player's `gun` ability at its cooldown,
//! its ability's charges a magazine ([`most_rounds`]), all back [`RELOAD_SECS`] after the last
//! or after a `reload` press, while no round fires and the trigger clicks;
//! its spawn payload fires a `bullet`, the kit's projectile, which 01 adopts
//! as it adopts the missile. The ability's charges count the rounds; 01 times
//! the reload and refills them, since the kit's tick starts an `all`
//! refill at the first round rather than the last, so the data's refill is
//! an hour, long past any reload. Each step, one decision a bullet: what its step met
//! first, the kit's hurtbox hit, its wall or range end, or a ball or iron
//! block, which the kit's pass never sees. A hit deals physical damage that
//! can crit; a ball takes a kick and ends the shot, a frozen one
//! shattering; iron turns it, twice at most. The HUD shows the magazine.

pub mod components;
pub mod systems;

use glam::{Vec2, Vec3};
use tungsten::core::{
    CommandBuffer, DataRegistry, Position, PrevPosition, Schedule, Stage, Transform, Visibility,
    World, system,
};
use tungsten::kit::{
    ABILITY_TICK, Abilities, Ability, LINEAR_MOTION, LinearMotion, PROJECTILE_TICK, Payload, Phase,
    Projectile, ensure_phases,
};
use tungsten::{Plugin, SceneEntity};

use crate::game::GAMEPLAY;
use crate::shared::{SaveId, prefab_part};
use components::Bullet;

/// The action that fires, held to repeat at the cooldown, and the one that
/// reloads a magazine short of its rounds.
pub const SHOOT: &str = "shoot";
pub const RELOAD: &str = "reload";
/// The player's ability, and the prefab its payload fires.
pub const GUN: &str = "gun";
pub const BULLET_PREFAB: &str = "bullet";
/// The walls' answers, named world cues (`data/world_cues/`): the kit's
/// turn off an iron wall's sparks, a wooden deck passed's splinters and a
/// wall end's dust.
pub const SHOT_SPARKED: &str = "shot_sparked";
pub const SHOT_SPLINTERED: &str = "shot_splintered";
pub const SHOT_DUSTED: &str = "shot_dusted";
/// A hit's physical damage and push, px/s.
pub const SHOT_DAMAGE: f32 = 6.0;
pub const SHOT_PUSH: f32 = 120.0;
/// The kick a shot gives a ball along its flight, px/s over the ball's
/// mass.
pub const BALL_KICK: f32 = 900.0;
/// A muzzle flash's frames and the seconds each shows, and the warm point
/// light it carries over them, fading: its colour, reach and intensity.
pub const MUZZLE_FRAMES: [&str; 3] = ["ex10_muzzle_0", "ex10_muzzle_1", "ex10_muzzle_2"];
pub const MUZZLE_FRAME_SECS: f32 = 0.03;
pub const MUZZLE_LIGHT_COLOR: Vec3 = Vec3::new(1.0, 0.72, 0.36);
pub const MUZZLE_LIGHT_REACH: f32 = 140.0;
pub const MUZZLE_LIGHT: f32 = 2.0;
/// The seconds a reload takes, from the last round or a `reload` press,
/// until the magazine is whole again.
pub const RELOAD_SECS: f32 = 1.2;

/// The gun as a plugin. In the fixed step: in the intent phase after the
/// kit's ability tick, the reload and the trigger; in the move phase after the movers,
/// each bullet's path; in the overlap phase after the kit's projectile
/// pass and before the cascades' run, the hits and the walls' answers. In `PreUpdate` before the
/// shell's save, the bullets the last flush landed taken in; in the present
/// phase the muzzle flashes.
pub struct GunPlugin;

impl Plugin for GunPlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        world.register_component::<Bullet>("bullet");
        world.register_saved_with(
            "bullet",
            tungsten::core::save::SaveOptions::<Bullet>::form::<components::BulletForm>(),
        );
        world.register_saved::<components::Reload>("reload");
        ensure_phases(schedule);
        let shoot = system("gun_shoot", systems::shoot).after(ABILITY_TICK);
        schedule.add(Stage::FixedUpdate, Phase::Intent.join(shoot));
        let paths = system("bullet_paths", systems::paths).after(LINEAR_MOTION);
        schedule.add(Stage::FixedUpdate, Phase::Move.join(paths));
        let mut cursors = systems::ShotCursors::default();
        let hits = system("gun_hits", move |world: &mut World| {
            systems::hits(world, &mut cursors);
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

/// The magazine's rounds at most, the `gun` ability's `charges.max` as
/// the data holds it now; none without the ability.
#[must_use]
pub fn most_rounds(world: &World) -> Option<u32> {
    let (most, ..) = world
        .get_resource::<DataRegistry>()?
        .get::<Ability>(GUN)?
        .charges()?;
    Some(most)
}

/// The player's magazine: its rounds left and its most ([`most_rounds`]);
/// none without the ability.
#[must_use]
pub fn rounds(world: &World, player: tungsten::core::Entity) -> Option<(u32, u32)> {
    let most = most_rounds(world)?;
    let left = world.get::<Abilities>(player)?.charges(GUN).unwrap_or(most);
    Some((left, most))
}

/// A bullet's hitbox as its prefab sets it: its centre in its tracer's
/// art, 32 × 3 along its flight, and its circle's radius there.
///
/// # Panics
///
/// When the prefab has no circle hitbox: it loads with the manifest.
#[must_use]
pub fn hitbox(world: &World) -> (Vec2, f32) {
    let offset = prefab_part::<[f32; 2]>(world, BULLET_PREFAB, &["hitbox", "offset"]);
    let radius = prefab_part::<f32>(world, BULLET_PREFAB, &["hitbox", "shape", "circle"]);
    match (offset, radius) {
        (Some(offset), Some(radius)) => (Vec2::from(offset), radius),
        _ => {
            panic!("the `{BULLET_PREFAB}` prefab has no circle hitbox: it loads with the manifest")
        }
    }
}

/// How far along the aim the shot leaves the courier's centre, the `gun`
/// ability's spawn payload's offset; 0 without one.
#[must_use]
pub fn muzzle_offset(world: &World) -> f32 {
    let spawn = |payload: &Payload| match payload {
        Payload::Spawn { offset, .. } => Some(*offset),
        Payload::SpawnWith(spawn) => Some(spawn.offset()),
        _ => None,
    };
    world
        .get_resource::<DataRegistry>()
        .and_then(|data| data.get::<Ability>(GUN))
        .and_then(|gun| gun.payload().iter().find_map(spawn))
        .unwrap_or(0.0)
}

/// A bullet's turn along `velocity`: its tracer's art lies along +x.
#[must_use]
pub fn heading(velocity: Vec2) -> f32 {
    velocity.y.atan2(velocity.x)
}

/// Fires a bullet with its centre at `centre` at `velocity`, from
/// `source`, credited and spared as the gun's payload would: spawned at the
/// frame's flush, one of the run's records, its flight and look its own, so
/// `bullet_adopt` leaves it be. Its save ID.
pub fn fire(
    world: &mut World,
    centre: Vec2,
    velocity: Vec2,
    source: Option<tungsten::core::Entity>,
) -> Option<SaveId> {
    let bullet = BulletShot {
        centre,
        motion: LinearMotion::new(velocity),
        projectile: None,
        ricochets: 0,
    };
    spawn(world, bullet, source)
}

/// A bullet to spawn: its centre, its flight, its range so far (a fresh
/// one's when none) and the times it turned off iron.
#[derive(Debug, Clone)]
pub struct BulletShot {
    pub centre: Vec2,
    pub motion: LinearMotion,
    pub projectile: Option<Projectile>,
    pub ricochets: u32,
}

/// Spawns `shot` at the frame's flush from `source`, so its source and its
/// emitter come with it, shown and turned along its flight, as one of the
/// run's records (`PreparedPrefab::saved`, the map's area). Its save ID,
/// none when its prefab did not load.
pub fn spawn(
    world: &mut World,
    shot: BulletShot,
    source: Option<tungsten::core::Entity>,
) -> Option<SaveId> {
    let at = shot.centre - hitbox(world).0;
    let prepared = world.prepare_prefab(BULLET_PREFAB, at.to_array()).ok()?;
    let transform = Transform {
        position: at,
        rotation: heading(shot.motion.velocity()),
        scale: Vec2::ONE,
    };
    let mut prepared = prepared.with((
        transform,
        Position(at),
        PrevPosition(at),
        crate::shared::Joined,
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
    let prepared = prepared.saved(world, Some(crate::features::level::MAP));
    let id = prepared.save_id();
    let commands = world.resource_mut::<CommandBuffer>();
    match source {
        Some(source) => {
            commands.spawn_prefab_from(source, prepared);
        }
        None => {
            commands.spawn_prefab(prepared);
        }
    }
    id
}
