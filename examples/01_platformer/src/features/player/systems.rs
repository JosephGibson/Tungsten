//! The player's systems and queries: the staff light and zoom actions,
//! the jump's shove, the kill row, the hurt a blow plays, the one-shot
//! clips its moves start, a cast's gesture, and the staff light's glow
//! with its flare. Its jumps', bounds' and landings' bursts and sounds are
//! the controller's cue events, played by the prefab's `cues` and the
//! `air_jumped` rules.

use glam::Vec2;
use tungsten::core::{
    AnimationRegistry, AnimationState, AssetRegistry, BodyKind, Collider, DataRegistry, Entity,
    EventCursor, EventQueue, Position, PrevPosition, RigidBody, Shape, Sprite, Time, Velocity,
    With, World,
};
use tungsten::kit::{
    CharacterAnimator, Damaged, Facing, Health, Intent, PlatformerController, Stats, Used,
    use_ability,
};
use tungsten::physics::{SpatialFilter, overlap, wake};

use super::components::{Lantern, LanternAnchors, Moves, Player, Shown, StaffGlow};
use super::{
    FLARE_CORE, FLARE_LIGHT, FLARE_RADIUS, GUTTER_PUFF, GUTTER_SECS, JUMP_SPEED, KINDLE_FROM,
    KINDLE_PEAK, KINDLE_RISE, KINDLE_SECS, LAND_SECS, RELIGHT_MANA, SHOVE_BESIDE, SHOVE_HEAD,
    SHOVE_OVER, SHOVE_SIDE, STAFF_LIGHT, STUCK_BY, ZOOM_MAX, ZOOM_MIN, ZOOM_STEP,
};
use crate::features::audio::{self, components::Sound};
use crate::features::level::components::{Glow, GlowTail, Zoom};
use crate::features::tomes;
use crate::shared::{body_centre, just_pressed, spawn_burst};

/// 01's one-shot clips, each with a left-facing twin named `<clip>_left`:
/// the land, the air jump's tuck, a blow's hurt and the death. A pressed
/// spell's gesture is its ability's `clip`, which the kit's animator plays.
pub const LAND_CLIP: &str = "ex10_player_land";
pub const AIR_JUMP_CLIP: &str = "ex10_player_double_jump";
pub const HURT_CLIP: &str = "ex10_player_hurt";
pub const DEATH_CLIP: &str = "ex10_player_death";
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
    let half = collider.shape.half_extents();
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

/// `player_hurt`, in the react phase: a blow on the player plays its tint
/// and clip and fills the hurt edge; a tick of burning or bleeding that does
/// not kill raises the edge to [`TICK_EDGE`] at least (K1 stays). Its
/// sounds and shakes are the `player_hurt` group's cue rules: the death's,
/// a blow's or a tick's, once a step. `cursor` reads each hit once.
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
}

