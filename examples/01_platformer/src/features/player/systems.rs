//! The player's systems and queries: the staff light and zoom actions,
//! the jump's shove, the kill row, the hurt a blow plays, the bursts and
//! one-shot clips its moves start, a cast's gesture, and the staff light's
//! glow with its flare.

use glam::Vec2;
use tungsten::core::{
    AnimationRegistry, AnimationState, AssetRegistry, BodyKind, Collider, DataRegistry, Entity,
    EventCursor, EventQueue, Position, PrevPosition, RigidBody, Shape, Sprite, Time, Velocity,
    With, World,
};
use tungsten::kit::{
    DamageRequest, Damaged, Facing, Health, Intent, PlatformerController, Shield, Stats, Used,
    use_ability,
};
use tungsten::physics::{SpatialFilter, overlap, wake};

use super::components::{Lantern, LanternAnchors, Moves, Player, Shown, StaffGlow};
use super::{
    FLARE_CORE, FLARE_LIGHT, FLARE_RADIUS, GUTTER_PUFF, Gesture, JUMP_SPEED, LAND_SECS,
    RELIGHT_MANA, SHOVE_BESIDE, SHOVE_HEAD, SHOVE_OVER, SHOVE_SIDE, STAFF_LIGHT, STUCK_BY,
    ZOOM_MAX, ZOOM_MIN, ZOOM_STEP,
};
use crate::features::audio::{self, components::Sound};
use crate::features::level::components::{Glow, Zoom};
use crate::features::level::{KILL_Y, WORLD_MAX, WORLD_MIN};
use crate::features::tomes;
use crate::shared::{body_centre, just_pressed, spawn_burst};

/// The one-shot clips, each with a left-facing twin named `<clip>_left`:
/// the land, the air jump's tuck, a blow's hurt, the death and the
/// gestures'.
pub const LAND_CLIP: &str = "ex10_player_land";
pub const AIR_JUMP_CLIP: &str = "ex10_player_double_jump";
pub const HURT_CLIP: &str = "ex10_player_hurt";
pub const DEATH_CLIP: &str = "ex10_player_death";
pub const TOSS_CLIP: &str = "ex10_player_toss";
pub const BOLT_CLIP: &str = "ex10_player_bolt";
pub const PUSH_CLIP: &str = "ex10_player_push";
pub const CONJURE_CLIP: &str = "ex10_player_conjure";
/// The idle clip, which a cast's pose replaces with a pose frame.
pub const IDLE_CLIP: &str = "ex10_player_idle";
/// The pose frames, facing right from straight up (0) to straight down (4)
/// by eighths of a turn, each with a left-facing twin: the staff levelled
/// along the aim, the sprays' and the small balls', and the pistol, the
/// gun's.
pub const STAFF_FRAME: &str = "ex10_player_staff";
pub const AIM_FRAME: &str = "ex10_player_aim";
/// The bursts a jump, an air jump and a landing play.
pub const JUMP_PUFF: &str = "ex10_jump_puff";
pub const AIR_JUMP_BURST: &str = "ex10_double_jump";
pub const LANDING_DUST: &str = "ex10_landing_dust";
/// The lantern's anchors, by their data entry.
pub const ANCHORS: &str = "player";
/// Game seconds the wizard shows red after a blow, fading, and holds the
/// hurt clip.
pub const HURT_SECS: f32 = 0.12;
pub const HURT_CLIP_SECS: f32 = 0.2;
/// The hurt edge a tick raises at least, and the seconds the edge falls
/// from full to nothing over.
pub const TICK_EDGE: f32 = 0.5;
pub const EDGE_SECS: f32 = 0.4;

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
    // Onto the step's grid before the clamp, so a press back from the
    // floor returns to 1 (I18).
    if let Some(zoom) = world.get_resource_mut::<Zoom>() {
        let snapped = ((zoom.0 + step) / ZOOM_STEP).round() * ZOOM_STEP;
        zoom.0 = snapped.clamp(ZOOM_MIN, ZOOM_MAX);
    }
}

/// A jump's press that shoved what buried the courier: where its body's
/// centre stood, kept for the next step's retry. A one-step resource, never
/// saved: a save between the two steps loses that jump.
#[derive(Debug, Clone, Copy, Default)]
pub struct Shove(pub Option<Vec2>);

