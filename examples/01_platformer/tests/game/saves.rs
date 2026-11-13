//! Saves: a capture loads back to itself; every component a saved entity
//! carries is saved (the registry names it, or 01's ball table holds it),
//! rebuilt from its prefab or the map, or presentation, and every loaded
//! entity holds its uninterrupted twin's but a projectile's emitter; a
//! loaded fire missile and bullet spare their caster; a cast in the step
//! that asks for an autosave saves its cost and its missile; a save during
//! `casting` loads with the mana still paused; a loaded run keeps with the
//! run it was saved from, in a fresh harness and in a churned world, saved
//! mid-chain; statused dummies load with their effects' sources; a save
//! mid-conduction, mid-chain or mid-keg-chain resumes its chain; a frozen
//! block's waiting thermal shock and a kept block's waiting freeze run as
//! they would have; a lava touch keeps its time; a moved player saves the
//! same again and the camera frames it; a save the engine refuses loads as
//! a new run with nothing staged; the recorded spawns are the run's toys
//! and props and the kit's; a
//! bullet in flight loads and lands; a reaction toasts nothing; every
//! schema before the engine's saves is refused; an enemy's hit, pending
//! transition, touch and frozen flight load as they were, and a loaded run
//! with enemies mid-attack and mid-search keeps with the uninterrupted one;
//! a run holding tomes loads them with their books gone; and a full pit
//! saves within 2 MiB with the map's enemies alive.

use std::any::type_name as name;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::time::Instant;

use example_01_platformer::features::audio::components::{Sound, Sounded};
use example_01_platformer::features::dummy::components::Dummy;
use example_01_platformer::features::elements;
use example_01_platformer::features::elements::cascade::{self, CascadeKind, Cascades};
use example_01_platformer::features::elements::components::{ColdFace, Flame};
use example_01_platformer::features::enemies::components::{
    Enemy, EnemyKind, EnemyState, Throw, Throws,
};
use example_01_platformer::features::enemies::drops::{self, Orb, OrbKind};
use example_01_platformer::features::enemies::{self, snowball};
use example_01_platformer::features::gun;
use example_01_platformer::features::gun::components::{Bullet, Reload};
use example_01_platformer::features::level;
use example_01_platformer::features::level::elevator::{Elevator, Stop};
use example_01_platformer::features::level::geyser::{self, Geyser, GeyserPhase};
use example_01_platformer::features::level::lava::{self, Lava};
use example_01_platformer::features::lightning;
use example_01_platformer::features::player::components::{Player, StaffGlow};
use example_01_platformer::features::props::components::{
    Bomb, Crate, Keg, PropKind, Rock, Stone, StoneSize,
};
use example_01_platformer::features::props::{self, CRATE_HALF, KEG_HALF, STONE_HALVES};
use example_01_platformer::features::run::Run;
use example_01_platformer::features::spells::components::Spell;
use example_01_platformer::features::tomes::{Tome, TomeKind, Tomes};
use example_01_platformer::features::toys::components::{
    Ball, BlackHole, FireMissile, IronBlock, IronScrap, SmallBall,
};
use example_01_platformer::features::toys::{self, BALL_CAP, IRON_HALF};
use example_01_platformer::game;
use example_01_platformer::save::{self, MAP_ENEMIES, SaveGame};
use example_01_platformer::shared::{Joined, SaveId, Saved};
use example_01_platformer::shell::toast::Toasts;
use example_01_platformer::shell::{self as game_shell, Difficulty, RunStart, SlotState};
use glam::Vec2;
use tungsten::core::save::{SaveRegistry, Value};
use tungsten::core::{
    CameraRig, Entity, Position, PrevPosition, SaveData, SaveSlots, TilemapRegistry, Time,
    Transform, UserFolder, Velocity, World,
};
use tungsten::kit::{
    Abilities, Chemistry, ContactDamage, ContactTouch, DamageRequest, Damaged, Effects, Health,
    Hitbox, Modifier, Oscillator, PathFollower, Pickup, Projectile, Shield, StateMachine, Stats,
    Tags,
};
use tungsten::testing::UiHarness;

use super::{
    aim_at, calm, centre, choose, floor, ground, harness, harness_with, mana, pin_camera,
    player_entity, run, settle, stand, start, steps_this_frame, teleport, top, user_folder,
};
use super::{kind_body, player_half};

/// The sky's ground and the camera's corner over it.
const TOP: f32 = 1000.0;
const CORNER: Vec2 = Vec2::new(0.0, 500.0);

/// A save as JSON, to compare whole.
fn json(save: &SaveGame) -> serde_json::Value {
    serde_json::to_value(save).expect("a save serializes")
}

fn capture(harness: &UiHarness) -> SaveGame {
    save::capture(harness.world()).expect("a live run captures")
}

/// Loads `save` into `harness` as Continue does, its clock held so the
/// loaded world is the save's when the curtain lifts.
fn load(harness: &mut UiHarness, save: SaveGame) {
    harness.world_mut().resource_mut::<Time>().pause();
    game_shell::start_run(harness.world_mut(), RunStart::from(save), None);
    settle(harness);
    // The missiles land at the load frame's flush.
    harness.step(1);
    assert_eq!(top(harness), Some(game::GAMEPLAY));
}

/// Casts `spell` at `at`, `cast` held `frames` frames.
fn cast(harness: &mut UiHarness, spell: Spell, at: Vec2, frames: u32) {
    aim_at(harness, at);
    choose(harness, spell);
    harness.harness_mut().press_action("cast");
    harness.step(frames);
    harness.harness_mut().release_action("cast");
    harness.step(1);
}

/// A run on the sky's ground, the camera held over it, with a pit to the
/// player's left, x 100 to 500, whose walls keep a pile from leaving.
fn grounded() -> (UiHarness, Entity) {
    let (mut harness, player) = run();
    scaffold(&mut harness);
    stand(&mut harness, player, 600.0, TOP);
    harness.step(1);
    pin_camera(&mut harness, CORNER);
    (harness, player)
}

/// The test's ground and pit: the tests', not the level's, so a save has
/// none of them and a loaded run builds them again.
fn scaffold(harness: &mut UiHarness) {
    ground(harness, TOP);
    for x in [90.0, 510.0] {
        floor(harness, Vec2::new(x, TOP - 200.0), Vec2::new(10.0, 200.0));
    }
}

/// Lights the `count` highest small balls, the pile's crest.
fn light(harness: &mut UiHarness, count: usize) {
    let mut balls: Vec<(f32, u64, Entity)> = harness
        .world()
        .query::<(Entity, &SmallBall, &Position, &Saved)>()
        .map(|(e, _, at, id)| (at.0.y, id.id().0, e))
        .collect();
    balls.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    for (_, _, ball) in balls.into_iter().take(count) {
        elements::ignite(harness.world_mut(), ball);
    }
}

/// Mana enough for a busy script.
fn rich(harness: &mut UiHarness) {
    let player = player_entity(harness);
    let stats = harness.world_mut().get_mut::<Stats>(player).unwrap();
    stats.set_base("max_mana", 1000.0);
    stats.set_base("mana", 1000.0);
}

/// Every kind a save holds, by every spell and the gun: two iron blocks,
/// one frozen and shattered by a missile, the other chilled; a pit of 500
/// small balls, its crest burning; a bowling ball, a push, a hole, and a
/// missile and a bullet in flight; a mana orb; a crate alight, a keg on its
/// fuse and a goblin's bomb and rock in flight; and last a toxic flask in
/// flight, beside the map's enemies of every kind and its crates and kegs.
fn busy(harness: &mut UiHarness) {
    rich(harness);
    let iron = TOP - IRON_HALF.y - 10.0;
    cast(harness, Spell::IronBlock, Vec2::new(850.0, iron), 1);
    harness.step(30);
    cast(harness, Spell::IronBlock, Vec2::new(1150.0, iron), 1);
    harness.step(60);
    cast(harness, Spell::IceSpray, Vec2::new(850.0, TOP - 60.0), 100);
    cast(harness, Spell::FireMissile, Vec2::new(850.0, TOP - 60.0), 1);
    harness.step(30);
    cast(harness, Spell::ForcePush, Vec2::new(900.0, TOP - 100.0), 1);
    cast(harness, Spell::SmallBalls, Vec2::new(300.0, 500.0), 192);
    harness.step(120);
    // The flamethrower lights what its short cone reaches; the crest, over
    // the pit's wall, catches by hand.
    cast(
        harness,
        Spell::Flamethrower,
        Vec2::new(700.0, TOP - 20.0),
        30,
    );
    light(harness, 5);
    cast(
        harness,
        Spell::BowlingBall,
        Vec2::new(900.0, TOP - 100.0),
        1,
    );
    harness.step(60);
    cast(harness, Spell::BlackHole, Vec2::new(1700.0, 700.0), 1);
    harness.step(60);
    cast(harness, Spell::FireMissile, Vec2::new(700.0, 0.0), 1);
    harness.harness_mut().press_action(gun::SHOOT);
    harness.step(1);
    harness.harness_mut().release_action(gun::SHOOT);
    let far = harness
        .world()
        .query::<(Entity, &IronBlock, &Position)>()
        .max_by(|a, b| a.2.0.x.total_cmp(&b.2.0.x))
        .map(|(e, ..)| e)
        .unwrap();
    elements::chill(harness.world_mut(), far, 0.5, Vec2::new(1090.0, TOP - 60.0));
    // A mana orb past the player's reach.
    let world = harness.world_mut();
    drops::spawn(world, OrbKind::Mana, Vec2::new(1400.0, TOP - 60.0));
    // A crate alight and a keg on its fuse, both the player's, and a
    // goblin's bomb and rock in flight, east of the hole's reach, which
    // puts a fire out, over the sky's ground.
    let player = player_entity(harness);
    let me = example_01_platformer::shared::save_id(harness.world(), player).map(|id| id.0);
    let world = harness.world_mut();
    let crate_ = props::spawn(
        world,
        PropKind::Crate,
        Vec2::new(2150.0, TOP - CRATE_HALF.y - 0.5),
    );
    assert!(props::light(world, crate_, me), "the crate catches");
    let keg = props::spawn(
        world,
        PropKind::Keg,
        Vec2::new(2250.0, TOP - KEG_HALF.y - 0.5),
    );
    assert!(props::light(world, keg, me), "the keg's fuse lights");
    let bomb = props::spawn_bomb(world, Vec2::new(2200.0, 600.0), Some(0));
    world.get_mut::<Velocity>(bomb).unwrap().0 = Vec2::new(-100.0, -300.0);
    let rock = props::lob_rock(world, Vec2::new(2300.0, 500.0), Vec2::new(-200.0, -100.0));
    let throw = Throw {
        body: rock,
        thrower: 0,
        factor: 1.0,
        left: 2.0,
    };
    world.resource_mut::<Throws>().0.push(throw);
    elements::systems::arm_throw(world, &throw);
    // And a crawler's snowball high over the sky's ground.
    snowball::throw(
        world,
        Vec2::new(2350.0, 400.0),
        Vec2::new(-100.0, -200.0),
        Some(0),
        1.0,
    );
    // A crate broken into its planks (e01sfx).
    let broken = props::spawn(
        world,
        PropKind::Crate,
        Vec2::new(2050.0, TOP - CRATE_HALF.y - 0.5),
    );
    props::break_crate(world, broken, Vec2::new(-200.0, 0.0));
    // A rubble stone (e01hollow) at the ground's east end.
    props::spawn(
        world,
        PropKind::Stone(StoneSize::Medium),
        Vec2::new(2380.0, TOP - STONE_HALVES[1].y - 0.5),
    );
    harness.step(1);
    // A toxic flask in flight, lobbed high over the sky's ground (e01flask).
    cast(harness, Spell::ToxicFlask, Vec2::new(1500.0, 300.0), 1);
    assert_eq!(toys::flask::count(harness.world()), 1, "a flask flies");
}

/// The moving platforms' points, by save ID: the decks but the elevator's.
fn platform_points(harness: &UiHarness) -> Vec<[f32; 2]> {
    let mut points: Vec<(u64, [f32; 2])> = harness
        .world()
        .query_filtered::<(&Saved, &Position, &PathFollower), tungsten::core::Without<Elevator>>()
        .map(|(id, at, _)| (id.id().0, at.0.to_array()))
        .collect();
    points.sort_by_key(|&(id, _)| id);
    points.into_iter().map(|(_, at)| at).collect()
}

/// Whether a bullet is in flight.
fn bullets_flying(harness: &UiHarness) -> usize {
    harness
        .world()
        .query::<(&Bullet, &Projectile)>()
        .filter(|(_, projectile)| !projectile.is_spent())
        .count()
}

#[test]
fn a_capture_round_trips_through_a_load_with_a_platform_displaced() {
    let (mut harness, _) = grounded();
    let first = platform_points(&harness);
    busy(&mut harness);
    let moved = |before: &[[f32; 2]], after: &[[f32; 2]]| {
        before
            .iter()
            .zip(after)
            .any(|(a, b)| Vec2::from(*a).distance(Vec2::from(*b)) > 8.0)
    };
    assert!(
        moved(&first, &platform_points(&harness)),
        "a platform is off its spawn"
    );
    let world = harness.world();
    assert!(world.query::<&FireMissile>().count() > 0 && world.query::<&BlackHole>().count() > 0);
    assert!(bullets_flying(&harness) > 0, "a bullet in flight");
    assert!(world.query::<&IronScrap>().count() > 0);
    assert!(
        world.query::<(Entity, &IronBlock)>().any(|(block, _)| {
            elements::systems::coat_of(world, block) > 0.0
                && elements::ice_left(world, block).is_none()
        }),
        "an iron block chilled, not frozen"
    );
    let lit = |kind: PropKind| {
        props::all(world).into_iter().any(|prop| {
            props::kind(world, prop) == Some(kind)
                && (props::fuse_left(world, prop).is_some()
                    || props::fire(world, prop).is_some_and(|fire| fire.lit))
        })
    };
    assert!(lit(PropKind::Crate), "a crate alight");
    assert!(
        lit(PropKind::Keg) && lit(PropKind::Bomb),
        "a keg's and a bomb's fuse"
    );
    assert!(
        world
            .resource::<Throws>()
            .0
            .iter()
            .any(|throw| world.has::<Rock>(throw.body)),
        "a rock in flight, its throw kept"
    );
    let saved = capture(&harness);
    assert!(saved.balls.iter().any(|b| b.burn.is_some()));
    let mut fresh = super::harness();
    load(&mut fresh, saved.clone());
    assert_eq!(json(&capture(&fresh)), json(&saved));
}

/// Each type of `T`, by name.
fn names<const N: usize>(types: [&'static str; N]) -> Vec<&'static str> {
    types.to_vec()
}

/// What a kind of saved entity carries past what the save registry names:
/// the components 01's records hold for one of its own spawns (a kept
/// identity, whose holder the engine writes no record of), those its
/// prefab, the map or a load gives it again, and those that only draw it.
/// A named entity's or an engine record's component is saved when the
/// registry names it ([`registered`]).
struct Lists {
    recorded: Vec<&'static str>,
    rebuilt: Vec<&'static str>,
    presentation: Vec<&'static str>,
}

/// The payload's emitter, which a spawn from a source derives at the flush:
/// 01's records spawn their projectiles from the saved source again; an
/// engine record loads without it (the plan's §2.6).
const EMITTER: &str = "tungsten_kit::overlap::grid::Emitter";

/// The components an engine record's load does not give back.
fn lost(kind: &str) -> &'static [&'static str] {
    match kind {
        "missile" | "bullet" | "flask" => &[EMITTER],
        _ => &[],
    }
}

/// Each of `types` the registry saves as a component, by type name, and
/// the names it saves them under.
macro_rules! registered {
    ($registry:expr; $($ty:ty),* $(,)?) => {{
        let mut types = BTreeSet::new();
        let mut names = BTreeSet::new();
        $(
            if let Some(saved) = $registry.component_name::<$ty>() {
                types.insert(name::<$ty>());
                names.insert(saved.to_owned());
            }
        )*
        (types, names)
    }};
}

/// The component types the registry saves, by type name: every one 01's
/// saved entities could carry. It names three more, the kit's own
/// `became`, `carried` and `spawned`, whose types no game can name.
fn registered(world: &World) -> BTreeSet<&'static str> {
    use example_01_platformer::features::enemies::components::{
        Brain, Displaced, EnemyState, Holding, Sight, Snowball,
    };
    use example_01_platformer::features::player::components::Lantern;
    use example_01_platformer::features::props::components::{MapProp, Plank};
    use example_01_platformer::features::spells::components::Spellbook;
    use tungsten::core::{Collider, PrevPosition, RigidBody, Spin, Visibility};
    use tungsten::kit::{
        Aim, Cooldown, Counters, Delivery, Facing, Flags, Follow, Hurtbox, ImpactDamage, Knockback,
        Level, Lifetime, LinearMotion, Orbit, Pickup, PlatformerController, Projectile, Resist,
        Source, Spawner, Steering, TopDownMover,
    };
    let registry = world.resource::<SaveRegistry>();
    let (types, names) = registered!(registry;
        Transform, Visibility, Position, PrevPosition, Velocity, Spin, Collider, RigidBody,
        Abilities, Aim, ContactDamage, Cooldown, Counters, Delivery, Effects, Facing, Flags,
        Follow, Health, Hitbox, Hurtbox, ImpactDamage, Knockback, Level, Lifetime, LinearMotion, Orbit,
        Oscillator, PathFollower, Pickup, PlatformerController, Projectile, Resist, Shield,
        Source, Spawner, Stats, Steering, Tags, TopDownMover, Tomes, Elevator, Geyser, Lava,
        Spellbook, Lantern, Bullet, Reload, Enemy, Brain, Sight, StateMachine<EnemyState>,
        Displaced, Orb, Holding, BlackHole, IronScrap, MapProp, Plank, Snowball,
    );
    let unnamed: BTreeSet<&str> = registry
        .component_names()
        .filter(|saved| !names.contains(*saved))
        .collect();
    assert_eq!(
        unnamed,
        BTreeSet::from(["became", "carried", "spawned"]),
        "the census asks the registry of every type it saves"
    );
    types
}

