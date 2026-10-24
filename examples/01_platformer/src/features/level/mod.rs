//! The level: `level.tmj`'s tile layers and its object layers, which place
//! the props, the slab colliders, the hazards, the moving platforms, the
//! areas and the ambient emitters from generated prefabs, under the world's
//! gravity, with the camera's bounds and its zoom by the window's height.
//! Its presentation: the sky, the moon and the parallax strips with each
//! area's far strip and night sky, the title's starry night while no run
//! is live (`title`), the glows with their lights, the cursor, each area's
//! light and today's post stack.

pub mod components;
pub mod draw;
pub mod elevator;
pub mod lava;
pub mod systems;
pub mod title;

use glam::{Vec2, Vec3};
use tungsten::core::post::{BloomParams, PostPass, PostStack, VignetteParams};
use tungsten::core::{
    AmbientLight, CameraBounds, CameraController, DataRegistry, Entity, Light, ParticleBudget,
    PhysicsConfig, Schedule, Stage, TilemapInstance, TilemapRegistry, Transform, World, system,
};
use tungsten::kit::{PathFollower, Phase, ensure_phases};
use tungsten::plugins::CAMERA_UPDATE;
use tungsten::{Extracts, Plugin, SceneEntity};

use crate::features::dummy::components::Dummy;
use crate::features::enemies::components::Enemy;
use crate::features::enemies::drops::Orb;
use crate::game::GAMEPLAY;
use crate::shared::{SaveIds, burst_cleanup};
use components::SceneTime;

/// The level's tilemap, by its manifest ID.
pub const MAP: &str = "ex10_level";
/// The `opaque_tiles` entry that names the sprites every pixel of which is
/// opaque.
pub const OPAQUE: &str = "level";
/// A tile's side, in world pixels.
pub const TILE: f32 = 64.0;
/// The map's size, in tiles: `tools/level.json`'s `cols` and `rows`.
pub const MAP_COLS: u32 = 430;
pub const MAP_ROWS: u32 = 72;
/// The physics layer of lava's surface, as a mask bit (`tools/level.py`'s
/// `LAVA_LAYER`, 4): a static box at each basin's skin that the player's
/// controller stands on and that meets no body, so balls, blocks and the
/// enemies, whose filters leave it out, fall in.
pub const LAVA_LAYER: u32 = 1 << 4;
/// The world's gravity, px/s² with y down, which the player and the bodies
/// fall by.
pub const GRAVITY: f32 = 3600.0;
/// The rows of tiles a view shows at a zoom of 1.
pub const CAMERA_ROWS: f32 = 18.0;
/// The map's marker where a fresh run's player stands, its feet's point.
pub const SPAWN_MARKER: &str = "spawn";
/// Below this, `tools/level.json`'s `kill_row`, a fall ends the run.
pub const KILL_Y: f32 = 71.0 * TILE;
/// The box the bodies live in: past it, one has escaped the level.
pub const WORLD_MIN: Vec2 = Vec2::new(-2.0 * TILE, -8.0 * TILE);
pub const WORLD_MAX: Vec2 = Vec2::new(
    (MAP_COLS as f32 + 2.0) * TILE,
    (MAP_ROWS as f32 + 8.0) * TILE,
);
/// The camera's shake at full trauma, in world pixels, its carrier and its
/// decay: a hit's trauma of 0.5 moves the view a few pixels for half a
/// second. The shell's Screen shake option scales the offset.
pub const SHAKE_OFFSET: Vec2 = Vec2::new(0.22 * TILE, 0.12 * TILE);
pub const SHAKE_HZ: f32 = 18.0;
pub const SHAKE_DECAY: f32 = 2.2;
/// Midnight: the ambient light the lit sprites take, and the moon's fill.
pub const AMBIENT: Vec3 = Vec3::new(0.62, 0.62, 0.57);
pub const MOONLIGHT: Vec3 = Vec3::new(0.85, 0.85, 0.72);
pub const MOONLIGHT_INTENSITY: f32 = 0.1;
/// Indoors, a cave: the ambient the lit sprites take there, the moon out.
pub const AMBIENT_INDOOR: Vec3 = Vec3::new(0.30, 0.28, 0.32);
/// The real seconds an area's light takes to ease from outdoors to indoors
/// or back.
pub const LIGHT_EASE: f32 = 1.0;
/// How far, in world pixels, a far strip and the night sky crossfade each
/// side of the border between two areas.
pub const AREA_FADE: f32 = 5.0 * TILE;
/// [`systems::glows`]'s name: the player's lantern writes its glow before.
pub const GLOWS: &str = "level_glows";
/// Most particles alive at once, the emitters' shared budget.
pub const PARTICLE_CAP: u32 = 2048;

