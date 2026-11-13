use super::*;
use serde_json::json;
use tungsten_core::physics::{Collider, Position, RigidBody};
use tungsten_core::{ComponentRegistry, Stage};

use crate::damage::pipeline::DamagePart;
use crate::lifecycle::ended::KIT_DESPAWN;
use crate::overlap::hit::{HIT_GRID, HitShape, Hitbox, Hurtbox};

#[test]
fn a_projectile_reads_back_its_pierce_range_and_bounces() {
    let plain = Projectile::new();
    assert_eq!(plain, Projectile::default());
    assert_eq!(
        (plain.pierce(), plain.range(), plain.bounces()),
        (0, None, 0)
    );
    assert_eq!(
        (plain.hits(), plain.travelled(), plain.is_spent()),
        (0, 0.0, false)
    );
    let lance = Projectile::new()
        .with_pierce(2)
        .with_range(300.0)
        .with_bounces(1);
    assert_eq!(
        (lance.pierce(), lance.range(), lance.bounces()),
        (2, Some(300.0), 1)
    );
}

#[test]
fn a_projectile_is_data_with_its_progress() {
    let mut bolt = Projectile::new().with_range(600.0);
    bolt.hits = 1;
    bolt.travelled = 120.5;
    bolt.bounced = 1;
    bolt.spent = true;
    let saved = serde_json::to_value(&bolt).unwrap();
    assert_eq!(
        saved,
        json!({ "pierce": 0, "range": 600.0, "bounces": 0, "hits": 1, "travelled": 120.5, "bounced": 1, "spent": true })
    );
    assert_eq!(serde_json::from_value::<Projectile>(saved).unwrap(), bolt);
    for bad in [
        json!({ "range": 0 }),
        json!({ "travelled": -1 }),
        json!({ "pierce": -1 }),
        json!({ "speed": 3 }),
    ] {
        assert!(
            serde_json::from_value::<Projectile>(bad.clone()).is_err(),
            "{bad}"
        );
    }
}

#[test]
fn the_scene_form_takes_each_field_optionally() {
    let mut world = World::new();
    let mut schedule = Schedule::new();
    ProjectilePlugin.build(&mut schedule, &mut world);
    let components = world.resource::<ComponentRegistry>();
    for good in [
        json!({}),
        json!({ "pierce": 1, "range": 600, "bounces": 2 }),
    ] {
        assert!(
            components.check_in(PROJECTILE, &good, &world).is_ok(),
            "{good}"
        );
    }
    for bad in [
        json!({ "range": -5 }),
        json!({ "hits": 1 }),
        json!({ "bounces": 1.5 }),
    ] {
        assert!(
            components.check_in(PROJECTILE, &bad, &world).is_err(),
            "{bad}"
        );
    }
}

#[test]
fn the_plugin_registers_its_queues_its_tick_after_the_grid_and_the_despawn() {
    let mut world = World::new();
    let mut schedule = Schedule::new();
    ProjectilePlugin.build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    assert!(world.has_event::<ProjectileExpired>() && world.has_event::<Struck>());
    assert!(world.has_event::<ProjectileBounced>() && world.has_event::<ProjectilePassed>());
    let order: Vec<&str> = schedule.names(Stage::FixedUpdate).collect();
    let at = |name: &str| order.iter().position(|n| *n == name).unwrap();
    assert!(at(HIT_GRID) < at(PROJECTILE_TICK) && at(PROJECTILE_TICK) < at(KIT_DESPAWN));
    let expired = ProjectileExpired::new(world.spawn(), glam::Vec2::new(1.0, 2.0));
    assert_eq!(
        (expired.position, expired.wall),
        (glam::Vec2::new(1.0, 2.0), None)
    );
    let wall = WallContact::new(None, glam::Vec2::Y, 1);
    let at_wall = ProjectileExpired::at_wall(expired.entity, glam::Vec2::ZERO, wall);
    assert_eq!(at_wall.wall, Some(wall));
}

#[test]
#[should_panic(expected = "distance must be finite and > 0")]
fn a_zero_range_panics() {
    let _ = Projectile::new().with_range(0.0);
}

/// A World running projectiles, stats and the damage pipeline at 1/60 s steps,
/// with `types` declared as tags and as damage types.
fn projectile_rig(types: &[&str]) -> (World, Schedule) {
    let mut world = World::new();
    world.insert_resource(tungsten_core::Time::new());
    world.insert_resource(tungsten_core::CommandBuffer::new());
    let mut schedule = Schedule::new();
    crate::TagsPlugin::new()
        .with_tags(types.iter().copied())
        .build(&mut schedule, &mut world);
    crate::HealthPlugin.build(&mut schedule, &mut world);
    crate::StatsPlugin::new().build(&mut schedule, &mut world);
    ProjectilePlugin.build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    let entry: serde_json::Map<String, serde_json::Value> = types
        .iter()
        .map(|name| ((*name).to_owned(), serde_json::json!({})))
        .collect();
    let types: crate::damage::kinds::DamageTypes =
        serde_json::from_value(serde_json::Value::Object(entry)).unwrap();
    world
        .resource_mut::<tungsten_core::DataRegistry>()
        .insert("game", types);
    (world, schedule)
}

/// One fixed step of 1/60 s.
fn fixed_step(world: &mut World, schedule: &mut Schedule) {
    world
        .resource_mut::<tungsten_core::Time>()
        .advance_frame(1.0 / 60.0);
    schedule.run_stage(Stage::FixedUpdate, world, |_, _| {});
}

/// Each part sent so far: its amount and its kinds by name.
fn parts_sent(world: &World) -> Vec<(f32, Vec<&str>)> {
    world
        .resource::<tungsten_core::EventQueue<crate::DamagedPart>>()
        .iter()
        .map(|part| (part.amount, part.kinds.names(world)))
        .collect()
}

fn typed(amount: f32, name: &str) -> DamagePart {
    DamagePart::new(amount).with_tags(Tags::new([name]))
}

/// A target at the origin: a hurtbox of radius 8 and plenty of health.
fn target(world: &mut World) -> Entity {
    world.spawn_with((
        tungsten_core::Transform::from_position(Vec2::ZERO),
        Hurtbox::new(HitShape::circle(8.0)),
        crate::Health::new(1000.0),
    ))
}

#[test]
fn a_typed_projectiles_hits_carry_its_hitboxs_parts() {
    let (mut world, mut schedule) = projectile_rig(&["fire", "ice"]);
    target(&mut world);
    let bolt = world.spawn_with((
        tungsten_core::Transform::from_position(Vec2::ZERO),
        Hitbox::new(HitShape::circle(2.0), 0.0)
            .with_parts([typed(3.0, "fire"), DamagePart::new(1.0)])
            .with_tags(Tags::new(["ice"])),
        Projectile::new(),
    ));
    fixed_step(&mut world, &mut schedule);
    assert_eq!(
        parts_sent(&world),
        [(3.0, vec!["fire"]), (1.0, vec!["ice"])],
        "its hitbox's parts, the untyped one of its tags"
    );
    assert_eq!(world.get::<Projectile>(bolt).unwrap().hits(), 1);
}

