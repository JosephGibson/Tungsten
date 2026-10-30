use super::pairs::{
    EMPTY_PAIR, PAIR_AWAKE, PAIR_MARGIN_SLOPS, build_pairs, collect_tripped, gravity_allowance,
};
use super::*;
use crate::assets::tilemap::TerrainProps;
use crate::assets::{
    EMPTY_TILE, TileDamage, TileProps, TilemapData, TilemapLayer, TilemapRegistry,
};
use crate::assets::{LayerKind, TilemapInstance};
use crate::ecs::World;
use crate::physics::components::RigidBodyBundle;

fn seed_world() -> World {
    let mut world = World::new();
    let mut time = Time::new();
    time.advance_frame(1.0 / 60.0);
    world.insert_resource(time);
    world.insert_resource(EventQueue::<CollisionEvent>::new());
    world.insert_resource(EventQueue::<SensorEntered>::new());
    world.insert_resource(PhysicsConfig::default());
    world.insert_resource(TilemapRegistry::new());
    world
}

#[test]
fn integrates_dynamic_position_from_velocity() {
    let mut world = seed_world();
    let e = world.spawn();
    world.insert(e, Position(Vec2::new(0.0, 0.0)));
    world.insert(e, Velocity(Vec2::new(60.0, 0.0)));
    world.insert(e, Collider::aabb(Vec2::new(8.0, 8.0)));
    world.insert(e, RigidBody::dynamic());

    physics_step(&mut world);

    let pos = world.get::<Position>(e).unwrap();
    assert!((pos.0.x - 1.0).abs() < 1e-3, "got {:?}", pos.0);
}

