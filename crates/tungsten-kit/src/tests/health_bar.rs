use std::path::PathBuf;

use super::*;
use tungsten_core::assets::{FilterMode, TextureHandle, UvRect};
use tungsten_core::{CommandBuffer, Stage, Time};

use crate::{HealthPlugin, damage, heal};

const DT: f32 = 1.0 / 60.0;

/// A World with a clock, a command buffer, health and the bars, and the
/// sprites `brute` (32 × 32) and, with `white`, `engine_white`.
fn rig(white: bool) -> (World, Schedule) {
    let mut world = World::new();
    world.insert_resource(Time::new());
    world.insert_resource(CommandBuffer::new());
    let mut assets = AssetRegistry::new();
    let mut sprite = |id: &str, size: u32| {
        assets.register_sprite(
            id.to_string(),
            FilterMode::Nearest,
            size,
            size,
            PathBuf::from(format!("{id}.png")),
            TextureHandle(0),
            UvRect::FULL,
            None,
            None,
            None,
        )
    };
    sprite("brute", 32);
    if white {
        sprite("engine_white", 1);
    }
    world.insert_resource(assets);
    let mut schedule = Schedule::new();
    HealthPlugin.build(&mut schedule, &mut world);
    HealthBarPlugin.build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    (world, schedule)
}

fn frame(world: &mut World, schedule: &mut Schedule) {
    world.resource_mut::<Time>().advance_frame(DT);
    schedule.run_stage(Stage::FixedUpdate, world, |_, _| {});
    let mut commands = world.remove_resource::<CommandBuffer>().unwrap();
    world.flush_reusing(&mut commands);
    world.insert_resource(commands);
    schedule.run_stage(Stage::PostUpdate, world, |_, _| {});
    world.flush_events();
}

/// A 32 px brute at `at`, depth 5, of 10 health, with `bar`.
fn brute(world: &mut World, at: Vec2, bar: HealthBar) -> Entity {
    let sprite = world
        .resource::<AssetRegistry>()
        .sprite_id("brute")
        .unwrap();
    let mut sprite = Sprite::new(sprite);
    sprite.z_order = 5;
    world.spawn_with((
        Transform::from_position(at),
        sprite,
        Visibility::default(),
        crate::Health::new(10.0),
        bar,
    ))
}

/// The bars' parts, back before fill by depth: each one's transform,
/// colour, depth and whether it shows.
fn parts(world: &World) -> Vec<(Transform, [u8; 4], i32, bool)> {
    let white = world.resource::<AssetRegistry>().sprite_id("engine_white");
    let mut found: Vec<_> = world
        .query::<(&Transform, &Sprite, &Visibility)>()
        .filter(|(_, sprite, _)| Some(sprite.asset_id) == white)
        .map(|(transform, sprite, visibility)| {
            (*transform, sprite.color, sprite.z_order, visibility.visible)
        })
        .collect();
    found.sort_by_key(|part| part.2);
    found
}

#[test]
fn a_bar_falls_with_the_health_and_hides_at_death() {
    let (mut world, mut schedule) = rig(true);
    let brute = brute(&mut world, Vec2::new(100.0, 100.0), HealthBar::new());
    frame(&mut world, &mut schedule);
    let shown = parts(&world);
    assert_eq!(shown.len(), 2);
    let (back, back_color, back_z, back_on) = shown[0];
    let (fill, fill_color, fill_z, fill_on) = shown[1];
    // Centre (116, 116), its top at 100: the back 6 px over it, 6 tall.
    assert_eq!(
        (back.position, back.scale),
        (Vec2::new(84.0, 88.0), Vec2::new(64.0, 6.0))
    );
    assert_eq!(
        (fill.position, fill.scale),
        (Vec2::new(85.0, 89.0), Vec2::new(62.0, 4.0))
    );
    assert_eq!((back_z, fill_z), (6, 7), "over the holder's depth");
    assert!(back_on && fill_on, "shown at full health");
    assert_eq!(back_color, linear_bytes([0.05, 0.04, 0.05, 0.86]));
    assert_eq!(fill_color, linear_bytes([0.77, 0.19, 0.16, 1.0]));
    damage(&mut world, brute, 5.0);
    frame(&mut world, &mut schedule);
    assert_eq!(parts(&world)[1].0.scale, Vec2::new(31.0, 4.0), "half");
    damage(&mut world, brute, 5.0);
    frame(&mut world, &mut schedule);
    assert!(
        parts(&world).iter().all(|part| !part.3),
        "hidden while dead"
    );
    world.despawn(brute);
    frame(&mut world, &mut schedule);
    assert!(parts(&world).is_empty(), "gone with its holder");
}

#[test]
fn a_bar_waits_for_a_hurt_without_full() {
    let (mut world, mut schedule) = rig(true);
    let brute = brute(
        &mut world,
        Vec2::ZERO,
        HealthBar::new()
            .with_size(Vec2::new(40.0, 4.0))
            .with_rise(2.0)
            .with_inset(0.0)
            .hidden_at_full(),
    );
    let showing = |world: &World| parts(world).iter().all(|part| part.3);
    frame(&mut world, &mut schedule);
    assert!(!showing(&world), "hidden at full health");
    damage(&mut world, brute, 2.5);
    frame(&mut world, &mut schedule);
    assert!(showing(&world), "shown once hurt");
    let fill = parts(&world)[1].0;
    // Centre (16, 16), its top at 0: the back 2 px over it, 4 tall.
    assert_eq!(
        (fill.position, fill.scale),
        (Vec2::new(-4.0, -6.0), Vec2::new(30.0, 4.0))
    );
    heal(&mut world, brute, 2.5);
    frame(&mut world, &mut schedule);
    assert!(!showing(&world), "hidden again when whole");
    // The scene form and its checks.
    let def: HealthBarDef =
        serde_json::from_value(serde_json::json!({ "full": false, "size": [40, 4] })).unwrap();
    let bar = HealthBar::from(def);
    assert_eq!((bar.size(), bar.full()), (Vec2::new(40.0, 4.0), false));
    for refused in [
        serde_json::json!({ "size": [0, 4] }),
        serde_json::json!({ "rise": -1 }),
        serde_json::json!({ "fill": [1, 1, 1] }),
        serde_json::json!({ "height": 4 }),
    ] {
        assert!(
            serde_json::from_value::<HealthBarDef>(refused.clone()).is_err(),
            "{refused}"
        );
    }
}

#[test]
fn a_bar_warns_once_without_engine_white() {
    let (mut world, mut schedule) = rig(false);
    brute(&mut world, Vec2::ZERO, HealthBar::new());
    brute(&mut world, Vec2::new(100.0, 0.0), HealthBar::new());
    let mut bars = Bars::default();
    bars.early(&mut world);
    assert!(bars.warned, "warned at the first holder");
    assert!(bars.parts.is_empty(), "and nothing spawned");
    bars.early(&mut world);
    assert!(bars.parts.is_empty());
    // The passes draw none either.
    frame(&mut world, &mut schedule);
    frame(&mut world, &mut schedule);
    assert_eq!(world.query::<&Sprite>().count(), 2, "the brutes alone");
}
