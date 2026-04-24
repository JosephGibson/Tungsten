//! M24 tween tick. Slot: `particles → tweens → flush commands → flush events` (D-039, D-040).
//! Writes `Transform`/`Sprite` in-place; completion + removal buffered so the archetype is
//! never mutated mid-iteration.
//!
//! M26 also dispatches `UniformVec4Lane`/`UniformScalar`/`UniformInt` into
//! `UniformOverrideBlock`; missing-block channels log and are skipped so a
//! tween authored for an absent component never panics.

use tungsten_core::tween::UniformOverrideBlock;
use tungsten_core::{
    lerp_f32, lerp_u8, CommandBuffer, DeltaTime, Entity, EventQueue, Sprite, Transform, Tween,
    TweenChannel, TweenComplete, TweenDirection, TweenRepeat, World,
};

pub fn tween_tick_system(world: &mut World) {
    let dt = world
        .get_resource::<DeltaTime>()
        .map_or(0.0, DeltaTime::seconds);
    if dt <= 0.0 {
        return;
    }

    let entities = world.query_entities::<Tween>();
    let mut completed: Vec<TweenComplete> = Vec::new();
    let mut to_remove: Vec<Entity> = Vec::new();

    for entity in entities {
        let Some((
            duration,
            elapsed,
            easing,
            direction,
            repeat,
            completed_cycles,
            channels,
            tag,
            pending,
        )) = world.get::<Tween>(entity).map(|t| {
            (
                t.duration,
                t.elapsed,
                t.easing,
                t.direction,
                t.repeat,
                t.completed_cycles,
                t.channels.clone(),
                t.on_complete_tag.clone(),
                t.pending_remove,
            )
        })
        else {
            continue;
        };

        // Waiting for command-buffer flush to drop the component.
        if pending {
            continue;
        }

        let signed_dt = if direction == TweenDirection::Backward {
            -dt
        } else {
            dt
        };
        let new_elapsed = (elapsed + signed_dt).clamp(0.0, duration);
        let u = (new_elapsed / duration).clamp(0.0, 1.0);
        let k = easing.apply(u);

        apply_channels(world, entity, &channels, k);

        if let Some(t) = world.get_mut::<Tween>(entity) {
            t.elapsed = new_elapsed;
        }

        let forward_done = direction == TweenDirection::Forward && new_elapsed >= duration;
        let backward_done = direction == TweenDirection::Backward && new_elapsed <= 0.0;
        if !(forward_done || backward_done) {
            continue;
        }

        match repeat {
            TweenRepeat::Once => {
                completed.push(TweenComplete { entity, tag });
                to_remove.push(entity);
                if let Some(t) = world.get_mut::<Tween>(entity) {
                    t.pending_remove = true;
                }
            }
            TweenRepeat::Times(n) => {
                let next_cycles = completed_cycles.saturating_add(1);
                if next_cycles >= n {
                    completed.push(TweenComplete { entity, tag });
                    to_remove.push(entity);
                    if let Some(t) = world.get_mut::<Tween>(entity) {
                        t.pending_remove = true;
                    }
                } else if let Some(t) = world.get_mut::<Tween>(entity) {
                    t.completed_cycles = next_cycles;
                    t.elapsed = 0.0;
                }
            }
            TweenRepeat::Loop => {
                if let Some(t) = world.get_mut::<Tween>(entity) {
                    t.elapsed = 0.0;
                }
            }
            TweenRepeat::PingPong => {
                if let Some(t) = world.get_mut::<Tween>(entity) {
                    t.direction = if direction == TweenDirection::Forward {
                        TweenDirection::Backward
                    } else {
                        TweenDirection::Forward
                    };
                    t.elapsed = if direction == TweenDirection::Forward {
                        duration
                    } else {
                        0.0
                    };
                }
            }
        }
    }

    if !completed.is_empty() {
        if let Some(q) = world.get_resource_mut::<EventQueue<TweenComplete>>() {
            for ev in completed {
                q.send(ev);
            }
        }
    }

    if !to_remove.is_empty() {
        if let Some(buf) = world.get_resource_mut::<CommandBuffer>() {
            for entity in to_remove {
                buf.remove_component::<Tween>(entity);
            }
        }
    }
}

fn apply_channels(world: &mut World, entity: Entity, channels: &[TweenChannel], k: f32) {
    for channel in channels {
        match channel {
            TweenChannel::PositionX { from, to } => {
                if let Some(t) = world.get_mut::<Transform>(entity) {
                    t.position.x = lerp_f32(*from, *to, k);
                }
            }
            TweenChannel::PositionY { from, to } => {
                if let Some(t) = world.get_mut::<Transform>(entity) {
                    t.position.y = lerp_f32(*from, *to, k);
                }
            }
            TweenChannel::Rotation { from, to } => {
                if let Some(t) = world.get_mut::<Transform>(entity) {
                    t.rotation = lerp_f32(*from, *to, k);
                }
            }
            TweenChannel::ScaleX { from, to } => {
                if let Some(t) = world.get_mut::<Transform>(entity) {
                    t.scale.x = lerp_f32(*from, *to, k);
                }
            }
            TweenChannel::ScaleY { from, to } => {
                if let Some(t) = world.get_mut::<Transform>(entity) {
                    t.scale.y = lerp_f32(*from, *to, k);
                }
            }
            TweenChannel::ColorR { from, to } => {
                if let Some(s) = world.get_mut::<Sprite>(entity) {
                    s.color[0] = lerp_u8(*from, *to, k);
                }
            }
            TweenChannel::ColorG { from, to } => {
                if let Some(s) = world.get_mut::<Sprite>(entity) {
                    s.color[1] = lerp_u8(*from, *to, k);
                }
            }
            TweenChannel::ColorB { from, to } => {
                if let Some(s) = world.get_mut::<Sprite>(entity) {
                    s.color[2] = lerp_u8(*from, *to, k);
                }
            }
            TweenChannel::ColorA { from, to } => {
                if let Some(s) = world.get_mut::<Sprite>(entity) {
                    s.color[3] = lerp_u8(*from, *to, k);
                }
            }
            TweenChannel::UniformVec4Lane {
                slot,
                lane,
                from,
                to,
            } => {
                let lane = (*lane as usize).min(3);
                if let Some(block) = world.get_mut::<UniformOverrideBlock>(entity) {
                    block.vec4[slot.index()][lane] = lerp_f32(*from, *to, k);
                } else {
                    log::debug!(
                        "tween UniformVec4Lane on entity {entity:?}: no UniformOverrideBlock"
                    );
                }
            }
            TweenChannel::UniformScalar { slot, from, to } => {
                if let Some(block) = world.get_mut::<UniformOverrideBlock>(entity) {
                    block.f32s[slot.index()] = lerp_f32(*from, *to, k);
                } else {
                    log::debug!(
                        "tween UniformScalar on entity {entity:?}: no UniformOverrideBlock"
                    );
                }
            }
            TweenChannel::UniformInt { slot, from, to } => {
                let value = if k >= 0.5 { *to } else { *from };
                if let Some(block) = world.get_mut::<UniformOverrideBlock>(entity) {
                    block.i32s[slot.index()] = value;
                } else {
                    log::debug!("tween UniformInt on entity {entity:?}: no UniformOverrideBlock");
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "tests/tweens.rs"]
mod tests;