/// `player_unstick`, in the intent phase after the action input and before
/// the controller: on a jump press the controller can take, every dynamic
/// body overlapping the box over the courier's head and shoulders
/// ([`SHOVE_HEAD`] tall, its width and [`SHOVE_SIDE`] each side) takes
/// [`SHOVE_OVER`] over its mass, so a pile's column goes up whole, and
/// every one beside it ([`SHOVE_SIDE`] each side, its height)
/// [`SHOVE_BESIDE`], out from the courier's centre and up, each woken. A jump they blocked ends its step
/// ungrounded with coyote time spent, so a press would spend the air jump;
/// when the last step's press kicked anything and its jump left the
/// courier within [`STUCK_BY`] of where it pressed, the controller is
/// launched at [`JUMP_SPEED`] instead, after its ground probe has given the
/// air jump back. One retry a press; a jump that rose is left alone.
pub fn unstick(world: &mut World) {
    let Some(player) = player(world) else {
        return;
    };
    if world.get::<Health>(player).is_some_and(Health::is_dead) {
        world.insert_resource(Shove(None));
        return;
    }
    let (Some(centre), Some(collider)) = (
        body_centre(world, player),
        world.get::<Collider>(player).copied(),
    ) else {
        return;
    };
    let last = world.get_resource::<Shove>().and_then(|shove| shove.0);
    world.insert_resource(Shove(None));
    if let Some(from) = last {
        let jumped = world
            .get::<PlatformerController>(player)
            .is_some_and(PlatformerController::jumped);
        if jumped && centre.distance(from) <= STUCK_BY {
            let run = world.get::<Velocity>(player).map_or(0.0, |v| v.0.x);
            if let Some(controller) = world.get_mut::<PlatformerController>(player) {
                controller.launch(Vec2::new(run, -JUMP_SPEED));
            }
        }
        return;
    }
    let pressed = world
        .get::<Intent>(player)
        .is_some_and(|intent| intent.pressed("jump"));
    let able = world
        .get::<PlatformerController>(player)
        .is_some_and(PlatformerController::can_jump);
    if !(pressed && able) {
        return;
    }
    let half = match collider.shape {
        Shape::Aabb { half_extents } => half_extents,
        Shape::Circle { radius } => Vec2::splat(radius),
    };
    let head = Vec2::new(centre.x, centre.y - half.y - SHOVE_HEAD * 0.5);
    let boxes = [
        (
            head,
            Vec2::new(half.x + SHOVE_SIDE, SHOVE_HEAD * 0.5),
            SHOVE_OVER,
        ),
        (
            centre - Vec2::new(half.x + SHOVE_SIDE * 0.5, 0.0),
            Vec2::new(SHOVE_SIDE * 0.5, half.y),
            SHOVE_BESIDE,
        ),
        (
            centre + Vec2::new(half.x + SHOVE_SIDE * 0.5, 0.0),
            Vec2::new(SHOVE_SIDE * 0.5, half.y),
            SHOVE_BESIDE,
        ),
    ];
    let mut kicks: Vec<(Entity, Vec2)> = Vec::new();
    let mut met = Vec::new();
    for (at, half_extents, speed) in boxes {
        overlap(
            world,
            Shape::Aabb { half_extents },
            at,
            &SpatialFilter::new(),
            &mut met,
        );
        for entity in met.iter().flatten().copied() {
            let Some(body) = world
                .get::<RigidBody>(entity)
                .filter(|body| body.kind == BodyKind::Dynamic)
            else {
                continue;
            };
            if kicks.iter().any(|(kicked, _)| *kicked == entity) {
                continue;
            }
            let out = body_centre(world, entity).map_or(Vec2::ZERO, |at| at - centre);
            let direction = (out.normalize_or_zero() - Vec2::Y).normalize_or(-Vec2::Y);
            kicks.push((entity, direction * speed * body.inv_mass));
        }
    }
    if kicks.is_empty() {
        return;
    }
    for (entity, kick) in kicks {
        if let Some(velocity) = world.get_mut::<Velocity>(entity) {
            velocity.0 += kick;
        }
        wake(world, entity);
    }
    world.insert_resource(Shove(Some(centre)));
}

/// Whether a body at `at` with `collider` lies wholly outside the box the
/// level's bodies live in.
#[must_use]
pub fn escaped(at: Vec2, collider: Option<&Collider>) -> bool {
    let (min, max) = extent(at, collider);
    max.y < WORLD_MIN.y || past_the_sides_or_foot(min, max)
}