#[test]
fn a_loaded_piercing_projectile_given_its_targets_passes_none_twice() {
    use tungsten_core::physics::{Position, PrevPosition};

    // A beam twice as long as its step, so a target stays in it for steps.
    let run = |named_again: bool| -> (Vec<f32>, u32) {
        let (mut world, mut schedule) = projectile_rig(&[]);
        let targets: Vec<Entity> = [30.0, 70.0]
            .into_iter()
            .map(|x| {
                world.spawn_with((
                    Position(Vec2::new(x, 0.0)),
                    PrevPosition(Vec2::new(x, 0.0)),
                    Hurtbox::new(HitShape::circle(4.0)),
                    crate::Health::new(100.0),
                ))
            })
            .collect();
        let beam = world.spawn_with((
            Position(Vec2::new(20.0, 0.0)),
            PrevPosition(Vec2::ZERO),
            Hitbox::new(HitShape::aabb(Vec2::new(20.0, 2.0)), 10.0),
            Projectile::new().with_pierce(3),
        ));
        fixed_step(&mut world, &mut schedule);
        assert_eq!(world.get::<Projectile>(beam).unwrap().hits(), 1);
        // The game's save: the projectile, its hitbox and the passed
        // targets by the game's IDs.
        let hitbox = world.get::<Hitbox>(beam).unwrap();
        let ids: Vec<usize> = hitbox
            .hit_targets()
            .iter()
            .map(|hit| targets.iter().position(|target| target == hit).unwrap())
            .collect();
        assert_eq!(ids, [0]);
        let mut loaded: Hitbox =
            serde_json::from_value(serde_json::to_value(hitbox).unwrap()).unwrap();
        let projectile: Projectile = serde_json::from_value(
            serde_json::to_value(world.get::<Projectile>(beam).unwrap()).unwrap(),
        )
        .unwrap();
        if named_again {
            loaded.set_hit_targets(ids.iter().map(|&id| targets[id]));
        }
        world.insert(beam, loaded);
        world.insert(beam, projectile);
        // Its next step: the first target still in the beam, the second
        // reached.
        world.get_mut::<PrevPosition>(beam).unwrap().0 = Vec2::new(20.0, 0.0);
        world.get_mut::<Position>(beam).unwrap().0 = Vec2::new(50.0, 0.0);
        fixed_step(&mut world, &mut schedule);
        let health = targets
            .iter()
            .map(|&target| world.get::<crate::Health>(target).unwrap().current())
            .collect();
        (health, world.get::<Projectile>(beam).unwrap().hits())
    };
    assert_eq!(run(true), (vec![90.0, 90.0], 2), "each passed once");
    assert_eq!(
        run(false),
        (vec![80.0, 90.0], 3),
        "unnamed: the first again"
    );
}

// M76: walls by their physics layers.

#[test]
fn the_wall_masks_default_to_every_layer_and_none_and_round_trip() {
    let plain = Projectile::new();
    assert_eq!((plain.bounce_layers(), plain.pass_layers()), (u32::MAX, 0));
    // A save without the masks is today's projectile, and one at their
    // defaults leaves them out.
    let saved = serde_json::to_value(&plain).unwrap();
    assert!(saved.get("bounce_layers").is_none() && saved.get("pass_layers").is_none());
    let bolt = Projectile::new()
        .with_bounces(2)
        .with_bounce_layers(1 << 2)
        .with_pass_layers(1 << 3);
    let saved = serde_json::to_value(&bolt).unwrap();
    assert_eq!(
        (saved["bounce_layers"].clone(), saved["pass_layers"].clone()),
        (json!(4), json!(8))
    );
    assert_eq!(serde_json::from_value::<Projectile>(saved).unwrap(), bolt);
    assert_eq!(
        serde_json::from_value::<Projectile>(json!({ "bounces": 1 })).unwrap(),
        Projectile::new().with_bounces(1)
    );
    // The scene form takes layer numbers.
    let mut world = World::new();
    let mut schedule = Schedule::new();
    ProjectilePlugin.build(&mut schedule, &mut world);
    let components = world.resource::<ComponentRegistry>();
    let good = json!({ "bounces": 2, "bounce_layers": [0, 2], "pass_layers": [3, 31] });
    assert!(components.check_in(PROJECTILE, &good, &world).is_ok());
    for bad in [
        json!({ "pass_layers": [32] }),
        json!({ "bounce_layers": [-1] }),
        json!({ "bounce_layers": 4 }),
    ] {
        assert!(
            components.check_in(PROJECTILE, &bad, &world).is_err(),
            "{bad}"
        );
    }
    let def: ProjectileDef = serde_json::from_value(good).unwrap();
    let read = def.into_projectile(&world);
    assert_eq!(
        (read.bounce_layers(), read.pass_layers()),
        (0b101, (1 << 3) | (1 << 31))
    );
}

/// A World with physics, linear motion and projectiles, and room for a
/// tile map.
fn wall_rig() -> (World, Schedule) {
    use tungsten_core::physics::PhysicsPlugin;

    let mut world = World::new();
    world.insert_resource(tungsten_core::Time::new());
    world.insert_resource(tungsten_core::CommandBuffer::new());
    world.insert_resource(tungsten_core::TilemapRegistry::new());
    let mut schedule = Schedule::new();
    PhysicsPlugin.build(&mut schedule, &mut world);
    crate::LinearMotionPlugin.build(&mut schedule, &mut world);
    ProjectilePlugin.build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    (world, schedule)
}

/// A static box of half extents (5, 50) at (`x`, 40) in `layers`.
fn slab(world: &mut World, x: f32, layers: u32) -> Entity {
    use tungsten_core::physics::RigidBodyBundle;

    let wall = world.spawn_with(RigidBodyBundle::r#static(
        Position::new(x, 40.0),
        Collider::aabb(Vec2::new(5.0, 50.0)),
    ));
    world.get_mut::<RigidBody>(wall).unwrap().layers = layers;
    wall
}

/// A column of 16 px tiles over x 160 to 176, rows 0 to 11.
fn tile_column(world: &mut World) {
    use tungsten_core::{LayerKind, TilemapData, TilemapInstance, TilemapRegistry};

    let mut map = TilemapData::new(16, 16, 12, 12);
    let solid = map.add_tile("solid");
    let layer = map.add_layer("solid", LayerKind::Collision);
    for row in 0..12 {
        map.set_tile(layer, 10, row, solid);
    }
    world
        .resource_mut::<TilemapRegistry>()
        .insert("column".into(), map);
    let instance = world.spawn();
    world.insert(instance, TilemapInstance::new("column", Vec2::ZERO));
}

