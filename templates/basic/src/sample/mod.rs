//! The sample game a new game deletes: a player to steer, easing in and out,
//! who shoots at the cursor, four red enemies, three that move and one that
//! stands, which come back after they fall, a target dummy, three gems and
//! two pickups to take, all composed from the kit in prefab files. Its
//! gameplay state spawns them for a run, fresh or from a save, whose player
//! no enemy touches for its first 2.5 s, frames the play space whatever the
//! window's size, blooms its brightest pixels, plays the run's music, and
//! opens the shell's game over when the player dies. While pause or game
//! over is on top, the run's music falls silent, under pause's loop or game
//! over's music, and comes back after pause. Gameplay's sounds are cues in the prefabs. It keeps the entities
//! it spawned in [`SampleRun`], which the death check, the HUD and
//! `save::capture` read, so a replaced run's entities, despawned at the
//! frame's end, never count. It names each in the area [`AREA`], the
//! player globally, which a save holds them by: a loaded run spawns as a
//! new one, then takes the save's changes over it.

pub mod components;
pub mod hud;
pub mod script;

use tungsten::core::post::{BloomParams, PostPass, PostStack};
use tungsten::core::save::{self, SaveState, Saved, restore_world};
use tungsten::core::ui::UiTree;
use tungsten::core::{
    AudioCommands, CameraController, CameraRig, DataRef, Entity, FollowRig, PreparedPrefab,
    Schedule, SoundRegistry, Stage, Time, Timer, Voice, With, World, system,
};
use tungsten::kit::{Counters, EffectRequest, Health, Stats, Tags};
use tungsten::{GameState, Plugin, SceneEntity, StateContext, StateId};

use crate::game::GAMEPLAY;
use crate::save::SaveGame;
use crate::shell::game_over::GAME_OVER;
use crate::shell::options::GameOptions;
use crate::shell::pause::PAUSE;
use crate::shell::{self, Difficulty, RunStart};
use components::Player;

/// The sample's code-built level, the area its placed entities are named
/// in.
pub const AREA: &str = "sample";
/// The player's prefab, by its manifest ID.
pub const PLAYER_PREFAB: &str = "player";
/// Where the player starts: the middle of a 1280 × 720 window.
pub const PLAYER_START: [f32; 2] = [624.0, 344.0];
/// The effect a continued run's player spawns with: for its 2.5 s it grants
/// `status.spawning`, which every enemy's touch filters out, so a save made
/// in an enemy's reach does not die on load. A fresh run starts clear of
/// every reach and takes none.
pub const SPAWN_GRACE: &str = "spawn_grace";
/// The hazard's prefab: an enemy on patrol whose touch also slows.
pub const HAZARD_PREFAB: &str = "hazard";
/// Where the hazard's patrol starts: 160 px below the player's start.
pub const HAZARD_START: [f32; 2] = [624.0, 504.0];
/// The enemies, by prefab and place: the hazard, one that circles its
/// place, one that stands at its place and one on patrol across the top.
/// Each takes three hits; a fallen one comes back at its place
/// [`ENEMY_RETURN_SECS`] of game time later.
pub const ENEMIES: [(&str, [f32; 2]); 4] = [
    (HAZARD_PREFAB, HAZARD_START),
    ("enemy_orbit", [200.0, 520.0]),
    ("enemy_still", [1100.0, 380.0]),
    ("enemy_patrol", [640.0, 80.0]),
];
pub const ENEMY_RETURN_SECS: f32 = 4.0;
/// The gem's prefab: a gold diamond, which drifts to the player in reach
/// and counts `gems` on the game's `Counters` when taken.
pub const GEM_PREFAB: &str = "gem";
/// Where the gems start: beyond the player's reach of 96 px.
pub const GEM_STARTS: [[f32; 2]; 3] = [[424.0, 344.0], [824.0, 344.0], [624.0, 184.0]];
/// The target dummy's prefab, which a `damaged` cue heals whole, and where
/// it stands.
pub const DUMMY_PREFAB: &str = "dummy";
pub const DUMMY_START: [f32; 2] = [944.0, 184.0];
/// The pickups, by their place, clear of the HUD at any scale: one heals
/// 2, one speeds the player for 4 s.
pub const PICKUPS: [(&str, [f32; 2]); 2] = [
    ("heal_pickup", [304.0, 280.0]),
    ("speed_pickup", [944.0, 584.0]),
];
/// The tag the player, the dummy and the enemies wear while the damage
/// numbers option is on; the `labels` cue rule floats a hit's damage only
/// with it.
pub const DAMAGE_NUMBERS_TAG: &str = "show.damage_numbers";
/// The play space's centre, which a run's camera starts on.
pub const PLAY_CENTRE: [f32; 2] = [640.0, 360.0];
/// The run's music and pause's loop, by manifest ID, and the seconds they
/// fade over: a run's start and end, and pause coming on top or going.
pub const MUSIC_RUN: &str = "music_run";
pub const MUSIC_PAUSE: &str = "music_pause";
pub const MUSIC_FADE: f32 = 0.5;
pub const PAUSE_FADE: f32 = 0.3;
/// The camera's dead zone, in world pixels: the player roams the middle of
/// a 1280 × 720 view before the camera follows.
pub const CAMERA_DEAD_ZONE: [f32; 2] = [640.0, 360.0];
/// The camera shake's peak offset, in world pixels, at full trauma: a hit's
/// `shake` cue adds trauma, and the shell's Screen shake option scales it.
pub const CAMERA_SHAKE: [f32; 2] = [24.0, 16.0];
/// The run's one post pass: a bloom whose threshold sits under white, so
/// the sprites' full channels glow on the sRGB scene: the bolts, the sparks
/// and the gems, and some of the pickups' and the enemies' highlights. The
/// run builds its stack whole and puts the default back when it leaves;
/// the shell takes the pass out while its Bloom option is off.
pub const BLOOM: BloomParams = BloomParams {
    threshold: 0.9,
    knee: 0.1,
    intensity: 3.0,
    radius: 1.0,
};

