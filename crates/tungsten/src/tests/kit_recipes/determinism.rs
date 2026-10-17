//! The kit determinism hash: each genre's fixture run through a fixed
//! input script for 300 frames at 1/60 s, with hits at fixed frames, the
//! world hashed in entity order and the game's counters and flags after
//! it, the four hashes combined; equal across two in-process runs and
//! pinned. The App flushes commands once a frame, so the hash is a
//! one-step-per-frame claim, as the physics hash is.

use std::fmt::Write as _;

use tungsten_core::physics::Position;
use tungsten_core::{Entity, Transform, World};

use super::{DTS, GENRES, recipe, spawn, tagged};
use crate::kit::{Counters, Flags, Health, KitPlugins, Stats, Tags, damage};
use crate::testing::Harness;

/// The hash of the four recipes, pinned from the first run; moved by M63,
/// whose script spawns the stats, cues, cooldowns, counters, flags and
/// children prefabs and lands hits.
const KIT_RECIPES_HASH: u64 = 0xd2a0_6495_3ac2_b9c1;

const FRAMES: u32 = 300;

fn fnv1a(hash: &mut u64, bytes: &[u8]) {
    for &byte in bytes {
        *hash ^= u64::from(byte);
        *hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
}

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;

/// An entity's index, point, health, then its tags, counters, flags and
/// stat values as text.
type Row = (u32, [f32; 2], Option<(f32, f32)>, String);

/// `counters` as text: `name=value` by name.
fn counters_text(counters: &Counters) -> String {
    counters
        .iter()
        .map(|(name, value)| format!("{name}={value}"))
        .collect::<Vec<_>>()
        .join(",")
}

fn flags_text(flags: &Flags) -> String {
    flags.iter().collect::<Vec<_>>().join(",")
}

/// Every entity's index, point, health, tags, counters, flags and stat
/// values, in entity order, then the game's counters and flags.
fn hash_world(hash: &mut u64, world: &World) {
    let mut rows: Vec<Row> = world
        .query::<(
            Entity,
            Option<&Position>,
            Option<&Transform>,
            Option<&Health>,
            Option<&Tags>,
            Option<&Counters>,
            Option<&Flags>,
            Option<&Stats>,
        )>()
        .map(
            |(entity, position, transform, health, tags, counters, flags, stats)| {
                let point = position.map_or_else(
                    || transform.map_or([0.0, 0.0], |t| [t.position.x, t.position.y]),
                    |p| [p.0.x, p.0.y],
                );
                let health = health.map(|h| (h.current(), h.max()));
                let mut text =
                    tags.map_or_else(String::new, |t| t.iter().collect::<Vec<_>>().join(","));
                if let Some(counters) = counters {
                    let _ = write!(text, "|c:{}", counters_text(counters));
                }
                if let Some(flags) = flags {
                    let _ = write!(text, "|f:{}", flags_text(flags));
                }
                if let Some(stats) = stats {
                    for (name, value) in stats.iter() {
                        let _ = write!(text, "|s:{name}={:08x}", value.to_bits());
                    }
                }
                (entity.id(), point, health, text)
            },
        )
        .collect();
    rows.sort_by_key(|row| row.0);
    for (id, point, health, text) in rows {
        fnv1a(hash, &id.to_le_bytes());
        fnv1a(hash, &point[0].to_bits().to_le_bytes());
        fnv1a(hash, &point[1].to_bits().to_le_bytes());
        if let Some((current, max)) = health {
            fnv1a(hash, &current.to_bits().to_le_bytes());
            fnv1a(hash, &max.to_bits().to_le_bytes());
        }
        fnv1a(hash, text.as_bytes());
    }
    fnv1a(hash, counters_text(world.resource::<Counters>()).as_bytes());
    fnv1a(hash, flags_text(world.resource::<Flags>()).as_bytes());
}

/// The script's hits: at `frame`, `amount` on each entity tagged `tag`.
fn hits(genre: &str) -> &'static [(u32, &'static str, f32)] {
    match genre {
        "platformer" => &[(100, "runner", 1.0)],
        "top_down" => &[(50, "brute", 1.0), (51, "brute", 2.0), (150, "slime", 2.0)],
        "shmup" => &[(120, "wing", 1.0)],
        "grid" => &[(90, "slug", 1.0), (91, "rogue", 3.0)],
        other => panic!("no hits for {other}"),
    }
}