/// A bolt of radius 2 at (`x`, 40), flying at `vx` px/s.
fn flier(world: &mut World, x: f32, vx: f32, projectile: Projectile) -> Entity {
    use tungsten_core::physics::PrevPosition;

    let at = Vec2::new(x, 40.0);
    world.spawn_with((
        Position(at),
        PrevPosition(at),
        crate::LinearMotion::new(Vec2::new(vx, 0.0)),
        Hitbox::new(HitShape::circle(2.0), 1.0),
        projectile,
    ))
}

/// What a projectile met, in order: "passed", "bounced" or "expired", the
/// x it met it at and the wall.
type Meeting = (&'static str, f32, Option<WallContact>);

/// Steps of `dt` until `bolt` ends, at most a second's worth, and every
/// meeting sent, in step order.
fn meetings(world: &mut World, schedule: &mut Schedule, bolt: Entity, dt: f32) -> Vec<Meeting> {
    let mut seen = (0, 0, 0);
    let mut out = Vec::new();
    for _ in 0..=((1.0 / dt).ceil() as u32) {
        world
            .resource_mut::<tungsten_core::Time>()
            .advance_frame(dt);
        schedule.run_stage(Stage::FixedUpdate, world, |_, _| {});
        let passed: Vec<_> = world
            .resource::<EventQueue<ProjectilePassed>>()
            .iter()
            .filter(|event| event.entity == bolt)
            .map(|event| ("passed", event.position.x, Some(event.wall)))
            .collect();
        let bounced: Vec<_> = world
            .resource::<EventQueue<ProjectileBounced>>()
            .iter()
            .filter(|event| event.entity == bolt)
            .map(|event| ("bounced", event.position.x, Some(event.wall)))
            .collect();
        let expired: Vec<_> = world
            .resource::<EventQueue<ProjectileExpired>>()
            .iter()
            .filter(|event| event.entity == bolt)
            .map(|event| ("expired", event.position.x, event.wall))
            .collect();
        out.extend(passed[seen.0..].iter().copied());
        out.extend(bounced[seen.1..].iter().copied());
        out.extend(expired[seen.2..].iter().copied());
        seen = (passed.len(), bounced.len(), expired.len());
        if !expired.is_empty() {
            break;
        }
    }
    out
}

/// `got` matches `want` by kind, x within 1e-3 and wall.
fn same_meetings(got: &[Meeting], want: &[Meeting]) -> bool {
    got.len() == want.len()
        && got
            .iter()
            .zip(want)
            .all(|(a, b)| a.0 == b.0 && (a.1 - b.1).abs() < 1e-3 && a.2 == b.2)
}

const DTS: [f32; 3] = [1.0 / 30.0, 1.0 / 60.0, 1.0 / 144.0];

#[test]
fn a_wall_in_the_pass_layers_is_passed_and_the_next_ends_it_at_three_step_rates() {
    for dt in DTS {
        let (mut world, mut schedule) = wall_rig();
        let wood = slab(&mut world, 100.0, 1 << 3);
        let stone = slab(&mut world, 200.0, 1);
        let bolt = flier(
            &mut world,
            0.0,
            1200.0,
            Projectile::new().with_pass_layers(1 << 3),
        );
        let got = meetings(&mut world, &mut schedule, bolt, dt);
        let left = Vec2::new(-1.0, 0.0);
        let want = [
            (
                "passed",
                93.0,
                Some(WallContact::new(Some(wood), left, 1 << 3)),
            ),
            (
                "expired",
                193.0,
                Some(WallContact::new(Some(stone), left, 1)),
            ),
        ];
        assert!(same_meetings(&got, &want), "dt {dt}: {got:?}");
    }
}

#[test]
fn a_wall_outside_the_bounce_layers_ends_a_shot_with_bounces_left() {
    for dt in DTS {
        let (mut world, mut schedule) = wall_rig();
        let iron = slab(&mut world, 100.0, 1 << 2);
        let stone = slab(&mut world, -100.0, 1);
        let bolt = flier(
            &mut world,
            0.0,
            1200.0,
            Projectile::new().with_bounces(2).with_bounce_layers(1 << 2),
        );
        let got = meetings(&mut world, &mut schedule, bolt, dt);
        let want = [
            (
                "bounced",
                93.0,
                Some(WallContact::new(Some(iron), Vec2::new(-1.0, 0.0), 1 << 2)),
            ),
            (
                "expired",
                -93.0,
                Some(WallContact::new(Some(stone), Vec2::new(1.0, 0.0), 1)),
            ),
        ];
        assert!(same_meetings(&got, &want), "dt {dt}: {got:?}");
        let projectile = world.get::<Projectile>(bolt).unwrap();
        assert_eq!(
            (projectile.bounces(), projectile.is_spent()),
            (2, true),
            "a bounce left, spent at stone"
        );
    }
}

#[test]
fn a_tile_is_in_layer_0_and_follows_the_masks() {
    let left = Vec2::new(-1.0, 0.0);
    let tile = Some(WallContact::new(None, left, 1));
    for dt in DTS {
        // Passed: met where it enters the column, then the box behind.
        let (mut world, mut schedule) = wall_rig();
        tile_column(&mut world);
        let iron = slab(&mut world, 260.0, 1 << 2);
        let bolt = flier(
            &mut world,
            100.0,
            1200.0,
            Projectile::new().with_pass_layers(1),
        );
        let got = meetings(&mut world, &mut schedule, bolt, dt);
        let want = [
            ("passed", 158.0, tile),
            (
                "expired",
                253.0,
                Some(WallContact::new(Some(iron), left, 1 << 2)),
            ),
        ];
        assert!(same_meetings(&got, &want), "passed, dt {dt}: {got:?}");
        // Turned, then ended at a box in layer 0.
        let (mut world, mut schedule) = wall_rig();
        tile_column(&mut world);
        let stone = slab(&mut world, 40.0, 1);
        let bolt = flier(
            &mut world,
            100.0,
            1200.0,
            Projectile::new().with_bounces(1).with_bounce_layers(1),
        );
        let got = meetings(&mut world, &mut schedule, bolt, dt);
        let want = [
            ("bounced", 158.0, tile),
            (
                "expired",
                47.0,
                Some(WallContact::new(Some(stone), Vec2::new(1.0, 0.0), 1)),
            ),
        ];
        assert!(same_meetings(&got, &want), "bounced, dt {dt}: {got:?}");
        // Outside its bounce layers: ended with its bounce left.
        let (mut world, mut schedule) = wall_rig();
        tile_column(&mut world);
        let bolt = flier(
            &mut world,
            100.0,
            1200.0,
            Projectile::new().with_bounces(1).with_bounce_layers(1 << 2),
        );
        let got = meetings(&mut world, &mut schedule, bolt, dt);
        assert!(
            same_meetings(&got, &[("expired", 158.0, tile)]),
            "ended, dt {dt}: {got:?}"
        );
    }
}

#[test]
fn a_pass_and_a_pierce_are_independent_and_a_hit_before_the_wall_lands_first() {
    let (mut world, mut schedule) = wall_rig();
    let wood = slab(&mut world, 100.0, 1 << 3);
    // Behind the wood, before the stone.
    let target = world.spawn_with((
        Position::new(150.0, 40.0),
        tungsten_core::physics::PrevPosition(Vec2::new(150.0, 40.0)),
        Hurtbox::new(HitShape::circle(4.0)),
        crate::Health::new(100.0),
    ));
    slab(&mut world, 200.0, 1);
    let bolt = flier(
        &mut world,
        0.0,
        1200.0,
        Projectile::new().with_pass_layers(1 << 3),
    );
    let got = meetings(&mut world, &mut schedule, bolt, 1.0 / 60.0);
    assert_eq!(got.len(), 2, "{got:?}");
    assert_eq!((got[0].0, got[0].2.unwrap().entity), ("passed", Some(wood)));
    assert_eq!(
        (got[1].0, got[1].2),
        ("expired", None),
        "ended at the target, past the wood"
    );
    assert!((got[1].1 - 144.0).abs() < 1e-3, "{got:?}");
    let struck: Vec<Entity> = world
        .resource::<EventQueue<Struck>>()
        .iter()
        .map(|event| event.target)
        .collect();
    assert_eq!(struck, [target]);
}

/// Columns of 16 px tiles over rows 0 to 11, each (column, the physics
/// layers of its own collision layer).
fn tile_columns(world: &mut World, columns: &[(u32, u32)]) {
    use tungsten_core::{LayerKind, LayerPhysics, TilemapData, TilemapInstance, TilemapRegistry};

    let mut map = TilemapData::new(16, 16, 16, 12);
    let solid = map.add_tile("solid");
    for &(col, layers) in columns {
        let layer = map.add_layer(format!("column {col}"), LayerKind::Collision);
        for row in 0..12 {
            map.set_tile(layer, col, row, solid);
        }
        map.set_layer_physics(
            layer,
            LayerPhysics {
                layers,
                mask: RigidBody::ALL_LAYERS,
            },
        );
    }
    world
        .resource_mut::<TilemapRegistry>()
        .insert("columns".into(), map);
    let instance = world.spawn();
    world.insert(instance, TilemapInstance::new("columns", Vec2::ZERO));
}

#[test]
fn shots_meet_tiles_by_their_layers() {
    let left = Vec2::new(-1.0, 0.0);
    for dt in DTS {
        // A column in layer 3, passed under `pass_layers` 3.
        let (mut world, mut schedule) = wall_rig();
        tile_columns(&mut world, &[(10, 1 << 3)]);
        let iron = slab(&mut world, 260.0, 1 << 2);
        let bolt = flier(
            &mut world,
            100.0,
            1200.0,
            Projectile::new().with_pass_layers(1 << 3),
        );
        let got = meetings(&mut world, &mut schedule, bolt, dt);
        let want = [
            ("passed", 158.0, Some(WallContact::new(None, left, 1 << 3))),
            (
                "expired",
                253.0,
                Some(WallContact::new(Some(iron), left, 1 << 2)),
            ),
        ];
        assert!(same_meetings(&got, &want), "passed, dt {dt}: {got:?}");
        // A column in layer 2, bounced under `bounce_layers` 2.
        let (mut world, mut schedule) = wall_rig();
        tile_columns(&mut world, &[(10, 1 << 2)]);
        let stone = slab(&mut world, 40.0, 1);
        let bolt = flier(
            &mut world,
            100.0,
            1200.0,
            Projectile::new().with_bounces(1).with_bounce_layers(1 << 2),
        );
        let got = meetings(&mut world, &mut schedule, bolt, dt);
        let want = [
            ("bounced", 158.0, Some(WallContact::new(None, left, 1 << 2))),
            (
                "expired",
                47.0,
                Some(WallContact::new(Some(stone), Vec2::new(1.0, 0.0), 1)),
            ),
        ];
        assert!(same_meetings(&got, &want), "bounced, dt {dt}: {got:?}");
        // A column in layers 0 and 1 is passed under `pass_layers` 0, and the
        // column in layer 2 behind it stops the shot.
        let (mut world, mut schedule) = wall_rig();
        tile_columns(&mut world, &[(10, 0b11), (14, 1 << 2)]);
        let bolt = flier(
            &mut world,
            100.0,
            1200.0,
            Projectile::new().with_pass_layers(1),
        );
        let got = meetings(&mut world, &mut schedule, bolt, dt);
        let want = [
            ("passed", 158.0, Some(WallContact::new(None, left, 0b11))),
            ("expired", 222.0, Some(WallContact::new(None, left, 1 << 2))),
        ];
        assert!(same_meetings(&got, &want), "two layers, dt {dt}: {got:?}");
    }
}

#[test]
fn a_bolt_carrying_a_delivery_strikes_from_it_whatever_its_hitboxs_damage() {
    let (mut world, mut schedule) = projectile_rig(&["fire"]);
    target(&mut world);
    let holder = world.spawn_with((Tags::new(["fire"]),));
    let zap = crate::Ability::new()
        .with_payload(crate::Payload::Hitbox(0.1))
        .with_skill(crate::Skill::new().with_damage("fire", 5.0, 5.0));
    let delivery =
        crate::intent::sheet::resolve(&world, holder, "zap", &zap, zap.skill().unwrap()).delivery;
    world.spawn_with((
        tungsten_core::Transform::from_position(Vec2::ZERO),
        Hitbox::new(HitShape::circle(2.0), 0.0),
        Projectile::new(),
        delivery,
    ));
    fixed_step(&mut world, &mut schedule);
    assert_eq!(
        parts_sent(&world),
        [(5.0, vec!["fire"])],
        "the snapshot's part"
    );
    let hit = world
        .resource::<tungsten_core::EventQueue<crate::Damaged>>()
        .iter()
        .next()
        .copied()
        .unwrap();
    assert_eq!(hit.source, Some(holder), "its holder's credit");
}

// 7.10c, Step 8: a projectile's end blast (S-73).

/// A World with physics, tags, stats, health, linear motion, lifetimes and
/// projectiles, chemistry with no rule when `chemistry`, a trace and room
/// for a tile map.
fn blast_rig(chemistry: bool) -> (World, Schedule) {
    use tungsten_core::physics::PhysicsPlugin;

    let mut world = World::new();
    world.insert_resource(tungsten_core::Time::new());
    world.insert_resource(tungsten_core::CommandBuffer::new());
    world.insert_resource(tungsten_core::TilemapRegistry::new());
    world.insert_resource(crate::foundation::trace::KitTrace::default());
    let mut schedule = Schedule::new();
    PhysicsPlugin.build(&mut schedule, &mut world);
    crate::TagsPlugin::new().build(&mut schedule, &mut world);
    crate::StatsPlugin::new().build(&mut schedule, &mut world);
    crate::HealthPlugin.build(&mut schedule, &mut world);
    crate::LinearMotionPlugin.build(&mut schedule, &mut world);
    crate::LifetimePlugin.build(&mut schedule, &mut world);
    if chemistry {
        crate::ChemistryPlugin::new().build(&mut schedule, &mut world);
    }
    ProjectilePlugin.build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    (world, schedule)
}

/// One 1/60 s step and the flushes that end its frame: the step's
/// expiries and the damage parts its resolve landed.
fn blast_step(
    world: &mut World,
    schedule: &mut Schedule,
) -> (Vec<ProjectileExpired>, Vec<crate::DamagedPart>) {
    world
        .resource_mut::<tungsten_core::Time>()
        .advance_frame(1.0 / 60.0);
    schedule.run_stage(Stage::FixedUpdate, world, |_, _| {});
    world
        .resource_mut::<tungsten_core::Time>()
        .leave_fixed_steps();
    let mut commands = world
        .remove_resource::<tungsten_core::CommandBuffer>()
        .unwrap();
    world.flush_reusing(&mut commands);
    world.insert_resource(commands);
    world.flush_events();
    let expired = world
        .resource::<EventQueue<ProjectileExpired>>()
        .iter()
        .copied()
        .collect();
    let parts = world
        .resource::<EventQueue<crate::DamagedPart>>()
        .iter()
        .copied()
        .collect();
    (expired, parts)
}

/// `projectile` ending in `{ "blast": { "radius": 50, "damage": 10 } }`.
fn blasting(mut projectile: Projectile) -> Projectile {
    let end: ProjectileEnd =
        serde_json::from_value(json!({ "blast": { "radius": 50, "damage": 10 } })).unwrap();
    projectile.end = Some(end);
    projectile
}

/// A bolt of radius 2 at the origin, flying along +x at 600 px/s: 10 px a
/// step.
fn bolt(world: &mut World, projectile: Projectile) -> Entity {
    world.spawn_with((
        Position(Vec2::ZERO),
        tungsten_core::physics::PrevPosition(Vec2::ZERO),
        crate::LinearMotion::new(Vec2::new(600.0, 0.0)),
        Hitbox::new(HitShape::circle(2.0), 0.0),
        projectile,
    ))
}

/// Something with 100 health at `at`, which nothing but a blast meets.
fn bystander(world: &mut World, at: Vec2) -> Entity {
    world.spawn_with((
        tungsten_core::Transform::from_position(at),
        crate::Health::new(100.0),
    ))
}

/// The parts in `parts` on `entity`: amount, credit and depth.
fn parts_on(parts: &[crate::DamagedPart], entity: Entity) -> Vec<(f32, Option<Entity>, u8)> {
    parts
        .iter()
        .filter(|part| part.entity == entity)
        .map(|part| (part.amount, part.source, part.depth))
        .collect()
}

#[test]
fn an_end_blast_goes_off_at_each_of_its_ends_and_not_at_a_lifetimes() {
    use tungsten_core::physics::RigidBodyBundle;

    // Each end, 10 px a step: its name, the projectile, the x its centre
    // ends at, and the x it met something at on the way without ending.
    let lanes = [
        // Past its pierce: it passes a hurtbox of radius 8 at x 100 and
        // ends at the next, at x 200.
        ("hit", Projectile::new().with_pierce(1), 190.0, Some(90.0)),
        // A wall it turns at, its face at x 195, then one it does not, its
        // face at x -95.
        (
            "wall",
            Projectile::new().with_bounces(1),
            -93.0,
            Some(193.0),
        ),
        // Its range.
        ("range", Projectile::new().with_range(150.0), 150.0, None),
    ];
    for (end, projectile, x, met) in lanes {
        let (mut world, mut schedule) = blast_rig(true);
        match end {
            "hit" => {
                for at in [100.0, 200.0] {
                    world.spawn_with((
                        tungsten_core::Transform::from_position(Vec2::new(at, 0.0)),
                        Hurtbox::new(HitShape::circle(8.0)),
                        crate::Health::new(100.0),
                    ));
                }
            }
            "wall" => {
                for at in [200.0, -100.0] {
                    world.spawn_with(RigidBodyBundle::r#static(
                        Position::new(at, 0.0),
                        Collider::aabb(Vec2::new(5.0, 50.0)),
                    ));
                }
            }
            _ => {}
        }
        // 30 px off where it ends, and where it met the first.
        let near = bystander(&mut world, Vec2::new(x, 30.0));
        let passed = met.map(|met| bystander(&mut world, Vec2::new(met, 30.0)));
        let shooter = world.spawn();
        let shot = bolt(&mut world, blasting(projectile));
        world.insert(shot, Source::new(shooter));
        let mut ended = None;
        let mut hurt = Vec::new();
        for _ in 0..60 {
            let (expired, parts) = blast_step(&mut world, &mut schedule);
            hurt = parts_on(&parts, near);
            if let Some(expired) = expired.iter().find(|expired| expired.entity == shot) {
                ended = Some(expired.position);
                break;
            }
            assert!(hurt.is_empty(), "{end}: nothing before it ends");
        }
        let at = ended.unwrap_or_else(|| panic!("{end}: it ends"));
        assert!(
            (at - Vec2::new(x, 0.0)).length() < 1e-3,
            "{end}: it ends at {at}"
        );
        assert!(
            world.resource::<Blasts>().0.is_empty(),
            "{end}: never queued for chemistry's run"
        );
        // At its own step's resolve: 10 at 1 - 30 / 50, from its source, at
        // depth 0; nothing where it passed, turned or met the first.
        assert_eq!(hurt.len(), 1, "{end}: {hurt:?}");
        assert!((hurt[0].0 - 4.0).abs() < 1e-4, "{end}: {hurt:?}");
        assert_eq!((hurt[0].1, hurt[0].2), (Some(shooter), 0), "{end}");
        if let Some(passed) = passed {
            assert_eq!(
                world.get::<crate::Health>(passed).unwrap().current(),
                100.0,
                "{end}: no blast on the way"
            );
        }
        // The kit despawned it at its step's end; its end went off once.
        let (_, parts) = blast_step(&mut world, &mut schedule);
        assert!(parts_on(&parts, near).is_empty(), "{end}: once");
        assert!(!world.is_alive(shot), "{end}");
        assert!(world.resource::<Blasts>().0.is_empty(), "{end}");
    }
    // A lifetime's end is none of the projectile's: no blast.
    let (mut world, mut schedule) = blast_rig(true);
    let near = bystander(&mut world, Vec2::new(60.0, 30.0));
    let shot = bolt(&mut world, blasting(Projectile::new()));
    world.insert(shot, crate::Lifetime::new(0.1));
    for _ in 0..20 {
        let (expired, parts) = blast_step(&mut world, &mut schedule);
        assert!(expired.is_empty() && parts_on(&parts, near).is_empty());
        assert!(world.resource::<Blasts>().0.is_empty());
    }
    assert!(!world.is_alive(shot), "its lifetime took it");
    assert_eq!(world.get::<crate::Health>(near).unwrap().current(), 100.0);
}

