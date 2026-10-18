//! The kit's recipes: each genre fixture under `tests/fixtures/kit/`, a
//! folder of prefabs and data with no game Rust, run on the harness with
//! `KitPlugins::set()`; the coverage test over every registered scene name
//! and data kind; the kit determinism hash.

use std::path::PathBuf;

use glam::Vec2;
use tungsten_core::physics::Position;
use tungsten_core::{ActionMap, Binding, Config, Entity, KeyCode, PluginSet, Transform};

use crate::App;
use crate::kit::{
    AbilitiesPlugin, ActionInputPlugin, AimPlugin, CharacterAnimatorPlugin, ContactDamagePlugin,
    CuesPlugin, DepthSortPlugin, EffectsPlugin, FacingPlugin, FollowPlugin, HealthPlugin,
    HitPlugin, KitPlugins, KnockbackPlugin, LevelPlugin, LinearMotionPlugin, OrbitPlugin,
    OscillatorPlugin, PickupPlugin, ProjectilePlugin, SpeedLimitPlugin, StatsPlugin,
    StayInViewPlugin, SteeringPlugin, WrapPlugin,
};
use crate::testing::Harness;

mod coverage;
mod determinism;
mod grid;
mod platformer;
mod shmup;
mod top_down;

/// The four reference genres, each a fixture folder.
pub(crate) const GENRES: [&str; 4] = ["platformer", "top_down", "shmup", "grid"];

/// The two frame lengths every recipe runs at: one step a frame, and two.
pub(crate) const DTS: [f32; 2] = [1.0 / 60.0, 1.0 / 30.0];

/// The genre's fixture folder.
pub(crate) fn fixture_dir(genre: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/kit")
        .join(genre)
}

/// The actions the fixtures name, bound to keys so `press_action` presses
/// them, declared before the manifest loads so `action_input` checks pass.
fn declare_actions(app: &mut App) {
    let actions = app.world_mut().resource_mut::<ActionMap>();
    for (action, code) in [
        ("move_left", KeyCode::ArrowLeft),
        ("move_right", KeyCode::ArrowRight),
        ("move_up", KeyCode::ArrowUp),
        ("move_down", KeyCode::ArrowDown),
        ("jump", KeyCode::Space),
        ("attack", KeyCode::KeyJ),
        ("dash", KeyCode::ShiftLeft),
        ("fire", KeyCode::KeyZ),
    ] {
        actions.replace_bindings(action, vec![Binding::Key { code }]);
    }
}

/// A harness with `plugins` built after the engine's, the fixture actions
/// declared and the genre's manifest loaded, at dt `dt`.
pub(crate) fn recipe(genre: &str, plugins: PluginSet, dt: f32) -> Harness {
    let mut app = App::new(Config::default()).expect("App::new failed");
    app.add_plugins(plugins);
    declare_actions(&mut app);
    let mut harness = Harness::new(app);
    harness
        .load_manifest(fixture_dir(genre).join("manifest.json"))
        .unwrap_or_else(|e| panic!("{genre}: the fixture loads: {e}"));
    harness.set_dt(dt);
    harness
}

/// `KitPlugins::set()` with M64's motion and facing plugins built last, in
/// the reverse of their order in the set: the within-phase edges, not
/// registration order, must order them.
pub(crate) fn motion_reversed() -> PluginSet {
    KitPlugins::set()
        .without::<LinearMotionPlugin>()
        .without::<OscillatorPlugin>()
        .without::<SteeringPlugin>()
        .without::<FollowPlugin>()
        .without::<OrbitPlugin>()
        .without::<KnockbackPlugin>()
        .without::<SpeedLimitPlugin>()
        .without::<WrapPlugin>()
        .without::<StayInViewPlugin>()
        .without::<AimPlugin>()
        .without::<FacingPlugin>()
        .without::<CharacterAnimatorPlugin>()
        .without::<DepthSortPlugin>()
        .with(DepthSortPlugin)
        .with(CharacterAnimatorPlugin)
        .with(FacingPlugin)
        .with(AimPlugin)
        .with(StayInViewPlugin)
        .with(WrapPlugin)
        .with(SpeedLimitPlugin)
        .with(KnockbackPlugin)
        .with(OrbitPlugin)
        .with(FollowPlugin)
        .with(SteeringPlugin)
        .with(OscillatorPlugin)
        .with(LinearMotionPlugin)
}