/// The genre's fixed script: its prefabs spawned at fixed points, the move
/// actions and the jump pressed and released at fixed frames.
fn script(genre: &str, harness: &mut Harness) {
    match genre {
        "platformer" => {
            spawn(harness, "ground", [0.0, 300.0]);
            spawn(harness, "hero", [0.0, 270.0]);
            spawn(harness, "door", [200.0, 260.0]);
            spawn(harness, "lift", [400.0, 200.0]);
            spawn(harness, "spark", [10.0, 10.0]);
            spawn(harness, "runner", [100.0, 270.0]);
            spawn(harness, "key", [250.0, 270.0]);
            spawn(harness, "torch", [300.0, 200.0]);
        }
        "top_down" => {
            spawn(harness, "wall", [300.0, 0.0]);
            spawn(harness, "hero", [100.0, 0.0]);
            spawn(harness, "nest", [50.0, 50.0]);
            spawn(harness, "trap", [200.0, 0.0]);
            spawn(harness, "drone", [0.0, 100.0]);
            spawn(harness, "bolt", [600.0, 100.0]);
            spawn(harness, "brute", [400.0, 200.0]);
            spawn(harness, "bomb", [450.0, 50.0]);
            spawn(harness, "hopper", [500.0, 50.0]);
        }
        "shmup" => {
            spawn(harness, "floor", [0.0, 400.0]);
            spawn(harness, "ship", [640.0, 600.0]);
            spawn(harness, "wave", [300.0, 0.0]);
            spawn(harness, "walker", [0.0, 380.0]);
            spawn(harness, "bullet", [640.0, 300.0]);
            spawn(harness, "turret", [200.0, 100.0]);
            spawn(harness, "gunship", [100.0, 100.0]);
        }
        "grid" => {
            spawn(harness, "rogue", [64.0, 64.0]);
            spawn(harness, "hole", [96.0, 96.0]);
            spawn(harness, "potion", [32.0, 32.0]);
            spawn(harness, "delver", [10.0, 10.0]);
            spawn(harness, "trapdoor", [20.0, 20.0]);
            spawn(harness, "slug", [30.0, 30.0]);
        }
        other => panic!("no script for {other}"),
    }
    for frame in 0..FRAMES {
        match frame {
            10 => harness.press_action("move_right"),
            60 => harness.press_action("jump"),
            62 => harness.release_action("jump"),
            120 => {
                harness.release_action("move_right");
                harness.press_action("move_down");
            }
            200 => harness.release_action("move_down"),
            _ => {}
        }
        for &(at, tag, amount) in hits(genre) {
            if at == frame {
                for entity in tagged(harness, tag) {
                    damage(harness.world_mut(), entity, amount);
                }
            }
        }
        harness.step(1);
    }
}

/// Whether the script's cues ran: the runner took the key, and the
/// game's counters counted the kills.
fn reacted(genre: &str, harness: &Harness) -> bool {
    let game = harness.world().resource::<Counters>();
    match genre {
        "platformer" => tagged(harness, "runner").iter().all(|&runner| {
            harness
                .world()
                .get::<Flags>(runner)
                .is_some_and(|flags| flags.has("has_key"))
        }),
        "top_down" => game.get("kills") >= 1,
        "shmup" => game.get("score") == 2,
        "grid" => game.get("slugs") == 1,
        other => panic!("no reaction for {other}"),
    }
}

fn run_once() -> u64 {
    let mut hash = FNV_OFFSET;
    for genre in GENRES {
        let mut harness = recipe(genre, KitPlugins::set(), DTS[0]);
        script(genre, &mut harness);
        assert!(reacted(genre, &harness), "{genre}: the script's cues ran");
        hash_world(&mut hash, harness.world());
        fnv1a(&mut hash, genre.as_bytes());
    }
    hash
}

#[test]
fn the_recipes_hash_the_same_twice_and_match_the_pin() {
    let first = run_once();
    let second = run_once();
    println!("kit recipes hash: {first:#018x}");
    assert_eq!(first, second, "two in-process runs");
    assert_eq!(
        first, KIT_RECIPES_HASH,
        "the pinned hash: a kit change that moves it on purpose records the new value"
    );
}
