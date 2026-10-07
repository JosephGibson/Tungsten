//! Example 03 states: menu, gameplay, pause.
//!
//! Pause uses `push/on_pause`; gameplay scene persists under top-state gate.
//!
//! M31 (`D-093`): state changes run behind engine screen transitions. A state
//! asks `StateStack::request_*_transition`; the command applies on the frame
//! the cover completes, and a request made while a transition runs is dropped.
//!
//! | Change | Effect |
//! | --- | --- |
//! | menu -> gameplay | fade to black |
//! | gameplay -> pause | pixelate |
//! | pause -> gameplay | radial wipe |
//! | gameplay -> menu | dissolve |
//! | pause -> menu | none: plain pop then replace, a hard cut |

use std::path::Path;

use glam::Vec2;

use tungsten::core::{ActionMap, AssetRegistry, InputState, SceneData, SpriteAssetId, World};
use tungsten::core::{CommandBuffer, Sprite, Tag, Transform, Visibility};
use tungsten::{
    GameState, SceneEntity, StateContext, StateId, StateStack, Transition, TransitionEffect,
    asset_loader,
};

use crate::{QUAD_ID, SPRITE_HALF, VIEW_CENTER};

const SCENE_PATH: &str = "examples/03_scene_state/assets/scene.json";
const MENU_DECORATION_COUNT: usize = 16;
const MENU_DECORATION_RADIUS: f32 = 300.0;
const MENU_DECORATION_SCALE: f32 = 1.5;
/// Seconds per phase of every interactive transition.
const TRANSITION_SECS: f32 = 0.35;

const FADE: TransitionEffect = TransitionEffect::Fade {
    color: [0.0, 0.0, 0.0, 1.0],
};
const PIXELATE: TransitionEffect = TransitionEffect::Pixelate { max_block_px: 48.0 };
const WIPE_RADIAL: TransitionEffect = TransitionEffect::WipeRadial {
    center: [0.5, 0.5],
    softness: 0.05,
};
const DISSOLVE: TransitionEffect = TransitionEffect::Dissolve {
    noise_scale: 8.0,
    edge_color: [1.0, 0.5, 0.0, 1.0],
};

/// The effect `TUNGSTEN_TRANSITION_FIXTURE` names.
pub(crate) fn fixture_effect(name: &str) -> Option<TransitionEffect> {
    match name {
        "fade" => Some(FADE),
        "wipe_radial" => Some(WIPE_RADIAL),
        "dissolve" => Some(DISSOLVE),
        "pixelate" => Some(PIXELATE),
        _ => None,
    }
}

fn transition(effect: TransitionEffect) -> Transition {
    Transition::new(effect, TRANSITION_SECS)
}

#[derive(Default)]
pub struct MainMenuState;

impl GameState for MainMenuState {
    fn id(&self) -> StateId {
        "menu"
    }

    fn on_enter(&mut self, ctx: &mut StateContext) {
        if let Some(clock) = ctx.world.get_resource_mut::<crate::MenuClock>() {
            clock.0 = 0.0;
        }
        spawn_menu_decorations(ctx.world);
    }

    fn on_exit(&mut self, _ctx: &mut StateContext) {}

    fn update(&mut self, world: &mut World) {
        if action_just_pressed(world, "state_start")
            && let Some(stack) = world.get_resource_mut::<StateStack>()
        {
            stack.request_replace_transition(GameplayState::default_scene(), transition(FADE));
        }
    }
}

pub struct GameplayState {
    scene_path: &'static str,
}

impl GameplayState {
    pub fn new(path: &'static str) -> Self {
        Self { scene_path: path }
    }

    /// Gameplay over the example's `scene.json` (`D-046`).
    pub fn default_scene() -> Self {
        Self::new(SCENE_PATH)
    }
}

impl GameState for GameplayState {
    fn id(&self) -> StateId {
        "gameplay"
    }

