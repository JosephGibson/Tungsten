//! Saves: a capture loads back to itself; every component a saved entity
//! carries is saved, rebuilt from its prefab or the map, or presentation;
//! a cast in the step that asks for an autosave saves its cost and its
//! missile; a save during `casting` loads with the mana still paused; a
//! loaded run keeps with the run it was saved from, in a fresh harness and
//! in a churned world; the template sample's schemas are refused; and a
//! full pit saves within 2 MiB.

use std::any::type_name as name;
use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;

use example_01_platformer::features::elements;
use example_01_platformer::features::elements::components::{BallBurn, Frozen};
use example_01_platformer::features::player::components::Player;
use example_01_platformer::features::toys::components::{
    Ball, BlackHole, FireMissile, IronBlock, IronScrap, SmallBall,
};
use example_01_platformer::features::toys::{self, BALL_CAP, IRON_HALF};
use example_01_platformer::game;
use example_01_platformer::save::{self, SaveGame};
use example_01_platformer::shared::SaveId;
use example_01_platformer::shell::{self as game_shell, RunStart, SlotState};
use glam::Vec2;
use tungsten::core::save::Value;
use tungsten::core::{Entity, Position, SaveData, SaveSlots, Time, Transform, UserFolder, World};
use tungsten::kit::{Abilities, Health, Oscillator, PathFollower, Stats, Tags};
use tungsten::testing::UiHarness;

use super::{
    aim_at, centre, choose, floor, ground, harness, harness_with, mana, pin_camera, player_entity,
    run, settle, stand, start, teleport, top, user_folder,
};

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