#[test]
fn an_end_blast_goes_off_with_no_rule_and_an_empty_queue() {
    // Chemistry with no `interactions` entry and nothing queued: the end's
    // blast goes off in its own step all the same.
    let (mut world, mut schedule) = blast_rig(true);
    assert!(!crate::chemistry::rules::any_rule(&world));
    let near = bystander(&mut world, Vec2::new(30.0, 20.0));
    let shot = bolt(&mut world, blasting(Projectile::new().with_range(30.0)));
    for _ in 0..2 {
        let (expired, _) = blast_step(&mut world, &mut schedule);
        assert!(expired.is_empty());
    }
    assert_eq!(world.resource::<crate::Chemistry>().pending(), 0);
    let (expired, parts) = blast_step(&mut world, &mut schedule);
    assert_eq!(
        expired
            .iter()
            .map(|expired| expired.entity)
            .collect::<Vec<_>>(),
        [shot]
    );
    assert!(
        world.resource::<Blasts>().0.is_empty(),
        "never queued for chemistry's run"
    );
    // 10 at 1 - 20 / 50 at the step's resolve, credited to the bolt
    // itself, which has no source.
    let hurt = parts_on(&parts, near);
    assert_eq!(hurt.len(), 1, "{hurt:?}");
    assert!((hurt[0].0 - 6.0).abs() < 1e-4, "{hurt:?}");
    assert_eq!((hurt[0].1, hurt[0].2), (Some(shot), 0));
    assert!((world.get::<crate::Health>(near).unwrap().current() - 94.0).abs() < 1e-4);
    // A World without chemistry: the end's blast traced as skipped.
    let (mut world, mut schedule) = blast_rig(false);
    let near = bystander(&mut world, Vec2::new(30.0, 20.0));
    let shot = bolt(&mut world, blasting(Projectile::new().with_range(30.0)));
    for _ in 0..6 {
        let (_, parts) = blast_step(&mut world, &mut schedule);
        assert!(parts_on(&parts, near).is_empty());
    }
    let skipped: Vec<_> = world
        .resource::<crate::foundation::trace::KitTrace>()
        .about(shot)
        .filter_map(|entry| match entry.kind {
            TraceKind::Skipped { event, action, why } => Some((event, action, why)),
            _ => None,
        })
        .collect();
    assert_eq!(
        skipped,
        [("expired", "blast", "the World has no ChemistryPlugin")]
    );
    assert_eq!(world.get::<crate::Health>(near).unwrap().current(), 100.0);
    // The load checks the blast's damage types and context tags; a save
    // keeps an end and leaves out none.
    let components = world.resource::<ComponentRegistry>();
    let good = json!({ "end": { "blast": { "radius": 64, "damage": 20 } } });
    assert!(components.check_in(PROJECTILE, &good, &world).is_ok());
    for bad in [
        json!({ "end": { "blast": { "radius": 64, "damage": { "damage.fire": 20 } } } }),
        json!({ "end": { "blast": { "radius": 64, "context": ["hit.blast"] } } }),
        json!({ "end": { "blast": { "radius": 0 } } }),
        json!({ "end": { "spawn": "smoke" } }),
        json!({ "end": {} }),
    ] {
        assert!(
            components.check_in(PROJECTILE, &bad, &world).is_err(),
            "{bad}"
        );
    }
    let grenade = blasting(Projectile::new());
    let saved = serde_json::to_value(&grenade).unwrap();
    assert!(saved.get("end").is_some());
    assert_eq!(
        serde_json::from_value::<Projectile>(saved).unwrap(),
        grenade
    );
    let plain = serde_json::to_value(Projectile::new()).unwrap();
    assert!(plain.get("end").is_none());
}

