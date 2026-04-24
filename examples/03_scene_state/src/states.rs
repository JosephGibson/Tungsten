//! Example 03 states: menu, gameplay, pause.
//!
//! Pause uses `push/on_pause`; gameplay scene persists under top-state gate.
//! Gameplay fades a black overlay in on enter and defers a state replace until the
//! reverse tween emits `TweenComplete { tag: "state_exit" }`.

use std::path::Path;

use glam::Vec2;

use tungsten::core::{ActionMap, InputState, SceneData, World};
use tungsten::core::{
    CommandBuffer, EventQueue, Sprite, Tag, Transform, Tween, TweenChannel, TweenComplete,
    Visibility,
};
use tungsten::{asset_loader, GameState, SceneEntity, StateContext, StateId, StateStack};

use crate::{QUAD_ID, SPRITE_HALF, VIEW_CENTER};

const SCENE_PATH: &str = "examples/03_scene_state/assets/scene.json";
const MENU_DECORATION_COUNT: usize = 16;
const MENU_DECORATION_RADIUS: f32 = 300.0;
const MENU_DECORATION_SCALE: f32 = 1.5;
const FADE_OVERLAY_TAG: &str = "fade_overlay";
const FADE_IN_DURATION: f32 = 0.45;
const FADE_OUT_DURATION: f32 = 0.35;
const FADE_OUT_TAG: &str = "state_exit";

/// `StateStack` mutation waits for the matching `TweenComplete { tag: "state_exit" }`.
#[derive(Debug, Clone, Copy, Default)]
pub struct PendingTransition(pub Option<TransitionTarget>);

#[derive(Debug, Clone, Copy)]
pub enum TransitionTarget {
    ReplaceWithMenu,
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
        if action_just_pressed(world, "state_start") {
            if let Some(stack) = world.get_resource_mut::<StateStack>() {
                stack.request_replace(GameplayState::new(SCENE_PATH));
            }
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
}

impl GameState for GameplayState {
    fn id(&self) -> StateId {
        "gameplay"
    }

    fn on_enter(&mut self, ctx: &mut StateContext) {
        if let Some(clock) = ctx.world.get_resource_mut::<crate::GameplayClock>() {
            clock.0 = 0.0;
        }
        if let Some(pending) = ctx.world.get_resource_mut::<PendingTransition>() {
            pending.0 = None;
        } else {
            ctx.world.insert_resource(PendingTransition::default());
        }
        let scene =
            SceneData::load(Path::new(self.scene_path)).expect("scene.json missing or invalid");
        asset_loader::spawn_scene(ctx.world, &scene, "gameplay");
        spawn_fade_overlay(ctx.world, "gameplay");
    }

    fn on_exit(&mut self, _ctx: &mut StateContext) {}

    fn on_pause(&mut self, _ctx: &mut StateContext) {}

    fn on_resume(&mut self, _ctx: &mut StateContext) {}

    fn update(&mut self, world: &mut World) {
        if action_just_pressed(world, "state_pause") {
            if let Some(stack) = world.get_resource_mut::<StateStack>() {
                stack.request_push(PauseState);
            }
        } else if action_just_pressed(world, "state_back") {
            let already_pending = world
                .get_resource::<PendingTransition>()
                .is_some_and(|p| p.0.is_some());
            if already_pending {
                return;
            }
            start_fade_out(world);
            if let Some(pending) = world.get_resource_mut::<PendingTransition>() {
                pending.0 = Some(TransitionTarget::ReplaceWithMenu);
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
        spawn_pause_overlay(ctx.world);
    }

    fn on_exit(&mut self, _ctx: &mut StateContext) {}

    fn update(&mut self, world: &mut World) {
        if action_just_pressed(world, "state_pause") {
            if let Some(stack) = world.get_resource_mut::<StateStack>() {
                stack.request_pop();
            }
        } else if action_just_pressed(world, "state_back") {
            if let Some(stack) = world.get_resource_mut::<StateStack>() {
                stack.request_replace(MainMenuState);
            }
        }
    }
}

/// Reads both `EventQueue` windows (D-040) — the completion lands in `previous`
/// by the frame this runs.
pub fn handle_tween_complete_system(world: &mut World) {
    let Some(queue) = world.get_resource::<EventQueue<TweenComplete>>() else {
        return;
    };
    let fired = queue
        .iter()
        .any(|ev| ev.tag.as_deref() == Some(FADE_OUT_TAG));
    if !fired {
        return;
    }
    let Some(pending) = world.get_resource_mut::<PendingTransition>() else {
        return;
    };
    let Some(target) = pending.0.take() else {
        return;
    };
    match target {
        TransitionTarget::ReplaceWithMenu => {
            if let Some(stack) = world.get_resource_mut::<StateStack>() {
                stack.request_replace(MainMenuState);
            }
        }
    }
}

fn spawn_menu_decorations(world: &mut World) {
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
                asset_id: QUAD_ID.into(),
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
            asset_id: QUAD_ID.into(),
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
            asset_id: QUAD_ID.into(),
            color: [28, 36, 60, 220],
            z_order: 510,
            material_id: None,
        },
    );
    buf.insert_pending(banner, Visibility { visible: true });
    buf.insert_pending(banner, Tag::new("pause_banner"));
    buf.insert_pending(banner, SceneEntity { state_id: "pause" });
}

fn spawn_fade_overlay(world: &mut World, state_id: StateId) {
    let buf = world
        .get_resource_mut::<CommandBuffer>()
        .expect("CommandBuffer resource missing");

    let overlay = buf.spawn();
    buf.insert_pending(
        overlay,
        Transform {
            position: Vec2::ZERO,
            rotation: 0.0,
            scale: Vec2::new(160.0, 90.0),
        },
    );
    buf.insert_pending(
        overlay,
        Sprite {
            asset_id: QUAD_ID.into(),
            color: [0, 0, 0, 255],
            z_order: 900,
            material_id: None,
        },
    );
    buf.insert_pending(overlay, Visibility { visible: true });
    buf.insert_pending(overlay, Tag::new(FADE_OVERLAY_TAG));
    buf.insert_pending(overlay, SceneEntity { state_id });
    buf.insert_pending(
        overlay,
        Tween::new(FADE_IN_DURATION, tungsten::core::Easing::CubicOut)
            .with_channel(TweenChannel::ColorA { from: 255, to: 0 }),
    );
}

fn start_fade_out(world: &mut World) {
    let entities = world.query2_entities::<Tag, Sprite>();
    let Some(overlay) = entities.into_iter().find(|e| {
        world
            .get::<Tag>(*e)
            .is_some_and(|t| t.name == FADE_OVERLAY_TAG)
    }) else {
        return;
    };
    let starting_alpha = world.get::<Sprite>(overlay).map_or(0, |s| s.color[3]);
    if let Some(buf) = world.get_resource_mut::<CommandBuffer>() {
        buf.remove_component::<Tween>(overlay);
        buf.insert(
            overlay,
            Tween::new(FADE_OUT_DURATION, tungsten::core::Easing::CubicIn)
                .with_channel(TweenChannel::ColorA {
                    from: starting_alpha,
                    to: 255,
                })
                .with_tag(FADE_OUT_TAG),
        );
    }
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
