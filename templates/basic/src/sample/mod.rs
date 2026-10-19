//! The sample game a new game deletes: a player to steer, a hazard on patrol
//! and three gems to take, all composed from the kit in prefab files. Its
//! gameplay state spawns them for a run, fresh or from a save, and opens the
//! shell's game over when the player dies. It keeps the entities it spawned
//! in [`SampleRun`], which the death check, the HUD and `save::capture`
//! read, so a replaced run's entities, despawned at the frame's end, never
//! count.

pub mod components;
pub mod hud;

use tungsten::core::ui::UiTree;
use tungsten::core::{
    CameraController, Entity, PreparedPrefab, Schedule, Stage, Time, World, system,
};
use tungsten::kit::{Counters, Health, Stats, Tags};
use tungsten::{GameState, Plugin, RootCover, SceneEntity, StateContext, StateId};

use crate::game::GAMEPLAY;
use crate::shell::options::GameOptions;
use crate::shell::{self, Difficulty, RunStart, theme};
use components::{GemSlot, Player};

/// The player's prefab, by its manifest ID.
pub const PLAYER_PREFAB: &str = "player";
/// Where the player starts: the middle of a 1280 × 720 window.
pub const PLAYER_START: [f32; 2] = [624.0, 344.0];
/// The hazard's prefab: the player's sprite tinted red, on patrol.
pub const HAZARD_PREFAB: &str = "hazard";
/// Where the hazard's patrol starts: 160 px below the player's start.
pub const HAZARD_START: [f32; 2] = [624.0, 504.0];
/// The gem's prefab: the player's sprite tinted gold, which drifts to the
/// player in reach and counts `gems` on the game's `Counters` when taken.
pub const GEM_PREFAB: &str = "gem";
/// Where the gems start: beyond the player's reach of 96 px.
pub const GEM_STARTS: [[f32; 2]; 3] = [[424.0, 344.0], [824.0, 344.0], [624.0, 184.0]];
/// The tag the player wears while the damage numbers option is on; the
/// `labels` cue rule floats a hit's damage only with it.
pub const DAMAGE_NUMBERS_TAG: &str = "show.damage_numbers";
/// The camera's dead zone, in world pixels: the player roams the middle of
/// a 1280 × 720 view before the camera follows.
pub const CAMERA_DEAD_ZONE: [f32; 2] = [640.0, 360.0];
/// The camera shake's peak offset, in world pixels, at full trauma: a hit's
/// `shake` cue adds trauma, and the shell's Screen shake option scales it.
pub const CAMERA_SHAKE: [f32; 2] = [24.0, 16.0];

/// The player's full health at `difficulty`: its `max_health` stat's base.
#[must_use]
pub const fn max_health(difficulty: Difficulty) -> f32 {
    match difficulty {
        Difficulty::Easy => 8.0,
        Difficulty::Normal => 5.0,
        Difficulty::Hard => 3.0,
    }
}

/// The entities the run spawned: a `World` resource while gameplay runs.
#[derive(Debug, Clone)]
pub struct SampleRun {
    pub player: Entity,
    /// Each gem's place in [`GEM_STARTS`] and its entity, taken or not.
    pub gems: Vec<(u8, Entity)>,
}

/// The sample as a plugin: the components its prefabs name, the camera's
/// dead zone and shake, and `sample_frame` in `Update`.
pub struct SamplePlugin;

impl Plugin for SamplePlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        // Before the manifest loads, which checks each prefab's components.
        world.register_component::<Player>("player");
        if let Some(camera) = world.get_resource_mut::<CameraController>() {
            camera.dead_zone_size = CAMERA_DEAD_ZONE.into();
            // The trauma rides an 18 Hz carrier and decays in under a second.
            camera.shake_frequency_hz = 18.0;
            camera.shake_max_offset = CAMERA_SHAKE.into();
        }
        schedule.add(Stage::Update, system("sample_frame", frame));
    }
}

/// `sample_frame`: the damage numbers tag follows the option, and the HUD
/// follows the run.
fn frame(world: &mut World) {
    let shown = world.resource::<GameOptions>().damage_numbers;
    if let Some(player) = world.get_resource::<SampleRun>().map(|run| run.player)
        && let Some(tags) = world.get_mut::<Tags>(player)
    {
        if shown {
            tags.insert(DAMAGE_NUMBERS_TAG);
        } else {
            tags.remove(DAMAGE_NUMBERS_TAG);
        }
    }
    hud::update(world);
}

/// Gameplay: the run's entities spawn with it and leave with it; it holds
/// the game clock while a screen covers it, and opens game over when the
/// player dies.
pub struct GameplayState {
    start: Option<RunStart>,
}

impl GameplayState {
    #[must_use]
    pub fn new(start: RunStart) -> Self {
        Self { start: Some(start) }
    }
}

/// `prefab` at `position`, owned by gameplay, to spawn. The startup load
/// checked each prefab, so a failure here is a bug.
fn prepare(world: &mut World, prefab: &str, position: [f32; 2]) -> PreparedPrefab {
    world
        .prepare_prefab(prefab, position)
        .expect("the manifest's prefabs load with it")
        .with((SceneEntity { state_id: GAMEPLAY },))
}

impl GameState for GameplayState {
    fn id(&self) -> StateId {
        GAMEPLAY
    }

    fn on_enter(&mut self, ctx: &mut StateContext) {
        let world = &mut *ctx.world;
        let start = self.start.take().unwrap_or_default();
        let (at, gems_left, counted) = match &start.save {
            Some(save) => (save.player, save.gems_left.clone(), save.gems),
            None => (PLAYER_START, vec![0, 1, 2], 0),
        };
        let mut player = prepare(world, PLAYER_PREFAB, at);
        if let Some(save) = start.save {
            player = player.with((save.health,));
        }
        let player = player.spawn(world);
        // Before any step, so the first one already runs at this max.
        if let Some(stats) = world.get_mut::<Stats>(player) {
            stats.set_base(Stats::MAX_HEALTH, max_health(start.difficulty));
        }
        prepare(world, HAZARD_PREFAB, HAZARD_START).spawn(world);
        let gems = gems_left
            .into_iter()
            .filter_map(|slot| {
                let at = *GEM_STARTS.get(usize::from(slot))?;
                let gem = prepare(world, GEM_PREFAB, at).with((GemSlot(slot),));
                Some((slot, gem.spawn(world)))
            })
            .collect();
        if !world.has_resource::<Counters>() {
            world.insert_resource(Counters::new());
        }
        world.resource_mut::<Counters>().set("gems", counted);
        world.insert_resource(SampleRun { player, gems });
        let tree = world.resource_mut::<UiTree>();
        let hud = hud::build(tree);
        theme::slide_in(tree, hud.root);
        shell::stack(world).own_root(hud.root, RootCover::Show);
        world.insert_resource(hud);
        hud::update(world);
    }

    fn on_exit(&mut self, ctx: &mut StateContext) {
        ctx.world.remove_resource::<SampleRun>();
        ctx.world.remove_resource::<hud::Hud>();
    }

    fn on_pause(&mut self, ctx: &mut StateContext) {
        ctx.world.resource_mut::<Time>().pause();
    }

    fn on_resume(&mut self, ctx: &mut StateContext) {
        ctx.world.resource_mut::<Time>().resume();
    }

    fn update(&mut self, world: &mut World) {
        let dead = world
            .get_resource::<SampleRun>()
            .and_then(|run| world.get::<Health>(run.player))
            .is_some_and(Health::is_dead);
        if dead {
            shell::game_over(world);
        }
    }
}