/// The level as a plugin: the components its prefabs name, the world's
/// gravity and its broadphase cells of one tile, which cap a dense pile's
/// pair candidates, the particle budget, the camera's follow and shake, the
/// level's clock on the
/// step, `level_zoom` before the camera's update, the glows in the present
/// phase, the bursts' cleanup and the title's sky in `Update`, the title's
/// view before the camera's update, and the backdrop under the engine's
/// default drawing.
pub struct LevelPlugin;

impl Plugin for LevelPlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        // Before the manifest loads, which checks each prefab's components.
        world.register_component::<components::AmbientEmitter>("ambient_emitter");
        world.register_component::<components::Hazard>("hazard");
        world.register_component::<components::Area>("area");
        world.register_component::<components::Glow>("glow");
        world.register_component::<elevator::ElevatorDef>("elevator");
        world.register_data_kind::<components::OpaqueTiles>("opaque_tiles");
        if let Some(physics) = world.get_resource_mut::<PhysicsConfig>() {
            physics.gravity = Vec2::new(0.0, GRAVITY);
            physics.broadphase_cell_size = TILE;
        }
        if let Some(camera) = world.get_resource_mut::<CameraController>() {
            camera.dead_zone_size = Vec2::ZERO;
            camera.smoothing_factor = 1.0;
            camera.shake_amplitude = Vec2::ZERO;
            camera.shake_frequency_hz = SHAKE_HZ;
            camera.shake_decay = SHAKE_DECAY;
            camera.shake_max_offset = SHAKE_OFFSET;
        }
        world.insert_resource(components::Zoom::default());
        world.insert_resource(ParticleBudget {
            global_cap: PARTICLE_CAP,
        });
        world.resource_mut::<Extracts>().replace_sprites(draw::base);
        ensure_phases(schedule);
        schedule.add(
            Stage::FixedUpdate,
            Phase::Tick.join(system("level_clock", systems::clock)),
        );
        schedule.add(
            Stage::FixedUpdate,
            Phase::Tick.join(system("lava_tick", lava::tick)),
        );
        schedule.add(
            Stage::FixedUpdate,
            Phase::Intent.join(system("elevator_tick", elevator::tick)),
        );
        let take = system("lava_take", lava::take)
            .after("props_heat")
            .before("cascade_run");
        schedule.add(Stage::FixedUpdate, Phase::Overlap.join(take));
        schedule.add(
            Stage::PostUpdate,
            Phase::Present.join(system("elevator_look", elevator::look)),
        );
        schedule.add(
            Stage::PostUpdate,
            Phase::Present.join(system("crust_look", lava::look)),
        );
        let zoom = system("level_zoom", systems::zoom).before(CAMERA_UPDATE);
        schedule.add(Stage::PostUpdate, zoom);
        let title_view = system("title_view", title::view)
            .after("level_zoom")
            .before(CAMERA_UPDATE);
        schedule.add(Stage::PostUpdate, title_view);
        schedule.add(Stage::Update, system("title_sky", title::sky));
        schedule.add(
            Stage::PostUpdate,
            Phase::Present.join(system(GLOWS, systems::glows)),
        );
        schedule.add(
            Stage::PostUpdate,
            Phase::Present.join(system("level_light", systems::light)),
        );
        schedule.add(Stage::Update, system("burst_cleanup", burst_cleanup));
    }
}

/// The cursor's sprite, drawn after every other sprite contribution: the
/// plugin `game::register` adds last.
pub struct CursorPlugin;

impl Plugin for CursorPlugin {
    fn build(&self, _schedule: &mut Schedule, world: &mut World) {
        world.resource_mut::<Extracts>().add_sprites(draw::cursor);
    }
}

