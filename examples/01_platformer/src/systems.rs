use glam::Vec2;
use tungsten::core::{
    ActionMap, AnimationRegistry, AnimationState, AudioCommands, CameraController, CameraState,
    CommandBuffer, DeltaTime, Entity, EventQueue, InputState, Light, ParticleConfigRegistry,
    ParticleEmitter, ParticleEmitterState, Transform, World,
};
use tungsten::physics::{BodyKind, Collider, CollisionEvent, Position, RigidBody, Shape, Velocity};
use tungsten::WindowSize;

use crate::state::{
    ActiveBlackHole, AudioState, Ball, BallHue, BallSpawnState, BlackHole, CurrentSprite,
    CycleMode, OrbitLight, Player, TextDisplayState, BALL_ANIMATION_ID, BALL_RADIUS,
    BALL_RESTITUTION, BALL_SPAWN_INTERVAL, BALL_SPAWN_JITTER, BALL_START_SPRITE_ID,
    BLACK_HOLE_FORCE, BLACK_HOLE_LIFETIME, BLACK_HOLE_RADIUS, MAP_ROWS, PLAYER_JUMP_IMPULSE,
    PLAYER_MOVE_SPEED, PLAYER_SPAWN, TEXT_UPDATE_INTERVAL, TILE, WORLD_BOUNDS_MAX,
    WORLD_BOUNDS_MIN,
};

/// Player input before physics; jump SFX only on grounded launch.
pub(crate) fn player_input(world: &mut World) {
    let (pressed_left, pressed_right, pressed_space);
    {
        let Some(input) = world.get_resource::<InputState>() else {
            return;
        };
        let Some(actions) = world.get_resource::<ActionMap>() else {
            return;
        };
        pressed_left = actions.is_pressed(input, "move_left");
        pressed_right = actions.is_pressed(input, "move_right");
        pressed_space = actions.is_pressed(input, "jump");
    }

    let player_entities: Vec<_> = world.query::<Player>().map(|(e, _)| e).collect();
    let mut did_jump = false;

    for entity in player_entities {
        let mut dx = 0.0f32;
        if pressed_left {
            dx -= 1.0;
        }
        if pressed_right {
            dx += 1.0;
        }

        let grounded = world.get::<Player>(entity).is_some_and(|p| p.grounded);
        let want_jump = pressed_space && grounded;

        if let Some(vel) = world.get_mut::<Velocity>(entity) {
            vel.0.x = dx * PLAYER_MOVE_SPEED;
            if want_jump {
                vel.0.y = -PLAYER_JUMP_IMPULSE;
                did_jump = true;
            }
        }

        // Consumed here; `ground_detection` re-sets after physics.
        if let Some(player) = world.get_mut::<Player>(entity) {
            player.grounded = false;
        }
    }

    if did_jump {
        let handle_and_vol = world
            .get_resource::<AudioState>()
            .map(|s| (s.sfx_handle, s.sfx_volume));
        if let Some((handle, vol)) = handle_and_vol {
            if let Some(cmds) = world.get_resource_mut::<AudioCommands>() {
                cmds.play_with(handle, vol, false);
            }
        }
    }
}

pub(crate) fn audio_input_system(world: &mut World) {
    let (just_m, just_1, just_2, just_3, just_s);
    {
        let Some(input) = world.get_resource::<InputState>() else {
            return;
        };
        let Some(actions) = world.get_resource::<ActionMap>() else {
            return;
        };
        just_m = actions.just_pressed(input, "audio_toggle_music");
        just_1 = actions.just_pressed(input, "volume_preset_low");
        just_2 = actions.just_pressed(input, "volume_preset_mid");
        just_3 = actions.just_pressed(input, "volume_preset_high");
        just_s = actions.just_pressed(input, "audio_stop_all");
    }

    if world.get_resource::<AudioState>().is_none() {
        return;
    }

    {
        let state = world.get_resource_mut::<AudioState>().unwrap();
        if just_1 {
            state.master_volume = 0.2;
        }
        if just_2 {
            state.master_volume = 0.5;
        }
        if just_3 {
            state.master_volume = 1.0;
        }
        if just_m {
            state.music_playing = !state.music_playing;
        }
        if just_s {
            state.music_playing = false;
        }
    }

    let (music_handle, music_volume, music_playing, master_volume) = {
        let state = world.get_resource::<AudioState>().unwrap();
        (
            state.music_handle,
            state.music_volume,
            state.music_playing,
            state.master_volume,
        )
    };

    let cmds = world.get_resource_mut::<AudioCommands>().unwrap();
    if just_1 || just_2 || just_3 {
        cmds.set_master_volume(master_volume);
    }
    if just_s {
        cmds.stop_all();
    }
    if just_m {
        if music_playing {
            cmds.play_with(music_handle, music_volume, true);
        } else {
            cmds.stop(music_handle);
        }
    }
}

