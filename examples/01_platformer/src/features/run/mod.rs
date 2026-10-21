//! A run: the gameplay state, which spawns the level and the player fresh or
//! from a save, plays the run's music, holds the game clock while a screen
//! covers it, toasts and autosaves as the player enters each area, and
//! opens the shell's game over when the player dies.

pub mod components;
pub mod systems;

use glam::Vec2;
use tungsten::core::{EventCursor, Schedule, Stage, Time, World, system};
use tungsten::kit::{Health, Phase, ensure_phases};
use tungsten::{GameState, Plugin, StateContext, StateId};

pub use components::Run;

use crate::features::{audio, level, player, spells};
use crate::game::GAMEPLAY;
use crate::save;
use crate::shared::SaveIds;
use crate::shell::{self, RunStart};

/// The run as a plugin: `run_areas` in the react phase.
pub struct RunPlugin;

impl Plugin for RunPlugin {
    fn build(&self, schedule: &mut Schedule, _world: &mut World) {
        ensure_phases(schedule);
        let mut cursor = EventCursor::new();
        let areas = system("run_areas", move |world: &mut World| {
            systems::areas(world, &mut cursor);
        });
        schedule.add(Stage::FixedUpdate, Phase::React.join(areas));
    }
}

/// Gameplay: the run's entities spawn with it and leave with it.
pub struct GameplayState {
    start: Option<RunStart>,
}

impl GameplayState {
    #[must_use]
    pub fn new(start: RunStart) -> Self {
        Self { start: Some(start) }
    }
}

impl GameState for GameplayState {
    fn id(&self) -> StateId {
        GAMEPLAY
    }

    fn on_enter(&mut self, ctx: &mut StateContext) {
        let world = &mut *ctx.world;
        let start = self.start.take().unwrap_or_default();
        world.insert_resource(SaveIds::default());
        level::spawn(world);
        let feet = level::marker(world, level::SPAWN_MARKER).expect("the map marks the spawn");
        // Half a pixel above the turf, so its first steps find the ground.
        let spawn = feet - Vec2::new(0.0, player::PLAYER_HALF.y + 0.5);
        let centre = start.save.as_ref().map_or(spawn, |save| {
            Vec2::from(save.player.position) + player::BODY_OFFSET
        });
        let player = player::spawn(world, centre, start.difficulty);
        // The area a save names is the run's already: no toast, no autosave.
        let area = start.save.as_ref().and_then(|save| save.area.clone());
        world.insert_resource(Run { player, area });
        if let Some(save) = &start.save {
            save::restore(world, player, save);
        }
        audio::start(world);
        spells::start(world);
    }

    fn on_exit(&mut self, ctx: &mut StateContext) {
        ctx.world.remove_resource::<Run>();
        level::despawn(ctx.world);
        audio::stop(ctx.world);
        spells::stop(ctx.world);
    }

    fn on_pause(&mut self, ctx: &mut StateContext) {
        ctx.world.resource_mut::<Time>().pause();
    }

    fn on_resume(&mut self, ctx: &mut StateContext) {
        ctx.world.resource_mut::<Time>().resume();
    }

    fn update(&mut self, world: &mut World) {
        let dead = world
            .get_resource::<Run>()
            .and_then(|run| world.get::<Health>(run.player))
            .is_some_and(Health::is_dead);
        if dead {
            shell::game_over(world);
        }
    }
}
