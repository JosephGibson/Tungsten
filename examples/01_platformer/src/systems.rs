use tungsten::core::{
    AnimationRegistry, AnimationState, AudioCommands, CameraController, CameraState, DeltaTime,
    DisplayMode, DisplayState, EventQueue, InputState, KeyCode, World,
};
use tungsten::physics::{CollisionEvent, Velocity};
use tungsten::{request_display_settings, WindowSize};

use crate::state::{
    AudioState, CurrentSprite, Player, TextDisplayState, MAP_ROWS, PLAYER_JUMP_IMPULSE,
    PLAYER_MOVE_SPEED, TEXT_UPDATE_INTERVAL, TILE,
};

/// Horizontal movement + jump. Plays the jump SFX when the player leaves the
/// ground. Runs BEFORE `physics_step` so velocity changes are integrated in
/// the same frame.
pub(crate) fn player_input(world: &mut World) {
    let (pressed_left, pressed_right, pressed_space);
    {
        let input = match world.get_resource::<InputState>() {
            Some(i) => i,
            None => return,
        };
        pressed_left = input.is_pressed(KeyCode::ArrowLeft) || input.is_pressed(KeyCode::KeyA);
        pressed_right = input.is_pressed(KeyCode::ArrowRight) || input.is_pressed(KeyCode::KeyD);
        pressed_space = input.is_pressed(KeyCode::Space);
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
        let input = match world.get_resource::<InputState>() {
            Some(i) => i,
            None => return,
        };
        just_m = input.just_pressed(KeyCode::KeyM);
        just_1 = input.just_pressed(KeyCode::Digit1);
        just_2 = input.just_pressed(KeyCode::Digit2);
        just_3 = input.just_pressed(KeyCode::Digit3);
        just_s = input.just_pressed(KeyCode::KeyS);
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
    let (just_equal, just_minus);
    {
        let input = match world.get_resource::<InputState>() {
            Some(i) => i,
            None => return,
        };
        just_equal = input.just_pressed(KeyCode::Equal);
        just_minus = input.just_pressed(KeyCode::Minus);
    }
    if !just_equal && !just_minus {
        return;
    }
    if let Some(controller) = world.get_resource_mut::<CameraController>() {
        if just_equal {
            controller.zoom_multiplier = (controller.zoom_multiplier + 0.25).min(2.0);
        }
        if just_minus {
            controller.zoom_multiplier = (controller.zoom_multiplier - 0.25).max(0.5);
        }
    }
}

/// F11 toggles windowed/borderless fullscreen. F9 toggles vsync and clears
/// `present_mode` so the renderer re-resolves from the new vsync intent.
pub(crate) fn display_input_system(world: &mut World) {
    let (just_f9, just_f11);
    {
        let input = match world.get_resource::<InputState>() {
            Some(i) => i,
            None => return,
        };
        just_f9 = input.just_pressed(KeyCode::F9);
        just_f11 = input.just_pressed(KeyCode::F11);
    }

    if !just_f9 && !just_f11 {
        return;
    }

    let current = world
        .get_resource::<DisplayState>()
        .copied()
        .unwrap_or_default();
    let mut next = current;

    if just_f11 {
        next.display_mode = match current.display_mode {
            DisplayMode::Windowed => DisplayMode::BorderlessFullscreen,
            DisplayMode::BorderlessFullscreen | DisplayMode::ExclusiveFullscreen => {
                DisplayMode::Windowed
            }
        };
    }

    if just_f9 {
        next.vsync = !current.vsync;
        next.present_mode = None;
    }

    if let Err(err) = request_display_settings(world, next) {
        log::error!("Display request rejected: {err}");
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