pub(crate) fn camera_zoom_input_system(world: &mut World) {
    let (just_zoom_in, just_zoom_out);
    {
        let Some(input) = world.get_resource::<InputState>() else {
            return;
        };
        let Some(actions) = world.get_resource::<ActionMap>() else {
            return;
        };
        just_zoom_in = actions.just_pressed(input, "zoom_in");
        just_zoom_out = actions.just_pressed(input, "zoom_out");
    }
    if !just_zoom_in && !just_zoom_out {
        return;
    }
    if let Some(controller) = world.get_resource_mut::<CameraController>() {
        if just_zoom_in {
            controller.zoom_multiplier = (controller.zoom_multiplier + 0.25).min(2.0);
        }
        if just_zoom_out {
            controller.zoom_multiplier = (controller.zoom_multiplier - 0.25).max(0.5);
        }
    }
}

pub(crate) fn animation_system(world: &mut World) {
    let dt_ms = world.get_resource::<DeltaTime>().unwrap().seconds() * 1000.0;
    let anim_registry = match world.get_resource::<AnimationRegistry>() {
        Some(r) => r.clone(),
        None => return,
    };
    let entities = world.query_entities::<AnimationState>();
    for entity in entities {
        let mut state = world.get::<AnimationState>(entity).unwrap().clone();
        let new_sprite = state.advance(dt_ms, &anim_registry);
        *world.get_mut::<AnimationState>(entity).unwrap() = state;
        if let Some(sprite_id) = new_sprite {
            if let Some(cs) = world.get_mut::<CurrentSprite>(entity) {
                cs.0 = sprite_id;
            }
        }
    }
}

pub(crate) fn rainbow_ball_hue_system(world: &mut World) {
    let dt = world
        .get_resource::<DeltaTime>()
        .map_or(0.0, DeltaTime::seconds);
    if dt <= 0.0 {
        return;
    }

    let entities = world.query_entities::<BallHue>();
    for entity in entities {
        let Some(hue) = world.get_mut::<BallHue>(entity) else {
            continue;
        };
        hue.hue = (hue.hue + hue.speed * dt).rem_euclid(1.0);
    }
}

pub(crate) fn ground_detection(world: &mut World) {
    let events: Vec<CollisionEvent> = match world.get_resource::<EventQueue<CollisionEvent>>() {
        Some(queue) => queue.iter().copied().collect(),
        None => return,
    };
    let player_entities: Vec<_> = world.query::<Player>().map(|(e, _)| e).collect();
    for entity in player_entities {
        if events
            .iter()
            .any(|event| player_is_grounded_by_event(entity, event))
        {
            if let Some(player) = world.get_mut::<Player>(entity) {
                player.grounded = true;
            }
        }
    }
}

fn player_is_grounded_by_event(player: Entity, event: &CollisionEvent) -> bool {
    if event.a == player {
        event.normal.y < -0.5
    } else if event.b == Some(player) {
        event.normal.y > 0.5
    } else {
        false
    }
}