// 7.13c: a projectile's end from Rust.

#[test]
fn a_projectile_takes_an_end_from_rust() {
    let built = Projectile::new()
        .with_range(30.0)
        .with_end(crate::Blast::new(50.0).with_damage(10.0));
    assert_eq!(
        built,
        blasting(Projectile::new().with_range(30.0)),
        "the data form's end"
    );
    // It goes off as data's does: 10 at 1 − 20 / 50 in the step it ends,
    // credited to the bolt, queued or with `land` `now`.
    for now in [false, true] {
        let (mut world, mut schedule) = blast_rig(true);
        let near = bystander(&mut world, Vec2::new(30.0, 20.0));
        let mut end = crate::Blast::new(50.0).with_damage(10.0);
        if now {
            end = end.landing_now();
        }
        let shot = bolt(&mut world, Projectile::new().with_range(30.0).with_end(end));
        let mut landed_at = None;
        for step in 0..6 {
            let (expired, parts) = blast_step(&mut world, &mut schedule);
            if !expired.is_empty() {
                assert_eq!(expired[0].entity, shot);
            }
            if let [hit] = parts_on(&parts, near).as_slice() {
                assert!((hit.0 - 6.0).abs() < 1e-4, "{hit:?}");
                assert_eq!(hit.1, Some(shot));
                landed_at = Some(step);
            }
        }
        assert_eq!(landed_at, Some(2), "now: {now}, the step it ended");
        assert!((world.get::<crate::Health>(near).unwrap().current() - 94.0).abs() < 1e-4);
    }
}

