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
    let read = Projectile::from(def);
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
