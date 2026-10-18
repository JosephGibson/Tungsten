//! The game's states on the engine's state stack: title, then gameplay, with
//! pause pushed over gameplay. Each state reads actions, never keys.
//! Gameplay continues from the autosave, which pause writes; pause holds
//! the game clock, so the kit's fixed-step systems hold with it.

use serde::{Deserialize, Serialize};
use tungsten::core::save::SaveError;
use tungsten::core::{
    ActionMap, InputState, Position, SaveData, SaveSlots, Time, Transform, With, World,
};
use tungsten::kit::Health;
use tungsten::{GameState, SceneEntity, StateContext, StateId, StateStack};

use crate::components::Player;

pub const TITLE: StateId = "title";
pub const GAMEPLAY: StateId = "gameplay";
pub const PAUSE: StateId = "pause";
/// The player's prefab, by its manifest ID: its sprite, `Player` and the
/// kit's components that move, hurt, flash and follow it.
pub const PLAYER_PREFAB: &str = "player";
/// Where the player starts: the middle of a 1280 × 720 window.
pub const PLAYER_START: [f32; 2] = [624.0, 344.0];
/// The hazard's prefab, by its manifest ID: the player's sprite tinted red,
/// which patrols a line and hurts the player on touch.
pub const HAZARD_PREFAB: &str = "hazard";
/// Where the hazard's patrol starts: 160 px below the player's start, so
/// its line stays clear of the player standing or walking across the view.
pub const HAZARD_START: [f32; 2] = [624.0, 504.0];
/// The gem's prefab, by its manifest ID: the player's sprite tinted gold,
/// bobbing, which drifts to the player once in reach of its
/// `pickup_radius` and counts `gems` on the game's `Counters` when taken.
pub const GEM_PREFAB: &str = "gem";
/// Where gameplay's three gems start: beyond the player's reach of 96 px,
/// left, right and above, so the player walks toward each.
pub const GEM_STARTS: [[f32; 2]; 3] = [[424.0, 344.0], [824.0, 344.0], [624.0, 184.0]];
/// The save slot gameplay continues from and pause writes.
pub const AUTOSAVE: &str = "autosave";

/// What the autosave holds: the player's position, its body's `Position`.
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

/// Gameplay: the player spawns with it, where the autosave left it, the
/// hazard at its patrol's start and the gems at theirs, and all leave with
/// it; `state_pause` pauses, and a player whose health runs out starts
/// gameplay over.
pub struct GameplayState;

impl GameState for GameplayState {
    fn id(&self) -> StateId {
        GAMEPLAY
    }

    fn on_enter(&mut self, ctx: &mut StateContext) {
        let start = load_autosave(ctx.world).map_or(PLAYER_START, |save| save.player);
        spawn(ctx.world, PLAYER_PREFAB, start);
        spawn(ctx.world, HAZARD_PREFAB, HAZARD_START);
        for at in GEM_STARTS {
            spawn(ctx.world, GEM_PREFAB, at);
        }
    }

    fn on_exit(&mut self, _ctx: &mut StateContext) {}

    fn update(&mut self, world: &mut World) {
        if player_dead(world) {
            // The fifth hit: gameplay starts over, from the autosave when
            // there is one.
            request(world, |stack| stack.request_replace(GameplayState));
        } else if just_pressed(world, "state_pause") {
            request(world, |stack| stack.request_push(PauseState::default()));
        }
    }
}

/// Whether the player's health has run out.
fn player_dead(world: &World) -> bool {
    world
        .query_filtered::<&Health, With<Player>>()
        .next()
        .is_some_and(Health::is_dead)
}

/// Pause, pushed over gameplay, pauses the game clock, so nothing on the
/// fixed step moves, and saves the autosave on its first frame that runs no
/// step: the pause takes effect from the next frame, and the frame that
/// pushes it still runs the steps it owed. `state_back` or `state_pause`
/// resumes.
#[derive(Default)]
pub struct PauseState {
    saved: bool,
}

impl GameState for PauseState {
    fn id(&self) -> StateId {
        PAUSE
    }

    fn on_enter(&mut self, ctx: &mut StateContext) {
        self.saved = false;
        if let Some(time) = ctx.world.get_resource_mut::<Time>() {
            time.pause();
        }
    }

    fn on_exit(&mut self, ctx: &mut StateContext) {
        if let Some(time) = ctx.world.get_resource_mut::<Time>() {
            time.resume();
        }
    }

    fn update(&mut self, world: &mut World) {
        let settled = world
            .get_resource::<Time>()
            .is_none_or(|time| time.fixed_steps_this_frame() == 0);
        if !self.saved && settled {
            write_autosave(world);
            self.saved = true;
        }
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

/// Saves the player's position, unless the autosave failed to load: its
/// body's `Position`, where the mover left it, which the drawn `Transform`
/// trails by up to a step.
fn write_autosave(world: &mut World) {
    if world.get_resource::<AutosaveReadOnly>().is_some() {
        return;
    }
    let Some(position) = world
        .query_filtered::<(&Position, &Transform), With<Player>>()
        .next()
        .map(|(position, _)| [position.0.x, position.0.y])
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

/// Spawns `prefab` at `position`, owned by gameplay. The startup load
/// checked each prefab against the game's components, so a failure here
/// is a bug.
fn spawn(world: &mut World, prefab: &str, position: [f32; 2]) {
    world
        .prepare_prefab(prefab, position)
        .expect("the manifest's prefabs load with it")
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
