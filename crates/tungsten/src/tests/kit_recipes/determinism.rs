//! The kit determinism hash: each genre's fixture run through a fixed
//! input script for 300 frames at 1/60 s, the world hashed in entity
//! order, the four hashes combined; equal across two in-process runs and
//! pinned. The App flushes commands once a frame, so the hash is a
//! one-step-per-frame claim, as the physics hash is.

use tungsten_core::physics::Position;
use tungsten_core::{Entity, Transform, World};

use super::{DTS, GENRES, recipe, spawn};
use crate::kit::{Health, KitPlugins, Tags};
use crate::testing::Harness;

/// The hash of the four recipes, pinned from the first run.
const KIT_RECIPES_HASH: u64 = 0x2b64_8d77_6dff_9170;

const FRAMES: u32 = 300;

fn fnv1a(hash: &mut u64, bytes: &[u8]) {
    for &byte in bytes {
        *hash ^= u64::from(byte);
        *hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
}

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;

/// An entity's index, point, health and tags.
type Row = (u32, [f32; 2], Option<(f32, f32)>, String);

/// Every entity's index, point, health and tags, in entity order.
fn hash_world(hash: &mut u64, world: &World) {
    let mut rows: Vec<Row> = world
        .query::<(
            Entity,
            Option<&Position>,
            Option<&Transform>,
            Option<&Health>,
            Option<&Tags>,
        )>()
        .map(|(entity, position, transform, health, tags)| {
            let point = position.map_or_else(
                || transform.map_or([0.0, 0.0], |t| [t.position.x, t.position.y]),
                |p| [p.0.x, p.0.y],
            );
            let health = health.map(|h| (h.current(), h.max()));
            let tags = tags.map_or_else(String::new, |t| t.iter().collect::<Vec<_>>().join(","));
            (entity.id(), point, health, tags)
        })
        .collect();
    rows.sort_by_key(|row| row.0);
    for (id, point, health, tags) in rows {
        fnv1a(hash, &id.to_le_bytes());
        fnv1a(hash, &point[0].to_bits().to_le_bytes());
        fnv1a(hash, &point[1].to_bits().to_le_bytes());
        if let Some((current, max)) = health {
            fnv1a(hash, &current.to_bits().to_le_bytes());
            fnv1a(hash, &max.to_bits().to_le_bytes());
        }
        fnv1a(hash, tags.as_bytes());
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
        }
        "top_down" => {
            spawn(harness, "wall", [300.0, 0.0]);
            spawn(harness, "hero", [100.0, 0.0]);
            spawn(harness, "nest", [50.0, 50.0]);
            spawn(harness, "trap", [200.0, 0.0]);
            spawn(harness, "drone", [0.0, 100.0]);
            spawn(harness, "bolt", [600.0, 100.0]);
        }
        "shmup" => {
            spawn(harness, "floor", [0.0, 400.0]);
            spawn(harness, "ship", [640.0, 600.0]);
            spawn(harness, "wave", [300.0, 0.0]);
            spawn(harness, "walker", [0.0, 380.0]);
            spawn(harness, "bullet", [640.0, 300.0]);
        }
        "grid" => {
            spawn(harness, "rogue", [64.0, 64.0]);
            spawn(harness, "hole", [96.0, 96.0]);
            spawn(harness, "potion", [32.0, 32.0]);
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
        harness.step(1);
    }
}

fn run_once() -> u64 {
    let mut hash = FNV_OFFSET;
    for genre in GENRES {
        let mut harness = recipe(genre, KitPlugins::set(), DTS[0]);
        script(genre, &mut harness);
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