#[test]
fn dynamic_aabb_resolves_against_static_aabb() {
    let mut world = seed_world();

    let dynamic = world.spawn();
    world.insert(dynamic, Position(Vec2::new(0.0, 0.0)));
    world.insert(dynamic, Velocity(Vec2::new(600.0, 0.0)));
    world.insert(dynamic, Collider::aabb(Vec2::new(8.0, 8.0)));
    world.insert(dynamic, RigidBody::dynamic());

    let wall = world.spawn();
    world.insert(wall, Position(Vec2::new(32.0, 0.0)));
    world.insert(wall, Collider::aabb(Vec2::new(8.0, 32.0)));
    world.insert(wall, RigidBody::r#static());

    if let Some(time) = world.get_resource_mut::<Time>() {
        time.advance_frame(0.1);
    }
    physics_step(&mut world);

    // Soft solver (D-063): approach velocity dies immediately; the residual
    // overlap recovers at the bias rate over subsequent steps instead of one
    // MTV push, settling at ~linear_slop.
    if let Some(time) = world.get_resource_mut::<Time>() {
        time.advance_frame(1.0 / 60.0);
    }
    for _ in 0..120 {
        physics_step(&mut world);
    }

    let pos = world.get::<Position>(dynamic).unwrap();
    assert!(pos.0.x + 8.0 <= 32.0 - 8.0 + 0.5, "penetrated: {:?}", pos.0);
    let events = world.get_resource::<EventQueue<CollisionEvent>>().unwrap();
    assert!(!events.is_empty(), "expected at least one collision event");
}

#[test]
fn tilemap_collision_layer_blocks_dynamic_body() {
    let mut world = seed_world();

    let registry = world.get_resource_mut::<TilemapRegistry>().unwrap();
    registry.insert(
        "test".into(),
        TilemapData {
            tile_width: 16,
            tile_height: 16,
            width: 3,
            height: 1,
            tileset: vec!["solid".into()],
            object_layers: Vec::new(),
            terrain: TerrainProps::default(),
            layers: vec![TilemapLayer {
                name: "solid".into(),
                kind: LayerKind::Collision,
                tiles: vec![-1, -1, 0],
            }],
        },
    );

    let map_e = world.spawn();
    world.insert(map_e, TilemapInstance::new("test", Vec2::ZERO));

    let player = world.spawn();
    world.insert(player, Position(Vec2::new(8.0 + 7.0, 8.0)));
    world.insert(player, Velocity(Vec2::new(600.0, 0.0)));
    world.insert(player, Collider::aabb(Vec2::new(7.0, 7.0)));
    world.insert(player, RigidBody::dynamic());

    if let Some(time) = world.get_resource_mut::<Time>() {
        time.advance_frame(0.05);
    }
    // First step kills the approach; the soft bias then recovers the
    // residual overlap toward linear_slop over subsequent steps (D-063).
    for _ in 0..30 {
        physics_step(&mut world);
    }

    let pos = world.get::<Position>(player).unwrap();
    // Solid tile x=[32,48]; player center <= 25 plus recovering overlap.
    assert!(pos.0.x <= 25.0 + 1.0, "penetrated tile: {:?}", pos.0);
    let events = world.get_resource::<EventQueue<CollisionEvent>>().unwrap();
    assert!(events.iter_any_tile(), "expected a tile collision event");
}

#[test]
fn circle_against_static_aabb_pushes_out() {
    let mut world = seed_world();

    let circle = world.spawn();
    world.insert(circle, Position(Vec2::new(0.0, 0.0)));
    world.insert(circle, Velocity(Vec2::new(-200.0, 0.0)));
    world.insert(circle, Collider::circle(4.0));
    world.insert(circle, RigidBody::dynamic());

    let wall = world.spawn();
    world.insert(wall, Position(Vec2::new(-8.0, 0.0)));
    world.insert(wall, Collider::aabb(Vec2::new(4.0, 16.0)));
    world.insert(wall, RigidBody::r#static());

    // Soft solver: push-out is bias-rate-limited and rests at ~linear_slop
    // instead of resolving in one MTV step (D-063).
    for _ in 0..90 {
        physics_step(&mut world);
    }

    let pos = world.get::<Position>(circle).unwrap();
    assert!(pos.0.x >= -0.5, "penetrated wall: {:?}", pos.0);
    assert!(pos.0.x <= 1.0, "overshot push-out: {:?}", pos.0);
}

#[test]
fn fast_body_stops_at_wall_under_fixed_substeps() {
    // Speculative contacts (D-064): 2000 px/s at dt 1/30 travels ~16.7 px per
    // fixed substep — well past the 4 px half-extent the old heuristic keyed
    // on. The gap contact clamps arrival at touching (rest ~linear_slop).
    let mut world = seed_world();
    if let Some(time) = world.get_resource_mut::<Time>() {
        time.advance_frame(1.0 / 30.0);
    }
    let dynamic = world.spawn();
    world.insert(dynamic, Position(Vec2::new(0.0, 0.0)));
    world.insert(dynamic, Velocity(Vec2::new(2000.0, 0.0)));
    world.insert(dynamic, Collider::aabb(Vec2::new(4.0, 4.0)));
    world.insert(dynamic, RigidBody::dynamic());

    let wall = world.spawn();
    world.insert(wall, Position(Vec2::new(40.0, 0.0)));
    world.insert(wall, Collider::aabb(Vec2::new(4.0, 32.0)));
    world.insert(wall, RigidBody::r#static());

    physics_step(&mut world);

    let pos = world.get::<Position>(dynamic).unwrap();
    assert!(pos.0.x + 4.0 <= 40.0 - 4.0 + 0.5, "tunneled: {:?}", pos.0);
}

#[test]
fn inflated_broadphase_catches_resolution_slip_into_unpaired_wall() {
    // Regression: GS slip crosses cell boundary before re-pairing; half-cell margin catches it.
    let mut world = seed_world();
    if let Some(cfg) = world.get_resource_mut::<PhysicsConfig>() {
        cfg.broadphase_cell_size = 16.0;
        cfg.substeps = 1;
    }

    // Wall x=[18,22], cell 1.
    let wall = world.spawn();
    world.insert(wall, Position(Vec2::new(20.0, 0.0)));
    world.insert(wall, Collider::aabb(Vec2::new(2.0, 8.0)));
    world.insert(wall, RigidBody::r#static());

    // Heavy shover travels 500/60 px in one capped substep.
    let shover = world.spawn();
    world.insert(shover, Position(Vec2::new(0.0, 0.0)));
    world.insert(shover, Velocity(Vec2::new(500.0, 0.0)));
    world.insert(shover, Collider::circle(4.0));
    world.insert(
        shover,
        RigidBody {
            kind: BodyKind::Dynamic,
            inv_mass: 0.1,
            restitution: 0.0,
            ..RigidBody::dynamic()
        },
    );

    // Target ball x=[6,14], cell 0.
    let ball = world.spawn();
    world.insert(ball, Position(Vec2::new(10.0, 0.0)));
    world.insert(ball, Velocity(Vec2::ZERO));
    world.insert(ball, Collider::circle(4.0));
    world.insert(ball, RigidBody::dynamic().with_restitution(0.0));

    if let Some(time) = world.get_resource_mut::<Time>() {
        time.advance_frame(1.0 / 60.0);
    }
    // The heavy shover squeezes the ball against the wall over many steps.
    // The soft solver tolerates a few px of transient overlap, but the ball
    // must never cross the wall centerline x=20 — past it, the narrow-phase
    // normal flips and ejects it out the far side.
    for step in 0..30 {
        physics_step(&mut world);
        let ball_pos = world.get::<Position>(ball).unwrap().0;
        assert!(
            ball_pos.x < 20.0,
            "ball slipped through wall at step {step}: {ball_pos:?}"
        );
    }
}

#[test]
fn speculative_contact_stops_extreme_bullet_in_one_substep() {
    // 4000 px/s at one 1/30 s substep travels ~133 px against an 8 px-thick
    // wall. The positive-gap contact (D-064) clamps arrival at touching and
    // the stored-approach restitution still reflects at e=0.5.
    let mut world = seed_world();
    if let Some(cfg) = world.get_resource_mut::<PhysicsConfig>() {
        cfg.substeps = 1;
    }
    if let Some(time) = world.get_resource_mut::<Time>() {
        time.advance_frame(1.0 / 30.0);
    }

    let ball = world.spawn();
    world.insert(ball, Position(Vec2::new(0.0, 0.0)));
    world.insert(ball, Velocity(Vec2::new(4000.0, 0.0)));
    world.insert(ball, Collider::circle(4.0));
    world.insert(ball, RigidBody::dynamic().with_restitution(0.5));

    let wall = world.spawn();
    world.insert(wall, Position(Vec2::new(40.0, 0.0)));
    world.insert(wall, Collider::aabb(Vec2::new(4.0, 32.0)));
    world.insert(wall, RigidBody::r#static());

    physics_step(&mut world);

    let pos = world.get::<Position>(ball).unwrap();
    // Wall left face x=36; ball must not clear right face x=44.
    assert!(
        pos.0.x <= 36.0 + 0.5,
        "ball tunneled through wall despite speculative contact: {:?}",
        pos.0
    );
    let vel = world.get::<Velocity>(ball).unwrap().0;
    assert!(
        vel.x < 0.0,
        "velocity should reflect off wall under speculative restitution: {vel:?}"
    );
}

/// Diagonal shover rig: a massive body moving +x hits the origin target
/// through a 45-degree contact normal, injecting ~(15k, 15k) px/s the pair
/// admission never saw (the shover's own path stays clear of the downrange
/// static). Returns the launched target entity.
fn spawn_diagonal_shover_rig(world: &mut World) -> Entity {
    if let Some(cfg) = world.get_resource_mut::<PhysicsConfig>() {
        cfg.substeps = 1;
    }

    let target = world.spawn();
    world.insert(target, Position(Vec2::new(0.0, 0.0)));
    world.insert(target, Velocity(Vec2::ZERO));
    world.insert(target, Collider::circle(4.0));
    world.insert(target, RigidBody::dynamic());

    // Center distance 9 at 45 degrees: 1 px gap, admitted speculatively.
    let shover = world.spawn();
    world.insert(shover, Position(Vec2::new(-6.364, -6.364)));
    world.insert(shover, Velocity(Vec2::new(30_000.0, 0.0)));
    world.insert(shover, Collider::circle(4.0));
    world.insert(shover, RigidBody::dynamic().with_mass(1_000.0));

    target
}

#[test]
fn sweep_net_catches_solver_injected_velocity_through_static_wall() {
    // The speculative pair admission sees pre-solve velocities; velocity
    // injected mid-substep by the solver sends the target through a slab it
    // was never paired with. The slab-sweep safety net must clamp it.
    let mut world = seed_world();
    let target = spawn_diagonal_shover_rig(&mut world);

    // Thin floor slab across the target's diagonal path (top face y=54),
    // outside the shover's horizontal travel band.
    let slab = world.spawn();
    world.insert(slab, Position(Vec2::new(250.0, 56.0)));
    world.insert(slab, Collider::aabb(Vec2::new(250.0, 2.0)));
    world.insert(slab, RigidBody::r#static());

    physics_step(&mut world);

    let pos = world.get::<Position>(target).unwrap();
    assert!(
        pos.0.y + 4.0 <= 54.0 + 0.5,
        "solver-injected velocity tunneled the static slab: {:?}",
        pos.0
    );
}

#[test]
fn sweep_net_covers_static_circles_via_bounding_square() {
    // Same solver-injected velocity spike, but the downrange static is a
    // circle: the safety net promotes it to its bounding square (D-064).
    let mut world = seed_world();
    let target = spawn_diagonal_shover_rig(&mut world);

    // Static circle centered on the diagonal path; bounding square [46,62]^2.
    let pillar = world.spawn();
    world.insert(pillar, Position(Vec2::new(54.0, 54.0)));
    world.insert(pillar, Collider::circle(8.0));
    world.insert(pillar, RigidBody::r#static());

    physics_step(&mut world);

    let pos = world.get::<Position>(target).unwrap();
    // Bounding-square near faces at 46; the target must stop at or before
    // them instead of passing through the pillar center.
    assert!(
        pos.0.x + 4.0 <= 46.0 + 0.5 || pos.0.y + 4.0 <= 46.0 + 0.5,
        "solver-injected velocity tunneled the static circle: {:?}",
        pos.0
    );
}

#[test]
fn sweep_net_restages_statics_when_the_proxy_set_changes() {
    // The sweep grid is staged per frame (D-080). After a static despawns and
    // two spawn, proxy indices shift: the old slab must stop clamping and the
    // new one must clamp.
    let mut world = seed_world();
    world.get_resource_mut::<PhysicsConfig>().unwrap().substeps = 1;
    // Three stray bullets sweep first every frame, so the statics-only grid
    // is staged before the target's query.
    for i in 0..3 {
        let bullet = world.spawn();
        world.insert(bullet, Position(Vec2::new(i as f32 * 100.0, -2_000.0)));
        world.insert(bullet, Velocity(Vec2::new(30_000.0, 0.0)));
        world.insert(bullet, Collider::circle(4.0));
        world.insert(bullet, RigidBody::dynamic());
    }
    let target = world.spawn();
    world.insert(target, Position(Vec2::ZERO));
    world.insert(target, Velocity(Vec2::ZERO));
    world.insert(target, Collider::circle(4.0));
    world.insert(target, RigidBody::dynamic());
    let shover = world.spawn();
    world.insert(shover, Position(Vec2::ZERO));
    world.insert(shover, Velocity(Vec2::ZERO));
    world.insert(shover, Collider::circle(4.0));
    world.insert(shover, RigidBody::dynamic().with_mass(1_000.0));
    // Same geometry as `spawn_diagonal_shover_rig`, re-armed every frame.
    let arm = |world: &mut World| {
        world.get_mut::<Position>(target).unwrap().0 = Vec2::ZERO;
        world.get_mut::<Velocity>(target).unwrap().0 = Vec2::ZERO;
        world.get_mut::<Position>(shover).unwrap().0 = Vec2::new(-6.364, -6.364);
        world.get_mut::<Velocity>(shover).unwrap().0 = Vec2::new(30_000.0, 0.0);
    };
    let spawn_static = |world: &mut World, center: Vec2, half: Vec2| {
        let e = world.spawn();
        world.insert(e, Position(center));
        world.insert(e, Collider::aabb(half));
        world.insert(e, RigidBody::r#static());
        e
    };

    let staged = |world: &World| {
        let buffers = world.get_resource::<PhysicsBuffers>().unwrap();
        buffers.static_grid.staged
    };

    let near = spawn_static(&mut world, Vec2::new(250.0, 56.0), Vec2::new(250.0, 2.0));
    arm(&mut world);
    physics_step(&mut world);
    assert!(staged(&world), "frame 1 never used the statics-only grid");
    let y = world.get::<Position>(target).unwrap().0.y;
    assert!(y + 4.0 <= 54.5, "near slab did not clamp: y={y}");

    world.despawn(near);
    spawn_static(&mut world, Vec2::splat(-5_000.0), Vec2::splat(4.0));
    spawn_static(&mut world, Vec2::new(250.0, 156.0), Vec2::new(250.0, 2.0));
    arm(&mut world);
    physics_step(&mut world);
    assert!(staged(&world), "frame 2 never used the statics-only grid");
    let y = world.get::<Position>(target).unwrap().0.y;
    assert!(y + 4.0 > 60.0, "despawned slab still clamps: y={y}");
    assert!(y + 4.0 <= 154.5, "new slab did not clamp: y={y}");
}

#[test]
fn sweep_rents_the_pair_grid_until_queries_reach_the_static_count() {
    // D-080's rent-or-buy rule: with three statics, a frame's first three
    // sweep queries go to the pair grid and the fourth stages the
    // statics-only grid. A frame with fewer queries never stages it.
    let mut world = seed_world();
    world.get_resource_mut::<PhysicsConfig>().unwrap().substeps = 1;
    for i in 0..3 {
        let wall = world.spawn();
        world.insert(wall, Position(Vec2::new(-1_000.0, i as f32 * 100.0)));
        world.insert(wall, Collider::aabb(Vec2::splat(4.0)));
        world.insert(wall, RigidBody::r#static());
    }
    let spawn_bullet = |world: &mut World, y: f32| {
        let bullet = world.spawn();
        world.insert(bullet, Position(Vec2::new(0.0, y)));
        world.insert(bullet, Velocity(Vec2::new(30_000.0, 0.0)));
        world.insert(bullet, Collider::circle(4.0));
        world.insert(bullet, RigidBody::dynamic());
    };
    let sweep_state = |world: &World| {
        let sweep = &world.get_resource::<PhysicsBuffers>().unwrap().static_grid;
        (sweep.statics, sweep.pair_queries, sweep.staged)
    };

    spawn_bullet(&mut world, 0.0);
    spawn_bullet(&mut world, 100.0);
    physics_step(&mut world);
    assert_eq!(sweep_state(&world), (3, 2, false));

    spawn_bullet(&mut world, 200.0);
    spawn_bullet(&mut world, 300.0);
    physics_step(&mut world);
    assert_eq!(sweep_state(&world), (3, 3, true));
}

#[test]
fn sweep_grid_returns_the_pair_grids_statics_in_order() {
    // D-080: for any query the statics-only grid returns the static
    // candidates the pair grid returns, in the same order, so the sweep's
    // first hit cannot change. Covers tiles, body-less colliders, sleepers
    // (staged in the pair grid, never a sweep target) and three cell sizes.
    use crate::Pcg32;

    for (seed, cell) in [8.0, 16.0, 32.0].into_iter().enumerate() {
        let mut rng = Pcg32::seeded(0xD080 + seed as u64);
        let mut world = seed_world();
        world
            .get_resource_mut::<PhysicsConfig>()
            .unwrap()
            .broadphase_cell_size = cell;
        world.get_resource_mut::<TilemapRegistry>().unwrap().insert(
            "tiles".into(),
            TilemapData {
                tile_width: 16,
                tile_height: 16,
                width: 12,
                height: 2,
                tileset: vec!["solid".into()],
                object_layers: Vec::new(),
                terrain: TerrainProps::default(),
                layers: vec![TilemapLayer {
                    name: "collision".into(),
                    kind: LayerKind::Collision,
                    tiles: (0..24).map(|i| if i % 5 == 0 { -1 } else { 0 }).collect(),
                }],
            },
        );
        let map = world.spawn();
        world.insert(map, TilemapInstance::new("tiles", Vec2::new(-96.0, 40.0)));
        for i in 0..60 {
            let e = world.spawn();
            world.insert(
                e,
                Position(Vec2::new(
                    rng.next_range(-120.0, 120.0),
                    rng.next_range(-80.0, 80.0),
                )),
            );
            world.insert(
                e,
                if i % 2 == 0 {
                    Collider::aabb(Vec2::new(
                        rng.next_range(1.0, 30.0),
                        rng.next_range(1.0, 30.0),
                    ))
                } else {
                    Collider::circle(rng.next_range(1.0, 20.0))
                },
            );
            match i % 3 {
                0 => world.insert(e, RigidBody::r#static()),
                1 => {
                    world.insert(
                        e,
                        Velocity(rng.next_unit_vec2() * rng.next_range(0.0, 400.0)),
                    );
                    world.insert(e, RigidBody::dynamic());
                }
                _ => {}
            }
        }
        let config = *world.get_resource::<PhysicsConfig>().unwrap();
        let mut buffers = PhysicsBuffers::default();
        gather_proxies(
            &world,
            &mut buffers.proxies,
            &mut buffers.layers,
            &mut buffers.sensors,
            &mut buffers.tile_ids,
            &mut buffers.tiles,
        );
        buffers.bodies = buffers.proxies.len() - buffers.tiles.len();
        for proxy in buffers.proxies.iter_mut().step_by(4) {
            proxy.sleeping = proxy.is_dynamic;
        }
        build_pairs(&config, 1.0 / 240.0, 1.0 / 60.0, &mut buffers);
        buffers.static_grid.stage(&config, &buffers.proxies);

        let mut from_pair_grid = Vec::new();
        let mut from_static_grid = Vec::new();
        let mut returned = 0;
        for _ in 0..200 {
            let probe = Aabb::new(
                Vec2::new(rng.next_range(-140.0, 140.0), rng.next_range(-100.0, 100.0)),
                Vec2::new(rng.next_range(0.0, 40.0), rng.next_range(0.0, 40.0)),
            );
            buffers.grid.query(&probe, None, &mut from_pair_grid);
            from_pair_grid.retain(|&id| !buffers.proxies[id as usize].is_dynamic);
            buffers
                .static_grid
                .grid
                .query(&probe, None, &mut from_static_grid);
            assert_eq!(from_static_grid, from_pair_grid, "cell {cell}, {probe:?}");
            returned += from_static_grid.len();
        }
        assert!(returned > 400, "probes met too few statics: {returned}");
    }
}

#[test]
fn speculative_gap_contact_emits_no_event_until_touch() {
    // Event gate (D-064): a positive-gap speculative contact enters the
    // solver but must not emit a CollisionEvent until actually penetrating.
    let mut world = seed_world();

    // 240 px/s = 1 px per default substep; start 3.5 px from touching so the
    // whole first frame stays separated while the last substeps admit the
    // gap contact.
    let ball = world.spawn();
    world.insert(ball, Position(Vec2::new(32.5, 0.0)));
    world.insert(ball, Velocity(Vec2::new(240.0, 0.0)));
    world.insert(ball, Collider::circle(4.0));
    world.insert(ball, RigidBody::dynamic());

    let wall = world.spawn();
    world.insert(wall, Position(Vec2::new(44.0, 0.0)));
    world.insert(wall, Collider::aabb(Vec2::new(4.0, 32.0)));
    world.insert(wall, RigidBody::r#static());

    physics_step(&mut world);
    {
        let events = world.get_resource::<EventQueue<CollisionEvent>>().unwrap();
        assert!(
            !events.iter_current().any(|e| e.a == ball),
            "speculative gap contact emitted an event before touch"
        );
    }
    world
        .get_resource_mut::<EventQueue<CollisionEvent>>()
        .unwrap()
        .flush();

    physics_step(&mut world);
    let events = world.get_resource::<EventQueue<CollisionEvent>>().unwrap();
    assert!(
        events
            .iter_current()
            .any(|e| e.a == ball && e.penetration > 0.0),
        "touching contact must emit with positive penetration"
    );
}

#[test]
fn zero_restitution_body_does_not_bounce_off_multi_tile_floor() {
    // Regression: multi-tile contact must not sum duplicate impulses.
    let mut world = seed_world();
    world.get_resource_mut::<TilemapRegistry>().unwrap().insert(
        "floor".into(),
        TilemapData {
            tile_width: 16,
            tile_height: 16,
            width: 4,
            height: 1,
            tileset: vec!["solid".into()],
            object_layers: Vec::new(),
            terrain: TerrainProps::default(),
            layers: vec![TilemapLayer {
                name: "collision".into(),
                kind: LayerKind::Collision,
                tiles: vec![0, 0, 0, 0],
            }],
        },
    );
    let map = world.spawn();
    world.insert(map, TilemapInstance::new("floor", Vec2::new(0.0, 16.0)));

    let player = world.spawn();
    world.insert(player, Position(Vec2::new(16.0, 9.0)));
    world.insert(player, Velocity(Vec2::new(0.0, 50.0)));
    world.insert(player, Collider::aabb(Vec2::new(6.0, 7.0)));
    world.insert(player, RigidBody::dynamic().with_restitution(0.0));

    // Contacts are measured pre-solve, so the impact lands on step 2; run a
    // few steps so the multi-tile contact actually resolves.
    for _ in 0..3 {
        physics_step(&mut world);
        let vel = world.get::<Velocity>(player).unwrap().0;
        assert!(
            vel.y >= -1e-3,
            "zero-restitution body bounced upward off flat floor: {vel:?}"
        );
    }
}

#[test]
fn bouncy_ball_does_not_double_impulse_on_multi_tile_seam() {
    // Regression: restitution applies once across tile seam contacts.
    let mut world = seed_world();
    world.get_resource_mut::<TilemapRegistry>().unwrap().insert(
        "floor".into(),
        TilemapData {
            tile_width: 16,
            tile_height: 16,
            width: 4,
            height: 1,
            tileset: vec!["solid".into()],
            object_layers: Vec::new(),
            terrain: TerrainProps::default(),
            layers: vec![TilemapLayer {
                name: "collision".into(),
                kind: LayerKind::Collision,
                tiles: vec![0, 0, 0, 0],
            }],
        },
    );
    let map = world.spawn();
    world.insert(map, TilemapInstance::new("floor", Vec2::new(0.0, 16.0)));

    let ball = world.spawn();
    world.insert(ball, Position(Vec2::new(16.0, 9.0)));
    world.insert(ball, Velocity(Vec2::new(0.0, 50.0)));
    world.insert(ball, Collider::aabb(Vec2::new(6.0, 6.0)));
    world.insert(ball, RigidBody::dynamic().with_restitution(0.85));

    // Contacts are measured pre-solve, so the impact lands on step 2.
    for _ in 0..3 {
        physics_step(&mut world);
    }

    let vel = world.get::<Velocity>(ball).unwrap().0;
    // Bound catches old 2x-3x seam amplification.
    assert!(
        vel.y > -60.0,
        "ball impulse was doubled — rebounded too fast: {vel:?}"
    );
    // The restitution pass must still fire once: stored approach ~-50 px/s
    // at e=0.85 rebounds near -42 px/s.
    assert!(
        vel.y < -20.0,
        "restitution was lost across the tile seam: {vel:?}"
    );
}

#[test]
fn stack_of_dynamic_bodies_does_not_tunnel_static_floor() {
    // Regression: multi-iteration GS propagates stack pressure to floor contacts.
    let mut world = seed_world();
    if let Some(cfg) = world.get_resource_mut::<PhysicsConfig>() {
        cfg.gravity = Vec2::new(0.0, 900.0);
    }

    let floor = world.spawn();
    world.insert(floor, Position(Vec2::new(64.0, 108.0)));
    world.insert(floor, Collider::aabb(Vec2::new(64.0, 8.0)));
    world.insert(floor, RigidBody::r#static());

    // Pre-separated stack above floor.
    const BALLS: u32 = 4;
    const RADIUS: f32 = 4.0;
    let mut ball_entities = Vec::new();
    for i in 0..BALLS {
        let e = world.spawn();
        let y = 100.0 - 8.0 - RADIUS - (i as f32) * (RADIUS * 2.0 + 0.5);
        world.insert(e, Position(Vec2::new(64.0, y)));
        world.insert(e, Velocity(Vec2::ZERO));
        world.insert(e, Collider::circle(RADIUS));
        world.insert(e, RigidBody::dynamic().with_restitution(0.3));
        ball_entities.push(e);
    }

    for _ in 0..120 {
        physics_step(&mut world);
        world
            .get_resource_mut::<EventQueue<CollisionEvent>>()
            .unwrap()
            .flush();
    }

    // Floor top y=100; ball top <= 100 plus rest tolerance. The soft solver
    // rests at linear_slop plus a small load-dependent sink for the bottom
    // of a column (D-063); step 3's fixed substeps lift the hertz cap and
    // tighten this again.
    let floor_top = 100.0;
    for (i, &ball) in ball_entities.iter().enumerate() {
        let pos = world.get::<Position>(ball).unwrap().0;
        assert!(
            pos.y + RADIUS <= floor_top + 1.0,
            "ball {i} clipped through floor: y={} (top={})",
            pos.y,
            pos.y + RADIUS
        );
    }
}

#[test]
fn pile_of_balls_does_not_escape_bottom_right_corner() {
    // Regression: pile pressure at L-corner seam must not escape tilemap.
    let mut world = seed_world();
    if let Some(cfg) = world.get_resource_mut::<PhysicsConfig>() {
        cfg.gravity = Vec2::new(0.0, 900.0);
        cfg.broadphase_cell_size = 16.0;
    }

    const W: u32 = 48;
    const H: u32 = 18;
    let mut tiles = vec![-1i32; (W * H) as usize];
    for row in 0..H {
        for col in 0..W {
            let solid = col == 0 || col == W - 1 || row == H - 2 || row == H - 1;
            if solid {
                tiles[(row * W + col) as usize] = 0;
            }
        }
    }
    world.get_resource_mut::<TilemapRegistry>().unwrap().insert(
        "level".into(),
        TilemapData {
            tile_width: 16,
            tile_height: 16,
            width: W,
            height: H,
            tileset: vec!["solid".into()],
            object_layers: Vec::new(),
            terrain: TerrainProps::default(),
            layers: vec![TilemapLayer {
                name: "collision".into(),
                kind: LayerKind::Collision,
                tiles,
            }],
        },
    );
    let map = world.spawn();
    world.insert(map, TilemapInstance::new("level", Vec2::ZERO));

    // Dense pile above bottom-right inside corner.
    const RADIUS: f32 = 6.0;
    const COLS: u32 = 6;
    const ROWS: u32 = 8;
    let wall_inner_x = (W - 1) as f32 * 16.0;
    let floor_top_y = (H - 2) as f32 * 16.0;
    let mut ball_entities = Vec::new();
    for row in 0..ROWS {
        for col in 0..COLS {
            let x = wall_inner_x - RADIUS - (col as f32) * (RADIUS * 2.0 + 0.25);
            let y = floor_top_y - RADIUS - (row as f32) * (RADIUS * 2.0 + 0.25);
            let e = world.spawn();
            world.insert(e, Position(Vec2::new(x, y)));
            world.insert(e, Velocity(Vec2::ZERO));
            world.insert(e, Collider::circle(RADIUS));
            world.insert(e, RigidBody::dynamic().with_restitution(0.85));
            ball_entities.push(e);
        }
    }

    for _ in 0..240 {
        physics_step(&mut world);
        world
            .get_resource_mut::<EventQueue<CollisionEvent>>()
            .unwrap()
            .flush();
    }

    for (i, &ball) in ball_entities.iter().enumerate() {
        let pos = world.get::<Position>(ball).unwrap().0;
        assert!(
            pos.x <= wall_inner_x + 0.5,
            "ball {i} escaped past right wall (x = {}, wall = {}), full pos {:?}",
            pos.x,
            wall_inner_x,
            pos,
        );
        assert!(
            pos.y <= floor_top_y + 0.5,
            "ball {i} escaped below floor (y = {}, floor = {})",
            pos.y,
            floor_top_y,
        );
    }
}

#[test]
fn settled_bodies_calm_below_jitter_floor() {
    // D-063: warm-started accumulated impulses + soft bias + the inelastic
    // restitution threshold must collapse the old solver's jitter floor
    // (~97 px/s at pile scale) to near-zero rest speeds — the prerequisite
    // for the sleep criterion (D-065).
    let mut world = seed_world();
    if let Some(cfg) = world.get_resource_mut::<PhysicsConfig>() {
        cfg.gravity = Vec2::new(0.0, 900.0);
        // Sleeping would zero velocities and mask the raw jitter floor this
        // test pins (D-063); measure with it disabled.
        cfg.sleep_threshold = 0.0;
    }

    let floor = world.spawn();
    world.insert(floor, Position(Vec2::new(0.0, 110.0)));
    world.insert(floor, Collider::aabb(Vec2::new(64.0, 8.0)));
    world.insert(floor, RigidBody::r#static());

    // Small column: floor top y=102, balls stacked with slim gaps.
    const RADIUS: f32 = 6.0;
    let mut balls = Vec::new();
    for i in 0..3 {
        let e = world.spawn();
        let y = 102.0 - RADIUS - (i as f32) * (RADIUS * 2.0 + 0.5);
        world.insert(e, Position(Vec2::new(0.0, y)));
        world.insert(e, Velocity(Vec2::ZERO));
        world.insert(e, Collider::circle(RADIUS));
        world.insert(e, RigidBody::dynamic().with_restitution(0.1));
        balls.push(e);
    }

    for _ in 0..180 {
        physics_step(&mut world);
        world
            .get_resource_mut::<EventQueue<CollisionEvent>>()
            .unwrap()
            .flush();
    }

    for (i, &ball) in balls.iter().enumerate() {
        let speed = world.get::<Velocity>(ball).unwrap().0.length();
        assert!(
            speed < 2.0,
            "ball {i} still jitters after settling: {speed} px/s"
        );
    }
}

#[test]
fn resting_contact_emits_until_asleep_and_resumes_on_wake() {
    // Event semantics (D-065, Box2D precedent): a resting contact keeps
    // emitting an event every step *while awake* (pre-solve contact set,
    // D-063 unchanged), goes silent once its island sleeps (sleeping bodies
    // never enter the narrow phase), and resumes emitting when woken.
    let mut world = seed_world();
    if let Some(cfg) = world.get_resource_mut::<PhysicsConfig>() {
        cfg.gravity = Vec2::new(0.0, 900.0);
    }

    let floor = world.spawn();
    world.insert(floor, Position(Vec2::new(0.0, 20.0)));
    world.insert(floor, Collider::aabb(Vec2::new(64.0, 8.0)));
    world.insert(floor, RigidBody::r#static());

    let ball = world.spawn();
    world.insert(ball, Position(Vec2::new(0.0, 9.0)));
    world.insert(ball, Velocity(Vec2::ZERO));
    world.insert(ball, Collider::circle(4.0));
    world.insert(ball, RigidBody::dynamic());

    // Land; rest penetration stays ~linear_slop, keeping the contact alive.
    for _ in 0..10 {
        physics_step(&mut world);
        world
            .get_resource_mut::<EventQueue<CollisionEvent>>()
            .unwrap()
            .flush();
    }

    // Awake resting contact: emits every step (still inside the 0.5 s
    // sleep window).
    for step in 0..5 {
        physics_step(&mut world);
        {
            let events = world.get_resource::<EventQueue<CollisionEvent>>().unwrap();
            assert!(
                events
                    .iter_current()
                    .any(|e| e.a == ball && e.penetration > 0.0),
                "awake resting contact stopped emitting events at step {step}"
            );
        }
        world
            .get_resource_mut::<EventQueue<CollisionEvent>>()
            .unwrap()
            .flush();
    }

    // Past time_to_sleep: the island sleeps and emission stops.
    for _ in 0..60 {
        physics_step(&mut world);
        world
            .get_resource_mut::<EventQueue<CollisionEvent>>()
            .unwrap()
            .flush();
    }
    assert!(
        world
            .get_resource::<PhysicsBuffers>()
            .unwrap()
            .is_sleeping(ball),
        "resting ball never fell asleep"
    );
    for step in 0..5 {
        physics_step(&mut world);
        {
            let events = world.get_resource::<EventQueue<CollisionEvent>>().unwrap();
            assert!(
                !events.iter_current().any(|e| e.a == ball),
                "sleeping resting contact emitted an event at step {step}"
            );
        }
        world
            .get_resource_mut::<EventQueue<CollisionEvent>>()
            .unwrap()
            .flush();
    }

    // Waking resumes emission on the next step.
    wake(&mut world, ball);
    physics_step(&mut world);
    let events = world.get_resource::<EventQueue<CollisionEvent>>().unwrap();
    assert!(
        events
            .iter_current()
            .any(|e| e.a == ball && e.penetration > 0.0),
        "woken resting contact did not resume emitting events"
    );
}

/// Spawn the shared sleep rig: static floor (top y=102) with a two-ball
/// column resting on it; settles and sleeps well within 150 steps.
fn spawn_sleeping_stack(world: &mut World) -> (Entity, Entity) {
    if let Some(cfg) = world.get_resource_mut::<PhysicsConfig>() {
        cfg.gravity = Vec2::new(0.0, 900.0);
    }

    let floor = world.spawn();
    world.insert(floor, Position(Vec2::new(0.0, 110.0)));
    world.insert(floor, Collider::aabb(Vec2::new(64.0, 8.0)));
    world.insert(floor, RigidBody::r#static());

    const RADIUS: f32 = 6.0;
    let mut balls = Vec::with_capacity(2);
    for i in 0..2 {
        let e = world.spawn();
        let y = 102.0 - RADIUS - (i as f32) * (RADIUS * 2.0 + 0.5);
        world.insert(e, Position(Vec2::new(0.0, y)));
        world.insert(e, Velocity(Vec2::ZERO));
        world.insert(e, Collider::circle(RADIUS));
        world.insert(e, RigidBody::dynamic().with_restitution(0.1));
        balls.push(e);
    }
    (balls[0], balls[1])
}

fn settle_to_sleep(world: &mut World, entities: &[Entity]) {
    for _ in 0..150 {
        physics_step(world);
        world
            .get_resource_mut::<EventQueue<CollisionEvent>>()
            .unwrap()
            .flush();
    }
    let buffers = world.get_resource::<PhysicsBuffers>().unwrap();
    for (i, &e) in entities.iter().enumerate() {
        assert!(buffers.is_sleeping(e), "body {i} never fell asleep");
    }
}

#[test]
fn settled_island_sleeps_and_freezes() {
    // D-065: a settled island sleeps (velocities zeroed) and its members are
    // bit-frozen — no integration, narrow phase, or writeback touches them.
    let mut world = seed_world();
    let (bottom, top) = spawn_sleeping_stack(&mut world);
    settle_to_sleep(&mut world, &[bottom, top]);

    for &e in &[bottom, top] {
        assert_eq!(
            world.get::<Velocity>(e).unwrap().0,
            Vec2::ZERO,
            "sleeping body kept a nonzero velocity"
        );
    }
    let frozen: Vec<Vec2> = [bottom, top]
        .iter()
        .map(|&e| world.get::<Position>(e).unwrap().0)
        .collect();
    for _ in 0..30 {
        physics_step(&mut world);
    }
    for (i, &e) in [bottom, top].iter().enumerate() {
        assert_eq!(
            world.get::<Position>(e).unwrap().0,
            frozen[i],
            "sleeping body {i} moved"
        );
    }
}

#[test]
fn bullet_wakes_sleeping_body_through_speculative_gap() {
    // D-065 wake path: a fast body approaching a sleeper is admitted as a
    // speculative gap contact and wakes it before touch resolves — the
    // sleeper must not be tunneled through while frozen.
    let mut world = seed_world();

    let target = world.spawn();
    world.insert(target, Position(Vec2::new(200.0, 0.0)));
    world.insert(target, Velocity(Vec2::ZERO));
    world.insert(target, Collider::circle(4.0));
    world.insert(target, RigidBody::dynamic());

    for _ in 0..40 {
        physics_step(&mut world);
    }
    assert!(
        world
            .get_resource::<PhysicsBuffers>()
            .unwrap()
            .is_sleeping(target),
        "idle zero-gravity body never slept"
    );

    let bullet = world.spawn();
    world.insert(bullet, Position(Vec2::new(0.0, 0.0)));
    world.insert(bullet, Velocity(Vec2::new(3_000.0, 0.0)));
    world.insert(bullet, Collider::circle(4.0));
    world.insert(bullet, RigidBody::dynamic());

    for _ in 0..6 {
        physics_step(&mut world);
    }

    assert!(
        !world
            .get_resource::<PhysicsBuffers>()
            .unwrap()
            .is_sleeping(target),
        "bullet impact did not wake the sleeping target"
    );
    let target_vel = world.get::<Velocity>(target).unwrap().0;
    assert!(
        target_vel.x > 0.0,
        "no momentum transferred to woken target: {target_vel:?}"
    );
    let bullet_x = world.get::<Position>(bullet).unwrap().0.x;
    let target_x = world.get::<Position>(target).unwrap().0.x;
    assert!(
        bullet_x < target_x,
        "bullet passed through the sleeping target: bullet {bullet_x}, target {target_x}"
    );
}

#[test]
fn external_velocity_write_wakes_island() {
    let mut world = seed_world();
    let (bottom, top) = spawn_sleeping_stack(&mut world);
    settle_to_sleep(&mut world, &[bottom, top]);

    world.get_mut::<Velocity>(top).unwrap().0 = Vec2::new(0.0, -300.0);
    physics_step(&mut world);

    let buffers = world.get_resource::<PhysicsBuffers>().unwrap();
    assert!(!buffers.is_sleeping(top), "written body stayed asleep");
    assert!(
        !buffers.is_sleeping(bottom),
        "island member of written body stayed asleep"
    );
    assert!(
        world.get::<Position>(top).unwrap().0.y < 102.0 - 6.0 - 6.5,
        "woken body did not integrate its written velocity"
    );
}

#[test]
fn external_position_write_wakes_island() {
    let mut world = seed_world();
    let (bottom, top) = spawn_sleeping_stack(&mut world);
    settle_to_sleep(&mut world, &[bottom, top]);

    // Teleport the top ball into free air above the floor.
    world.get_mut::<Position>(top).unwrap().0 = Vec2::new(200.0, 0.0);
    physics_step(&mut world);

    let buffers = world.get_resource::<PhysicsBuffers>().unwrap();
    assert!(!buffers.is_sleeping(top), "teleported body stayed asleep");
    assert!(
        !buffers.is_sleeping(bottom),
        "island member of teleported body stayed asleep"
    );
    for _ in 0..5 {
        physics_step(&mut world);
    }
    assert!(
        world.get::<Velocity>(top).unwrap().0.y > 0.0,
        "teleported body is not falling under gravity"
    );
}

#[test]
fn despawn_in_sleeping_island_wakes_the_rest() {
    let mut world = seed_world();
    let (bottom, top) = spawn_sleeping_stack(&mut world);
    settle_to_sleep(&mut world, &[bottom, top]);

    let top_y = world.get::<Position>(top).unwrap().0.y;
    world.despawn(bottom);
    for _ in 0..20 {
        physics_step(&mut world);
    }

    assert!(
        !world
            .get_resource::<PhysicsBuffers>()
            .unwrap()
            .is_sleeping(top),
        "island survivor stayed asleep after member despawn"
    );
    assert!(
        world.get::<Position>(top).unwrap().0.y > top_y + 5.0,
        "unsupported body did not fall after its support despawned"
    );
}

#[test]
fn wake_api_wakes_whole_island_and_it_can_resleep() {
    let mut world = seed_world();
    let (bottom, top) = spawn_sleeping_stack(&mut world);
    settle_to_sleep(&mut world, &[bottom, top]);

    wake(&mut world, top);
    {
        let buffers = world.get_resource::<PhysicsBuffers>().unwrap();
        assert!(!buffers.is_sleeping(top), "wake() left the target asleep");
        assert!(
            !buffers.is_sleeping(bottom),
            "wake() did not wake the rest of the island"
        );
    }

    // Undisturbed, the island sleeps again after time_to_sleep.
    for _ in 0..40 {
        physics_step(&mut world);
        world
            .get_resource_mut::<EventQueue<CollisionEvent>>()
            .unwrap()
            .flush();
    }
    let buffers = world.get_resource::<PhysicsBuffers>().unwrap();
    assert!(
        buffers.is_sleeping(top) && buffers.is_sleeping(bottom),
        "woken island never re-slept"
    );
}

#[test]
fn late_sleeper_adopts_supporting_island_for_despawn_wake() {
    // D-065 tag adoption: a body that settles on an already sleeping island
    // joins that island's tag, so an island wake (despawn deep below) also
    // lifts it — otherwise it would float frozen in the air.
    let mut world = seed_world();
    if let Some(cfg) = world.get_resource_mut::<PhysicsConfig>() {
        cfg.gravity = Vec2::new(0.0, 900.0);
    }

    let floor = world.spawn();
    world.insert(floor, Position(Vec2::new(0.0, 110.0)));
    world.insert(floor, Collider::aabb(Vec2::new(64.0, 8.0)));
    world.insert(floor, RigidBody::r#static());

    const RADIUS: f32 = 6.0;
    let base = world.spawn();
    world.insert(base, Position(Vec2::new(0.0, 102.0 - RADIUS)));
    world.insert(base, Velocity(Vec2::ZERO));
    world.insert(base, Collider::circle(RADIUS));
    world.insert(base, RigidBody::dynamic());
    for _ in 0..60 {
        physics_step(&mut world);
    }
    assert!(
        world
            .get_resource::<PhysicsBuffers>()
            .unwrap()
            .is_sleeping(base),
        "base ball never slept"
    );

    // Lower a second ball gently onto the sleeper: the approach speed stays
    // under the wake tolerance, so the base stays asleep while the newcomer
    // settles on it and sleeps as a later island.
    let base_top = world.get::<Position>(base).unwrap().0.y - RADIUS;
    let rider = world.spawn();
    world.insert(rider, Position(Vec2::new(0.0, base_top - RADIUS - 0.3)));
    world.insert(rider, Velocity(Vec2::ZERO));
    world.insert(rider, Collider::circle(RADIUS));
    world.insert(rider, RigidBody::dynamic());
    for _ in 0..60 {
        physics_step(&mut world);
    }
    {
        let buffers = world.get_resource::<PhysicsBuffers>().unwrap();
        assert!(buffers.is_sleeping(base), "gentle rider woke the base");
        assert!(buffers.is_sleeping(rider), "rider never slept");
    }

    let rider_y = world.get::<Position>(rider).unwrap().0.y;
    world.despawn(base);
    for _ in 0..20 {
        physics_step(&mut world);
    }
    assert!(
        world.get::<Position>(rider).unwrap().0.y > rider_y + 5.0,
        "rider stayed frozen in the air after its support despawned"
    );
}

#[test]
fn sleeper_that_stops_being_a_dynamic_body_wakes_its_island() {
    // D-065 wake path 3 without a despawn: the entity and its key stay, but
    // it no longer has a sleep entry, so the rest of its island must wake.
    let mut world = seed_world();
    let (bottom, top) = spawn_sleeping_stack(&mut world);
    settle_to_sleep(&mut world, &[bottom, top]);

    world.insert(bottom, RigidBody::r#static());
    physics_step(&mut world);

    let buffers = world.get_resource::<PhysicsBuffers>().unwrap();
    assert!(!buffers.is_sleeping(bottom), "a static body cannot sleep");
    assert!(!buffers.is_sleeping(top), "island survivor stayed asleep");
}

#[test]
fn sleep_table_is_rebuilt_only_when_the_body_sequence_changes() {
    // D-082: with the same bodies in the same order the entries stay at
    // their proxies' indices. A spawn or despawn shifts the sequence, and
    // the rebuild carries sleepers over by entity key.
    let mut world = seed_world();
    let (bottom, top) = spawn_sleeping_stack(&mut world);
    settle_to_sleep(&mut world, &[bottom, top]);
    let rebuilds = |world: &World| {
        world
            .get_resource::<PhysicsBuffers>()
            .unwrap()
            .sleep
            .rebuilds
    };
    let index_of = |world: &World, entity: Entity| {
        let buffers = world.get_resource::<PhysicsBuffers>().unwrap();
        buffers
            .proxies
            .iter()
            .position(|proxy| proxy.entity == Some(entity))
    };
    assert_eq!(
        rebuilds(&world),
        1,
        "only the first frame lines the table up"
    );
    let bottom_index = index_of(&world, bottom);

    // A body-less collider gathers before every body and shifts the sleepers'
    // indices; a new dynamic body gathers after them.
    let pillar = world.spawn();
    world.insert(pillar, Position(Vec2::new(500.0, 0.0)));
    world.insert(pillar, Collider::circle(4.0));
    let drifter = world.spawn();
    world.insert(drifter, Position(Vec2::new(-500.0, 0.0)));
    world.insert(drifter, Velocity(Vec2::new(40.0, 0.0)));
    world.insert(drifter, Collider::circle(4.0));
    world.insert(drifter, RigidBody::dynamic());
    for _ in 0..3 {
        physics_step(&mut world);
    }
    assert_eq!(rebuilds(&world), 2);
    assert_ne!(
        index_of(&world, bottom),
        bottom_index,
        "sleeper kept its index"
    );
    let buffers = world.get_resource::<PhysicsBuffers>().unwrap();
    assert!(buffers.is_sleeping(bottom) && buffers.is_sleeping(top));
    assert!(!buffers.is_sleeping(drifter) && !buffers.is_sleeping(pillar));
    assert_eq!(buffers.sleeping_count(), 2);
    assert_eq!(buffers.sleep.entries.len(), buffers.proxy_count());

    // `wake` still finds a body after its index moved.
    wake(&mut world, top);
    let buffers = world.get_resource::<PhysicsBuffers>().unwrap();
    assert!(!buffers.is_sleeping(bottom) && !buffers.is_sleeping(top));

    world.despawn(pillar);
    physics_step(&mut world);
    assert_eq!(rebuilds(&world), 3);
    physics_step(&mut world);
    assert_eq!(rebuilds(&world), 3);
}

#[test]
fn contact_wake_stays_local_to_the_disturbance() {
    // D-065 locality: waking spreads through contacts only while motion
    // exceeds the threshold, so a gentle poke on one end of a sleeping row
    // must not wake the far end (no whole-island wake on contact).
    let mut world = seed_world();
    if let Some(cfg) = world.get_resource_mut::<PhysicsConfig>() {
        cfg.gravity = Vec2::new(0.0, 900.0);
    }

    let floor = world.spawn();
    world.insert(floor, Position(Vec2::new(0.0, 110.0)));
    world.insert(floor, Collider::aabb(Vec2::new(256.0, 8.0)));
    world.insert(floor, RigidBody::r#static());

    const RADIUS: f32 = 6.0;
    const COUNT: usize = 16;
    let mut row = Vec::with_capacity(COUNT);
    for i in 0..COUNT {
        let e = world.spawn();
        world.insert(
            e,
            Position(Vec2::new(i as f32 * (RADIUS * 2.0), 102.0 - RADIUS)),
        );
        world.insert(e, Velocity(Vec2::ZERO));
        world.insert(e, Collider::circle(RADIUS));
        world.insert(e, RigidBody::dynamic());
        row.push(e);
    }
    for _ in 0..60 {
        physics_step(&mut world);
    }
    {
        let buffers = world.get_resource::<PhysicsBuffers>().unwrap();
        for (i, &e) in row.iter().enumerate() {
            assert!(buffers.is_sleeping(e), "row ball {i} never slept");
        }
    }

    // Gentle poke: drop a ball a short distance onto the leftmost member.
    let poke = world.spawn();
    world.insert(poke, Position(Vec2::new(0.0, 102.0 - RADIUS * 3.0 - 8.0)));
    world.insert(poke, Velocity(Vec2::ZERO));
    world.insert(poke, Collider::circle(RADIUS));
    world.insert(poke, RigidBody::dynamic());
    for _ in 0..5 {
        physics_step(&mut world);
    }

    let buffers = world.get_resource::<PhysicsBuffers>().unwrap();
    for (i, &e) in row.iter().enumerate().skip(COUNT / 2) {
        assert!(
            buffers.is_sleeping(e),
            "poke on ball 0 woke distant ball {i}"
        );
    }
}

trait CollisionEventQueueExt {
    fn iter_any_tile(&self) -> bool;
}

impl CollisionEventQueueExt for EventQueue<CollisionEvent> {
    fn iter_any_tile(&self) -> bool {
        self.iter_current().any(|e| e.b.is_none())
    }
}

#[test]
fn diagnostic_counts_track_the_last_step() {
    let mut world = seed_world();
    assert_eq!(PhysicsBuffers::default().proxy_count(), 0);
    let (bottom, top) = spawn_sleeping_stack(&mut world);

    physics_step(&mut world);
    let buffers = world.get_resource::<PhysicsBuffers>().unwrap();
    assert_eq!(buffers.proxy_count(), 3, "floor plus two balls");
    assert_eq!(
        buffers.dynamic_count(),
        2,
        "the static floor is not dynamic"
    );
    assert_eq!(buffers.sleeping_count(), 0);
    assert!(buffers.pair_count() >= 2, "ball-ball and ball-floor pair");
    assert!(buffers.contact_count() >= 1);

    settle_to_sleep(&mut world, &[bottom, top]);
    physics_step(&mut world);
    let buffers = world.get_resource::<PhysicsBuffers>().unwrap();
    assert_eq!(buffers.dynamic_count(), 2);
    assert_eq!(buffers.sleeping_count(), 2);
    assert_eq!(buffers.pair_count(), 0, "sleeping bodies never initiate");
}

/// Compare the persistent list with fresh per-substep pair finding and an
/// exhaustive oracle, before any contact wakes mutate the snapshot. The
/// oracle prevents a bug shared by both broadphases from hiding a missed pair.
pub(super) fn assert_pair_contacts(config: &PhysicsConfig, sub_dt: f32, buffers: &PhysicsBuffers) {
    use std::collections::BTreeSet;

    let proxies = &buffers.proxies;
    // A pair whose layers do not meet is never a contact.
    let contact_key = |a: usize, b: usize| {
        let margin = (proxies[a].velocity - proxies[b].velocity).length() * sub_dt
            + 4.0 * config.linear_slop;
        narrow_phase(&proxies[a], &proxies[b], margin)
            .filter(|_| buffers.layers.meet(a, b))
            .map(|_| pair_key(proxies[a].key, proxies[b].key))
    };
    let mut persistent = BTreeSet::new();
    for &(a, b) in &buffers.pairs {
        if let Some(key) = contact_key(a as usize, b as usize) {
            assert!(persistent.insert(key), "duplicate persistent contact");
        }
    }

    let mut fresh_grid = SpatialGrid::new(config.broadphase_cell_size);
    let bounds: Vec<_> = proxies
        .iter()
        .enumerate()
        .map(|(i, proxy)| {
            let mut aabb = proxy.world_aabb();
            aabb.half_extents +=
                Vec2::splat(proxy.velocity.length() * sub_dt + 2.0 * config.linear_slop);
            fresh_grid.insert(i as u32, &aabb);
            aabb
        })
        .collect();
    let mut fresh = BTreeSet::new();
    let mut exhaustive = BTreeSet::new();
    // A kinematic proxy initiates as an awake dynamic one does.
    for (a, proxy) in proxies.iter().enumerate() {
        if !proxy.moves() {
            continue;
        }
        let eligible = |b: usize| a != b && (!proxies[b].moves() || b > a);
        fresh_grid.for_each_in(&bounds[a], Some(a as u32), |b| {
            let b = b as usize;
            if eligible(b)
                && bounds[a].overlaps(&bounds[b])
                && let Some(key) = contact_key(a, b)
            {
                assert!(fresh.insert(key), "duplicate fresh contact");
            }
        });
        for b in 0..proxies.len() {
            if eligible(b)
                && let Some(key) = contact_key(a, b)
            {
                exhaustive.insert(key);
            }
        }
    }
    assert_eq!(fresh, exhaustive, "fresh pair finding missed a contact");
    assert_eq!(
        persistent, exhaustive,
        "persistent pair list missed a contact"
    );
}

#[test]
fn persistent_pairs_match_fresh_contacts_on_randomized_piles_bullets_and_wakes() {
    use crate::Pcg32;

    let mut checked = 0;
    let mut repairs = 0;
    for seed in 0..8 {
        let mut rng = Pcg32::seeded(0xD075 + seed);
        let mut world = seed_world();
        let (bottom, top) = spawn_sleeping_stack(&mut world);
        settle_to_sleep(&mut world, &[bottom, top]);
        let buffers = world.get_resource_mut::<PhysicsBuffers>().unwrap();
        buffers.check_pair_contacts = true;
        buffers.reference_impulses = Some(buffers.impulses.clone());
        // Cover differing substep counts and cell boundaries, mixed shape
        // pairs, sleepers and fast bullets in the same evolving snapshot.
        let config = world.get_resource_mut::<PhysicsConfig>().unwrap();
        config.substeps = [1, 2, 4, 8][seed as usize % 4];
        config.broadphase_cell_size = [8.0, 16.0, 32.0][seed as usize % 3];
        for i in 0..36 {
            let e = world.spawn();
            world.insert(
                e,
                Position(Vec2::new(
                    (i % 6) as f32 * 11.0 - 28.0 + rng.next_range(-1.0, 1.0),
                    65.0 - (i / 6) as f32 * 11.0 + rng.next_range(-1.0, 1.0),
                )),
            );
            world.insert(
                e,
                Velocity(rng.next_unit_vec2() * rng.next_range(0.0, 80.0)),
            );
            world.insert(
                e,
                RigidBody::dynamic().with_restitution(rng.next_range(0.0, 0.5)),
            );
            world.insert(
                e,
                if i % 3 == 0 {
                    Collider::aabb(Vec2::splat(5.0))
                } else {
                    Collider::circle(5.0)
                },
            );
        }
        let bullet = world.spawn();
        world.insert(bullet, Position(Vec2::new(-90.0, 95.0)));
        world.insert(
            bullet,
            Velocity(Vec2::new(rng.next_range(2400.0, 15000.0), 0.0)),
        );
        world.insert(bullet, Collider::circle(4.0));
        world.insert(bullet, RigidBody::dynamic().with_restitution(0.8));
        for frame in 0..40 {
            match frame {
                8 => wake(&mut world, top),
                16 => world.get_mut::<Velocity>(bottom).unwrap().0 = Vec2::new(0.0, -180.0),
                24 => {
                    world.despawn(top);
                }
                _ => {}
            }
            physics_step(&mut world);
            world
                .get_resource_mut::<EventQueue<CollisionEvent>>()
                .unwrap()
                .flush();
        }
        let buffers = world.get_resource::<PhysicsBuffers>().unwrap();
        assert!(
            !buffers.is_sleeping(bottom),
            "wake cases never disturbed the sleeper"
        );
        checked += buffers.checked_substeps;
        repairs += buffers.pair_repairs;
    }
    assert_eq!(
        checked, 1200,
        "every substep must run the equivalence oracle"
    );
    assert!(repairs > 0, "the oracle never checked a repaired list");
}

#[test]
fn contact_woken_body_rebuilds_pairs_over_static_floor_in_waking_frame() {
    let mut world = seed_world();
    let (bottom, top) = spawn_sleeping_stack(&mut world);
    settle_to_sleep(&mut world, &[bottom, top]);
    // Hit the bottom body horizontally, below the top body: the first
    // build must omit sleeper-floor, then include it immediately after wake.
    let y = world.get::<Position>(bottom).unwrap().0.y;
    let bullet = world.spawn();
    world.insert(bullet, Position(Vec2::new(-20.0, y)));
    world.insert(bullet, Velocity(Vec2::new(2400.0, 0.0)));
    world.insert(bullet, Collider::circle(4.0));
    world.insert(bullet, RigidBody::dynamic());
    let config = *world.get_resource::<PhysicsConfig>().unwrap();
    let dt = 1.0 / 60.0;
    let sub_dt = dt / config.substeps as f32;
    let mut buffers = world.remove_resource::<PhysicsBuffers>().unwrap();
    gather_proxies(
        &world,
        &mut buffers.proxies,
        &mut buffers.layers,
        &mut buffers.sensors,
        &mut buffers.tile_ids,
        &mut buffers.tiles,
    );
    buffers.bodies = buffers.proxies.len() - buffers.tiles.len();
    sleep_frame_start(&mut buffers, 4.0 * config.linear_slop);
    buffers.island_parent = (0..buffers.proxies.len() as u32).collect();
    buffers.events.clear();
    buffers.pairs_invalidated = true;
    buffers.static_grid.begin_frame();
    buffers.check_pair_contacts = true;
    let bottom_idx = buffers
        .proxies
        .iter()
        .position(|p| p.entity == Some(bottom))
        .unwrap();
    let floor_idx = buffers.proxies.iter().position(|p| !p.is_dynamic).unwrap();
    let floor_pair = |pairs: &[(u32, u32)]| {
        pairs
            .iter()
            .any(|&(a, b)| a as usize == bottom_idx && b as usize == floor_idx)
    };
    build_pairs(&config, sub_dt, dt, &mut buffers);
    assert!(!floor_pair(&buffers.pairs), "sleepers must not initiate");
    substep(&config, sub_dt, dt, &mut buffers, true);
    assert!(
        !buffers.proxies[bottom_idx].sleeping,
        "impact must wake this substep"
    );
    assert!(
        buffers.pairs_invalidated,
        "wake must invalidate even if travel fits"
    );
    let builds = buffers.pair_builds;
    for step in 1..config.substeps {
        substep(
            &config,
            sub_dt,
            dt - step as f32 * sub_dt,
            &mut buffers,
            true,
        );
        assert!(
            floor_pair(&buffers.pairs),
            "woken body missing its static floor"
        );
        assert!(
            buffers.proxies[bottom_idx].center.y + 6.0 < 103.0,
            "woken body fell through its floor"
        );
    }
    assert!(buffers.pair_builds > builds);
    assert!(
        buffers
            .contacts
            .iter()
            .any(|c| c.a as usize == bottom_idx && c.b as usize == floor_idx)
    );
}

#[test]
fn lone_fast_body_trips_its_budget_and_rebuilds_before_narrow_phase() {
    let mut world = seed_world();
    let bullet = world.spawn();
    world.insert(bullet, Position(Vec2::ZERO));
    world.insert(bullet, Velocity(Vec2::new(240.0, 0.0)));
    world.insert(bullet, Collider::circle(4.0));
    world.insert(bullet, RigidBody::dynamic());
    let wall = world.spawn();
    world.insert(wall, Position(Vec2::new(48.0, 0.0)));
    world.insert(wall, Collider::aabb(Vec2::new(2.0, 32.0)));
    world.insert(wall, RigidBody::r#static());
    let config = *world.get_resource::<PhysicsConfig>().unwrap();
    let dt = 1.0 / 60.0;
    let sub_dt = dt / config.substeps as f32;
    let mut buffers = PhysicsBuffers::default();
    gather_proxies(
        &world,
        &mut buffers.proxies,
        &mut buffers.layers,
        &mut buffers.sensors,
        &mut buffers.tile_ids,
        &mut buffers.tiles,
    );
    buffers.bodies = buffers.proxies.len() - buffers.tiles.len();
    buffers.island_parent = (0..buffers.proxies.len() as u32).collect();
    buffers.check_pair_contacts = true;
    build_pairs(&config, sub_dt, dt, &mut buffers);
    assert!(
        buffers.pairs.is_empty(),
        "wall should start outside the budget"
    );
    let idx = buffers
        .proxies
        .iter()
        .position(|p| p.entity == Some(bullet))
        .unwrap();
    substep(&config, sub_dt, dt, &mut buffers, false);
    assert_eq!(
        (buffers.pair_builds, buffers.pair_repairs),
        (1, 0),
        "unspent budget should reuse pairs"
    );
    assert!(buffers.pair_budgets[idx].travel > 0.0);
    // Model an impulse spike after the first build. The next admission must
    // see the wall even though it wasn't a candidate in the old list. The
    // only awake body tripped, which is more than a quarter of them, so the
    // list is rebuilt (D-081).
    buffers.proxies[idx].velocity = Vec2::new(15000.0, 0.0);
    assert_eq!(collect_tripped(&config, sub_dt, &mut buffers), 1);
    assert_eq!(buffers.tripped, [idx as u32]);
    substep(&config, sub_dt, dt - sub_dt, &mut buffers, false);
    assert_eq!((buffers.pair_builds, buffers.pair_repairs), (2, 0));
    assert!(buffers.contacts.iter().any(|c| c.a as usize == idx));
    assert!(
        buffers.proxies[idx].center.x + 4.0 <= 46.5,
        "budget trip tunneled wall"
    );
    let radius = 15000.0 * (dt - sub_dt) + (PAIR_MARGIN_SLOPS + 2.0) * config.linear_slop;
    assert!(
        (buffers.pair_budgets[idx].radius - radius).abs() < 1e-4,
        "rebuild must use the remaining frame time"
    );
    // Accumulated travel is independently sufficient to trip even after
    // velocity drops to zero (e.g. a solver/sweep clamp).
    buffers.proxies[idx].velocity = Vec2::ZERO;
    buffers.pair_budgets[idx].travel = buffers.pair_budgets[idx].radius;
    collect_tripped(&config, sub_dt, &mut buffers);
    assert_eq!(buffers.tripped, [idx as u32]);
}

#[test]
fn gravity_allowance_is_the_exact_worst_case_of_a_falling_body() {
    // D-081: replay the integrator (velocity first, then position) for a
    // body falling along gravity. Before every substep the budget check adds
    // one substep of travel at the current speed to the travel so far; the
    // allowance must cover each check and be tight at the frame's last one.
    let config = PhysicsConfig {
        gravity: Vec2::new(0.0, 900.0),
        ..PhysicsConfig::default()
    };
    let (speed, dt) = (350.0_f32, 1.0_f32 / 60.0);
    for substeps in 1..=8 {
        let h = dt / substeps as f32;
        let allowance = gravity_allowance(&config, h, dt);
        let (mut velocity, mut travel, mut worst) = (speed, 0.0_f32, 0.0_f32);
        for _ in 0..substeps {
            let needed = travel + velocity * h - speed * dt;
            assert!(needed <= allowance + 1e-4, "{substeps} substeps: {needed}");
            worst = needed;
            velocity += 900.0 * h;
            travel += velocity * h;
        }
        assert!(
            (worst - allowance).abs() < 1e-4,
            "{substeps} substeps: last check needs {worst}, allowance {allowance}"
        );
        // D-075's `g t^2 / 2` differs by `g h^2 (n - 2) / 2`: one `g h^2`
        // short at four substeps, which tripped every falling body.
        let old = 0.5 * 900.0 * dt * dt;
        let shortfall = 900.0 * h * h * (substeps as f32 - 2.0) / 2.0;
        assert!((allowance - old - shortfall).abs() < 1e-4, "{substeps}");
    }
    // A repair late in the frame predicts only what is left.
    let h = dt / 4.0;
    assert_eq!(gravity_allowance(&config, h, h), 0.0);
    assert!((gravity_allowance(&config, h, 2.0 * h) - 2.0 * 900.0 * h * h).abs() < 1e-6);
}

/// `cols` by `rows` dynamic circles of radius 5 on a 9.5 px lattice, in
/// proxy order row by row. Neighbours overlap by 0.5 px, so every contact
/// carries a small impulse while the bodies creep apart at about 2 px/s, far
/// below what a budget needs to trip.
fn spawn_lattice(world: &mut World, cols: usize, rows: usize) {
    for i in 0..cols * rows {
        let e = world.spawn();
        world.insert(
            e,
            Position(Vec2::new((i % cols) as f32, (i / cols) as f32) * 9.5),
        );
        world.insert(e, Velocity(Vec2::ZERO));
        world.insert(e, Collider::circle(5.0));
        world.insert(e, RigidBody::dynamic());
    }
}

/// Gathered buffers with both oracles armed, for driving substeps by hand:
/// the contact set against per-substep pair finding (D-075) and every warm
/// start against the per-substep keyed map (D-076).
fn oracle_buffers(world: &World) -> PhysicsBuffers {
    let mut buffers = PhysicsBuffers::default();
    gather_proxies(
        world,
        &mut buffers.proxies,
        &mut buffers.layers,
        &mut buffers.sensors,
        &mut buffers.tile_ids,
        &mut buffers.tiles,
    );
    buffers.bodies = buffers.proxies.len() - buffers.tiles.len();
    buffers.island_parent = (0..buffers.proxies.len() as u32).collect();
    buffers.pairs_invalidated = true;
    buffers.check_pair_contacts = true;
    buffers.reference_impulses = Some(ImpulseMap::default());
    buffers
}

/// The carried impulse of pair `(a, b)`, which must be in the list once and
/// in that orientation.
fn carried_impulse(buffers: &PhysicsBuffers, a: u32, b: u32) -> f32 {
    let mut found = buffers
        .pairs
        .iter()
        .zip(&buffers.pair_impulses)
        .filter(|&(&(x, y), _)| (x, y) == (a, b) || (x, y) == (b, a));
    let (&pair, &impulse) = found.next().expect("pair missing from the list");
    assert_eq!(pair, (a, b), "pair stored in the wrong orientation");
    assert!(found.next().is_none(), "pair ({a}, {b}) listed twice");
    impulse
}

#[test]
fn tripped_bodies_are_repaired_without_a_rebuild() {
    // D-081 on a 5 by 4 lattice (index = row * 5 + column), one frame driven
    // by hand with both oracles checking every substep.
    let mut world = seed_world();
    spawn_lattice(&mut world, 5, 4);
    let config = *world.get_resource::<PhysicsConfig>().unwrap();
    let dt = 1.0 / 60.0;
    let sub_dt = dt / 4.0;
    let mut buffers = oracle_buffers(&world);
    substep(&config, sub_dt, dt, &mut buffers, false);
    assert_eq!((buffers.pair_builds, buffers.pair_repairs), (1, 0));
    let before = buffers.pairs.clone();
    let kept_impulse = carried_impulse(&buffers, 7, 8);
    let left_impulse = carried_impulse(&buffers, 8, 9);
    let above_impulse = carried_impulse(&buffers, 4, 9);
    let member_impulse = carried_impulse(&buffers, 9, 14);
    let below_impulse = carried_impulse(&buffers, 14, 19);
    for impulse in [kept_impulse, left_impulse, above_impulse, member_impulse] {
        assert!(impulse > 0.0, "lattice contacts must carry an impulse");
    }

    // Two neighbours on the right edge speed up and leave. Two of twenty
    // awake bodies tripped, so their pairs are repaired.
    buffers.proxies[9].velocity = Vec2::new(300.0, 0.0);
    buffers.proxies[14].velocity = Vec2::new(300.0, 0.0);
    refresh_pairs(&config, sub_dt, dt - sub_dt, &mut buffers);
    assert_eq!((buffers.pair_builds, buffers.pair_repairs), (1, 1));
    assert_eq!(buffers.tripped, [9, 14]);
    assert_eq!(buffers.repaired, [9, 14]);
    // Pairs without a member keep their order at the front of the list.
    let kept: Vec<_> = before
        .iter()
        .copied()
        .filter(|&(a, b)| ![9, 14].contains(&a) && ![9, 14].contains(&b))
        .collect();
    assert_eq!(buffers.pairs[..kept.len()], kept[..]);
    assert_eq!(carried_impulse(&buffers, 7, 8), kept_impulse);
    // An untripped awake neighbour of lower index: the build's initiator
    // rule would drop these two pairs, since only the members query.
    assert_eq!(carried_impulse(&buffers, 8, 9), left_impulse);
    assert_eq!(carried_impulse(&buffers, 4, 9), above_impulse);
    // A pair between two members is added once, by the lower one.
    assert_eq!(carried_impulse(&buffers, 9, 14), member_impulse);
    assert_eq!(carried_impulse(&buffers, 14, 19), below_impulse);
    let radius = 300.0 * (dt - sub_dt) + (PAIR_MARGIN_SLOPS + 2.0) * config.linear_slop;
    assert!((buffers.pair_budgets[9].radius - radius).abs() < 1e-4);
    assert_eq!(buffers.pair_budgets[9].travel, 0.0);
    substep(&config, sub_dt, dt - sub_dt, &mut buffers, false);
    assert_eq!((buffers.pair_builds, buffers.pair_repairs), (1, 1));

    // A pair between bodies repaired in different substeps: 13 trips now,
    // and finds 14 and 9 through the repair grid.
    buffers.proxies[13].velocity = Vec2::new(300.0, 0.0);
    refresh_pairs(&config, sub_dt, dt - 2.0 * sub_dt, &mut buffers);
    assert_eq!(buffers.tripped, [13]);
    assert_eq!(buffers.repaired, [9, 14, 13]);
    carried_impulse(&buffers, 13, 14);
    carried_impulse(&buffers, 9, 13);
    carried_impulse(&buffers, 12, 13);
    substep(&config, sub_dt, dt - 2.0 * sub_dt, &mut buffers, false);

    // A body repaired twice in one frame stays in the repaired list once.
    buffers.proxies[9].velocity = Vec2::new(900.0, 0.0);
    refresh_pairs(&config, sub_dt, dt - 3.0 * sub_dt, &mut buffers);
    assert_eq!(buffers.tripped, [9]);
    assert_eq!(buffers.repaired, [9, 14, 13]);
    substep(&config, sub_dt, dt - 3.0 * sub_dt, &mut buffers, false);
    assert_eq!((buffers.pair_builds, buffers.pair_repairs), (1, 3));
    assert_eq!(buffers.checked_substeps, 4);
}

#[test]
fn more_than_a_quarter_tripped_rebuilds_instead_of_repairing() {
    let mut world = seed_world();
    spawn_lattice(&mut world, 4, 2);
    let config = *world.get_resource::<PhysicsConfig>().unwrap();
    let dt = 1.0 / 60.0;
    let sub_dt = dt / 4.0;
    let mut buffers = oracle_buffers(&world);
    substep(&config, sub_dt, dt, &mut buffers, false);

    // Two of eight is a quarter: still a repair.
    for index in [3, 7] {
        buffers.proxies[index].velocity = Vec2::new(300.0, 0.0);
    }
    substep(&config, sub_dt, dt - sub_dt, &mut buffers, false);
    assert_eq!((buffers.pair_builds, buffers.pair_repairs), (1, 1));

    // Three of eight is more: the list is rebuilt, and the build forgets
    // which proxies were repaired.
    for index in [0, 3, 4] {
        buffers.proxies[index].velocity = Vec2::new(-900.0, 0.0);
    }
    substep(&config, sub_dt, dt - 2.0 * sub_dt, &mut buffers, false);
    assert_eq!((buffers.pair_builds, buffers.pair_repairs), (2, 1));
    assert!(buffers.repaired.is_empty());
    assert!(buffers.pair_flags.iter().all(|&flags| flags == PAIR_AWAKE));
}

#[test]
fn warm_start_forgets_disappearing_contacts_and_syncs_empty_final_substep() {
    let mut world = seed_world();
    let body = world.spawn();
    world.insert(body, Position(Vec2::ZERO));
    world.insert(body, Velocity(Vec2::new(100.0, 0.0)));
    world.insert(body, Collider::circle(5.0));
    world.insert(body, RigidBody::dynamic());
    let wall = world.spawn();
    world.insert(wall, Position(Vec2::new(8.0, 0.0)));
    world.insert(wall, Collider::circle(5.0));
    let config = PhysicsConfig {
        gravity: Vec2::ZERO,
        ..PhysicsConfig::default()
    };
    let mut buffers = PhysicsBuffers::default();
    gather_proxies(
        &world,
        &mut buffers.proxies,
        &mut buffers.layers,
        &mut buffers.sensors,
        &mut buffers.tile_ids,
        &mut buffers.tiles,
    );
    buffers.bodies = buffers.proxies.len() - buffers.tiles.len();
    buffers.reference_impulses = Some(ImpulseMap::default());
    let body_index = buffers
        .proxies
        .iter()
        .position(|p| p.entity == Some(body))
        .unwrap();
    let key = pair_key(entity_key(body), entity_key(wall));
    // A generous real travel budget keeps the candidate present while the
    // exact contact disappears and returns. The reference map checks the
    // warm-start value before every solve, not only the final result.
    build_pairs(&config, 1.0 / 240.0, 1.0, &mut buffers);
    substep(&config, 1.0 / 240.0, 1.0, &mut buffers, false);
    assert!(buffers.contacts.iter().any(|c| c.impulse > 0.0));
    let builds = buffers.pair_builds;
    for (x, rebuild) in [
        (-30.0, false),
        (0.0, false),
        (-30.0, true),
        (0.0, true),
        (-30.0, false),
    ] {
        buffers.proxies[body_index].center = Vec2::new(x, 0.0);
        buffers.proxies[body_index].velocity = Vec2::ZERO;
        if rebuild {
            buffers.pairs_invalidated = true;
        }
        substep(&config, 1.0 / 240.0, 1.0, &mut buffers, false);
        if x < 0.0 {
            assert!(buffers.contacts.is_empty());
        }
    }
    assert!(
        buffers.pair_builds > builds,
        "exercise reordered pair-cache boundary"
    );
    sync_impulses(&mut buffers);
    assert_eq!(buffers.impulses.get(key), 0.0);
    assert!(buffers.impulses.keys.iter().all(|key| *key == EMPTY_PAIR));
    assert!(!buffers.impulses_dirty);
}

fn arrival_counts(world: &World) -> (usize, usize) {
    let buffers = world
        .get_resource::<PhysicsBuffers>()
        .expect("physics buffers");
    (buffers.arrival.listed, buffers.arrival.clamps)
}

#[test]
fn settled_stack_never_enters_the_arrival_pass() {
    // Warm starts and gravity cancel in a resting pile, so no body's velocity
    // moves by the arrival threshold and the pass lists nobody (D-092).
    let mut world = seed_world();
    {
        let config = world.get_resource_mut::<PhysicsConfig>().unwrap();
        config.gravity = Vec2::new(0.0, 900.0);
        config.sleep_threshold = 0.0;
    }
    let floor = world.spawn();
    world.insert(floor, Position(Vec2::new(0.0, 500.0)));
    world.insert(floor, Collider::aabb(Vec2::new(400.0, 20.0)));
    world.insert(floor, RigidBody::r#static());
    for i in 0..5 {
        let body = world.spawn();
        world.insert(body, Position(Vec2::new(0.0, 464.0 - 32.0 * i as f32)));
        world.insert(body, Velocity(Vec2::ZERO));
        world.insert(body, Collider::aabb(Vec2::splat(16.0)));
        world.insert(body, RigidBody::dynamic());
    }

    for _ in 0..240 {
        physics_step(&mut world);
    }
    let settled = arrival_counts(&world);
    for _ in 0..120 {
        physics_step(&mut world);
    }
    assert_eq!(arrival_counts(&world), settled);
}

#[test]
fn pushed_body_arrives_at_a_static_wall_and_stops_its_pusher() {
    // A heavy pusher drives a light body at a thin static wall. The wall
    // contact is solved before or after the push depending on spawn order;
    // either way the body ends against the wall and the pusher behind it.
    const RADIUS: f32 = 8.0;
    const WALL_FACE: f32 = 798.0;
    for wall_first in [false, true] {
        let mut world = seed_world();
        let spawn_wall = |world: &mut World| {
            let wall = world.spawn();
            world.insert(wall, Position(Vec2::new(WALL_FACE + 2.0, 0.0)));
            world.insert(wall, Collider::aabb(Vec2::new(2.0, 400.0)));
            world.insert(wall, RigidBody::r#static());
        };
        let spawn_ball = |world: &mut World, x: f32, speed: f32, mass: f32| {
            let ball = world.spawn();
            world.insert(ball, Position(Vec2::new(x, 0.0)));
            world.insert(ball, Velocity(Vec2::new(speed, 0.0)));
            world.insert(ball, Collider::circle(RADIUS));
            world.insert(ball, RigidBody::dynamic().with_mass(mass));
            ball
        };
        if wall_first {
            spawn_wall(&mut world);
        }
        let pushed = spawn_ball(&mut world, WALL_FACE - RADIUS - 3.0, 0.0, 1.0);
        if !wall_first {
            spawn_wall(&mut world);
        }
        let pusher = spawn_ball(&mut world, WALL_FACE - RADIUS - 3.0 - 46.0, 1_920.0, 100.0);

        for _ in 0..60 {
            physics_step(&mut world);
        }

        let pushed_x = world.get::<Position>(pushed).unwrap().0.x;
        let pusher_x = world.get::<Position>(pusher).unwrap().0.x;
        assert!(
            pushed_x + RADIUS <= WALL_FACE + 1.0,
            "wall_first={wall_first}: pushed body sits {} px into the wall",
            pushed_x + RADIUS - WALL_FACE
        );
        assert!(
            pusher_x + 2.0 * RADIUS <= pushed_x + 1.0,
            "wall_first={wall_first}: pusher at {pusher_x} overlaps the pushed body at {pushed_x}"
        );
        let (listed, clamps) = arrival_counts(&world);
        assert!(listed > 0 && clamps > 0, "the arrival pass did not run");
    }
}

/// Two bodies in free fall with nothing in reach, one per integration path:
/// a collider body (substeps) and a collider-less one (`integrate_loose_bodies`).
fn spawn_free_fallers(world: &mut World, x: f32) -> [Entity; 2] {
    let ball = world.spawn();
    world.insert(ball, Position(Vec2::new(x, 0.0)));
    world.insert(ball, Velocity(Vec2::new(60.0, 0.0)));
    world.insert(ball, Collider::circle(7.5));
    world.insert(ball, RigidBody::dynamic());
    let loose = world.spawn();
    world.insert(loose, Position(Vec2::new(x + 200.0, 0.0)));
    world.insert(loose, Velocity(Vec2::new(60.0, 0.0)));
    world.insert(loose, RigidBody::dynamic());
    [ball, loose]
}

fn body_bits(world: &World, bodies: &[Entity]) -> Vec<u32> {
    bodies
        .iter()
        .flat_map(|&body| {
            let position = world.get::<Position>(body).unwrap().0;
            let velocity = world.get::<Velocity>(body).unwrap().0;
            [position.x, position.y, velocity.x, velocity.y].map(f32::to_bits)
        })
        .collect()
}

/// Step-bound scene (D-094): a five-ball column on a static floor under
/// gravity, still awake when the measured steps begin, with the two free
/// fallers beside it. Ten steps at 1/60 s, then 20 of `dt`; returns every
/// body's position and velocity bits.
fn step_bound_scene(max_step_dt: f32, dt: f32) -> Vec<u32> {
    const RADIUS: f32 = 7.5;
    let mut world = seed_world();
    {
        let config = world.get_resource_mut::<PhysicsConfig>().unwrap();
        config.gravity = Vec2::new(0.0, 900.0);
        config.max_step_dt = max_step_dt;
    }
    let floor = world.spawn();
    world.insert(floor, Position(Vec2::new(0.0, 520.0)));
    world.insert(floor, Collider::aabb(Vec2::new(100.0, 20.0)));
    world.insert(floor, RigidBody::r#static());
    let mut bodies: Vec<Entity> = (0..5)
        .map(|i| {
            let body = world.spawn();
            let y = 500.0 - RADIUS - 2.0 * RADIUS * i as f32;
            world.insert(body, Position(Vec2::new(0.0, y)));
            world.insert(body, Velocity(Vec2::ZERO));
            world.insert(body, Collider::circle(RADIUS));
            world.insert(body, RigidBody::dynamic());
            body
        })
        .collect();
    bodies.extend(spawn_free_fallers(&mut world, 300.0));

    for _ in 0..10 {
        physics_step(&mut world);
    }
    world.get_resource_mut::<Time>().unwrap().advance_frame(dt);
    for _ in 0..20 {
        physics_step(&mut world);
    }
    body_bits(&world, &bodies)
}

#[test]
fn slow_frame_advances_only_the_step_bound() {
    // D-094: handed the 0.1 s dt cap of D-088, each call advances
    // `max_step_dt` and drops the rest, on both integration paths.
    const STEPS: usize = 3;
    let bound = PhysicsConfig::default().max_step_dt;
    let run = |dt: f32| {
        let mut world = seed_world();
        world.get_resource_mut::<PhysicsConfig>().unwrap().gravity = Vec2::new(0.0, 900.0);
        let fallers = spawn_free_fallers(&mut world, 0.0);
        world.get_resource_mut::<Time>().unwrap().advance_frame(dt);
        for _ in 0..STEPS {
            physics_step(&mut world);
        }
        let state = fallers.map(|body| {
            (
                world.get::<Position>(body).unwrap().0,
                world.get::<Velocity>(body).unwrap().0,
            )
        });
        (state, body_bits(&world, &fallers))
    };

    let (slow, slow_bits) = run(0.1);
    let (_, bound_bits) = run(bound);
    assert_eq!(slow_bits, bound_bits, "a 0.1 s dt is not a {bound} s step");

    let elapsed = STEPS as f32 * bound;
    for (start_x, (position, velocity)) in [0.0, 200.0].into_iter().zip(slow) {
        assert!(
            (position.x - start_x - 60.0 * elapsed).abs() < 1.0e-3,
            "advanced {} px in x, not {elapsed} s of 60 px/s",
            position.x - start_x
        );
        assert!(
            (velocity.y - 900.0 * elapsed).abs() < 1.0e-2,
            "fell to {} px/s, not {elapsed} s of gravity",
            velocity.y
        );
    }
}

#[test]
fn step_bound_off_takes_the_whole_dt_as_before() {
    // `max_step_dt <= 0` is unbounded. The hash (FNV-1a over the scene's
    // position and velocity bits) is from the tree before the bound existed.
    const BEFORE_D094: u64 = 0x070b_6b82_ad91_b1fb;
    let hash = |bits: &[u32]| {
        bits.iter().fold(0xcbf2_9ce4_8422_2325_u64, |hash, &bits| {
            (hash ^ u64::from(bits)).wrapping_mul(0x0000_0100_0000_01b3)
        })
    };
    for off in [0.0, -1.0] {
        let unbounded = hash(&step_bound_scene(off, 0.1));
        assert_eq!(
            unbounded, BEFORE_D094,
            "max_step_dt = {off}: {unbounded:#018x}"
        );
    }
    let bounded = hash(&step_bound_scene(PhysicsConfig::default().max_step_dt, 0.1));
    assert_ne!(
        bounded, BEFORE_D094,
        "the default bound left a 0.1 s step alone"
    );
}

#[test]
fn step_bound_is_invisible_at_a_sixtieth() {
    // The pinned 1/60 s step of benchmarks, smoke runs and the pixel test
    // (D-088) must not see the bound: default and off agree bit for bit.
    let bound = PhysicsConfig::default().max_step_dt;
    assert_eq!(
        step_bound_scene(bound, 1.0 / 60.0),
        step_bound_scene(0.0, 1.0 / 60.0)
    );
}

/// A body with `Position`, `Velocity`, `Collider` and `RigidBody`, so
/// kinematic and dynamic bodies share one archetype and proxy order is spawn
/// order.
fn spawn_body(
    world: &mut World,
    body: RigidBody,
    collider: Collider,
    position: Vec2,
    velocity: Vec2,
) -> Entity {
    let entity = world.spawn();
    world.insert(entity, Position(position));
    world.insert(entity, Velocity(velocity));
    world.insert(entity, collider);
    world.insert(entity, body);
    entity
}

fn spawn_kinematic(
    world: &mut World,
    collider: Collider,
    position: Vec2,
    velocity: Vec2,
) -> Entity {
    spawn_body(world, RigidBody::kinematic(), collider, position, velocity)
}

fn spawn_ball(world: &mut World, position: Vec2) -> Entity {
    spawn_body(
        world,
        RigidBody::dynamic(),
        Collider::circle(10.0),
        position,
        Vec2::ZERO,
    )
}

fn position_bits(world: &World, body: Entity) -> [u32; 2] {
    world
        .get::<Position>(body)
        .unwrap()
        .0
        .to_array()
        .map(f32::to_bits)
}

fn is_sleeping(world: &World, body: Entity) -> bool {
    world
        .get_resource::<PhysicsBuffers>()
        .unwrap()
        .is_sleeping(body)
}

/// Position bits of a kinematic body after `steps` steps of 1/60 s in a
/// world that holds nothing else, under `gravity`.
fn kinematic_alone(
    collider: Collider,
    start: Vec2,
    velocity: Vec2,
    gravity: Vec2,
    steps: usize,
) -> [u32; 2] {
    let mut world = seed_world();
    world.get_resource_mut::<PhysicsConfig>().unwrap().gravity = gravity;
    let body = spawn_kinematic(&mut world, collider, start, velocity);
    for _ in 0..steps {
        physics_step(&mut world);
    }
    position_bits(&world, body)
}

/// Every collision event sent by the last step, flushing the queue.
fn drain_events(world: &mut World) -> Vec<CollisionEvent> {
    let events = world
        .get_resource_mut::<EventQueue<CollisionEvent>>()
        .unwrap();
    let sent = events.iter_current().copied().collect();
    events.flush();
    sent
}

#[test]
fn kinematic_body_moves_by_its_velocity_without_gravity() {
    // Gravity, falling bodies and a floor elsewhere leave its path alone.
    let gravity = Vec2::new(0.0, 900.0);
    let velocity = Vec2::new(60.0, -30.0);
    let collider = Collider::aabb(Vec2::splat(12.0));
    let mut world = seed_world();
    world.get_resource_mut::<PhysicsConfig>().unwrap().gravity = gravity;
    let floor = world.spawn();
    world.insert(floor, Position(Vec2::new(0.0, 400.0)));
    world.insert(floor, Collider::aabb(Vec2::new(200.0, 8.0)));
    world.insert(floor, RigidBody::r#static());
    for i in 0..4 {
        spawn_ball(&mut world, Vec2::new(-150.0 + 30.0 * i as f32, 300.0));
    }
    let body = spawn_kinematic(&mut world, collider, Vec2::ZERO, velocity);
    for _ in 0..60 {
        physics_step(&mut world);
    }
    let position = world.get::<Position>(body).unwrap().0;
    assert!(
        (position - velocity).length() < 1.0e-3,
        "one second at {velocity:?} ended at {position:?}"
    );
    assert_eq!(
        world.get::<Velocity>(body).unwrap().0,
        velocity,
        "the step changed a kinematic velocity"
    );
    assert_eq!(
        position_bits(&world, body),
        kinematic_alone(collider, Vec2::ZERO, velocity, gravity, 60),
        "the kinematic body left the path it takes alone"
    );
}

#[test]
fn kinematic_body_pushes_a_dynamic_body_and_is_never_pushed() {
    let collider = Collider::aabb(Vec2::splat(12.0));
    let velocity = Vec2::new(150.0, 0.0);
    let mut world = seed_world();
    let body = spawn_kinematic(&mut world, collider, Vec2::ZERO, velocity);
    let ball = spawn_ball(&mut world, Vec2::new(40.0, 0.0));
    let heavy = spawn_body(
        &mut world,
        RigidBody::dynamic().with_mass(1.0e6),
        Collider::circle(10.0),
        Vec2::new(40.0, 60.0),
        Vec2::ZERO,
    );
    // A second kinematic body drives into the heavy one from below.
    let lifter = spawn_kinematic(
        &mut world,
        collider,
        Vec2::new(40.0, 100.0),
        Vec2::new(0.0, -60.0),
    );
    for _ in 0..60 {
        physics_step(&mut world);
    }
    assert_eq!(
        position_bits(&world, body),
        kinematic_alone(collider, Vec2::ZERO, velocity, Vec2::ZERO, 60),
        "the pushed ball moved the kinematic body"
    );
    assert_eq!(
        position_bits(&world, lifter),
        kinematic_alone(
            collider,
            Vec2::new(40.0, 100.0),
            Vec2::new(0.0, -60.0),
            Vec2::ZERO,
            60
        ),
        "a heavy body moved the kinematic body"
    );
    let face = world.get::<Position>(body).unwrap().0.x + 12.0;
    let ball_x = world.get::<Position>(ball).unwrap().0.x;
    assert!(
        ball_x - 10.0 > face - 1.0,
        "the ball sank into the kinematic body: ball {ball_x}, face {face}"
    );
    assert!(
        world.get::<Velocity>(ball).unwrap().0.x > 100.0,
        "the ball was not pushed along"
    );
    let heavy_y = world.get::<Position>(heavy).unwrap().0.y;
    let top = world.get::<Position>(lifter).unwrap().0.y - 12.0;
    assert!(
        heavy_y + 10.0 < top + 1.0,
        "the heavy body sank into the lifter: heavy {heavy_y}, top {top}"
    );
}

#[test]
fn kinematic_contact_event_names_the_dynamic_body_as_a() {
    for kinematic_first in [true, false] {
        let mut world = seed_world();
        let spawn = |world: &mut World| {
            spawn_kinematic(
                world,
                Collider::aabb(Vec2::splat(12.0)),
                Vec2::ZERO,
                Vec2::new(150.0, 0.0),
            )
        };
        let (body, ball) = if kinematic_first {
            let body = spawn(&mut world);
            (body, spawn_ball(&mut world, Vec2::new(40.0, 0.0)))
        } else {
            let ball = spawn_ball(&mut world, Vec2::new(40.0, 0.0));
            (spawn(&mut world), ball)
        };
        let mut seen = 0;
        for _ in 0..30 {
            physics_step(&mut world);
            for event in drain_events(&mut world) {
                assert_eq!(
                    event.a, ball,
                    "a is not the pushed body (kinematic first: {kinematic_first})"
                );
                assert_eq!(event.b, Some(body));
                assert!(
                    event.normal.x > 0.99,
                    "normal {:?} is not the push (kinematic first: {kinematic_first})",
                    event.normal
                );
                seen += 1;
            }
        }
        assert!(
            seen > 0,
            "no contact event (kinematic first: {kinematic_first})"
        );
    }
}

#[test]
fn kinematic_body_passes_static_bodies_tiles_and_kinematic_bodies() {
    let mut world = seed_world();
    world.get_resource_mut::<TilemapRegistry>().unwrap().insert(
        "strip".into(),
        TilemapData {
            tile_width: 16,
            tile_height: 16,
            width: 10,
            height: 1,
            tileset: vec!["solid".into()],
            object_layers: Vec::new(),
            terrain: TerrainProps::default(),
            layers: vec![TilemapLayer {
                name: "solid".into(),
                kind: LayerKind::Collision,
                tiles: vec![-1, -1, -1, -1, -1, 0, -1, -1, -1, -1],
            }],
        },
    );
    let map = world.spawn();
    world.insert(map, TilemapInstance::new("strip", Vec2::ZERO));
    let wall = world.spawn();
    world.insert(wall, Position(Vec2::new(40.0, 8.0)));
    world.insert(wall, Collider::aabb(Vec2::splat(8.0)));
    world.insert(wall, RigidBody::r#static());
    let collider = Collider::aabb(Vec2::splat(6.0));
    let mover = (Vec2::new(0.0, 8.0), Vec2::new(150.0, 0.0));
    let other = (Vec2::new(120.0, 8.0), Vec2::new(-50.0, 0.0));
    let body = spawn_kinematic(&mut world, collider, mover.0, mover.1);
    let oncoming = spawn_kinematic(&mut world, Collider::circle(6.0), other.0, other.1);
    for step in 0..60 {
        physics_step(&mut world);
        assert!(
            drain_events(&mut world).is_empty(),
            "a kinematic body sent an event at step {step}"
        );
        assert_eq!(
            world
                .get_resource::<PhysicsBuffers>()
                .unwrap()
                .contact_count(),
            0,
            "a kinematic body made a contact at step {step}"
        );
    }
    assert_eq!(
        position_bits(&world, body),
        kinematic_alone(collider, mover.0, mover.1, Vec2::ZERO, 60)
    );
    assert_eq!(
        position_bits(&world, oncoming),
        kinematic_alone(Collider::circle(6.0), other.0, other.1, Vec2::ZERO, 60)
    );
    assert!(
        world.get::<Position>(body).unwrap().0.x > 140.0,
        "the kinematic body stopped"
    );
}

#[test]
fn kinematic_body_moving_into_a_sleeping_body_wakes_it() {
    let mut world = seed_world();
    let body = spawn_kinematic(
        &mut world,
        Collider::aabb(Vec2::splat(12.0)),
        Vec2::ZERO,
        Vec2::ZERO,
    );
    let ball = spawn_ball(&mut world, Vec2::new(60.0, 0.0));
    for _ in 0..60 {
        physics_step(&mut world);
    }
    assert!(is_sleeping(&world, ball), "the idle ball never slept");
    world.get_mut::<Velocity>(body).unwrap().0 = Vec2::new(150.0, 0.0);
    let mut woke = false;
    for step in 0..60 {
        physics_step(&mut world);
        woke |= !is_sleeping(&world, ball);
        let face = world.get::<Position>(body).unwrap().0.x + 12.0;
        let ball_x = world.get::<Position>(ball).unwrap().0.x;
        assert!(
            ball_x > face,
            "passed the ball at step {step}: ball {ball_x}, face {face}"
        );
    }
    assert!(woke, "the kinematic body never woke the ball");
    assert!(
        world.get::<Position>(ball).unwrap().0.x > 150.0,
        "the ball was not pushed"
    );
}

#[test]
fn kinematic_body_moving_away_from_a_resting_body_wakes_it() {
    let mut world = seed_world();
    world.get_resource_mut::<PhysicsConfig>().unwrap().gravity = Vec2::new(0.0, 900.0);
    let platform = spawn_kinematic(
        &mut world,
        Collider::aabb(Vec2::new(40.0, 8.0)),
        Vec2::new(0.0, 100.0),
        Vec2::ZERO,
    );
    let rider = spawn_body(
        &mut world,
        RigidBody::dynamic(),
        Collider::aabb(Vec2::splat(8.0)),
        Vec2::new(0.0, 84.2),
        Vec2::ZERO,
    );
    for _ in 0..150 {
        physics_step(&mut world);
    }
    assert!(
        is_sleeping(&world, rider),
        "the rider never slept on the still platform"
    );
    let rest_y = world.get::<Position>(rider).unwrap().0.y;
    // The platform drops away, below the sleep threshold.
    world.get_mut::<Velocity>(platform).unwrap().0 = Vec2::new(0.0, 10.0);
    physics_step(&mut world);
    assert!(
        !is_sleeping(&world, rider),
        "the platform left without waking the rider"
    );
    for _ in 0..59 {
        physics_step(&mut world);
    }
    let rider_y = world.get::<Position>(rider).unwrap().0.y;
    assert!(
        rider_y > rest_y + 9.0,
        "the rider stayed behind: {rest_y} to {rider_y}"
    );
}

/// A kinematic box at `speed` walks into a ball asleep 2 px ahead of it; the
/// ball's centre stays ahead of the box's leading face throughout.
fn kinematic_walks_into_a_sleeper(speed: f32) {
    let mut world = seed_world();
    let body = spawn_kinematic(
        &mut world,
        Collider::aabb(Vec2::splat(12.0)),
        Vec2::ZERO,
        Vec2::ZERO,
    );
    let ball = spawn_ball(&mut world, Vec2::new(24.0, 0.0));
    for _ in 0..60 {
        physics_step(&mut world);
    }
    assert!(is_sleeping(&world, ball), "the idle ball never slept");
    world.get_mut::<Velocity>(body).unwrap().0 = Vec2::new(speed, 0.0);
    let mut woke = false;
    for step in 0..240 {
        physics_step(&mut world);
        woke |= !is_sleeping(&world, ball);
        let face = world.get::<Position>(body).unwrap().0.x + 12.0;
        let ball_x = world.get::<Position>(ball).unwrap().0.x;
        assert!(
            ball_x > face,
            "at {speed} px/s the box passed the ball at step {step}: ball {ball_x}, face {face}"
        );
    }
    assert!(woke, "at {speed} px/s the box never woke the ball");
    let ball_x = world.get::<Position>(ball).unwrap().0.x;
    assert!(
        ball_x > 24.0 + 4.0 * speed - 4.0,
        "at {speed} px/s the ball was not pushed: {ball_x}"
    );
}

#[test]
fn kinematic_body_wakes_a_sleeper_at_half_the_sleep_threshold() {
    assert_eq!(PhysicsConfig::default().sleep_threshold, 20.0);
    kinematic_walks_into_a_sleeper(10.0);
}

#[test]
fn kinematic_body_wakes_a_sleeper_at_the_sleep_threshold() {
    kinematic_walks_into_a_sleeper(20.0);
}

/// A box resting on a kinematic platform that rises at 10 px/s under
/// gravity stays awake for 2 s at `time_to_sleep`.
fn rides_a_rising_platform(time_to_sleep: f32) {
    let mut world = seed_world();
    {
        let config = world.get_resource_mut::<PhysicsConfig>().unwrap();
        config.gravity = Vec2::new(0.0, 900.0);
        config.time_to_sleep = time_to_sleep;
    }
    let platform = spawn_kinematic(
        &mut world,
        Collider::aabb(Vec2::new(40.0, 8.0)),
        Vec2::new(0.0, 100.0),
        Vec2::new(0.0, -10.0),
    );
    let rider = spawn_body(
        &mut world,
        RigidBody::dynamic(),
        Collider::aabb(Vec2::splat(8.0)),
        Vec2::new(0.0, 84.2),
        Vec2::ZERO,
    );
    for step in 0..120 {
        physics_step(&mut world);
        assert!(
            !is_sleeping(&world, rider),
            "slept on the rising platform at step {step} (time_to_sleep {time_to_sleep})"
        );
    }
    let top = world.get::<Position>(platform).unwrap().0.y - 8.0;
    let bottom = world.get::<Position>(rider).unwrap().0.y + 8.0;
    assert!(
        (top - bottom).abs() < 1.0,
        "the rider left the platform: top {top}, bottom {bottom}"
    );
}

#[test]
fn body_on_a_rising_kinematic_platform_stays_awake() {
    rides_a_rising_platform(PhysicsConfig::default().time_to_sleep);
}

#[test]
fn body_on_a_rising_kinematic_platform_stays_awake_when_time_to_sleep_is_under_a_step() {
    rides_a_rising_platform(0.01);
}

#[test]
fn body_on_a_rising_kinematic_platform_stays_awake_when_time_to_sleep_is_zero() {
    rides_a_rising_platform(0.0);
}

#[test]
fn body_on_a_still_kinematic_body_falls_asleep() {
    let mut world = seed_world();
    world.get_resource_mut::<PhysicsConfig>().unwrap().gravity = Vec2::new(0.0, 900.0);
    spawn_kinematic(
        &mut world,
        Collider::aabb(Vec2::new(40.0, 8.0)),
        Vec2::new(0.0, 100.0),
        Vec2::ZERO,
    );
    let rider = spawn_body(
        &mut world,
        RigidBody::dynamic(),
        Collider::aabb(Vec2::splat(8.0)),
        Vec2::new(0.0, 84.2),
        Vec2::ZERO,
    );
    for _ in 0..150 {
        physics_step(&mut world);
    }
    assert!(
        is_sleeping(&world, rider),
        "a still kinematic body kept the rider awake"
    );
}

#[test]
fn persistent_pairs_match_fresh_contacts_with_kinematic_bodies() {
    use crate::Pcg32;

    let mut checked = 0;
    for seed in 0..4 {
        let mut rng = Pcg32::seeded(0x0049 + seed);
        let mut world = seed_world();
        let (bottom, top) = spawn_sleeping_stack(&mut world);
        settle_to_sleep(&mut world, &[bottom, top]);
        let buffers = world.get_resource_mut::<PhysicsBuffers>().unwrap();
        buffers.check_pair_contacts = true;
        buffers.reference_impulses = Some(buffers.impulses.clone());
        let config = world.get_resource_mut::<PhysicsConfig>().unwrap();
        config.substeps = [1, 2, 4, 8][seed as usize % 4];
        config.broadphase_cell_size = [8.0, 16.0, 32.0][seed as usize % 3];
        for i in 0..30 {
            let position = Vec2::new(
                (i % 6) as f32 * 11.0 - 28.0 + rng.next_range(-1.0, 1.0),
                65.0 - (i / 6) as f32 * 11.0 + rng.next_range(-1.0, 1.0),
            );
            let collider = if i % 3 == 0 {
                Collider::aabb(Vec2::splat(5.0))
            } else {
                Collider::circle(5.0)
            };
            let speed = rng.next_range(0.0, 80.0);
            spawn_body(
                &mut world,
                RigidBody::dynamic(),
                collider,
                position,
                rng.next_unit_vec2() * speed,
            );
        }
        // Kinematic bodies cross the pile and the sleeping stack, one still.
        for i in 0..5 {
            let velocity = if i == 0 {
                Vec2::ZERO
            } else {
                rng.next_unit_vec2() * rng.next_range(20.0, 240.0)
            };
            let collider = if i % 2 == 0 {
                Collider::aabb(Vec2::new(12.0, 4.0))
            } else {
                Collider::circle(7.0)
            };
            let start = Vec2::new(rng.next_range(-60.0, 60.0), rng.next_range(0.0, 110.0));
            spawn_kinematic(&mut world, collider, start, velocity);
        }
        for _ in 0..40 {
            physics_step(&mut world);
            drain_events(&mut world);
        }
        checked += world
            .get_resource::<PhysicsBuffers>()
            .unwrap()
            .checked_substeps;
    }
    assert_eq!(
        checked, 600,
        "every substep must run the equivalence oracle"
    );
}

#[test]
fn collider_less_kinematic_body_moves_by_its_velocity() {
    let mut world = seed_world();
    world.get_resource_mut::<PhysicsConfig>().unwrap().gravity = Vec2::new(0.0, 900.0);
    let velocity = Vec2::new(60.0, -30.0);
    let loose = world.spawn();
    world.insert(loose, Position(Vec2::ZERO));
    world.insert(loose, Velocity(velocity));
    world.insert(loose, RigidBody::kinematic());
    for _ in 0..30 {
        physics_step(&mut world);
    }
    let position = world.get::<Position>(loose).unwrap().0;
    assert!(
        (position - velocity * 0.5).length() < 1.0e-3,
        "half a second at {velocity:?} ended at {position:?}"
    );
    assert_eq!(world.get::<Velocity>(loose).unwrap().0, velocity);
}

#[test]
fn proxy_stays_80_bytes_with_the_kinematic_flag() {
    assert_eq!(std::mem::size_of::<Proxy>(), 80);
}

/// Every sensor event sent by the last step, flushing the queue.
fn drain_sensor_events(world: &mut World) -> Vec<SensorEntered> {
    let events = world
        .get_resource_mut::<EventQueue<SensorEntered>>()
        .unwrap();
    let sent = events.iter_current().copied().collect();
    events.flush();
    sent
}

fn spawn_sensor(world: &mut World, collider: Collider, position: Vec2, velocity: Vec2) -> Entity {
    spawn_body(world, RigidBody::sensor(), collider, position, velocity)
}

fn spawn_static(world: &mut World, collider: Collider, position: Vec2) -> Entity {
    let entity = world.spawn();
    world.insert(entity, Position(position));
    world.insert(entity, collider);
    world.insert(entity, RigidBody::r#static());
    entity
}

/// 16 px tiles, `width` wide, every layer a collision layer.
fn map_data(width: u32, layers: Vec<Vec<i32>>) -> TilemapData {
    TilemapData {
        tile_width: 16,
        tile_height: 16,
        width,
        height: layers[0].len() as u32 / width,
        tileset: vec!["solid".into()],
        object_layers: Vec::new(),
        terrain: TerrainProps::default(),
        layers: layers
            .into_iter()
            .enumerate()
            .map(|(index, tiles)| TilemapLayer {
                name: format!("solid{index}"),
                kind: LayerKind::Collision,
                tiles,
            })
            .collect(),
    }
}

fn spawn_map(
    world: &mut World,
    id: &str,
    origin: Vec2,
    width: u32,
    layers: Vec<Vec<i32>>,
) -> Entity {
    world
        .get_resource_mut::<TilemapRegistry>()
        .unwrap()
        .insert(id.into(), map_data(width, layers));
    let map = world.spawn();
    world.insert(map, TilemapInstance::new(id, origin));
    map
}

#[test]
fn sensor_moves_by_its_velocity_and_bodies_pass_through_it() {
    let collider = Collider::aabb(Vec2::splat(12.0));
    let velocity = Vec2::new(60.0, 0.0);
    let alone = {
        let mut world = seed_world();
        let sensor = spawn_sensor(&mut world, collider, Vec2::ZERO, velocity);
        for _ in 0..60 {
            physics_step(&mut world);
        }
        position_bits(&world, sensor)
    };
    let mut world = seed_world();
    let sensor = spawn_sensor(&mut world, collider, Vec2::ZERO, velocity);
    let ball = spawn_body(
        &mut world,
        RigidBody::dynamic(),
        Collider::circle(10.0),
        Vec2::new(100.0, 0.0),
        Vec2::new(-150.0, 0.0),
    );
    let mut entered = Vec::new();
    for step in 0..60 {
        physics_step(&mut world);
        assert!(
            drain_events(&mut world).is_empty(),
            "a sensor made a contact at step {step}"
        );
        entered.extend(drain_sensor_events(&mut world));
    }
    assert_eq!(
        position_bits(&world, sensor),
        alone,
        "the ball moved the sensor"
    );
    assert_eq!(entered, [SensorEntered::new(sensor, Some(ball))]);
    let ball_x = world.get::<Position>(ball).unwrap().0.x;
    assert!(
        (ball_x + 50.0).abs() < 1.0e-2,
        "the ball did not pass through: {ball_x}"
    );
    assert_eq!(
        world.get::<Velocity>(ball).unwrap().0,
        Vec2::new(-150.0, 0.0)
    );
}

#[test]
fn sensor_enters_once_per_overlap_with_every_body_kind_and_a_tile() {
    let mut world = seed_world();
    let sensor = spawn_sensor(
        &mut world,
        Collider::aabb(Vec2::splat(6.0)),
        Vec2::new(-20.0, 0.0),
        Vec2::new(120.0, 0.0),
    );
    let dynamic = spawn_body(
        &mut world,
        RigidBody::dynamic(),
        Collider::circle(5.0),
        Vec2::new(20.0, 0.0),
        Vec2::ZERO,
    );
    let kinematic = spawn_kinematic(
        &mut world,
        Collider::aabb(Vec2::splat(4.0)),
        Vec2::new(50.0, 0.0),
        Vec2::ZERO,
    );
    let fixed = spawn_static(
        &mut world,
        Collider::aabb(Vec2::splat(4.0)),
        Vec2::new(80.0, 0.0),
    );
    let sleeper = spawn_body(
        &mut world,
        RigidBody::dynamic(),
        Collider::circle(5.0),
        Vec2::new(110.0, 0.0),
        Vec2::ZERO,
    );
    // One solid tile, x 144 to 160 and y -8 to 8.
    let mut strip = vec![-1; 10];
    strip[9] = 0;
    spawn_map(&mut world, "strip", Vec2::new(0.0, -8.0), 10, vec![strip]);
    let mut entered = Vec::new();
    let mut asleep_at_entry = false;
    for _ in 0..120 {
        physics_step(&mut world);
        for event in drain_sensor_events(&mut world) {
            if event.other == Some(sleeper) {
                asleep_at_entry = is_sleeping(&world, sleeper);
            }
            entered.push(event);
        }
    }
    assert_eq!(
        entered.iter().map(|event| event.other).collect::<Vec<_>>(),
        [
            Some(dynamic),
            Some(kinematic),
            Some(fixed),
            Some(sleeper),
            None
        ]
    );
    assert!(entered.iter().all(|event| event.sensor == sensor));
    assert!(
        asleep_at_entry,
        "the sleeper woke before the sensor reached it"
    );
}

#[test]
fn sensor_enters_again_after_separating() {
    let mut world = seed_world();
    let sensor = spawn_sensor(
        &mut world,
        Collider::aabb(Vec2::splat(6.0)),
        Vec2::ZERO,
        Vec2::new(60.0, 0.0),
    );
    let target = spawn_static(
        &mut world,
        Collider::aabb(Vec2::splat(4.0)),
        Vec2::new(20.0, 0.0),
    );
    let mut entered = Vec::new();
    for step in 0..120 {
        if step == 60 {
            world.get_mut::<Velocity>(sensor).unwrap().0 = Vec2::new(-60.0, 0.0);
        }
        physics_step(&mut world);
        entered.extend(drain_sensor_events(&mut world));
    }
    assert_eq!(entered, [SensorEntered::new(sensor, Some(target)); 2]);
}

#[test]
fn sensor_does_not_detect_sensors() {
    let mut world = seed_world();
    let first = spawn_sensor(&mut world, Collider::circle(10.0), Vec2::ZERO, Vec2::ZERO);
    let second = spawn_sensor(
        &mut world,
        Collider::aabb(Vec2::splat(8.0)),
        Vec2::new(5.0, 0.0),
        Vec2::ZERO,
    );
    let body = spawn_static(
        &mut world,
        Collider::aabb(Vec2::splat(4.0)),
        Vec2::new(2.0, 0.0),
    );
    physics_step(&mut world);
    assert_eq!(
        drain_sensor_events(&mut world),
        [
            SensorEntered::new(first, Some(body)),
            SensorEntered::new(second, Some(body))
        ]
    );
    for _ in 0..10 {
        physics_step(&mut world);
        assert!(drain_sensor_events(&mut world).is_empty());
    }
}

#[test]
fn sensor_spawned_overlapping_a_body_reports_it_on_the_first_step() {
    let mut world = seed_world();
    let body = spawn_ball(&mut world, Vec2::ZERO);
    let sensor = world.spawn_with(RigidBodyBundle::sensor(
        Position::new(3.0, 0.0),
        Collider::circle(4.0),
    ));
    physics_step(&mut world);
    assert_eq!(
        drain_sensor_events(&mut world),
        [SensorEntered::new(sensor, Some(body))]
    );
}

#[test]
fn swapping_which_entity_is_the_sensor_sends_one_event_for_the_new_sensor() {
    let mut world = seed_world();
    let a = spawn_sensor(
        &mut world,
        Collider::aabb(Vec2::splat(8.0)),
        Vec2::ZERO,
        Vec2::ZERO,
    );
    let b = spawn_static(
        &mut world,
        Collider::aabb(Vec2::splat(8.0)),
        Vec2::new(4.0, 0.0),
    );
    physics_step(&mut world);
    assert_eq!(
        drain_sensor_events(&mut world),
        [SensorEntered::new(a, Some(b))]
    );
    *world.get_mut::<RigidBody>(a).unwrap() = RigidBody::r#static();
    *world.get_mut::<RigidBody>(b).unwrap() = RigidBody::sensor();
    physics_step(&mut world);
    assert_eq!(
        drain_sensor_events(&mut world),
        [SensorEntered::new(b, Some(a))]
    );
    physics_step(&mut world);
    assert!(drain_sensor_events(&mut world).is_empty());
}

#[test]
fn the_only_sensor_turned_into_a_body_and_back_reports_again() {
    let mut world = seed_world();
    let sensor = spawn_sensor(&mut world, Collider::circle(6.0), Vec2::ZERO, Vec2::ZERO);
    let target = spawn_static(
        &mut world,
        Collider::aabb(Vec2::splat(4.0)),
        Vec2::new(5.0, 0.0),
    );
    let overlap = [SensorEntered::new(sensor, Some(target))];
    physics_step(&mut world);
    assert_eq!(drain_sensor_events(&mut world), overlap);
    *world.get_mut::<RigidBody>(sensor).unwrap() = RigidBody::r#static();
    physics_step(&mut world);
    assert!(drain_sensor_events(&mut world).is_empty());
    *world.get_mut::<RigidBody>(sensor).unwrap() = RigidBody::sensor();
    physics_step(&mut world);
    assert_eq!(drain_sensor_events(&mut world), overlap);
}

#[test]
fn sensor_on_a_tile_of_a_moving_map_sends_one_event() {
    let mut world = seed_world();
    let map = spawn_map(&mut world, "moving", Vec2::ZERO, 1, vec![vec![0]]);
    let sensor = spawn_sensor(
        &mut world,
        Collider::aabb(Vec2::new(20.0, 4.0)),
        Vec2::new(8.0, 8.0),
        Vec2::ZERO,
    );
    let mut entered = Vec::new();
    for _ in 0..10 {
        physics_step(&mut world);
        entered.extend(drain_sensor_events(&mut world));
        world.get_mut::<TilemapInstance>(map).unwrap().origin.x += 0.5;
    }
    assert_eq!(entered, [SensorEntered::new(sensor, None)]);
}

#[test]
fn sensor_sees_another_layers_tile_at_the_same_cell_as_new() {
    let mut world = seed_world();
    spawn_map(&mut world, "layers", Vec2::ZERO, 1, vec![vec![0], vec![-1]]);
    let sensor = spawn_sensor(
        &mut world,
        Collider::circle(4.0),
        Vec2::new(8.0, 8.0),
        Vec2::ZERO,
    );
    let tile = [SensorEntered::new(sensor, None)];
    physics_step(&mut world);
    assert_eq!(drain_sensor_events(&mut world), tile);
    physics_step(&mut world);
    assert!(drain_sensor_events(&mut world).is_empty());
    // The cell's solid tile now comes from the other layer.
    world
        .get_resource_mut::<TilemapRegistry>()
        .unwrap()
        .insert("layers".into(), map_data(1, vec![vec![-1], vec![0]]));
    physics_step(&mut world);
    assert_eq!(drain_sensor_events(&mut world), tile);
}

#[test]
fn paused_clock_sends_no_sensor_events() {
    let mut world = seed_world();
    let sensor = spawn_sensor(
        &mut world,
        Collider::circle(6.0),
        Vec2::ZERO,
        Vec2::new(30.0, 0.0),
    );
    let target = spawn_static(
        &mut world,
        Collider::aabb(Vec2::splat(4.0)),
        Vec2::new(5.0, 0.0),
    );
    physics_step(&mut world);
    assert_eq!(
        drain_sensor_events(&mut world),
        [SensorEntered::new(sensor, Some(target))]
    );
    let position = position_bits(&world, sensor);
    world.get_resource_mut::<Time>().unwrap().advance_frame(0.0);
    for _ in 0..5 {
        physics_step(&mut world);
        assert!(drain_sensor_events(&mut world).is_empty());
    }
    assert_eq!(
        position_bits(&world, sensor),
        position,
        "a paused step moved the sensor"
    );
    world
        .get_resource_mut::<Time>()
        .unwrap()
        .advance_frame(1.0 / 60.0);
    physics_step(&mut world);
    assert!(
        drain_sensor_events(&mut world).is_empty(),
        "the pause forgot the pair"
    );
}

#[test]
fn sensor_events_follow_sensor_order_then_index_order() {
    let mut world = seed_world();
    let first = spawn_sensor(
        &mut world,
        Collider::aabb(Vec2::splat(30.0)),
        Vec2::new(8.0, 8.0),
        Vec2::ZERO,
    );
    let second = spawn_sensor(
        &mut world,
        Collider::circle(30.0),
        Vec2::new(8.0, 8.0),
        Vec2::ZERO,
    );
    let left = spawn_static(
        &mut world,
        Collider::aabb(Vec2::splat(4.0)),
        Vec2::new(-10.0, 0.0),
    );
    let right = spawn_static(
        &mut world,
        Collider::aabb(Vec2::splat(4.0)),
        Vec2::new(20.0, 0.0),
    );
    spawn_map(&mut world, "pair", Vec2::ZERO, 2, vec![vec![0, 0]]);
    physics_step(&mut world);
    let expected: Vec<_> = [first, second]
        .into_iter()
        .flat_map(|sensor| {
            [Some(left), Some(right), None, None].map(|other| SensorEntered::new(sensor, other))
        })
        .collect();
    assert_eq!(drain_sensor_events(&mut world), expected);
}

#[test]
fn identical_runs_send_identical_sensor_events() {
    use crate::Pcg32;

    let run = || {
        let mut world = seed_world();
        world.get_resource_mut::<PhysicsConfig>().unwrap().gravity = Vec2::new(0.0, 900.0);
        let mut rng = Pcg32::seeded(0x5E);
        spawn_map(
            &mut world,
            "ground",
            Vec2::new(-80.0, 100.0),
            10,
            vec![vec![0; 10]],
        );
        for _ in 0..20 {
            let position = Vec2::new(rng.next_range(-70.0, 70.0), rng.next_range(-60.0, 60.0));
            let velocity = rng.next_unit_vec2() * rng.next_range(0.0, 120.0);
            let collider = Collider::circle(rng.next_range(3.0, 7.0));
            spawn_body(
                &mut world,
                RigidBody::dynamic(),
                collider,
                position,
                velocity,
            );
        }
        for _ in 0..4 {
            let position = Vec2::new(rng.next_range(-70.0, 70.0), rng.next_range(-60.0, 90.0));
            let velocity = rng.next_unit_vec2() * rng.next_range(20.0, 90.0);
            let collider = Collider::aabb(Vec2::splat(rng.next_range(6.0, 20.0)));
            spawn_sensor(&mut world, collider, position, velocity);
        }
        let mut entered = Vec::new();
        for _ in 0..90 {
            physics_step(&mut world);
            entered.extend(drain_sensor_events(&mut world));
            drain_events(&mut world);
        }
        entered
    };
    let first = run();
    assert!(first.len() > 4, "only {} sensor events", first.len());
    assert_eq!(first, run());
}

#[test]
fn collider_less_sensor_moves_by_its_velocity() {
    let mut world = seed_world();
    world.get_resource_mut::<PhysicsConfig>().unwrap().gravity = Vec2::new(0.0, 900.0);
    let velocity = Vec2::new(60.0, -30.0);
    let loose = world.spawn();
    world.insert(loose, Position(Vec2::ZERO));
    world.insert(loose, Velocity(velocity));
    world.insert(loose, RigidBody::sensor());
    for _ in 0..30 {
        physics_step(&mut world);
    }
    let position = world.get::<Position>(loose).unwrap().0;
    assert!(
        (position - velocity * 0.5).length() < 1.0e-3,
        "half a second at {velocity:?} ended at {position:?}"
    );
    assert_eq!(world.get::<Velocity>(loose).unwrap().0, velocity);
}

/// Substeps run by one step of a frame of `dt` under `config`, in a world
/// `spawn` fills.
fn substeps_in_one_step(config: PhysicsConfig, dt: f32, spawn: impl Fn(&mut World)) -> usize {
    let mut world = seed_world();
    *world.get_resource_mut::<PhysicsConfig>().unwrap() = config;
    spawn(&mut world);
    world.get_resource_mut::<Time>().unwrap().advance_frame(dt);
    physics_step(&mut world);
    world.get_resource::<PhysicsBuffers>().unwrap().substeps_run
}

#[test]
fn kinematic_world_runs_more_substeps_on_steps_over_a_sixtieth() {
    let default = PhysicsConfig::default();
    assert_eq!(default.substeps, 4);
    let bounded = PhysicsConfig {
        max_step_dt: 0.05,
        ..default
    };
    let single = PhysicsConfig {
        substeps: 1,
        ..default
    };
    // A still kinematic body counts as a moving one does.
    for velocity in [Vec2::new(30.0, 0.0), Vec2::ZERO] {
        let spawn = |world: &mut World| {
            spawn_ball(world, Vec2::new(60.0, 0.0));
            spawn_kinematic(
                world,
                Collider::aabb(Vec2::splat(12.0)),
                Vec2::ZERO,
                velocity,
            );
        };
        for (config, dt, expected) in [
            (default, 1.0 / 120.0, 4),
            (default, 1.0 / 60.0, 4),
            (default, 1.0 / 45.0, 6),
            (default, 1.0 / 30.0, 8),
            (default, 0.1, 8),
            (bounded, 0.05, 8),
            (single, 1.0 / 30.0, 2),
        ] {
            assert_eq!(
                substeps_in_one_step(config, dt, spawn),
                expected,
                "{} substeps, bound {}, dt {dt}, kinematic velocity {velocity:?}",
                config.substeps,
                config.max_step_dt
            );
        }
    }
}

#[test]
fn world_without_a_kinematic_proxy_keeps_its_substeps_on_long_steps() {
    let spawn = |world: &mut World| {
        spawn_static(
            world,
            Collider::aabb(Vec2::new(100.0, 8.0)),
            Vec2::new(0.0, 50.0),
        );
        spawn_map(world, "floor", Vec2::new(-32.0, 70.0), 4, vec![vec![0; 4]]);
        spawn_ball(world, Vec2::ZERO);
        spawn_sensor(
            world,
            Collider::circle(8.0),
            Vec2::new(40.0, 0.0),
            Vec2::new(30.0, 0.0),
        );
        // A kinematic body without a collider is no proxy.
        let loose = world.spawn();
        world.insert(loose, Position(Vec2::new(-40.0, 0.0)));
        world.insert(loose, Velocity(Vec2::new(30.0, 0.0)));
        world.insert(loose, RigidBody::kinematic());
    };
    for dt in [1.0 / 30.0, 0.1] {
        assert_eq!(
            substeps_in_one_step(PhysicsConfig::default(), dt, spawn),
            4,
            "dt {dt}"
        );
    }
}

/// A kinematic box walks at 10 px/s for 4 s, at frame dt `dt`, into a ball
/// asleep 1 px ahead of it with a row of `sleepers` more ahead of that ball,
/// each asleep 1 px from the last. The pressed ball never sinks more than
/// 1 px into the box; returns the step at which each of the row first woke.
fn kinematic_press_into_sleepers(dt: f32, sleepers: usize) -> Vec<Option<usize>> {
    let mut world = seed_world();
    let body = spawn_kinematic(
        &mut world,
        Collider::aabb(Vec2::splat(12.0)),
        Vec2::ZERO,
        Vec2::ZERO,
    );
    let pressed = spawn_ball(&mut world, Vec2::new(23.0, 0.0));
    let row: Vec<Entity> = (1..=sleepers)
        .map(|i| spawn_ball(&mut world, Vec2::new(23.0 + 21.0 * i as f32, 0.0)))
        .collect();
    for _ in 0..60 {
        physics_step(&mut world);
    }
    assert!(
        is_sleeping(&world, pressed) && row.iter().all(|&ball| is_sleeping(&world, ball)),
        "the idle row never slept"
    );
    world.get_mut::<Velocity>(body).unwrap().0 = Vec2::new(10.0, 0.0);
    world.get_resource_mut::<Time>().unwrap().advance_frame(dt);
    let mut woke = vec![None; sleepers];
    for step in 0..(4.0 / dt).round() as usize {
        physics_step(&mut world);
        drain_events(&mut world);
        let face = world.get::<Position>(body).unwrap().0.x + 12.0;
        let sink = face - (world.get::<Position>(pressed).unwrap().0.x - 10.0);
        assert!(
            sink <= 1.0,
            "at dt {dt} the pressed ball sank {sink} px into the box at step {step}"
        );
        for (first, &ball) in woke.iter_mut().zip(&row) {
            if first.is_none() && !is_sleeping(&world, ball) {
                *first = Some(step);
            }
        }
    }
    woke
}

#[test]
fn kinematic_press_wakes_the_sleeper_ahead_of_the_pressed_body() {
    for dt in [1.0 / 60.0, 1.0 / 30.0] {
        let woke = kinematic_press_into_sleepers(dt, 1);
        assert!(woke[0].is_some(), "at dt {dt} the sleeper ahead never woke");
    }
}

#[test]
fn kinematic_press_wakes_a_row_of_sleepers_in_order() {
    for dt in [1.0 / 60.0, 1.0 / 30.0] {
        let woke = kinematic_press_into_sleepers(dt, 3);
        assert!(
            woke.iter().all(Option::is_some),
            "at dt {dt} a sleeper never woke: {woke:?}"
        );
        assert!(
            woke.windows(2).all(|pair| pair[0] <= pair[1]),
            "at dt {dt} the row woke out of order: {woke:?}"
        );
    }
}

/// A ball moving at 10 px/s, half the sleep threshold, arrives at a ball
/// asleep ahead of it; the sleeper stays asleep for 2 s.
fn slow_arrival_leaves_the_sleeper_asleep(world: &mut World, mover: Entity, sleeper: Entity) {
    for _ in 0..60 {
        physics_step(world);
    }
    assert!(
        is_sleeping(world, mover) && is_sleeping(world, sleeper),
        "the idle bodies never slept"
    );
    world.get_mut::<Velocity>(mover).unwrap().0 = Vec2::new(10.0, 0.0);
    let start = world.get::<Position>(mover).unwrap().0.x;
    for step in 0..120 {
        physics_step(world);
        drain_events(world);
        assert!(
            is_sleeping(world, sleeper),
            "the slow arrival woke the sleeper at step {step}"
        );
    }
    assert!(
        world.get::<Position>(mover).unwrap().0.x > start + 0.5,
        "the slow body never moved"
    );
}

#[test]
fn slow_body_without_a_kinematic_press_leaves_a_sleeper_asleep() {
    let mut world = seed_world();
    let mover = spawn_ball(&mut world, Vec2::new(23.0, 0.0));
    let sleeper = spawn_ball(&mut world, Vec2::new(44.0, 0.0));
    slow_arrival_leaves_the_sleeper_asleep(&mut world, mover, sleeper);
}

#[test]
fn still_kinematic_body_carries_no_wake_through_a_body_touching_it() {
    // Both boxes rest on a still kinematic platform under gravity.
    let mut world = seed_world();
    world.get_resource_mut::<PhysicsConfig>().unwrap().gravity = Vec2::new(0.0, 900.0);
    spawn_kinematic(
        &mut world,
        Collider::aabb(Vec2::new(100.0, 8.0)),
        Vec2::new(0.0, 100.0),
        Vec2::ZERO,
    );
    let rest = |world: &mut World, x: f32| {
        spawn_body(
            world,
            RigidBody::dynamic(),
            Collider::aabb(Vec2::splat(8.0)),
            Vec2::new(x, 84.2),
            Vec2::ZERO,
        )
    };
    let mover = rest(&mut world, 0.0);
    let sleeper = rest(&mut world, 17.0);
    slow_arrival_leaves_the_sleeper_asleep(&mut world, mover, sleeper);
}

/// The static floor `spawn_sleeping_stack` rests its balls on.
fn stack_floor(world: &World) -> Entity {
    world
        .query_filtered::<Entity, (crate::ecs::With<Collider>, crate::ecs::Without<Velocity>)>()
        .next()
        .expect("the stack's floor")
}

#[test]
fn a_static_support_that_goes_or_moves_wakes_the_islands_on_it() {
    // D-065's wake paths run through dynamic members: statics hold no sleep
    // entry, so a floor could go or move under a frozen island, which then
    // hung in the air.
    for change in 0..3 {
        let mut world = seed_world();
        let (bottom, top) = spawn_sleeping_stack(&mut world);
        let far = world.spawn();
        world.insert(far, Position(Vec2::new(900.0, 110.0)));
        world.insert(far, Collider::aabb(Vec2::new(16.0, 8.0)));
        world.insert(far, RigidBody::r#static());
        settle_to_sleep(&mut world, &[bottom, top]);

        // A support far from the island moving wakes nothing.
        world.get_mut::<Position>(far).unwrap().0.x += 50.0;
        physics_step(&mut world);
        let buffers = world.get_resource::<PhysicsBuffers>().unwrap();
        assert!(buffers.is_sleeping(bottom) && buffers.is_sleeping(top));

        let floor = stack_floor(&world);
        match change {
            0 => world.despawn(floor),
            1 => world.get_mut::<Position>(floor).unwrap().0.y += 40.0,
            _ => world.get_mut::<Position>(floor).unwrap().0.x += 500.0,
        }
        let resting = world.get::<Position>(bottom).unwrap().0.y;
        for _ in 0..10 {
            physics_step(&mut world);
        }
        let buffers = world.get_resource::<PhysicsBuffers>().unwrap();
        assert!(
            !buffers.is_sleeping(bottom) && !buffers.is_sleeping(top),
            "change {change}: the island stayed asleep"
        );
        assert!(
            world.get::<Position>(bottom).unwrap().0.y > resting + 1.0,
            "change {change}: the bottom ball did not fall"
        );
    }
}

#[test]
fn a_tile_support_removed_wakes_the_island_on_it() {
    let mut world = seed_world();
    world.get_resource_mut::<PhysicsConfig>().unwrap().gravity = Vec2::new(0.0, 900.0);
    let floor = |tiles: Vec<i32>| TilemapData {
        tile_width: 16,
        tile_height: 16,
        width: 4,
        height: 1,
        tileset: vec!["solid".into()],
        object_layers: Vec::new(),
        terrain: TerrainProps::default(),
        layers: vec![TilemapLayer {
            name: "collision".into(),
            kind: LayerKind::Collision,
            tiles,
        }],
    };
    world
        .get_resource_mut::<TilemapRegistry>()
        .unwrap()
        .insert("floor".into(), floor(vec![0, 0, 0, 0]));
    let map = world.spawn();
    world.insert(map, TilemapInstance::new("floor", Vec2::new(0.0, 16.0)));
    let ball = world.spawn();
    world.insert(ball, Position(Vec2::new(24.0, 9.0)));
    world.insert(ball, Velocity(Vec2::ZERO));
    world.insert(ball, Collider::circle(6.0));
    world.insert(ball, RigidBody::dynamic().with_restitution(0.0));
    settle_to_sleep(&mut world, &[ball]);

    // The map replaced without the tiles under the ball.
    world
        .get_resource_mut::<TilemapRegistry>()
        .unwrap()
        .insert("floor".into(), floor(vec![0, -1, -1, 0]));
    let resting = world.get::<Position>(ball).unwrap().0.y;
    for _ in 0..10 {
        physics_step(&mut world);
    }
    assert!(
        !world
            .get_resource::<PhysicsBuffers>()
            .unwrap()
            .is_sleeping(ball),
        "the ball stayed asleep over the gap"
    );
    assert!(world.get::<Position>(ball).unwrap().0.y > resting + 1.0);
}

#[test]
fn a_time_to_sleep_of_zero_never_sleeps_a_moving_body() {
    // A body above the threshold has a timer of 0, and `0 < 0` is false: at
    // `time_to_sleep` 0 a body at 1,000 px/s slept mid-flight.
    let mut world = seed_world();
    world
        .get_resource_mut::<PhysicsConfig>()
        .unwrap()
        .time_to_sleep = 0.0;
    let moving = world.spawn();
    world.insert(moving, Position(Vec2::ZERO));
    world.insert(moving, Velocity(Vec2::new(1_000.0, 0.0)));
    world.insert(moving, Collider::circle(4.0));
    world.insert(moving, RigidBody::dynamic());
    let still = world.spawn();
    world.insert(still, Position(Vec2::new(0.0, 500.0)));
    world.insert(still, Velocity(Vec2::ZERO));
    world.insert(still, Collider::circle(4.0));
    world.insert(still, RigidBody::dynamic());
    physics_step(&mut world);
    let buffers = world.get_resource::<PhysicsBuffers>().unwrap();
    assert!(!buffers.is_sleeping(moving), "a moving body fell asleep");
    assert!(buffers.is_sleeping(still), "a still body sleeps at once");
    assert_eq!(world.get::<Velocity>(moving).unwrap().0.x, 1_000.0);
}

/// Layer 1 alone, meeting only layer 1: a body that meets nothing at the
/// defaults.
fn layer_one(body: RigidBody) -> RigidBody {
    body.with_layers(1 << 1).with_mask(1 << 1)
}

/// A 10 px ball moving at 150 px/s for 1 s at a ball at rest 60 px ahead:
/// the collision events sent and where the mover and the resting ball end.
fn layered_crossing(mover: RigidBody, rest: RigidBody) -> (usize, f32, f32) {
    let mut world = seed_world();
    let ball = spawn_body(
        &mut world,
        mover,
        Collider::circle(10.0),
        Vec2::ZERO,
        Vec2::new(150.0, 0.0),
    );
    let target = spawn_body(
        &mut world,
        rest,
        Collider::circle(10.0),
        Vec2::new(60.0, 0.0),
        Vec2::ZERO,
    );
    let mut events = 0;
    for _ in 0..60 {
        physics_step(&mut world);
        events += drain_events(&mut world).len();
    }
    let x = |entity: Entity| world.get::<Position>(entity).unwrap().0.x;
    (events, x(ball), x(target))
}

/// The crossing passed: no event, the mover kept its speed and the resting
/// ball stayed.
fn assert_passed(mover: RigidBody, rest: RigidBody) {
    let (events, mover_x, rest_x) = layered_crossing(mover, rest);
    assert_eq!(events, 0, "{mover:?} met {rest:?}");
    assert!(
        (mover_x - 150.0).abs() < 1.0e-2,
        "the mover was held at {mover_x}"
    );
    assert_eq!(rest_x, 60.0, "the resting ball moved");
}

#[test]
fn layers_bodies_whose_layers_do_not_meet_pass_through_each_other() {
    let (events, mover_x, rest_x) = layered_crossing(RigidBody::dynamic(), RigidBody::dynamic());
    assert!(
        events > 0 && mover_x < rest_x,
        "the default balls did not meet"
    );
    assert_passed(layer_one(RigidBody::dynamic()), RigidBody::dynamic());
    assert_passed(RigidBody::dynamic(), layer_one(RigidBody::dynamic()));
}

#[test]
fn layers_an_asymmetric_pair_does_not_touch() {
    // The default ball's mask holds layer 1; the layer-1 ball's lacks layer 0.
    let default = RigidBody::dynamic();
    let one_sided = layer_one(RigidBody::dynamic());
    assert!(default.mask & one_sided.layers != 0 && !default.meets(&one_sided));
    assert_passed(default, one_sided);
    assert_passed(one_sided, default);
    // Both masks holding the other's layer is a contact.
    let both = RigidBody::dynamic().with_layers(1 << 1);
    assert!(both.meets(&default));
    let (events, mover_x, rest_x) = layered_crossing(both, default);
    assert!(events > 0 && mover_x < rest_x);
}

/// A 4 px box falling at `speed`, from y 0 for 0.5 s, toward a tile floor at
/// y 96 and a static box at y 292: its end height and the events sent.
fn fall_through_floors(body: RigidBody, speed: f32) -> (f32, usize) {
    let mut world = seed_world();
    spawn_map(
        &mut world,
        "floor",
        Vec2::new(0.0, 96.0),
        6,
        vec![vec![0; 6]],
    );
    spawn_static(
        &mut world,
        Collider::aabb(Vec2::new(50.0, 8.0)),
        Vec2::new(40.0, 300.0),
    );
    let faller = spawn_body(
        &mut world,
        body,
        Collider::aabb(Vec2::splat(4.0)),
        Vec2::new(40.0, 0.0),
        Vec2::new(0.0, speed),
    );
    let mut events = 0;
    for _ in 0..30 {
        physics_step(&mut world);
        events += drain_events(&mut world).len();
    }
    (world.get::<Position>(faller).unwrap().0.y, events)
}

#[test]
fn layers_a_mask_without_layer_0_falls_through_tiles_and_a_static_box() {
    // 1,200 px/s is 5 px a substep, past the 4 px half extent, so the safety
    // net sweeps the body against every static it reaches.
    let speed = 1_200.0;
    let substep_travel = speed / 60.0 / PhysicsConfig::default().substeps as f32;
    assert!(substep_travel > 4.0);
    let (y, events) = fall_through_floors(RigidBody::dynamic(), speed);
    assert!(
        y + 4.0 <= 96.5 && events > 0,
        "the default body ended at {y}"
    );
    let ghost = RigidBody::dynamic().with_mask(RigidBody::ALL_LAYERS & !RigidBody::DEFAULT_LAYERS);
    let (y, events) = fall_through_floors(ghost, speed);
    assert_eq!(events, 0);
    assert!((y - speed * 0.5).abs() < 1.0e-2, "the body was held at {y}");
}

/// A 6 × 10 map of 16 px tiles: an upper floor on row 2 in collision layer
/// `solid1`, in physics layer 1 meeting layer 1 alone, and a lower floor on
/// row 8 in `solid0`, at the defaults.
fn two_floor_map(world: &mut World) {
    let mut lower = vec![-1; 60];
    lower[48..54].fill(0);
    let mut upper = vec![-1; 60];
    upper[12..18].fill(0);
    let mut map = map_data(6, vec![lower, upper]);
    map.set_layer_physics(
        1,
        crate::assets::LayerPhysics {
            layers: 1 << 1,
            mask: 1 << 1,
        },
    );
    world
        .resource_mut::<TilemapRegistry>()
        .insert("floors".into(), map);
    let instance = world.spawn();
    world.insert(instance, TilemapInstance::new("floors", Vec2::ZERO));
}

#[test]
fn a_tile_layer_meets_bodies_by_its_layers_and_mask() {
    // Each 8 px box falls at 120 px/s for 2 s from y 8: it stops on the upper
    // floor's top (y 32) when the two meet, else on the lower's (y 128).
    let cases = [
        ("the default body", RigidBody::dynamic(), 128.0),
        (
            "a body in layer 1",
            RigidBody::dynamic().with_layers(1 << 1),
            32.0,
        ),
        (
            "a layer-1 body meeting layer 1",
            layer_one(RigidBody::dynamic()),
            32.0,
        ),
        (
            "a body in layer 2",
            RigidBody::dynamic().with_layers(1 << 2),
            128.0,
        ),
    ];
    for (name, body, floor) in cases {
        let mut world = seed_world();
        two_floor_map(&mut world);
        let faller = spawn_body(
            &mut world,
            body,
            Collider::aabb(Vec2::splat(4.0)),
            Vec2::new(40.0, 8.0),
            Vec2::new(0.0, 120.0),
        );
        for _ in 0..120 {
            physics_step(&mut world);
        }
        let bottom = world.get::<Position>(faller).unwrap().0.y + 4.0;
        assert!((bottom - floor).abs() < 0.5, "{name} ended at {bottom}");
    }
    // A layer-1 body whose mask lacks layer 0 passes the lower floor too.
    let mut world = seed_world();
    two_floor_map(&mut world);
    let ghost = spawn_body(
        &mut world,
        layer_one(RigidBody::dynamic()),
        Collider::aabb(Vec2::splat(4.0)),
        Vec2::new(40.0, 56.0),
        Vec2::new(0.0, 120.0),
    );
    for _ in 0..120 {
        physics_step(&mut world);
    }
    let y = world.get::<Position>(ghost).unwrap().0.y;
    assert!((y - 296.0).abs() < 1.0e-2, "the body was held at {y}");
    // A sensor over both floors reports the tiles in a layer its mask
    // holds: both by default, the upper floor's for layer 1.
    for (mask, tiles) in [(RigidBody::ALL_LAYERS, 2), (1 << 1, 1), (1 << 2, 0)] {
        let mut world = seed_world();
        two_floor_map(&mut world);
        let sensor = spawn_body(
            &mut world,
            RigidBody::sensor().with_mask(mask),
            Collider::aabb(Vec2::new(4.0, 60.0)),
            Vec2::new(40.0, 80.0),
            Vec2::ZERO,
        );
        physics_step(&mut world);
        let entered = drain_sensor_events(&mut world);
        assert_eq!(
            entered,
            vec![SensorEntered::new(sensor, None); tiles],
            "mask {mask:#x}"
        );
    }
}

/// `pushed_body_arrives_at_a_static_wall_and_stops_its_pusher`'s rig, with
/// `wall` for the wall (none without it) and `ball` for both balls: the
/// arrival pass's listed and clamp counts, and the balls' position bits.
fn arrival_rig(wall: Option<RigidBody>, ball: RigidBody) -> (usize, usize, [[u32; 2]; 2]) {
    const RADIUS: f32 = 8.0;
    const WALL_FACE: f32 = 798.0;
    let mut world = seed_world();
    if let Some(wall) = wall {
        let entity = world.spawn();
        world.insert(entity, Position(Vec2::new(WALL_FACE + 2.0, 0.0)));
        world.insert(entity, Collider::aabb(Vec2::new(2.0, 400.0)));
        world.insert(entity, wall);
    }
    let pushed = spawn_body(
        &mut world,
        ball,
        Collider::circle(RADIUS),
        Vec2::new(WALL_FACE - RADIUS - 3.0, 0.0),
        Vec2::ZERO,
    );
    let pusher = spawn_body(
        &mut world,
        ball.with_mass(100.0),
        Collider::circle(RADIUS),
        Vec2::new(WALL_FACE - RADIUS - 3.0 - 46.0, 0.0),
        Vec2::new(1_920.0, 0.0),
    );
    for _ in 0..60 {
        physics_step(&mut world);
    }
    let (listed, clamps) = arrival_counts(&world);
    let at = [position_bits(&world, pushed), position_bits(&world, pusher)];
    (listed, clamps, at)
}

#[test]
fn layers_the_arrival_pass_clamps_only_against_a_wall_the_body_meets() {
    // Without a wall, the pass clamps the pusher's own contact alone.
    let ball = RigidBody::dynamic().with_mask(RigidBody::DEFAULT_LAYERS);
    let (listed, alone, free) = arrival_rig(None, ball);
    assert!(listed > 0, "the arrival pass never ran");
    // A wall in layer 1, which the balls' masks lack: no clamp against it,
    // and the balls move as they do without it.
    let (_, clamps, passed) = arrival_rig(Some(RigidBody::r#static().with_layers(1 << 1)), ball);
    assert_eq!(clamps, alone, "the pass clamped against the wall");
    assert_eq!(passed, free);
    assert!(f32::from_bits(passed[0][0]) - 8.0 > 802.0);
    // A wall they meet: at least one clamp against it, and the pushed body
    // stops at its face.
    let (_, clamps, held) = arrival_rig(Some(RigidBody::r#static()), ball);
    assert!(clamps > alone, "clamps {clamps}, {alone} without the wall");
    let x = f32::from_bits(held[0][0]);
    assert!(x + 8.0 <= 799.0, "the pushed body sits in the wall at {x}");
}

#[test]
fn layers_a_kinematic_body_pushes_and_wakes_only_the_bodies_it_meets() {
    // A tall box, its face at x 12, with two balls asleep 1 px ahead of it:
    // one at the defaults and one in layer 1 alone.
    let mut world = seed_world();
    let body = spawn_kinematic(
        &mut world,
        Collider::aabb(Vec2::new(12.0, 60.0)),
        Vec2::ZERO,
        Vec2::ZERO,
    );
    let met = spawn_ball(&mut world, Vec2::new(23.0, -30.0));
    let passed = spawn_body(
        &mut world,
        layer_one(RigidBody::dynamic()),
        Collider::circle(10.0),
        Vec2::new(23.0, 30.0),
        Vec2::ZERO,
    );
    for _ in 0..60 {
        physics_step(&mut world);
    }
    assert!(
        is_sleeping(&world, met) && is_sleeping(&world, passed),
        "the idle balls never slept"
    );
    let passed_at = position_bits(&world, passed);
    world.get_mut::<Velocity>(body).unwrap().0 = Vec2::new(60.0, 0.0);
    for step in 0..60 {
        physics_step(&mut world);
        assert!(
            drain_events(&mut world)
                .iter()
                .all(|event| event.a != passed && event.b != Some(passed)),
            "a contact named the passed ball at step {step}"
        );
        assert!(
            is_sleeping(&world, passed),
            "the kinematic body woke the passed ball at step {step}"
        );
    }
    assert_eq!(position_bits(&world, passed), passed_at);
    let face = world.get::<Position>(body).unwrap().0.x + 12.0;
    let met_left = world.get::<Position>(met).unwrap().0.x - 10.0;
    assert!(
        face > 70.0 && met_left >= face - 1.0,
        "face {face}, ball {met_left}"
    );
    assert!(!is_sleeping(&world, met));
}

#[test]
fn layers_a_sensor_reports_what_its_mask_holds_whatever_the_others_mask() {
    let mut world = seed_world();
    let sensor_with = |world: &mut World, mask: u32| {
        spawn_body(
            world,
            RigidBody::sensor().with_mask(mask),
            Collider::circle(30.0),
            Vec2::ZERO,
            Vec2::ZERO,
        )
    };
    let world_sensor = sensor_with(&mut world, RigidBody::DEFAULT_LAYERS);
    let layer_sensor = sensor_with(&mut world, 1 << 2);
    let blind = sensor_with(&mut world, 0);
    // Layer 0 meeting only layer 3, and layer 2 meeting nothing.
    let plain = spawn_body(
        &mut world,
        RigidBody::r#static().with_mask(1 << 3),
        Collider::aabb(Vec2::splat(4.0)),
        Vec2::new(-10.0, 0.0),
        Vec2::ZERO,
    );
    let layered = spawn_body(
        &mut world,
        RigidBody::r#static().with_layers(1 << 2).with_mask(0),
        Collider::aabb(Vec2::splat(4.0)),
        Vec2::new(10.0, 0.0),
        Vec2::ZERO,
    );
    spawn_map(&mut world, "tile", Vec2::new(-8.0, 8.0), 1, vec![vec![0]]);
    physics_step(&mut world);
    let events = drain_sensor_events(&mut world);
    let seen = |sensor: Entity| {
        events
            .iter()
            .filter(|event| event.sensor == sensor)
            .map(|event| event.other)
            .collect::<Vec<_>>()
    };
    assert_eq!(seen(world_sensor), [Some(plain), None]);
    assert_eq!(seen(layer_sensor), [Some(layered)]);
    assert!(seen(blind).is_empty());
}

#[test]
fn layers_a_sensor_whose_mask_grows_reports_a_body_it_already_overlaps() {
    let mut world = seed_world();
    let sensor = spawn_body(
        &mut world,
        RigidBody::sensor().with_mask(RigidBody::DEFAULT_LAYERS),
        Collider::circle(8.0),
        Vec2::ZERO,
        Vec2::ZERO,
    );
    let body = spawn_body(
        &mut world,
        RigidBody::r#static().with_layers(1 << 3),
        Collider::aabb(Vec2::splat(4.0)),
        Vec2::new(4.0, 0.0),
        Vec2::ZERO,
    );
    let entered = [SensorEntered::new(sensor, Some(body))];
    for _ in 0..3 {
        physics_step(&mut world);
        assert!(drain_sensor_events(&mut world).is_empty());
    }
    world.get_mut::<RigidBody>(sensor).unwrap().mask |= 1 << 3;
    physics_step(&mut world);
    assert_eq!(drain_sensor_events(&mut world), entered);
    physics_step(&mut world);
    assert!(drain_sensor_events(&mut world).is_empty());
    // A mask that shrinks forgets the pair, so it grows into a new one.
    world.get_mut::<RigidBody>(sensor).unwrap().mask = RigidBody::DEFAULT_LAYERS;
    physics_step(&mut world);
    assert!(drain_sensor_events(&mut world).is_empty());
    world.get_mut::<RigidBody>(sensor).unwrap().mask |= 1 << 3;
    physics_step(&mut world);
    assert_eq!(drain_sensor_events(&mut world), entered);
}

#[test]
fn layers_a_sleeper_whose_layers_or_mask_change_wakes_its_island() {
    for change in 0..2 {
        let mut world = seed_world();
        let (bottom, top) = spawn_sleeping_stack(&mut world);
        settle_to_sleep(&mut world, &[bottom, top]);
        let resting = world.get::<Position>(top).unwrap().0.y;
        let body = world.get_mut::<RigidBody>(top).unwrap();
        match change {
            // Still meeting everything it met.
            0 => body.layers = 0b11,
            // Meeting nothing: it falls through the ball and the floor.
            _ => body.mask = 0,
        }
        physics_step(&mut world);
        assert!(
            !is_sleeping(&world, bottom) && !is_sleeping(&world, top),
            "change {change}: the island stayed asleep"
        );
        if change == 1 {
            for _ in 0..30 {
                physics_step(&mut world);
            }
            assert!(world.get::<Position>(top).unwrap().0.y > resting + 30.0);
        }
    }
}

#[test]
fn layers_a_static_support_whose_layers_or_mask_change_wakes_the_island_on_it() {
    for change in 0..2 {
        let mut world = seed_world();
        let (bottom, top) = spawn_sleeping_stack(&mut world);
        settle_to_sleep(&mut world, &[bottom, top]);
        let floor = stack_floor(&world);
        let resting = world.get::<Position>(bottom).unwrap().0.y;
        let body = world.get_mut::<RigidBody>(floor).unwrap();
        match change {
            // Still meeting both balls.
            0 => body.layers = 0b11,
            // A door that stops meeting them: they fall.
            _ => body.mask = 1 << 4,
        }
        physics_step(&mut world);
        assert!(
            !is_sleeping(&world, bottom) && !is_sleeping(&world, top),
            "change {change}: the island stayed asleep"
        );
        if change == 1 {
            for _ in 0..30 {
                physics_step(&mut world);
            }
            assert!(world.get::<Position>(bottom).unwrap().0.y > resting + 30.0);
        }
    }
}

#[test]
fn layers_constructors_and_bundles_give_the_defaults_and_a_default_world_is_not_layered() {
    let defaults = (RigidBody::DEFAULT_LAYERS, RigidBody::ALL_LAYERS);
    assert_eq!(defaults, (1, u32::MAX));
    for body in [
        RigidBody::dynamic(),
        RigidBody::r#static(),
        RigidBody::kinematic(),
        RigidBody::sensor(),
    ] {
        assert_eq!((body.layers, body.mask), defaults, "{:?}", body.kind);
        let set = body.with_layers(0b101).with_mask(0b110);
        assert_eq!(
            set,
            RigidBody {
                layers: 0b101,
                mask: 0b110,
                ..body
            }
        );
    }
    let at = Position::new(1.0, 2.0);
    let collider = Collider::circle(4.0);
    for bundle in [
        RigidBodyBundle::dynamic(at, collider),
        RigidBodyBundle::r#static(at, collider),
        RigidBodyBundle::kinematic(at, collider),
        RigidBodyBundle::sensor(at, collider),
    ] {
        assert_eq!((bundle.body.layers, bundle.body.mask), defaults);
        let set = bundle.with_layers(0b101).with_mask(0b110);
        assert_eq!(set.body, bundle.body.with_layers(0b101).with_mask(0b110));
        assert_eq!(
            (set.position, set.velocity),
            (bundle.position, bundle.velocity)
        );
    }
    // The rule: each mask holds a layer the other is in.
    let a = RigidBody::dynamic().with_layers(0b01).with_mask(0b10);
    let b = RigidBody::dynamic().with_layers(0b10).with_mask(0b01);
    assert!(a.meets(&b) && b.meets(&a));
    assert!(!a.meets(&a));
    assert!(
        !RigidBody::dynamic()
            .with_mask(0)
            .meets(&RigidBody::dynamic())
    );

    // Every kind, a tile and a collider without a body, all at the defaults.
    let mut world = seed_world();
    spawn_ball(&mut world, Vec2::ZERO);
    spawn_static(
        &mut world,
        Collider::aabb(Vec2::splat(4.0)),
        Vec2::new(40.0, 0.0),
    );
    spawn_kinematic(&mut world, collider, Vec2::new(80.0, 0.0), Vec2::ZERO);
    let sensor = spawn_sensor(&mut world, collider, Vec2::new(120.0, 0.0), Vec2::ZERO);
    spawn_map(&mut world, "tile", Vec2::new(160.0, 0.0), 1, vec![vec![0]]);
    let bare = world.spawn();
    world.insert(bare, Position(Vec2::new(200.0, 0.0)));
    world.insert(bare, collider);
    let layered = |world: &World| {
        world
            .get_resource::<PhysicsBuffers>()
            .unwrap()
            .layers
            .layered
    };
    for _ in 0..3 {
        physics_step(&mut world);
        assert!(!layered(&world));
    }
    // A sensor off the defaults is enough.
    world.get_mut::<RigidBody>(sensor).unwrap().layers = 0b10;
    physics_step(&mut world);
    assert!(layered(&world));
}

#[test]
fn layers_persistent_pairs_match_fresh_contacts_with_masks() {
    use crate::Pcg32;

    let mut checked = 0;
    let mut repairs = 0;
    let mut unmet = 0;
    for seed in 0..8 {
        let mut rng = Pcg32::seeded(0x1A7E + seed);
        let mut world = seed_world();
        world.get_resource_mut::<PhysicsConfig>().unwrap().gravity = Vec2::new(0.0, 900.0);
        // A static floor: a tile row's seams would admit contacts the
        // oracle's exhaustive pass finds along the whole row.
        spawn_static(
            &mut world,
            Collider::aabb(Vec2::new(80.0, 8.0)),
            Vec2::new(0.0, 104.0),
        );
        let buffers = PhysicsBuffers {
            check_pair_contacts: true,
            ..PhysicsBuffers::default()
        };
        world.insert_resource(buffers);
        let config = world.get_resource_mut::<PhysicsConfig>().unwrap();
        config.substeps = [1, 2, 4, 8][seed as usize % 4];
        config.broadphase_cell_size = [8.0, 16.0, 32.0][seed as usize % 3];
        let mut bodies = Vec::new();
        for i in 0..30 {
            let body = RigidBody::dynamic()
                .with_layers(1 << (rng.next_u32() % 3))
                .with_mask([0b111, 0b011, 0b101, 0b001, 0b110][rng.next_u32() as usize % 5]);
            let position = Vec2::new(
                (i % 6) as f32 * 11.0 - 28.0 + rng.next_range(-1.0, 1.0),
                60.0 - (i / 6) as f32 * 11.0 + rng.next_range(-1.0, 1.0),
            );
            let velocity = rng.next_unit_vec2() * rng.next_range(0.0, 80.0);
            let collider = if i % 3 == 0 {
                Collider::aabb(Vec2::splat(5.0))
            } else {
                Collider::circle(5.0)
            };
            bodies.push(spawn_body(&mut world, body, collider, position, velocity));
        }
        // A bullet in layer 1 meeting layers 0 and 1 trips budgets.
        spawn_body(
            &mut world,
            RigidBody::dynamic().with_layers(0b10).with_mask(0b11),
            Collider::circle(4.0),
            Vec2::new(-90.0, 40.0),
            Vec2::new(rng.next_range(2_400.0, 9_000.0), 0.0),
        );
        for _ in 0..40 {
            physics_step(&mut world);
            drain_events(&mut world);
        }
        // Some overlapping bodies must not meet, or the masks were idle.
        for (index, &a) in bodies.iter().enumerate() {
            for &b in &bodies[index + 1..] {
                let (pa, pb) = (
                    world.get::<Position>(a).unwrap().0,
                    world.get::<Position>(b).unwrap().0,
                );
                let (ba, bb) = (
                    world.get::<RigidBody>(a).unwrap(),
                    world.get::<RigidBody>(b).unwrap(),
                );
                if pa.distance(pb) < 9.0 && !ba.meets(bb) {
                    unmet += 1;
                }
            }
        }
        let buffers = world.get_resource::<PhysicsBuffers>().unwrap();
        assert!(buffers.layers.layered);
        checked += buffers.checked_substeps;
        repairs += buffers.pair_repairs;
    }
    assert_eq!(
        checked,
        40 * (1 + 2 + 4 + 8) * 2,
        "every substep runs the oracle"
    );
    assert!(repairs > 0, "the oracle never checked a repaired list");
    assert!(unmet > 0, "no pair that does not meet ever overlapped");
}

/// A 6 × 3 map of 16 px tiles: `tiles` in its collision layer, after a
/// render layer when `decor`.
fn cache_map(tiles: Vec<i32>, decor: bool) -> TilemapData {
    let mut layers = Vec::new();
    if decor {
        layers.push(TilemapLayer {
            name: "decor".into(),
            kind: LayerKind::Render,
            tiles: vec![0; 18],
        });
    }
    layers.push(TilemapLayer {
        name: "collision".into(),
        kind: LayerKind::Collision,
        tiles,
    });
    TilemapData {
        tile_width: 16,
        tile_height: 16,
        width: 6,
        height: 3,
        tileset: vec!["solid".into()],
        object_layers: Vec::new(),
        terrain: TerrainProps::default(),
        layers,
    }
}

/// Gathers `world` into `buffers`, then asserts the tile proxies after the
/// entity proxies, and their layers, identities and cells, are a fresh
/// scan's.
fn assert_tiles_fresh(world: &World, buffers: &mut PhysicsBuffers, when: &str) {
    gather_proxies(
        world,
        &mut buffers.proxies,
        &mut buffers.layers,
        &mut buffers.sensors,
        &mut buffers.tile_ids,
        &mut buffers.tiles,
    );
    buffers.bodies = buffers.proxies.len() - buffers.tiles.len();
    let mut fresh = super::gather::TileArrays::default();
    super::gather::gather_tilemap_proxies(world, &mut fresh);
    let tail = &buffers.proxies[buffers.bodies..];
    assert_eq!(tail.len(), fresh.proxies.len(), "{when}: tile count");
    for (at, (kept, scanned)) in tail.iter().zip(&fresh.proxies).enumerate() {
        assert!(
            kept.key == scanned.key
                && kept.center == scanned.center
                && kept.half_extents() == scanned.half_extents()
                && kept.face_mask == scanned.face_mask
                && kept.entity.is_none()
                && !kept.is_dynamic,
            "{when}: tile {at} differs"
        );
    }
    // Identities are gathered in a frame with sensors only.
    if buffers.sensors.is_empty() {
        assert!(buffers.tile_ids.is_empty(), "{when}: no identities");
    } else {
        assert_eq!(buffers.tile_ids, fresh.ids, "{when}: identities");
    }
    assert_eq!(buffers.tiles.ids, fresh.ids, "{when}: cached identities");
    assert_eq!(buffers.tiles.cells, fresh.cells, "{when}: cells");
    assert_eq!(
        buffers.layers.of[buffers.bodies..],
        fresh.layers[..],
        "{when}: tile layers"
    );
}

#[test]
fn the_tile_cache_matches_a_fresh_scan_after_every_change() {
    // Edited by a registry insert and in place, broken, moved, joined by a
    // second map, given a render layer before its collision layer (which
    // moves every identity), its registry replaced by one of equal and of
    // different content, and despawned: after each the gathered tiles are a
    // fresh scan's, only the maps that changed are scanned, and a frame
    // with no change scans nothing.
    let mut world = seed_world();
    let solid = |gaps: &[usize]| {
        (0..18)
            .map(|i| if gaps.contains(&i) { -1 } else { 0 })
            .collect()
    };
    world
        .get_resource_mut::<TilemapRegistry>()
        .unwrap()
        .insert("a".into(), cache_map(solid(&[2, 9]), false));
    let map = world.spawn();
    world.insert(map, TilemapInstance::new("a", Vec2::new(-48.0, 0.0)));
    // A sensor, so the gather keeps the tiles' identities.
    world.spawn_with(RigidBodyBundle::sensor(
        Position(Vec2::ZERO),
        Collider::aabb(Vec2::splat(4.0)),
    ));
    let mut buffers = PhysicsBuffers::default();
    assert_tiles_fresh(&world, &mut buffers, "first");
    assert_eq!(buffers.tiles.scans, 1);
    assert_tiles_fresh(&world, &mut buffers, "unchanged");
    assert_eq!(buffers.tiles.scans, 1, "an unchanged frame scans nothing");

    fn registry(world: &mut World) -> &mut TilemapRegistry {
        world.get_resource_mut::<TilemapRegistry>().unwrap()
    }
    let breakable = |gaps: &[usize]| {
        let mut data = cache_map(solid(gaps), false);
        data.set_tile_props(
            0,
            TileProps {
                health: Some(1.0),
                ..TileProps::default()
            },
        );
        data
    };
    type Change<'a> = (&'static str, usize, Box<dyn Fn(&mut World) + 'a>);
    let changes: [Change<'_>; 13] = [
        (
            "edited by an insert",
            1,
            Box::new(move |world: &mut World| {
                registry(world).insert("a".into(), cache_map(solid(&[2, 9, 13]), false));
            }),
        ),
        (
            "edited in place",
            1,
            Box::new(move |world: &mut World| {
                assert!(registry(world).set_tile("a", 0, 4, 1, EMPTY_TILE));
            }),
        ),
        (
            "written and written back",
            0,
            Box::new(move |world: &mut World| {
                registry(world).set_tile("a", 0, 2, 0, 0);
                registry(world).set_tile("a", 0, 2, 0, EMPTY_TILE);
            }),
        ),
        (
            "made breakable",
            1,
            Box::new(move |world: &mut World| {
                registry(world).insert("a".into(), breakable(&[2, 9, 13]));
            }),
        ),
        (
            "damaged",
            0,
            Box::new(move |world: &mut World| {
                let damage = registry(world).damage_tile("a", 0, 0, 2, 0.5);
                assert_eq!(damage, TileDamage::Damaged { health: 0.5 });
            }),
        ),
        (
            "broken",
            1,
            Box::new(move |world: &mut World| {
                let damage = registry(world).damage_tile("a", 0, 0, 2, 0.5);
                assert_eq!(damage, TileDamage::Broken { tile: EMPTY_TILE });
            }),
        ),
        (
            "moved",
            1,
            Box::new(move |world: &mut World| {
                world.get_mut::<TilemapInstance>(map).unwrap().origin = Vec2::new(-40.0, 8.0);
            }),
        ),
        (
            "joined",
            1,
            Box::new(|world: &mut World| {
                registry(world).insert("b".into(), cache_map(solid(&[0]), false));
                let second = world.spawn();
                world.insert(second, TilemapInstance::new("b", Vec2::new(64.0, 0.0)));
            }),
        ),
        (
            "the second edited in place",
            1,
            Box::new(move |world: &mut World| {
                assert!(registry(world).set_tile("b", 0, 0, 0, 0));
            }),
        ),
        (
            "decor before collision",
            1,
            Box::new(move |world: &mut World| {
                registry(world).insert("a".into(), cache_map(solid(&[2, 9, 13]), true));
            }),
        ),
        (
            "a registry of equal content",
            0,
            Box::new(|world: &mut World| {
                let old = world.get_resource::<TilemapRegistry>().unwrap();
                let mut equal = TilemapRegistry::new();
                for id in ["b", "a"] {
                    equal.insert(id.into(), old.get(id).unwrap().clone());
                }
                world.insert_resource(equal);
            }),
        ),
        (
            "a registry of different content",
            1,
            Box::new(|world: &mut World| {
                let old = world.get_resource::<TilemapRegistry>().unwrap();
                let mut other = TilemapRegistry::new();
                other.insert("a".into(), cache_map(solid(&[1]), true));
                other.insert("b".into(), old.get("b").unwrap().clone());
                world.insert_resource(other);
            }),
        ),
        (
            "despawned",
            0,
            Box::new(move |world: &mut World| {
                world.despawn(map);
            }),
        ),
    ];
    let mut scans = 1;
    let mut version = buffers.tiles.version;
    for (when, scanned, change) in changes {
        let ids_before = buffers.tile_ids.clone();
        let tiles_before = buffers.tiles.len();
        change(&mut world);
        assert_tiles_fresh(&world, &mut buffers, when);
        scans += scanned;
        assert_eq!(buffers.tiles.scans, scans, "{when}: maps scanned");
        let changed = scanned > 0 || buffers.tiles.len() != tiles_before;
        assert_eq!(
            buffers.tiles.version > version,
            changed,
            "{when}: a new version only for a change"
        );
        version = buffers.tiles.version;
        if when == "decor before collision" {
            assert_ne!(
                buffers.tile_ids, ids_before,
                "the layer index is in each identity"
            );
        }
        assert_tiles_fresh(&world, &mut buffers, when);
        assert_eq!(buffers.tiles.scans, scans, "{when}: then no more");
    }
}

#[test]
fn an_edit_rescans_its_map_alone() {
    let mut world = seed_world();
    let full = || vec![0; 18];
    for (id, x) in [("a", 0.0), ("b", 200.0)] {
        world
            .get_resource_mut::<TilemapRegistry>()
            .unwrap()
            .insert(id.into(), cache_map(full(), false));
        let map = world.spawn();
        world.insert(map, TilemapInstance::new(id, Vec2::new(x, 0.0)));
    }
    let mut buffers = PhysicsBuffers::default();
    assert_tiles_fresh(&world, &mut buffers, "first");
    assert_eq!(buffers.tiles.scans, 2);
    let b_before: Vec<[u32; 2]> = buffers.tiles.proxies[18..]
        .iter()
        .map(|proxy| proxy.center.to_array().map(f32::to_bits))
        .collect();
    let version = buffers.tiles.version;
    assert!(
        world
            .get_resource_mut::<TilemapRegistry>()
            .unwrap()
            .set_tile("a", 0, 3, 1, EMPTY_TILE)
    );
    assert_tiles_fresh(&world, &mut buffers, "a edited");
    assert_eq!(buffers.tiles.scans, 3, "map a alone scanned again");
    assert!(buffers.tiles.version > version);
    assert_eq!(buffers.tiles.len(), 35);
    let b_after: Vec<[u32; 2]> = buffers.tiles.proxies[17..]
        .iter()
        .map(|proxy| proxy.center.to_array().map(f32::to_bits))
        .collect();
    assert_eq!(b_after, b_before, "map b's proxies kept");
    assert_tiles_fresh(&world, &mut buffers, "unchanged");
    assert_eq!(buffers.tiles.scans, 3);
}

#[test]
fn an_edited_cell_collides_at_the_next_step() {
    // A 6 px box rests on a 3-cell floor of 16 px tiles, its top at y 32;
    // the cell under it is cleared, then filled again under a second box
    // falling through the gap.
    let mut world = seed_world();
    world.resource_mut::<PhysicsConfig>().gravity = Vec2::new(0.0, 900.0);
    let mut tiles = vec![-1; 9];
    tiles[6..9].fill(0);
    world
        .resource_mut::<TilemapRegistry>()
        .insert("floor".into(), map_data(3, vec![tiles]));
    let map = world.spawn();
    world.insert(map, TilemapInstance::new("floor", Vec2::ZERO));
    let resting = spawn_body(
        &mut world,
        RigidBody::dynamic(),
        Collider::aabb(Vec2::splat(6.0)),
        Vec2::new(24.0, 20.0),
        Vec2::ZERO,
    );
    for _ in 0..120 {
        physics_step(&mut world);
    }
    let y = |world: &World, body: Entity| world.get::<Position>(body).unwrap().0.y;
    let rest = y(&world, resting);
    assert!((rest + 6.0 - 32.0).abs() < 0.5, "resting at {rest}");
    assert!(
        world
            .resource_mut::<TilemapRegistry>()
            .set_tile("floor", 0, 1, 2, EMPTY_TILE)
    );
    physics_step(&mut world);
    assert!(y(&world, resting) > rest, "it falls at the next step");
    for _ in 0..30 {
        physics_step(&mut world);
    }
    assert!(y(&world, resting) > 48.0, "through the gap");
    // A box falling through the gap meets the cell filled before it gets
    // there.
    let falling = spawn_body(
        &mut world,
        RigidBody::dynamic(),
        Collider::aabb(Vec2::splat(6.0)),
        Vec2::new(24.0, -40.0),
        Vec2::ZERO,
    );
    for _ in 0..5 {
        physics_step(&mut world);
    }
    assert!(y(&world, falling) + 6.0 < 32.0, "still above the floor");
    assert!(
        world
            .resource_mut::<TilemapRegistry>()
            .set_tile("floor", 0, 1, 2, 0)
    );
    for _ in 0..120 {
        physics_step(&mut world);
    }
    let landed = y(&world, falling);
    assert!((landed + 6.0 - 32.0).abs() < 0.5, "landed at {landed}");
}

#[test]
fn a_changed_linear_slop_restages_the_tile_grid() {
    // The tile grid's boxes are grown by 2·linear_slop: a new slop with the
    // same map stages them again, so the pair boxes are a fresh build's.
    let mut world = seed_world();
    world
        .get_resource_mut::<TilemapRegistry>()
        .unwrap()
        .insert("a".into(), cache_map(vec![0; 18], false));
    let map = world.spawn();
    world.insert(map, TilemapInstance::new("a", Vec2::ZERO));
    world.spawn_with(
        RigidBodyBundle::dynamic(
            Position(Vec2::new(40.0, -20.0)),
            Collider::aabb(Vec2::splat(4.0)),
        )
        .with_velocity(Velocity(Vec2::new(0.0, 30.0))),
    );
    let mut buffers = PhysicsBuffers::default();
    for slop in [0.25_f32, 1.5, 0.25] {
        world
            .get_resource_mut::<PhysicsConfig>()
            .unwrap()
            .linear_slop = slop;
        let config = *world.get_resource::<PhysicsConfig>().unwrap();
        assert_tiles_fresh(&world, &mut buffers, "gathered");
        build_pairs(&config, 1.0 / 240.0, 1.0 / 60.0, &mut buffers);
        for (at, proxy) in buffers.proxies.iter().enumerate().skip(buffers.bodies) {
            let mut want = proxy.world_aabb();
            want.half_extents += Vec2::splat(2.0 * slop);
            let got = buffers.pair_aabbs[at];
            assert!(
                got.center == want.center && got.half_extents == want.half_extents,
                "slop {slop}: tile {at}'s pair box"
            );
        }
    }
}

/// The step's sensor pairs and new pairs as a test of every proxy and tile
/// finds them, from the buffers after the step: each sensor in gather order
/// against the entity proxies, then the tiles, in a layer its mask holds.
fn sensor_oracle(buffers: &PhysicsBuffers) -> (Vec<u128>, Vec<(u64, u64)>) {
    use crate::physics::query::shapes_overlap;
    let mut pairs = Vec::new();
    let mut order = Vec::new();
    let tiles = buffers.proxies.len() - buffers.bodies;
    for sensor in &buffers.sensors {
        let others = buffers.proxies[..buffers.bodies]
            .iter()
            .zip(&buffers.layers.of)
            .map(|(proxy, layers)| (proxy, proxy.key, layers.layers));
        let tile_ids = if buffers.tile_ids.len() == tiles {
            buffers.tile_ids.clone()
        } else {
            vec![0; tiles]
        };
        let tiles = buffers.proxies[buffers.bodies..]
            .iter()
            .zip(tile_ids)
            .zip(&buffers.layers.of[buffers.bodies..])
            .map(|((proxy, id), layers)| (proxy, id, layers.layers));
        for (proxy, identity, layers) in others.chain(tiles) {
            if sensor.layers.mask & layers != 0
                && shapes_overlap(sensor.shape, sensor.center, proxy.shape, proxy.center)
            {
                pairs.push((u128::from(sensor.key) << 64) | u128::from(identity));
                order.push((sensor.key, identity));
            }
        }
    }
    pairs.sort_unstable();
    (pairs, order)
}

#[test]
fn kept_sensor_tile_pairs_are_what_testing_every_tile_finds() {
    // A still sensor keeps its tile pairs from step to step, one that moves
    // every third step and one whose mask leaves out the tiles' layer test
    // again; a body falls through them, the map is edited and a step runs
    // without sensors. Every step's pairs and new-pair events are a full
    // test's (`sensor_oracle`).
    let mut world = seed_world();
    let gaps = |gaps: &[usize]| {
        (0..18)
            .map(|i| if gaps.contains(&i) { -1 } else { 0 })
            .collect()
    };
    world
        .get_resource_mut::<TilemapRegistry>()
        .unwrap()
        .insert("a".into(), cache_map(gaps(&[3, 8]), false));
    let map = world.spawn();
    world.insert(map, TilemapInstance::new("a", Vec2::new(-48.0, 0.0)));
    let still = world.spawn_with(RigidBodyBundle::sensor(
        Position(Vec2::new(0.0, 16.0)),
        Collider::aabb(Vec2::new(30.0, 20.0)),
    ));
    let mover = world.spawn_with(RigidBodyBundle::sensor(
        Position(Vec2::new(-30.0, 8.0)),
        Collider::aabb(Vec2::splat(10.0)),
    ));
    let masked = world.spawn_with(
        RigidBodyBundle::sensor(Position(Vec2::new(10.0, 20.0)), Collider::circle(14.0))
            .with_mask(!RigidBody::DEFAULT_LAYERS),
    );
    world.spawn_with(
        RigidBodyBundle::dynamic(Position(Vec2::new(4.0, -40.0)), Collider::circle(5.0))
            .with_velocity(Velocity(Vec2::new(0.0, 60.0))),
    );
    let mut previous: Vec<u128> = Vec::new();
    let mut kept_steps = 0;
    for step in 0..48 {
        if step % 3 == 0 {
            world.get_mut::<Position>(mover).unwrap().0.x += 8.0;
        }
        if step == 20 {
            world
                .get_resource_mut::<TilemapRegistry>()
                .unwrap()
                .insert("a".into(), cache_map(gaps(&[3, 8, 14]), false));
        }
        let sensorless = step == 30;
        if sensorless {
            for sensor in [still, mover, masked] {
                world.get_mut::<RigidBody>(sensor).unwrap().kind = BodyKind::Static;
            }
            previous.clear();
        }
        if step == 31 {
            for sensor in [still, mover, masked] {
                world.get_mut::<RigidBody>(sensor).unwrap().kind = BodyKind::Sensor;
            }
        }
        physics_step(&mut world);
        let events: Vec<SensorEntered> = {
            let queue = world
                .get_resource_mut::<EventQueue<SensorEntered>>()
                .unwrap();
            let sent = queue.iter_current().copied().collect();
            queue.flush();
            sent
        };
        if sensorless {
            continue;
        }
        let buffers = world.get_resource::<PhysicsBuffers>().unwrap();
        let (want, order) = sensor_oracle(buffers);
        assert_eq!(buffers.sensor_pairs, want, "step {step}: the pairs");
        let new: Vec<(u64, u64)> = order
            .into_iter()
            .filter(|&(sensor, other)| {
                previous
                    .binary_search(&((u128::from(sensor) << 64) | u128::from(other)))
                    .is_err()
            })
            .collect();
        assert_eq!(events.len(), new.len(), "step {step}: the new pairs");
        for (event, (sensor, _)) in events.iter().zip(&new) {
            assert_eq!(
                entity_key(event.sensor),
                *sensor,
                "step {step}: event order"
            );
        }
        kept_steps += buffers
            .sensor_tiles
            .iter()
            .filter(|kept| kept.used == buffers.sensor_passes && !kept.pairs.is_empty())
            .count();
        previous = want;
    }
    assert!(
        kept_steps > 40,
        "the still sensor's pairs were kept: {kept_steps}"
    );
}

#[test]
fn both_pair_grid_layouts_build_the_same_pairs() {
    // The tiles in a grid of their own or staged with the entity proxies:
    // a matter of cost, so a build's pair list is the same, pair for pair
    // and in order, with sleepers, statics, kinematic bodies and tiles.
    use crate::Pcg32;
    for seed in 0..4_u64 {
        let mut rng = Pcg32::seeded(0x5917 + seed);
        let mut world = seed_world();
        let tiles = (0..72).map(|i| if i % 7 == 3 { -1 } else { 0 }).collect();
        let mut map = cache_map(tiles, seed % 2 == 1);
        (map.width, map.height) = (12, 6);
        world
            .get_resource_mut::<TilemapRegistry>()
            .unwrap()
            .insert("a".into(), map);
        let map = world.spawn();
        world.insert(map, TilemapInstance::new("a", Vec2::new(-96.0, 20.0)));
        for i in 0..80 {
            let at = Position(Vec2::new(
                rng.next_range(-120.0, 120.0),
                rng.next_range(-60.0, 140.0),
            ));
            let collider = if i % 2 == 0 {
                Collider::aabb(Vec2::splat(rng.next_range(2.0, 14.0)))
            } else {
                Collider::circle(rng.next_range(2.0, 10.0))
            };
            let velocity = Velocity(rng.next_unit_vec2() * rng.next_range(0.0, 500.0));
            let bundle = match i % 4 {
                0 => RigidBodyBundle::r#static(at, collider),
                1 => RigidBodyBundle::kinematic(at, collider).with_velocity(velocity),
                _ => RigidBodyBundle::dynamic(at, collider).with_velocity(velocity),
            };
            world.spawn_with(bundle);
        }
        let config = *world.get_resource::<PhysicsConfig>().unwrap();
        let mut built = Vec::new();
        for split in [true, false] {
            let mut buffers = PhysicsBuffers::default();
            assert_tiles_fresh(&world, &mut buffers, "gathered");
            for proxy in buffers.proxies.iter_mut().step_by(5) {
                proxy.sleeping = proxy.is_dynamic;
            }
            buffers.grid.force_split = Some(split);
            build_pairs(&config, 1.0 / 240.0, 1.0 / 60.0, &mut buffers);
            built.push(buffers.pairs.clone());
        }
        assert_eq!(built[0], built[1], "seed {seed}");
        assert!(built[0].len() > 40, "seed {seed}: too few pairs");
    }
}