/// `player_moves`, in the react phase: an air jump tucks, a bound opens
/// its ring under the feet it left from, and a landing holds the land
/// clip. The puff, the bursts, the dust and the sounds are the cues' (the
/// prefab's `jumped`, `landed` and `jump_refused`, and the two
/// `air_jumped` rules, a free one's and a paid one's). A dead player's
/// body drops where it died and falls as its controller falls, its input
/// locked for good (F13), as its health's death policy (`disarm`) stopped
/// it at the kill, and its clips end.
pub fn moves(world: &mut World) {
    let Some(player) = player(world) else {
        return;
    };
    let dead = world.get::<Health>(player).is_some_and(Health::is_dead);
    if dead {
        let left = world
            .get::<Facing>(player)
            .is_some_and(|facing| facing.direction().x < 0.0);
        if let Some(moves) = world.get_mut::<Moves>(player) {
            moves.landing = 0.0;
            moves.tuck = false;
            if !std::mem::replace(&mut moves.dead, true) {
                // The death clip takes over from any gesture, flare or
                // hurt, facing as the wizard faced.
                (moves.cast, moves.glow, moves.hurt_clip) = (0.0, 0.0, 0.0);
                moves.dead_left = left;
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
    let (bound, landed) = (controller.paid_air_jumped(), controller.landed());
    let rising = world.get::<Velocity>(player).is_some_and(|v| v.0.y < 0.0);
    let Some(moves) = world.get_mut::<Moves>(player) else {
        return;
    };
    if jumped {
        moves.landing = 0.0;
        moves.tuck = air;
    }
    if landed {
        moves.landing = LAND_SECS;
    }
    if grounded || !rising {
        moves.tuck = false;
    }
    if jumped && bound {
        // The ring stays a one-shot of 01's: a cue's prefab, spawned from
        // the saved player, would be saved with the run.
        let body = super::body(world);
        let feet = body.centre + Vec2::new(0.0, body.half.y);
        let at = world.get::<Position>(player).map_or(Vec2::ZERO, |p| p.0);
        let from = world.get::<PrevPosition>(player).map_or(at, |p| p.0);
        tomes::systems::bound(world, from + feet);
    }
}

/// `player_clips`, in the present phase after the character animator: the
/// death clip once the wizard dies, held on its last frame, facing as he
/// died; else the hurt clip after a blow, facing as he does; else, while
/// the animator plays no one-shot (a pressed spell's gesture, its
/// ability's clip), the air jump's tuck while he rises and the land clip
/// while it holds on the ground, facing as he does: shown over the
/// animator's clip and then handed back to it. And the pose tags, held
/// exactly while a pour's or a shot's cast seconds run, whose rules show
/// the pose strips on the animator, standing ([`super::pose_tags`]): a tag
/// a load restored goes at its first frame.
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
    let gesturing = world
        .get::<CharacterAnimator>(player)
        .is_some_and(|animator| animator.playing().is_some());
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
    let gesture = moves.gesture.filter(|_| moves.cast > 0.0);
    let clip = if moves.dead {
        Some((DEATH_CLIP, moves.dead_left))
    } else if moves.hurt_clip > 0.0 {
        Some((HURT_CLIP, left))
    } else if gesturing {
        None
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
    super::pose_tags(world, player, gesture);
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
    if let Some(moves) = world.get_mut::<Moves>(player) {
        moves.shown = next;
    }
    world.insert_resource(registry);
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
/// staff light's glow sits on the orb of the frame shown. Lit, it kindles
/// as it lights ([`kindle`]); guttered out for want of mana it fades and
/// sputters out over [`GUTTER_SECS`] ([`gutter`]); turned off, or the player
/// dead, it goes dark at once. A spawn or a load kindles nothing. While a
/// spell's flare runs its halo, core and lights swell from the prefab's
/// glow ([`FLARE_RADIUS`], [`FLARE_CORE`], [`FLARE_LIGHT`]), its reach the
/// prefab's throughout, and after it they are the prefab's again.
/// Presentation, on the frame's game time: a pause holds it.
pub fn lantern(world: &mut World) {
    let Some(player) = player(world) else {
        return;
    };
    let dt = world.get_resource::<Time>().map_or(0.0, Time::delta);
    let lantern = world.get::<Lantern>(player).copied().unwrap_or_default();
    let lit = lantern.lit();
    let dead = world.get::<Health>(player).is_some_and(Health::is_dead);
    let anchor = anchor(world, player);
    let base = world.get::<StaffGlow>(player).map(|staff| staff.0);
    let Some(moves) = world.get_mut::<Moves>(player) else {
        return;
    };
    let flaring = moves.glow > 0.0;
    match (moves.was_lit.replace(lit), lit) {
        (None, _) => moves.kindled = KINDLE_SECS,
        (Some(false), true) => {
            moves.kindled = 0.0;
            moves.guttering = None;
        }
        (Some(true), false) if lantern.guttered => moves.guttering = Some(0.0),
        _ => {}
    }
    if !lantern.on || dead {
        moves.guttering = None;
    }
    moves.kindled += dt;
    moves.guttering = moves
        .guttering
        .map(|seconds| seconds + dt)
        .filter(|&seconds| seconds < GUTTER_SECS);
    let (shown, envelope) = match (lit, moves.guttering) {
        _ if dead => (false, 1.0),
        (true, _) => (true, kindle(moves.kindled)),
        (false, Some(seconds)) => (true, gutter(seconds)),
        (false, None) => (false, 1.0),
    };
    if let Some(glow) = world.get_mut::<Glow>(player) {
        glow.hidden = !shown;
        if let Some(anchor) = anchor {
            glow.offset = anchor;
        }
        if let Some(base) = base {
            let (radius, core, light) = if flaring {
                (FLARE_RADIUS, FLARE_CORE, FLARE_LIGHT)
            } else {
                (1.0, 1.0, 1.0)
            };
            glow.radius = base.radius * radius * envelope;
            glow.core = base.core * core * envelope;
            glow.light = base.light * light * envelope;
            glow.reach = Some(base.reach());
            glow.tail = base.tail.map(|tail| GlowTail {
                light: tail.light * light * envelope,
                ..tail
            });
        }
    }
}

/// The staff light's kindling flare `seconds` after it lit, a share of the
/// prefab's glow: from [`KINDLE_FROM`] up to [`KINDLE_PEAK`] over the first
/// [`KINDLE_RISE`] of [`KINDLE_SECS`], easing out, then back to 1, easing
/// in and out; 1 after.
#[must_use]
pub fn kindle(seconds: f32) -> f32 {
    let u = (seconds / KINDLE_SECS).clamp(0.0, 1.0);
    if u < KINDLE_RISE {
        let v = u / KINDLE_RISE;
        KINDLE_FROM + (KINDLE_PEAK - KINDLE_FROM) * (1.0 - (1.0 - v) * (1.0 - v))
    } else {
        let v = (u - KINDLE_RISE) / (1.0 - KINDLE_RISE);
        KINDLE_PEAK + (1.0 - KINDLE_PEAK) * v * v * (3.0 - 2.0 * v)
    }
}

/// The staff light's fade `seconds` after it guttered out, a share of the
/// prefab's glow: falling to nothing over [`GUTTER_SECS`] as the square of
/// the time left, sputtering deeper as it dies.
#[must_use]
pub fn gutter(seconds: f32) -> f32 {
    let u = (seconds / GUTTER_SECS).clamp(0.0, 1.0);
    let left = 1.0 - u;
    let sputter = 1.0 - 0.45 * u * (std::f32::consts::TAU * 7.5 * seconds).sin().abs();
    left * left * sputter
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