// 7.13c's foes and cues, Step 1: an end's blast in its own step.

/// The tracked target's health as each step's projectiles are done, before
/// the step's resolve: a World resource of [`ending_rig`]'s.
#[derive(Default)]
struct Probe(Option<Entity>, Vec<f32>);

/// [`blast_rig`] with effects, the tag `creature` and a probe after the
/// projectiles; with `chemistry`, a rule dazing each creature a blast
/// reaches.
fn ending_rig(chemistry: bool) -> (World, Schedule) {
    use tungsten_core::physics::PhysicsPlugin;

    let mut world = World::new();
    world.insert_resource(tungsten_core::Time::new());
    world.insert_resource(tungsten_core::CommandBuffer::new());
    world.insert_resource(tungsten_core::TilemapRegistry::new());
    world.insert_resource(crate::foundation::trace::KitTrace::default());
    world.insert_resource(Probe::default());
    let mut schedule = Schedule::new();
    PhysicsPlugin.build(&mut schedule, &mut world);
    crate::TagsPlugin::new()
        .with_tags(["creature"])
        .build(&mut schedule, &mut world);
    crate::StatsPlugin::new().build(&mut schedule, &mut world);
    crate::HealthPlugin.build(&mut schedule, &mut world);
    crate::EffectsPlugin.build(&mut schedule, &mut world);
    crate::LinearMotionPlugin.build(&mut schedule, &mut world);
    crate::LifetimePlugin.build(&mut schedule, &mut world);
    if chemistry {
        crate::ChemistryPlugin::new().build(&mut schedule, &mut world);
    }
    ProjectilePlugin.build(&mut schedule, &mut world);
    schedule.add(
        Phase::Overlap.stage(),
        Phase::Overlap
            .join(system("probe", |world: &mut World| {
                let Some(tracked) = world.resource::<Probe>().0 else {
                    return;
                };
                let health = world.get::<crate::Health>(tracked).unwrap().current();
                world.resource_mut::<Probe>().1.push(health);
            }))
            .after(PROJECTILE_TICK),
    );
    schedule.resolve().unwrap();
    let data = world.resource_mut::<tungsten_core::DataRegistry>();
    data.insert("dazed", crate::Effect::new());
    if chemistry {
        let rule: crate::Interaction = serde_json::from_value(json!({
            "stimulus": "stimulus.blast", "on": { "all": ["creature"] },
            "outputs": [{ "apply": { "status": "dazed" } }]
        }))
        .unwrap();
        data.insert("daze", rule);
    }
    (world, schedule)
}