/// M26 damage-flash: when the player collides with a Ball, queue a one-shot
/// tween that drives the `damage_flash` material uniform block from a lit-up
/// red overlay back to zero in 250 ms.
pub(crate) fn damage_flash_on_ball_hit(world: &mut World) {
    use crate::state::{Ball, PlayerMaterial};
    use tungsten::core::{Easing, ScalarSlot, Tween, TweenChannel, UniformOverrideBlock, Vec4Slot};

    let events: Vec<CollisionEvent> = match world.get_resource::<EventQueue<CollisionEvent>>() {
        Some(queue) => queue.iter().copied().collect(),
        None => return,
    };
    if events.is_empty() {
        return;
    }

    // Pre-collect players with the damage material; skip work when none exist.
    let players: Vec<_> = world.query::<PlayerMaterial>().map(|(e, _)| e).collect();
    if players.is_empty() {
        return;
    }
    let balls: std::collections::HashSet<_> = world.query::<Ball>().map(|(e, _)| e).collect();

    for player in players {
        let was_hit = events.iter().any(|ev| {
            (ev.a == player && ev.b.is_some_and(|b| balls.contains(&b)))
                || (ev.b == Some(player) && balls.contains(&ev.a))
        });
        if !was_hit {
            continue;
        }
        // D-055 keeps one Tween per entity — overwrite any active tween.
        let tween = Tween::new(0.25, Easing::QuadOut)
            .with_channel(TweenChannel::UniformVec4Lane {
                slot: Vec4Slot::V0,
                lane: 0,
                from: 1.0,
                to: 0.0,
            })
            .with_channel(TweenChannel::UniformScalar {
                slot: ScalarSlot::F0,
                from: 0.8,
                to: 0.0,
            });
        // Seed the override block at its hit-state so the first frame is red.
        if let Some(block) = world.get_mut::<UniformOverrideBlock>(player) {
            block.vec4[Vec4Slot::V0.index()] = [1.0, 0.2, 0.1, 1.0];
            block.f32s[ScalarSlot::F0.index()] = 0.8;
        }
        world.insert(player, tween);
    }
}

pub(crate) fn update_text_display(world: &mut World) {
    let dt = world
        .get_resource::<DeltaTime>()
        .map_or(0.0, DeltaTime::seconds);

    let timer = world
        .get_resource::<TextDisplayState>()
        .map_or(0.0, |s| s.timer);
    let new_timer = timer + dt;

    if new_timer < TEXT_UPDATE_INTERVAL {
        if let Some(state) = world.get_resource_mut::<TextDisplayState>() {
            state.timer = new_timer;
        }
        return;
    }

    let fps = if dt > 0.0 {
        (1.0 / dt).round() as u32
    } else {
        0
    };
    let contacts = world
        .get_resource::<EventQueue<CollisionEvent>>()
        .map_or(0, EventQueue::len);
    let grounded = world
        .query::<Player>()
        .next()
        .is_some_and(|(_, p)| p.grounded);
    let (music_on, vol_pct) = world.get_resource::<AudioState>().map_or((false, 0), |s| {
        (s.music_playing, (s.master_volume * 100.0).round() as u32)
    });
    let zoom_pct = world
        .get_resource::<CameraController>()
        .map_or(100, |controller| {
            (controller.zoom_multiplier * 100.0).round() as u32
        });

    if let Some(state) = world.get_resource_mut::<TextDisplayState>() {
        state.fps = fps;
        state.contacts = contacts;
        state.grounded = grounded;
        state.music_on = music_on;
        state.vol_pct = vol_pct;
        state.zoom_pct = zoom_pct;
        state.timer = new_timer - TEXT_UPDATE_INTERVAL;
    }
}

/// Screen cursor to world point; rotated cameras refused.
pub(crate) fn cursor_to_world(cursor: Vec2, camera: &CameraState) -> Option<Vec2> {
    if camera.rotation != 0.0 {
        return None;
    }
    let zoom = camera.zoom.max(f32::EPSILON);
    Some(Vec2::new(
        camera.position.x + cursor.x / zoom,
        camera.position.y + cursor.y / zoom,
    ))
}

