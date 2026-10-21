//! The player's systems and queries: the lantern and zoom actions, the
//! kill row, the bursts and one-shot clips its moves start, and the
//! lantern's glow.

use glam::Vec2;
use tungsten::core::{
    AnimationRegistry, AnimationState, AssetRegistry, Collider, DataRegistry, Entity, Position,
    PrevPosition, Shape, Sprite, Time, Velocity, With, World,
};
use tungsten::kit::{DamageRequest, Facing, Health, PlatformerController};

use super::components::{Lantern, LanternAnchors, Moves, Player, Shown};
use super::{BODY_OFFSET, LAND_SECS, PLAYER_HALF, ZOOM_MAX, ZOOM_MIN, ZOOM_STEP};
use crate::features::level::components::{Glow, Zoom};
use crate::features::level::{KILL_Y, WORLD_MAX, WORLD_MIN};
use crate::shared::{just_pressed, spawn_burst};

/// The one-shot clips, each with a left-facing twin named `<clip>_left`.
pub const LAND_CLIP: &str = "ex10_player_land";
pub const AIR_JUMP_CLIP: &str = "ex10_player_double_jump";
pub const TOSS_CLIP: &str = "ex10_player_toss";
/// The idle clip, which a cast's pose replaces with an aim frame.
pub const IDLE_CLIP: &str = "ex10_player_idle";
/// The aim frames, facing right from straight up (0) to straight down (4)
/// by eighths of a turn, each with a left-facing twin.
pub const AIM_FRAME: &str = "ex10_player_aim";
/// The bursts a jump, an air jump and a landing play.
pub const JUMP_PUFF: &str = "ex10_jump_puff";
pub const AIR_JUMP_BURST: &str = "ex10_double_jump";
pub const LANDING_DUST: &str = "ex10_landing_dust";
/// The lantern's anchors, by their data entry.
pub const ANCHORS: &str = "player";

/// The centre of `player`'s body: its `Position` plus its collider's offset.
#[must_use]
pub fn centre(world: &World, player: Entity) -> Option<Vec2> {
    let at = world.get::<Position>(player)?.0;
    let offset = world
        .get::<Collider>(player)
        .map_or(Vec2::ZERO, |c| c.offset);
    Some(at + offset)
}

/// The player.
fn player(world: &World) -> Option<Entity> {
    world.query_filtered::<Entity, With<Player>>().next()
}

/// `player_actions`, in the intent phase: the `lantern` action turns the
/// lantern on or off, and `zoom_in` and `zoom_out` step the zoom within its
/// bounds.
pub fn actions(world: &mut World) {
    if just_pressed(world, "lantern") {
        for lantern in world.query_mut::<&mut Lantern>() {
            lantern.on = !lantern.on;
        }
    }
    let step = match (
        just_pressed(world, "zoom_in"),
        just_pressed(world, "zoom_out"),
    ) {
        (true, false) => ZOOM_STEP,
        (false, true) => -ZOOM_STEP,
        _ => return,
    };
    if let Some(zoom) = world.get_resource_mut::<Zoom>() {
        zoom.0 = (zoom.0 + step).clamp(ZOOM_MIN, ZOOM_MAX);
    }
}

/// Whether a body at `at` with `collider` lies wholly outside the box the
/// level's bodies live in.
#[must_use]
pub fn escaped(at: Vec2, collider: Option<&Collider>) -> bool {
    let (min, max) = collider.map_or((at, at), |collider| {
        let centre = at + collider.offset;
        let half = match collider.shape {
            Shape::Aabb { half_extents } => half_extents,
            Shape::Circle { radius } => Vec2::splat(radius),
        };
        (centre - half, centre + half)
    });
    max.x < WORLD_MIN.x || min.x > WORLD_MAX.x || max.y < WORLD_MIN.y || min.y > WORLD_MAX.y
}

/// `player_bounds`, in the overlap phase: a player whose body falls past
/// the kill row, or leaves the level's box, takes a hit of all its health.
/// The hit is periodic, so it lands inside the invulnerability window.
pub fn bounds(world: &mut World) {
    let Some(player) = player(world) else {
        return;
    };
    let (Some(at), Some(health)) = (
        world.get::<Position>(player).map(|p| p.0),
        world.get::<Health>(player),
    ) else {
        return;
    };
    if health.is_dead() {
        return;
    }
    let amount = health.current();
    let collider = world.get::<Collider>(player);
    let below = centre(world, player).is_some_and(|centre| centre.y > KILL_Y);
    if below || escaped(at, collider) {
        DamageRequest::new(player, amount)
            .with_periodic(true)
            .queue(world);
    }
}