#[test]
fn an_end_blast_lands_in_its_own_step() {
    use tungsten_core::physics::RigidBodyBundle;

    // A bolt ending at a wall whose face is at x 95, its centre at x 93,
    // beside a goblin 30 px below: 10 at 1 - 30 / 50.
    for now in [false, true] {
        let (mut world, mut schedule) = ending_rig(true);
        world.spawn_with(RigidBodyBundle::r#static(
            Position::new(100.0, 0.0),
            Collider::aabb(Vec2::new(5.0, 50.0)),
        ));
        let goblin = world.spawn_with((
            tungsten_core::Transform::from_position(Vec2::new(93.0, 30.0)),
            crate::Health::new(100.0),
            crate::Tags::new(["creature"]),
        ));
        world.resource_mut::<Probe>().0 = Some(goblin);
        let mut end = crate::Blast::new(50.0).with_damage(10.0);
        if now {
            end = end.landing_now();
        }
        let shot = bolt(&mut world, Projectile::new().with_end(end));
        let mut ended = None;
        for step in 0..20 {
            let (expired, parts) = blast_step(&mut world, &mut schedule);
            let hurt = parts_on(&parts, goblin);
            if expired.iter().any(|expired| expired.entity == shot) {
                ended = Some((step, hurt));
                break;
            }
            assert!(hurt.is_empty(), "now {now}: nothing before its end");
        }
        let (step, hurt) = ended.expect("it ends at the wall");
        assert_eq!(hurt.len(), 1, "now {now}: in its own step: {hurt:?}");
        assert!((hurt[0].0 - 4.0).abs() < 1e-4, "now {now}: {hurt:?}");
        // Queued, the hit waits for the step's resolve; landing now, it is
        // in before the projectiles' step is over.
        let probed = world.resource::<Probe>().1[step];
        let want = if now { 96.0 } else { 100.0 };
        assert!(
            (probed - want).abs() < 1e-4,
            "now {now}: {probed} as the projectiles finish"
        );
        assert!(
            world
                .get::<crate::Effects>(goblin)
                .is_some_and(|effects| effects.has("dazed")),
            "now {now}: dazed by its stimulus at the step's run"
        );
    }
    // Without chemistry: its skip traced, nothing landed.
    let (mut world, mut schedule) = ending_rig(false);
    let goblin = world.spawn_with((
        tungsten_core::Transform::from_position(Vec2::new(30.0, 20.0)),
        crate::Health::new(100.0),
        crate::Tags::new(["creature"]),
    ));
    let shot = bolt(
        &mut world,
        Projectile::new()
            .with_range(30.0)
            .with_end(crate::Blast::new(50.0).with_damage(10.0).landing_now()),
    );
    for _ in 0..6 {
        blast_step(&mut world, &mut schedule);
    }
    let skipped = world
        .resource::<crate::foundation::trace::KitTrace>()
        .about(shot)
        .filter(|entry| {
            matches!(
                entry.kind,
                TraceKind::Skipped {
                    action: "blast",
                    ..
                }
            )
        })
        .count();
    assert_eq!(skipped, 1);
    assert_eq!(world.get::<crate::Health>(goblin).unwrap().current(), 100.0);
}

// 7.13c: a projectile's own shape, bodies as walls and named layers.

/// A kinematic box of half extents (5, 50) at (`x`, 40) in `layers`,
/// moving at `vx` px/s.
fn paddle(world: &mut World, x: f32, layers: u32, vx: f32) -> Entity {
    use tungsten_core::physics::{RigidBodyBundle, Velocity};

    let wall = world.spawn_with(RigidBodyBundle::kinematic(
        Position::new(x, 40.0),
        Collider::aabb(Vec2::new(5.0, 50.0)),
    ));
    world.get_mut::<RigidBody>(wall).unwrap().layers = layers;
    world.get_mut::<Velocity>(wall).unwrap().0 = Vec2::new(vx, 0.0);
    wall
}

/// A projectile at (`x`, 40) flying +x at 600 px/s, with `hitbox` or none.
fn shot(world: &mut World, x: f32, projectile: Projectile, hitbox: Option<Hitbox>) -> Entity {
    use tungsten_core::physics::PrevPosition;

    let at = Vec2::new(x, 40.0);
    let entity = world.spawn_with((
        Position(at),
        PrevPosition(at),
        crate::LinearMotion::new(Vec2::new(600.0, 0.0)),
        projectile,
    ));
    if let Some(hitbox) = hitbox {
        world.insert(entity, hitbox);
    }
    entity
}

/// Where `shot` ended, at the end of `meetings`.
fn ended_at(world: &mut World, schedule: &mut Schedule, bolt: Entity) -> Option<f32> {
    meetings(world, schedule, bolt, 1.0 / 60.0)
        .iter()
        .find(|meeting| meeting.0 == "expired")
        .map(|meeting| meeting.1)
}