/// The box of a body at `at` with `collider`, min and max; its point with
/// none.
fn extent(at: Vec2, collider: Option<&Collider>) -> (Vec2, Vec2) {
    collider.map_or((at, at), |collider| {
        let centre = at + collider.offset;
        let half = match collider.shape {
            Shape::Aabb { half_extents } => half_extents,
            Shape::Circle { radius } => Vec2::splat(radius),
        };
        (centre - half, centre + half)
    })
}

/// Whether a box, min to max, lies wholly past the level box's sides or its
/// foot: what the player's check reads, since a bound may carry the wizard
/// over its top, and gravity always brings him back.
fn past_the_sides_or_foot(min: Vec2, max: Vec2) -> bool {
    max.x < WORLD_MIN.x || min.x > WORLD_MAX.x || min.y > WORLD_MAX.y
}

/// `player_bounds`, in the overlap phase: a player whose body falls past
/// the kill row, or leaves the level's box by a side or its foot, takes a
/// hit of all its health and the pact's shield over it; over the box's top,
/// where a bound may carry him, he falls back. The hit is periodic, so it
/// lands inside the invulnerability window.
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
    // Through the pact's shield too, which takes a hit first.
    let amount = health.current() + world.get::<Shield>(player).map_or(0.0, Shield::current);
    let collider = world.get::<Collider>(player);
    let below = body_centre(world, player).is_some_and(|centre| centre.y > KILL_Y);
    let (min, max) = extent(at, collider);
    if below || past_the_sides_or_foot(min, max) {
        DamageRequest::new(player, amount)
            .with_periodic(true)
            .queue(world);
    }
}

/// `player_staff_light`, in the intent phase before the spells: a lit
/// staff light burns its ability's mana each step, its effect granting
/// `staff.lit`; at full mana the regeneration's surplus feeds it, so the
/// step's cost goes back. Refused, it gutters out with a puff at its flame
/// and a sound, and relights once the mana reaches [`RELIGHT_MANA`] (L5's
/// first half, Q15).
pub fn staff_light(world: &mut World) {
    let Some(player) = player(world) else {
        return;
    };
    let alive = world
        .get::<Health>(player)
        .is_some_and(|health| !health.is_dead());
    let Some(lantern) = world.get::<Lantern>(player).copied() else {
        return;
    };
    if !alive || !lantern.on {
        return;
    }
    let (mana, max) = world.get::<Stats>(player).map_or((0.0, 0.0), |stats| {
        (
            stats.base("mana").unwrap_or(0.0),
            stats.get("max_mana").unwrap_or(0.0),
        )
    });
    if lantern.guttered {
        if mana < RELIGHT_MANA {
            return;
        }
        if let Some(lantern) = world.get_mut::<Lantern>(player) {
            lantern.guttered = false;
        }
    }
    match use_ability(world, player, STAFF_LIGHT) {
        Used::Used if mana >= max => {
            if let Some(stats) = world.get_mut::<Stats>(player) {
                stats.set_base("mana", mana);
            }
        }
        Used::NoCost => {
            if let Some(lantern) = world.get_mut::<Lantern>(player) {
                lantern.guttered = true;
            }
            let flame = world
                .get::<Glow>(player)
                .zip(world.get::<Position>(player))
                .map(|(glow, at)| glow.centre(at.0));
            if let Some(at) = flame {
                spawn_burst(world, GUTTER_PUFF, at);
                audio::play(world, Sound::StaffGuttered, at);
            }
        }
        _ => {}
    }
}

