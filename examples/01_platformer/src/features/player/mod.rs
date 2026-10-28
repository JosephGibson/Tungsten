//! The player: a wizard on the kit's platformer controller, from the
//! `player` prefab, which the camera follows. The prefab holds its kit
//! items: input, the controller with one air jump, the aim, health,
//! hurtbox, knockback, hit flash, the hit's cues, facing, the character
//! animator and the camera target. 01 adds the staff light, which the code
//! names the lantern and which burns mana, the zoom, the kill row, the
//! bursts its moves raise, the land, air-jump and hurt one-shots, a cast's
//! gesture and the staff's flare, and the jump's shove that throws off
//! what buries it.

pub mod components;
pub mod systems;

use glam::Vec2;
use tungsten::core::{Entity, EventCursor, Schedule, Stage, World, system};
use tungsten::kit::{
    ACTION_INPUT, CHARACTER_ANIMATOR, CUES, Phase, PlatformerController, ensure_phases,
};
use tungsten::{Plugin, SceneEntity};

use crate::features::level::GLOWS;
use crate::features::level::components::Glow;
use crate::game::GAMEPLAY;
use crate::shared::{Bursts, SaveId};
use components::{Lantern, LanternAnchors, Moves, StaffGlow};

/// The player's prefab, by its manifest ID.
pub const PLAYER_PREFAB: &str = "player";
/// The name the player's save ID derives from ([`SaveId::authored`]),
/// which no map object takes.
pub const SAVE_NAME: &str = "player";
/// The body's half size and where its centre sits from the sprite's
/// top-left corner, the entity's point: the wizard's 96 px art stands its
/// feet on its row 93.
pub const PLAYER_HALF: Vec2 = Vec2::new(20.0, 28.0);
pub const BODY_OFFSET: Vec2 = Vec2::new(48.0, 65.0);
/// The side of the square the player's status overlays draw in, centred on
/// his body: the courier's 64 px frame, which they were drawn for.
pub const OVERLAY_SIDE: f32 = 64.0;
/// Game seconds a landing holds the land clip.
pub const LAND_SECS: f32 = 0.18;
/// Game seconds a cast's gesture holds, its one-shot clip or its pose, and
/// the staff flares.
pub const CAST_SECS: f32 = 0.3;
/// The staff's flare over a cast: the staff light's halo radius, its hot
/// core and its light, each times the prefab's; its reach stays the
/// prefab's, which the crawlers' edge reads.
pub const FLARE_RADIUS: f32 = 1.35;
pub const FLARE_CORE: f32 = 1.6;
pub const FLARE_LIGHT: f32 = 1.5;
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
/// The mass the wizard walks bodies aside as, by the kit's shove (N6), and
/// the share of it a body he moves may weigh: up to 30, so a ball, a
/// bowling ball, a rock, a bomb, a crate, a keg and a scrap, each slowing
/// him by its mass over 60 (a crate 20 %, a keg 27 %); the dummy and iron
/// stop him.
pub const PLAYER_SHOVE_MASS: f32 = 60.0;
pub const PLAYER_SHOVE_SHARE: f32 = 0.5;

/// How far the player draws a pickup in, its prefab's `pickup_radius`
/// stat: restated so a save can move an older player's reach to it.
pub const PICKUP_RADIUS: f32 = 192.0;

/// The player's full health at every difficulty, its prefab's
/// `max_health` stat and `health.max`: a hazard's touch takes 34, so it
/// falls on the third. Restated so a save can move an older player's
/// maximum to it.
pub const MAX_HEALTH: f32 = 100.0;

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

/// What a cast shows the wizard doing, by its spell or the gun: a one-shot
/// clip facing its aim, or, standing, a pose along it, for [`CAST_SECS`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gesture {
    /// The bowling ball: the free hand throws it.
    Toss,
    /// The fire missile and lightning: the staff thrust at the aim.
    Bolt,
    /// The force push and the black hole: the free hand shoves out.
    Push,
    /// Iron and the summons: the staff raised high and struck down.
    Conjure,
    /// The held sprays and the small balls: the staff levelled along the
    /// aim, a pose.
    Pour,
    /// A shot: the pistol along the aim, a pose.
    Shoot,
}

/// Spawns the player with its body's centre at `centre`, owned by gameplay,
/// at its prefab's health, with the save ID of [`SAVE_NAME`], its staff
/// light off, and its prefab's glow kept for the staff's flare.
pub fn spawn(world: &mut World, centre: Vec2) -> Entity {
    let at = centre - BODY_OFFSET;
    let id = SaveId::authored(SAVE_NAME);
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
    if let Some(glow) = world.get::<Glow>(player).copied() {
        world.insert(player, StaffGlow(glow));
    }
    shove_bodies(world, player);
    world.insert_resource(Bursts::default());
    player
}

/// Gives `player`'s controller its shove ([`PLAYER_SHOVE_MASS`]): at a
/// spawn, and after a load, whose saved controller may be from before it.
pub fn shove_bodies(world: &mut World, player: Entity) {
    if let Some(controller) = world.get_mut::<PlatformerController>(player) {
        *controller = controller
            .clone()
            .with_shove(PLAYER_SHOVE_MASS, PLAYER_SHOVE_SHARE);
    }
}

/// Shows a cast by `player` along `aim` with `gesture`, for [`CAST_SECS`]:
/// a spell's flares the staff; a shot, the gun's, leaves it be.
pub fn cast(world: &mut World, player: Entity, aim: Vec2, gesture: Gesture) {
    if let Some(moves) = world.get_mut::<Moves>(player) {
        if gesture != Gesture::Shoot {
            moves.glow = CAST_SECS;
        }
        moves.aim = aim;
        moves.gesture = Some(gesture);
        moves.cast = CAST_SECS;
    }
}

/// Where the player's body is: its centre.
#[must_use]
pub fn centre(world: &World, player: Entity) -> Option<Vec2> {
    crate::shared::body_centre(world, player)
}
