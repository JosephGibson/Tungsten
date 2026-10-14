//! The game's states on the engine's state stack: title, then gameplay, with
//! pause pushed over gameplay. Each state reads actions, never keys.
//! Gameplay continues from the autosave, which pause writes.

use serde::{Deserialize, Serialize};
use tungsten::core::save::SaveError;
use tungsten::core::{ActionMap, InputState, SaveData, SaveSlots, Transform, With, World};
use tungsten::{GameState, SceneEntity, StateContext, StateId, StateStack};

use crate::components::Player;

pub const TITLE: StateId = "title";
pub const GAMEPLAY: StateId = "gameplay";
pub const PAUSE: StateId = "pause";
/// The player's prefab, by its manifest ID: its sprite and `Player`.
pub const PLAYER_PREFAB: &str = "player";
/// Where the player starts: the middle of a 1280 × 720 window.
pub const PLAYER_START: [f32; 2] = [624.0, 344.0];
/// The save slot gameplay continues from and pause writes.
pub const AUTOSAVE: &str = "autosave";

/// What the autosave holds: the player's position.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SaveGame {
    pub player: [f32; 2],
}

impl SaveData for SaveGame {
    /// Bump it when `SaveGame` changes shape, and write `migrate`.
    const SCHEMA: u32 = 1;
}

/// Set for the run when the autosave failed to load, so pause never
/// replaces a slot it could not read, such as one from a newer build.
struct AutosaveReadOnly;

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

/// Gameplay: the player spawns with it, where the autosave left it, and
/// leaves with it; `state_pause` pauses.
pub struct GameplayState;

impl GameState for GameplayState {
    fn id(&self) -> StateId {
        GAMEPLAY
    }

    fn on_enter(&mut self, ctx: &mut StateContext) {
        let start = load_autosave(ctx.world).map_or(PLAYER_START, |save| save.player);
        spawn_player(ctx.world, start);
    }

    fn on_exit(&mut self, _ctx: &mut StateContext) {}

    fn update(&mut self, world: &mut World) {
        if just_pressed(world, "state_pause") {
            request(world, |stack| stack.request_push(PauseState));
        }
    }
}

/// Pause, pushed over gameplay, saves the autosave; `state_back` or
/// `state_pause` resumes.
pub struct PauseState;

impl GameState for PauseState {
    fn id(&self) -> StateId {
        PAUSE
    }

    fn on_enter(&mut self, ctx: &mut StateContext) {
        write_autosave(ctx.world);
    }

    fn on_exit(&mut self, _ctx: &mut StateContext) {}

    fn update(&mut self, world: &mut World) {
        if just_pressed(world, "state_back") || just_pressed(world, "state_pause") {
            request(world, StateStack::request_pop);
        }
    }
}

/// The autosave, or `None` to start fresh: none saved, no user folder, or
/// one that failed to load, which also makes the slot read-only for the run.
fn load_autosave(world: &mut World) -> Option<SaveGame> {
    match world
        .get_resource::<SaveSlots>()?
        .load::<SaveGame>(AUTOSAVE)
    {
        Ok(save) => Some(save),
        Err(SaveError::NotFound { .. } | SaveError::NoFolder) => None,
        Err(err) => {
            log::warn!("Autosave not loaded, so it is not overwritten this run: {err}");
            world.insert_resource(AutosaveReadOnly);
            None
        }
    }
}

/// Saves the player's position, unless the autosave failed to load.
fn write_autosave(world: &mut World) {
    if world.get_resource::<AutosaveReadOnly>().is_some() {
        return;
    }
    let Some(position) = world
        .query_filtered::<&Transform, With<Player>>()
        .next()
        .map(|transform| [transform.position.x, transform.position.y])
    else {
        return;
    };
    let Some(slots) = world.get_resource::<SaveSlots>() else {
        return;
    };
    match slots.save(AUTOSAVE, &SaveGame { player: position }) {
        Ok(()) | Err(SaveError::NoFolder) => {}
        Err(err) => log::warn!("Autosave not saved: {err}"),
    }
}

/// Spawns the player prefab at `position`. The startup load checked the
/// prefab against the game's components, so a failure here is a bug.
fn spawn_player(world: &mut World, position: [f32; 2]) {
    world
        .prepare_prefab(PLAYER_PREFAB, position)
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