/// Hold-to-spawn balls via fixed accumulator and deferred commands.
pub(crate) fn spawn_ball_system(world: &mut World) {
    let held = {
        let Some(input) = world.get_resource::<InputState>() else {
            return;
        };
        let Some(actions) = world.get_resource::<ActionMap>() else {
            return;
        };
        actions.is_pressed(input, "spawn_ball")
    };

    if !held {
        if let Some(state) = world.get_resource_mut::<BallSpawnState>() {
            state.accumulator = 0.0;
        }
        return;
    }

    let dt = world
        .get_resource::<DeltaTime>()
        .map_or(0.0, DeltaTime::seconds);
    let (spawn_count, phase_start) = {
        let Some(state) = world.get_resource_mut::<BallSpawnState>() else {
            return;
        };
        state.accumulator += dt;
        let mut count = 0u32;
        while state.accumulator >= BALL_SPAWN_INTERVAL {
            state.accumulator -= BALL_SPAWN_INTERVAL;
            count += 1;
        }
        let phase_start = state.spawn_phase;
        state.spawn_phase = state.spawn_phase.wrapping_add(count);
        (count, phase_start)
    };
    if spawn_count == 0 {
        return;
    }

    let Some((cursor_x, cursor_y)) = world
        .get_resource::<InputState>()
        .and_then(InputState::cursor_position)
    else {
        return;
    };
    let cursor = Vec2::new(cursor_x, cursor_y);
    let Some(camera) = world.get_resource::<CameraState>().copied() else {
        return;
    };
    let Some(world_pos) = cursor_to_world(cursor, &camera) else {
        return;
    };
    // Golden-angle jitter avoids coincident circle degeneracy.
    const GOLDEN_ANGLE: f32 = 2.399_963_2;
    if let Some(cmds) = world.get_resource_mut::<CommandBuffer>() {
        for i in 0..spawn_count {
            let phase = phase_start.wrapping_add(i);
            let angle = phase as f32 * GOLDEN_ANGLE;
            let offset = Vec2::new(angle.cos(), angle.sin()) * BALL_SPAWN_JITTER;
            let ball = cmds.spawn();
            cmds.insert_pending(ball, Ball);
            cmds.insert_pending(ball, Position(world_pos + offset));
            cmds.insert_pending(ball, Velocity(Vec2::ZERO));
            cmds.insert_pending(ball, Collider::circle(BALL_RADIUS));
            cmds.insert_pending(
                ball,
                RigidBody {
                    kind: BodyKind::Dynamic,
                    inv_mass: 1.0,
                    restitution: BALL_RESTITUTION,
                },
            );
            cmds.insert_pending(ball, AnimationState::new(BALL_ANIMATION_ID));
            cmds.insert_pending(ball, CurrentSprite(BALL_START_SPRITE_ID.into()));
            cmds.insert_pending(ball, BallHue::from_seed(phase));
        }
    }
}

/// Mouse2 drag-spawns active black hole; release despawns dragged entity.
pub(crate) fn spawn_black_hole_system(world: &mut World) {
    let (just_pressed, is_held, just_released) = {
        let Some(input) = world.get_resource::<InputState>() else {
            return;
        };
        let Some(actions) = world.get_resource::<ActionMap>() else {
            return;
        };
        (
            actions.just_pressed(input, "spawn_black_hole"),
            actions.is_pressed(input, "spawn_black_hole"),
            actions.just_released(input, "spawn_black_hole"),
        )
    };

    if just_released {
        let prev = world
            .get_resource_mut::<ActiveBlackHole>()
            .and_then(|a| a.0.take());
        if let Some(entity) = prev {
            world.despawn(entity);
        }
    }
    if !is_held {
        return;
    }

    let Some((cursor_x, cursor_y)) = world
        .get_resource::<InputState>()
        .and_then(InputState::cursor_position)
    else {
        return;
    };
    let cursor = Vec2::new(cursor_x, cursor_y);
    let Some(camera) = world.get_resource::<CameraState>().copied() else {
        return;
    };
    let Some(world_pos) = cursor_to_world(cursor, &camera) else {
        return;
    };

    if just_pressed {
        let entity = world.spawn();
        world.insert(
            entity,
            BlackHole {
                remaining: BLACK_HOLE_LIFETIME,
            },
        );
        world.insert(entity, Position(world_pos));
        world.insert(entity, Transform::from_position(world_pos));
        if let Some(cfg_id) = world
            .get_resource::<ParticleConfigRegistry>()
            .and_then(|r| r.id_for_name("ex10_black_hole"))
        {
            world.insert(entity, ParticleEmitter::new(cfg_id));
            world.insert(entity, ParticleEmitterState::default());
        }
        if let Some(active) = world.get_resource_mut::<ActiveBlackHole>() {
            active.0 = Some(entity);
        }
        let handle_and_vol = world
            .get_resource::<AudioState>()
            .map(|s| (s.black_hole_sfx_handle, s.black_hole_sfx_volume));
        if let Some((handle, vol)) = handle_and_vol {
            if let Some(cmds) = world.get_resource_mut::<AudioCommands>() {
                cmds.play_with(handle, vol, false);
            }
        }
        return;
    }

    let active_entity = world.get_resource::<ActiveBlackHole>().and_then(|a| a.0);
    if let Some(entity) = active_entity {
        if let Some(pos) = world.get_mut::<Position>(entity) {
            pos.0 = world_pos;
        }
        if let Some(t) = world.get_mut::<Transform>(entity) {
            t.position = world_pos;
        }
        if let Some(hole) = world.get_mut::<BlackHole>(entity) {
            hole.remaining = BLACK_HOLE_LIFETIME;
        }
    }
}

