use glam::Vec2;
use tungsten::WindowSize;
use tungsten::core::{
    ActionMap, AnimationRegistry, AnimationState, AssetRegistry, AudioCommands, AudioHandle,
    CameraController, CameraState, CommandBuffer, DeltaTime, Entity, EventQueue, InputState, Light,
    ParticleConfigRegistry, ParticleEmitter, ParticleEmitterState, ShakeEvent, SquashEvent,
    SquashTrigger, Transform, World,
};
use tungsten::physics::{BodyKind, Collider, CollisionEvent, Position, RigidBody, Shape, Velocity};

use crate::burning::BallBurn;
use crate::gameplay::EmitterAnchor;
use crate::state::PlayerEffect;
use crate::state::{
    ActiveBlackHole, AudioState, BALL_ANIMATION_ID, BALL_CAP, BALL_RADIUS, BALL_RESTITUTION,
    BALL_SPAWN_INTERVAL, BALL_SPAWN_JITTER, BALL_START_SPRITE_ID, BLACK_HOLE_FORCE,
    BLACK_HOLE_LIFETIME, BLACK_HOLE_RADIUS, Ball, BallHue, BallSpawnState, BlackHole, CAMERA_ROWS,
    CurrentSprite, CycleMode, EXTINGUISH_BURSTS_PER_FRAME, EXTINGUISH_SFX_INTERVAL, EffectSequence,
    EffectSounds, KILL_Y, OrbitLight, PLAYER_HALF, PLAYER_JUMP_IMPULSE, PLAYER_MOVE_SPEED,
    PLAYER_SPAWN, PLAYER_START_SPRITE_ID, Player, PlayerPresentation, TEXT_UPDATE_INTERVAL, TILE,
    TRANSIENT_EMITTER_CAP, TextDisplayState, TransientEmitter, WORLD_BOUNDS_MAX, WORLD_BOUNDS_MIN,
};
use crate::state::{
    SMALL_BALL_ANIMATION_ID, SMALL_BALL_SCALE, SMALL_BALL_SPAWN_INTERVAL,
    SMALL_BALL_START_SPRITE_ID, SmallBall,
};