/// The music playing: the run's voice, which plays on at 0 under pause, and
/// pause's loop while pause is on top.
#[derive(Debug, Clone, Copy, Default)]
struct Music {
    run: Option<Voice>,
    pause: Option<Voice>,
    ducked: bool,
}

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
    pub dummy: Entity,
    /// Each pickup's place in [`PICKUPS`] and its entity, taken or not.
    pub pickups: Vec<(u8, Entity)>,
    /// Each enemy of [`ENEMIES`], in its order.
    pub enemies: Vec<Enemy>,
}

/// An enemy of [`ENEMIES`]: its entity, and the timer on the game clock
/// that brings it back [`ENEMY_RETURN_SECS`] after it falls.
#[derive(Debug, Clone, Copy)]
pub struct Enemy {
    pub entity: Entity,
    pub back: Timer,
}

impl Enemy {
    /// Spawns the enemy at `slot` of [`ENEMIES`] at its place, named
    /// `enemy_<slot>`: a fallen one's name is free.
    fn spawn(world: &mut World, slot: usize) -> Self {
        let (prefab, at) = ENEMIES[slot];
        let entity = prepare(world, prefab, at).spawn(world);
        name(world, entity, Some(AREA), &format!("enemy_{slot}"));
        let back = Timer::once(ENEMY_RETURN_SECS);
        Self { entity, back }
    }

    /// Whether it stands: its health above 0.
    #[must_use]
    pub fn alive(&self, world: &World) -> bool {
        world
            .get::<Health>(self.entity)
            .is_some_and(|h| !h.is_dead())
    }
}

/// Names `entity` for saves; a run's names are free, since its exit drops
/// the last run's.
fn name(world: &mut World, entity: Entity, area: Option<&str>, name: &str) {
    if let Err(err) = save::name(world, entity, area, name) {
        log::warn!("{err}");
    }
}

/// Spawns each of `items`, named `<what>_<slot>` in [`AREA`].
fn place(world: &mut World, what: &str, items: &[(&str, [f32; 2])]) -> Vec<(u8, Entity)> {
    (0..)
        .zip(items)
        .map(|(slot, &(prefab, at))| {
            let entity = prepare(world, prefab, at).spawn(world);
            name(world, entity, Some(AREA), &format!("{what}_{slot}"));
            (slot, entity)
        })
        .collect()
}

/// Lays `save`'s run over the new one, each fallen enemy down as long as
/// it was, under the spawn grace; a save that does not load leaves the new
/// run.
fn continue_run(world: &mut World, save: &SaveGame, player: Entity) {
    let scene = (SceneEntity { state_id: GAMEPLAY },);
    if let Err(err) = restore_world(world, &save.world, &scene) {
        log::warn!("The save's run did not load: {err}");
    }
    let enemies = &mut world.resource_mut::<SampleRun>().enemies;
    for &(slot, down) in &save.down {
        if let Some(enemy) = enemies.get_mut(usize::from(slot)) {
            enemy.back.tick(down);
        }
    }
    EffectRequest::new(player, DataRef::by_id(SPAWN_GRACE)).apply(world);
}

/// The sample as a plugin: the components its prefabs name, the camera's
/// shake, and `sample_frame` in `Update`.
pub struct SamplePlugin;

impl Plugin for SamplePlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        // Before the manifest loads, which checks each prefab's components.
        world.register_component::<Player>("player");
        script::register(world);
        if let Some(camera) = world.get_resource_mut::<CameraController>() {
            // The trauma rides an 18 Hz carrier and decays in under a second.
            camera.shake_frequency_hz = 18.0;
            camera.shake_max_offset = CAMERA_SHAKE.into();
        }
        schedule.add(Stage::Update, system("sample_frame", frame));
    }
}

/// The player's camera: a follow of its point with the dead zone, 720
/// world units tall at every window, so a taller window shows the same
/// play space larger.
fn player_camera() -> CameraRig {
    CameraRig::follow(FollowRig::default().with_dead_zone(CAMERA_DEAD_ZONE.into()))
        .with_view_height(720.0)
}

