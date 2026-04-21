use glam::Vec2;
use tungsten::core::{
    ActionMap, AnimationRegistry, AnimationState, AudioCommands, CameraController, CameraState,
    CommandBuffer, DeltaTime, Entity, EventQueue, InputState, ParticleConfigRegistry,
    ParticleEmitter, ParticleEmitterState, Transform, World,
};
use tungsten::physics::{BodyKind, Collider, CollisionEvent, Position, RigidBody, Velocity};
use tungsten::WindowSize;

use crate::state::{
    ActiveBlackHole, AudioState, Ball, BallSpawnState, BlackHole, CurrentSprite, Player,
    TextDisplayState, BALL_RADIUS, BALL_RESTITUTION, BALL_SPAWN_INTERVAL, BALL_SPAWN_JITTER,
    BLACK_HOLE_FORCE, BLACK_HOLE_LIFETIME, BLACK_HOLE_RADIUS, MAP_ROWS, PLAYER_JUMP_IMPULSE,
    PLAYER_MOVE_SPEED, PLAYER_SPAWN, TEXT_UPDATE_INTERVAL, TILE, WORLD_BOUNDS_MAX,
    WORLD_BOUNDS_MIN,
};

/// Horizontal movement + jump. Plays the jump SFX when the player leaves the
/// ground. Runs BEFORE `physics_step` so velocity changes are integrated in
/// the same frame.
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

        let grounded = world
            .get::<Player>(entity)
            .map(|p| p.grounded)
            .unwrap_or(false);
        let want_jump = pressed_space && grounded;

        if let Some(vel) = world.get_mut::<Velocity>(entity) {
            vel.0.x = dx * PLAYER_MOVE_SPEED;
            if want_jump {
                vel.0.y = -PLAYER_JUMP_IMPULSE;
                did_jump = true;
            }
        }

        // Consume the grounded flag; `ground_detection` will re-set it below.
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

/// Music toggle (M), volume steps (1/2/3), stop-all (S).
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

/// = / - to zoom in or out. Adjusts `CameraController.zoom_multiplier` in 25%
/// steps, clamped to [0.5, 2.0] relative to the base window-height zoom.
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

/// Advances `AnimationState` and writes the resulting sprite ID into `CurrentSprite`.
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

/// Scans `EventQueue<CollisionEvent>` and flags the player as grounded on an upward contact.
pub(crate) fn ground_detection(world: &mut World) {
    let events: Vec<CollisionEvent> = match world.get_resource::<EventQueue<CollisionEvent>>() {
        Some(queue) => queue.iter().copied().collect(),
        None => return,
    };
    let player_entities: Vec<_> = world.query::<Player>().map(|(e, _)| e).collect();
    for entity in player_entities {
        if events.iter().any(|ev| ev.a == entity && ev.normal.y < -0.5) {
            if let Some(player) = world.get_mut::<Player>(entity) {
                player.grounded = true;
            }
        }
    }
}

/// Refreshes `TextDisplayState` at `TEXT_UPDATE_INTERVAL` so HUD values
/// update at a readable rate instead of every frame.
pub(crate) fn update_text_display(world: &mut World) {
    let dt = world
        .get_resource::<DeltaTime>()
        .map(|d| d.seconds())
        .unwrap_or(0.0);

    let timer = world
        .get_resource::<TextDisplayState>()
        .map(|s| s.timer)
        .unwrap_or(0.0);
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
        .map(|queue| queue.len())
        .unwrap_or(0);
    let grounded = world
        .query::<Player>()
        .next()
        .map(|(_, p)| p.grounded)
        .unwrap_or(false);
    let (music_on, vol_pct) = world
        .get_resource::<AudioState>()
        .map(|s| (s.music_playing, (s.master_volume * 100.0).round() as u32))
        .unwrap_or((false, 0));
    let zoom_pct = world
        .get_resource::<CameraController>()
        .map(|controller| (controller.zoom_multiplier * 100.0).round() as u32)
        .unwrap_or(100);

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

/// Convert a screen-space cursor position (physical pixels, top-left origin)
/// into a world-space point using the non-rotated camera transform. Returns
/// `None` if the camera is rotated (the platformer never rotates, so rather
/// than silently producing the wrong point we refuse).
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

/// Spawns balls at the world-space cursor position while `spawn_ball` is
/// held. A fixed accumulator advanced by `DeltaTime` produces one ball
/// per `BALL_SPAWN_INTERVAL` of held time, so the rate is stable under
/// variable frame times. Uses the deferred `CommandBuffer` so new
/// entities are visible to this frame's extract stage but not to
/// systems that already ran.
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
        .map(|d| d.seconds())
        .unwrap_or(0.0);
    let (spawn_count, phase_start) = {
        let state = match world.get_resource_mut::<BallSpawnState>() {
            Some(s) => s,
            None => return,
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

    let cursor = match world
        .get_resource::<InputState>()
        .and_then(|i| i.cursor_position())
    {
        Some((x, y)) => Vec2::new(x, y),
        None => return,
    };
    let camera = match world.get_resource::<CameraState>().copied() {
        Some(c) => c,
        None => return,
    };
    let Some(world_pos) = cursor_to_world(cursor, &camera) else {
        return;
    };
    // Golden angle (π(3 - √5)) distributes consecutive indices over the
    // circle with no rational period, so no two spawned balls share a
    // jitter offset in any session.
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
        }
    }
}