#[test]
fn a_projectile_sweeps_its_own_shape() {
    let round = HitShape::circle(5.0);
    let offset = Vec2::new(8.0, 8.0);
    // A slab whose face is at x 95, then a paddle in its body walls: the
    // shape's centre meets each 5 px short of it, its point 13 px short.
    for body in [false, true] {
        let ends: Vec<Option<f32>> = (0..3)
            .map(|lane| {
                let (mut world, mut schedule) = wall_rig();
                if body {
                    paddle(&mut world, 100.0, 1 << 2, 0.0);
                } else {
                    slab(&mut world, 100.0, 1);
                }
                let projectile = Projectile::new().with_body_walls(1 << 2);
                let bolt = match lane {
                    0 => shot(&mut world, 0.0, projectile.with_shape(round, offset), None),
                    1 => shot(
                        &mut world,
                        0.0,
                        projectile,
                        Some(Hitbox::new(round, 0.0).with_offset(offset)),
                    ),
                    _ => shot(&mut world, 0.0, projectile, None),
                };
                ended_at(&mut world, &mut schedule, bolt)
            })
            .collect();
        let close = |got: Option<f32>, want: f32| got.is_some_and(|x| (x - want).abs() < 1e-3);
        assert!(close(ends[0], 82.0), "body {body}: its shape: {ends:?}");
        assert!(
            close(ends[1], 82.0),
            "body {body}: a hitbox of it: {ends:?}"
        );
        assert!(close(ends[2], 95.0), "body {body}: a point: {ends:?}");
    }
    // Its end's blast at the shape's centre: 10 at 1 - 30 / 50 on a
    // bystander 30 px below where the centre stopped, (90, 48).
    let (mut world, mut schedule) = blast_rig(true);
    slab(&mut world, 100.0, 1);
    let near = bystander(&mut world, Vec2::new(90.0, 78.0));
    let blasting = Projectile::new()
        .with_shape(round, offset)
        .with_end(crate::Blast::new(50.0).with_damage(10.0));
    shot(&mut world, 0.0, blasting, None);
    for _ in 0..30 {
        blast_step(&mut world, &mut schedule);
    }
    let health = world.get::<crate::Health>(near).unwrap().current();
    assert!((health - 96.0).abs() < 1e-3, "{health}");
}

#[test]
fn a_projectile_ends_at_a_body_in_its_body_walls() {
    // A bolt bouncing once at bodies in layer 2: a paddle closing at
    // 100 px/s turns it in its own frame, to 800 px/s; a body in layer 3
    // and its own source's in layer 2 pass.
    for walls in [1 << 2, 0] {
        let (mut world, mut schedule) = wall_rig();
        let owner = paddle(&mut world, 30.0, 1 << 2, 0.0);
        paddle(&mut world, 60.0, 1 << 3, 0.0);
        let bat = paddle(&mut world, 120.0, 1 << 2, -100.0);
        let bolt = flier(
            &mut world,
            0.0,
            600.0,
            Projectile::new().with_bounces(1).with_body_walls(walls),
        );
        world.insert(bolt, crate::Source::new(owner));
        let got = meetings(&mut world, &mut schedule, bolt, 1.0 / 60.0);
        let velocity = world.get::<crate::LinearMotion>(bolt).unwrap().velocity();
        if walls == 0 {
            assert!(got.is_empty(), "every body passes: {got:?}");
            assert_eq!(velocity, Vec2::new(600.0, 0.0));
            continue;
        }
        assert_eq!(got.len(), 1, "{got:?}");
        let (kind, _, wall) = got[0];
        assert_eq!(
            (
                kind,
                wall.map(|wall| (wall.entity, wall.normal, wall.layers))
            ),
            ("bounced", Some((Some(bat), Vec2::NEG_X, 1 << 2)))
        );
        assert!(
            (velocity - Vec2::new(-800.0, 0.0)).length() < 1e-2,
            "in the paddle's frame: {velocity}"
        );
    }
}

#[test]
fn body_walls_keep_their_order_with_hits_and_passes() {
    // A hedge it passes, a target it pierces, then a body in its body walls
    // that ends it: in sweep order.
    let (mut world, mut schedule) = wall_rig();
    let hedge = slab(&mut world, 60.0, 1 << 3);
    let target = world.spawn_with((
        Position::new(120.0, 40.0),
        tungsten_core::physics::PrevPosition(Vec2::new(120.0, 40.0)),
        Hurtbox::new(HitShape::circle(4.0)),
        crate::Health::new(100.0),
    ));
    let body = paddle(&mut world, 180.0, 1 << 2, 0.0);
    let bolt = flier(
        &mut world,
        0.0,
        1200.0,
        Projectile::new()
            .with_pierce(1)
            .with_pass_layers(1 << 3)
            .with_body_walls(1 << 2),
    );
    let got = meetings(&mut world, &mut schedule, bolt, 1.0 / 60.0);
    assert_eq!(got.len(), 2, "{got:?}");
    assert_eq!(
        (got[0].0, got[0].2.unwrap().entity),
        ("passed", Some(hedge))
    );
    assert_eq!(
        (got[1].0, got[1].2.and_then(|wall| wall.entity)),
        ("expired", Some(body))
    );
    assert!((got[1].1 - 173.0).abs() < 1e-3, "at its face: {got:?}");
    let struck: Vec<Entity> = world
        .resource::<EventQueue<Struck>>()
        .iter()
        .map(|event| event.target)
        .collect();
    assert_eq!(struck, [target], "the target between them");
    assert_eq!(world.get::<Projectile>(bolt).unwrap().hits(), 1);
}

#[test]
fn projectile_layers_take_names() {
    use tungsten_core::physics::PhysicsLayers;

    let mut world = World::new();
    world.insert_resource(PhysicsLayers::new([
        ("armour".to_string(), 2),
        ("hedge".to_string(), 3),
        ("crates".to_string(), 5),
    ]));
    let mut schedule = Schedule::new();
    ProjectilePlugin.build(&mut schedule, &mut world);
    let components = world.resource::<ComponentRegistry>();
    let good = json!({
        "bounce_layers": ["armour", 0], "pass_layers": ["hedge"], "body_walls": ["crates", 1]
    });
    assert!(components.check_in(PROJECTILE, &good, &world).is_ok());
    let err = format!(
        "{:?}",
        components
            .check_in(PROJECTILE, &json!({ "body_walls": ["glass"] }), &world)
            .unwrap_err()
    );
    assert!(err.contains("glass") && err.contains("body_walls"), "{err}");
    assert!(
        components
            .check_in(PROJECTILE, &json!({ "offset": [8, 8] }), &world)
            .is_err(),
        "an offset without its shape"
    );
    let def: ProjectileDef = serde_json::from_value(good).unwrap();
    let read = def.into_projectile(&world);
    assert_eq!(
        (read.bounce_layers(), read.pass_layers(), read.body_walls()),
        (0b101, 1 << 3, (1 << 5) | (1 << 1))
    );
    // A save keeps the masks, its body walls and its shape, and leaves
    // each out at its default.
    let shaped = read.with_shape(HitShape::circle(5.0), Vec2::new(8.0, 8.0));
    let saved = serde_json::to_value(&shaped).unwrap();
    assert_eq!(saved["body_walls"], json!(0b10_0010));
    assert_eq!(saved["offset"], json!([8.0, 8.0]));
    assert_eq!(serde_json::from_value::<Projectile>(saved).unwrap(), shaped);
    let plain = serde_json::to_value(Projectile::new()).unwrap();
    assert!(plain.get("body_walls").is_none() && plain.get("shape").is_none());
}