    fn on_enter(&mut self, ctx: &mut StateContext) {
        if let Some(clock) = ctx.world.get_resource_mut::<crate::GameplayClock>() {
            clock.0 = 0.0;
        }
        let scene =
            SceneData::load(Path::new(self.scene_path)).expect("scene.json missing or invalid");
        asset_loader::spawn_scene(ctx.world, &scene, "gameplay");
    }

    fn on_exit(&mut self, _ctx: &mut StateContext) {}

    fn on_pause(&mut self, _ctx: &mut StateContext) {}

    fn on_resume(&mut self, _ctx: &mut StateContext) {}

    fn update(&mut self, world: &mut World) {
        if action_just_pressed(world, "state_pause") {
            if let Some(stack) = world.get_resource_mut::<StateStack>() {
                stack.request_push_transition(PauseState, transition(PIXELATE));
            }
        } else if action_just_pressed(world, "state_back")
            && let Some(stack) = world.get_resource_mut::<StateStack>()
        {
            stack.request_replace_transition(MainMenuState, transition(DISSOLVE));
        }
    }
}

#[derive(Default)]
pub struct PauseState;

impl GameState for PauseState {
    fn id(&self) -> StateId {
        "pause"
    }

    fn on_enter(&mut self, ctx: &mut StateContext) {
        spawn_pause_overlay(ctx.world);
    }

    fn on_exit(&mut self, _ctx: &mut StateContext) {}

    fn update(&mut self, world: &mut World) {
        if action_just_pressed(world, "state_pause") {
            if let Some(stack) = world.get_resource_mut::<StateStack>() {
                stack.request_pop_transition(transition(WIPE_RADIAL));
            }
        } else if action_just_pressed(world, "state_back")
            && let Some(stack) = world.get_resource_mut::<StateStack>()
        {
            // Remove the pause overlay and its underlying gameplay state.
            stack.request_pop();
            stack.request_replace(MainMenuState);
        }
    }
}

/// The quad sprite's ID, interned before a spawn takes the `CommandBuffer`.
fn quad_id(world: &mut World) -> SpriteAssetId {
    world
        .get_resource_mut::<AssetRegistry>()
        .expect("AssetRegistry resource missing")
        .intern_sprite(QUAD_ID)
}

fn spawn_menu_decorations(world: &mut World) {
    let quad = quad_id(world);
    let buf = world
        .get_resource_mut::<CommandBuffer>()
        .expect("CommandBuffer resource missing");
    let half = Vec2::splat(MENU_DECORATION_SCALE * SPRITE_HALF);

    for i in 0..MENU_DECORATION_COUNT {
        let theta = (i as f32 / MENU_DECORATION_COUNT as f32) * std::f32::consts::TAU;
        let ring_center =
            VIEW_CENTER + Vec2::new(theta.cos(), theta.sin()) * MENU_DECORATION_RADIUS;
        let hue = i as f32 / MENU_DECORATION_COUNT as f32;
        let color = menu_palette(hue);

        let entity = buf.spawn();
        buf.insert_pending(
            entity,
            Transform {
                position: ring_center - half,
                rotation: theta,
                scale: Vec2::splat(MENU_DECORATION_SCALE),
            },
        );
        buf.insert_pending(
            entity,
            Sprite {
                asset_id: quad,
                color,
                z_order: 2,
                material_id: None,
            },
        );
        buf.insert_pending(entity, Visibility { visible: true });
        buf.insert_pending(entity, Tag::new("menu_decoration"));
        buf.insert_pending(entity, SceneEntity { state_id: "menu" });
    }
}