/// Ground jump preserves hold behavior; the aerial jump needs a fresh press.
pub(crate) fn player_input(world: &mut World) {
    let (pressed_left, pressed_right, pressed_space, jump_pressed);
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
        jump_pressed = actions.just_pressed(input, "jump");
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
        let locked = world
            .get::<crate::gameplay::Health>(entity)
            .is_some_and(|h| h.control_lock > 0.0);
        let second_jump = !grounded
            && jump_pressed
            && !locked
            && world
                .get::<Player>(entity)
                .is_some_and(|p| !p.air_jump_used);
        let want_jump = (pressed_space && grounded && !locked) || second_jump;
        if second_jump {
            world.get_mut::<Player>(entity).unwrap().air_jump_used = true;
        }

        if let Some(vel) = world.get_mut::<Velocity>(entity) {
            if !locked {
                vel.0.x = dx * PLAYER_MOVE_SPEED;
            }
            if want_jump {
                vel.0.y = -PLAYER_JUMP_IMPULSE;
                did_jump = true;
            }
        }

        let feet = world
            .get::<Position>(entity)
            .map(|p| p.0 + Vec2::new(0.0, PLAYER_HALF.y));
        if let Some(presentation) = world.get_mut::<PlayerPresentation>(entity) {
            presentation.pending_effect = want_jump.then_some(if second_jump {
                PlayerEffect::DoubleJump
            } else {
                PlayerEffect::Jump
            });
            if want_jump && let Some(feet) = feet {
                presentation.jump_origin = feet;
            }
            if dx != 0.0 {
                presentation.facing_left = dx < 0.0;
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
        if let Some((handle, vol)) = handle_and_vol
            && let Some(cmds) = world.get_resource_mut::<AudioCommands>()
        {
            cmds.play_with(handle, vol, false);
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
            controller.zoom_multiplier = (controller.zoom_multiplier + 0.25).min(3.0);
        }
        if just_zoom_out {
            controller.zoom_multiplier = (controller.zoom_multiplier - 0.25).max(0.35);
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
        // `CurrentSprite` keeps the name; the custom extract resolves it.
        let name = new_sprite.and_then(|id| {
            world
                .get_resource::<AssetRegistry>()?
                .sprite_name(id)
                .map(str::to_owned)
        });
        if let Some(name) = name
            && let Some(cs) = world.get_mut::<CurrentSprite>(entity)
        {
            cs.0 = name;
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
        Some(queue) => queue.iter_current().copied().collect(),
        None => return,
    };
    let player_entities: Vec<_> = world.query::<Player>().map(|(e, _)| e).collect();
    let mut landed: Vec<Entity> = Vec::new();
    for entity in player_entities {
        // Sleeping bodies emit no new contacts. Retain their settled state;
        // an accepted upward jump ignores the initial separating floor contact.
        let sleeping = world
            .get_resource::<tungsten::physics::PhysicsBuffers>()
            .is_some_and(|buffers| buffers.is_sleeping(entity));
        let was_grounded = world.get::<Player>(entity).is_some_and(|p| p.was_grounded);
        let rising = world.get::<Velocity>(entity).is_some_and(|v| v.0.y < -1.0);
        let launched = world.get::<PlayerPresentation>(entity).is_some_and(|p| {
            matches!(
                p.pending_effect,
                Some(PlayerEffect::Jump | PlayerEffect::DoubleJump)
            )
        });
        let grounded = !rising
            && !launched
            && ((sleeping && was_grounded)
                || events
                    .iter()
                    .any(|event| player_is_grounded_by_event(entity, event)));
        if let Some(player) = world.get_mut::<Player>(entity) {
            player.grounded = grounded;
            if grounded {
                player.air_jump_used = false;
            }
            // M30: rising edge only. `player.grounded` is cleared every frame in
            // `player_input`, so `was_grounded` is what distinguishes a landing
            // from standing still.
            if grounded && !player.was_grounded {
                landed.push(entity);
            }
            player.was_grounded = grounded;
        }
    }

    for entity in player_entities_for_landing(world, &landed) {
        if let Some(queue) = world.get_resource_mut::<EventQueue<SquashEvent>>() {
            queue.send(SquashEvent {
                entity,
                trigger: SquashTrigger::OnLand,
            });
        }
    }
}

fn player_entities_for_landing(world: &mut World, landed: &[Entity]) -> Vec<Entity> {
    let mut effects = Vec::new();
    for entity in world.query_entities::<Player>() {
        let grounded = world.get::<Player>(entity).is_some_and(|p| p.grounded);
        if let Some(p) = world.get_mut::<PlayerPresentation>(entity) {
            if landed.contains(&entity) && !p.suppress_landing {
                p.pending_effect = Some(PlayerEffect::Land);
                effects.push(entity);
            }
            if grounded {
                p.suppress_landing = false;
            }
        } else if landed.contains(&entity) {
            effects.push(entity);
        }
    }
    effects
}

/// Select after ground detection/reset; reset a clip only on a transition.
pub(crate) fn player_presentation_system(world: &mut World) {
    let dt = world
        .get_resource::<DeltaTime>()
        .map_or(0.0, DeltaTime::seconds);
    for entity in world.query_entities::<PlayerPresentation>() {
        let grounded = world.get::<Player>(entity).is_some_and(|p| p.grounded);
        let velocity = world.get::<Velocity>(entity).map_or(Vec2::ZERO, |v| v.0);
        let Some(position) = world.get::<Position>(entity).map(|p| p.0) else {
            continue;
        };
        let (clip, changed, jumped, landed) = {
            let p = world.get_mut::<PlayerPresentation>(entity).unwrap();
            p.landing_lock = (p.landing_lock - dt).max(0.0);
            if p.pending_effect == Some(PlayerEffect::Land) {
                p.landing_lock = 0.18;
            }
            if matches!(
                p.pending_effect,
                Some(PlayerEffect::Jump | PlayerEffect::DoubleJump)
            ) || !grounded
            {
                p.landing_lock = 0.0;
            }
            if p.pending_effect == Some(PlayerEffect::DoubleJump) {
                p.aerial_tuck = true;
            }
            if grounded || velocity.y >= 0.0 {
                p.aerial_tuck = false;
            }
            let clip = if !grounded {
                if velocity.y < 0.0 && p.aerial_tuck {
                    "ex10_player_double_jump"
                } else if velocity.y < 0.0 {
                    "ex10_player_jump"
                } else {
                    "ex10_player_fall"
                }
            } else if p.landing_lock > 0.0 {
                "ex10_player_land"
            } else if velocity.x.abs() > 1.0 {
                "ex10_player_walk"
            } else {
                "ex10_player_idle"
            };
            let result = (
                clip,
                clip != p.clip || p.pending_effect == Some(PlayerEffect::DoubleJump),
                matches!(
                    p.pending_effect,
                    Some(PlayerEffect::Jump | PlayerEffect::DoubleJump)
                )
                .then_some((
                    p.jump_origin,
                    p.pending_effect == Some(PlayerEffect::DoubleJump),
                )),
                p.pending_effect == Some(PlayerEffect::Land),
            );
            p.clip = clip;
            p.pending_effect = None;
            result
        };
        if changed {
            let state = AnimationState::new(clip);
            let sprite = world
                .get_resource::<AnimationRegistry>()
                .and_then(|registry| state.current_sprite(registry))
                .and_then(|id| world.get_resource::<AssetRegistry>()?.sprite_name(id))
                .map(str::to_owned);
            world.insert(entity, state);
            if let Some(sprite) = sprite {
                world.insert(entity, CurrentSprite(sprite));
            }
        }
        let feet = position + Vec2::new(0.0, PLAYER_HALF.y);
        if let Some((origin, aerial)) = jumped {
            spawn_transient_effect(
                world,
                if aerial {
                    "ex10_double_jump"
                } else {
                    "ex10_jump_puff"
                },
                origin,
            );
        }
        if landed {
            spawn_transient_effect(world, "ex10_landing_dust", feet);
        }
    }
}

pub(crate) fn spawn_transient_effect(world: &mut World, name: &str, position: Vec2) {
    if world.query::<TransientEmitter>().count() >= TRANSIENT_EMITTER_CAP {
        return;
    }
    let Some(config) = world
        .get_resource::<ParticleConfigRegistry>()
        .and_then(|r| r.id_for_name(name))
    else {
        return;
    };
    let seed = if let Some(sequence) = world.get_resource_mut::<EffectSequence>() {
        sequence.0 = sequence.0.wrapping_add(1);
        0x1a_0000 + sequence.0
    } else {
        0x1a_0000
    };
    // Direct creation lets the engine's single emit pass see this frame's trigger.
    let entity = world.spawn();
    world.insert(entity, TransientEmitter);
    world.insert(entity, Transform::from_position(position));
    world.insert(entity, ParticleEmitter::with_seed(config, seed));
    world.insert(entity, ParticleEmitterState::default());
}

/// Play one of the spell/fire sounds when the audio resources exist.
pub(crate) fn play_effect_sound(world: &mut World, pick: fn(&EffectSounds) -> (AudioHandle, f32)) {
    let Some((handle, volume)) = world.get_resource::<EffectSounds>().map(pick) else {
        return;
    };
    if let Some(cmds) = world.get_resource_mut::<AudioCommands>() {
        cmds.play_with(handle, volume, false);
    }
}

pub(crate) fn transient_emitter_cleanup(world: &mut World) {
    let finished: Vec<_> = world
        .query::<TransientEmitter>()
        .filter_map(|(entity, _)| {
            let state = world.get::<ParticleEmitterState>(entity)?;
            // Counts refresh after user systems. Wait for the engine's drained report,
            // then verify ownership too; a fresh burst must receive its first tick.
            (state.first_tick_done
                && state.drained
                && state.drain_reported
                && state.active_count == 0
                && !world
                    .query::<tungsten::core::Particle>()
                    .any(|(_, p)| p.emitter == Some(entity)))
            .then_some(entity)
        })
        .collect();
    for entity in finished {
        world.despawn(entity);
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

/// Shared hazard hit flash and camera feedback; balls never call this.
pub(crate) fn damage_feedback(world: &mut World, player: Entity) {
    use tungsten::core::{Easing, ScalarSlot, Tween, TweenChannel, UniformOverrideBlock, Vec4Slot};
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
    // M30: the same hit kicks the camera. The flash keeps the entity's one
    // `D-055` tween slot; trauma lives on the camera controller, so both
    // read on screen at once.
    if let Some(queue) = world.get_resource_mut::<EventQueue<ShakeEvent>>() {
        queue.send(ShakeEvent { trauma_add: 0.5 });
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

/// Hold-to-spawn balls via fixed accumulator and deferred commands, up to
/// `BALL_CAP` live balls.
pub(crate) fn spawn_ball_system(world: &mut World) {
    // Counted once: both spawners draw on one budget, so a frame never
    // overshoots the cap.
    let live = world.query::<Ball>().count();
    let budget = u32::try_from(BALL_CAP.saturating_sub(live)).unwrap_or(u32::MAX);
    let spawned = spawn_balls(world, false, budget);
    spawn_balls(world, true, budget - spawned);
}

/// Spawns at most `budget` balls and returns how many it queued.
fn spawn_balls(world: &mut World, small: bool, budget: u32) -> u32 {
    let held = {
        let Some(input) = world.get_resource::<InputState>() else {
            return 0;
        };
        let Some(actions) = world.get_resource::<ActionMap>() else {
            return 0;
        };
        actions.is_pressed(
            input,
            if small {
                "spawn_small_ball"
            } else {
                "spawn_ball"
            },
        )
    };

    // At the cap the accumulated time is dropped, as on release, so balls do
    // not burst out when the count falls again.
    if !held || budget == 0 {
        if let Some(state) = world.get_resource_mut::<BallSpawnState>() {
            if small {
                state.small_accumulator = 0.0;
            } else {
                state.accumulator = 0.0;
            }
        }
        return 0;
    }

    let dt = world
        .get_resource::<DeltaTime>()
        .map_or(0.0, DeltaTime::seconds);
    let (spawn_count, phase_start) = {
        let Some(state) = world.get_resource_mut::<BallSpawnState>() else {
            return 0;
        };
        let (accumulator, phase, interval) = if small {
            (
                &mut state.small_accumulator,
                &mut state.small_spawn_phase,
                SMALL_BALL_SPAWN_INTERVAL,
            )
        } else {
            (
                &mut state.accumulator,
                &mut state.spawn_phase,
                BALL_SPAWN_INTERVAL,
            )
        };
        *accumulator += dt;
        let mut count = 0u32;
        while *accumulator >= interval {
            *accumulator -= interval;
            count += 1;
        }
        let count = count.min(budget);
        let phase_start = *phase;
        *phase = phase.wrapping_add(count);
        (count, phase_start)
    };
    if spawn_count == 0 {
        return 0;
    }

    let Some((cursor_x, cursor_y)) = world
        .get_resource::<InputState>()
        .and_then(InputState::cursor_position)
    else {
        return 0;
    };
    let cursor = Vec2::new(cursor_x, cursor_y);
    let Some(camera) = world.get_resource::<CameraState>().copied() else {
        return 0;
    };
    let Some(world_pos) = cursor_to_world(cursor, &camera) else {
        return 0;
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
            if small {
                cmds.insert_pending(ball, SmallBall::default());
            }
            cmds.insert_pending(ball, Position(world_pos + offset));
            cmds.insert_pending(ball, Velocity(Vec2::ZERO));
            cmds.insert_pending(
                ball,
                Collider::circle(BALL_RADIUS * if small { SMALL_BALL_SCALE } else { 1.0 }),
            );
            cmds.insert_pending(
                ball,
                RigidBody {
                    kind: BodyKind::Dynamic,
                    inv_mass: 1.0,
                    restitution: BALL_RESTITUTION,
                },
            );
            cmds.insert_pending(
                ball,
                AnimationState::new(if small {
                    SMALL_BALL_ANIMATION_ID
                } else {
                    BALL_ANIMATION_ID
                }),
            );
            cmds.insert_pending(
                ball,
                CurrentSprite(
                    if small {
                        SMALL_BALL_START_SPRITE_ID
                    } else {
                        BALL_START_SPRITE_ID
                    }
                    .into(),
                ),
            );
            // Only the glass marbles cycle through the rainbow; orbs keep their bronze.
            if small {
                cmds.insert_pending(ball, BallHue::from_seed(phase));
            }
        }
        return spawn_count;
    }
    0
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
        // Dark gas needs its own emitter; `scene_effects` steers it with the hole's.
        if let Some(cfg_id) = world
            .get_resource::<ParticleConfigRegistry>()
            .and_then(|r| r.id_for_name("ex10_black_hole_dust"))
        {
            let dust = world.spawn();
            world.insert(
                dust,
                EmitterAnchor {
                    parent: entity,
                    offset: Vec2::ZERO,
                },
            );
            world.insert(dust, Transform::from_position(world_pos));
            world.insert(dust, ParticleEmitter::new(cfg_id));
            world.insert(dust, ParticleEmitterState::default());
        }
        if let Some(active) = world.get_resource_mut::<ActiveBlackHole>() {
            active.0 = Some(entity);
        }
        let handle_and_vol = world
            .get_resource::<AudioState>()
            .map(|s| (s.black_hole_sfx_handle, s.black_hole_sfx_volume));
        if let Some((handle, vol)) = handle_and_vol
            && let Some(cmds) = world.get_resource_mut::<AudioCommands>()
        {
            cmds.play_with(handle, vol, false);
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

    let holes = black_hole_positions(world);
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

        let accel = black_hole_acceleration(&holes, pos.0);
        if accel == Vec2::ZERO {
            continue;
        }
        if let Some(vel) = world.get_mut::<Velocity>(entity) {
            vel.0 += accel * dt;
        }
    }
}

pub(crate) fn black_hole_positions(world: &World) -> Vec<Vec2> {
    world
        .query::<BlackHole>()
        .filter_map(|(entity, _)| world.get::<Position>(entity).map(|p| p.0))
        .collect()
}

/// Summed pull of every hole on a point, falling linearly to zero at the radius.
pub(crate) fn black_hole_acceleration(holes: &[Vec2], position: Vec2) -> Vec2 {
    let mut accel = Vec2::ZERO;
    for &hole_pos in holes {
        let delta = hole_pos - position;
        let dist_sq = delta.length_squared();
        if !(1.0e-4..BLACK_HOLE_RADIUS * BLACK_HOLE_RADIUS).contains(&dist_sq) {
            continue;
        }
        let dist = dist_sq.sqrt();
        let falloff = 1.0 - (dist / BLACK_HOLE_RADIUS);
        let dir = delta / dist;
        accel += dir * BLACK_HOLE_FORCE * falloff;
    }
    accel
}

/// Holes put out burning small balls inside their radius. The balls stay spent,
/// as after a normal burnout; a few sampled balls puff steam.
pub(crate) fn black_hole_extinguish_system(world: &mut World) {
    let dt = world
        .get_resource::<DeltaTime>()
        .map_or(0.0, DeltaTime::seconds);
    if let Some(sounds) = world.get_resource_mut::<EffectSounds>() {
        sounds.extinguish_cooldown = (sounds.extinguish_cooldown - dt).max(0.0);
    }
    let holes = black_hole_positions(world);
    if holes.is_empty() {
        return;
    }
    let mut doused: Vec<(Entity, Vec2)> = world
        .query::<BallBurn>()
        .filter(|(_, burn)| burn.remaining > 0.0)
        .filter_map(|(e, _)| world.get::<Position>(e).map(|p| (e, p.0)))
        .filter(|(_, p)| {
            holes
                .iter()
                .any(|h| h.distance_squared(*p) < BLACK_HOLE_RADIUS * BLACK_HOLE_RADIUS)
        })
        .collect();
    if doused.is_empty() {
        return;
    }
    doused.sort_by_key(|(e, _)| e.id());
    for &(e, _) in &doused {
        if let Some(burn) = world.get_mut::<BallBurn>(e) {
            burn.remaining = 0.0;
        }
    }
    // Spread the puffs evenly over the set so a doused pile steams across its width.
    let stride = doused.len().div_ceil(EXTINGUISH_BURSTS_PER_FRAME);
    for &(_, position) in doused.iter().step_by(stride) {
        spawn_transient_effect(world, "ex10_extinguish", position);
    }
    if world
        .get_resource::<EffectSounds>()
        .is_some_and(|s| s.extinguish_cooldown <= 0.0)
    {
        play_effect_sound(world, |s| s.extinguish);
        if let Some(sounds) = world.get_resource_mut::<EffectSounds>() {
            sounds.extinguish_cooldown = EXTINGUISH_SFX_INTERVAL;
        }
    }
}

pub(crate) fn black_hole_lifetime_system(world: &mut World) {
    let dt = world
        .get_resource::<DeltaTime>()
        .map_or(0.0, DeltaTime::seconds);
    // Anchored emitters leave with their parent (black-hole dust, missile drips).
    let orphans: Vec<Entity> = world
        .query::<EmitterAnchor>()
        .filter(|(_, anchor)| !world.is_alive(anchor.parent))
        .map(|(e, _)| e)
        .collect();
    for entity in orphans {
        world.despawn(entity);
    }

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
        if active_expired && let Some(active) = world.get_resource_mut::<ActiveBlackHole>() {
            active.0 = None;
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

    if !escaped_balls.is_empty()
        && let Some(cmds) = world.get_resource_mut::<CommandBuffer>()
    {
        for entity in escaped_balls {
            cmds.despawn(entity);
        }
    }

    let escaped_players: Vec<Entity> = world
        .query::<Player>()
        .filter_map(|(entity, _)| {
            let pos = world.get::<Position>(entity)?.0;
            let collider = world.get::<Collider>(entity).copied();
            (pos.y > KILL_Y || is_body_out_of_bounds(pos, collider)).then_some(entity)
        })
        .collect();

    for entity in escaped_players {
        respawn_player(world, entity);
    }
}

pub(crate) fn respawn_player(world: &mut World, entity: Entity) {
    world.insert(
        entity,
        crate::gameplay::Health {
            immunity: 1.2,
            ..Default::default()
        },
    );
    world.insert(entity, crate::gameplay::PreviousPosition(PLAYER_SPAWN));
    world.remove_component::<tungsten::core::Tween>(entity);
    world.insert(entity, tungsten::core::UniformOverrideBlock::default());
    world.insert(entity, Player::default());
    world.insert(entity, PlayerPresentation::default());
    world.insert(
        entity,
        AnimationState::new(crate::state::PLAYER_ANIMATION_ID),
    );
    world.insert(entity, CurrentSprite(PLAYER_START_SPRITE_ID.into()));
    world.remove_component::<tungsten::core::SquashStretchState>(entity);
    if let Some(transform) = world.get_mut::<Transform>(entity) {
        transform.scale = Vec2::ONE;
    }

    if let Some(pos) = world.get_mut::<Position>(entity) {
        pos.0 = PLAYER_SPAWN;
    }
    if let Some(vel) = world.get_mut::<Velocity>(entity) {
        vel.0 = Vec2::ZERO;
    }
    tungsten::physics::wake(world, entity);
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
        .query::<Player>()
        .next()
        .and_then(|(entity, _)| world.get::<Position>(entity))
        .map_or(PLAYER_SPAWN, |p| p.0);
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

pub(crate) fn hsv_to_rgb(hue: f32, saturation: f32, value: f32) -> glam::Vec3 {
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
    let map_h = CAMERA_ROWS * TILE;
    let base_zoom = (window.height as f32 / map_h).max(f32::EPSILON);
    if let Some(camera) = world.get_resource_mut::<CameraState>() {
        camera.zoom = base_zoom;
    }
}