/// Spawns the level for a run, owned by gameplay: the map and its objects,
/// each one a save names ([`saved`]) taking the next of the run's
/// [`SaveIds`] in layer and file order, the basins' chill and crust ([`lava::start`]), the elevator at rest
/// ([`elevator::start`]), the moon's light, the
/// level's clock, the midnight ambient and the post stack, and bounds the
/// camera to the map. Returns the objects in that order.
pub fn spawn(world: &mut World) -> Vec<Entity> {
    let map = TilemapInstance::new(MAP, Vec2::ZERO);
    world.spawn_with((map, SceneEntity { state_id: GAMEPLAY }));
    let objects = world
        .spawn_map_objects(MAP, Vec2::ZERO)
        .expect("the level's objects load with the manifest");
    for &entity in &objects {
        if saved(world, entity) {
            let id = world.resource_mut::<SaveIds>().take();
            world.insert(entity, id);
        }
        world.insert(entity, SceneEntity { state_id: GAMEPLAY });
    }
    lava::start(world);
    elevator::start(world);
    let mut moon = Light::directional(MOONLIGHT, -std::f32::consts::FRAC_PI_4);
    moon.intensity = MOONLIGHT_INTENSITY;
    world.spawn_with((
        moon,
        Transform::default(),
        SceneEntity { state_id: GAMEPLAY },
    ));
    world.insert_resource(SceneTime::default());
    world.insert_resource(components::AreaLight::default());
    // What the map's opaque tiles hide of the backdrop.
    let cover = world
        .get_resource::<TilemapRegistry>()
        .and_then(|maps| maps.get(MAP))
        .zip(
            world
                .get_resource::<DataRegistry>()
                .and_then(|data| data.get::<components::OpaqueTiles>(OPAQUE)),
        )
        .map(|(map, opaque)| components::Cover::of(map, Vec2::ZERO, opaque));
    if let Some(cover) = cover {
        world.insert_resource(cover);
    }
    world.insert_resource(AmbientLight(AMBIENT));
    world.insert_resource(post_stack());
    let size = Vec2::new(MAP_COLS as f32, MAP_ROWS as f32) * TILE;
    let bounds = CameraBounds {
        min: Vec2::ZERO,
        max: size,
    };
    world.resource_mut::<CameraController>().bounds = Some(bounds);
    objects
}

/// Whether a save names the map object `entity`, which then takes a save
/// ID: a lava basin, a deck, a dummy, an enemy or a placed orb. Props,
/// slabs, iron, areas, emitters and markers take none, so an art change
/// moves no save (schema 10).
fn saved(world: &World, entity: Entity) -> bool {
    world.has::<components::Hazard>(entity)
        || world.has::<PathFollower>(entity)
        || world.has::<elevator::ElevatorDef>(entity)
        || world.has::<Dummy>(entity)
        || world.has::<Enemy>(entity)
        || world.has::<Orb>(entity)
}

/// Undoes what [`spawn`] set beyond its entities, which leave with
/// gameplay: the clock, the area's light, the ambient light and the post
/// stack.
pub fn despawn(world: &mut World) {
    world.remove_resource::<SceneTime>();
    world.remove_resource::<components::Cover>();
    world.remove_resource::<components::AreaLight>();
    world.insert_resource(AmbientLight::default());
    world.insert_resource(PostStack::default());
}

/// Midnight's post stack, today's: highlights bloom enough that the Bloom
/// option reads (B6) and the corners darken a little, with no blur over the
/// pixel art.
#[must_use]
pub fn post_stack() -> PostStack {
    PostStack(vec![
        PostPass::Bloom(BloomParams {
            threshold: 0.72,
            knee: 0.16,
            intensity: 0.6,
            radius: 0.65,
        }),
        PostPass::Vignette(VignetteParams {
            inner: 0.38,
            outer: 0.78,
            strength: 0.18,
            color: [0.015, 0.013, 0.02, 1.0],
        }),
    ])
}

/// The point of the map's marker `name`, an object with no class.
#[must_use]
pub fn marker(world: &World, name: &str) -> Option<Vec2> {
    markers(world, name).into_iter().next()
}

/// The points of every map marker named `name`, objects with no class, in
/// file order.
#[must_use]
pub fn markers(world: &World, name: &str) -> Vec<Vec2> {
    let Some(map) = world
        .get_resource::<TilemapRegistry>()
        .and_then(|maps| maps.get(MAP))
    else {
        return Vec::new();
    };
    map.object_layers
        .iter()
        .flat_map(|layer| &layer.objects)
        .filter(|object| object.class.is_empty() && object.name == name)
        .map(|object| object.position)
        .collect()
}

/// The map's markers where a fresh run sets a loose iron block down: each
/// a block's centre, on the mine's cut, a lightning rod.
pub const IRON_MARKER: &str = "iron";
/// The map's markers where a fresh run sets a pine crate and a powder keg
/// down: each a box's centre.
pub const CRATE_MARKER: &str = "crate";
pub const KEG_MARKER: &str = "keg";