fn spawn_pause_overlay(world: &mut World) {
    let quad = quad_id(world);
    let buf = world
        .get_resource_mut::<CommandBuffer>()
        .expect("CommandBuffer resource missing");

    let dim = buf.spawn();
    buf.insert_pending(
        dim,
        Transform {
            position: Vec2::ZERO,
            rotation: 0.0,
            scale: Vec2::new(80.0, 45.0),
        },
    );
    buf.insert_pending(
        dim,
        Sprite {
            asset_id: quad,
            color: [6, 10, 20, 170],
            z_order: 500,
            material_id: None,
        },
    );
    buf.insert_pending(dim, Visibility { visible: true });
    buf.insert_pending(dim, Tag::new("pause_dim"));
    buf.insert_pending(dim, SceneEntity { state_id: "pause" });

    let banner = buf.spawn();
    let banner_half = Vec2::new(24.0 * SPRITE_HALF, 6.0 * SPRITE_HALF);
    buf.insert_pending(
        banner,
        Transform {
            position: VIEW_CENTER - banner_half,
            rotation: 0.0,
            scale: Vec2::new(24.0, 6.0),
        },
    );
    buf.insert_pending(
        banner,
        Sprite {
            asset_id: quad,
            color: [28, 36, 60, 220],
            z_order: 510,
            material_id: None,
        },
    );
    buf.insert_pending(banner, Visibility { visible: true });
    buf.insert_pending(banner, Tag::new("pause_banner"));
    buf.insert_pending(banner, SceneEntity { state_id: "pause" });
}

fn menu_palette(t: f32) -> [u8; 4] {
    let tau = std::f32::consts::TAU;
    let r = ((t * tau).sin() * 0.5 + 0.5) * 140.0 + 100.0;
    let g = ((t * tau + 2.1).sin() * 0.5 + 0.5) * 160.0 + 80.0;
    let b = ((t * tau + 4.2).sin() * 0.5 + 0.5) * 180.0 + 75.0;
    [r as u8, g as u8, b as u8, 240]
}

fn action_just_pressed(world: &World, action: &str) -> bool {
    let Some(input) = world.get_resource::<InputState>() else {
        return false;
    };
    let Some(actions) = world.get_resource::<ActionMap>() else {
        return false;
    };
    actions.just_pressed(input, action)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tungsten::core::Entity;
    use tungsten::state_dispatcher_system;

    struct TestGameplay;
    impl GameState for TestGameplay {
        fn id(&self) -> StateId {
            "gameplay"
        }
        fn on_enter(&mut self, ctx: &mut StateContext) {
            let entity = ctx.world.spawn();
            ctx.world.insert(
                entity,
                SceneEntity {
                    state_id: "gameplay",
                },
            );
        }
        fn on_exit(&mut self, _: &mut StateContext) {}
        fn update(&mut self, _: &mut World) {}
    }

    fn flush(world: &mut World) {
        let commands = world.remove_resource::<CommandBuffer>().unwrap();
        world.flush(commands);
        world.insert_resource(CommandBuffer::new());
    }

    #[test]
    fn back_from_pause_removes_gameplay_before_entering_menu() {
        let mut world = World::new();
        world.insert_resource(AssetRegistry::new());
        world.insert_resource(CommandBuffer::new());
        world.insert_resource(StateStack::new());
        world.insert_resource(ActionMap::default_map());
        world.insert_resource(InputState::new());
        world
            .get_resource_mut::<StateStack>()
            .unwrap()
            .request_push(TestGameplay);
        state_dispatcher_system(&mut world);
        world
            .get_resource_mut::<StateStack>()
            .unwrap()
            .request_push(PauseState);
        state_dispatcher_system(&mut world);
        flush(&mut world);
        world
            .get_resource_mut::<InputState>()
            .unwrap()
            .key_down(tungsten::core::KeyCode::Backspace);
        state_dispatcher_system(&mut world);
        world
            .get_resource_mut::<InputState>()
            .unwrap()
            .begin_frame();
        state_dispatcher_system(&mut world);
        flush(&mut world);
        let stack = world.get_resource::<StateStack>().unwrap();
        assert_eq!(stack.active_id(), Some("menu"));
        assert_eq!(stack.depth(), 1);
        assert!(
            world
                .query::<(Entity, &SceneEntity)>()
                .all(|(_, marker)| marker.state_id == "menu")
        );
    }
}
