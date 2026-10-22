//! A run: the gameplay state, which spawns the level and the player fresh or
//! from a save, plays the run's music, holds the game clock while a screen
//! covers it, names each area the player enters on its banner and
//! autosaves, and opens the shell's game over two game seconds after the
//! player dies.

pub mod banner;
pub mod components;
pub mod systems;

use glam::Vec2;
use tungsten::core::{EventCursor, Schedule, Stage, Time, World, system};
use tungsten::kit::{Health, Phase, ensure_phases};
use tungsten::{GameState, Plugin, StateContext, StateId};

pub use components::Run;

use crate::features::combat::components::Hints;
use crate::features::combat::numbers::Numbers;
use crate::features::elements::cascade::Cascades;
use crate::features::spells::components::Summon;
use crate::features::{audio, dummy, enemies, level, player, spells, toys};
use crate::game::GAMEPLAY;
use crate::save;
use crate::shared::SaveIds;
use crate::shell::{self, RunStart};

/// The run as a plugin: `run_areas` in the react phase, the banner's fade
/// each frame.
pub struct RunPlugin;

impl Plugin for RunPlugin {
    fn build(&self, schedule: &mut Schedule, _world: &mut World) {
        ensure_phases(schedule);
        let mut cursor = EventCursor::new();
        let areas = system("run_areas", move |world: &mut World| {
            systems::areas(world, &mut cursor);
        });
        schedule.add(Stage::FixedUpdate, Phase::React.join(areas));
        schedule.add(Stage::Update, system("run_banner", banner::tick));
    }
}

/// Game seconds a dead player falls, as its controller drops it, and its
/// death clip plays and holds, before the shell's game over opens (F13).
pub const DEATH_SECS: f32 = 2.0;

/// Gameplay: the run's entities spawn with it and leave with it.
pub struct GameplayState {
    start: Option<RunStart>,
    /// Game seconds since the player died.
    dead_for: f32,
}

impl GameplayState {
    #[must_use]
    pub fn new(start: RunStart) -> Self {
        Self {
            start: Some(start),
            dead_for: 0.0,
        }
    }
}

impl GameState for GameplayState {
    fn id(&self) -> StateId {
        GAMEPLAY
    }

    fn on_enter(&mut self, ctx: &mut StateContext) {
        let world = &mut *ctx.world;
        let start = self.start.take().unwrap_or_default();
        self.dead_for = 0.0;
        world.insert_resource(SaveIds::default());
        world.insert_resource(Cascades::default());
        world.insert_resource(Hints::default());
        world.insert_resource(Summon::default());
        level::spawn(world);
        dummy::start(world);
        enemies::start(world, start.difficulty);
        enemies::drops::start(world);
        let feet = level::marker(world, level::SPAWN_MARKER).expect("the map marks the spawn");
        // Half a pixel above the turf, so its first steps find the ground.
        let spawn = feet - Vec2::new(0.0, player::PLAYER_HALF.y + 0.5);
        let centre = start.save.as_ref().map_or(spawn, |save| {
            Vec2::from(save.player.position) + player::BODY_OFFSET
        });
        let player = player::spawn(world, centre, start.difficulty);
        // A fresh run sets the mine's loose iron down, the blocks taking the
        // save IDs after the player's, before the run sounds; a load restores
        // the blocks it saved.
        if start.save.is_none() {
            for at in level::markers(world, level::IRON_MARKER) {
                toys::place_iron(world, at);
            }
        }
        // The area a save names is the run's already: no toast, no autosave.
        let area = start.save.as_ref().and_then(|save| save.area.clone());
        world.insert_resource(Run { player, area });
        if let Some(save) = &start.save {
            save::restore(world, player, save);
        }
        audio::start(world);
        spells::start(world);
        banner::start(world);
    }

    fn on_exit(&mut self, ctx: &mut StateContext) {
        ctx.world.remove_resource::<Run>();
        ctx.world.remove_resource::<banner::Banner>();
        ctx.world
            .remove_resource::<dummy::components::PlacedDummies>();
        ctx.world
            .remove_resource::<enemies::components::PlacedEnemies>();
        ctx.world.remove_resource::<enemies::drops::PlacedOrbs>();
        ctx.world.remove_resource::<Cascades>();
        ctx.world.remove_resource::<Hints>();
        ctx.world.remove_resource::<Summon>();
        ctx.world.remove_resource::<Numbers>();
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
        if !dead {
            return;
        }
        self.dead_for += world.resource::<Time>().delta();
        if self.dead_for >= DEATH_SECS {
            shell::game_over(world);
        }
    }
}
