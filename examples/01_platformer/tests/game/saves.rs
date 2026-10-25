//! Saves: a capture loads back to itself; every component a saved entity
//! carries is saved, rebuilt from its prefab or the map, or presentation;
//! a cast in the step that asks for an autosave saves its cost and its
//! missile; a save during `casting` loads with the mana still paused; a
//! loaded run keeps with the run it was saved from, in a fresh harness and
//! in a churned world; statused dummies load with their effects' sources;
//! a save mid-conduction resumes its chain; a bullet in flight loads and
//! lands; a reaction toasts nothing; the template sample's
//! schemas and 01's older ones are refused; an enemy's hit, pending transition, touch and frozen
//! flight load as they were, and a loaded run with enemies mid-attack and
//! mid-search keeps with the uninterrupted one; a keg chain saved mid-chain
//! finishes the same; a schema 11 save's IDs move past the map's goblins;
//! a schema 12 player keeps his body where it stood on the wizard's frame;
//! a schema 13 save drops its hints and takes 01 (i)'s tuning, its
//! player's health keeping its share; and a full pit saves within 2 MiB
//! with the map's enemies alive.

use std::any::type_name as name;
use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;

use example_01_platformer::features::audio::components::{Sound, Sounded};
use example_01_platformer::features::dummy::components::Dummy;
use example_01_platformer::features::elements;
use example_01_platformer::features::elements::cascade::{self, CascadeKind, Cascades};
use example_01_platformer::features::elements::components::{BallBurn, Frozen};
use example_01_platformer::features::enemies::components::{
    Enemy, EnemyKind, EnemyState, Throw, Throws,
};
use example_01_platformer::features::enemies::drops::{self, ORB_SECS, Orb, OrbKind};
use example_01_platformer::features::enemies::{self, crawler, troll};
use example_01_platformer::features::gun::components::{Bullet, Reload};
use example_01_platformer::features::gun::{self};
use example_01_platformer::features::level::elevator::{Elevator, Stop};
use example_01_platformer::features::level::lava::{self, Lava};
use example_01_platformer::features::lightning;
use example_01_platformer::features::player::components::{Player, StaffGlow};
use example_01_platformer::features::player::{
    BODY_OFFSET, MAX_HEALTH, PICKUP_RADIUS, PLAYER_HALF,
};
use example_01_platformer::features::props::components::{Bomb, Crate, Keg, PropKind, Rock};
use example_01_platformer::features::props::{self, CRATE_HALF, KEG_HALF};
use example_01_platformer::features::toys::components::{
    Ball, BlackHole, FireMissile, IronBlock, IronScrap, SmallBall,
};
use example_01_platformer::features::toys::{self, BALL_CAP, IRON_HALF};
use example_01_platformer::game;
use example_01_platformer::save::{self, HoleSave, MAP_ENEMIES, SaveGame};
use example_01_platformer::shared::{SaveId, SaveIds};
use example_01_platformer::shell::toast::Toasts;
use example_01_platformer::shell::{self as game_shell, Difficulty, RunStart, SlotState};
use glam::Vec2;
use tungsten::core::save::Value;
use tungsten::core::{
    Entity, Position, SaveData, SaveSlots, Time, Transform, UserFolder, Velocity, World,
};
use tungsten::kit::{
    Abilities, ContactDamage, Damaged, Effects, Health, Hitbox, Oscillator, PathFollower,
    StateMachine, Stats, Tags,
};
use tungsten::testing::UiHarness;

use super::{
    aim_at, calm, centre, choose, floor, ground, harness, harness_with, mana, pin_camera,
    player_entity, run, settle, stand, start, steps_this_frame, teleport, top, user_folder,
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
/// missile and a bullet in flight; a mana orb; and a crate alight, a keg on
/// its fuse and a goblin's bomb and rock in flight, beside the map's
/// enemies of every kind and its crates and kegs.
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
    // A crate alight and a keg on its fuse, both the player's, and a
    // goblin's bomb and rock in flight, east of the hole's reach, which
    // puts a fire out, over the sky's ground.
    let player = player_entity(harness);
    let me = harness.world().get::<SaveId>(player).map(|id| id.0);
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
    world.resource_mut::<Throws>().0.push(Throw {
        body: rock,
        thrower: 0,
        factor: 1.0,
        left: 2.0,
    });
    harness.step(1);
}

