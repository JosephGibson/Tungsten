//! A run: the gameplay state, which spawns the level and the player fresh or
//! from a save, plays the run's music, holds the game clock while a screen
//! covers it, names each area the player enters on its banner and
//! autosaves, and opens the shell's game over two game seconds after the
//! player dies.

pub mod banner;
pub mod components;
pub mod pit;
pub mod script;
pub mod systems;

use glam::Vec2;
use tungsten::core::save::{self as engine_save, SaveState};
use tungsten::core::{Entity, EventCursor, Schedule, Stage, Time, With, World, system};
use tungsten::kit::{Health, Phase, ensure_phases};
use tungsten::{GameState, Plugin, StateContext, StateId};

pub use components::Run;

use crate::features::elements::cascade::Cascades;
use crate::features::props::components::{MapProp, PropKind, StoneSize};
use crate::features::spells::components::Summon;
use crate::features::{audio, dummy, enemies, level, player, props, spells, toys};
use crate::game::GAMEPLAY;
use crate::save;
use crate::shared::Saved;
use crate::shell::{self, RunStart};

/// The run as a plugin: `run_areas` in the react phase, the banner's fade
/// each frame, and the run's script hooks.
pub struct RunPlugin;

impl Plugin for RunPlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        ensure_phases(schedule);
        script::register(world);
        let mut cursor = EventCursor::new();
        let areas = system("run_areas", move |world: &mut World| {
            systems::areas(world, &mut cursor);
        });
        schedule.add(Stage::FixedUpdate, Phase::React.join(areas));
        schedule.add(Stage::Update, system("run_banner", banner::tick));
    }
}

/// Names the run for saves once its setup is done, so each name's baseline
/// is its set-up state: the player, global, then the map's saved objects in
/// the map's area ([`level::name_saved`]). The first naming gives the World
/// its save state, whose counter the run's spawns take their IDs from.
fn name_run(world: &mut World, player: Entity) {
    world.remove_component::<Saved>(player);
    if let Err(err) = engine_save::name(world, player, None, player::SAVE_NAME) {
        log::warn!("{err}");
    }
    level::name_saved(world);
}

/// A fresh run's loose things, quietly, each taking the run's next save ID,
/// before the run sounds: the mine's iron at the map's markers, then the
/// map's crates and kegs (`MapProp`) and the hollow's stones. A load's are
/// its save's; a load that fails sets them down as a fresh run does.
fn set_down_props(world: &mut World) {
    for at in level::markers(world, level::IRON_MARKER) {
        toys::place_iron(world, at);
    }
    for (marker, kind) in [
        (level::CRATE_MARKER, PropKind::Crate),
        (level::KEG_MARKER, PropKind::Keg),
    ] {
        for at in level::markers(world, marker) {
            let prop = props::spawn(world, kind, at);
            world.insert(prop, MapProp);
        }
    }
    for size in StoneSize::ALL {
        for at in level::markers(world, level::STONE_MARKERS[size as usize]) {
            props::spawn(world, PropKind::Stone(size), at);
        }
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
        world.insert_resource(Cascades::default());
        world.insert_resource(Summon::default());
        level::spawn(world);
        dummy::start(world);
        enemies::start(world, start.difficulty);
        enemies::drops::start(world);
        let feet = level::marker(world, level::SPAWN_MARKER).expect("the map marks the spawn");
        // Half a pixel above the turf, so its first steps find the ground.
        // A load's player stands here too: its saved point is a patch over
        // the fresh spawn's, which the restore lays in this frame.
        let spawn = feet - Vec2::new(0.0, player::body(world).half.y + 0.5);
        let player = player::spawn(world, spawn);
        name_run(world, player);
        world.insert_resource(Run { player, area: None });
        match &start.save {
            None => set_down_props(world),
            Some(save) => match save::restore(world, save) {
                // The area a save names is the run's already: no toast, no
                // autosave.
                Ok(()) => world.resource_mut::<Run>().area = save.area.clone(),
                Err(err) => {
                    log::warn!("The save's run did not load, so a new run starts: {err}");
                    set_down_props(world);
                }
            },
        }
        audio::start(world);
        spells::start(world);
        banner::start(world);
    }

    fn on_exit(&mut self, ctx: &mut StateContext) {
        // The next run names its own: this one's, despawned at the frame's
        // flush, hold no save identity meanwhile.
        let saved: Vec<Entity> = ctx.world.query_filtered::<Entity, With<Saved>>().collect();
        for entity in saved {
            ctx.world.remove_component::<Saved>(entity);
        }
        ctx.world.remove_resource::<SaveState>();
        ctx.world.remove_resource::<Run>();
        ctx.world.remove_resource::<banner::Banner>();
        ctx.world
            .remove_resource::<dummy::components::PlacedDummies>();
        ctx.world
            .remove_resource::<enemies::components::PlacedEnemies>();
        ctx.world.remove_resource::<enemies::drops::PlacedOrbs>();
        ctx.world.remove_resource::<Cascades>();
        ctx.world.remove_resource::<Summon>();
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