/// Black-hole acceleration before physics; static tiles excluded by no velocity.
pub(crate) fn black_hole_force_system(world: &mut World) {
    let dt = world
        .get_resource::<DeltaTime>()
        .map_or(0.0, DeltaTime::seconds);
    if dt <= 0.0 {
        return;
    }

    let holes: Vec<Vec2> = world
        .query::<BlackHole>()
        .filter_map(|(entity, _)| world.get::<Position>(entity).map(|p| p.0))
        .collect();
    if holes.is_empty() {
        return;
    }

    let targets: Vec<Entity> = world.query_entities::<Velocity>();
    for entity in targets {
        let body_is_dynamic = world
            .get::<RigidBody>(entity)
            .is_some_and(|b| b.kind == BodyKind::Dynamic);
        if !body_is_dynamic {
            continue;
        }
        let Some(pos) = world.get::<Position>(entity).copied() else {
            continue;
        };

        let mut accel = Vec2::ZERO;
        for &hole_pos in &holes {
            let delta = hole_pos - pos.0;
            let dist_sq = delta.length_squared();
            if !(1.0e-4..BLACK_HOLE_RADIUS * BLACK_HOLE_RADIUS).contains(&dist_sq) {
                continue;
            }
            let dist = dist_sq.sqrt();
            let falloff = 1.0 - (dist / BLACK_HOLE_RADIUS);
            let dir = delta / dist;
            accel += dir * BLACK_HOLE_FORCE * falloff;
        }
        if accel == Vec2::ZERO {
            continue;
        }
        if let Some(vel) = world.get_mut::<Velocity>(entity) {
            vel.0 += accel * dt;
        }
    }
}

pub(crate) fn black_hole_lifetime_system(world: &mut World) {
    let dt = world
        .get_resource::<DeltaTime>()
        .map_or(0.0, DeltaTime::seconds);

    let entities = world.query_entities::<BlackHole>();
    let mut to_despawn: Vec<Entity> = Vec::new();
    for entity in entities {
        if let Some(hole) = world.get_mut::<BlackHole>(entity) {
            hole.remaining -= dt;
            if hole.remaining <= 0.0 {
                to_despawn.push(entity);
            }
        }
    }
    if !to_despawn.is_empty() {
        let active_expired = world
            .get_resource::<ActiveBlackHole>()
            .and_then(|active| active.0)
            .is_some_and(|active| to_despawn.contains(&active));
        if active_expired {
            if let Some(active) = world.get_resource_mut::<ActiveBlackHole>() {
                active.0 = None;
            }
        }
        if let Some(cmds) = world.get_resource_mut::<CommandBuffer>() {
            for entity in to_despawn {
                cmds.despawn(entity);
            }
        }
    }
}

/// Cull escaped bodies after physics; bounds prevent substep-cost runaway.
pub(crate) fn despawn_out_of_bounds(world: &mut World) {
    let escaped_balls: Vec<Entity> = world
        .query::<Ball>()
        .filter_map(|(entity, _)| {
            let pos = world.get::<Position>(entity)?.0;
            let collider = world.get::<Collider>(entity).copied();
            is_body_out_of_bounds(pos, collider).then_some(entity)
        })
        .collect();

    if !escaped_balls.is_empty() {
        if let Some(cmds) = world.get_resource_mut::<CommandBuffer>() {
            for entity in escaped_balls {
                cmds.despawn(entity);
            }
        }
    }

    let escaped_players: Vec<Entity> = world
        .query::<Player>()
        .filter_map(|(entity, _)| {
            let pos = world.get::<Position>(entity)?.0;
            let collider = world.get::<Collider>(entity).copied();
            is_body_out_of_bounds(pos, collider).then_some(entity)
        })
        .collect();

    for entity in escaped_players {
        if let Some(pos) = world.get_mut::<Position>(entity) {
            pos.0 = PLAYER_SPAWN;
        }
        if let Some(vel) = world.get_mut::<Velocity>(entity) {
            vel.0 = Vec2::ZERO;
        }
    }
}

fn is_body_out_of_bounds(pos: Vec2, collider: Option<Collider>) -> bool {
    let (min, max) = body_bounds(pos, collider);
    max.x < WORLD_BOUNDS_MIN.x
        || min.x > WORLD_BOUNDS_MAX.x
        || max.y < WORLD_BOUNDS_MIN.y
        || min.y > WORLD_BOUNDS_MAX.y
}