/// `player_hurt`, in the react phase: a blow on the player plays its hurt
/// sound, shake, tint and clip and fills the hurt edge, and the hit that
/// kills it, a blow or a tick, its death sound and a harder shake in their
/// place; a tick of burning or bleeding that does not kill raises the edge
/// to [`TICK_EDGE`] at least and plays its soft tick, once a step however
/// many land (K1 stays). `cursor` reads each hit once.
pub fn hurt(world: &mut World, cursor: &mut EventCursor<Damaged>) {
    let Some(player) = player(world) else {
        return;
    };
    let Some(events) = world.get_resource::<EventQueue<Damaged>>() else {
        return;
    };
    let (mut hurt, mut ticked, mut killed) = (false, false, false);
    for hit in cursor.read(events).filter(|hit| hit.entity == player) {
        hurt |= !hit.periodic;
        ticked |= hit.periodic && !hit.killed;
        killed |= hit.killed;
    }
    if let Some(moves) = world.get_mut::<Moves>(player) {
        if hurt {
            moves.hurt = HURT_SECS;
            moves.hurt_clip = HURT_CLIP_SECS;
            moves.edge = 1.0;
        } else if ticked && !killed {
            moves.edge = moves.edge.max(TICK_EDGE);
        }
    }
    let sound = if killed {
        Sound::PlayerDied
    } else if hurt {
        Sound::PlayerHurt
    } else {
        Sound::PlayerTicked
    };
    if (hurt || ticked || killed)
        && let Some(at) = body_centre(world, player)
    {
        audio::play(world, sound, at);
    }
}

/// `player_moves`, in the react phase: a jump puffs at the feet it left
/// from and an air jump bursts there with its whoosh and tucks, a bound
/// with its ring and its sound, and a bound refused for mana sounds its
/// refusal; a landing after the first raises dust and holds the land clip.
/// A dead player's body drops where it died and falls as its controller
/// falls, its input locked for good (F13), and its clips and bursts end.
pub fn moves(world: &mut World) {
    let Some(player) = player(world) else {
        return;
    };
    let dead = world.get::<Health>(player).is_some_and(Health::is_dead);
    if dead {
        let left = world
            .get::<Facing>(player)
            .is_some_and(|facing| facing.direction().x < 0.0);
        let first = world.get_mut::<Moves>(player).is_some_and(|moves| {
            moves.landing = 0.0;
            moves.tuck = false;
            let first = !std::mem::replace(&mut moves.dead, true);
            if first {
                // The death clip takes over from any gesture, flare or
                // hurt, facing as the wizard faced.
                (moves.cast, moves.glow, moves.hurt_clip) = (0.0, 0.0, 0.0);
                moves.dead_left = left;
            }
            first
        });
        if first {
            // Straight down from where it died: no run or knockback slides
            // it, and its rise or fall carries on.
            let fall = world
                .get::<Velocity>(player)
                .map_or(0.0, |velocity| velocity.0.y);
            if let Some(controller) = world.get_mut::<PlatformerController>(player) {
                controller.set_run(0.0);
                controller.launch(Vec2::new(0.0, fall));
                controller.lock_input(f32::MAX);
            }
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
    let (bound, refused) = (controller.paid_air_jumped(), controller.air_jump_refused());
    let body = super::body(world);
    let feet = body.centre + Vec2::new(0.0, body.half.y);
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
    if jumped && bound {
        tomes::systems::bound(world, from + feet);
    } else if jumped {
        let burst = if air { AIR_JUMP_BURST } else { JUMP_PUFF };
        spawn_burst(world, burst, from + feet);
        if air {
            audio::play(world, Sound::AirJumped, from + feet);
        }
    }
    // A bound the mana could not pay for, once a press.
    if refused {
        audio::play(world, Sound::NoMana, at + feet);
    }
    if landed {
        spawn_burst(world, LANDING_DUST, at + feet);
    }
}

/// `player_clips`, in the present phase after the character animator: the
/// death clip once the wizard dies, held on its last frame, facing as he
/// died; else the hurt clip after a blow, facing as he does; a cast's
/// gesture facing its aim; the air jump's tuck while he rises and the land
/// clip while it holds on the ground, facing as he does: shown over the
/// animator's clip and then handed back to it. Then, standing, a cast's
/// pose over the idle clip's frame.
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
        &mut moves.cast,
        &mut moves.glow,
        &mut moves.hurt,
        &mut moves.hurt_clip,
    ] {
        *timer = (*timer - dt).max(0.0);
    }
    moves.edge = (moves.edge - dt / EDGE_SECS).max(0.0);
    let aim = moves.aim;
    let gesture = moves.gesture.filter(|_| moves.cast > 0.0);
    let clip = if moves.dead {
        Some((DEATH_CLIP, moves.dead_left))
    } else if moves.hurt_clip > 0.0 {
        Some((HURT_CLIP, left))
    } else if let Some(clip) = gesture.and_then(one_shot) {
        Some((clip, aim.x < 0.0))
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
    let pose = gesture
        .and_then(pose_frames)
        .filter(|_| grounded && next.is_none());
    if let Some(moves) = world.get_mut::<Moves>(player) {
        moves.shown = next;
    }
    dress(world, player, &registry, pose, aim);
    world.insert_resource(registry);
}

/// A gesture's one-shot clip, none for a pose.
fn one_shot(gesture: Gesture) -> Option<&'static str> {
    match gesture {
        Gesture::Toss => Some(TOSS_CLIP),
        Gesture::Bolt => Some(BOLT_CLIP),
        Gesture::Push => Some(PUSH_CLIP),
        Gesture::Conjure => Some(CONJURE_CLIP),
        Gesture::Pour | Gesture::Shoot => None,
    }
}