/// `player_moves`, in the react phase: a jump puffs at the feet it left
/// from and an air jump bursts there and tucks; a landing after the first
/// raises dust and holds the land clip. A dead player's body stops where
/// it fell, its controller gone, and its clips and bursts end.
pub fn moves(world: &mut World) {
    let Some(player) = player(world) else {
        return;
    };
    let dead = world.get::<Health>(player).is_some_and(Health::is_dead);
    if dead {
        if world.has::<PlatformerController>(player) {
            world.remove_component::<PlatformerController>(player);
            if let Some(velocity) = world.get_mut::<Velocity>(player) {
                velocity.0 = Vec2::ZERO;
            }
        }
        if let Some(moves) = world.get_mut::<Moves>(player) {
            moves.landing = 0.0;
            moves.tuck = false;
        }
        return;
    }
    let Some(controller) = world.get::<PlatformerController>(player) else {
        return;
    };
    let (jumped, air, grounded) = (
        controller.jumped(),
        controller.air_jumped(),
        controller.grounded(),
    );
    let feet = BODY_OFFSET + Vec2::new(0.0, PLAYER_HALF.y);
    let at = world.get::<Position>(player).map_or(Vec2::ZERO, |p| p.0);
    let from = world.get::<PrevPosition>(player).map_or(at, |p| p.0);
    let rising = world.get::<Velocity>(player).is_some_and(|v| v.0.y < 0.0);
    let Some(moves) = world.get_mut::<Moves>(player) else {
        return;
    };
    let landed = grounded && !moves.grounded && moves.settled;
    if jumped {
        moves.landing = 0.0;
        moves.tuck = air;
    }
    if landed {
        moves.landing = LAND_SECS;
    }
    if grounded {
        moves.settled = true;
    }
    if grounded || !rising {
        moves.tuck = false;
    }
    moves.grounded = grounded;
    if jumped {
        let burst = if air { AIR_JUMP_BURST } else { JUMP_PUFF };
        spawn_burst(world, burst, from + feet);
    }
    if landed {
        spawn_burst(world, LANDING_DUST, at + feet);
    }
}

/// `player_clips`, in the present phase after the character animator: a
/// cast's toss facing its aim, the land clip while it holds on the ground,
/// and the air jump's tuck while it rises, facing as the player does,
/// shown over the animator's clip and then handed back to it; then a
/// cast's dress over the frame shown.
pub fn clips(world: &mut World) {
    let Some(player) = player(world) else {
        return;
    };
    let dt = world.resource::<Time>().delta();
    let grounded = world
        .get::<PlatformerController>(player)
        .is_some_and(PlatformerController::grounded);
    let rising = world.get::<Velocity>(player).is_some_and(|v| v.0.y < 0.0);
    let left = world
        .get::<Facing>(player)
        .is_some_and(|facing| facing.direction().x < 0.0);
    let Some(moves) = world.get_mut::<Moves>(player) else {
        return;
    };
    for timer in [
        &mut moves.landing,
        &mut moves.toss,
        &mut moves.pose,
        &mut moves.glow,
    ] {
        *timer = (*timer - dt).max(0.0);
    }
    let (pose, glow, aim) = (moves.pose > 0.0, moves.glow > 0.0, moves.aim);
    let clip = if moves.toss > 0.0 {
        Some((TOSS_CLIP, aim.x < 0.0))
    } else if !grounded && rising && moves.tuck {
        Some((AIR_JUMP_CLIP, left))
    } else if grounded && moves.landing > 0.0 {
        Some((LAND_CLIP, left))
    } else {
        None
    };
    let wanted = clip.map(|(clip, left)| {
        if left {
            format!("{clip}_left")
        } else {
            clip.to_string()
        }
    });
    let shown = moves.shown.take();
    let Some(registry) = world.remove_resource::<AnimationRegistry>() else {
        return;
    };
    let current = world
        .get::<AnimationState>(player)
        .map(|state| state.animation_id.clone())
        .unwrap_or_default();
    let next = match (wanted, shown) {
        (Some(wanted), Some(mut shown)) if shown.state.animation_id == wanted => {
            if current == wanted {
                // Ours, advanced by this frame's tick.
                if let Some(state) = world.get::<AnimationState>(player) {
                    shown.state = state.clone();
                }
            } else {
                // The animator changed its clip under ours: ours stays, with
                // the frame its tick missed.
                shown.base = current;
                shown.state.advance(dt * 1000.0, &registry);
                show(world, player, shown.state.clone(), &registry);
            }
            Some(shown)
        }
        (Some(wanted), shown) => {
            let base = match shown {
                Some(shown) if shown.state.animation_id == current => shown.base,
                _ => current,
            };
            let state = AnimationState::new(wanted);
            show(world, player, state.clone(), &registry);
            Some(Shown { state, base })
        }
        (None, Some(shown)) => {
            if shown.state.animation_id == current {
                show(world, player, AnimationState::new(shown.base), &registry);
            }
            None
        }
        (None, None) => None,
    };
    let posed = pose && grounded && next.is_none();
    if let Some(moves) = world.get_mut::<Moves>(player) {
        moves.shown = next;
    }
    dress(world, player, &registry, posed, glow, aim);
    world.insert_resource(registry);
}

