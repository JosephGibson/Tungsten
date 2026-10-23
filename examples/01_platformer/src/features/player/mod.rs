//! The player: a courier on the kit's platformer controller, from the
//! `player` prefab, which the camera follows. The prefab holds its kit
//! items: input, the controller with one air jump, the aim, health,
//! hurtbox, knockback, hit flash, the hit's cues, facing, the character
//! animator and the camera target. 01 adds the lantern, whose staff light
//! burns mana, the zoom, the kill row, the bursts its moves raise, the land
//! and air-jump one-shots, a cast's toss, pose and hot lantern, and the
//! jump's shove that throws off what buries it.

pub mod components;
pub mod systems;

use glam::Vec2;
use tungsten::core::{Entity, EventCursor, Schedule, Stage, World, system};
use tungsten::kit::{ACTION_INPUT, CHARACTER_ANIMATOR, CUES, Phase, Stats, ensure_phases};
use tungsten::{Plugin, SceneEntity};

use crate::features::level::GLOWS;
use crate::game::GAMEPLAY;
use crate::shared::{Bursts, SaveIds};
use crate::shell::Difficulty;
use components::{Lantern, LanternAnchors, Moves};

/// The player's prefab, by its manifest ID.
pub const PLAYER_PREFAB: &str = "player";
/// The body's half size and where its centre sits from the sprite's
/// top-left corner, the entity's point: the art's feet stand on its row 61.
pub const PLAYER_HALF: Vec2 = Vec2::new(20.0, 28.0);
pub const BODY_OFFSET: Vec2 = Vec2::new(32.0, 33.0);
/// Game seconds a landing holds the land clip.
pub const LAND_SECS: f32 = 0.18;
/// Game seconds the toss clip plays, and a cast holds its pose and the
/// lantern's glow.
pub const TOSS_SECS: f32 = 0.3;
pub const CAST_SECS: f32 = 0.3;
/// The staff light: its ability, which burns mana each step it is lit, the
/// tag its effect grants while it burns, the mana it relights at after
/// guttering out, and the puff it gutters with (L5's first half, Q15).
pub const STAFF_LIGHT: &str = "staff_light";
pub const STAFF_LIT: &str = "staff.lit";
pub const RELIGHT_MANA: f32 = 10.0;
pub const GUTTER_PUFF: &str = "ex10_staff_gutter";
/// The zoom's range over the window's base, and one press's step.
pub const ZOOM_MIN: f32 = 0.35;
pub const ZOOM_MAX: f32 = 3.0;
pub const ZOOM_STEP: f32 = 0.25;
/// The controller's jump speed, as the prefab sets it.
pub const JUMP_SPEED: f32 = 1280.0;
/// The jump's shove: the bodies over the head and shoulders, in a box this
/// tall and as wide as the courier and [`SHOVE_SIDE`] each side, take
/// [`SHOVE_OVER`] over their mass, faster than the courier's rise, and
/// those beside it, this wide each side, [`SHOVE_BESIDE`]; a jump they
/// blocked, the courier within [`STUCK_BY`] of its press, is launched again
/// at [`JUMP_SPEED`] the next step.
pub const SHOVE_HEAD: f32 = 48.0;
pub const SHOVE_SIDE: f32 = 16.0;
pub const SHOVE_OVER: f32 = 1600.0;
pub const SHOVE_BESIDE: f32 = 600.0;
pub const STUCK_BY: f32 = 2.0;

/// The player's full health at `difficulty`: its `max_health` stat's base.
/// A hazard's touch takes 34, so Normal falls on the third.
#[must_use]
pub const fn max_health(difficulty: Difficulty) -> f32 {
    match difficulty {
        Difficulty::Easy => 150.0,
        Difficulty::Normal => 100.0,
        Difficulty::Hard => 68.0,
    }
}

/// The player as a plugin: the `player` marker its prefab names, the
/// lantern's anchors, its actions and its staff light in the intent phase
/// before the spells, the jump's shove there before the controller, the
/// kill row in the
/// overlap phase, its hurt and its moves' bursts in the react phase, and in
/// the present
/// phase its one-shot clips after the animator and its lantern's glow
/// before the level draws the glows.
pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        // Before the manifest loads, which checks each prefab's components.
        world.register_component::<components::Player>("player");
        world.register_data_kind::<LanternAnchors>("lantern_anchors");
        ensure_phases(schedule);
        let mut cursor = EventCursor::new();
        // Before the kit's cues, which play its sound in the same step.
        let hurt = system("player_hurt", move |world: &mut World| {
            systems::hurt(world, &mut cursor);
        })
        .before(CUES);
        // Before the spells, so a cast at full mana leaves its cost alone.
        let staff = system("player_staff_light", systems::staff_light)
            .after("player_actions")
            .before("spells_cast");
        let unstick = system("player_unstick", systems::unstick).after(ACTION_INPUT);
        let fixed = [
            (Phase::Intent, system("player_actions", systems::actions)),
            (Phase::Intent, staff),
            (Phase::Intent, unstick),
            (Phase::Overlap, system("player_bounds", systems::bounds)),
            (Phase::React, hurt),
            (Phase::React, system("player_moves", systems::moves)),
        ];
        for (phase, desc) in fixed {
            schedule.add(Stage::FixedUpdate, phase.join(desc));
        }
        let clips = system("player_clips", systems::clips).after(CHARACTER_ANIMATOR);
        let lantern = system("player_lantern", systems::lantern)
            .after("player_clips")
            .before(GLOWS);
        for desc in [clips, lantern] {
            schedule.add(Stage::PostUpdate, Phase::Present.join(desc));
        }
    }
}

/// Spawns the player with its body's centre at `centre`, owned by gameplay,
/// at `difficulty`'s health, with the run's next save ID, its staff light
/// off.
pub fn spawn(world: &mut World, centre: Vec2, difficulty: Difficulty) -> Entity {
    let at = centre - BODY_OFFSET;
    let id = world.resource_mut::<SaveIds>().take();
    let player = world
        .prepare_prefab(PLAYER_PREFAB, at.to_array())
        .expect("the manifest's prefabs load with it")
        .with((
            SceneEntity { state_id: GAMEPLAY },
            id,
            Lantern::default(),
            Moves::default(),
        ))
        .spawn(world);
    // Before any step, so the first one already runs at this max.
    if let Some(stats) = world.get_mut::<Stats>(player) {
        stats.set_base(Stats::MAX_HEALTH, max_health(difficulty));
    }
    world.insert_resource(Bursts::default());
    player
}

/// Shows a cast by `player` along `aim`: its lantern burns hot, and it
/// tosses toward the aim or, standing, faces it with its arm and gun along
/// it, for [`CAST_SECS`].
pub fn cast(world: &mut World, player: Entity, aim: Vec2, toss: bool) {
    if let Some(moves) = world.get_mut::<Moves>(player) {
        moves.glow = CAST_SECS;
        moves.aim = aim;
        if toss {
            moves.toss = TOSS_SECS;
            moves.pose = 0.0;
        } else {
            moves.pose = CAST_SECS;
        }
    }
}

/// Where the player's body is: its centre.
#[must_use]
pub fn centre(world: &World, player: Entity) -> Option<Vec2> {
    crate::shared::body_centre(world, player)
}