#[test]
fn a_capture_round_trips_through_a_load_with_a_platform_displaced() {
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
        save.platforms
            .iter()
            .map(|p| p.position)
            .collect::<Vec<_>>()
    };
    let (platforms, platforms_now) = (points(&first), points(&saved));
    assert!(
        moved(&platforms, &platforms_now),
        "a platform is off its spawn"
    );
    assert!(!saved.missiles.is_empty() && !saved.black_holes.is_empty());
    assert!(!saved.bullets.is_empty(), "a bullet in flight");
    assert!(saved.blocks.iter().any(|b| b.scrap.is_some()));
    assert!(
        saved
            .blocks
            .iter()
            .any(|b| b.scrap.is_none() && b.chill.is_some())
    );
    assert!(saved.balls.iter().any(|b| b.burn.is_some()));
    let lit = |kind: PropKind| {
        saved.props.iter().any(|prop| {
            prop.kind == kind && (prop.fuse.is_some() || prop.fuel.is_some_and(|fuel| fuel.lit))
        })
    };
    assert!(lit(PropKind::Crate), "a crate alight");
    assert!(
        lit(PropKind::Keg) && lit(PropKind::Bomb),
        "a keg's and a bomb's fuse"
    );
    let rocks: Vec<u64> = saved
        .props
        .iter()
        .filter(|prop| prop.kind == PropKind::Rock)
        .map(|prop| prop.id)
        .collect();
    assert!(
        saved.throws.iter().any(|throw| rocks.contains(&throw.body)),
        "a rock in flight, its throw saved"
    );
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
    use example_01_platformer::features::level::components::{Glow, GlowParts, Hazard};
    use example_01_platformer::features::level::elevator::{CableLook, ElevatorDef};
    use example_01_platformer::features::level::lava::CrustLook;
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
        Projectile, Source, Stats,
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
                    name::<StaffGlow>(),
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
        "elevator" => Lists {
            saved: [&at[..], &[name::<Elevator>(), name::<PathFollower>()]].concat(),
            rebuilt: [
                &body[..],
                &[
                    name::<SceneEntity>(),
                    name::<Velocity>(),
                    name::<ElevatorDef>(),
                ],
            ]
            .concat(),
            presentation: [&drawn[..], &[name::<CableLook>()]].concat(),
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
        // The crates, kegs, bombs and rocks: a crate's or a rock's art, a
        // crate's fire, a keg's or a bomb's fuse, each saved whole; a
        // rock's throw is the throws'.
        "crate" | "keg" | "bomb" | "rock" => {
            use example_01_platformer::features::props::components::Fuel;
            Lists {
                saved: [
                    &at[..],
                    &[
                        name::<Velocity>(),
                        name::<Crate>(),
                        name::<Fuel>(),
                        name::<Keg>(),
                        name::<Bomb>(),
                        name::<Rock>(),
                        name::<Chill>(),
                        name::<Frozen>(),
                    ],
                ]
                .concat(),
                rebuilt: [
                    &body[..],
                    &[name::<SceneEntity>(), name::<PreviousVelocity>()],
                ]
                .concat(),
                presentation: drawn.to_vec(),
            }
        }
        "wisp" | "crawler" | "beetle" | "bat" | "troll" | "goblin" => {
            use example_01_platformer::features::enemies::components::{
                Brain, Displaced, EnemyState, HeldTouch, Scared, Shown, Sight,
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
                        name::<HeldTouch>(),
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
                        name::<Scared>(),
                    ],
                ]
                .concat(),
            }
        }
        "orb" => {
            use tungsten::kit::{Lifetime, Pickup};
            Lists {
                saved: [&at[..], &[name::<Orb>(), name::<Lifetime>()]].concat(),
                rebuilt: names([
                    name::<SceneEntity>(),
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
            saved: names([name::<SaveId>(), name::<Lava>()]),
            rebuilt: [
                &body[..],
                &[
                    name::<SceneEntity>(),
                    name::<Position>(),
                    name::<PrevPosition>(),
                    name::<Hazard>(),
                    name::<ContactDamage>(),
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
    } else if world.has::<Crate>(entity) {
        "crate"
    } else if world.has::<Keg>(entity) {
        "keg"
    } else if world.has::<Bomb>(entity) {
        "bomb"
    } else if world.has::<Rock>(entity) {
        "rock"
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
            "goblin",
            "keg",
            "missile",
            "orb",
            "platform",
            "player",
            "rock",
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
    choose(&mut harness, 0);
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
    assert_eq!(saved.player.stats.base("mana"), Some(96.0), "its cost");
    assert_eq!(saved.missiles.len(), 1, "and its missile");
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
    cast(&mut harness, 4, Vec2::new(800.0, 400.0), 1);
    let spent = mana(&harness);
    assert!(spent < 88.0, "a push's 12.5 spent: {spent}");
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
        let thawed = until(harness, 480, |h| {
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
    elements::ignite(harness.world_mut(), balls[1]);
    let at = super::body(&harness, balls[1]);
    cascade::queue(
        harness.world_mut(),
        CascadeKind::Steam,
        Some(balls[1]),
        at,
        at,
        elements::SMALL_STEAM_REACH,
    );
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

/// The live kegs, by save ID.
fn kegs(harness: &UiHarness) -> BTreeSet<u64> {
    harness
        .world()
        .query::<(&Keg, &SaveId)>()
        .map(|(_, id)| id.0)
        .collect()
}

#[test]
fn a_keg_chain_saved_mid_chain_finishes_the_same_after_a_load() {
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
    assert!(props::set_off(harness.world_mut(), row[0], None, Some(me)));
    // Saved after the second wave: two gone, the third's detonation waiting.
    let blown = |harness: &UiHarness| ids.difference(&kegs(harness)).count();
    for _ in 0..10 {
        if blown(&harness) >= 2 {
            break;
        }
        harness.step(1);
    }
    assert_eq!(blown(&harness), 2, "two waves");
    let saved = capture(&harness);
    let waiting = |harness: &UiHarness| harness.world().resource::<Cascades>().entries.clone();
    assert!(
        waiting(&harness)
            .iter()
            .any(|entry| entry.kind == CascadeKind::Detonation),
        "mid-chain"
    );
    let read: SaveGame = serde_json::from_value(json(&saved)).expect("a save reads");
    let mut fresh = super::harness();
    load(&mut fresh, read);
    assert_eq!(waiting(&fresh), waiting(&harness), "the queue's entries");
    scaffold(&mut fresh);
    pin_camera(&mut fresh, CORNER);
    fresh.world_mut().resource_mut::<Time>().resume();
    harness.step(30);
    fresh.step(30);
    assert_eq!(kegs(&harness), kegs(&fresh), "the same kegs gone");
    assert!(ids.is_disjoint(&kegs(&harness)), "the chain ran to its end");
    assert!(
        queue(&harness).is_empty() && queue(&fresh).is_empty(),
        "both queues empty"
    );
}

#[test]
fn schemas_one_to_six_are_refused() {
    // The template sample's 1 to 3, and 01 (a) to (c)'s 4 to 6, whose map
    // 01 (d) replaced: every save ID and place moved.
    for from in 1..=6 {
        assert!(
            SaveGame::migrate(from, Value::Null).is_err(),
            "schema {from}"
        );
    }
    // An older save on disk reads as an older version's, never loads.
    let dir = user_folder("schemas");
    let mut harness = harness_with(UserFolder::At(dir.clone()));
    let saves = harness
        .world()
        .resource::<SaveSlots>()
        .dir()
        .expect("a user folder")
        .to_path_buf();
    std::fs::create_dir_all(&saves).unwrap();
    let older = [
        ("slot_1", 1),
        ("slot_2", 2),
        ("slot_3", 3),
        ("slot_4", 4),
        ("slot_5", 5),
        ("slot_6", 6),
    ];
    for (slot, schema) in older {
        let envelope = serde_json::json!({
            "envelope": 1, "slot": slot, "schema": schema, "engine_version": "0.92.0",
            "saved_at_ms": 1_760_400_000_000_u64, "data": { "player": [0.0, 0.0] }
        });
        std::fs::write(saves.join(format!("{slot}.json")), envelope.to_string()).unwrap();
    }
    let (slots, _) = game_shell::read_slots(harness.world_mut());
    for (slot, _) in older {
        let state = slots.iter().find(|(s, _)| *s == slot).map(|(_, s)| s);
        assert!(matches!(state, Some(SlotState::Older(_))), "{slot}");
    }
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn schema_6_autosaves_get_an_autosave() {
    // An upgrader from 0.96: three autosaves of schema 6, which this build
    // refuses, read as an older version's and give way to the run's, the
    // oldest first.
    let dir = user_folder("schema-6-autosaves");
    let mut harness = harness_with(UserFolder::At(dir.clone()));
    let saves = harness
        .world()
        .resource::<SaveSlots>()
        .dir()
        .expect("a user folder")
        .to_path_buf();
    std::fs::create_dir_all(&saves).unwrap();
    for (slot, at) in game_shell::AUTOSAVES.into_iter().zip([3_u64, 1, 2]) {
        let envelope = serde_json::json!({
            "envelope": 1, "slot": slot, "schema": 6, "engine_version": "0.96.0",
            "saved_at_ms": 1_760_400_000_000_u64 + at, "data": { "player": [0.0, 0.0] }
        });
        std::fs::write(saves.join(format!("{slot}.json")), envelope.to_string()).unwrap();
    }
    start(&mut harness);
    harness.step(5);
    let slots = SaveSlots::new(Some(saves));
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
    assert!(harness.world().get::<Lantern>(player).unwrap().guttered);
    let saved = capture(&harness);
    assert!(saved.lantern && saved.lantern_guttered);
    let mut fresh = super::harness();
    load(&mut fresh, saved);
    fresh.world_mut().resource_mut::<Time>().resume();
    fresh.step(2);
    let lantern = *fresh.world().get::<Lantern>(player_entity(&fresh)).unwrap();
    assert!(lantern.guttered && !lantern.lit(), "{lantern:?}");
}

#[test]
fn schema_7_autosaves_migrate_and_rotate() {
    // A 0.97 upgrader's three autosaves: schema 7, the player's abilities
    // without the staff light, its lantern on.
    let (mut source, player) = run();
    let lantern = example_01_platformer::features::player::components::Lantern {
        on: false,
        guttered: false,
    };
    source.world_mut().insert(player, lantern);
    source.step(12);
    let mut data = serde_json::to_value(capture(&source)).unwrap();
    let entries = data
        .pointer_mut("/player/abilities/entries")
        .and_then(serde_json::Value::as_array_mut)
        .unwrap();
    entries.retain(|entry| entry["name"] != "staff_light");
    assert_eq!(entries.len(), 11, "0.97's eleven");
    data["lantern"] = true.into();
    let dir = user_folder("schema-7-autosaves");
    let mut harness = harness_with(UserFolder::At(dir.clone()));
    let saves = harness
        .world()
        .resource::<SaveSlots>()
        .dir()
        .expect("a user folder")
        .to_path_buf();
    std::fs::create_dir_all(&saves).unwrap();
    for (slot, at) in game_shell::AUTOSAVES.into_iter().zip([3_u64, 1, 2]) {
        let envelope = serde_json::json!({
            "envelope": 1, "slot": slot, "schema": 7, "engine_version": "0.97.0",
            "saved_at_ms": 1_760_400_000_000_u64 + at, "data": data.clone()
        });
        std::fs::write(saves.join(format!("{slot}.json")), envelope.to_string()).unwrap();
    }
    // Each migrates: ready, with the staff light.
    let slots = SaveSlots::new(Some(saves.clone()));
    for slot in game_shell::AUTOSAVES {
        let migrated = slots.load::<SaveGame>(slot).expect("schema 7 migrates");
        let names: Vec<&str> = migrated.player.abilities.names().collect();
        assert_eq!(names.last(), Some(&"staff_light"), "{slot}");
    }
    let (states, _) = game_shell::read_slots(harness.world_mut());
    for slot in game_shell::AUTOSAVES {
        let state = states.iter().find(|(s, _)| *s == slot).map(|(_, s)| s);
        assert!(matches!(state, Some(SlotState::Ready(..))), "{slot}");
    }
    // The run's first autosave takes the oldest.
    start(&mut harness);
    harness.step(5);
    let schemas: Vec<u64> = game_shell::AUTOSAVES
        .into_iter()
        .map(|slot| {
            let text = std::fs::read_to_string(saves.join(format!("{slot}.json"))).unwrap();
            let envelope: serde_json::Value = serde_json::from_str(&text).unwrap();
            envelope["schema"].as_u64().unwrap()
        })
        .collect();
    let now = u64::from(SaveGame::SCHEMA);
    assert_eq!(schemas, [7, now, 7], "autosave_2, the oldest, rotated");
    // A migrated save loads with its light, which relights at once.
    let migrated = slots.load::<SaveGame>("autosave_1").unwrap();
    let mut fresh = super::harness();
    load(&mut fresh, migrated);
    fresh.world_mut().resource_mut::<Time>().resume();
    fresh.step(2);
    let player = player_entity(&fresh);
    let lantern = fresh
        .world()
        .get::<example_01_platformer::features::player::components::Lantern>(player)
        .copied()
        .unwrap();
    assert!(lantern.lit(), "{lantern:?}");
    let _ = std::fs::remove_dir_all(dir);
}

/// Walks `old` and `new` together, pushing the path of each number that
/// differs, which must be a save ID that `to` moves where `new` has it;
/// anything else must be equal. Members `new` adds are skipped.
fn moved_ids(
    old: &Value,
    new: &Value,
    path: &str,
    to: &dyn Fn(u64) -> u64,
    moved: &mut Vec<String>,
) {
    match (old, new) {
        (Value::Object(old), Value::Object(new)) => {
            for (key, value) in old {
                let next = new.get(key).unwrap_or_else(|| panic!("{path}/{key} kept"));
                moved_ids(value, next, &format!("{path}/{key}"), to, moved);
            }
        }
        (Value::Array(old), Value::Array(new)) => {
            assert_eq!(old.len(), new.len(), "{path}: as many");
            for (i, (value, next)) in old.iter().zip(new).enumerate() {
                moved_ids(value, next, &format!("{path}/{i}"), to, moved);
            }
        }
        _ if old == new => {}
        // An `f32` written out and read back may print a digit apart.
        (Value::Number(a), Value::Number(b))
            if a.is_f64() && a.as_f64().map(|v| v as f32) == b.as_f64().map(|v| v as f32) => {}
        _ => {
            let id = old
                .as_u64()
                .unwrap_or_else(|| panic!("{path}: {old} became {new}"));
            assert_eq!(new.as_u64(), Some(to(id)), "{path}: {id}");
            moved.push(path.to_owned());
        }
    }
}

/// A schema 9 save ID as schema 10 numbers it, written out: the lake's lava
/// as its west pool's and the shelf's, the decks, the dummy, the enemies,
/// and the placed orbs, the player's and every later one past the new
/// beetle.
fn schema_10_id(id: u64) -> u64 {
    match id {
        839 => 0,
        840 => 2,
        841..=845 => id - 838,
        859 => 8,
        860..=877 => id - 851,
        878.. => id - 850,
        _ => panic!("{id}: no schema 9 save names it"),
    }
}

/// A schema 11 save ID as schema 12 numbers it: from the first placed
/// orb's on, past the map's seven goblins.
fn schema_12_id(id: u64) -> u64 {
    if id >= save::S11_PICKUPS {
        id + save::SHIFT_12
    } else {
        id
    }
}

/// `path` with every index starred: a tuple's place too, where only an ID
/// can have moved.
fn starred(path: &str) -> String {
    path.split('/')
        .map(|part| {
            if part.parse::<usize>().is_ok() {
                "*"
            } else {
                part
            }
        })
        .collect::<Vec<_>>()
        .join("/")
}

/// `fixture`'s envelope written to slot 1 of a user folder of its own, and
/// read back through the migration.
fn migrate(tag: &str, fixture: &str) -> (SaveGame, std::path::PathBuf) {
    let dir = user_folder(tag);
    let harness = harness_with(UserFolder::At(dir.clone()));
    let saves = harness
        .world()
        .resource::<SaveSlots>()
        .dir()
        .expect("a user folder")
        .to_path_buf();
    std::fs::create_dir_all(&saves).unwrap();
    std::fs::write(saves.join("slot_1.json"), fixture).unwrap();
    let migrated = SaveSlots::new(Some(saves))
        .load::<SaveGame>("slot_1")
        .expect("the fixture migrates");
    (migrated, dir)
}

/// Drops schema 9's ping-pong deck, the elevator's, from `old`'s platforms:
/// the migration drops it.
fn without_old_lift(old: &mut Value) {
    let platforms = old["platforms"].as_array_mut().unwrap();
    let before = platforms.len();
    platforms.retain(|platform| platform["id"].as_u64() != Some(845));
    assert_eq!(platforms.len() + 1, before, "the elevator's old deck saved");
}

/// What schema 11 makes of an older save: its one black hole the list's
/// only one, or an empty list, and the gun's rounds kept to its nine.
fn as_schema_11(old: &mut Value) {
    let hole = old
        .as_object_mut()
        .unwrap()
        .remove("black_hole")
        .unwrap_or(Value::Null);
    old["black_holes"] = Value::Array(if hole.is_null() {
        Vec::new()
    } else {
        vec![hole]
    });
    let entries = old["player"]["abilities"]["entries"]
        .as_array_mut()
        .unwrap();
    for entry in entries.iter_mut().filter(|entry| entry["name"] == "gun") {
        if let Some(charges) = entry["charges"].as_u64() {
            entry["charges"] = Value::from(charges.min(u64::from(gun::MAGAZINE)));
        }
    }
}

/// What schema 12 writes of an older save's balls: each array's trailing
/// `null`s left off.
fn as_schema_12(old: &mut Value) {
    for ball in old["balls"].as_array_mut().unwrap() {
        let record = ball.as_array_mut().unwrap();
        while record.len() > 6 && record.last().is_some_and(Value::is_null) {
            record.pop();
        }
    }
}

/// What schema 13 makes of an older save's player: its point, the sprite's
/// corner, moved to the wizard's frame, so the body stays where it stood.
fn as_schema_13(old: &mut Value) {
    let at = &mut old["player"]["position"];
    let corner = Vec2::new(
        at[0].as_f64().unwrap() as f32,
        at[1].as_f64().unwrap() as f32,
    ) + save::S12_BODY_OFFSET
        - BODY_OFFSET;
    *at = Value::from(vec![corner.x, corner.y]);
}

/// What schema 14 makes of an older save: its hints gone; the player's
/// maximum the same at every difficulty, its health keeping its share; its
/// reach, each crawler's damage and each troll's walk at their new bases,
/// their modifiers kept; and each saved cold build-up halved with the cold
/// ladder's gates.
fn as_schema_14(old: &mut Value) {
    old.as_object_mut().unwrap().remove("hints");
    let full = f64::from(MAX_HEALTH);
    let player = &mut old["player"];
    let current = player["health"]["current"].as_f64().unwrap();
    let max = player["health"]["max"].as_f64().unwrap();
    player["health"]["current"] = Value::from((current / max * full).clamp(0.0, full));
    player["health"]["max"] = Value::from(full);
    player["stats"]["max_health"]["base"] = Value::from(full);
    player["stats"]["pickup_radius"]["base"] = Value::from(f64::from(PICKUP_RADIUS));
    let halve = |buckets: &mut Value| {
        if let Some(fill) = buckets.get_mut("damage.cold") {
            *fill = Value::from(fill.as_f64().unwrap() * 0.5);
        }
    };
    halve(&mut player["buckets"]);
    for dummy in old["dummies"].as_array_mut().unwrap() {
        halve(&mut dummy["buckets"]);
    }
    for enemy in old["enemies"].as_array_mut().unwrap() {
        if enemy["kind"] == "crawler" {
            enemy["stats"]["damage"]["base"] = Value::from(f64::from(crawler::DAMAGE));
        }
        if enemy["kind"] == "troll" {
            enemy["stats"]["move_speed"]["base"] = Value::from(f64::from(troll::MOVE_SPEED));
        }
        halve(&mut enemy["buckets"]);
    }
}

/// Sets aside what a capture of a migrated save adds: the map's new enemies
/// (`added`), the elevator at rest and the basins at rest, which a save
/// from before schema 10 has none of, and the shoves and steps 01 gives
/// the controllers at a load, which a save from before 0.105 has none of.
fn without_new(save: &mut Value, added: &std::ops::Range<u64>) {
    let unset = |controller: Option<&mut Value>| {
        if let Some(fields) = controller.and_then(Value::as_object_mut) {
            fields.remove("shove");
            fields.remove("step_height");
        }
    };
    unset(save["player"].get_mut("controller"));
    let enemies = save["enemies"].as_array_mut().unwrap();
    enemies.retain(|enemy| !added.contains(&enemy["id"].as_u64().unwrap()));
    for enemy in enemies.iter_mut() {
        unset(enemy.get_mut("controller"));
    }
    let lifts = save["elevators"].as_array().unwrap();
    assert_eq!(lifts.len(), 1, "the elevator");
    assert_eq!(lifts[0]["stop"], "bottom", "at rest");
    assert!(lifts[0]["follower"].is_null(), "at rest");
    save["elevators"] = Value::Array(Vec::new());
    for basin in save["basins"].as_array().unwrap() {
        assert_eq!(basin["cold"].as_f64(), Some(0.0), "unchilled");
        assert!(basin["crust"].is_null(), "uncrusted");
    }
    save["basins"] = Value::Array(Vec::new());
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
        .query::<(Entity, &Orb, &SaveId)>()
        .filter(|(_, _, id)| placed.contains(&id.0))
        .map(|(entity, _, id)| (id.0, entity))
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
    let saved = capture(&harness);
    assert!(saved.gone.contains(&taken), "{:?}", saved.gone);
    assert!(
        saved.orbs.is_empty(),
        "a placed orb is the map's, not a drop's"
    );
    let mut fresh = super::harness();
    load(&mut fresh, saved);
    let left: Vec<u64> = placed_orbs(&fresh).into_iter().map(|(id, _)| id).collect();
    let expected: Vec<u64> = orbs[1..].iter().map(|&(id, _)| id).collect();
    assert_eq!(left, expected, "the taken one gone, the rest there");
}

#[test]
fn schema_8_saves_migrate_with_the_new_map_objects_ids_moved() {
    // 0.98's save, captured from its build with every saved ID field in
    // use above its player's: summoned trolls holding and throwing iron, a
    // bat's hit, touches, sources, a waiting explosion and jump.
    const FIXTURE: &str = include_str!("fixtures/schema8.json");
    let envelope: Value = serde_json::from_str(FIXTURE).unwrap();
    let mut old = envelope["data"].clone();
    let from = old["player"]["id"].as_u64().unwrap();
    assert_eq!((envelope["schema"].as_u64(), from), (Some(8), 873));
    let (migrated, dir) = migrate("schema-8", FIXTURE);
    // Every ID field moved, by schema 9's shift, schema 10's remap and
    // schema 12's shift, the elevator's old deck dropped, and nothing else.
    without_old_lift(&mut old);
    as_schema_11(&mut old);
    as_schema_12(&mut old);
    as_schema_13(&mut old);
    as_schema_14(&mut old);
    let to = |id: u64| {
        schema_12_id(schema_10_id(if id >= from {
            id + save::SHIFT_9
        } else {
            id
        }))
    };
    let mut moved = Vec::new();
    moved_ids(&old, &json(&migrated), "", &to, &mut moved);
    let fields: BTreeSet<String> = moved.iter().map(|path| starred(path)).collect();
    let expected = [
        "/next_id",
        "/player/id",
        "/platforms/*/id",
        "/gone/*",
        "/player/sources/*/1",
        "/dummies/*/id",
        "/dummies/*/sources/*/1",
        "/enemies/*/id",
        "/enemies/*/sources/*/1",
        "/enemies/*/hit/*",
        "/enemies/*/touches/*/0",
        "/enemies/*/held",
        "/throws/*/body",
        "/throws/*/thrower",
        "/missiles/*/id",
        "/missiles/*/source",
        "/bullets/*/id",
        "/bullets/*/source",
        "/orbs/*/id",
        "/black_holes/*/id",
        "/blocks/*/id",
        "/balls/*/0",
        "/cascades/entries/*/target",
        "/cascades/entries/*/source",
        "/cascades/entries/*/chain/*",
    ];
    assert_eq!(fields, expected.into_iter().map(starred).collect());
    // Loaded: the player under its moved ID, the gone bat gone, the map
    // enemies since added alive, five in schema 9, the beetle in 10 and the
    // goblins in 12, and every other saved thing as it was saved.
    let mut run = super::harness();
    load(&mut run, migrated.clone());
    let player = player_entity(&run);
    let world = run.world();
    assert_eq!(
        world.get::<SaveId>(player).unwrap().0,
        schema_12_id(save::S10_PLAYER)
    );
    let live: BTreeSet<u64> = enemies::live(world).into_iter().map(|(id, _)| id).collect();
    let added = save::S10_ENEMIES + 13..save::S10_ENEMIES + MAP_ENEMIES;
    assert_eq!(added.clone().count(), 13);
    assert!(added.clone().all(|id| live.contains(&id)), "{live:?}");
    assert!(!live.contains(&to(863)), "the gone bat");
    assert_eq!(
        placed_orbs(&run).len() as u64,
        save::MAP_PICKUPS,
        "every orb there"
    );
    let mut again = json(&capture(&run));
    without_new(&mut again, &added);
    // A recapture names the build that took it, the fixture 0.98's.
    let migrated = json(&migrated);
    assert_eq!(
        again["meta"]["game_version"].as_str(),
        Some(env!("CARGO_PKG_VERSION"))
    );
    again["meta"]["game_version"] = migrated["meta"]["game_version"].clone();
    let mut differ = Vec::new();
    differences(&again, &migrated, "", &mut differ);
    assert!(
        differ.is_empty(),
        "the migrated save, reloaded whole: {differ:?}"
    );
    let _ = std::fs::remove_dir_all(dir);
}

/// The map's objects a save names, by save ID: the basins, the decks with
/// the elevator's, the dummy, the enemies and the placed orbs.
fn map_ids(harness: &UiHarness) -> [Vec<u64>; 5] {
    let world = harness.world();
    let ids = |mut ids: Vec<u64>| {
        ids.sort_unstable();
        ids
    };
    let placed = &world.resource::<drops::PlacedOrbs>().0;
    [
        ids(world
            .query::<(&Lava, &SaveId)>()
            .map(|(_, id)| id.0)
            .collect()),
        ids(world
            .query::<(Entity, &SaveId)>()
            .filter(|(entity, _)| {
                world.has::<PathFollower>(*entity) || world.has::<Elevator>(*entity)
            })
            .map(|(_, id)| id.0)
            .collect()),
        ids(world
            .query::<(&Dummy, &SaveId)>()
            .map(|(_, id)| id.0)
            .collect()),
        ids(enemies::live(world).into_iter().map(|(id, _)| id).collect()),
        ids(placed.clone()),
    ]
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
        .query::<(Entity, &Lava, &SaveId)>()
        .find(|(_, _, own)| own.0 == id)
        .map(|(entity, ..)| entity)
        .expect("the basin")
}

#[test]
fn schema_9_saves_migrate_to_the_new_save_ids() {
    // 0.99's save (step 1's fixture): the first map wisp killed, the first
    // placed orb taken, the decks mid-path, a summoned beetle and dummy,
    // iron blocks, balls and a missile in flight.
    const FIXTURE: &str = include_str!("fixtures/schema9.json");
    let envelope: Value = serde_json::from_str(FIXTURE).unwrap();
    let mut old = envelope["data"].clone();
    let from = old["player"]["id"].as_u64().unwrap();
    assert_eq!((envelope["schema"].as_u64(), from), (Some(9), 886));
    // A new run numbers the map as the migrations' frozen layouts have it:
    // schema 10's, its orbs and its player past schema 12's goblins.
    let (fresh, player) = run();
    let span = |start: u64, count: u64| (start..start + count).collect::<Vec<u64>>();
    assert_eq!(
        map_ids(&fresh),
        [
            vec![save::S10_LAKE, 1, save::S10_SHELF],
            span(save::S10_DECKS, 5),
            vec![save::S10_DUMMY],
            span(save::S10_ENEMIES, MAP_ENEMIES),
            span(schema_12_id(save::S10_PICKUPS), save::MAP_PICKUPS),
        ]
    );
    assert_eq!(id_of(&fresh, player), schema_12_id(save::S10_PLAYER));
    let (lift, _) = elevator(&fresh);
    assert_eq!(
        id_of(&fresh, lift),
        schema_10_id(845),
        "the elevator's deck"
    );
    let x = |id: u64| {
        fresh
            .world()
            .get::<Position>(basin(&fresh, id))
            .unwrap()
            .0
            .x
    };
    assert_eq!(
        [x(save::S10_LAKE), x(save::S10_SHELF)],
        [230.0 * 64.0, 260.0 * 64.0],
        "the lake's west pool and the shelf's"
    );
    drop(fresh);
    let (migrated, dir) = migrate("schema-9", FIXTURE);
    // Every ID field moved by the remap and schema 12's shift, the
    // elevator's old deck dropped, and nothing else.
    without_old_lift(&mut old);
    as_schema_11(&mut old);
    as_schema_12(&mut old);
    as_schema_13(&mut old);
    as_schema_14(&mut old);
    let to = |id: u64| schema_12_id(schema_10_id(id));
    let mut moved = Vec::new();
    moved_ids(&old, &json(&migrated), "", &to, &mut moved);
    let fields: BTreeSet<String> = moved.iter().map(|path| starred(path)).collect();
    let expected = [
        "/next_id",
        "/player/id",
        "/platforms/*/id",
        "/dummies/*/id",
        "/enemies/*/id",
        "/missiles/*/id",
        "/blocks/*/id",
        "/balls/*/0",
        "/gone/*",
    ];
    assert_eq!(fields, expected.into_iter().map(starred).collect());
    assert!(migrated.elevators.is_empty() && migrated.basins.is_empty());
    // Loaded: the elevator at rest at the bottom, the gone wisp and the
    // taken orb gone, the new beetle and goblins alive, and the rest as it
    // was saved.
    let mut run = super::harness();
    load(&mut run, migrated.clone());
    let (deck, lift) = elevator(&run);
    assert_eq!(lift.stop, Stop::Bottom);
    assert!(run.world().get::<PathFollower>(deck).is_none(), "at rest");
    assert_eq!(run.world().get::<Position>(deck).unwrap().0, lift.bottom);
    let live: BTreeSet<u64> = enemies::live(run.world())
        .into_iter()
        .map(|(id, _)| id)
        .collect();
    let wisp = schema_10_id(866);
    assert!(!live.contains(&wisp), "the gone wisp");
    // Schema 10's last enemy; the goblins follow it.
    let beetle = save::S11_PICKUPS - 1;
    let world = run.world();
    assert_eq!(
        world.get::<Enemy>(by_id(&run, beetle)).unwrap().kind,
        EnemyKind::Beetle,
        "the new beetle"
    );
    let orbs: Vec<u64> = placed_orbs(&run).into_iter().map(|(id, _)| id).collect();
    assert_eq!(
        orbs,
        span(schema_12_id(save::S10_PICKUPS) + 1, save::MAP_PICKUPS - 1)
    );
    let mut again = json(&capture(&run));
    without_new(&mut again, &(beetle..save::S10_ENEMIES + MAP_ENEMIES));
    let migrated = json(&migrated);
    again["meta"]["game_version"] = migrated["meta"]["game_version"].clone();
    let mut differ = Vec::new();
    differences(&again, &migrated, "", &mut differ);
    assert!(
        differ.is_empty(),
        "the migrated save, reloaded whole: {differ:?}"
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_crusted_basin_and_a_riding_elevator_load_back_to_themselves() {
    let (mut harness, player) = run();
    calm(&mut harness);
    harness.harness_mut().set_dt(1.0 / 60.0);
    let lake = basin(&harness, save::S10_LAKE);
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
    let shelf = basin(&harness, save::S10_SHELF);
    harness.world_mut().get_mut::<Lava>(shelf).unwrap().cold = 0.2;
    let saved = capture(&harness);
    let crust = |id: u64| saved.basins.iter().find(|b| b.id == id).unwrap();
    assert!(
        crust(save::S10_LAKE)
            .crust
            .is_some_and(|left| left > 1.0 && left < 4.0)
    );
    assert_eq!(crust(save::S10_SHELF).cold, 0.2);
    assert_eq!(saved.elevators.len(), 1);
    assert!(saved.elevators[0].follower.is_some() && saved.elevators[0].ridden);
    // Loaded from its file, as Continue loads it, it captures back to
    // itself: the crust's touch held, its surface solid, the deck mid-path.
    let read: SaveGame = serde_json::from_value(json(&saved)).expect("a save reads");
    let mut fresh = super::harness();
    fresh.harness_mut().set_dt(1.0 / 60.0);
    load(&mut fresh, read);
    assert_eq!(json(&capture(&fresh)), json(&saved));
    let lake = basin(&fresh, save::S10_LAKE);
    let world = fresh.world();
    assert!(
        world.get::<tungsten::kit::ContactDamage>(lake).is_none(),
        "held"
    );
    assert_eq!(
        world.get::<tungsten::core::RigidBody>(lake).unwrap().layers,
        1
    );
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
        let feet = centre(&fresh, rider).y + PLAYER_HALF.y;
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
    assert!(world.get::<Lava>(lake).unwrap().crust.is_none(), "melted");
    assert!(
        world.get::<tungsten::kit::ContactDamage>(lake).is_some(),
        "its touch back"
    );
}

#[test]
fn a_player_saved_under_a_new_lava_surface_loads_on_its_bank() {
    const FIXTURE: &str = include_str!("fixtures/schema9.json");
    let tile = 64.0;
    // Schema 9's player, by its body's middle column and its feet: on the
    // lake's old beds, its west pool now the stone's and its east pool the
    // east bank's; on the old bed where the stone now stands; on the old
    // ground of the lake's widened west columns; on the shelf's old bed; and
    // on the old ground of its widened east column. Each moves to the
    // middle column of its nearest bank or the stone, on its top.
    let cases = [
        (233.0, 67.0, 235.0, 63.0),
        (239.0, 67.0, 240.5, 63.0),
        (234.5, 67.0, 235.0, 63.0),
        (230.6, 63.0, 229.5, 63.0),
        (261.0, 58.0, 259.5, 57.0),
        (262.6, 57.0, 263.5, 57.0),
    ];
    for (i, (column, feet, bank, row)) in cases.into_iter().enumerate() {
        let mut envelope: Value = serde_json::from_str(FIXTURE).unwrap();
        let middle = Vec2::new(column * tile, feet * tile - PLAYER_HALF.y - 0.5);
        let corner = middle - save::S12_BODY_OFFSET;
        envelope["data"]["player"]["position"] = serde_json::json!([corner.x, corner.y]);
        let fixture = serde_json::to_string(&envelope).unwrap();
        let (migrated, dir) = migrate(&format!("under-lava-{i}"), &fixture);
        let spot = Vec2::new(bank * tile, row * tile - PLAYER_HALF.y - 0.5);
        let at = Vec2::from(migrated.player.position) + BODY_OFFSET;
        assert!(at.distance(spot) < 0.01, "{column},{feet}: moved to {at}");
        // Loaded with the clock running, the player stands there a second,
        // unhurt (an orb its reach draws in may heal it).
        let mut harness = super::harness();
        harness.harness_mut().set_dt(1.0 / 60.0);
        load(&mut harness, migrated);
        calm(&mut harness);
        harness.world_mut().resource_mut::<Time>().resume();
        let player = player_entity(&harness);
        let mut hurt = false;
        for _ in 0..60 {
            harness.step(1);
            hurt |= harness
                .harness()
                .events::<Damaged>()
                .any(|hit| hit.entity == player);
        }
        let now = centre(&harness, player);
        assert!(
            now.distance(spot) < 1.0,
            "{column},{feet}: standing at {now}, not {spot}"
        );
        let world = harness.world();
        assert!(
            world
                .get::<tungsten::kit::PlatformerController>(player)
                .unwrap()
                .grounded()
        );
        assert!(!hurt, "{column},{feet}: unhurt");
        let _ = std::fs::remove_dir_all(dir);
    }
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
    let saved = capture(&harness);
    assert_eq!(saved.black_holes.len(), 3, "every hole saved");
    // Through JSON, as Continue reads a slot.
    let loaded: SaveGame = serde_json::from_value(json(&saved)).expect("it loads");
    let mut fresh = super::harness();
    load(&mut fresh, loaded);
    assert_eq!(fresh.world().query::<&BlackHole>().count(), 3);
    assert_eq!(capture(&fresh).black_holes, saved.black_holes);
}

#[test]
fn schema_10_saves_migrate_their_black_hole_into_the_list() {
    // 0.100's save (step 1's fixture): one hole open, a missile in flight.
    const FIXTURE: &str = include_str!("fixtures/schema10.json");
    let envelope: Value = serde_json::from_str(FIXTURE).unwrap();
    assert_eq!(envelope["schema"].as_u64(), Some(10));
    let mut hole: HoleSave =
        serde_json::from_value(envelope["data"]["black_hole"].clone()).unwrap();
    let (migrated, dir) = migrate("schema-10", FIXTURE);
    hole.id = schema_12_id(hole.id);
    assert_eq!(
        migrated.black_holes,
        [hole],
        "its place and time left, its ID past the goblins"
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn schema_11_saves_move_their_ids_past_the_map_goblins() {
    // 0.101's save (step 1's fixture): a busy run on the sky's ground, two
    // holes open and an explosion waiting, credited to the player.
    const FIXTURE: &str = include_str!("fixtures/schema11.json");
    let envelope: Value = serde_json::from_str(FIXTURE).unwrap();
    let mut old = envelope["data"].clone();
    assert_eq!(envelope["schema"].as_u64(), Some(11));
    as_schema_12(&mut old);
    as_schema_13(&mut old);
    as_schema_14(&mut old);
    let (migrated, dir) = migrate("schema-11", FIXTURE);
    // Every ID from the first placed orb's on moved past the goblins, every
    // one under it kept, and nothing else.
    let mut moved = Vec::new();
    moved_ids(&old, &json(&migrated), "", &schema_12_id, &mut moved);
    let fields: BTreeSet<String> = moved.iter().map(|path| starred(path)).collect();
    let expected = [
        "/next_id",
        "/player/id",
        "/player/sources/*/1",
        "/orbs/*/id",
        "/blocks/*/id",
        "/black_holes/*/id",
        "/missiles/*/id",
        "/missiles/*/source",
        "/bullets/*/id",
        "/bullets/*/source",
        "/balls/*/0",
        "/cascades/entries/*/target",
        "/cascades/entries/*/source",
    ];
    assert_eq!(fields, expected.into_iter().map(starred).collect());
    assert_eq!(migrated.black_holes.len(), 2, "both holes");
    let goblins = save::S11_PICKUPS..save::S11_PICKUPS + save::SHIFT_12;
    assert!(
        !migrated.gone.iter().any(|id| goblins.contains(id)),
        "{:?}",
        migrated.gone
    );
    assert!(
        migrated
            .enemies
            .iter()
            .all(|enemy| !goblins.contains(&enemy.id))
    );
    assert!(migrated.props.is_empty());
    // Loaded, the map's goblins stand at their homes.
    let mut run = super::harness();
    load(&mut run, migrated);
    for id in goblins {
        let goblin = by_id(&run, id);
        let enemy = *run.world().get::<Enemy>(goblin).unwrap();
        assert_eq!(enemy.kind, EnemyKind::Goblin, "{id}");
        let at = super::body(&run, goblin);
        assert!(at.distance(enemy.home()) < 1.0, "{id} at {at}");
    }
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_schema_12_player_keeps_its_body_where_it_stood() {
    // 0.102's save (step 1's fixture): the busy run on the sky's ground,
    // its player's point the corner of the courier's 64 px frame.
    const FIXTURE: &str = include_str!("fixtures/schema12.json");
    let envelope: Value = serde_json::from_str(FIXTURE).unwrap();
    assert_eq!(envelope["schema"].as_u64(), Some(12));
    let mut old = envelope["data"].clone();
    let at = &old["player"]["position"];
    let corner = Vec2::new(
        at[0].as_f64().unwrap() as f32,
        at[1].as_f64().unwrap() as f32,
    );
    let (migrated, dir) = migrate("schema-12", FIXTURE);
    // Its point moved to the wizard's frame, and nothing else.
    let moved_to = Vec2::from(migrated.player.position);
    assert!(
        moved_to.distance(corner + Vec2::new(-16.0, -32.0)) < 1e-3,
        "{corner} became {moved_to}"
    );
    as_schema_13(&mut old);
    as_schema_14(&mut old);
    let mut moved = Vec::new();
    moved_ids(&old, &json(&migrated), "", &|id| id, &mut moved);
    assert!(moved.is_empty(), "{moved:?}");
    // Loaded, the body stands where the courier's stood.
    let mut run = super::harness();
    load(&mut run, migrated);
    let player = player_entity(&run);
    let body = centre(&run, player);
    let stood = corner + Vec2::new(32.0, 33.0);
    assert!(body.distance(stood) < 0.01, "{body} against {stood}");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_schema_13_save_drops_its_hints_and_keeps_its_health_share() {
    // 0.103's save (step 1's fixture): an Easy run, its player hurt to 120
    // of 150, a freeze's hint seen and the map's dummy part chilled.
    const FIXTURE: &str = include_str!("fixtures/schema13.json");
    let envelope: Value = serde_json::from_str(FIXTURE).unwrap();
    assert_eq!(envelope["schema"].as_u64(), Some(13));
    let mut old = envelope["data"].clone();
    let (migrated, dir) = migrate("schema-13", FIXTURE);
    let new = json(&migrated);
    assert!(new.get("hints").is_none(), "its hints gone");
    let player = &new["player"];
    let health = (
        player["health"]["current"].as_f64(),
        player["health"]["max"].as_f64(),
    );
    assert_eq!(health, (Some(80.0), Some(100.0)), "its share of 100");
    assert_eq!(player["stats"]["max_health"]["base"].as_f64(), Some(100.0));
    assert_eq!(
        player["stats"]["pickup_radius"]["base"].as_f64(),
        Some(192.0)
    );
    let crawlers: Vec<&Value> = new["enemies"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|enemy| enemy["kind"] == "crawler")
        .collect();
    assert!(!crawlers.is_empty());
    for enemy in crawlers {
        let damage = &enemy["stats"]["damage"];
        assert_eq!(damage["base"].as_f64(), Some(15.0));
        assert_eq!(
            damage["modifiers"],
            serde_json::json!([["difficulty", { "mul": 0.75 }]]),
            "Easy's kept"
        );
    }
    let cold = |save: &Value| {
        save["dummies"][0]["buckets"]["damage.cold"]
            .as_f64()
            .unwrap()
    };
    assert!(
        (cold(&new) - cold(&old) * 0.5).abs() < 1e-5,
        "the dummy's build-up halved: {} of {}",
        cold(&new),
        cold(&old)
    );
    // And nothing else.
    as_schema_14(&mut old);
    let mut moved = Vec::new();
    moved_ids(&old, &new, "", &|id| id, &mut moved);
    assert!(moved.is_empty(), "{moved:?}");
    // Loaded, it plays on with the new values.
    let mut run = super::harness();
    load(&mut run, migrated);
    run.harness_mut().set_dt(1.0 / 60.0);
    run.world_mut().resource_mut::<Time>().resume();
    run.step(2);
    let world = run.world();
    let player = player_entity(&run);
    let health = world.get::<Health>(player).unwrap();
    assert_eq!((health.current(), health.max()), (80.0, 100.0));
    let reach = world.get::<Stats>(player).unwrap().get("pickup_radius");
    assert_eq!(reach, Some(192.0));
    let of_kind = |kind: EnemyKind| {
        world
            .query::<(Entity, &Enemy)>()
            .find(|(_, enemy)| enemy.kind == kind)
            .map(|(entity, _)| entity)
            .unwrap_or_else(|| panic!("a placed {kind:?}"))
    };
    let touch = world
        .get::<ContactDamage>(of_kind(EnemyKind::Crawler))
        .unwrap()
        .damage();
    assert_eq!(touch, 15.0 * 0.75, "a crawler's touch at Easy");
    let wisp = world.get::<Health>(of_kind(EnemyKind::Wisp)).unwrap().max();
    assert_eq!(wisp, 40.0 * 0.75, "a wisp's health at Easy");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_schema_10_trolls_deep_chill_goes_at_its_first_step() {
    // 0.100's save (step 1's fixture), its placed troll past the old second
    // stage: a `deep_chill` slow on its speed, and no chill.
    const FIXTURE: &str = include_str!("fixtures/schema10.json");
    let mut envelope: Value = serde_json::from_str(FIXTURE).unwrap();
    let troll = envelope["data"]["enemies"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|enemy| enemy["kind"] == "troll")
        .expect("a placed troll");
    let id = troll["id"].as_u64().unwrap();
    let effects = troll["effects"]["instances"].as_array().unwrap();
    assert!(
        effects.iter().all(|effect| effect["effect"] != "chilled"),
        "no chill"
    );
    troll["stats"]["move_speed"]["modifiers"] =
        serde_json::json!([["deep_chill", { "pct": -0.35 }]]);
    let (migrated, dir) = migrate("schema-10-deep-chill", &envelope.to_string());
    let mut fresh = super::harness();
    load(&mut fresh, migrated);
    let troll = fresh
        .world()
        .query::<(Entity, &Enemy, &SaveId)>()
        .find(|(_, _, own)| own.0 == id)
        .map(|(entity, ..)| entity)
        .expect("the troll loaded");
    let speed = |h: &UiHarness| h.world().get::<Stats>(troll).unwrap().get("move_speed");
    let slowed = speed(&fresh).unwrap();
    assert!(
        (slowed - troll::MOVE_SPEED * 0.65).abs() < 1e-3,
        "its saved slow: {slowed}"
    );
    fresh.harness_mut().set_dt(1.0 / 60.0);
    fresh.world_mut().resource_mut::<Time>().resume();
    fresh.step(1);
    assert_eq!(
        speed(&fresh),
        Some(troll::MOVE_SPEED),
        "whole after its first step"
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn schema_10_saves_keep_at_most_nine_rounds() {
    // 0.100's save (step 1's fixture), its gun's magazine set to 12, to 10
    // and to 0 with a reload running.
    const FIXTURE: &str = include_str!("fixtures/schema10.json");
    for (charges, reload, kept) in [(12, None, 9), (10, None, 9), (0, Some(1.0), 0)] {
        let mut envelope: Value = serde_json::from_str(FIXTURE).unwrap();
        let player = &mut envelope["data"]["player"];
        let gun = player["abilities"]["entries"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|entry| entry["name"] == "gun")
            .expect("the gun");
        gun["charges"] = charges.into();
        if let Some(left) = reload {
            player["reload"] = Value::from(left);
        }
        let (migrated, dir) = migrate(&format!("schema-10-gun-{charges}"), &envelope.to_string());
        assert_eq!(
            migrated.player.abilities.charges("gun"),
            Some(kept),
            "{charges}"
        );
        assert_eq!(migrated.player.reload, reload, "{charges}: its reload kept");
        if reload.is_some() {
            // Loaded, the reload runs on to a whole magazine.
            let mut fresh = super::harness();
            load(&mut fresh, migrated);
            fresh.world_mut().resource_mut::<Time>().resume();
            fresh.step(90);
            let player = player_entity(&fresh);
            assert_eq!(
                gun::rounds(fresh.world(), player),
                Some((9, 9)),
                "nine at its end"
            );
        }
        let _ = std::fs::remove_dir_all(dir);
    }
}

#[test]
fn a_wizard_loaded_from_an_older_save_still_shoves() {
    let (mut harness, _) = grounded();
    harness.step(5);
    // A 0.104 save: its controller has no shove or step fields.
    let mut older = json(&capture(&harness));
    let controller = older["player"]["controller"]
        .as_object_mut()
        .expect("the player's controller");
    assert!(
        controller.remove("shove").is_some(),
        "a 0.105 save holds it"
    );
    assert!(controller.remove("step_height").is_none(), "he has no step");
    let read: SaveGame = serde_json::from_value(older).expect("an older save reads");
    let mut loaded = reloaded(read, false);
    loaded.harness_mut().set_dt(1.0 / 60.0);
    let player = player_entity(&loaded);
    let at = centre(&loaded, player);
    let ball = toys::spawn_ball(
        loaded.world_mut(),
        Vec2::new(at.x + PLAYER_HALF.x + 40.0, TOP - 7.5),
        true,
    );
    loaded.step(2);
    let start = super::body(&loaded, ball);
    loaded.harness_mut().press_action("move_right");
    loaded.step(30);
    loaded.harness_mut().release_action("move_right");
    let end = super::body(&loaded, ball);
    assert!(
        end.x - start.x > 100.0,
        "the ball went ahead: {start} {end}"
    );
}