/// Dresses the frame `player` shows for a cast: with `pose`, standing in
/// its idle clip, the aim frame nearest `aim`, facing it; with `glow`, the
/// frame's cast twin, its lantern hot. A dress no longer wanted gives the
/// clip's frame back.
fn dress(
    world: &mut World,
    player: Entity,
    registry: &AnimationRegistry,
    pose: bool,
    glow: bool,
    aim: Vec2,
) {
    let (Some(state), Some(assets)) = (
        world.get::<AnimationState>(player),
        world.get_resource::<AssetRegistry>(),
    ) else {
        return;
    };
    let shown = world.get::<Sprite>(player).map(|sprite| sprite.asset_id);
    let dressed = shown
        .and_then(|id| assets.sprite_name(id))
        .is_some_and(|name| name.starts_with(AIM_FRAME) || name.contains("_cast"));
    let Some(frame) = state.current_sprite(registry) else {
        return;
    };
    if !pose && !glow && !dressed {
        return;
    }
    let mut name = if pose && state.animation_id.starts_with(IDLE_CLIP) {
        aim_frame(aim)
    } else {
        match assets.sprite_name(frame) {
            Some(name) => name.to_string(),
            None => return,
        }
    };
    if glow {
        let twin = cast_twin(&name);
        if assets.sprite_id(&twin).is_some() {
            name = twin;
        }
    }
    let id = assets.sprite_id(&name).unwrap_or(frame);
    if let Some(sprite) = world.get_mut::<Sprite>(player) {
        sprite.asset_id = id;
    }
}

/// The aim frame nearest `aim`, facing it.
#[must_use]
pub fn aim_frame(aim: Vec2) -> String {
    let up = (-aim.y).atan2(aim.x.abs()).to_degrees();
    let index = ((90.0 - up) / 45.0).round().clamp(0.0, 4.0) as usize;
    if aim.x < 0.0 {
        format!("{AIM_FRAME}_{index}_left")
    } else {
        format!("{AIM_FRAME}_{index}")
    }
}

/// The cast twin of the courier frame `name`, its lantern hot: `_cast`
/// before a left-facing frame's `_left`.
#[must_use]
pub fn cast_twin(name: &str) -> String {
    match name.strip_suffix("_left") {
        Some(base) => format!("{base}_cast_left"),
        None => format!("{name}_cast"),
    }
}

/// Shows `state` on `player`, its current frame in the sprite at once.
fn show(world: &mut World, player: Entity, state: AnimationState, registry: &AnimationRegistry) {
    let frame = state.current_sprite(registry);
    world.insert(player, state);
    if let (Some(frame), Some(sprite)) = (frame, world.get_mut::<Sprite>(player)) {
        sprite.asset_id = frame;
    }
}

/// `player_lantern`, in the present phase before the level's glows: the
/// lantern's glow sits on the flame of the frame shown, and goes dark while
/// the lantern is off or the player is dead.
pub fn lantern(world: &mut World) {
    let Some(player) = player(world) else {
        return;
    };
    let on = world
        .get::<Lantern>(player)
        .is_some_and(|lantern| lantern.on);
    let dead = world.get::<Health>(player).is_some_and(Health::is_dead);
    let anchor = world.get::<Sprite>(player).and_then(|sprite| {
        let name = world
            .get_resource::<AssetRegistry>()?
            .sprite_name(sprite.asset_id)?;
        let anchors = world
            .get_resource::<DataRegistry>()?
            .get::<LanternAnchors>(ANCHORS)?;
        anchors.0.get(name).copied()
    });
    if let Some(glow) = world.get_mut::<Glow>(player) {
        glow.hidden = !on || dead;
        if let Some(anchor) = anchor {
            glow.offset = anchor;
        }
    }
}