fn lists(kind: &str) -> Lists {
    use example_01_platformer::features::level::components::{Glow, GlowParts, Hazard};
    use example_01_platformer::features::level::elevator::{CableLook, ElevatorDef};
    use example_01_platformer::features::level::lava::CrustLook;
    use example_01_platformer::features::player::components::Moves;
    use example_01_platformer::features::toys::components::{
        BallHue, BowlingBall, PreviousVelocity, ScrapTumble, SmallBall,
    };
    use tungsten::SceneEntity;
    use tungsten::core::{
        AnimationState, Collider, ParticleEmitter, ParticleEmitterState, RigidBody, Sprite,
        Velocity, Visibility,
    };

    use tungsten::kit::{
        ActionInput, Aim, CharacterAnimator, Children, ContactDamage, Cues, Effects, Facing,
        HitFlash, Hitbox, Hurtbox, Intent, Knockback, Stats,
    };
    let body = [name::<Collider>(), name::<RigidBody>()];
    let at = [name::<Saved>(), name::<Position>()];
    let drawn = [name::<Transform>(), name::<Sprite>(), name::<Visibility>()];
    match kind {
        "player" => Lists {
            recorded: Vec::new(),
            rebuilt: [
                &body[..],
                &[
                    name::<Player>(),
                    name::<SceneEntity>(),
                    name::<ActionInput>(),
                    name::<Intent>(),
                    name::<Aim>(),
                    name::<Knockback>(),
                    name::<Hurtbox>(),
                    name::<Cues>(),
                    name::<Glow>(),
                    name::<StaffGlow>(),
                ],
            ]
            .concat(),
            presentation: [
                &drawn[..],
                &[
                    name::<AnimationState>(),
                    name::<HitFlash>(),
                    name::<Moves>(),
                    name::<GlowParts>(),
                    name::<CharacterAnimator>(),
                    name::<Facing>(),
                    name::<CameraRig>(),
                ],
            ]
            .concat(),
        },
        "platform" => Lists {
            recorded: Vec::new(),
            rebuilt: [&body[..], &[name::<SceneEntity>()]].concat(),
            presentation: drawn.to_vec(),
        },
        "elevator" => Lists {
            recorded: Vec::new(),
            rebuilt: [&body[..], &[name::<SceneEntity>(), name::<ElevatorDef>()]].concat(),
            presentation: [&drawn[..], &[name::<CableLook>()]].concat(),
        },
        "hazard" => Lists {
            recorded: Vec::new(),
            rebuilt: names([
                name::<SceneEntity>(),
                name::<Hazard>(),
                name::<Glow>(),
                name::<Children>(),
            ]),
            presentation: [
                &drawn[..],
                &[
                    name::<AnimationState>(),
                    name::<CharacterAnimator>(),
                    name::<Facing>(),
                    name::<GlowParts>(),
                ],
            ]
            .concat(),
        },
        "ball" => Lists {
            recorded: [
                &at[..],
                &[
                    name::<Velocity>(),
                    name::<SmallBall>(),
                    name::<BowlingBall>(),
                    name::<BallHue>(),
                    name::<Flame>(),
                    name::<Effects>(),
                    name::<Tags>(),
                ],
            ]
            .concat(),
            rebuilt: [
                &body[..],
                &[
                    name::<Ball>(),
                    name::<SceneEntity>(),
                    name::<PreviousVelocity>(),
                    name::<tungsten::kit::ImpactDamage>(),
                ],
            ]
            .concat(),
            presentation: [&drawn[..], &[name::<AnimationState>(), name::<ColdFace>()]].concat(),
        },
        "block" => Lists {
            recorded: Vec::new(),
            rebuilt: [
                &body[..],
                &[
                    name::<IronBlock>(),
                    name::<SceneEntity>(),
                    name::<PreviousVelocity>(),
                    name::<Children>(),
                ],
            ]
            .concat(),
            presentation: names([
                name::<Transform>(),
                name::<Visibility>(),
                name::<ColdFace>(),
            ]),
        },
        "scrap" => Lists {
            recorded: Vec::new(),
            rebuilt: [
                &body[..],
                &[
                    name::<SceneEntity>(),
                    name::<PreviousVelocity>(),
                    name::<Children>(),
                ],
            ]
            .concat(),
            presentation: [&drawn[..], &[name::<ScrapTumble>(), name::<ColdFace>()]].concat(),
        },
        "black hole" => Lists {
            recorded: Vec::new(),
            rebuilt: names([
                name::<SceneEntity>(),
                name::<PreviousVelocity>(),
                name::<Children>(),
                name::<ParticleEmitter>(),
            ]),
            presentation: names([name::<Visibility>(), name::<ParticleEmitterState>()]),
        },
        "dummy" => Lists {
            recorded: Vec::new(),
            rebuilt: [
                &body[..],
                &[
                    name::<Dummy>(),
                    name::<SceneEntity>(),
                    name::<PreviousVelocity>(),
                    name::<Hurtbox>(),
                    name::<Knockback>(),
                    name::<Stats>(),
                    name::<Cues>(),
                ],
            ]
            .concat(),
            presentation: [&drawn[..], &[name::<HitFlash>()]].concat(),
        },
        "missile" => Lists {
            recorded: Vec::new(),
            rebuilt: names([
                name::<FireMissile>(),
                name::<Joined>(),
                name::<SceneEntity>(),
                name::<PreviousVelocity>(),
                name::<Glow>(),
                name::<Children>(),
            ]),
            presentation: [&drawn[..], &[name::<AnimationState>(), name::<GlowParts>()]].concat(),
        },
        "flask" => Lists {
            recorded: Vec::new(),
            rebuilt: names([
                name::<example_01_platformer::features::toys::components::ToxicFlask>(),
                name::<SceneEntity>(),
                name::<PreviousVelocity>(),
            ]),
            presentation: [&drawn[..], &[name::<AnimationState>()]].concat(),
        },
        "bullet" => Lists {
            recorded: Vec::new(),
            rebuilt: names([name::<SceneEntity>(), name::<Joined>(), name::<Hitbox>()]),
            presentation: drawn.to_vec(),
        },
        // The crates, kegs, bombs and rocks: a crate's or a rock's art, a
        // crate's fire (its fuel stat and its statuses), a keg's or a bomb's
        // fuse (its status), each saved whole, and whether the map set it
        // down; a rock's throw is the throws'; a keg's or a bomb's looks
        // by its tags are its prefab's.
        "crate" | "keg" | "bomb" | "rock" => Lists {
            recorded: Vec::new(),
            rebuilt: [
                &body[..],
                &[
                    name::<SceneEntity>(),
                    name::<PreviousVelocity>(),
                    name::<tungsten::kit::Looks>(),
                    name::<Crate>(),
                    name::<Keg>(),
                    name::<Bomb>(),
                    name::<Rock>(),
                ],
            ]
            .concat(),
            presentation: [&drawn[..], &[name::<ColdFace>()]].concat(),
        },
        // A rubble stone: its size and art, saved whole; it takes no frost.
        "stone" => Lists {
            recorded: Vec::new(),
            rebuilt: [
                &body[..],
                &[
                    name::<SceneEntity>(),
                    name::<PreviousVelocity>(),
                    name::<Stone>(),
                ],
            ]
            .concat(),
            presentation: drawn.to_vec(),
        },
        // A broken crate's planks: its art, its fire (its fuel stat and its
        // statuses) and its frost, each saved whole; their tumble is
        // presentation.
        "plank" => Lists {
            recorded: Vec::new(),
            rebuilt: [
                &body[..],
                &[name::<SceneEntity>(), name::<PreviousVelocity>()],
            ]
            .concat(),
            presentation: [&drawn[..], &[name::<ScrapTumble>(), name::<ColdFace>()]].concat(),
        },
        "snowball" => {
            use tungsten::core::AnimationState;
            Lists {
                recorded: Vec::new(),
                rebuilt: [
                    &body[..],
                    &[name::<SceneEntity>(), name::<PreviousVelocity>()],
                ]
                .concat(),
                presentation: [&drawn[..], &[name::<AnimationState>()]].concat(),
            }
        }
        "wisp" | "crawler" | "beetle" | "bat" | "troll" | "goblin" => {
            use example_01_platformer::features::enemies::components::Shown;

            Lists {
                recorded: Vec::new(),
                rebuilt: [
                    &body[..],
                    &[
                        name::<SceneEntity>(),
                        name::<PreviousVelocity>(),
                        name::<Hurtbox>(),
                        name::<ContactDamage>(),
                        name::<Cues>(),
                        name::<Glow>(),
                        name::<Children>(),
                    ],
                ]
                .concat(),
                presentation: [
                    &drawn[..],
                    &[
                        name::<AnimationState>(),
                        name::<HitFlash>(),
                        name::<GlowParts>(),
                        name::<Shown>(),
                    ],
                ]
                .concat(),
            }
        }
        "orb" => {
            use tungsten::kit::Pickup;
            Lists {
                recorded: Vec::new(),
                rebuilt: names([
                    name::<SceneEntity>(),
                    name::<Joined>(),
                    name::<Pickup>(),
                    name::<Cues>(),
                    name::<Glow>(),
                ]),
                presentation: [&drawn[..], &[name::<GlowParts>()]].concat(),
            }
        }
        // The lava basins, which the map spawns again whole but their chill
        // and crust, a crust's sprites drawn again.
        "basin" => Lists {
            recorded: Vec::new(),
            rebuilt: [
                &body[..],
                &[
                    name::<SceneEntity>(),
                    name::<Hazard>(),
                    name::<Cues>(),
                    name::<Glow>(),
                    name::<Children>(),
                ],
            ]
            .concat(),
            presentation: names([
                name::<Transform>(),
                name::<Visibility>(),
                name::<GlowParts>(),
                name::<CrustLook>(),
            ]),
        },
        // The geysers (e01hollow), which the map spawns again whole but
        // their cycle and choke, its touch on again in an eruption and its
        // plume and embers hung again.
        "geyser" => Lists {
            recorded: Vec::new(),
            rebuilt: names([name::<SceneEntity>(), name::<Children>()]),
            presentation: drawn.to_vec(),
        },
        other => panic!("{other}: a saved entity of no known kind"),
    }
}

/// The kind of saved entity `entity` is.
fn kind(world: &World, entity: Entity) -> &'static str {
    if world.has::<Player>(entity) {
        "player"
    } else if world.has::<Elevator>(entity) {
        "elevator"
    } else if world.has::<PathFollower>(entity) {
        "platform"
    } else if world.has::<Lava>(entity) {
        "basin"
    } else if world.has::<example_01_platformer::features::level::geyser::Geyser>(entity) {
        "geyser"
    } else if world.has::<Oscillator>(entity) {
        "hazard"
    } else if world.has::<Ball>(entity) {
        "ball"
    } else if world.has::<IronBlock>(entity) {
        "block"
    } else if world.has::<IronScrap>(entity) {
        "scrap"
    } else if world.has::<BlackHole>(entity) {
        "black hole"
    } else if world.has::<FireMissile>(entity) {
        "missile"
    } else if world.has::<example_01_platformer::features::toys::components::ToxicFlask>(entity) {
        "flask"
    } else if world.has::<Bullet>(entity) {
        "bullet"
    } else if world.has::<Dummy>(entity) {
        "dummy"
    } else if world.has::<Orb>(entity) {
        "orb"
    } else if world.has::<Crate>(entity) {
        "crate"
    } else if world.has::<Keg>(entity) {
        "keg"
    } else if world.has::<Bomb>(entity) {
        "bomb"
    } else if world.has::<Rock>(entity) {
        "rock"
    } else if world.has::<example_01_platformer::features::props::components::Plank>(entity) {
        "plank"
    } else if world.has::<Stone>(entity) {
        "stone"
    } else if world.has::<example_01_platformer::features::enemies::components::Snowball>(entity) {
        "snowball"
    } else if let Some(enemy) = world.get::<Enemy>(entity) {
        match enemy.kind {
            EnemyKind::Wisp => "wisp",
            EnemyKind::Crawler => "crawler",
            EnemyKind::Beetle => "beetle",
            EnemyKind::Bat => "bat",
            EnemyKind::Troll => "troll",
            EnemyKind::Goblin => "goblin",
        }
    } else {
        "unknown"
    }
}

#[test]
fn every_component_on_a_saved_entity_is_saved_rebuilt_or_presentation() {
    let (mut harness, _) = grounded();
    busy(&mut harness);
    let world = harness.world();
    assert!(
        world.query::<&SmallBall>().count() >= 500,
        "the pit holds 500"
    );
    let frozen = world
        .query::<&Tags>()
        .filter(|tags| tags.has("status.frozen"))
        .count();
    assert!(frozen + world.query::<&IronScrap>().count() > 0);
    assert!(world.query::<&Flame>().count() > 0);
    assert_eq!(world.query::<&BlackHole>().count(), 1);
    assert!(world.query::<&FireMissile>().count() > 0);
    let registered = registered(world);
    let mut seen: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    let mut wrong = BTreeSet::new();
    for (entity, saved) in world.query::<(Entity, &Saved)>() {
        let kind = kind(world, entity);
        let lists = lists(kind);
        // A named entity or an engine record saves what the registry
        // names; one of 01's own spawns, what its record holds.
        let engine = saved.prefab().is_some() || saved.id().is_authored();
        for (_, ty) in world.component_types(entity) {
            let count = |homes: &[&[&str]]| homes.iter().filter(|list| list.contains(&ty)).count();
            // The identity, and its point's last step, which every restore
            // writes with the point, are saved with the entity; so is what
            // the registry names on a named entity or an engine record.
            let structural = ty == name::<Saved>() || ty == name::<PrevPosition>();
            let homes = if structural || (engine && registered.contains(ty)) {
                1
            } else if engine {
                count(&[&lists.rebuilt, &lists.presentation, lost(kind)])
            } else {
                count(&[&lists.recorded, &lists.rebuilt, &lists.presentation])
            };
            if homes != 1 {
                let who = if engine { "the engine's" } else { "01's own" };
                wrong.insert(format!("{kind} ({who}): {ty} is in {homes} lists, not one"));
            }
            seen.entry(kind).or_default().insert(ty);
        }
    }
    assert!(wrong.is_empty(), "{wrong:#?}");
    let kinds: Vec<&str> = seen.keys().copied().collect();
    assert_eq!(
        kinds,
        [
            "ball",
            "basin",
            "bat",
            "beetle",
            "black hole",
            "block",
            "bomb",
            "bullet",
            "crate",
            "crawler",
            "dummy",
            "elevator",
            "flask",
            "geyser",
            "goblin",
            "keg",
            "missile",
            "orb",
            "plank",
            "platform",
            "player",
            "rock",
            "scrap",
            "snowball",
            "stone",
            "troll",
            "wisp"
        ]
    );
}

#[test]
fn every_loaded_entity_holds_its_uninterrupted_twins_components() {
    // A load gives each saved entity back what it carried, saved, rebuilt
    // or drawn, but a projectile's emitter, which a record's spawn from its
    // prefab cannot derive (the plan's §2.6).
    let (mut harness, _) = grounded();
    busy(&mut harness);
    let types = |world: &World| -> BTreeMap<u64, (&'static str, BTreeSet<&'static str>)> {
        world
            .query::<(Entity, &Saved)>()
            .map(|(entity, saved)| {
                let held = world
                    .component_types(entity)
                    .into_iter()
                    .map(|(_, ty)| ty)
                    .collect();
                (saved.id().0, (kind(world, entity), held))
            })
            .collect()
    };
    let saved = capture(&harness);
    let mut fresh = super::harness();
    load(&mut fresh, saved);
    scaffold(&mut fresh);
    pin_camera(&mut fresh, CORNER);
    fresh.world_mut().resource_mut::<Time>().resume();
    // One step on each run: what the kit and 01 set up at a spawn's first
    // step (a prefab's children, a flare's embers) is there on both.
    harness.step(1);
    fresh.step(1);
    let twins = types(harness.world());
    let loaded = types(fresh.world());
    let mut lacking: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for (id, (kind, held)) in &twins {
        let Some((_, back)) = loaded.get(id) else {
            lacking.entry(kind).or_default().insert("(the entity)");
            continue;
        };
        for ty in held.difference(back) {
            if *ty != EMITTER {
                lacking.entry(kind).or_default().insert(ty);
            }
        }
    }
    assert!(lacking.is_empty(), "{lacking:#?}");
}