/// A pose's frames, none for a one-shot gesture.
fn pose_frames(gesture: Gesture) -> Option<&'static str> {
    match gesture {
        Gesture::Pour => Some(STAFF_FRAME),
        Gesture::Shoot => Some(AIM_FRAME),
        _ => None,
    }
}

/// Dresses the frame `player` shows for a cast's pose: standing in its idle
/// clip, the frame of `pose`'s set nearest `aim`, facing it. A pose no
/// longer wanted gives the clip's frame back.
fn dress(
    world: &mut World,
    player: Entity,
    registry: &AnimationRegistry,
    pose: Option<&str>,
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
        .is_some_and(|name| name.starts_with(STAFF_FRAME) || name.starts_with(AIM_FRAME));
    let Some(frame) = state.current_sprite(registry) else {
        return;
    };
    let name = match pose {
        Some(frames) if state.animation_id.starts_with(IDLE_CLIP) => pose_frame(frames, aim),
        _ if dressed => match assets.sprite_name(frame) {
            Some(name) => name.to_string(),
            None => return,
        },
        _ => return,
    };
    let id = assets.sprite_id(&name).unwrap_or(frame);
    if let Some(sprite) = world.get_mut::<Sprite>(player) {
        sprite.asset_id = id;
    }
}

/// The frame of the pose set `frames` nearest `aim`, facing it.
#[must_use]
pub fn pose_frame(frames: &str, aim: Vec2) -> String {
    let up = (-aim.y).atan2(aim.x.abs()).to_degrees();
    let index = ((90.0 - up) / 45.0).round().clamp(0.0, 4.0) as usize;
    if aim.x < 0.0 {
        format!("{frames}_{index}_left")
    } else {
        format!("{frames}_{index}")
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
/// staff light's glow sits on the orb of the frame shown, and goes dark
/// while the light is off or guttered out, or the player is dead; while a
/// spell's flare runs its halo, core and light swell from the prefab's
/// glow ([`FLARE_RADIUS`], [`FLARE_CORE`], [`FLARE_LIGHT`]), its reach the
/// prefab's throughout, and after it they are the prefab's again.
pub fn lantern(world: &mut World) {
    let Some(player) = player(world) else {
        return;
    };
    let on = world.get::<Lantern>(player).is_some_and(Lantern::lit);
    let dead = world.get::<Health>(player).is_some_and(Health::is_dead);
    let anchor = anchor(world, player);
    let flaring = world
        .get::<Moves>(player)
        .is_some_and(|moves| moves.glow > 0.0);
    let base = world.get::<StaffGlow>(player).map(|staff| staff.0);
    if let Some(glow) = world.get_mut::<Glow>(player) {
        glow.hidden = !on || dead;
        if let Some(anchor) = anchor {
            glow.offset = anchor;
        }
        if let Some(base) = base {
            let (radius, core, light) = if flaring {
                (FLARE_RADIUS, FLARE_CORE, FLARE_LIGHT)
            } else {
                (1.0, 1.0, 1.0)
            };
            glow.radius = base.radius * radius;
            glow.core = base.core * core;
            glow.light = base.light * light;
            glow.reach = Some(base.reach());
        }
    }
}

/// Where the staff's orb sits in `player`'s frame now, from its sprite's
/// top-left: its sprite's lantern anchor.
#[must_use]
pub fn anchor(world: &World, player: Entity) -> Option<[f32; 2]> {
    let sprite = world.get::<Sprite>(player)?;
    let name = world
        .get_resource::<AssetRegistry>()?
        .sprite_name(sprite.asset_id)?;
    let anchors = world
        .get_resource::<DataRegistry>()?
        .get::<LanternAnchors>(ANCHORS)?;
    anchors.0.get(name).copied()
}