/// Pause's music: when pause comes on top, the run's voice falls to 0 and
/// pause's loop fades in; when gameplay is back on top, the loop stops and
/// the run's voice comes back. A screen over pause keeps the loop. Game
/// over silences the run's voice too, under the music the shell plays.
fn duck(world: &mut World) {
    let Some(music) = world.get_resource::<Music>().copied() else {
        return;
    };
    let top = shell::top(world);
    let paused = top == Some(PAUSE);
    let (on, off) = (paused || top == Some(GAME_OVER), top == Some(GAMEPLAY));
    if on == music.ducked || !(on || off) {
        return;
    }
    let pause = if paused {
        shell::play(world, MUSIC_PAUSE, PAUSE_FADE)
    } else {
        None
    };
    let registry = world.resource::<SoundRegistry>();
    let level = registry
        .get_by_id(MUSIC_RUN)
        .map_or(1.0, |run| registry.get_volume(run));
    let audio = world.resource_mut::<AudioCommands>();
    if let Some(run) = music.run {
        audio.set_voice_volume(run, if on { 0.0 } else { level }, PAUSE_FADE);
    }
    if let Some(looped) = music.pause {
        audio.stop_voice(looped, PAUSE_FADE);
    }
    world.insert_resource(Music {
        pause,
        ducked: on,
        ..music
    });
}

/// Ticks each fallen enemy's timer by the game clock and brings the enemy
/// back at its place when the timer ends.
fn bring_back(world: &mut World) {
    let delta = world.resource::<Time>().delta();
    let Some(mut enemies) = world
        .get_resource::<SampleRun>()
        .map(|run| run.enemies.clone())
    else {
        return;
    };
    for (slot, enemy) in enemies.iter_mut().enumerate() {
        if enemy.alive(world) {
            enemy.back.reset();
        } else if enemy.back.tick(delta) > 0 {
            *enemy = Enemy::spawn(world, slot);
        }
    }
    world.resource_mut::<SampleRun>().enemies = enemies;
}

/// `sample_frame`: fallen enemies come back, the damage numbers tag follows
/// the option on whatever has health (the player, the dummy and the
/// enemies), the music the stack and the HUD the run.
fn frame(world: &mut World) {
    bring_back(world);
    let shown = world.resource::<GameOptions>().damage_numbers;
    for tags in world.query_mut_filtered::<&mut Tags, With<Health>>() {
        if shown {
            tags.insert(DAMAGE_NUMBERS_TAG);
        } else {
            tags.remove(DAMAGE_NUMBERS_TAG);
        }
    }
    duck(world);
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
        let player = prepare(world, PLAYER_PREFAB, PLAYER_START)
            .with((player_camera(),))
            .spawn(world);
        // Before any step, so the first one already runs at this max.
        if let Some(stats) = world.get_mut::<Stats>(player) {
            stats.set_base(Stats::MAX_HEALTH, max_health(start.difficulty));
        }
        name(world, player, None, "player");
        let enemies = (0..ENEMIES.len())
            .map(|slot| Enemy::spawn(world, slot))
            .collect();
        let dummy = prepare(world, DUMMY_PREFAB, DUMMY_START).spawn(world);
        name(world, dummy, Some(AREA), "dummy");
        let pickups = place(world, "pickup", &PICKUPS);
        let gems = GEM_STARTS.map(|at| (GEM_PREFAB, at));
        let gems = place(world, "gem", &gems);
        if !world.has_resource::<Counters>() {
            world.insert_resource(Counters::new());
        }
        world.resource_mut::<Counters>().set("gems", 0);
        world.insert_resource(SampleRun {
            player,
            gems,
            dummy,
            pickups,
            enemies,
        });
        if let Some(save) = &start.save {
            continue_run(world, save, player);
        }
        // Every run starts its view centred on the play space, a load's too.
        world
            .resource_mut::<CameraController>()
            .cut_to(PLAY_CENTRE.into());
        let run = shell::play(world, MUSIC_RUN, MUSIC_FADE);
        world.insert_resource(Music {
            run,
            ..Music::default()
        });
        let hud = hud::build(world.resource_mut::<UiTree>());
        shell::enter(world, hud.root);
        world.insert_resource(hud);
        hud::update(world);
        world.insert_resource(PostStack(vec![PostPass::Bloom(BLOOM)]));
    }

    fn on_exit(&mut self, ctx: &mut StateContext) {
        ctx.world.remove_resource::<SampleRun>();
        // The next run names its own: this one's, despawned at the frame's
        // flush, hold no save identity meanwhile.
        let saved: Vec<Entity> = ctx.world.query_filtered::<Entity, With<Saved>>().collect();
        for entity in saved {
            ctx.world.remove_component::<Saved>(entity);
        }
        ctx.world.remove_resource::<SaveState>();
        ctx.world.remove_resource::<hud::Hud>();
        ctx.world.insert_resource(PostStack::default());
        // Both loops stop with the run, pause's too: a Quit to Title from
        // pause leaves the stack in one pass, with gameplay never on top.
        let music = ctx.world.remove_resource::<Music>().unwrap_or_default();
        let audio = ctx.world.resource_mut::<AudioCommands>();
        for voice in [music.run, music.pause].into_iter().flatten() {
            audio.stop_voice(voice, MUSIC_FADE);
        }
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
