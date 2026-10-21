//! Saves: a capture loads back to itself; every component a saved entity
//! carries is saved, rebuilt from its prefab or the map, or presentation;
//! a cast in the step that asks for an autosave saves its cost and its
//! missile; a save during `casting` loads with the mana still paused; a
//! loaded run keeps with the run it was saved from, in a fresh harness and
//! in a churned world; statused dummies load with their effects' sources;
//! a save mid-conduction resumes its chain; a bullet in flight loads and
//! lands; each reaction's hint toasts once a save; a schema-4 save
//! migrates past the map's new dummy; the template
//! sample's schemas are refused; a schema-5 save migrates past the map's
//! new enemies; an enemy's hit, pending transition, touch and frozen
//! flight load as they were, and a loaded run with enemies mid-attack and
//! mid-search keeps with the uninterrupted one; and a full pit saves
//! within 2 MiB with the map's enemies alive.

use std::any::type_name as name;
use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;

use example_01_platformer::features::audio::components::{Sound, Sounded};
use example_01_platformer::features::combat::components::Reaction;
use example_01_platformer::features::dummy::components::Dummy;
use example_01_platformer::features::elements;
use example_01_platformer::features::elements::cascade::{self, CascadeKind, Cascades};
use example_01_platformer::features::elements::components::{BallBurn, Frozen};
use example_01_platformer::features::enemies::components::{Enemy, EnemyKind, EnemyState};
use example_01_platformer::features::enemies::drops::{self, ORB_SECS, Orb, OrbKind};
use example_01_platformer::features::enemies::{self};
use example_01_platformer::features::gun::components::{Bullet, Reload};
use example_01_platformer::features::gun::{self};
use example_01_platformer::features::lightning;
use example_01_platformer::features::player::components::Player;
use example_01_platformer::features::toys::components::{
    Ball, BlackHole, FireMissile, IronBlock, IronScrap, SmallBall,
};
use example_01_platformer::features::toys::{self, BALL_CAP, IRON_HALF};
use example_01_platformer::game;
use example_01_platformer::save::{self, MAP_DUMMIES, MAP_ENEMIES, SaveGame};
use example_01_platformer::shared::{SaveId, SaveIds};
use example_01_platformer::shell::toast::Toasts;
use example_01_platformer::shell::{self as game_shell, Difficulty, RunStart, SlotState};
use glam::Vec2;
use tungsten::core::save::Value;
use tungsten::core::{Entity, Position, SaveData, SaveSlots, Time, Transform, UserFolder, World};
use tungsten::kit::{
    Abilities, Damaged, Effects, Health, Hitbox, Oscillator, PathFollower, StateMachine, Stats,
    Tags,
};
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