#[test]
fn a_cast_in_the_step_that_asks_for_an_autosave_saves_its_cost_and_its_missile() {
    let dir = user_folder("same-frame");
    let mut harness = harness_with(UserFolder::At(dir.clone()));
    start(&mut harness);
    let player = player_entity(&harness);
    // The Lantern clearing ends at x 5,952; the aqueduct starts there. A
    // floor in the empty sky over the border.
    floor(
        &mut harness,
        Vec2::new(5952.0, TOP + 50.0),
        Vec2::new(600.0, 50.0),
    );
    stand(&mut harness, player, 5800.0, TOP);
    harness.step(1);
    pin_camera(&mut harness, Vec2::new(5400.0, CORNER.y));
    aim_at(&mut harness, Vec2::new(6200.0, 600.0));
    choose(&mut harness, Spell::FireMissile);
    assert_eq!(mana(&harness), 100.0);
    // One step casts and walks into the aqueduct, whose entry asks for an
    // autosave; the next frame's capture takes both.
    let eye = centre(&harness, player);
    teleport(&mut harness, player, Vec2::new(6000.0, eye.y));
    harness.harness_mut().press_action("cast");
    harness.step(1);
    harness.harness_mut().release_action("cast");
    harness.step(1);
    // The newest autosave: the aqueduct's, after the run's first.
    let (slots, _) = game_shell::read_slots(harness.world_mut());
    let Some((_, saved)) = game_shell::newest(&slots) else {
        panic!("the autosave is written");
    };
    assert_eq!(saved.area.as_deref(), Some("Broken aqueduct"));
    let mut fresh = super::harness();
    load(&mut fresh, saved.clone());
    assert_eq!(mana(&fresh), 96.0, "its cost");
    assert_eq!(
        fresh.world().query::<&FireMissile>().count(),
        1,
        "and its missile"
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_save_during_casting_loads_with_the_mana_still_paused() {
    let (mut harness, player) = run();
    // The staff light off, which would burn through the pause.
    harness
        .world_mut()
        .get_mut::<example_01_platformer::features::player::components::Lantern>(player)
        .unwrap()
        .on = false;
    cast(&mut harness, Spell::ForcePush, Vec2::new(800.0, 400.0), 1);
    let spent = mana(&harness);
    assert!(spent < 88.0, "a push's 12.5 spent: {spent}");
    let casting = |tags: &Tags| tags.iter().any(|tag| tag == "casting");
    assert!(casting(harness.world().get::<Tags>(player).unwrap()));
    let saved = capture(&harness);
    let mut fresh = super::harness();
    load(&mut fresh, saved);
    let player = player_entity(&fresh);
    assert!(casting(fresh.world().get::<Tags>(player).unwrap()));
    assert_eq!(mana(&fresh), spent);
    fresh.world_mut().resource_mut::<Time>().resume();
    fresh.step(30);
    assert_eq!(mana(&fresh), spent, "paused for the rest of the window");
    fresh.step(60);
    assert!(mana(&fresh) > spent, "then it regenerates");
}

/// What two runs are compared by: each saved entity's point by save ID,
/// what burns and what is frozen, balls and iron alike, each dummy's
/// statuses, the cascades waiting, and the player's health, mana and
/// cooldowns.
struct Outcome {
    points: BTreeMap<u64, (&'static str, Vec2)>,
    burning: BTreeSet<u64>,
    frozen: BTreeSet<u64>,
    statuses: BTreeMap<u64, Vec<String>>,
    queue: Vec<(CascadeKind, Option<u64>, u64)>,
    health: f32,
    mana: f32,
    cooldowns: Vec<f32>,
    player: Vec2,
}

/// The cascades waiting: each one's kind, target and due run, by due run,
/// then target save ID, as their runs order them past ties in distance.
fn queue(harness: &UiHarness) -> Vec<(CascadeKind, Option<u64>, u64)> {
    let mut waiting: Vec<(CascadeKind, Option<u64>, u64)> = harness
        .world()
        .resource::<Cascades>()
        .entries
        .iter()
        .map(|entry| (entry.kind, entry.target, entry.due))
        .collect();
    waiting.sort_by_key(|&(kind, target, due)| (due, target, kind as u8));
    waiting
}

/// A frozen pile of `count` small balls on the sky's ground from `x`, 16 px
/// apart in rows of twelve.
fn ice_pile(harness: &mut UiHarness, x: f32, count: usize) -> Vec<Entity> {
    (0..count)
        .map(|i| {
            let at = Vec2::new(
                x + (i % 12) as f32 * 16.0,
                TOP - 8.0 - (i / 12) as f32 * 16.0,
            );
            let ball = toys::spawn_ball(harness.world_mut(), at, true);
            elements::freeze_iron(harness.world_mut(), ball);
            ball
        })
        .collect()
}

fn outcome(harness: &UiHarness) -> Outcome {
    let world = harness.world();
    let player = player_entity(harness);
    let mut points = BTreeMap::new();
    for (entity, id) in world.query::<(Entity, &Saved)>() {
        let at = world
            .get::<Position>(entity)
            .map(|p| p.0)
            .or_else(|| world.get::<Transform>(entity).map(|t| t.position))
            .unwrap_or_default();
        points.insert(id.id().0, (kind(world, entity), at));
    }
    let id = |entity: Entity| {
        example_01_platformer::shared::save_id(world, entity)
            .unwrap()
            .0
    };
    let abilities = world.get::<Abilities>(player).unwrap();
    Outcome {
        points,
        burning: world
            .query::<(Entity, &Flame, &Tags)>()
            .filter(|(_, _, tags)| tags.has("status.burning"))
            .map(|(e, ..)| id(e))
            .collect(),
        frozen: world
            .query::<(Entity, &Tags)>()
            .filter(|(e, tags)| tags.has("status.frozen") && world.get::<Health>(*e).is_none())
            .map(|(e, _)| id(e))
            .collect(),
        statuses: world
            .query::<(Entity, &Dummy, &Tags)>()
            .map(|(e, _, tags)| {
                let statuses = tags
                    .iter()
                    .filter(|tag| tag.starts_with("status."))
                    .map(str::to_owned)
                    .collect();
                (id(e), statuses)
            })
            .collect(),
        queue: queue(harness),
        health: world.get::<Health>(player).unwrap().current(),
        mana: mana(harness),
        cooldowns: abilities
            .names()
            .map(|name| abilities.cooldown_left(name).unwrap_or(0.0))
            .collect(),
        player: centre(harness, player),
    }
}

/// The scripted three seconds both runs play after the save.
fn script(harness: &mut UiHarness) {
    let input = |harness: &mut UiHarness, action: &str, down: bool| {
        if down {
            harness.harness_mut().press_action(action);
        } else {
            harness.harness_mut().release_action(action);
        }
    };
    input(harness, "move_right", true);
    harness.step(15);
    input(harness, "move_right", false);
    input(harness, "jump", true);
    harness.step(2);
    input(harness, "jump", false);
    harness.step(40);
    cast(harness, Spell::IceSpray, Vec2::new(950.0, TOP - 60.0), 91);
    // Up, clear of the iron and the pile.
    cast(harness, Spell::FireMissile, Vec2::new(700.0, 200.0), 1);
    harness.step(30);
}

fn divergence(churned: bool) {
    let (mut uninterrupted, _) = grounded();
    // Mid-play in the Lantern clearing: iron to the right, a settled pile
    // in the pit to the left, its crest alight.
    cast(
        &mut uninterrupted,
        Spell::IronBlock,
        Vec2::new(950.0, TOP - IRON_HALF.y - 10.0),
        1,
    );
    cast(
        &mut uninterrupted,
        Spell::SmallBalls,
        Vec2::new(300.0, 500.0),
        90,
    );
    uninterrupted.step(180);
    light(&mut uninterrupted, 5);
    // A chain mid-run: a frozen pile to the right, its edge's thermal shock
    // two rings in; the clearing's dummy shocked by the player.
    let pile = ice_pile(&mut uninterrupted, 1300.0, 48);
    let edge = super::body(&uninterrupted, pile[0]);
    elements::thermal_shatter(uninterrupted.world_mut(), edge, 72.0);
    let player = player_entity(&uninterrupted);
    let dummy = uninterrupted
        .world()
        .query::<(Entity, &Dummy)>()
        .map(|(e, _)| e)
        .next()
        .unwrap();
    super::combat::apply(&mut uninterrupted, dummy, "shocked", Some(player));
    uninterrupted.step(2);
    // Saved mid-chain: its endings wait in chemistry's queue, which the
    // save holds.
    let pending = |harness: &UiHarness| harness.world().resource::<Chemistry>().pending();
    let shocked = uninterrupted
        .world()
        .query::<(&Dummy, &Tags)>()
        .any(|(_, tags)| tags.has("status.shocked"));
    assert!(shocked, "the dummy shocked");
    let saved = capture(&uninterrupted);
    assert_eq!(saved.area.as_deref(), Some("Lantern clearing"));
    assert!(pending(&uninterrupted) > 0, "mid-chain");
    let mut loaded = harness();
    if churned {
        let world = loaded.world_mut();
        let spawned: Vec<Entity> = (0..10_000).map(|_| world.spawn()).collect();
        for entity in spawned {
            world.despawn(entity);
        }
    }
    load(&mut loaded, saved.clone());
    assert_eq!(json(&capture(&loaded)), json(&saved), "the run whole");
    scaffold(&mut loaded);
    pin_camera(&mut loaded, CORNER);
    loaded.world_mut().resource_mut::<Time>().resume();
    script(&mut uninterrupted);
    script(&mut loaded);
    let (a, b) = (outcome(&uninterrupted), outcome(&loaded));
    assert!(!a.frozen.is_empty(), "the script freezes the iron");
    assert!(a.burning.len() > 5, "and the fire spreads through the pile");
    let worst = |kinds: &[&str]| {
        a.points
            .iter()
            .filter(|(_, (kind, _))| kinds.contains(kind))
            .map(|(id, (_, at))| at.distance(b.points[id].1))
            .fold(0.0_f32, f32::max)
    };
    println!(
        "divergence (churned {churned}): {} alive, {} burning, {} frozen; player {:.2} px, platforms and hazards {:.3} px, iron {:.2} px, mana {} and {}",
        a.points.len(),
        a.burning.len(),
        a.frozen.len(),
        a.player.distance(b.player),
        worst(&["platform", "hazard"]),
        worst(&["block", "scrap"]),
        a.mana,
        b.mana
    );
    assert_eq!(
        a.points.keys().collect::<Vec<_>>(),
        b.points.keys().collect::<Vec<_>>(),
        "the same save IDs live"
    );
    assert_eq!(a.burning, b.burning, "the same balls burn");
    assert_eq!(a.frozen, b.frozen, "the same balls and iron are frozen");
    assert_eq!(a.statuses, b.statuses, "the dummies' statuses");
    assert_eq!(a.queue, b.queue, "the same cascades wait");
    assert_eq!(a.health, b.health);
    assert!(
        (a.mana - b.mana).abs() <= 1.5,
        "mana {} and {}",
        a.mana,
        b.mana
    );
    for (x, y) in a.cooldowns.iter().zip(&b.cooldowns) {
        assert!((x - y).abs() <= 1.0 / 60.0 + 1e-4, "cooldowns {x} and {y}");
    }
    for (id, (kind, at)) in &a.points {
        let (_, other) = b.points[id];
        let limit = match *kind {
            "platform" | "hazard" => 0.5,
            "block" | "scrap" => 8.0,
            _ => continue,
        };
        assert!(at.distance(other) <= limit, "{kind} {id}: {at} and {other}");
    }
    assert!(
        a.player.distance(b.player) <= 12.0,
        "the player: {} and {}",
        a.player,
        b.player
    );
}

#[test]
fn a_loaded_run_keeps_with_the_uninterrupted_one_from_a_fresh_harness() {
    divergence(false);
}

#[test]
fn a_loaded_run_keeps_with_the_uninterrupted_one_in_a_churned_world() {
    divergence(true);
}

/// An enemy of `kind` with its body's centre at `centre`, at Normal, its
/// save ID the run's next.
fn enemy(harness: &mut UiHarness, kind: EnemyKind, centre: Vec2) -> Entity {
    let world = harness.world_mut();
    let entity = enemies::spawn(world, kind, centre - kind_body(kind).centre).expect("its prefab");
    enemies::scale(world, entity, Difficulty::Normal);
    entity
}

/// The entity of save ID `id`.
fn by_id(harness: &UiHarness, id: u64) -> Entity {
    harness
        .world()
        .query::<(Entity, &Saved)>()
        .find(|&(_, own)| own.id().0 == id)
        .map(|(entity, _)| entity)
        .expect("a live entity of that save ID")
}

fn id_of(harness: &UiHarness, entity: Entity) -> u64 {
    example_01_platformer::shared::save_id(harness.world(), entity)
        .expect("a save ID")
        .0
}

fn state(harness: &UiHarness, enemy: Entity) -> EnemyState {
    harness
        .world()
        .get::<StateMachine<EnemyState>>(enemy)
        .expect("a machine")
        .state()
}

/// Steps one frame at a time, at most `frames`, until `done`; whether it
/// came.
fn until(harness: &mut UiHarness, frames: u32, mut done: impl FnMut(&UiHarness) -> bool) -> bool {
    for _ in 0..frames {
        harness.step(1);
        if done(harness) {
            return true;
        }
    }
    false
}

/// The frames, of the next `frames`, in which `source` hits `target`.
fn hit_frames(harness: &mut UiHarness, target: u64, source: u64, frames: u32) -> Vec<u32> {
    let mut hits = Vec::new();
    for frame in 0..frames {
        harness.step(1);
        let (target, source) = (by_id(harness, target), by_id(harness, source));
        let hit = harness
            .harness()
            .events::<Damaged>()
            .any(|hit| hit.entity == target && hit.source == Some(source));
        if hit {
            hits.push(frame);
        }
    }
    hits
}

/// A fresh game that loads `saved` as Continue does, its clock held until
/// the test's ground and camera are back, a world churned first when
/// `churned`.
fn reloaded(saved: SaveGame, churned: bool) -> UiHarness {
    let mut loaded = harness();
    if churned {
        let world = loaded.world_mut();
        let spawned: Vec<Entity> = (0..10_000).map(|_| world.spawn()).collect();
        for entity in spawned {
            world.despawn(entity);
        }
    }
    load(&mut loaded, saved);
    scaffold(&mut loaded);
    pin_camera(&mut loaded, CORNER);
    loaded.world_mut().resource_mut::<Time>().resume();
    loaded
}

#[test]
fn a_save_after_an_attack_has_hit_loads_without_a_second_hit() {
    let (mut harness, player) = grounded();
    let beetle = enemy(
        &mut harness,
        EnemyKind::Beetle,
        Vec2::new(880.0, TOP - 13.5),
    );
    let (player_id, beetle_id) = (id_of(&harness, player), id_of(&harness, beetle));
    // Its charge passes through the player, its hitbox hitting once.
    let struck = until(&mut harness, 400, |h| {
        h.harness()
            .events::<Damaged>()
            .any(|hit| hit.entity == player && hit.source == Some(beetle))
    });
    assert!(struck, "the charge hits the player");
    let hitbox = harness.world().get::<Hitbox>(beetle).expect("its hitbox");
    assert_eq!(hitbox.hit_targets(), [player], "its activation's target");
    assert!(hitbox.is_armed(), "saved mid-window");
    let saved = capture(&harness);
    let mut loaded = reloaded(saved, false);
    for (run, harness) in [("uninterrupted", &mut harness), ("loaded", &mut loaded)] {
        let again = hit_frames(harness, player_id, beetle_id, 50);
        assert!(again.is_empty(), "{run}: no second hit: {again:?}");
    }
}

#[test]
fn a_save_with_a_pending_transition_enters_it_once_after_the_load() {
    let (mut harness, player) = grounded();
    harness
        .world_mut()
        .get_mut::<example_01_platformer::features::player::components::Lantern>(player)
        .unwrap()
        .on = false;
    let crawler = enemy(
        &mut harness,
        EnemyKind::Crawler,
        Vec2::new(800.0, TOP - 10.5),
    );
    let crawler_id = id_of(&harness, crawler);
    // Its snowball cooling a minute, so its tell is the lunge's.
    harness
        .world_mut()
        .get_mut::<example_01_platformer::features::enemies::components::Brain>(crawler)
        .unwrap()
        .cooldown = 60.0;
    let asked = until(&mut harness, 400, |h| {
        h.world()
            .get::<StateMachine<EnemyState>>(crawler)
            .is_some_and(|machine| machine.pending() == Some(EnemyState::Attack))
    });
    assert!(asked, "its tell asked for the lunge");
    let machine = harness
        .world()
        .get::<StateMachine<EnemyState>>(crawler)
        .expect("its machine");
    assert_eq!(machine.state(), EnemyState::WindUp);
    assert_eq!(machine.pending(), Some(EnemyState::Attack));
    let saved = capture(&harness);
    let mut loaded = reloaded(saved, false);
    let (mut hisses, mut lunges) = (0, 0);
    for _ in 0..10 {
        loaded.step(1);
        for sounded in loaded.harness().events::<Sounded>() {
            match sounded.sound {
                Sound::CrawlerHiss => hisses += 1,
                Sound::CrawlerLunge => lunges += 1,
                _ => {}
            }
        }
    }
    assert_eq!(
        (hisses, lunges),
        (0, 1),
        "the lunge once, the tell not again"
    );
    let crawler = by_id(&loaded, crawler_id);
    assert_eq!(state(&loaded, crawler), EnemyState::Attack);
}

#[test]
fn a_cooling_a_dashing_and_a_panicking_goblin_save_and_load_back_the_same() {
    use example_01_platformer::features::enemies::components::{Brain, Strike};
    let (mut harness, _) = grounded();
    harness.harness_mut().set_dt(1.0 / 60.0);
    let goblin = |harness: &mut UiHarness, x: f32| {
        enemy(harness, EnemyKind::Goblin, Vec2::new(x, TOP - 20.5))
    };
    // Within 1,200 px of the player at x 600, awake.
    let (cooling, dashing, panicking) = (
        goblin(&mut harness, 1000.0),
        goblin(&mut harness, 1300.0),
        goblin(&mut harness, 1600.0),
    );
    harness.step(2);
    let world = harness.world_mut();
    world.get_mut::<Brain>(cooling).unwrap().cooldown = 1.5;
    world.get_mut::<Brain>(dashing).unwrap().strike = Some(Strike::Dash);
    world.insert(dashing, StateMachine::new(EnemyState::Attack));
    tungsten::kit::EffectRequest::new(panicking, tungsten::core::DataRef::by_id("burning"))
        .apply(world);
    harness.step(3);
    let ids = [cooling, dashing, panicking].map(|goblin| id_of(&harness, goblin));
    let brain =
        |harness: &UiHarness, goblin: Entity| *harness.world().get::<Brain>(goblin).unwrap();
    assert!(brain(&harness, cooling).cooldown > 0.0, "cooling");
    assert_eq!(
        (state(&harness, dashing), brain(&harness, dashing).strike),
        (EnemyState::Attack, Some(Strike::Dash)),
        "dashing"
    );
    assert_eq!(state(&harness, panicking), EnemyState::Panic);
    let before = ids.map(|id| enemy_parts(&harness, id));
    let saved = capture(&harness);
    let read: SaveGame = serde_json::from_value(json(&saved)).expect("the save reads");
    let loaded = reloaded(read, false);
    for (id, before) in ids.into_iter().zip(before) {
        assert_eq!(enemy_parts(&loaded, id), before, "{id} back the same");
    }
}

/// The parts of the enemy of save ID `id` a save keeps, as JSON: its body,
/// its drivers, its machine and what it knows, its health, stats,
/// resistances, effects, tags, abilities, knockback and hitbox; none of its
/// resistances, effects or tags reads as an empty one, as a load gives it.
fn enemy_parts(harness: &UiHarness, id: u64) -> serde_json::Value {
    use example_01_platformer::features::enemies::components::{Brain, Displaced, Sight};
    use tungsten::kit::{Knockback, PlatformerController, Resist, TopDownMover};
    let world = harness.world();
    let enemy = by_id(harness, id);
    serde_json::json!({
        "position": world.get::<Position>(enemy).map(|at| at.0.to_array()),
        "velocity": world.get::<Velocity>(enemy).map(|v| v.0.to_array()),
        "controller": world.get::<PlatformerController>(enemy),
        "mover": world.get::<TopDownMover>(enemy),
        "displaced": world.get::<Displaced>(enemy).map(|displaced| &displaced.0),
        "machine": world.get::<StateMachine<EnemyState>>(enemy),
        "enemy": world.get::<Enemy>(enemy),
        "brain": world.get::<Brain>(enemy),
        "sight": world.get::<Sight>(enemy),
        "health": world.get::<Health>(enemy),
        "stats": world.get::<Stats>(enemy),
        "resist": world.get::<Resist>(enemy).cloned().unwrap_or_default(),
        "effects": world.get::<Effects>(enemy).cloned().unwrap_or_default(),
        "tags": world.get::<Tags>(enemy).cloned().unwrap_or_default(),
        "abilities": world.get::<Abilities>(enemy),
        "knockback": world.get::<Knockback>(enemy),
        "hitbox": world.get::<Hitbox>(enemy),
    })
}

/// Steps `harness` until a snowball bursts, at most 120 frames: the fixed
/// steps that took and where it burst.
fn snowball_landing(harness: &mut UiHarness) -> Option<(u32, Vec2)> {
    let mut steps = 0;
    for _ in 0..120 {
        harness.step(1);
        steps += steps_this_frame(harness);
        let burst = harness
            .harness()
            .events::<Sounded>()
            .find(|sounded| sounded.sound == Sound::SnowballBurst)
            .map(|sounded| sounded.at);
        if let Some(at) = burst {
            return Some((steps, at));
        }
    }
    None
}

/// The snowballs in flight: each one's save ID, point, velocity, thrower,
/// damage factor and seconds left.
type Flight = (u64, [f32; 2], [f32; 2], Option<u64>, f32, f32);

fn snowballs(harness: &UiHarness) -> Vec<Flight> {
    use example_01_platformer::features::enemies::components::Snowball;
    let world = harness.world();
    snowball::flying(world)
        .into_iter()
        .map(|(id, ball)| {
            let flying = world.get::<Snowball>(ball).unwrap();
            (
                id,
                world.get::<Position>(ball).unwrap().0.to_array(),
                world.get::<Velocity>(ball).unwrap().0.to_array(),
                flying.thrower,
                flying.factor,
                flying.left,
            )
        })
        .collect()
}

#[test]
fn a_snowball_in_flight_and_a_cooling_crawler_save_and_load_and_it_lands_as_it_would_have() {
    let (mut harness, player) = grounded();
    harness.harness_mut().set_dt(1.0 / 60.0);
    harness
        .world_mut()
        .get_mut::<example_01_platformer::features::player::components::Lantern>(player)
        .unwrap()
        .on = false;
    let crawler = enemy(
        &mut harness,
        EnemyKind::Crawler,
        Vec2::new(600.0 + 260.0, TOP - 10.5),
    );
    enemies::turn(harness.world_mut(), crawler, -1.0);
    let crawler_id = id_of(&harness, crawler);
    let thrown = until(&mut harness, 240, |h| {
        !snowball::flying(h.world()).is_empty()
    });
    assert!(thrown, "its snowball flies");
    harness.step(3);
    let flying = snowballs(&harness);
    assert_eq!(flying.len(), 1, "the snowball flies");
    let cooling = harness
        .world()
        .get::<example_01_platformer::features::enemies::components::Brain>(crawler)
        .unwrap()
        .cooldown;
    assert!(cooling > 0.0, "its cooldown: {cooling}");
    let saved = capture(&harness);
    let read: SaveGame = serde_json::from_value(json(&saved)).expect("the save reads");
    let mut loaded = reloaded(read, false);
    loaded.harness_mut().set_dt(1.0 / 60.0);
    assert_eq!(snowballs(&loaded), flying, "the snowball back");
    let brain = loaded
        .world()
        .get::<example_01_platformer::features::enemies::components::Brain>(by_id(
            &loaded, crawler_id,
        ))
        .copied()
        .unwrap();
    assert_eq!(brain.cooldown, cooling, "its cooldown back");
    let (before, after) = (
        snowball_landing(&mut harness),
        snowball_landing(&mut loaded),
    );
    let (steps, at) = before.expect("it lands");
    let (loaded_steps, loaded_at) = after.expect("it lands after the load");
    assert_eq!(loaded_steps, steps, "as soon");
    assert!(at.distance(loaded_at) < 0.5, "{at} and {loaded_at}");
}

#[test]
fn a_save_mid_touch_keeps_the_touch_schedule() {
    let (mut harness, player) = grounded();
    let at = centre(&harness, player);
    let wisp = enemy(&mut harness, EnemyKind::Wisp, at);
    let (player_id, wisp_id) = (id_of(&harness, player), id_of(&harness, wisp));
    let touched = until(&mut harness, 120, |h| {
        h.harness()
            .events::<Damaged>()
            .any(|hit| hit.entity == player && hit.source == Some(wisp))
    });
    assert!(touched, "its touch burns the player");
    harness.step(15);
    let touches: Vec<ContactTouch> = harness
        .world()
        .get::<ContactDamage>(wisp)
        .expect("its touch")
        .touches()
        .collect();
    assert_eq!(touches.len(), 1, "its touch's times: {touches:?}");
    assert_eq!(touches[0].target(), player);
    let saved = capture(&harness);
    let mut loaded = reloaded(saved, false);
    let before = hit_frames(&mut harness, player_id, wisp_id, 80);
    let after = hit_frames(&mut loaded, player_id, wisp_id, 80);
    assert!(
        !before.is_empty() && before[0] > 0,
        "not at once: {before:?}"
    );
    assert_eq!(before, after, "the same touches, when they would have come");
}

#[test]
fn a_flyer_saved_frozen_mid_flight_thaws_into_its_saved_flight() {
    use example_01_platformer::features::enemies::components::Displaced;
    use tungsten::kit::{PlatformerController, TopDownMover};
    let (mut harness, _) = grounded();
    let wisp = enemy(&mut harness, EnemyKind::Wisp, Vec2::new(1300.0, 600.0));
    let wisp_id = id_of(&harness, wisp);
    harness.step(40);
    let flying = harness
        .world()
        .get::<TopDownMover>(wisp)
        .unwrap()
        .direction();
    assert!(flying.length() > 0.05, "in flight: {flying}");
    super::combat::apply(&mut harness, wisp, "frozen", None);
    harness.step(10);
    let world = harness.world();
    assert!(
        world.get::<TopDownMover>(wisp).is_none()
            && world.get::<PlatformerController>(wisp).is_some(),
        "it falls"
    );
    let displaced = world
        .get::<Displaced>(wisp)
        .map(|displaced| displaced.0.clone())
        .expect("its flight set aside");
    assert_eq!(displaced.direction(), flying);
    let saved = capture(&harness);
    let mut loaded = reloaded(saved, false);
    let wisp = by_id(&loaded, wisp_id);
    let world = loaded.world();
    // Its flight as the save holds it: the kit's mover form leaves out
    // whether its first step ran, so a loaded mover's first step is still.
    let flight = |mover: &TopDownMover| (mover.direction(), mover.velocity());
    assert_eq!(
        world.get::<Displaced>(wisp).map(|d| flight(&d.0)),
        Some(flight(&displaced))
    );
    assert!(world.get::<PlatformerController>(wisp).is_some());
    let thaw = |harness: &mut UiHarness| -> (TopDownMover, Vec2) {
        let wisp = by_id(harness, wisp_id);
        let thawed = until(harness, 480, |h| {
            h.world().get::<TopDownMover>(wisp).is_some()
        });
        assert!(thawed, "it thaws");
        let mover = harness.world().get::<TopDownMover>(wisp).unwrap().clone();
        harness.step(30);
        (mover, super::body(harness, wisp))
    };
    let (before, after) = (thaw(&mut harness), thaw(&mut loaded));
    assert_eq!(
        before.0.direction(),
        after.0.direction(),
        "the same flight back at the thaw"
    );
    assert!(
        before.1.distance(after.1) < 2.0,
        "flying on alike: {} and {}",
        before.1,
        after.1
    );
}

/// The enemies' scripted seconds both runs play after the save: the
/// player stands, steps right, then stands.
fn enemy_script(harness: &mut UiHarness) {
    harness.step(60);
    harness.harness_mut().press_action("move_right");
    harness.step(15);
    harness.harness_mut().release_action("move_right");
    harness.step(75);
}

fn enemy_divergence(churned: bool) {
    use example_01_platformer::features::enemies::components::Brain;
    let (mut uninterrupted, player) = grounded();
    let bat = enemy(&mut uninterrupted, EnemyKind::Bat, Vec2::new(900.0, 700.0));
    let wisp = enemy(
        &mut uninterrupted,
        EnemyKind::Wisp,
        Vec2::new(1500.0, 700.0),
    );
    // The wisp lost the player at x 1250, past its sight: it searches there.
    let world = uninterrupted.world_mut();
    world.get_mut::<Brain>(wisp).unwrap().last_seen = Some([1250.0, 700.0]);
    world
        .get_mut::<StateMachine<EnemyState>>(wisp)
        .unwrap()
        .go(EnemyState::Search);
    let diving = until(&mut uninterrupted, 400, |h| {
        state(h, bat) == EnemyState::Attack
    });
    assert!(diving, "the bat dives");
    uninterrupted.step(4);
    assert_eq!(state(&uninterrupted, bat), EnemyState::Attack, "mid-dive");
    assert_eq!(
        state(&uninterrupted, wisp),
        EnemyState::Search,
        "mid-search"
    );
    let ids = [id_of(&uninterrupted, bat), id_of(&uninterrupted, wisp)];
    let saved = capture(&uninterrupted);
    let mut loaded = reloaded(saved, churned);
    enemy_script(&mut uninterrupted);
    enemy_script(&mut loaded);
    let look = |harness: &UiHarness| -> Vec<(EnemyState, Vec2)> {
        ids.iter()
            .map(|&id| {
                let enemy = by_id(harness, id);
                (state(harness, enemy), super::body(harness, enemy))
            })
            .collect()
    };
    let (a, b) = (look(&uninterrupted), look(&loaded));
    let player_b = by_id(&loaded, id_of(&uninterrupted, player));
    let health = |h: &UiHarness, p: Entity| h.world().get::<Health>(p).unwrap().current();
    println!(
        "enemy divergence (churned {churned}): bat {:?} and {:?} {:.2} px, wisp {:?} and {:?} {:.2} px, player {:.2} px, health {} and {}",
        a[0].0,
        b[0].0,
        a[0].1.distance(b[0].1),
        a[1].0,
        b[1].0,
        a[1].1.distance(b[1].1),
        centre(&uninterrupted, player).distance(centre(&loaded, player_b)),
        health(&uninterrupted, player),
        health(&loaded, player_b)
    );
    for ((state_a, at_a), (state_b, at_b)) in a.iter().zip(&b) {
        assert_eq!(state_a, state_b, "the same states");
        assert!(at_a.distance(*at_b) <= 12.0, "{at_a} and {at_b}");
    }
    assert_eq!(health(&uninterrupted, player), health(&loaded, player_b));
}

#[test]
fn a_loaded_run_with_enemies_mid_attack_and_mid_search_keeps_with_the_uninterrupted_one_from_a_fresh_harness()
 {
    enemy_divergence(false);
}

#[test]
fn a_loaded_run_with_enemies_mid_attack_and_mid_search_keeps_with_the_uninterrupted_one_in_a_churned_world()
 {
    enemy_divergence(true);
}

/// Whether the dummy of save ID `id` is the map's.
fn dummy_placed(harness: &UiHarness, id: u64) -> bool {
    example_01_platformer::features::dummy::placed(harness.world(), id)
}

#[test]
fn burning_frozen_and_shocked_dummies_load_with_their_sources() {
    use super::combat::{apply, dummy_at};
    let (mut harness, player) = grounded();
    let dummies: Vec<(Entity, &str)> =
        [(900.0, "burning"), (1200.0, "frozen"), (1500.0, "shocked")]
            .into_iter()
            .map(|(x, effect)| (dummy_at(&mut harness, x), effect))
            .collect();
    harness.step(2);
    for &(dummy, effect) in &dummies {
        apply(&mut harness, dummy, effect, Some(player));
    }
    harness.step(5);
    let world = harness.world();
    assert_eq!(
        world.query::<&Dummy>().count(),
        4,
        "the clearing's and the three"
    );
    let id = |entity: Entity| {
        example_01_platformer::shared::save_id(harness.world(), entity)
            .unwrap()
            .0
    };
    for &(dummy, effect) in &dummies {
        assert!(
            !dummy_placed(&harness, id(dummy)),
            "summoned, not the map's"
        );
        let effects = world.get::<Effects>(dummy).unwrap();
        assert!(effects.has(effect), "{effect}");
        assert!(
            effects
                .instances()
                .any(|instance| instance.source() == Some(player)),
            "{effect}: its source the player"
        );
    }
    let saved = capture(&harness);
    let mut fresh = super::harness();
    load(&mut fresh, saved.clone());
    let loaded_player = player_entity(&fresh);
    for &(dummy, effect) in &dummies {
        let wanted = id(dummy);
        let (entity, _) = fresh
            .world()
            .query::<(Entity, &Saved)>()
            .find(|(_, id)| id.id().0 == wanted)
            .expect("the dummy loads");
        let effects = fresh.world().get::<Effects>(entity).unwrap();
        let instance = effects
            .instances()
            .find(|instance| instance.effect().id() == Some(effect))
            .unwrap();
        assert_eq!(
            instance.source(),
            Some(loaded_player),
            "{effect}: named again"
        );
        let tag = format!("status.{effect}");
        assert!(fresh.world().get::<Tags>(entity).unwrap().has(&tag));
    }
    assert_eq!(json(&capture(&fresh)), json(&saved));
}

/// A waiting jump: its receiver, strike, count, chain, due run and
/// strength.
type Waiting = (Option<u64>, Option<u64>, u32, Vec<u64>, u64, f32);

/// The jumps waiting.
fn conduction(harness: &UiHarness) -> Vec<Waiting> {
    harness
        .world()
        .resource::<Cascades>()
        .entries
        .iter()
        .filter(|entry| entry.kind == CascadeKind::Jump)
        .map(|entry| {
            let chain = entry.chain.clone();
            (
                entry.target,
                entry.strike,
                entry.jumps,
                chain,
                entry.due,
                entry.strength,
            )
        })
        .collect()
}

#[test]
fn a_save_mid_conduction_resumes_the_chain() {
    let (mut harness, player) = grounded();
    for i in 0..4 {
        let at = Vec2::new(800.0 + 200.0 * i as f32, TOP - IRON_HALF.y);
        toys::place_iron(harness.world_mut(), at);
    }
    harness.step(2);
    let first = Vec2::new(800.0, TOP - IRON_HALF.y);
    lightning::strike(harness.world_mut(), player, first);
    harness.step(2);
    let saved = capture(&harness);
    assert_eq!(conduction(&harness).len(), 1, "a jump waits");
    let mut fresh = super::harness();
    load(&mut fresh, saved.clone());
    assert_eq!(json(&capture(&fresh)), json(&saved), "the chain whole");
    scaffold(&mut fresh);
    pin_camera(&mut fresh, CORNER);
    fresh.world_mut().resource_mut::<Time>().resume();
    assert_eq!(conduction(&harness), conduction(&fresh));
    for _ in 0..4 {
        harness.step(1);
        fresh.step(1);
        assert_eq!(
            conduction(&harness),
            conduction(&fresh),
            "the same jumps down the same chain"
        );
    }
    assert!(conduction(&fresh).is_empty(), "the line's end");
}

/// The toasts on screen.
fn toasts(harness: &UiHarness) -> Vec<String> {
    harness.world().resource::<Toasts>().texts(harness.tree())
}

#[test]
fn a_reaction_toasts_nothing() {
    let (mut harness, _) = grounded();
    let dummy = super::combat::dummy_at(&mut harness, 1400.0);
    let balls: Vec<Entity> = [900.0, 1100.0]
        .into_iter()
        .map(|x| toys::spawn_ball(harness.world_mut(), Vec2::new(x, TOP - 8.0), true))
        .collect();
    harness.step(2);
    // A freeze, a shatter, a steam burst and a shock.
    elements::freeze_iron(harness.world_mut(), balls[0]);
    harness.step(2);
    cascade::shatter_at(harness.world_mut(), balls[0]);
    harness.step(2);
    elements::ignite(harness.world_mut(), balls[1]);
    tungsten::kit::stimulate(harness.world_mut(), balls[1], elements::COLD, 1.0, None);
    harness.step(2);
    super::combat::apply(&mut harness, dummy, "shocked", None);
    harness.step(2);
    assert!(toasts(&harness).is_empty(), "{:?}", toasts(&harness));
}

#[test]
fn a_bullet_in_flight_loads_and_lands() {
    let (mut harness, player) = grounded();
    let dummy = super::combat::dummy_at(&mut harness, 1400.0);
    harness.step(2);
    let y = super::body(&harness, dummy).y;
    let id = example_01_platformer::shared::save_id(harness.world(), dummy).unwrap();
    gun::fire(
        harness.world_mut(),
        Vec2::new(700.0, y),
        Vec2::X * 2400.0,
        Some(player),
    )
    .expect("a bullet");
    harness.step(6);
    assert_eq!(bullets_flying(&harness), 1, "in flight");
    let saved = capture(&harness);
    let mut fresh = super::harness();
    load(&mut fresh, saved.clone());
    assert_eq!(json(&capture(&fresh)), json(&saved), "the bullet whole");
    scaffold(&mut fresh);
    pin_camera(&mut fresh, CORNER);
    fresh.world_mut().resource_mut::<Time>().resume();
    // The dummy's hits, step by step, in each world.
    let hits = |harness: &mut UiHarness| -> Vec<(usize, f32, bool)> {
        let mut hits = Vec::new();
        for step in 0..30 {
            harness.step(1);
            let world = harness.world();
            let target = world
                .query::<(Entity, &Dummy, &Saved)>()
                .find(|&(_, _, own)| own.id() == id)
                .map(|(entity, ..)| entity)
                .expect("the test's dummy, not the map's");
            hits.extend(
                world
                    .resource::<tungsten::core::EventQueue<tungsten::kit::DamagedPart>>()
                    .iter()
                    .filter(|part| part.entity == target && !part.periodic)
                    .map(|part| (step, part.amount, part.crit)),
            );
        }
        hits
    };
    let (before, after) = (hits(&mut harness), hits(&mut fresh));
    assert_eq!(before.len(), 1, "it landed: {before:?}");
    assert_eq!(before, after, "where and when it would have");
    assert_eq!(bullets_flying(&fresh), 0, "spent");
}

#[test]
fn a_save_mid_chain_resumes_the_same_chain() {
    let (mut harness, _) = grounded();
    let pile = ice_pile(&mut harness, 900.0, 48);
    harness.step(2);
    let edge = super::body(&harness, pile[0]);
    elements::thermal_shatter(harness.world_mut(), edge, 72.0);
    harness.step(2);
    let pending = |harness: &UiHarness| harness.world().resource::<Chemistry>().pending();
    let ran = |harness: &UiHarness| harness.world().resource::<Chemistry>().ran();
    assert!(pending(&harness) > 0, "mid-chain");
    let saved = capture(&harness);
    let mut fresh = super::harness();
    load(&mut fresh, saved.clone());
    assert_eq!(
        json(&capture(&fresh)),
        json(&saved),
        "the pile and the chain whole"
    );
    assert_eq!(
        (pending(&fresh), ran(&fresh)),
        (pending(&harness), ran(&harness))
    );
    scaffold(&mut fresh);
    pin_camera(&mut fresh, CORNER);
    fresh.world_mut().resource_mut::<Time>().resume();
    let live = |harness: &UiHarness| -> BTreeSet<u64> {
        harness
            .world()
            .query::<(&SmallBall, &Saved)>()
            .map(|(_, id)| id.id().0)
            .collect()
    };
    // The first run after the load works from the saved queues and the
    // saved points alone: the same entries run, in save-ID order.
    let at_save = live(&harness);
    harness.step(1);
    fresh.step(1);
    assert_eq!(live(&harness), live(&fresh), "the same balls went");
    assert_eq!(
        queue(&harness),
        queue(&fresh),
        "the same cascades queued next"
    );
    assert_eq!(
        (pending(&fresh), ran(&fresh)),
        (pending(&harness), ran(&harness))
    );
    // After it, the solver's warm starts, which no save holds, move the
    // pile a hair apart; each chain runs on to its end.
    harness.step(40);
    fresh.step(40);
    assert_eq!(
        (pending(&harness), pending(&fresh)),
        (0, 0),
        "both chains ran out"
    );
    assert!(live(&harness).len() < at_save.len(), "the chain took more");
    assert_eq!(live(&harness), live(&fresh), "the same endings");
}

/// The live kegs, by save ID.
fn kegs(harness: &UiHarness) -> BTreeSet<u64> {
    harness
        .world()
        .query::<(&Keg, &Saved)>()
        .map(|(_, id)| id.id().0)
        .collect()
}

#[test]
fn a_keg_chain_saved_mid_chain_resumes_with_the_same_kegs_gone() {
    // A keg's blast waits in chemistry's queue, its `jolt`, between the
    // blast that sets it off and its run: a save mid-chain loads with it
    // waiting, and the chain runs on to its end.
    let (mut harness, player) = grounded();
    let world = harness.world_mut();
    let row: Vec<Entity> = (0..6)
        .map(|i| {
            let at = Vec2::new(900.0 + 150.0 * i as f32, TOP - KEG_HALF.y - 0.5);
            props::spawn(world, PropKind::Keg, at)
        })
        .collect();
    harness.step(2);
    let ids: BTreeSet<u64> = row.iter().map(|&keg| id_of(&harness, keg)).collect();
    let me = id_of(&harness, player);
    assert!(props::set_off(harness.world_mut(), row[0], Some(me)));
    // Saved after the second wave: two gone, the third's blast waiting.
    let blown = |harness: &UiHarness| ids.difference(&kegs(harness)).count();
    for _ in 0..10 {
        if blown(&harness) >= 2 {
            break;
        }
        harness.step(1);
    }
    assert_eq!(blown(&harness), 2, "two waves");
    let pending = |harness: &UiHarness| harness.world().resource::<Chemistry>().pending();
    assert!(pending(&harness) > 0, "mid-chain");
    let saved = capture(&harness);
    let read: SaveGame = serde_json::from_value(json(&saved)).expect("a save reads");
    let mut fresh = super::harness();
    load(&mut fresh, read);
    assert_eq!(
        json(&capture(&fresh)),
        json(&saved),
        "the row and its blast whole"
    );
    assert_eq!(pending(&fresh), pending(&harness), "the waiting blast kept");
    scaffold(&mut fresh);
    pin_camera(&mut fresh, CORNER);
    fresh.world_mut().resource_mut::<Time>().resume();
    let standing = kegs(&fresh);
    assert_eq!(standing, kegs(&harness), "the kegs as they stood");
    assert_eq!(ids.intersection(&standing).count(), 4, "four of the row");
    for frame in 0..30 {
        harness.step(1);
        fresh.step(1);
        assert_eq!(
            kegs(&fresh),
            kegs(&harness),
            "frame {frame}: the same kegs stand"
        );
    }
    assert!(ids.is_disjoint(&kegs(&harness)), "the chain ran to its end");
    assert!(
        pending(&harness) == 0 && pending(&fresh) == 0,
        "both queues empty"
    );
}

/// Chemistry's waiting entries and the entries its runs took.
fn queue_counts(harness: &UiHarness) -> (usize, u64) {
    let chemistry = harness.world().resource::<Chemistry>();
    (chemistry.pending(), chemistry.ran())
}

/// Whether `entity` is frozen through.
fn frozen(harness: &UiHarness, entity: Entity) -> bool {
    harness
        .world()
        .get::<Tags>(entity)
        .is_some_and(|tags| tags.has("status.frozen"))
}

/// The live scraps' velocities, by save ID.
fn scrap_throws(harness: &UiHarness) -> BTreeMap<u64, Vec2> {
    harness
        .world()
        .query::<(&IronScrap, &Saved, &Velocity)>()
        .map(|(_, id, velocity)| (id.id().0, velocity.0))
        .collect()
}

/// An iron block of the run's on the sky's ground at `x`, settled.
fn iron_at(harness: &mut UiHarness, x: f32) -> Entity {
    let iron = toys::place_iron(harness.world_mut(), Vec2::new(x, TOP - IRON_HALF.y - 0.5));
    harness.step(10);
    iron
}

#[test]
fn a_frozen_blocks_waiting_thermal_shock_throws_its_scraps_as_the_uninterrupted_run() {
    let (mut harness, _) = grounded();
    let iron = iron_at(&mut harness, 950.0);
    elements::freeze_iron(harness.world_mut(), iron);
    harness.step(2);
    // Heat from the block's left: its shock waits for the next run, and
    // `Cascades` keeps the point its scraps fly away from.
    let heat = super::body(&harness, iron) - Vec2::new(IRON_HALF.x + 20.0, 0.0);
    elements::thermal_shatter(harness.world_mut(), heat, 72.0);
    assert!(queue_counts(&harness).0 > 0, "the shock waits");
    assert_eq!(harness.world().resource::<Cascades>().heat_from.len(), 1);
    let saved = capture(&harness);
    let mut fresh = super::harness();
    load(&mut fresh, saved.clone());
    assert_eq!(
        json(&capture(&fresh)),
        json(&saved),
        "the shock and its point whole"
    );
    scaffold(&mut fresh);
    pin_camera(&mut fresh, CORNER);
    fresh.world_mut().resource_mut::<Time>().resume();
    for _ in 0..5 {
        if !scrap_throws(&harness).is_empty() {
            break;
        }
        harness.step(1);
        fresh.step(1);
    }
    let (a, b) = (scrap_throws(&harness), scrap_throws(&fresh));
    assert_eq!(a.len(), 16, "the block shattered");
    assert_eq!(a.keys().collect::<Vec<_>>(), b.keys().collect::<Vec<_>>());
    for (id, thrown) in &a {
        assert!(
            thrown.distance(b[id]) <= 1e-3,
            "scrap {id}: {thrown} and {}",
            b[id]
        );
    }
    let east = a.values().map(|v| v.x).sum::<f32>() / a.len() as f32;
    assert!(east > 0.0, "thrown away from the heat on the left: {east}");
}

#[test]
fn a_kept_block_saved_at_its_freeze_band_loads_with_one_freeze_waiting() {
    let (mut harness, _) = grounded();
    let iron = iron_at(&mut harness, 950.0);
    let face = super::body(&harness, iron) - Vec2::new(IRON_HALF.x, 0.0);
    elements::chill(harness.world_mut(), iron, 1.0, face);
    // The run builds the coat to its band, whose freeze waits for the next.
    harness.step(1);
    assert!(!frozen(&harness, iron), "not yet");
    assert_eq!(queue_counts(&harness).0, 1, "the band's freeze waits");
    let saved = capture(&harness);
    let mut fresh = super::harness();
    load(&mut fresh, saved.clone());
    // The coat put back reaches the band again; the saved queue replaces
    // what that queued.
    assert_eq!(queue_counts(&fresh), queue_counts(&harness), "one freeze");
    assert_eq!(json(&capture(&fresh)), json(&saved));
    scaffold(&mut fresh);
    pin_camera(&mut fresh, CORNER);
    fresh.world_mut().resource_mut::<Time>().resume();
    let loaded = fresh
        .world()
        .query::<(Entity, &IronBlock, &Saved)>()
        .find(|(_, _, id)| id.id().0 == id_of(&harness, iron))
        .map(|(entity, ..)| entity)
        .expect("the block");
    harness.step(1);
    fresh.step(1);
    assert!(
        frozen(&harness, iron) && frozen(&fresh, loaded),
        "both froze"
    );
    assert_eq!(queue_counts(&fresh), queue_counts(&harness));
}

/// The player's health and the hits it took this frame.
fn hurt(harness: &UiHarness) -> (f32, Vec<f32>) {
    let player = player_entity(harness);
    let hits = harness
        .harness()
        .events::<Damaged>()
        .filter(|hit| hit.entity == player && !hit.periodic)
        .map(|hit| hit.amount)
        .collect();
    let health = harness.world().get::<Health>(player).unwrap().current();
    (health, hits)
}

#[test]
fn a_hazards_touch_on_the_player_at_a_save_bites_no_sooner_after_the_load() {
    // The map places no fire hazard (known issues, since 0.97); a lava
    // basin's touch is the same kit contact, whose times the engine now
    // saves with the basin. The player takes every bite, no window of its
    // own, so a touch that forgot its last would bite at the load's first
    // step.
    let (mut harness, player) = run();
    calm(&mut harness);
    harness.harness_mut().set_dt(1.0 / 60.0);
    harness.step(10);
    let health = harness.world_mut().get_mut::<Health>(player).unwrap();
    *health = health.clone().with_invulnerability(0.0);
    let lake = basin(&harness, SaveId::authored("lava_lake_west").0);
    let x = harness.world().get::<Transform>(lake).unwrap().position.x + 96.0;
    let surface = harness.world().get::<Transform>(lake).unwrap().position.y + 6.0;
    stand(&mut harness, player, x, surface);
    harness.step(1);
    assert_eq!(hurt(&harness).1, [16.0], "the lava's bite");
    let saved = capture(&harness);
    let mut fresh = super::harness();
    fresh.harness_mut().set_dt(1.0 / 60.0);
    load(&mut fresh, saved.clone());
    calm(&mut fresh);
    assert_eq!(json(&capture(&fresh)), json(&saved));
    fresh.world_mut().resource_mut::<Time>().resume();
    let loaded = player_entity(&fresh);
    // Back on the lava each step: both bite at the touch's next interval
    // (0.1 s), not before. The push differs at the load's first step,
    // which finds no contacts after a load, as a deck's rider does.
    let mut bites = Vec::new();
    for frame in 0..12 {
        stand(&mut harness, player, x, surface);
        stand(&mut fresh, loaded, x, surface);
        harness.step(1);
        fresh.step(1);
        assert_eq!(hurt(&fresh), hurt(&harness), "frame {frame}");
        if !hurt(&harness).1.is_empty() {
            bites.push(frame);
        }
    }
    assert!(
        bites.first().is_some_and(|&first| first >= 4),
        "the next bite waits out the interval: {bites:?}"
    );
}

/// A New Game at Normal started as `load` starts a save, its clock held.
fn new_run_held(harness: &mut UiHarness) {
    harness.world_mut().resource_mut::<Time>().pause();
    let start = RunStart {
        difficulty: Difficulty::Normal,
        save: None,
    };
    game_shell::start_run(harness.world_mut(), start, None);
    settle(harness);
    harness.step(1);
    assert_eq!(top(harness), Some(game::GAMEPLAY));
}

/// Each saved entity's kind, by save ID.
fn identities(harness: &UiHarness) -> BTreeMap<u64, &'static str> {
    let world = harness.world();
    world
        .query::<(Entity, &Saved)>()
        .map(|(entity, saved)| (saved.id().0, kind(world, entity)))
        .collect()
}

#[test]
fn a_save_the_engine_refuses_loads_as_a_fresh_run_with_nothing_staged() {
    let (mut harness, _) = grounded();
    busy(&mut harness);
    let saved = capture(&harness);
    assert!(!saved.balls.is_empty(), "balls the first pass would stage");
    assert!(
        !json(&saved)["world"]["areas"]["ex10_level"]["spawns"]
            .as_array()
            .is_none_or(Vec::is_empty),
        "and records the restore would spawn"
    );
    let mut newer = json(&saved);
    newer["world"]["versions"]["components"]["health"] = serde_json::json!(99);
    let newer: SaveGame = serde_json::from_value(newer).expect("a save reads");
    let mut failed = super::harness();
    load(&mut failed, newer);
    let mut fresh = super::harness();
    new_run_held(&mut fresh);
    assert_eq!(identities(&failed), identities(&fresh), "nothing staged");
    let counts = |harness: &UiHarness| {
        let chemistry = harness.world().resource::<Chemistry>();
        (
            chemistry.pending(),
            chemistry.ran(),
            chemistry.dropped(),
            chemistry.cut(),
        )
    };
    assert_eq!(counts(&failed), counts(&fresh), "a fresh run's queue");
    assert_eq!(queue(&failed), queue(&fresh), "and its cascades");
    let next = example_01_platformer::shared::next_id(failed.world());
    assert!(
        !identities(&failed).contains_key(&next),
        "the next ID is free"
    );
    assert_eq!(
        super::body(&failed, player_entity(&failed)),
        super::body(&fresh, player_entity(&fresh)),
        "the player at the map's spawn"
    );
    assert_eq!(failed.world().resource::<Run>().area, None, "no area");
}

#[test]
fn a_moved_player_saves_and_loads_to_itself_and_the_camera_frames_it() {
    let (mut harness, player) = run();
    calm(&mut harness);
    let spawn = super::body(&harness, player);
    // Far along the meadow from the map's spawn, on its ground.
    let at = spawn + Vec2::new(1600.0, 0.0);
    teleport(&mut harness, player, at);
    harness.step(120);
    let standing = super::body(&harness, player);
    assert!(standing.distance(spawn) > 1000.0, "{standing} from {spawn}");
    let saved = capture(&harness);
    let mut fresh = super::harness();
    load(&mut fresh, saved.clone());
    calm(&mut fresh);
    assert_eq!(
        json(&capture(&fresh)),
        json(&saved),
        "saved again, the same"
    );
    let loaded = player_entity(&fresh);
    assert_eq!(super::body(&fresh, loaded), standing);
    let view = fresh
        .world()
        .resource::<tungsten::core::CameraController>()
        .frame();
    let inside =
        |point: Vec2| point.cmpge(view.corner).all() && point.cmple(view.corner + view.size).all();
    assert!(
        inside(standing),
        "the camera frames the saved point: {view:?}"
    );
    assert!(!inside(spawn), "not the map's spawn: {view:?}");
}

#[test]
fn a_captures_recorded_spawns_are_the_runs_toys_props_and_the_kits() {
    // The run's spawns, 01's and the kit's from a saved source, are the
    // World save's records; the balls are 01's table. Pinned, so a spawn
    // added later (a cue's prefab) shows here.
    let (mut harness, _) = grounded();
    busy(&mut harness);
    let saved = json(&capture(&harness));
    let world = &saved["world"];
    let prefabs: BTreeSet<&str> = std::iter::once(&world["global"])
        .chain(
            world["areas"]
                .as_object()
                .into_iter()
                .flat_map(|areas| areas.values()),
        )
        .filter_map(|area| area["spawns"].as_array())
        .flatten()
        .filter_map(|spawn| spawn["prefab"].as_str())
        .collect();
    let [small, medium, large] = props::STONE_PREFABS;
    let expected = BTreeSet::from([
        gun::BULLET_PREFAB,
        toys::FIRE_MISSILE_PREFAB,
        OrbKind::Mana.prefab(),
        toys::IRON_BLOCK_PREFAB,
        toys::IRON_SCRAP_PREFAB,
        toys::BLACK_HOLE_PREFAB,
        toys::flask::FLASK_PREFAB,
        snowball::SNOWBALL_PREFAB,
        props::CRATE_PREFAB,
        props::KEG_PREFAB,
        props::BOMB_PREFAB,
        props::ROCK_PREFAB,
        props::PLANK_PREFAB,
        small,
        medium,
        large,
    ]);
    assert_eq!(prefabs, expected);
    assert!(
        saved["balls"]
            .as_array()
            .is_some_and(|balls| balls.len() >= 500),
        "the balls in 01's table"
    );
}

/// Puts the player on `shot` and steps once: whether the shot still flies,
/// and the player's health after.
fn through_its_caster(harness: &mut UiHarness, shot: u64) -> (bool, f32) {
    let player = player_entity(harness);
    let at = super::body(harness, by_id(harness, shot));
    teleport(harness, player, at);
    harness.step(1);
    let flies = harness
        .world()
        .query::<&Saved>()
        .any(|saved| saved.id().0 == shot);
    (flies, hurt(harness).0)
}

/// A bullet and the spell's fire missile in flight from the player: the
/// run and each shot's save ID.
fn shots() -> (UiHarness, [u64; 2]) {
    let (mut harness, player) = grounded();
    let centre = super::body(&harness, player);
    let bullet = gun::fire(
        harness.world_mut(),
        centre + Vec2::new(0.0, -120.0),
        Vec2::new(300.0, 0.0),
        Some(player),
    )
    .expect("a bullet")
    .0;
    cast(
        &mut harness,
        Spell::FireMissile,
        centre + Vec2::new(-800.0, -200.0),
        1,
    );
    let missile = harness
        .world()
        .query::<(&FireMissile, &Saved)>()
        .map(|(_, saved)| saved.id().0)
        .next()
        .expect("the spell's missile");
    (harness, [bullet, missile])
}

#[test]
fn a_loaded_fire_missile_and_bullet_spare_their_caster() {
    // Each loads without the emitter its spawn derived (the plan's §2.6),
    // its `Source` by reference: a hit spares the source's entity, as the
    // run it was saved from does.
    for index in 0..2 {
        let (mut harness, ids) = shots();
        let shot = ids[index];
        let saved = capture(&harness);
        let mut fresh = super::harness();
        load(&mut fresh, saved);
        scaffold(&mut fresh);
        pin_camera(&mut fresh, CORNER);
        fresh.world_mut().resource_mut::<Time>().resume();
        let health = hurt(&harness).0;
        let live = through_its_caster(&mut harness, shot);
        let loaded = through_its_caster(&mut fresh, shot);
        assert_eq!(loaded, live, "shot {shot}");
        assert_eq!(live, (true, health), "shot {shot} passes and spares");
    }
}

/// The harness's save slots.
fn harness_slots(harness: &UiHarness) -> SaveSlots {
    let dir = harness
        .world()
        .resource::<SaveSlots>()
        .dir()
        .map(Path::to_path_buf);
    SaveSlots::new(dir)
}

/// Writes each `(slot, schema)` of `older` as an older build's save, as it
/// wrote one, its data a stand-in, at `at` Unix milliseconds.
fn write_older(harness: &UiHarness, older: &[(&str, u32)], at: u64) {
    let slots = harness_slots(harness);
    for &(slot, schema) in older {
        let stamp = tungsten::core::SaveStamp::new(schema, "0.117.0", at);
        let stand_in = serde_json::json!({ "player": [0.0, 0.0] });
        slots
            .save_stamped(slot, &stand_in, &stamp)
            .expect("an older save is written");
    }
}

#[test]
fn every_schema_before_the_engines_saves_is_refused() {
    // The template sample's 1 to 3, 01 (a) to (c)'s 4 to 6, whose map 01 (d)
    // replaced, and 7 to 27, whose run 01's own records held: schema 28
    // moves it onto the engine's saves.
    for from in 1..SaveGame::SCHEMA {
        assert!(
            SaveGame::migrate(from, Value::Null).is_err(),
            "schema {from}"
        );
    }
    // An older save on disk reads as an older version's, never loads.
    let dir = user_folder("schemas");
    let mut harness = harness_with(UserFolder::At(dir.clone()));
    let older = [("slot_1", 1), ("slot_2", 6), ("slot_3", 7), ("slot_4", 27)];
    write_older(&harness, &older, 1_760_400_000_000);
    let (slots, _) = game_shell::read_slots(harness.world_mut());
    for (slot, _) in older {
        let state = slots.iter().find(|(s, _)| *s == slot).map(|(_, s)| s);
        assert!(matches!(state, Some(SlotState::Older(_))), "{slot}");
    }
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn older_autosaves_get_an_autosave() {
    // An upgrader: three autosaves of schema 27, which this build refuses,
    // read as an older version's and give way to the run's, the oldest
    // first.
    let dir = user_folder("older-autosaves");
    let mut harness = harness_with(UserFolder::At(dir.clone()));
    for (slot, at) in game_shell::AUTOSAVES.into_iter().zip([3_u64, 1, 2]) {
        write_older(&harness, &[(slot, 27)], 1_760_400_000_000 + at);
    }
    start(&mut harness);
    harness.step(5);
    let slots = harness_slots(&harness);
    let written: Vec<_> = game_shell::AUTOSAVES
        .into_iter()
        .filter(|slot| slots.load::<SaveGame>(slot).is_ok())
        .collect();
    assert_eq!(written, ["autosave_2"], "the run's first autosave");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_guttered_staff_light_loads_guttered() {
    use example_01_platformer::features::player::components::Lantern;
    let (mut harness, player) = run();
    harness.world_mut().get_mut::<Lantern>(player).unwrap().on = true;
    harness.step(2);
    // Out of mana under a cast's pause the light gutters, and a save at once
    // keeps it so, while its effect's tag is still on.
    let stats = harness
        .world_mut()
        .get_mut::<tungsten::kit::Stats>(player)
        .unwrap();
    stats.set_base("mana", 0.0);
    super::combat::apply(&mut harness, player, "casting", None);
    harness.step(1);
    let lantern = *harness.world().get::<Lantern>(player).unwrap();
    assert!(lantern.on && lantern.guttered, "{lantern:?}");
    let saved = capture(&harness);
    let mut fresh = super::harness();
    load(&mut fresh, saved);
    fresh.world_mut().resource_mut::<Time>().resume();
    fresh.step(2);
    let lantern = *fresh.world().get::<Lantern>(player_entity(&fresh)).unwrap();
    assert!(lantern.guttered && !lantern.lit(), "{lantern:?}");
}

/// The paths where `a` and `b` differ, at most 20; fractions within a
/// hundredth agree (a held block is placed at its troll's hands again).
fn differences(a: &Value, b: &Value, path: &str, out: &mut Vec<String>) {
    if out.len() >= 20 || a == b {
        return;
    }
    match (a, b) {
        (Value::Number(x), Value::Number(y))
            if x.is_f64()
                && x.as_f64()
                    .zip(y.as_f64())
                    .is_some_and(|(x, y)| (x - y).abs() <= 0.01) => {}
        (Value::Object(a), Value::Object(b)) => {
            let keys: BTreeSet<&String> = a.keys().chain(b.keys()).collect();
            for key in keys {
                let (x, y) = (a.get(key), b.get(key));
                match (x, y) {
                    (Some(x), Some(y)) => differences(x, y, &format!("{path}/{key}"), out),
                    _ => out.push(format!("{path}/{key}: {x:?} against {y:?}")),
                }
            }
        }
        (Value::Array(a), Value::Array(b)) if a.len() == b.len() => {
            for (i, (x, y)) in a.iter().zip(b).enumerate() {
                differences(x, y, &format!("{path}/{i}"), out);
            }
        }
        _ => out.push(format!("{path}: {a} against {b}")),
    }
}

/// The placed health orbs there, by save ID.
fn placed_orbs(harness: &UiHarness) -> Vec<(u64, Entity)> {
    let world = harness.world();
    let placed = &world.resource::<drops::PlacedOrbs>().0;
    let mut orbs: Vec<(u64, Entity)> = world
        .query::<(Entity, &Orb, &Saved)>()
        .filter(|(_, _, id)| placed.contains(&id.id().0))
        .map(|(entity, _, id)| (id.id().0, entity))
        .collect();
    orbs.sort_unstable_by_key(|(id, _)| *id);
    orbs
}

#[test]
fn placed_health_orbs_never_fade_and_one_taken_stays_gone_through_a_load() {
    let (mut harness, player) = run();
    let orbs = placed_orbs(&harness);
    assert_eq!(orbs.len() as u64, save::MAP_PICKUPS);
    for &(_, orb) in &orbs {
        let world = harness.world();
        assert_eq!(world.get::<Orb>(orb).unwrap().kind, OrbKind::Health);
        assert!(
            world.get::<tungsten::kit::Lifetime>(orb).is_none(),
            "it never fades"
        );
    }
    // The player walks into the first and takes it.
    let (taken, orb) = orbs[0];
    let at = harness.world().get::<Position>(orb).unwrap().0 + Vec2::splat(12.0);
    teleport(&mut harness, player, at);
    for _ in 0..30 {
        harness.step(1);
        if !harness.world().is_alive(orb) {
            break;
        }
    }
    assert!(!harness.world().is_alive(orb), "taken");
    assert!(
        placed_orbs(&harness).iter().all(|&(id, _)| id != taken),
        "off the map"
    );
    assert_eq!(
        harness
            .world()
            .query::<(&Orb, &tungsten::kit::Lifetime)>()
            .count(),
        0,
        "a placed orb is the map's, not a drop's"
    );
    let saved = capture(&harness);
    let mut fresh = super::harness();
    load(&mut fresh, saved);
    let left: Vec<u64> = placed_orbs(&fresh).into_iter().map(|(id, _)| id).collect();
    let expected: Vec<u64> = orbs[1..].iter().map(|&(id, _)| id).collect();
    assert_eq!(left, expected, "the taken one gone, the rest there");
}

/// The elevator's deck and its state.
fn elevator(harness: &UiHarness) -> (Entity, Elevator) {
    harness
        .world()
        .query::<(Entity, &Elevator)>()
        .map(|(entity, lift)| (entity, *lift))
        .next()
        .expect("the mine's elevator")
}

/// `deck`'s top, in world pixels.
fn deck_top(harness: &UiHarness, deck: Entity) -> f32 {
    let world = harness.world();
    let at = world.get::<Position>(deck).unwrap().0;
    let collider = world.get::<tungsten::core::Collider>(deck).unwrap();
    let tungsten::core::Shape::Aabb { half_extents } = collider.shape else {
        unreachable!("a deck is a box")
    };
    at.y + collider.offset.y - half_extents.y
}

/// The basin with save ID `id`.
fn basin(harness: &UiHarness, id: u64) -> Entity {
    harness
        .world()
        .query::<(Entity, &Lava, &Saved)>()
        .find(|(_, _, own)| own.id().0 == id)
        .map(|(entity, ..)| entity)
        .expect("the basin")
}

#[test]
fn a_crusted_basin_and_a_riding_elevator_load_back_to_themselves() {
    let (mut harness, player) = run();
    calm(&mut harness);
    harness.harness_mut().set_dt(1.0 / 60.0);
    let lake = basin(&harness, SaveId::authored("lava_lake_west").0);
    lava::harden(harness.world_mut(), lake, 4.0);
    // The player boards the elevator; the save is taken halfway up.
    let (deck, lift) = elevator(&harness);
    let middle = harness.world().get::<Position>(deck).unwrap().0.x + 96.0;
    let bottom = deck_top(&harness, deck);
    stand(&mut harness, player, middle, bottom);
    let halfway = bottom - (lift.bottom.y - lift.top.y) * 0.5;
    for _ in 0..600 {
        harness.step(1);
        if deck_top(&harness, deck) < halfway {
            break;
        }
    }
    assert_eq!(elevator(&harness).1.stop, Stop::Rising, "halfway up");
    let shelf = basin(&harness, SaveId::authored("lava_shelf").0);
    lava::set_cold(harness.world_mut(), shelf, 0.2);
    let world = harness.world();
    assert!(lava::crust_left(world, lake).is_some_and(|left| left > 1.0 && left < 4.0));
    assert_eq!(lava::cold_of(world, shelf), 0.2);
    assert_eq!(world.query::<&Elevator>().count(), 1);
    let (deck, lift) = elevator(&harness);
    assert!(world.has::<PathFollower>(deck) && lift.ridden);
    let saved = capture(&harness);
    // Loaded from its file, as Continue loads it, it captures back to
    // itself: the crust's touch held, its surface solid, the deck mid-path.
    let read: SaveGame = serde_json::from_value(json(&saved)).expect("a save reads");
    let mut fresh = super::harness();
    fresh.harness_mut().set_dt(1.0 / 60.0);
    load(&mut fresh, read);
    // Calmed as the saved run was: the map's geysers, which it took off.
    calm(&mut fresh);
    assert_eq!(json(&capture(&fresh)), json(&saved));
    let lake = basin(&fresh, SaveId::authored("lava_lake_west").0);
    let world = fresh.world();
    assert!(
        world
            .get::<tungsten::kit::ContactDamage>(lake)
            .is_some_and(|touch| !touch.is_on()),
        "held"
    );
    assert_eq!(
        world.get::<tungsten::core::RigidBody>(lake).unwrap().layers,
        1
    );
    // What the crust's handler set beside its status, read off the restored
    // crust: the body's layers and mask, the touch and the glow, as the
    // uninterrupted run's.
    let crust = |world: &World, basin: Entity| {
        use example_01_platformer::features::level::components::Glow;
        let body = world.get::<tungsten::core::RigidBody>(basin).unwrap();
        (
            body.layers,
            body.mask,
            world.get::<ContactDamage>(basin).unwrap().is_on(),
            world.get::<Glow>(basin).map(|glow| glow.hidden),
        )
    };
    let uninterrupted = basin(&harness, SaveId::authored("lava_lake_west").0);
    assert_eq!(crust(world, lake), crust(harness.world(), uninterrupted));
    let (layers, mask, on, hidden) = crust(world, lake);
    assert_eq!(
        (layers, mask, on),
        (1, tungsten::core::RigidBody::ALL_LAYERS, false)
    );
    assert_ne!(hidden, Some(false), "a glow, where it has one, out");
    // With the clock running, the deck holds through the rider's first
    // step, which finds no ground after a load, then carries the rider on
    // to the top, its feet on the deck all the way.
    calm(&mut fresh);
    fresh.world_mut().resource_mut::<Time>().resume();
    let (deck, _) = elevator(&fresh);
    let rider = player_entity(&fresh);
    let at = deck_top(&fresh, deck);
    fresh.step(1);
    assert_eq!(steps_this_frame(&fresh), 1);
    assert_eq!(deck_top(&fresh, deck), at, "held through the first step");
    let mut steps = 0;
    while steps < 240 {
        fresh.step(1);
        steps += steps_this_frame(&fresh);
        let feet = centre(&fresh, rider).y + player_half().y;
        let top = deck_top(&fresh, deck);
        assert!(
            (feet - top).abs() <= 1.0,
            "step {steps}: feet at {feet}, the deck's top at {top}"
        );
    }
    let (_, lift) = elevator(&fresh);
    assert_eq!(lift.stop, Stop::Top, "carried to the top");
    // The crust melted on the way: its touch is back.
    let world = fresh.world();
    assert!(!lava::crusted(world, lake), "melted");
    assert!(
        world
            .get::<tungsten::kit::ContactDamage>(lake)
            .is_some_and(tungsten::kit::ContactDamage::is_on),
        "its touch back"
    );
}

#[test]
fn pit_full_saves_within_two_mib() {
    let dir = user_folder("pit-full");
    let mut harness = harness_with(UserFolder::At(dir.clone()));
    start(&mut harness);
    let world = harness.world_mut();
    let live = toys::ball_count(world);
    for i in 0..BALL_CAP - live {
        let at = Vec2::new(
            100.0 + (i % 150) as f32 * 15.0,
            100.0 + (i / 150) as f32 * 15.0,
        );
        toys::spawn_ball(world, at, true);
    }
    let alive = enemies::live(world).len();
    assert_eq!(alive as u64, MAP_ENEMIES, "the map's enemies alive");
    let started = Instant::now();
    assert!(game_shell::save_to(world, "slot_1"), "the slot is written");
    let took = started.elapsed();
    let saves = world.resource::<SaveSlots>().dir().unwrap().to_path_buf();
    let size = std::fs::metadata(saves.join("slot_1.json")).unwrap().len();
    println!(
        "pit_full: {} balls, {alive} enemies, {size} bytes ({:.2} MiB), capture and write {:.1} ms",
        world.query::<&Ball>().count(),
        size as f64 / (1024.0 * 1024.0),
        took.as_secs_f64() * 1000.0
    );
    assert!(size <= 2 * 1024 * 1024, "{size} bytes");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn three_open_black_holes_load_back_to_themselves() {
    let (mut harness, _) = grounded();
    let world = harness.world_mut();
    for (i, x) in [700.0, 1000.0, 1300.0].into_iter().enumerate() {
        let hole = toys::place_black_hole(world, Vec2::new(x, TOP - 300.0));
        world.get_mut::<BlackHole>(hole).unwrap().left = 2.0 + i as f32;
    }
    let open = holes(&harness);
    assert_eq!(open.len(), 3, "every hole open");
    let saved = capture(&harness);
    // Through JSON, as Continue reads a slot.
    let loaded: SaveGame = serde_json::from_value(json(&saved)).expect("it loads");
    let mut fresh = super::harness();
    load(&mut fresh, loaded);
    assert_eq!(fresh.world().query::<&BlackHole>().count(), 3);
    assert_eq!(holes(&fresh), open, "each where it was, its time left");
}

/// The open black holes: each one's save ID, point and seconds left.
fn holes(harness: &UiHarness) -> Vec<(u64, [f32; 2], f32)> {
    let mut open: Vec<(u64, [f32; 2], f32)> = harness
        .world()
        .query::<(&Saved, &Transform, &BlackHole)>()
        .map(|(id, at, hole)| (id.id().0, at.position.to_array(), hole.left))
        .collect();
    open.sort_by_key(|&(id, ..)| id);
    open
}

/// The kinds of the books on the map, in [`TomeKind::ALL`]'s order.
fn books(harness: &UiHarness) -> Vec<TomeKind> {
    let mut kinds: Vec<TomeKind> = harness
        .world()
        .query::<&Tome>()
        .map(|tome| tome.kind)
        .collect();
    kinds.sort_by_key(|kind| kind.index());
    kinds
}

#[test]
fn a_run_holding_two_tomes_and_a_half_shield_loads_them_back_with_their_books_gone() {
    let (mut harness, player) = run();
    assert_eq!(books(&harness), TomeKind::ALL, "the map's three");
    // As two takes leave it: those books gone, their kinds on the player,
    // the pact's half health and its shield, hit to half and waiting.
    let world = harness.world_mut();
    let taken: Vec<Entity> = world
        .query::<(Entity, &Tome)>()
        .filter(|(_, tome)| tome.kind != TomeKind::Arcane)
        .map(|(book, _)| book)
        .collect();
    for book in taken {
        example_01_platformer::shared::despawn_tree(world, book);
    }
    world.insert(player, Tomes(vec![TomeKind::Bounding, TomeKind::Pact]));
    let stats = world.get_mut::<Stats>(player).unwrap();
    stats.add_modifier(Stats::MAX_HEALTH, "tome.pact", Modifier::Mul(0.5));
    world.insert(player, Shield::new(50.0).with_recharge(20.0, 3.0));
    harness.step(1);
    DamageRequest::new(player, 25.0).queue(harness.world_mut());
    harness.step(30);
    let held = Tomes(vec![TomeKind::Bounding, TomeKind::Pact]);
    let world = harness.world();
    assert_eq!(world.get::<Tomes>(player), Some(&held));
    let shield = world
        .get::<Shield>(player)
        .cloned()
        .expect("the pact's shield");
    assert_eq!(shield.current(), 25.0, "half full");
    let health = |h: &UiHarness| {
        let health = h.world().get::<Health>(player_entity(h)).unwrap();
        (health.current(), health.max())
    };
    assert_eq!(health(&harness).1, 50.0);
    let saved = capture(&harness);
    let read: SaveGame = serde_json::from_value(json(&saved)).expect("the save reads");
    let mut fresh = super::harness();
    load(&mut fresh, read);
    let player = player_entity(&fresh);
    assert_eq!(fresh.world().get::<Tomes>(player), Some(&held));
    assert_eq!(books(&fresh), [TomeKind::Arcane], "the third on the map");
    assert_eq!(
        fresh.world().get::<Shield>(player),
        Some(&shield),
        "its wait too"
    );
    assert_eq!(health(&fresh), health(&harness));
}

#[test]
fn burning_planks_save_through_json_and_load_back() {
    use example_01_platformer::features::props::components::{Plank, PropFire};
    let (mut harness, _) = grounded();
    let world = harness.world_mut();
    let crate_ = props::spawn(
        world,
        PropKind::Crate,
        Vec2::new(1500.0, TOP - CRATE_HALF.y - 0.5),
    );
    assert!(props::light(world, crate_, Some(5)));
    props::break_crate(world, crate_, Vec2::ZERO);
    harness.step(20);
    let planks = |harness: &UiHarness| {
        let world = harness.world();
        let mut planks: Vec<(u64, usize, PropFire)> = world
            .query::<(Entity, &Saved, &Plank)>()
            .map(|(entity, id, plank)| {
                let fire = props::fire(world, entity).expect("a plank's fire");
                (id.id().0, plank.variant, fire)
            })
            .collect();
        planks.sort_by_key(|(id, ..)| *id);
        planks
    };
    assert!(!harness.world().is_alive(crate_), "broken");
    let kept = planks(&harness);
    assert_eq!(kept.len(), 5, "its planks");
    assert!(kept.iter().all(|(.., fire)| fire.lit));
    let saved = capture(&harness);
    // Through JSON, as Continue reads a slot.
    let loaded: SaveGame = serde_json::from_value(json(&saved)).expect("a save loads");
    let mut fresh = super::harness();
    load(&mut fresh, loaded);
    let back = planks(&fresh);
    assert_eq!(back.len(), 5);
    for ((id, variant, fuel), (want_id, want_variant, want)) in back.into_iter().zip(kept) {
        assert_eq!((id, variant), (want_id, want_variant));
        assert!(fuel.lit && fuel.lighter == Some(5));
        assert!(
            (fuel.left - want.left).abs() < 0.05,
            "{} for {}",
            fuel.left,
            want.left
        );
    }
}

#[test]
fn three_stones_of_three_sizes_save_through_json_and_load_back() {
    let (mut harness, _) = grounded();
    // The map's own stones aside (e01hollow).
    calm(&mut harness);
    let world = harness.world_mut();
    for (size, x) in StoneSize::ALL.into_iter().zip([1500.0, 1600.0, 1700.0]) {
        let half = STONE_HALVES[size as usize];
        props::spawn(
            world,
            PropKind::Stone(size),
            Vec2::new(x, TOP - half.y - 0.5),
        );
    }
    harness.step(30);
    // Each stone's save ID, size, look, point and velocity.
    type Rubble = (u64, StoneSize, usize, [f32; 2], [f32; 2]);
    let stones = |harness: &UiHarness| {
        let mut stones: Vec<Rubble> = harness
            .world()
            .query::<(&Saved, &Stone, &Position, &Velocity)>()
            .map(|(id, stone, at, velocity)| {
                let (at, velocity) = (at.0.to_array(), velocity.0.to_array());
                (id.id().0, stone.size, stone.variant, at, velocity)
            })
            .collect();
        stones.sort_by_key(|(id, ..)| *id);
        stones
    };
    let kept = stones(&harness);
    assert_eq!(kept.len(), 3, "the three stones");
    let sizes: Vec<StoneSize> = kept.iter().map(|&(_, size, ..)| size).collect();
    assert_eq!(sizes, StoneSize::ALL);
    let variants: BTreeSet<usize> = kept.iter().map(|&(_, _, variant, ..)| variant).collect();
    assert_eq!(variants.len(), 2, "both looks, by the IDs' parity");
    let saved = capture(&harness);
    // Through JSON, as Continue reads a slot.
    let loaded: SaveGame = serde_json::from_value(json(&saved)).expect("a save loads");
    let mut fresh = super::harness();
    load(&mut fresh, loaded);
    assert_eq!(
        stones(&fresh),
        kept,
        "each at its place, size, look and velocity"
    );
}

/// The map's geysers, with their save IDs, in the map's order.
fn geysers(harness: &UiHarness) -> Vec<(u64, Entity)> {
    ["geyser_floor_west", "geyser_floor_east", "geyser_shelf"]
        .map(|name| {
            let id = SaveId::authored(name).0;
            (id, by_id(harness, id))
        })
        .into()
}

#[test]
fn the_maps_saved_objects_and_the_player_are_named_with_the_save_ids_their_names_derive() {
    let (harness, player) = run();
    let world = harness.world();
    let state = world.resource::<tungsten::core::save::SaveState>();
    let names = &world.resource::<level::SavedNames>().0;
    for (entity, name) in names {
        let id = SaveId::authored(name);
        let saved = world.get::<Saved>(*entity).expect("a saved map object");
        assert_eq!(saved.id(), id, "{name}");
        assert_eq!(saved.area(), Some(level::MAP), "{name}");
        assert_eq!(state.name_of(id), Some(name.as_str()), "{name}, named");
    }
    let count = |kind: fn(&World, Entity) -> bool| {
        names
            .iter()
            .filter(|(entity, _)| kind(world, *entity))
            .count() as u64
    };
    assert_eq!(count(|w, e| w.has::<Geyser>(e)), save::MAP_GEYSERS);
    assert_eq!(count(|w, e| w.has::<Enemy>(e)), MAP_ENEMIES);
    assert_eq!(count(|w, e| w.has::<Orb>(e)), save::MAP_PICKUPS);
    assert_eq!(count(|w, e| w.has::<Dummy>(e)), save::MAP_DUMMIES);
    let id = SaveId::authored("player");
    assert_eq!(id_of(&harness, player), id.0);
    assert_eq!(state.name_of(id), Some("player"));
    assert_eq!(world.get::<Saved>(player).unwrap().area(), None, "global");
}

/// The geysers' states by save ID: each one's ID, phase, the phase's
/// seconds left and its choke's.
fn geyser_states(harness: &UiHarness) -> Vec<(u64, GeyserPhase, f32, f32)> {
    let mut states: Vec<(u64, GeyserPhase, f32, f32)> = harness
        .world()
        .query::<(&Saved, &Geyser)>()
        .map(|(id, geyser)| (id.id().0, geyser.phase, geyser.left, geyser.choked))
        .collect();
    states.sort_by_key(|&(id, ..)| id);
    states
}

#[test]
fn a_geyser_saved_mid_eruption_and_one_saved_choked_load_back_to_finish_as_they_would_have() {
    let (mut harness, _) = run();
    harness.harness_mut().set_dt(1.0 / 60.0);
    let [(_, erupting), (_, choked), _] = geysers(&harness)[..] else {
        panic!("the map's three geysers")
    };
    geyser::set(harness.world_mut(), erupting, GeyserPhase::Erupt, 0.4, 0.0);
    geyser::set(
        harness.world_mut(),
        choked,
        GeyserPhase::Idle,
        geyser::IDLE_SECS,
        3.0,
    );
    let saved = capture(&harness);
    let read: SaveGame = serde_json::from_value(json(&saved)).expect("a save reads");
    let mut fresh = super::harness();
    fresh.harness_mut().set_dt(1.0 / 60.0);
    load(&mut fresh, read);
    fresh.world_mut().resource_mut::<Time>().resume();
    let touched = |harness: &UiHarness, at: usize| {
        harness.world().has::<ContactDamage>(geysers(harness)[at].1)
    };
    assert!(touched(&fresh, 0), "loaded erupting, its touch on");
    assert!(!touched(&fresh, 1), "loaded choked, its touch held");
    // Each step on, the loaded run's geysers as the saved run's.
    let (mut ended, mut sputtered) = (None, None);
    for step in 1..=420_u32 {
        harness.step(1);
        fresh.step(1);
        let (a, b) = (geyser_states(&harness), geyser_states(&fresh));
        for (want, got) in a.iter().zip(&b) {
            assert_eq!((want.0, want.1), (got.0, got.1), "step {step}");
            assert!((want.2 - got.2).abs() < 1e-4, "step {step}");
            assert!((want.3 - got.3).abs() < 1e-4, "step {step}");
        }
        assert_eq!(touched(&fresh, 0), touched(&harness, 0), "step {step}");
        if ended.is_none() && !touched(&fresh, 0) {
            ended = Some(step);
        }
        if sputtered.is_none() && b[1].1 == GeyserPhase::Sputter {
            sputtered = Some(step);
        }
    }
    let ended = ended.expect("the eruption ended");
    assert!(ended.abs_diff(24) <= 1, "0.4 s left: {ended}");
    let sputtered = sputtered.expect("the choked one sputtered");
    assert!(
        sputtered.abs_diff(330) <= 2,
        "3 s choked, 2.5 s idle: {sputtered}"
    );
}

#[test]
fn a_fresh_run_sets_nine_stones_down() {
    let (mut harness, _) = run();
    harness.step(30);
    let world = harness.world();
    let mut sizes: BTreeMap<&str, usize> = BTreeMap::new();
    for (stone, at, collider) in world.query::<(&Stone, &Position, &tungsten::core::Collider)>() {
        *sizes.entry(stone.size.name()).or_default() += 1;
        let half = STONE_HALVES[stone.size as usize];
        let foot = at.0.y + collider.offset.y + half.y;
        assert!(
            (foot - 63.0 * 64.0).abs() < 1.0,
            "on the hollow's floor: {foot}"
        );
    }
    assert_eq!(
        sizes,
        BTreeMap::from([("large", 3), ("medium", 3), ("small", 3)])
    );
}

/// Steps until a flask's burst shows, at most 120 frames: the frames it
/// took and where.
fn flask_landing(harness: &mut UiHarness) -> Option<(u32, Vec2)> {
    for frame in 1..=120 {
        harness.step(1);
        let world = harness.world();
        let config = world
            .resource::<tungsten::core::ParticleConfigRegistry>()
            .id_for_name(toys::flask::BURST_PARTICLES);
        let burst = world
            .query::<(
                &example_01_platformer::shared::Burst,
                &tungsten::core::ParticleEmitter,
                &Transform,
            )>()
            .find(|(_, emitter, _)| Some(emitter.config) == config)
            .map(|(.., at)| at.position);
        if let Some(at) = burst {
            return Some((frame, at));
        }
    }
    None
}

/// A flask in flight: its save ID, point, flight, range so far and the save
/// ID of what threw it.
type Flask = (
    u64,
    [f32; 2],
    Projectile,
    tungsten::kit::LinearMotion,
    Option<u64>,
);

/// The flasks in flight, by save ID.
fn flasks(harness: &UiHarness) -> Vec<Flask> {
    let world = harness.world();
    let id = |entity: Entity| example_01_platformer::shared::save_id(world, entity).map(|id| id.0);
    let mut flying: Vec<Flask> = toys::flask::flying(world)
        .into_iter()
        .map(|(own, flask)| {
            (
                own.0,
                world.get::<Position>(flask).unwrap().0.to_array(),
                world.get::<Projectile>(flask).unwrap().clone(),
                world
                    .get::<tungsten::kit::LinearMotion>(flask)
                    .unwrap()
                    .clone(),
                world
                    .get::<tungsten::kit::Source>(flask)
                    .and_then(|source| id(source.entity())),
            )
        })
        .collect();
    flying.sort_by_key(|flask| flask.0);
    flying
}

#[test]
fn a_flask_in_flight_saves_loads_and_bursts_where_it_would_have() {
    let (mut harness, player) = grounded();
    harness.harness_mut().set_dt(1.0 / 60.0);
    let from = centre(&harness, player);
    let to = Vec2::new(from.x + 700.0, TOP);
    let thrown = toys::flask::throw(harness.world_mut(), player, from, to);
    assert!(thrown.is_some(), "a flask flies");
    harness.step(6);
    let flying = flasks(&harness);
    assert_eq!(flying.len(), 1, "the flask flies");
    assert_eq!(flying[0].4, Some(id_of(&harness, player)), "the thrower's");
    let saved = capture(&harness);
    let read: SaveGame = serde_json::from_value(json(&saved)).expect("the save reads");
    let mut loaded = reloaded(read, false);
    loaded.harness_mut().set_dt(1.0 / 60.0);
    assert_eq!(flasks(&loaded), flying, "the flask back");
    let (before, after) = (flask_landing(&mut harness), flask_landing(&mut loaded));
    let (steps, at) = before.expect("it bursts");
    let (loaded_steps, loaded_at) = after.expect("it bursts after the load");
    assert_eq!(loaded_steps, steps, "as soon");
    assert!(at.distance(loaded_at) < 0.5, "{at} and {loaded_at}");
}

/// Each saved `projectile` among `value`'s records without its `end`,
/// `shape`, `offset` and `body_walls`, as a save from before the flask's
/// splash was its end's blast; how many had an end.
fn strip_flask(value: &mut serde_json::Value) -> usize {
    let mut stripped = 0;
    match value {
        serde_json::Value::Object(object) => {
            if let Some(serde_json::Value::Object(projectile)) = object.get_mut("projectile") {
                for field in ["shape", "offset", "body_walls"] {
                    projectile.remove(field);
                }
                if projectile.remove("end").is_some() {
                    stripped += 1;
                }
            }
            for inner in object.values_mut() {
                stripped += strip_flask(inner);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                stripped += strip_flask(item);
            }
        }
        _ => {}
    }
    stripped
}

/// Steps until `goblin` takes a hit's poison, at most 120 frames: the
/// frames it took, the poison and its source's save ID.
fn poisoned_at(harness: &mut UiHarness, goblin: Entity) -> Option<(u32, f32, Option<u64>)> {
    for frame in 1..=120 {
        harness.step(1);
        let world = harness.world();
        let id =
            |entity: Entity| example_01_platformer::shared::save_id(world, entity).map(|id| id.0);
        let poison = harness
            .harness()
            .events::<tungsten::kit::DamagedPart>()
            .find(|part| {
                part.entity == goblin && !part.periodic && part.kinds.has(world, "damage.poison")
            })
            .map(|part| (frame, part.amount, part.source.and_then(id)));
        if poison.is_some() {
            return poison;
        }
    }
    None
}

#[test]
fn an_older_flask_still_splashes() {
    let (mut harness, player) = grounded();
    harness.harness_mut().set_dt(1.0 / 60.0);
    // A still goblin on the ground 400 px off, and a flask thrown at it.
    let from = centre(&harness, player);
    let kind = EnemyKind::Goblin;
    let at = Vec2::new(from.x + 400.0, TOP - kind_body(kind).half.y - 0.5);
    let goblin = enemy(&mut harness, kind, at);
    harness
        .world_mut()
        .remove_component::<StateMachine<EnemyState>>(goblin);
    harness.step(1);
    assert!(toys::flask::throw(harness.world_mut(), player, from, at).is_some());
    harness.step(6);
    assert_eq!(toys::flask::count(harness.world()), 1, "the flask flies");
    let goblin_id = id_of(&harness, goblin);
    let mut older = json(&capture(&harness));
    assert_eq!(strip_flask(&mut older["world"]), 1, "the flask's end");
    let read: SaveGame = serde_json::from_value(older).expect("the save reads");
    let mut loaded = reloaded(read, false);
    loaded.harness_mut().set_dt(1.0 / 60.0);
    let flask = toys::flask::flying(loaded.world())[0].1;
    let projectile = loaded.world().get::<Projectile>(flask).unwrap();
    assert!(
        projectile.end_blast().is_some()
            && projectile.shape().is_some()
            && projectile.body_walls() != 0,
        "its prefab's end, shape and body walls"
    );
    let loaded_goblin = by_id(&loaded, goblin_id);
    let run = poisoned_at(&mut harness, goblin).expect("the run's flask splashes it");
    let after = poisoned_at(&mut loaded, loaded_goblin).expect("the loaded flask splashes it");
    assert_eq!(after.0, run.0, "as soon");
    assert!(
        (after.1 - run.1).abs() < 1e-4,
        "as much: {after:?} and {run:?}"
    );
    assert_eq!(after.2, Some(id_of(&harness, player)), "from the wizard");
    assert_eq!(after.2, run.2);
    // Its status's band enters at chemistry's next run.
    harness.step(2);
    loaded.step(2);
    let poisoned =
        |h: &UiHarness, e: Entity| h.world().get::<Tags>(e).unwrap().has("status.poisoned");
    assert!(poisoned(&harness, goblin), "poisoned in the run");
    assert!(poisoned(&loaded, loaded_goblin), "poisoned after the load");
}

/// Each `impact_damage` among `value`'s records taken out, as a save from
/// before 01's bodies hit by the kit's impact rule; how many there were.
fn strip_impacts(value: &mut serde_json::Value) -> usize {
    let mut stripped = 0;
    match value {
        serde_json::Value::Object(object) => {
            if object.remove("impact_damage").is_some() {
                stripped += 1;
            }
            for inner in object.values_mut() {
                stripped += strip_impacts(inner);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                stripped += strip_impacts(item);
            }
        }
        _ => {}
    }
    stripped
}

/// `body` set at `target`'s left side, a pixel off it, flying at it at
/// `speed`, its bounce off, thrown by `thrower` at `factor` when given, its
/// rule set as a throw sets it.
fn flung_at(
    harness: &mut UiHarness,
    body: Entity,
    target: Entity,
    speed: f32,
    thrower: Option<(Entity, f32)>,
) {
    let half = |h: &UiHarness, e: Entity| {
        h.world()
            .get::<tungsten::core::Collider>(e)
            .unwrap()
            .shape
            .half_extents()
            .x
    };
    let gap = Vec2::new(half(harness, target) + half(harness, body) + 1.0, 0.0);
    let side = example_01_platformer::shared::body_centre(harness.world(), target).unwrap() - gap;
    let id = thrower.map(|(by, _)| id_of(harness, by));
    let world = harness.world_mut();
    let corner = side - world.get::<tungsten::core::Collider>(body).unwrap().offset;
    world.get_mut::<Position>(body).unwrap().0 = corner;
    world.get_mut::<PrevPosition>(body).unwrap().0 = corner;
    world.get_mut::<Velocity>(body).unwrap().0 = Vec2::new(speed, 0.0);
    if let Some(own) = world.get_mut::<tungsten::core::RigidBody>(body) {
        own.restitution = 0.0;
    }
    if let (Some((_, factor)), Some(thrower)) = (thrower, id) {
        let throw = Throw {
            body,
            thrower,
            factor,
            left: 2.0,
        };
        world.resource_mut::<Throws>().0.push(throw);
        elements::systems::arm_throw(world, &throw);
    }
}

/// A still enemy of `kind` standing on the ground at `x`.
fn still_enemy(harness: &mut UiHarness, kind: EnemyKind, x: f32) -> Entity {
    let foe = enemy(
        harness,
        kind,
        Vec2::new(x, TOP - kind_body(kind).half.y - 0.5),
    );
    harness
        .world_mut()
        .remove_component::<StateMachine<EnemyState>>(foe);
    foe
}

/// The hits each of `targets` took in one step: its parts' amounts, and
/// the save ID each came from.
fn impacts_on(harness: &mut UiHarness, targets: &[u64]) -> Vec<Vec<(f32, Option<u64>)>> {
    harness.step(1);
    let world = harness.world();
    let id = |entity: Entity| example_01_platformer::shared::save_id(world, entity).map(|id| id.0);
    targets
        .iter()
        .map(|&target| {
            harness
                .harness()
                .events::<tungsten::kit::DamagedPart>()
                .filter(|part| id(part.entity) == Some(target) && !part.periodic)
                .map(|part| (part.amount, part.source.and_then(id)))
                .collect()
        })
        .collect()
}

#[test]
fn older_bodies_still_hit() {
    let (mut harness, player) = grounded();
    harness.harness_mut().set_dt(1.0 / 60.0);
    // Four still goblins, a troll and a goblin to throw at them, and a
    // frozen bat.
    let marks: Vec<Entity> = [1100.0, 1400.0, 1700.0, 2000.0]
        .into_iter()
        .map(|x| still_enemy(&mut harness, EnemyKind::Goblin, x))
        .collect();
    let troll = still_enemy(&mut harness, EnemyKind::Troll, 2600.0);
    let thrower = still_enemy(&mut harness, EnemyKind::Goblin, 2900.0);
    let crate_ = props::spawn(
        harness.world_mut(),
        PropKind::Crate,
        Vec2::new(800.0, TOP - CRATE_HALF.y - 0.5),
    );
    let rock = props::lob_rock(
        harness.world_mut(),
        Vec2::new(850.0, TOP - 40.0),
        Vec2::ZERO,
    );
    let bat = enemy(&mut harness, EnemyKind::Bat, Vec2::new(900.0, TOP - 300.0));
    tungsten::kit::EffectRequest::new(bat, tungsten::core::DataRef::by_id("frozen"))
        .apply(harness.world_mut());
    let block = toys::place_iron(
        harness.world_mut(),
        Vec2::new(950.0, TOP - IRON_HALF.y - 0.5),
    );
    harness.step(2);
    // Each mid-flight at its mark: the crate and the bat thrown by the
    // troll, the rock by the goblin, at Hard's factor; the iron the
    // player's.
    flung_at(&mut harness, crate_, marks[0], 900.0, Some((troll, 1.5)));
    flung_at(&mut harness, rock, marks[1], 900.0, Some((thrower, 1.5)));
    flung_at(&mut harness, bat, marks[2], 2400.0, Some((troll, 1.5)));
    flung_at(&mut harness, block, marks[3], 700.0, None);
    let ids: Vec<u64> = marks.iter().map(|&mark| id_of(&harness, mark)).collect();
    let mut older = json(&capture(&harness));
    assert!(
        strip_impacts(&mut older["world"]) >= 4,
        "the records' rules"
    );
    let read: SaveGame = serde_json::from_value(older).expect("the save reads");
    let mut loaded = reloaded(read, false);
    loaded.harness_mut().set_dt(1.0 / 60.0);
    // A body's bounce is its prefab's again after the load: off, as the
    // run's.
    for body in [crate_, rock, block] {
        let loaded_body = by_id(&loaded, id_of(&harness, body));
        if let Some(own) = loaded
            .world_mut()
            .get_mut::<tungsten::core::RigidBody>(loaded_body)
        {
            own.restitution = 0.0;
        }
    }
    let run = impacts_on(&mut harness, &ids);
    let after = impacts_on(&mut loaded, &ids);
    let (troll, thrower, me) = (
        id_of(&harness, troll),
        id_of(&harness, thrower),
        id_of(&harness, player),
    );
    for (i, (run, after)) in run.iter().zip(&after).enumerate() {
        assert!(!run.is_empty(), "mark {i} hit in the run");
        assert_eq!(after.len(), run.len(), "mark {i}: {after:?} and {run:?}");
        for (got, want) in after.iter().zip(run) {
            assert!(
                (got.0 - want.0).abs() < 0.05,
                "mark {i}: {after:?} and {run:?}"
            );
            assert_eq!(got.1, want.1, "mark {i}");
        }
    }
    assert_eq!(after[0][0].1, Some(troll), "the crate's thrower");
    assert!(
        (after[1][0].0 - 12.0).abs() < 1e-3 && after[1][0].1 == Some(thrower),
        "the rock's flat 8 at 1.5: {:?}",
        after[1]
    );
    assert_eq!(after[2][0].1, Some(troll), "the bat's thrower");
    assert_eq!(after[3][0].1, Some(me), "the iron the player's");
    let along = loaded
        .world()
        .get::<tungsten::kit::Knockback>(by_id(&loaded, ids[1]))
        .map(|knockback| knockback.push());
    assert!(
        along.is_some_and(|push| push.x > 0.0),
        "pushed along its flight: {along:?}"
    );
    let _ = rock;
}

/// Puts one more enemy on `harness`'s map before a run spawns it: a copy of
/// the meadow goblin named `name`, four tiles east of it.
fn with_one_more_enemy(harness: &mut UiHarness, name: &str) {
    let world = harness.world_mut();
    let mut map = world
        .resource::<TilemapRegistry>()
        .get(level::MAP)
        .expect("the level's map")
        .clone();
    let next = map
        .object_layers
        .iter()
        .flat_map(|layer| &layer.objects)
        .map(|object| object.id)
        .max()
        .unwrap_or(0)
        + 1;
    let layer = map
        .object_layers
        .iter_mut()
        .find(|layer| layer.name == "enemies")
        .expect("the enemies' layer");
    let mut added = layer
        .objects
        .iter()
        .find(|object| object.name == "meadow goblin")
        .expect("the meadow goblin")
        .clone();
    added.id = next;
    added.name = name.to_owned();
    added.position.x += 4.0 * level::TILE;
    layer.objects.push(added);
    world
        .resource_mut::<TilemapRegistry>()
        .insert(level::MAP.to_owned(), map);
}

#[test]
fn an_enemy_added_to_the_map_needs_no_schema_bump() {
    // P7-9: a run on the shipped map, an enemy killed and one hurt, saved
    // at today's schema, loads into a map with one more enemy as it is.
    let (mut harness, _) = run();
    harness.step(2);
    let placed = enemies::live(harness.world());
    let [(_, killed), (hurt_id, hurt), ..] = placed[..] else {
        panic!("the map's enemies")
    };
    tungsten::kit::damage(harness.world_mut(), killed, 10_000.0);
    tungsten::kit::damage(harness.world_mut(), hurt, 5.0);
    harness.step(3);
    assert!(!harness.world().is_alive(killed), "killed");
    let saved = json(&capture(&harness));
    // Written to a slot and read back, as Continue reads one.
    let dir = user_folder("added-enemy");
    let slots = SaveSlots::new(Some(dir.clone()));
    let written: SaveGame = serde_json::from_value(saved.clone()).expect("the save reads");
    slots.save("slot_1", &written).expect("the slot is written");
    let read: SaveGame = slots.load("slot_1").expect("the slot loads");
    let _ = std::fs::remove_dir_all(dir);
    let mut differ = Vec::new();
    differences(&json(&read), &saved, "", &mut differ);
    assert!(differ.is_empty(), "read as it was written: {differ:?}");
    let read_again = read.clone();
    let mut fresh = super::harness();
    with_one_more_enemy(&mut fresh, "added goblin");
    load(&mut fresh, read);
    // The new enemy stands at its home; every saved object came back by
    // its ID, as it was saved.
    let added = by_id(&fresh, SaveId::authored("added goblin").0);
    let enemy = *fresh.world().get::<Enemy>(added).unwrap();
    assert_eq!(enemy.kind, EnemyKind::Goblin);
    assert!(super::body(&fresh, added).distance(enemy.home()) < 1.0);
    let health = |h: &UiHarness| {
        h.world()
            .get::<Health>(by_id(h, hurt_id))
            .unwrap()
            .current()
    };
    assert_eq!(health(&fresh), health(&harness), "the hurt one");
    // Every saved enemy back by its ID, where it was and as hurt, and the
    // added one beside them.
    let live = |h: &UiHarness| -> BTreeMap<u64, ([f32; 2], f32)> {
        let world = h.world();
        enemies::live(world)
            .into_iter()
            .map(|(id, enemy)| {
                let at = world.get::<Position>(enemy).unwrap().0.to_array();
                (id, (at, world.get::<Health>(enemy).unwrap().current()))
            })
            .collect()
    };
    let mut expected = live(&harness);
    let added_id = SaveId::authored("added goblin").0;
    expected.insert(added_id, live(&fresh)[&added_id]);
    assert_eq!(
        live(&fresh),
        expected,
        "the saved enemies, and the added one"
    );
    // The same save in the shipped map loads whole.
    let mut plain = super::harness();
    load(&mut plain, read_again);
    let mut differ = Vec::new();
    differences(&json(&capture(&plain)), &saved, "", &mut differ);
    assert!(differ.is_empty(), "the save, loaded whole: {differ:?}");
}

/// Each saved `stats` under `value` holding arcane power's `damage_dealt`,
/// in schema 29's shape: the tome's `spell_power` in its place.
fn arcane_as_schema_29(value: &mut serde_json::Value) -> usize {
    let mut rewritten = 0;
    match value {
        serde_json::Value::Object(object) => {
            if let Some(serde_json::Value::Object(stats)) = object.get_mut("stats")
                && stats.remove("damage_dealt").is_some()
            {
                stats.insert(
                    "spell_power".to_owned(),
                    serde_json::json!({
                        "base": 1.0, "min": 0.0, "max": null,
                        "modifiers": [["tome.arcane", { "mul": 2.0 }]]
                    }),
                );
                rewritten += 1;
            }
            for inner in object.values_mut() {
                rewritten += arcane_as_schema_29(inner);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                rewritten += arcane_as_schema_29(item);
            }
        }
        _ => {}
    }
    rewritten
}

#[test]
fn a_schema_29_save_keeps_its_arcane_power() {
    use example_01_platformer::features::tomes::TomeKind;
    use example_01_platformer::features::{gun, lightning};
    use tungsten::kit::{Read, TagSet};

    use super::combat::dummy_at;
    use super::tomes::{first_part, take};

    // What a strike and a shot deal a fresh dummy at x 900, from the
    // player on the sky's ground at x 300.
    let strike = |harness: &mut UiHarness, player: Entity| {
        let dummy = dummy_at(harness, 900.0);
        harness.step(2);
        let at = super::body(harness, dummy);
        first_part(harness, dummy, 120, |harness| {
            lightning::strike(harness.world_mut(), player, at);
        })
    };
    let shot = |harness: &mut UiHarness, _: Entity| {
        let dummy = dummy_at(harness, 900.0);
        harness.step(2);
        let at = super::body(harness, dummy);
        first_part(harness, dummy, 120, |harness| {
            aim_at(harness, at);
            harness.harness_mut().press_action(gun::SHOOT);
        })
    };
    let plain = |measure: &dyn Fn(&mut UiHarness, Entity) -> f32| {
        let (mut harness, player) = run();
        ground(&mut harness, TOP);
        stand(&mut harness, player, 300.0, TOP);
        harness.step(2);
        pin_camera(&mut harness, CORNER);
        measure(&mut harness, player)
    };
    // A run whose player holds the tome, written as schema 29 wrote it and
    // loaded through its slot, which migrates it.
    let loaded = |measure: &dyn Fn(&mut UiHarness, Entity) -> f32, tag: &str| {
        let dir = user_folder(tag);
        let mut harness = harness_with(UserFolder::At(dir.clone()));
        start(&mut harness);
        let player = player_entity(&harness);
        ground(&mut harness, TOP);
        stand(&mut harness, player, 300.0, TOP);
        harness.step(2);
        take(&mut harness, player, TomeKind::Arcane);
        let mut older = json(&capture(&harness));
        assert_eq!(arcane_as_schema_29(&mut older), 1, "the player's stats");
        let stamp = tungsten::core::SaveStamp::new(29, "0.117.0", 1_760_400_000_000);
        let slots = harness_slots(&harness);
        slots
            .save_stamped("slot_1", &older, &stamp)
            .expect("a schema 29 save is written");
        let save = slots
            .load::<SaveGame>("slot_1")
            .expect("a schema 29 save migrates");
        load(&mut harness, save);
        harness.world_mut().resource_mut::<Time>().resume();
        let player = player_entity(&harness);
        let stats = harness.world().get::<Stats>(player).unwrap();
        assert_eq!(stats.base("spell_power"), None, "spell_power gone");
        let spell = [TagSet::Name("hit.spell")];
        let read = |sets| stats.read(Stats::DAMAGE_DEALT, &Read::new(sets));
        assert_eq!((read(&spell), read(&[])), (Some(2.0), Some(1.0)));
        ground(&mut harness, TOP);
        stand(&mut harness, player, 300.0, TOP);
        harness.step(2);
        pin_camera(&mut harness, CORNER);
        let dealt = measure(&mut harness, player);
        let _ = std::fs::remove_dir_all(dir);
        dealt
    };
    let (strike_plain, strike_loaded) = (plain(&strike), loaded(&strike, "arcane-29-strike"));
    let (shot_plain, shot_loaded) = (plain(&shot), loaded(&shot, "arcane-29-shot"));
    assert!(strike_plain > 0.0 && shot_plain > 0.0);
    assert!(
        (strike_loaded - 2.0 * strike_plain).abs() < 1e-3,
        "the strike doubled: {strike_loaded} for {strike_plain}"
    );
    assert!(
        (shot_loaded / shot_plain - 1.0).abs() < 0.01,
        "the shot as it was: {shot_loaded} for {shot_plain}"
    );
}

/// Each saved `health` among `value`'s records without its `disarm`, as a
/// save from before the death policy wrote it; how many it stripped.
fn strip_disarm(value: &mut serde_json::Value) -> usize {
    let mut stripped = 0;
    match value {
        serde_json::Value::Object(object) => {
            if let Some(serde_json::Value::Object(health)) = object.get_mut("health")
                && health.remove("disarm").is_some()
            {
                stripped += 1;
            }
            for inner in object.values_mut() {
                stripped += strip_disarm(inner);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                stripped += strip_disarm(item);
            }
        }
        _ => {}
    }
    stripped
}

#[test]
fn older_foes_still_disarm() {
    use tungsten::kit::kill;

    let (mut harness, _) = grounded();
    // Two records: a crawler, which hurts by touch, and a beetle, whose
    // charge's hitbox is armed for the test.
    let crawler = enemy(
        &mut harness,
        EnemyKind::Crawler,
        Vec2::new(880.0, TOP - 40.0),
    );
    let beetle = enemy(
        &mut harness,
        EnemyKind::Beetle,
        Vec2::new(1080.0, TOP - 13.5),
    );
    harness.step(1);
    let ids = (id_of(&harness, crawler), id_of(&harness, beetle));
    let mut older = json(&capture(&harness));
    let records = &mut older["world"];
    assert!(strip_disarm(records) >= 2, "the two records' health");
    let read: SaveGame = serde_json::from_value(older).expect("the save reads");
    let mut loaded = reloaded(read, false);
    let (crawler, beetle) = (by_id(&loaded, ids.0), by_id(&loaded, ids.1));
    let world = loaded.world_mut();
    for foe in [crawler, beetle] {
        assert!(
            world.get::<Health>(foe).unwrap().disarms(),
            "its prefab's disarm"
        );
    }
    world.get_mut::<Hitbox>(beetle).unwrap().set_on(true);
    assert!(world.get::<ContactDamage>(crawler).unwrap().is_on());
    for foe in [crawler, beetle] {
        assert_eq!(kill(world, foe), tungsten::kit::Hit::Killed);
    }
    assert!(
        !world.get::<ContactDamage>(crawler).unwrap().is_on(),
        "its touch off at the kill"
    );
    assert!(
        !world.get::<Hitbox>(beetle).unwrap().is_on(),
        "its hitbox off at the kill"
    );
    loaded.step(2);
    let world = loaded.world();
    if world.is_alive(crawler)
        && let Some(touch) = world.get::<ContactDamage>(crawler)
    {
        assert!(!touch.is_on(), "left off while it lies dead");
    }
}

/// Each saved `pickup` among `value`'s records without its `fall` and
/// `fall_layers`, as a save from before the orbs fell by the kit's pickup;
/// how many had a fall.
fn strip_fall(value: &mut serde_json::Value) -> usize {
    let mut stripped = 0;
    match value {
        serde_json::Value::Object(object) => {
            if let Some(serde_json::Value::Object(pickup)) = object.get_mut("pickup") {
                pickup.remove("fall_layers");
                if pickup.remove("fall").is_some() {
                    stripped += 1;
                }
            }
            for inner in object.values_mut() {
                stripped += strip_fall(inner);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                stripped += strip_fall(item);
            }
        }
        _ => {}
    }
    stripped
}

fn point(harness: &UiHarness, entity: Entity) -> Vec2 {
    harness.world().get::<Position>(entity).expect("a point").0
}

#[test]
fn older_orbs_still_fall() {
    let (mut harness, player) = grounded();
    // A dropped orb in the air past the player's reach, and the placed orb
    // farthest from the player, which floats.
    let orb = drops::spawn(
        harness.world_mut(),
        OrbKind::Mana,
        Vec2::new(1400.0, TOP - 200.0),
    )
    .expect("an orb");
    harness.step(1);
    let me = centre(&harness, player);
    let placed = placed_orbs(&harness)
        .into_iter()
        .map(|(_, orb)| orb)
        .max_by(|&a, &b| {
            let far = |orb| point(&harness, orb).distance(me);
            far(a).total_cmp(&far(b))
        })
        .expect("the map's placed orbs");
    let ids = (id_of(&harness, orb), id_of(&harness, placed));
    let (aloft, float) = (point(&harness, orb), point(&harness, placed));
    let mut older = json(&capture(&harness));
    assert_eq!(strip_fall(&mut older["world"]), 1, "the dropped orb's fall");
    let read: SaveGame = serde_json::from_value(older).expect("the save reads");
    // The run as it was: the orb lands on the ground, the placed one floats.
    harness.step(150);
    let landed = point(&harness, orb);
    assert!(
        landed.y - aloft.y > 150.0 && (landed.y + 24.0 - TOP).abs() < 0.1,
        "it fell onto the ground: {aloft} to {landed}"
    );
    assert_eq!(point(&harness, placed), float, "the placed orb floats");
    let mut loaded = reloaded(read, false);
    let (orb, placed) = (by_id(&loaded, ids.0), by_id(&loaded, ids.1));
    let world = loaded.world();
    assert_eq!(
        world.get::<Pickup>(orb).unwrap().fall(),
        120.0,
        "its prefab's"
    );
    assert_eq!(
        world.get::<Pickup>(placed).unwrap().fall(),
        0.0,
        "none for the map's"
    );
    assert_eq!(point(&loaded, orb), aloft, "in the air");
    loaded.step(150);
    assert_eq!(point(&loaded, orb), landed, "it lands as the run's did");
    assert_eq!(point(&loaded, placed), float, "the placed orb stays put");
}

/// Each saved foe's `facing` among `value`'s records and patches moved back
/// onto its `enemy` as the `heading` schema 30 wrote, 1 right or -1 left;
/// how many it moved.
fn facing_as_schema_30(value: &mut serde_json::Value) -> usize {
    let mut moved = 0;
    match value {
        serde_json::Value::Object(object) => {
            if object
                .get("enemy")
                .is_some_and(serde_json::Value::is_object)
            {
                let left = object
                    .remove("facing")
                    .and_then(|facing| facing["direction"][0].as_f64())
                    .is_some_and(|x| x < 0.0);
                object["enemy"]["heading"] = serde_json::Value::from(if left { -1.0 } else { 1.0 });
                moved += 1;
            }
            for inner in object.values_mut() {
                moved += facing_as_schema_30(inner);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                moved += facing_as_schema_30(item);
            }
        }
        _ => {}
    }
    moved
}

#[test]
fn an_older_save_keeps_its_foes_facing() {
    let dir = user_folder("facing-30");
    let mut harness = harness_with(UserFolder::At(dir.clone()));
    start(&mut harness);
    let player = player_entity(&harness);
    scaffold(&mut harness);
    stand(&mut harness, player, 600.0, TOP);
    harness.step(1);
    // Two crawlers on the sky's ground, the first turned left, written as
    // schema 30 wrote them and loaded through their slot, which migrates
    // them.
    let left = enemy(
        &mut harness,
        EnemyKind::Crawler,
        Vec2::new(1400.0, TOP - 40.0),
    );
    let right = enemy(
        &mut harness,
        EnemyKind::Crawler,
        Vec2::new(1800.0, TOP - 40.0),
    );
    enemies::turn(harness.world_mut(), left, -1.0);
    let ids = (id_of(&harness, left), id_of(&harness, right));
    let mut older = json(&capture(&harness));
    assert!(
        facing_as_schema_30(&mut older["world"]) >= 2,
        "the two crawlers' records"
    );
    let stamp = tungsten::core::SaveStamp::new(30, "0.117.0", 1_760_400_000_000);
    let slots = harness_slots(&harness);
    slots
        .save_stamped("slot_1", &older, &stamp)
        .expect("a schema 30 save is written");
    let save = slots
        .load::<SaveGame>("slot_1")
        .expect("a schema 30 save migrates");
    load(&mut harness, save);
    let heading = |id| enemies::heading_of(harness.world(), by_id(&harness, id));
    assert_eq!(heading(ids.0), Some(-1.0), "it loads facing left");
    assert_eq!(heading(ids.1), Some(1.0), "and the other right");
    let _ = std::fs::remove_dir_all(dir);
}