fn body_bounds(pos: Vec2, collider: Option<Collider>) -> (Vec2, Vec2) {
    let Some(collider) = collider else {
        return (pos, pos);
    };
    let center = pos + collider.offset;
    let half = match collider.shape {
        Shape::Aabb { half_extents } => half_extents,
        Shape::Circle { radius } => Vec2::splat(radius),
    };
    (center - half, center + half)
}

/// M29 lighting fixture: orbit warm + cool point lights around the camera
/// target, advance each light's phase, and (per `CycleMode`) drive its
/// `Light.color` and `Light.intensity` so the lighting visibly cycles.
///
/// `CycleMode::Pulse` modulates intensity sin-style around 1.0 while holding
/// the authored `base_color`. `CycleMode::Hue` rotates the color around the
/// HSV wheel using `phase` as the angle while holding intensity. `None`
/// only orbits the position.
pub(crate) fn orbit_lights_system(world: &mut World) {
    let dt = world
        .get_resource::<DeltaTime>()
        .map(|d| d.dt)
        .unwrap_or_default();
    let center = world
        .get_resource::<CameraState>()
        .map_or(PLAYER_SPAWN, |camera| camera.position);
    struct Update {
        entity: Entity,
        pos: Vec2,
        phase: f32,
        color: Option<Vec2X3>,
        intensity: Option<f32>,
    }
    let updates: Vec<Update> = world
        .query::<OrbitLight>()
        .map(|(e, ol)| {
            let new_phase = ol.phase + ol.speed * dt;
            let pos = Vec2::new(
                center.x + new_phase.cos() * ol.radius,
                center.y + new_phase.sin() * ol.radius * 0.5,
            );
            let (color, intensity) = match ol.cycle {
                CycleMode::None => (None, None),
                CycleMode::Pulse => {
                    // Sin-pulse intensity in [0.45, 1.45] so the orbit reads
                    // even at the dim end. Color held at base_color.
                    let i = 0.95 + 0.5 * (new_phase * 1.7).sin();
                    (Some(Vec2X3(ol.base_color)), Some(i))
                }
                CycleMode::Hue => {
                    // Slow hue rotation; full revolution every ~10 sec.
                    let hue = (new_phase * 0.1).rem_euclid(1.0);
                    (Some(Vec2X3(hsv_to_rgb(hue, 0.8, 1.0))), None)
                }
            };
            Update {
                entity: e,
                pos,
                phase: new_phase,
                color,
                intensity,
            }
        })
        .collect();
    for u in updates {
        if let Some(t) = world.get_mut::<Transform>(u.entity) {
            t.position = u.pos;
        }
        if let Some(ol) = world.get_mut::<OrbitLight>(u.entity) {
            ol.phase = u.phase;
        }
        if let Some(l) = world.get_mut::<Light>(u.entity) {
            if let Some(c) = u.color {
                l.color = c.0;
            }
            if let Some(i) = u.intensity {
                l.intensity = i;
            }
        }
    }
}

/// Tiny wrapper to keep glam::Vec3 out of the local Update struct's signature
/// without dragging the import here. Pure ergonomics, not behavior.
struct Vec2X3(glam::Vec3);

fn hsv_to_rgb(hue: f32, saturation: f32, value: f32) -> glam::Vec3 {
    let hue_sector = hue.rem_euclid(1.0) * 6.0;
    let sector_index = hue_sector.floor();
    let fraction = hue_sector - sector_index;
    let base = value * (1.0 - saturation);
    let falling = value * (1.0 - saturation * fraction);
    let rising = value * (1.0 - saturation * (1.0 - fraction));
    match sector_index as i32 % 6 {
        0 => glam::Vec3::new(value, rising, base),
        1 => glam::Vec3::new(falling, value, base),
        2 => glam::Vec3::new(base, value, rising),
        3 => glam::Vec3::new(base, falling, value),
        4 => glam::Vec3::new(rising, base, value),
        _ => glam::Vec3::new(value, base, falling),
    }
}

/// Base zoom from window height before shared camera update.
pub(crate) fn platformer_camera_base_zoom(world: &mut World) {
    let window = world
        .get_resource::<WindowSize>()
        .copied()
        .unwrap_or(WindowSize {
            width: 1920,
            height: 1080,
        });
    let map_h = (MAP_ROWS as f32) * TILE;
    let base_zoom = (window.height as f32 / map_h).max(f32::EPSILON);
    if let Some(camera) = world.get_resource_mut::<CameraState>() {
        camera.zoom = base_zoom;
    }
}