/// Every kind a save holds, by every spell and the gun: two iron blocks,
/// one frozen and shattered by a missile, the other chilled; a pit of 500
/// small balls, its crest burning; a bowling ball, a push, a hole, and a
/// missile and a bullet in flight; and a mana orb, beside the map's
/// enemies of every kind.
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
    let id = world.resource_mut::<SaveIds>().take();
    drops::spawn(
        world,
        OrbKind::Mana,
        Vec2::new(1400.0, TOP - 60.0),
        id,
        ORB_SECS,
    );
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
    assert!(!saved.bullets.is_empty(), "a bullet in flight");
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
        HitFlash, Hitbox, Hurtbox, Intent, Knockback, LinearMotion, PlatformerController,
        Projectile, Source, Stats, Zone,
    };
    use tungsten::kit::{AilmentBuckets, AilmentThreshold, Resist};
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
                    name::<AilmentBuckets>(),
                    name::<Tags>(),
                    name::<Spellbook>(),
                    name::<Lantern>(),
                    name::<Reload>(),
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
                    name::<AilmentThreshold>(),
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
        "dummy" => Lists {
            saved: [
                &at[..],
                &[
                    name::<Velocity>(),
                    name::<Health>(),
                    name::<Resist>(),
                    name::<Effects>(),
                    name::<AilmentBuckets>(),
                    name::<Tags>(),
                ],
            ]
            .concat(),
            rebuilt: [
                &body[..],
                &[
                    name::<Dummy>(),
                    name::<SceneEntity>(),
                    name::<PreviousVelocity>(),
                    name::<Hurtbox>(),
                    name::<Knockback>(),
                    name::<AilmentThreshold>(),
                    name::<Cues>(),
                ],
            ]
            .concat(),
            presentation: [&drawn[..], &[name::<HitFlash>()]].concat(),
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
        "bullet" => Lists {
            saved: [
                &at[..],
                &[
                    name::<Projectile>(),
                    name::<LinearMotion>(),
                    name::<Source>(),
                    EMITTER,
                    name::<Bullet>(),
                ],
            ]
            .concat(),
            rebuilt: names([name::<SceneEntity>(), name::<Hitbox>()]),
            presentation: drawn.to_vec(),
        },
        "wisp" | "crawler" | "beetle" | "bat" | "troll" => {
            use example_01_platformer::features::enemies::components::{
                Brain, Displaced, EnemyState, Shown, Sight,
            };
            use tungsten::kit::{Abilities, StateMachine, TopDownMover};
            Lists {
                saved: [
                    &at[..],
                    &[
                        name::<Velocity>(),
                        name::<PlatformerController>(),
                        name::<TopDownMover>(),
                        name::<Displaced>(),
                        name::<StateMachine<EnemyState>>(),
                        name::<Enemy>(),
                        name::<Brain>(),
                        name::<Sight>(),
                        name::<Health>(),
                        name::<Stats>(),
                        name::<Resist>(),
                        name::<Effects>(),
                        name::<AilmentBuckets>(),
                        name::<Tags>(),
                        name::<Knockback>(),
                        name::<Abilities>(),
                        name::<Hitbox>(),
                    ],
                ]
                .concat(),
                rebuilt: [
                    &body[..],
                    &[
                        name::<SceneEntity>(),
                        name::<PreviousVelocity>(),
                        name::<Hurtbox>(),
                        name::<ContactDamage>(),
                        name::<AilmentThreshold>(),
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
            use tungsten::kit::{Lifetime, Pickup};
            Lists {
                saved: [&at[..], &[name::<Orb>(), name::<Lifetime>()]].concat(),
                rebuilt: names([name::<SceneEntity>(), name::<Pickup>(), name::<Cues>()]),
                presentation: drawn.to_vec(),
            }
        }
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
    } else if world.has::<Bullet>(entity) {
        "bullet"
    } else if world.has::<Dummy>(entity) {
        "dummy"
    } else if world.has::<Orb>(entity) {
        "orb"
    } else if let Some(enemy) = world.get::<Enemy>(entity) {
        match enemy.kind {
            EnemyKind::Wisp => "wisp",
            EnemyKind::Crawler => "crawler",
            EnemyKind::Beetle => "beetle",
            EnemyKind::Bat => "bat",
            EnemyKind::Troll => "troll",
        }
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
            "bat",
            "beetle",
            "black hole",
            "block",
            "bullet",
            "crawler",
            "dummy",
            "hazard",
            "missile",
            "orb",
            "platform",
            "player",
            "scenery",
            "scrap",
            "troll",
            "wisp"
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
    // The newest autosave: the aqueduct's, after the run's first.
    let (slots, _) = game_shell::read_slots(harness.world_mut());
    let Some((_, saved)) = game_shell::newest(&slots) else {
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
    let saved = capture(&uninterrupted);
    assert_eq!(saved.area.as_deref(), Some("Lantern clearing"));
    assert!(!saved.cascades.entries.is_empty(), "mid-chain");
    let shocked = |save: &SaveGame| {
        save.dummies
            .iter()
            .any(|d| d.tags.iter().any(|tag| tag == "status.shocked"))
    };
    assert!(shocked(&saved));
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
    let id = world.resource_mut::<SaveIds>().take();
    let entity = enemies::spawn(world, kind, centre - kind.info().centre, id).expect("its prefab");
    enemies::scale(world, entity, Difficulty::Normal);
    entity
}

/// The entity of save ID `id`.
fn by_id(harness: &UiHarness, id: u64) -> Entity {
    harness
        .world()
        .query::<(Entity, &SaveId)>()
        .find(|&(_, own)| own.0 == id)
        .map(|(entity, _)| entity)
        .expect("a live entity of that save ID")
}

fn id_of(harness: &UiHarness, entity: Entity) -> u64 {
    harness.world().get::<SaveId>(entity).expect("a save ID").0
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
    let saved = capture(&harness);
    let entry = saved.enemies.iter().find(|enemy| enemy.id == beetle_id);
    let entry = entry.expect("the beetle saved");
    assert_eq!(
        entry.hit,
        [player_id],
        "its activation's target, by save ID"
    );
    assert!(
        entry.hitbox.as_ref().is_some_and(Hitbox::is_armed),
        "saved mid-window"
    );
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
    let asked = until(&mut harness, 400, |h| {
        h.world()
            .get::<StateMachine<EnemyState>>(crawler)
            .is_some_and(|machine| machine.pending() == Some(EnemyState::Attack))
    });
    assert!(asked, "its tell asked for the lunge");
    let saved = capture(&harness);
    let entry = saved.enemies.iter().find(|enemy| enemy.id == crawler_id);
    let machine = &entry.expect("the crawler saved").machine;
    assert_eq!(machine.state(), EnemyState::WindUp);
    assert_eq!(machine.pending(), Some(EnemyState::Attack));
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
    let saved = capture(&harness);
    let entry = saved.enemies.iter().find(|enemy| enemy.id == wisp_id);
    let touches = &entry.expect("the wisp saved").touches;
    assert_eq!(touches.len(), 1, "its touch's times: {touches:?}");
    assert_eq!(touches[0].0, player_id);
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
    let saved = capture(&harness);
    let entry = saved.enemies.iter().find(|enemy| enemy.id == wisp_id);
    let entry = entry.expect("the wisp saved");
    assert!(
        entry.mover.is_none() && entry.controller.is_some(),
        "it falls"
    );
    let displaced = entry.displaced.clone().expect("its flight set aside");
    assert_eq!(displaced.direction(), flying);
    let mut loaded = reloaded(saved, false);
    let wisp = by_id(&loaded, wisp_id);
    let world = loaded.world();
    assert_eq!(
        world.get::<Displaced>(wisp).map(|d| d.0.clone()),
        Some(displaced)
    );
    assert!(world.get::<PlatformerController>(wisp).is_some());
    let thaw = |harness: &mut UiHarness| -> (TopDownMover, Vec2) {
        let wisp = by_id(harness, wisp_id);
        let thawed = until(harness, 360, |h| {
            h.world().get::<TopDownMover>(wisp).is_some()
        });
        assert!(thawed, "it thaws");
        let mover = harness.world().get::<TopDownMover>(wisp).unwrap().clone();
        harness.step(30);
        (mover, super::body(harness, wisp))
    };
    let (before, after) = (thaw(&mut harness), thaw(&mut loaded));
    assert_eq!(before.0, after.0, "the same flight back at the thaw");
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
    let saved = capture(&harness);
    assert_eq!(saved.dummies.len(), 4, "the clearing's and the three");
    let id = |entity: Entity| harness.world().get::<SaveId>(entity).unwrap().0;
    let player_id = id(player);
    for &(dummy, effect) in &dummies {
        let save = saved.dummies.iter().find(|d| d.id == id(dummy)).unwrap();
        assert!(!save.map, "summoned, not the map's");
        assert!(save.effects.has(effect), "{effect}");
        assert!(
            save.sources.iter().any(|&(_, source)| source == player_id),
            "{effect}: its source by save ID"
        );
    }
    let mut fresh = super::harness();
    load(&mut fresh, saved.clone());
    let loaded_player = player_entity(&fresh);
    for &(dummy, effect) in &dummies {
        let wanted = id(dummy);
        let (entity, _) = fresh
            .world()
            .query::<(Entity, &SaveId)>()
            .find(|(_, id)| id.0 == wanted)
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

/// The schema-4 save a short busy run of 0.94 wrote: the run's player 316,
/// eight balls, frozen iron and the black hole.
const SCHEMA_FOUR: &str = include_str!("fixtures/schema4.json");

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

/// How many of the toasts on screen read `text`.
fn toasted(harness: &UiHarness, text: &str) -> usize {
    harness
        .world()
        .resource::<Toasts>()
        .texts(harness.tree())
        .iter()
        .filter(|shown| shown.as_str() == text)
        .count()
}

#[test]
fn each_hint_toasts_once_per_save() {
    let (mut harness, _) = grounded();
    let dummy = super::combat::dummy_at(&mut harness, 1400.0);
    let balls: Vec<Entity> = [900.0, 1000.0, 1100.0]
        .into_iter()
        .map(|x| toys::spawn_ball(harness.world_mut(), Vec2::new(x, TOP - 8.0), true))
        .collect();
    harness.step(2);
    // Two freezes, one toast.
    for &ball in &balls[..2] {
        elements::freeze_iron(harness.world_mut(), ball);
        harness.step(2);
        assert_eq!(toasted(&harness, Reaction::Freeze.hint()), 1);
    }
    // A shatter, a steam burst and a shock, each once.
    let at = super::body(&harness, balls[0]);
    cascade::queue(
        harness.world_mut(),
        CascadeKind::Shatter,
        Some(balls[0]),
        at,
        at,
        0.0,
    );
    harness.step(2);
    elements::ignite(harness.world_mut(), balls[2]);
    let at = super::body(&harness, balls[2]);
    cascade::queue(
        harness.world_mut(),
        CascadeKind::Steam,
        Some(balls[2]),
        at,
        at,
        elements::SMALL_STEAM_REACH,
    );
    harness.step(2);
    super::combat::apply(&mut harness, dummy, "shocked", None);
    harness.step(2);
    for reaction in [Reaction::Shatter, Reaction::Steam, Reaction::Shock] {
        assert_eq!(toasted(&harness, reaction.hint()), 1, "{reaction:?}");
    }
    let saved = capture(&harness);
    assert_eq!(
        saved.hints,
        [
            Reaction::Freeze,
            Reaction::Shatter,
            Reaction::Steam,
            Reaction::Shock
        ]
    );
    // The save keeps them seen: a loaded run's freeze toasts nothing.
    let mut fresh = super::harness();
    load(&mut fresh, saved);
    scaffold(&mut fresh);
    fresh.world_mut().resource_mut::<Time>().resume();
    let ball = toys::spawn_ball(fresh.world_mut(), Vec2::new(1200.0, TOP - 8.0), true);
    fresh.step(2);
    elements::freeze_iron(fresh.world_mut(), ball);
    fresh.step(2);
    assert_eq!(toasted(&fresh, Reaction::Freeze.hint()), 0);
    // A new run has seen none.
    let (mut new, _) = grounded();
    let ball = toys::spawn_ball(new.world_mut(), Vec2::new(1200.0, TOP - 8.0), true);
    new.step(2);
    elements::freeze_iron(new.world_mut(), ball);
    new.step(2);
    assert_eq!(toasted(&new, Reaction::Freeze.hint()), 1);
}

#[test]
fn a_bullet_in_flight_loads_and_lands() {
    let (mut harness, player) = grounded();
    let dummy = super::combat::dummy_at(&mut harness, 1400.0);
    harness.step(2);
    let y = super::body(&harness, dummy).y;
    let id = *harness.world().get::<SaveId>(dummy).unwrap();
    gun::fire(
        harness.world_mut(),
        Vec2::new(700.0, y),
        Vec2::X * 2400.0,
        Some(player),
    )
    .expect("a bullet");
    harness.step(6);
    let saved = capture(&harness);
    assert_eq!(saved.bullets.len(), 1, "in flight");
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
                .query::<(Entity, &Dummy, &SaveId)>()
                .find(|&(_, _, own)| *own == id)
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
    assert!(capture(&fresh).bullets.is_empty());
}

#[test]
fn a_schema_four_save_migrates_past_the_dummy() {
    let dir = user_folder("schema-four");
    let mut harness = harness_with(UserFolder::At(dir.clone()));
    let saves = harness
        .world()
        .resource::<SaveSlots>()
        .dir()
        .expect("a user folder")
        .to_path_buf();
    std::fs::create_dir_all(&saves).unwrap();
    std::fs::write(saves.join("slot_1.json"), SCHEMA_FOUR).unwrap();
    let save: SaveGame = harness
        .world()
        .resource::<SaveSlots>()
        .load("slot_1")
        .expect("schema 4 migrates");
    // Schema 4's player had 316, which the map's dummy took, its enemies the
    // IDs after: every ID from there moved up past both.
    let (dummy_id, shift) = (316, MAP_DUMMIES + MAP_ENEMIES);
    assert_eq!(
        (save.player.id, save.next_id),
        (dummy_id + shift, 327 + shift)
    );
    let balls: Vec<u64> = save.balls.iter().map(|ball| ball.id).collect();
    assert_eq!(balls, (317 + shift..325 + shift).collect::<Vec<_>>());
    assert_eq!(
        save.blocks.iter().map(|b| b.id).collect::<Vec<_>>(),
        [325 + shift]
    );
    assert_eq!(
        save.black_hole.as_ref().map(|hole| hole.id),
        Some(326 + shift)
    );
    assert!(
        save.hazards.iter().all(|hazard| hazard.id < dummy_id),
        "the map's kept theirs"
    );
    assert!(save.dummies.is_empty() && save.gone.is_empty() && save.enemies.is_empty());
    assert_eq!(save.player.stats.base("move_speed"), Some(560.0));
    assert_eq!(
        save.player.abilities.cooldown_left("lightning"),
        Some(0.0),
        "the new spell, ready"
    );
    assert!(
        save.player.abilities.ready("gun"),
        "the gun, its magazine whole"
    );
    assert!(save.player.abilities.ready("summon"));
    assert!(save.bullets.is_empty() && save.hints.is_empty());
    assert_eq!(save.summon, 0);
    load(&mut harness, save);
    let player = player_entity(&harness);
    let world = harness.world();
    let (dummy, id) = world
        .query::<(Entity, &Dummy, &SaveId)>()
        .map(|(entity, _, id)| (entity, id.0))
        .next()
        .expect("the map's dummy");
    assert_eq!(id, dummy_id);
    assert!(world.is_alive(dummy));
    assert_eq!(world.get::<SaveId>(player).unwrap().0, dummy_id + shift);
    harness.world_mut().resource_mut::<Time>().resume();
    super::combat::apply(&mut harness, player, "chilled", None);
    harness.step(2);
    let speed = harness
        .world()
        .get::<Stats>(player)
        .unwrap()
        .get("move_speed")
        .unwrap();
    assert!((speed - 364.0).abs() < 1e-3, "chilled slows it: {speed}");
    let _ = std::fs::remove_dir_all(dir);
}

/// 01 (b)'s save of a light scene: balls, an iron block, a summoned dummy
/// burning from the player, a cascade naming balls, a missile and a bullet.
const SCHEMA_FIVE: &str = include_str!("fixtures/schema5.json");

#[test]
fn a_schema_five_save_migrates_past_the_map_enemies() {
    let dir = user_folder("schema-five");
    let mut harness = harness_with(UserFolder::At(dir.clone()));
    let saves = harness
        .world()
        .resource::<SaveSlots>()
        .dir()
        .expect("a user folder")
        .to_path_buf();
    std::fs::create_dir_all(&saves).unwrap();
    std::fs::write(saves.join("slot_1.json"), SCHEMA_FIVE).unwrap();
    let old: serde_json::Value = serde_json::from_str(SCHEMA_FIVE).unwrap();
    let old = &old["data"];
    let save: SaveGame = harness
        .world()
        .resource::<SaveSlots>()
        .load("slot_1")
        .expect("schema 5 migrates");
    // Every save ID from schema 5's player on moved up past the enemies.
    let player = old["player"]["id"].as_u64().unwrap();
    let moved = |id: u64| if id >= player { id + MAP_ENEMIES } else { id };
    let ids = |list: &serde_json::Value, key: &str| -> Vec<u64> {
        list.as_array()
            .unwrap()
            .iter()
            .map(|item| item[key].as_u64().unwrap())
            .collect()
    };
    assert_eq!(save.player.id, player + MAP_ENEMIES);
    assert_eq!(save.next_id, moved(old["next_id"].as_u64().unwrap()));
    let balls: Vec<u64> = save.balls.iter().map(|ball| ball.id).collect();
    let old_balls: Vec<u64> = old["balls"]
        .as_array()
        .unwrap()
        .iter()
        .map(|ball| moved(ball[0].as_u64().unwrap()))
        .collect();
    assert_eq!(balls, old_balls);
    let blocks: Vec<u64> = save.blocks.iter().map(|b| b.id).collect();
    assert_eq!(
        blocks,
        ids(&old["blocks"], "id")
            .into_iter()
            .map(moved)
            .collect::<Vec<_>>()
    );
    let flying = |save_list: Vec<(u64, Option<u64>)>, key: &str| {
        let expected: Vec<(u64, Option<u64>)> = old[key]
            .as_array()
            .unwrap()
            .iter()
            .map(|item| {
                (
                    moved(item["id"].as_u64().unwrap()),
                    item["source"].as_u64().map(moved),
                )
            })
            .collect();
        assert_eq!(save_list, expected, "{key}");
        assert!(!expected.is_empty(), "{key}: one in flight");
    };
    flying(
        save.missiles.iter().map(|m| (m.id, m.source)).collect(),
        "missiles",
    );
    flying(
        save.bullets.iter().map(|b| (b.id, b.source)).collect(),
        "bullets",
    );
    let summoned = save
        .dummies
        .iter()
        .find(|d| !d.map)
        .expect("the summoned dummy");
    assert!(summoned.id > player + MAP_ENEMIES);
    assert_eq!(
        summoned
            .sources
            .iter()
            .map(|&(_, source)| source)
            .collect::<Vec<_>>(),
        [player + MAP_ENEMIES],
        "its burn names the player"
    );
    let map_dummy = save
        .dummies
        .iter()
        .find(|d| d.map)
        .expect("the map's dummy");
    assert!(map_dummy.id < player, "the map's dummy keeps its ID");
    let entries: Vec<(Option<u64>, Option<u64>)> = save
        .cascades
        .entries
        .iter()
        .map(|entry| (entry.target, entry.source))
        .collect();
    let old_entries: Vec<(Option<u64>, Option<u64>)> = old["cascades"]["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| {
            (
                entry["target"].as_u64().map(moved),
                entry["source"].as_u64().map(moved),
            )
        })
        .collect();
    assert!(!entries.is_empty());
    assert_eq!(entries, old_entries, "the cascades name the same things");
    assert!(save.enemies.is_empty() && save.gone.is_empty());
    // The run loads: the map's enemies as the map places them, the player
    // past them, the burn's source named again.
    load(&mut harness, save);
    let world = harness.world();
    let player_entity = player_entity(&harness);
    assert_eq!(
        world.get::<SaveId>(player_entity).unwrap().0,
        player + MAP_ENEMIES
    );
    let mut enemy_ids: Vec<u64> = world
        .query::<(&Enemy, &SaveId)>()
        .map(|(_, id)| id.0)
        .collect();
    enemy_ids.sort_unstable();
    assert_eq!(
        enemy_ids,
        (player..player + MAP_ENEMIES).collect::<Vec<_>>()
    );
    let burning = world
        .query::<(&Dummy, &SaveId, &Effects)>()
        .find(|(_, id, _)| id.0 == summoned_id(&harness))
        .map(|(_, _, effects)| {
            effects
                .instances()
                .any(|instance| instance.source() == Some(player_entity))
        });
    assert_eq!(burning, Some(true), "the burn credits the player");
    let _ = std::fs::remove_dir_all(dir);
}

/// The summoned dummy's save ID: the live dummy the map did not place.
fn summoned_id(harness: &UiHarness) -> u64 {
    let world = harness.world();
    example_01_platformer::features::dummy::live(world)
        .into_iter()
        .find(|&(id, _)| !example_01_platformer::features::dummy::placed(world, id))
        .map(|(id, _)| id)
        .expect("the summoned dummy")
}

#[test]
fn a_save_mid_chain_resumes_the_same_queue_in_save_id_order() {
    let (mut harness, _) = grounded();
    let pile = ice_pile(&mut harness, 900.0, 48);
    harness.step(2);
    let edge = super::body(&harness, pile[0]);
    elements::thermal_shatter(harness.world_mut(), edge, 72.0);
    harness.step(2);
    let saved = capture(&harness);
    assert!(!saved.cascades.entries.is_empty(), "mid-chain");
    let mut fresh = super::harness();
    load(&mut fresh, saved.clone());
    assert_eq!(json(&capture(&fresh)), json(&saved), "the queue whole");
    scaffold(&mut fresh);
    pin_camera(&mut fresh, CORNER);
    fresh.world_mut().resource_mut::<Time>().resume();
    let live = |harness: &UiHarness| -> BTreeSet<u64> {
        harness
            .world()
            .query::<(&SmallBall, &SaveId)>()
            .map(|(_, id)| id.0)
            .collect()
    };
    // The first run after the load works from the saved queue and the
    // saved points alone: the same entries run, in save-ID order past ties.
    assert_eq!(queue(&harness), queue(&fresh));
    harness.step(1);
    fresh.step(1);
    assert_eq!(live(&harness), live(&fresh), "the same balls went");
    assert_eq!(queue(&harness), queue(&fresh), "and the same queued next");
    // After it, the solver's warm starts, which no save holds, move the
    // pile a hair apart; each chain still runs out.
    harness.step(40);
    fresh.step(40);
    assert!(
        queue(&harness).is_empty() && queue(&fresh).is_empty(),
        "both chains ran out"
    );
    assert!(live(&harness).len() < pile.len() && live(&fresh).len() < pile.len());
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