/// Casts slot `slot` at `at`, `cast` held `frames` frames.
fn cast(harness: &mut UiHarness, slot: usize, at: Vec2, frames: u32) {
    aim_at(harness, at);
    choose(harness, slot);
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
        .query::<(Entity, &SmallBall, &Position, &SaveId)>()
        .map(|(e, _, at, id)| (at.0.y, id.0, e))
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

/// Every kind a save holds, by every spell: two iron blocks, one frozen and
/// shattered by a missile, the other chilled; a pit of 500 small balls,
/// its crest burning; a bowling ball, a push, a hole and a missile in
/// flight.
fn busy(harness: &mut UiHarness) {
    rich(harness);
    let iron = TOP - IRON_HALF.y - 10.0;
    cast(harness, 6, Vec2::new(850.0, iron), 1);
    harness.step(30);
    cast(harness, 6, Vec2::new(1150.0, iron), 1);
    harness.step(60);
    cast(harness, 2, Vec2::new(850.0, TOP - 60.0), 100);
    cast(harness, 0, Vec2::new(850.0, TOP - 60.0), 1);
    harness.step(30);
    cast(harness, 4, Vec2::new(900.0, TOP - 100.0), 1);
    cast(harness, 8, Vec2::new(300.0, 500.0), 192);
    harness.step(120);
    // The flamethrower lights what its short cone reaches; the crest, over
    // the pit's wall, catches by hand.
    cast(harness, 1, Vec2::new(700.0, TOP - 20.0), 30);
    light(harness, 5);
    cast(harness, 7, Vec2::new(900.0, TOP - 100.0), 1);
    harness.step(60);
    cast(harness, 5, Vec2::new(1700.0, 700.0), 1);
    harness.step(60);
    cast(harness, 0, Vec2::new(700.0, 0.0), 1);
    let far = harness
        .world()
        .query::<(Entity, &IronBlock, &Position)>()
        .max_by(|a, b| a.2.0.x.total_cmp(&b.2.0.x))
        .map(|(e, ..)| e)
        .unwrap();
    elements::chill(harness.world_mut(), far, 0.5, Vec2::new(1090.0, TOP - 60.0));
    harness.step(1);
}

#[test]
fn a_capture_round_trips_through_a_load_with_a_hazard_and_a_platform_displaced() {
    let (mut harness, _) = grounded();
    let first = capture(&harness);
    busy(&mut harness);
    let saved = capture(&harness);
    let moved = |before: &[[f32; 2]], after: &[[f32; 2]]| {
        before
            .iter()
            .zip(after)
            .any(|(a, b)| Vec2::from(*a).distance(Vec2::from(*b)) > 8.0)
    };
    let points = |save: &SaveGame| {
        (
            save.hazards.iter().map(|h| h.position).collect::<Vec<_>>(),
            save.platforms
                .iter()
                .map(|p| p.position)
                .collect::<Vec<_>>(),
        )
    };
    let (hazards, platforms) = points(&first);
    let (hazards_now, platforms_now) = points(&saved);
    assert!(
        moved(&hazards, &hazards_now),
        "a fire hazard is off its spawn"
    );
    assert!(
        moved(&platforms, &platforms_now),
        "a platform is off its spawn"
    );
    assert!(!saved.missiles.is_empty() && saved.black_hole.is_some());
    assert!(saved.blocks.iter().any(|b| b.scrap.is_some()));
    assert!(
        saved
            .blocks
            .iter()
            .any(|b| b.scrap.is_none() && b.chill.is_some())
    );
    assert!(saved.balls.iter().any(|b| b.burn.is_some()));
    let mut fresh = super::harness();
    load(&mut fresh, saved.clone());
    assert_eq!(json(&capture(&fresh)), json(&saved));
}

/// Each type of `T`, by name.
fn names<const N: usize>(types: [&'static str; N]) -> Vec<&'static str> {
    types.to_vec()
}

/// What a kind of saved entity carries: the components a save holds, those
/// its prefab or the map rebuilds, and those that are only presentation.
struct Lists {
    saved: Vec<&'static str>,
    rebuilt: Vec<&'static str>,
    presentation: Vec<&'static str>,
}

fn lists(kind: &str) -> Lists {
    use example_01_platformer::features::elements::components::Chill;
    use example_01_platformer::features::level::components::{
        AmbientEmitter, Area, Glow, GlowParts, Hazard,
    };
    use example_01_platformer::features::player::components::{Lantern, Moves};
    use example_01_platformer::features::spells::components::Spellbook;
    use example_01_platformer::features::toys::components::{
        BallHue, BowlingBall, PreviousVelocity, ScrapTumble, SmallBall,
    };
    use tungsten::SceneEntity;
    use tungsten::core::{
        AnimationState, Collider, ParticleEmitter, ParticleEmitterState, PrevPosition, RigidBody,
        Sprite, Velocity, Visibility,
    };
    use tungsten::kit::{
        ActionInput, Aim, CharacterAnimator, Children, ContactDamage, Cues, Effects, Facing,
        HitFlash, Hurtbox, Intent, Knockback, LinearMotion, PlatformerController, Projectile,
        Source, Stats, Zone,
    };
    // The payload's emitter, which a load spawns from the saved source again.
    const EMITTER: &str = "tungsten_kit::overlap::grid::Emitter";
    let body = [name::<Collider>(), name::<RigidBody>()];
    let at = [name::<SaveId>(), name::<Position>(), name::<PrevPosition>()];
    let drawn = [name::<Transform>(), name::<Sprite>(), name::<Visibility>()];
    match kind {
        "player" => Lists {
            saved: [
                &at[..],
                &[
                    name::<Velocity>(),
                    name::<PlatformerController>(),
                    name::<Health>(),
                    name::<Stats>(),
                    name::<Effects>(),
                    name::<Abilities>(),
                    name::<Tags>(),
                    name::<Spellbook>(),
                    name::<Lantern>(),
                ],
            ]
            .concat(),
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
                ],
            ]
            .concat(),
        },
        "platform" => Lists {
            saved: [&at[..], &[name::<PathFollower>()]].concat(),
            rebuilt: [&body[..], &[name::<SceneEntity>(), name::<Velocity>()]].concat(),
            presentation: drawn.to_vec(),
        },
        "hazard" => Lists {
            saved: [&at[..], &[name::<Oscillator>()]].concat(),
            rebuilt: names([
                name::<SceneEntity>(),
                name::<Hazard>(),
                name::<ContactDamage>(),
                name::<Aim>(),
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
            saved: [
                &at[..],
                &[
                    name::<Velocity>(),
                    name::<SmallBall>(),
                    name::<BowlingBall>(),
                    name::<BallHue>(),
                    name::<BallBurn>(),
                ],
            ]
            .concat(),
            rebuilt: [
                &body[..],
                &[
                    name::<Ball>(),
                    name::<SceneEntity>(),
                    name::<PreviousVelocity>(),
                ],
            ]
            .concat(),
            presentation: [&drawn[..], &[name::<AnimationState>()]].concat(),
        },
        "block" => Lists {
            saved: [
                &at[..],
                &[name::<Velocity>(), name::<Chill>(), name::<Frozen>()],
            ]
            .concat(),
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
            presentation: names([name::<Transform>(), name::<Visibility>()]),
        },
        "scrap" => Lists {
            saved: [
                &at[..],
                &[
                    name::<Velocity>(),
                    name::<IronScrap>(),
                    name::<Chill>(),
                    name::<Frozen>(),
                ],
            ]
            .concat(),
            rebuilt: [
                &body[..],
                &[
                    name::<SceneEntity>(),
                    name::<PreviousVelocity>(),
                    name::<Children>(),
                ],
            ]
            .concat(),
            presentation: [&drawn[..], &[name::<ScrapTumble>()]].concat(),
        },
        "black hole" => Lists {
            saved: names([name::<SaveId>(), name::<Transform>(), name::<BlackHole>()]),
            rebuilt: names([
                name::<SceneEntity>(),
                name::<PreviousVelocity>(),
                name::<Children>(),
                name::<ParticleEmitter>(),
            ]),
            presentation: names([name::<Visibility>(), name::<ParticleEmitterState>()]),
        },
        "missile" => Lists {
            saved: [
                &at[..],
                &[
                    name::<Projectile>(),
                    name::<LinearMotion>(),
                    name::<Source>(),
                    EMITTER,
                ],
            ]
            .concat(),
            rebuilt: names([
                name::<FireMissile>(),
                name::<SceneEntity>(),
                name::<PreviousVelocity>(),
                name::<Glow>(),
                name::<Children>(),
            ]),
            presentation: [&drawn[..], &[name::<AnimationState>(), name::<GlowParts>()]].concat(),
        },
        // The map's still objects: props, slabs, spikes, areas, emitters and
        // the spawn's marker, which the map spawns again whole.
        _ => Lists {
            saved: names([name::<SaveId>()]),
            rebuilt: [
                &body[..],
                &[
                    name::<SceneEntity>(),
                    name::<Position>(),
                    name::<PrevPosition>(),
                    name::<Velocity>(),
                    name::<Hazard>(),
                    name::<ContactDamage>(),
                    name::<Zone>(),
                    name::<Area>(),
                    name::<AmbientEmitter>(),
                    name::<ParticleEmitter>(),
                    name::<Glow>(),
                ],
            ]
            .concat(),
            presentation: [
                &drawn[..],
                &[
                    name::<AnimationState>(),
                    name::<ParticleEmitterState>(),
                    name::<GlowParts>(),
                ],
            ]
            .concat(),
        },
    }
}

/// The kind of saved entity `entity` is.
fn kind(world: &World, entity: Entity) -> &'static str {
    if world.has::<Player>(entity) {
        "player"
    } else if world.has::<PathFollower>(entity) {
        "platform"
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
    } else {
        "scenery"
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
    assert!(world.query::<&Frozen>().count() + world.query::<&IronScrap>().count() > 0);
    assert!(world.query::<&BallBurn>().count() > 0);
    assert_eq!(world.query::<&BlackHole>().count(), 1);
    assert!(world.query::<&FireMissile>().count() > 0);
    let mut seen: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for (entity, _) in world.query::<(Entity, &SaveId)>() {
        let kind = kind(world, entity);
        let lists = lists(kind);
        for (_, ty) in world.component_types(entity) {
            let homes = [&lists.saved, &lists.rebuilt, &lists.presentation]
                .iter()
                .filter(|list| list.contains(&ty))
                .count();
            assert_eq!(homes, 1, "{kind}: {ty} is in {homes} lists, not one");
            seen.entry(kind).or_default().insert(ty);
        }
    }
    let kinds: Vec<&str> = seen.keys().copied().collect();
    assert_eq!(
        kinds,
        [
            "ball",
            "black hole",
            "block",
            "hazard",
            "missile",
            "platform",
            "player",
            "scenery",
            "scrap"
        ]
    );
}

#[test]
fn a_cast_in_the_step_that_asks_for_an_autosave_saves_its_cost_and_its_missile() {
    let dir = user_folder("same-frame");
    let mut harness = harness_with(UserFolder::At(dir.clone()));
    start(&mut harness);
    let player = player_entity(&harness);
    ground(&mut harness, TOP);
    // The Lantern clearing ends at x 1,536; the aqueduct starts there.
    stand(&mut harness, player, 1400.0, TOP);
    harness.step(1);
    pin_camera(&mut harness, CORNER);
    aim_at(&mut harness, Vec2::new(1800.0, 600.0));
    choose(&mut harness, 0);
    assert_eq!(mana(&harness), 100.0);
    // One step casts and walks into the aqueduct, whose entry asks for an
    // autosave; the next frame's capture takes both.
    let eye = centre(&harness, player);
    teleport(&mut harness, player, Vec2::new(1700.0, eye.y));
    harness.harness_mut().press_action("cast");
    harness.step(1);
    harness.harness_mut().release_action("cast");
    harness.step(1);
    let (slots, _) = game_shell::read_slots(harness.world_mut());
    let Some((_, SlotState::Ready(saved, _))) = slots.into_iter().find(|(s, _)| *s == "autosave")
    else {
        panic!("the autosave is written");
    };
    assert_eq!(saved.area.as_deref(), Some("Broken aqueduct"));
    assert_eq!(saved.player.stats.base("mana"), Some(92.0), "its cost");
    assert_eq!(saved.missiles.len(), 1, "and its missile");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_save_during_casting_loads_with_the_mana_still_paused() {
    let (mut harness, _) = run();
    cast(&mut harness, 4, Vec2::new(800.0, 400.0), 1);
    let spent = mana(&harness);
    assert!(spent < 76.0);
    let saved = capture(&harness);
    let casting = |tags: &Tags| tags.iter().any(|tag| tag == "casting");
    assert!(casting(&saved.player.tags));
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
/// what burns and what is frozen, and the player's health, mana and
/// cooldowns.
struct Outcome {
    points: BTreeMap<u64, (&'static str, Vec2)>,
    burning: BTreeSet<u64>,
    frozen: BTreeSet<u64>,
    health: f32,
    mana: f32,
    cooldowns: Vec<f32>,
    player: Vec2,
}

fn outcome(harness: &UiHarness) -> Outcome {
    let world = harness.world();
    let player = player_entity(harness);
    let mut points = BTreeMap::new();
    for (entity, id) in world.query::<(Entity, &SaveId)>() {
        let at = world
            .get::<Position>(entity)
            .map(|p| p.0)
            .or_else(|| world.get::<Transform>(entity).map(|t| t.position))
            .unwrap_or_default();
        points.insert(id.0, (kind(world, entity), at));
    }
    let id = |entity: Entity| world.get::<SaveId>(entity).unwrap().0;
    let abilities = world.get::<Abilities>(player).unwrap();
    Outcome {
        points,
        burning: world
            .query::<(Entity, &BallBurn)>()
            .filter(|(_, burn)| burn.remaining > 0.0)
            .map(|(e, _)| id(e))
            .collect(),
        frozen: world
            .query::<(Entity, &Frozen)>()
            .map(|(e, _)| id(e))
            .collect(),
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
    cast(harness, 2, Vec2::new(950.0, TOP - 60.0), 91);
    // Up, clear of the iron and the pile.
    cast(harness, 0, Vec2::new(700.0, 200.0), 1);
    harness.step(30);
}

fn divergence(churned: bool) {
    let (mut uninterrupted, _) = grounded();
    // Mid-play in the Lantern clearing: iron to the right, a settled pile
    // in the pit to the left, its crest alight.
    cast(
        &mut uninterrupted,
        6,
        Vec2::new(950.0, TOP - IRON_HALF.y - 10.0),
        1,
    );
    cast(&mut uninterrupted, 8, Vec2::new(300.0, 500.0), 90);
    uninterrupted.step(180);
    light(&mut uninterrupted, 5);
    uninterrupted.step(2);
    let saved = capture(&uninterrupted);
    assert_eq!(saved.area.as_deref(), Some("Lantern clearing"));
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
    assert_eq!(a.frozen, b.frozen, "the same iron is frozen");
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

#[test]
fn schemas_one_to_three_are_refused() {
    for from in 1..=3 {
        assert!(
            SaveGame::migrate(from, Value::Null).is_err(),
            "schema {from}"
        );
    }
    // A sample save on disk reads as damaged, never loads.
    let dir = user_folder("schemas");
    let mut harness = harness_with(UserFolder::At(dir.clone()));
    let saves = harness
        .world()
        .resource::<SaveSlots>()
        .dir()
        .expect("a user folder")
        .to_path_buf();
    std::fs::create_dir_all(&saves).unwrap();
    for (slot, schema) in [("slot_1", 1), ("slot_2", 2), ("slot_3", 3)] {
        let envelope = serde_json::json!({
            "envelope": 1, "slot": slot, "schema": schema, "engine_version": "0.92.0",
            "saved_at_ms": 1_760_400_000_000_u64, "data": { "player": [0.0, 0.0] }
        });
        std::fs::write(saves.join(format!("{slot}.json")), envelope.to_string()).unwrap();
    }
    let (slots, _) = game_shell::read_slots(harness.world_mut());
    for slot in ["slot_1", "slot_2", "slot_3"] {
        let state = slots.iter().find(|(s, _)| *s == slot).map(|(_, s)| s);
        assert!(matches!(state, Some(SlotState::Damaged)), "{slot}");
    }
    let _ = std::fs::remove_dir_all(dir);
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
    let started = Instant::now();
    assert!(game_shell::save_to(world, "slot_1"), "the slot is written");
    let took = started.elapsed();
    let saves = world.resource::<SaveSlots>().dir().unwrap().to_path_buf();
    let size = std::fs::metadata(saves.join("slot_1.json")).unwrap().len();
    println!(
        "pit_full: {} balls, {size} bytes ({:.2} MiB), capture and write {:.1} ms",
        world.query::<&Ball>().count(),
        size as f64 / (1024.0 * 1024.0),
        took.as_secs_f64() * 1000.0
    );
    assert!(size <= 2 * 1024 * 1024, "{size} bytes");
    let _ = std::fs::remove_dir_all(dir);
}