/// Spawns a black hole at the world-space cursor position on Mouse2 press
/// and drags it along while the button is held. While held, the hole's
/// `remaining` is refreshed every frame so it never expires mid-drag; on
/// release the dragged hole is despawned immediately. `ActiveBlackHole`
/// tracks the dragged entity so the release path only affects the hole
/// the user was actively controlling.
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

    let cursor = match world
        .get_resource::<InputState>()
        .and_then(|i| i.cursor_position())
    {
        Some((x, y)) => Vec2::new(x, y),
        None => return,
    };
    let camera = match world.get_resource::<CameraState>().copied() {
        Some(c) => c,
        None => return,
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

/// Applies an attractive acceleration to every dynamic body within
/// `BLACK_HOLE_RADIUS` of any active black hole. Pull strength peaks at
/// `BLACK_HOLE_FORCE` px/s² at the centre and falls linearly to zero at
/// the radius. Runs BEFORE `physics_step` so the velocity change is
/// integrated in the same frame. Tiles have no `Velocity` component
/// and are naturally excluded.
pub(crate) fn black_hole_force_system(world: &mut World) {
    let dt = world
        .get_resource::<DeltaTime>()
        .map(|d| d.seconds())
        .unwrap_or(0.0);
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
            .map(|b| b.kind == BodyKind::Dynamic)
            .unwrap_or(false);
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

/// Decrements the lifetime counter on every active black hole and
/// despawns those whose lifetime has expired.
pub(crate) fn black_hole_lifetime_system(world: &mut World) {
    let dt = world
        .get_resource::<DeltaTime>()
        .map(|d| d.seconds())
        .unwrap_or(0.0);

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
        if let Some(cmds) = world.get_resource_mut::<CommandBuffer>() {
            for entity in to_despawn {
                cmds.despawn(entity);
            }
        }
    }
}

/// Culls bodies that have escaped the active physics region defined by
/// `WORLD_BOUNDS_MIN`/`WORLD_BOUNDS_MAX`. Runaway balls are despawned via
/// the command buffer; the player is teleported back to `PLAYER_SPAWN`
/// with zero velocity. Runs after `physics_step`/`ground_detection` so
/// the position it checks is the post-resolve value for the frame.
///
/// Why: `compute_substeps` scales the whole world's cost by the single
/// worst `velocity / min_half_extent` ratio. A ball in free fall past the
/// tilemap accumulates velocity indefinitely and pegs that ratio to the
/// `PhysicsConfig::max_substeps` cap (default 8), multiplying every
/// frame's physics cost by ~8×. Bounding the active region keeps all
/// dynamic bodies inside the bounce regime.
pub(crate) fn despawn_out_of_bounds(world: &mut World) {
    let escaped_balls: Vec<Entity> = world
        .query::<Ball>()
        .filter_map(|(entity, _)| {
            let pos = world.get::<Position>(entity)?.0;
            is_out_of_bounds(pos).then_some(entity)
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
            is_out_of_bounds(pos).then_some(entity)
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

fn is_out_of_bounds(pos: Vec2) -> bool {
    pos.x < WORLD_BOUNDS_MIN.x
        || pos.x > WORLD_BOUNDS_MAX.x
        || pos.y < WORLD_BOUNDS_MIN.y
        || pos.y > WORLD_BOUNDS_MAX.y
}

/// Recomputes the platformer's base zoom from the current window height.
/// `camera_update_system` multiplies this by `CameraController.zoom_multiplier`
/// before producing the authoritative `CameraState` for the frame.
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