/// `KitPlugins::set()` with the health, cue and M65 overlap plugins built
/// last, in the reverse of their order in the set: the overlap edges and
/// the despawn's, not registration order, must order them.
pub(crate) fn overlap_reversed() -> PluginSet {
    KitPlugins::set()
        .without::<HealthPlugin>()
        .without::<CuesPlugin>()
        .without::<HitPlugin>()
        .without::<ContactDamagePlugin>()
        .without::<ProjectilePlugin>()
        .without::<PickupPlugin>()
        .with(PickupPlugin)
        .with(ProjectilePlugin)
        .with(ContactDamagePlugin)
        .with(HitPlugin)
        .with(CuesPlugin)
        .with(HealthPlugin)
}

/// `KitPlugins::set()` with the health, stats, cue, hit, input, aim and
/// M67 effect, ability and level plugins built last, in the reverse of
/// their order in the set: the effect edges, not registration order, must
/// order them.
pub(crate) fn effects_reversed() -> PluginSet {
    KitPlugins::set()
        .without::<HealthPlugin>()
        .without::<StatsPlugin>()
        .without::<CuesPlugin>()
        .without::<HitPlugin>()
        .without::<ActionInputPlugin>()
        .without::<AimPlugin>()
        .without::<EffectsPlugin>()
        .without::<AbilitiesPlugin>()
        .without::<LevelPlugin>()
        .with(LevelPlugin)
        .with(AbilitiesPlugin)
        .with(EffectsPlugin)
        .with(AimPlugin)
        .with(ActionInputPlugin)
        .with(HitPlugin)
        .with(CuesPlugin)
        .with(StatsPlugin::new())
        .with(HealthPlugin)
}

/// Spawns the fixture's prefab `id` at `at`.
pub(crate) fn spawn(harness: &mut Harness, id: &str, at: [f32; 2]) -> Entity {
    let prepared = harness
        .world_mut()
        .prepare_prefab(id, at)
        .unwrap_or_else(|e| panic!("{id}: {e}"));
    prepared.spawn(harness.world_mut())
}

/// Where `entity`'s body is.
pub(crate) fn body_at(harness: &Harness, entity: Entity) -> Vec2 {
    harness.world().get::<Position>(entity).unwrap().0
}

/// Where `entity` is drawn.
pub(crate) fn drawn_at(harness: &Harness, entity: Entity) -> Vec2 {
    harness.world().get::<Transform>(entity).unwrap().position
}

/// The frames that cover `seconds` of game time at `dt`.
pub(crate) fn frames_for(seconds: f32, dt: f32) -> u32 {
    (seconds / dt).round() as u32
}

/// Writes `entity`'s `Position` and `PrevPosition` to `at`: a teleport, as
/// a game moves a body to a new place.
pub(crate) fn teleport(harness: &mut Harness, entity: Entity, at: Vec2) {
    use tungsten_core::physics::PrevPosition;
    let world = harness.world_mut();
    world.get_mut::<Position>(entity).unwrap().0 = at;
    if let Some(prev) = world.get_mut::<PrevPosition>(entity) {
        prev.0 = at;
    }
}

/// The entities whose `Tag` is `name`.
pub(crate) fn tagged(harness: &Harness, name: &str) -> Vec<Entity> {
    use tungsten_core::Tag;
    let mut found: Vec<Entity> = harness
        .world()
        .query::<(Entity, &Tag)>()
        .filter(|(_, tag)| tag.name == name)
        .map(|(entity, _)| entity)
        .collect();
    found.sort_by_key(|entity| entity.id());
    found
}

/// The 1-based fixed steps at which the kit's trace saw `event` about
/// `entity`.
pub(crate) fn traced_steps(harness: &Harness, entity: Entity, event: &str) -> Vec<u64> {
    use crate::kit::{KitTrace, TraceKind};
    harness
        .world()
        .resource::<KitTrace>()
        .iter()
        .filter(|entry| {
            entry.entity == entity
                && matches!(entry.kind, TraceKind::Event { name, .. } if name == event)
        })
        .map(|entry| entry.step + 1)
        .collect()
}

/// Where `entity` is: its `Position`, else its `Transform.position`.
pub(crate) fn point_of(harness: &Harness, entity: Entity) -> Vec2 {
    let world = harness.world();
    world.get::<Position>(entity).map_or_else(
        || world.get::<Transform>(entity).unwrap().position,
        |position| position.0,
    )
}
