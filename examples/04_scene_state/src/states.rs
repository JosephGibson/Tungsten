//! States for example 04: `MainMenuState`, `GameplayState`, `PauseState`.
//!
//! Each state is visualised by a sprite quad so the default sprite extract
//! path renders the scene without custom extract plumbing.

use std::path::PathBuf;

use glam::Vec2;

use tungsten::asset_loader;
use tungsten::core::{
    ActionMap, CommandBuffer, InputState, SceneData, Sprite, Tag, Transform, Visibility,
};
use tungsten::{GameState, SceneEntity, StateContext, StateId, StateStack};

const QUAD_ID: &str = "ex04_quad";

#[derive(Default)]
pub struct MainMenuState;

impl GameState for MainMenuState {
    fn id(&self) -> StateId {
        "menu"
    }

    fn on_enter(&mut self, ctx: &mut StateContext) {
        let buf = ctx
            .world
            .get_resource_mut::<CommandBuffer>()
            .expect("CommandBuffer resource missing");
        let e = buf.spawn();
        buf.insert_pending(e, Transform::from_position(Vec2::ZERO));
        buf.insert_pending(
            e,
            Sprite {
                asset_id: QUAD_ID.into(),
                color: [200, 80, 80, 255],
                z_order: 0,
            },
        );
        buf.insert_pending(e, Visibility { visible: true });
        buf.insert_pending(e, Tag::new("menu_marker"));
        buf.insert_pending(e, SceneEntity { state_id: "menu" });
    }

    fn on_exit(&mut self, _ctx: &mut StateContext) {}

    fn update(&mut self, world: &mut tungsten::core::World) {
        if action_just_pressed(world, "state_start") {
            if let Some(stack) = world.get_resource_mut::<StateStack>() {
                stack.request_replace(GameplayState::new(
                    "examples/04_scene_state/assets/scene.json",
                ));
            }
        }
    }
}

pub struct GameplayState {
    scene_path: PathBuf,
}

impl GameplayState {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            scene_path: path.into(),
        }
    }
}

impl GameState for GameplayState {
    fn id(&self) -> StateId {
        "gameplay"
    }

    fn on_enter(&mut self, ctx: &mut StateContext) {
        let scene = SceneData::load(&self.scene_path).expect("scene.json missing or invalid");
        asset_loader::spawn_scene(ctx.world, &scene, "gameplay");
    }

    fn on_exit(&mut self, _ctx: &mut StateContext) {}

    fn update(&mut self, world: &mut tungsten::core::World) {
        if action_just_pressed(world, "state_pause") {
            if let Some(stack) = world.get_resource_mut::<StateStack>() {
                stack.request_push(PauseState);
            }
        } else if action_just_pressed(world, "state_back") {
            if let Some(stack) = world.get_resource_mut::<StateStack>() {
                stack.request_replace(MainMenuState);
            }
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
        let buf = ctx
            .world
            .get_resource_mut::<CommandBuffer>()
            .expect("CommandBuffer resource missing");
        let e = buf.spawn();
        buf.insert_pending(e, Transform::from_position(Vec2::new(0.0, -40.0)));
        buf.insert_pending(
            e,
            Sprite {
                asset_id: QUAD_ID.into(),
                color: [40, 40, 40, 200],
                z_order: 100,
            },
        );
        buf.insert_pending(e, Visibility { visible: true });
        buf.insert_pending(e, Tag::new("pause_overlay"));
        buf.insert_pending(e, SceneEntity { state_id: "pause" });
    }

    fn on_exit(&mut self, _ctx: &mut StateContext) {}

    fn update(&mut self, world: &mut tungsten::core::World) {
        if action_just_pressed(world, "state_pause") {
            if let Some(stack) = world.get_resource_mut::<StateStack>() {
                stack.request_pop();
            }
        }
    }
}

fn action_just_pressed(world: &tungsten::core::World, action: &str) -> bool {
    let Some(input) = world.get_resource::<InputState>() else {
        return false;
    };
    let Some(actions) = world.get_resource::<ActionMap>() else {
        return false;
    };
    actions.just_pressed(input, action)
}
