//! M30 game-feel systems: camera trauma shake and squash/stretch envelopes
//! (`D-073`). Examples own frame placement, exactly as `camera_update_system`
//! does — the engine cannot know where in a game's order these belong.
//!
//! Required order, since all three read the current event window only:
//!
//! 1. the gameplay systems that send `ShakeEvent` / `SquashEvent`, then
//! 2. `squash_stretch_trigger_system`, then `squash_stretch_tick_system`, and
//! 3. `shake_tick_system` before `camera_update_system`.
//!
//! Both squash systems follow the `tween_tick_system` pattern: structural work
//! is buffered through `CommandBuffer` (`D-039`) so the archetype is never
//! mutated mid-iteration.

use glam::Vec2;
use tungsten_core::{
    CameraController, CommandBuffer, Entity, EventQueue, ShakeEvent, SpriteSquashStretch,
    SquashEvent, SquashStretchState, Time, Transform, World,
};

/// Drains `EventQueue<ShakeEvent>` into `CameraController::shake_trauma`, then
/// decays trauma by `shake_decay * dt`, floored at `0.0`.
///
/// Reads the current event window only, so it must run after the gameplay
/// systems that send `ShakeEvent` and before `camera_update_system`, so the
/// frame's trauma reaches that frame's `CameraState`.
pub fn shake_tick_system(world: &mut World) {
    let dt = world.get_resource::<Time>().map_or(0.0, Time::delta);

    // Only the current window: `iter()` would re-apply the previous frame's
    // trauma every frame.
    let trauma_add: f32 = world
        .get_resource::<EventQueue<ShakeEvent>>()
        .map_or(0.0, |q| q.iter_current().map(|ev| ev.trauma_add).sum());

    let Some(controller) = world.get_resource_mut::<CameraController>() else {
        return;
    };
    if trauma_add != 0.0 {
        controller.add_trauma(trauma_add);
    }
    let decayed = controller.shake_trauma - controller.shake_decay.max(0.0) * dt;
    controller.shake_trauma = decayed.clamp(0.0, 1.0);
}

/// Matches `EventQueue<SquashEvent>` against each target's
/// `SpriteSquashStretch.on` and arms the envelope by inserting
/// `SquashStretchState` with the entity's live `Transform.scale` as
/// `base_scale`.
///
/// Reads the current event window only, so it must be registered after the
/// gameplay systems that send `SquashEvent`, and before
/// `squash_stretch_tick_system`.
pub fn squash_stretch_trigger_system(world: &mut World) {
    let requests: Vec<SquashEvent> = world
        .get_resource::<EventQueue<SquashEvent>>()
        .map(|q| q.iter_current().copied().collect())
        .unwrap_or_default();
    if requests.is_empty() {
        return;
    }

    let mut armed: Vec<(Entity, SquashStretchState)> = Vec::new();
    for request in requests {
        let Some(squash) = world.get::<SpriteSquashStretch>(request.entity).copied() else {
            continue;
        };
        if squash.on != request.trigger {
            continue;
        }
        // An in-flight envelope already owns the authored scale, so restart it
        // in place: re-reading `Transform` would bake the squashed scale in as
        // the new base, and buffering a fresh insert would be undone by the
        // removal `squash_stretch_tick_system` buffers on the frame the
        // envelope completes (`World::flush` applies commands in queue order).
        if let Some(state) = world.get_mut::<SquashStretchState>(request.entity) {
            state.elapsed = 0.0;
            continue;
        }
        let Some(base_scale) = world.get::<Transform>(request.entity).map(|t| t.scale) else {
            continue;
        };
        armed.push((
            request.entity,
            SquashStretchState {
                elapsed: 0.0,
                base_scale,
            },
        ));
    }

    if armed.is_empty() {
        return;
    }
    let Some(buf) = world.get_resource_mut::<CommandBuffer>() else {
        return;
    };
    for (entity, state) in armed {
        buf.insert(entity, state);
    }
}

/// Advances every `SquashStretchState`, writes `Transform.scale` from the
/// symmetric envelope, and removes finished states through `CommandBuffer`.
///
/// The envelope is exact at both endpoints, so completion restores
/// `base_scale` without a separate cleanup write.
pub fn squash_stretch_tick_system(world: &mut World) {
    let dt = world.get_resource::<Time>().map_or(0.0, Time::delta);
    // A frozen frame freezes the envelope, as in `tween_tick_system`.
    if dt <= 0.0 {
        return;
    }

    let mut writes: Vec<(Entity, Vec2)> = Vec::new();
    let mut finished: Vec<Entity> = Vec::new();

    for (entity, state, squash) in world.query2_mut::<SquashStretchState, SpriteSquashStretch>() {
        state.elapsed += dt;
        writes.push((entity, squash.scale_at(state.base_scale, state.elapsed)));
        if squash.duration <= 0.0 || state.elapsed >= squash.duration {
            finished.push(entity);
        }
    }

    // A state whose config was removed mid-flight is not matched above. Restore
    // its authored scale and retire it rather than freezing `Transform.scale`.
    for entity in world.query_entities::<SquashStretchState>() {
        if world.get::<SpriteSquashStretch>(entity).is_some() {
            continue;
        }
        if let Some(base_scale) = world
            .get::<SquashStretchState>(entity)
            .map(|s| s.base_scale)
        {
            writes.push((entity, base_scale));
        }
        finished.push(entity);
    }

    for (entity, scale) in writes {
        if let Some(transform) = world.get_mut::<Transform>(entity) {
            transform.scale = scale;
        }
    }

    if finished.is_empty() {
        return;
    }
    let Some(buf) = world.get_resource_mut::<CommandBuffer>() else {
        return;
    };
    for entity in finished {
        buf.remove_component::<SquashStretchState>(entity);
    }
}

#[cfg(test)]
#[path = "tests/game_feel.rs"]
mod tests;
