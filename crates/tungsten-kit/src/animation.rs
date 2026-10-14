//! The animated sprite: [`AnimationPlugin`] drives each entity's
//! `AnimationState` and shows its current frame through the entity's
//! `Sprite`, which the default extract draws.

use serde::Deserialize;
use tungsten_core::{
    AnimationRegistry, AnimationState, Plugin, Schedule, SpawnContext, Sprite, Stage, Time, World,
    system,
};

/// [`AnimationPlugin`]'s system, in `PostUpdate`.
pub const ANIMATION_TICK: &str = "animation_tick";

/// The scene component the plugin registers.
const ANIMATION: &str = "animation";

/// The animated sprite. Each frame its system, [`ANIMATION_TICK`], advances
/// every entity that has an [`AnimationState`] and a [`Sprite`] by the game
/// clock's dt ([`Time::delta`]: a pause holds it, the time scale stretches
/// it) against the [`AnimationRegistry`] resource, then writes the current
/// frame's sprite into `Sprite.asset_id` when the two differ. So the first
/// frame shows from the entity's first frame, and a clip the game swaps in
/// shows on the next. Without a registry the system does nothing, and an
/// unknown animation ID leaves the sprite as it is.
///
/// The plugin also registers the `animation` scene component,
/// `{ "id": "<animation id>" }`, which spawns `AnimationState::new(id)`; an
/// unknown key fails the scene.
///
/// # Examples
///
/// ```
/// use tungsten_core::assets::AnimationFrame;
/// use tungsten_core::{
///     AnimationData, AnimationRegistry, AnimationState, Plugin, Schedule, Sprite,
///     SpriteAssetId, Stage, Time, World,
/// };
/// use tungsten_kit::AnimationPlugin;
///
/// let (open, shut) = (SpriteAssetId::new(0), SpriteAssetId::new(1));
/// let mut clips = AnimationRegistry::new();
/// clips.insert(
///     "blink".to_string(),
///     AnimationData {
///         looping: true,
///         frames: vec![
///             AnimationFrame { sprite: open, duration_ms: 100 },
///             AnimationFrame { sprite: shut, duration_ms: 100 },
///         ],
///     },
/// );
/// let mut world = World::new();
/// world.insert_resource(clips);
/// world.insert_resource(Time::new());
/// let mut schedule = Schedule::new();
/// AnimationPlugin.build(&mut schedule, &mut world);
/// schedule.resolve().unwrap();
///
/// let eye = world.spawn_with((AnimationState::new("blink"), Sprite::new(shut)));
/// // The first frame shows at once, the second 0.1 s of game time later.
/// for shown in [open, shut] {
///     schedule.run_stage(Stage::PostUpdate, &mut world, |_, _| {});
///     assert_eq!(world.get::<Sprite>(eye).unwrap().asset_id, shown);
///     world.resource_mut::<Time>().advance_frame(0.1);
/// }
/// ```
#[derive(Debug, Default, Clone, Copy)]
pub struct AnimationPlugin;

impl Plugin for AnimationPlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        world.register_component_with(ANIMATION, |def: AnimationDef, _: &SpawnContext<'_>| {
            (AnimationState::new(def.id),)
        });
        schedule.add(Stage::PostUpdate, system(ANIMATION_TICK, animation_tick));
    }
}

/// `components.animation` as a scene writes it.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AnimationDef {
    id: String,
}

/// Advances every animated sprite and shows its current frame. The registry
/// leaves the World for the pass and goes back after it, never cloned.
fn animation_tick(world: &mut World) {
    let dt_ms = world.get_resource::<Time>().map_or(0.0, Time::delta) * 1000.0;
    let Some(registry) = world.remove_resource::<AnimationRegistry>() else {
        return;
    };
    for (state, sprite) in world.query_mut::<(&mut AnimationState, &mut Sprite)>() {
        state.advance(dt_ms, &registry);
        if let Some(current) = state.current_sprite(&registry)
            && current != sprite.asset_id
        {
            sprite.asset_id = current;
        }
    }
    world.insert_resource(registry);
}

#[cfg(test)]
#[path = "tests/animation.rs"]
mod tests;
