//! The game's states on the engine's state stack: title, then gameplay, with
//! pause pushed over gameplay. Each state reads actions, never keys.

use tungsten::core::{ActionMap, InputState, World};
use tungsten::{GameState, SceneEntity, StateContext, StateId, StateStack};

pub const TITLE: StateId = "title";
pub const GAMEPLAY: StateId = "gameplay";
pub const PAUSE: StateId = "pause";
/// The player's prefab, by its manifest ID: its sprite and `Player`.
pub const PLAYER_PREFAB: &str = "player";
/// Where the player starts: the middle of a 1280 × 720 window.
pub const PLAYER_START: [f32; 2] = [624.0, 344.0];

/// The title screen; `state_start` starts gameplay.
pub struct TitleState;

impl GameState for TitleState {
    fn id(&self) -> StateId {
        TITLE
    }

    fn on_enter(&mut self, _ctx: &mut StateContext) {}

    fn on_exit(&mut self, _ctx: &mut StateContext) {}

    fn update(&mut self, world: &mut World) {
        if just_pressed(world, "state_start") {
            request(world, |stack| stack.request_replace(GameplayState));
        }
    }
}

/// Gameplay: the player spawns with it and leaves with it; `state_pause`
/// pauses.
pub struct GameplayState;

impl GameState for GameplayState {
    fn id(&self) -> StateId {
        GAMEPLAY
    }

    fn on_enter(&mut self, ctx: &mut StateContext) {
        spawn_player(ctx.world);
    }

    fn on_exit(&mut self, _ctx: &mut StateContext) {}

    fn update(&mut self, world: &mut World) {
        if just_pressed(world, "state_pause") {
            request(world, |stack| stack.request_push(PauseState));
        }
    }
}

/// Pause, pushed over gameplay; `state_back` or `state_pause` resumes.
pub struct PauseState;

impl GameState for PauseState {
    fn id(&self) -> StateId {
        PAUSE
    }

    fn on_enter(&mut self, _ctx: &mut StateContext) {}

    fn on_exit(&mut self, _ctx: &mut StateContext) {}

    fn update(&mut self, world: &mut World) {
        if just_pressed(world, "state_back") || just_pressed(world, "state_pause") {
            request(world, StateStack::request_pop);
        }
    }
}

/// Spawns the player prefab at its start. The startup load checked the
/// prefab against the game's components, so a failure here is a bug.
fn spawn_player(world: &mut World) {
    world
        .prepare_prefab(PLAYER_PREFAB, PLAYER_START)
        .expect("the player prefab loads with the manifest")
        // The state stack despawns it when gameplay leaves the stack.
        .with((SceneEntity { state_id: GAMEPLAY },))
        .spawn(world);
}

fn just_pressed(world: &World, action: &str) -> bool {
    match (
        world.get_resource::<InputState>(),
        world.get_resource::<ActionMap>(),
    ) {
        (Some(input), Some(actions)) => actions.just_pressed(input, action),
        _ => false,
    }
}

fn request(world: &mut World, change: impl FnOnce(&mut StateStack)) {
    if let Some(stack) = world.get_resource_mut::<StateStack>() {
        change(stack);
    }
}
