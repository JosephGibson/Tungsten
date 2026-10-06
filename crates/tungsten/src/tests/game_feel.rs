use glam::Vec2;
use tungsten_core::{
    CameraController, CommandBuffer, Easing, Entity, EventQueue, ShakeEvent, SpriteSquashStretch,
    SquashEvent, SquashStretchState, SquashTrigger, Time, Transform, World,
};

use crate::game_feel::{
    shake_tick_system, squash_stretch_tick_system, squash_stretch_trigger_system,
};

fn make_world(dt: f32) -> World {
    let mut world = World::new();
    let mut time = Time::new();
    time.advance_frame(dt);
    world.insert_resource(time);
    world.insert_resource(CommandBuffer::new());
    world.insert_resource(EventQueue::<ShakeEvent>::new());
    world.insert_resource(EventQueue::<SquashEvent>::new());
    world
}

fn flush(world: &mut World) {
    let buf = world.remove_resource::<CommandBuffer>().unwrap();
    world.flush(buf);
    world.insert_resource(CommandBuffer::new());
}

fn send_shake(world: &mut World, trauma_add: f32) {
    world
        .get_resource_mut::<EventQueue<ShakeEvent>>()
        .unwrap()
        .send(ShakeEvent { trauma_add });
}

fn send_squash(world: &mut World, entity: Entity, trigger: SquashTrigger) {
    world
        .get_resource_mut::<EventQueue<SquashEvent>>()
        .unwrap()
        .send(SquashEvent { entity, trigger });
}

fn squash(on: SquashTrigger) -> SpriteSquashStretch {
    SpriteSquashStretch {
        on,
        amount: Vec2::new(1.25, 0.75),
        duration: 0.2,
        easing: Easing::Linear,
    }
}

fn spawn_squasher(world: &mut World, scale: Vec2, on: SquashTrigger) -> Entity {
    let entity = world.spawn();
    world.insert(
        entity,
        Transform {
            position: Vec2::ZERO,
            rotation: 0.0,
            scale,
        },
    );
    world.insert(entity, squash(on));
    entity
}

#[test]
fn shake_tick_accumulates_events_in_one_frame() {
    let mut world = make_world(0.0);
    world.insert_resource(CameraController::default());
    send_shake(&mut world, 0.2);
    send_shake(&mut world, 0.3);

    shake_tick_system(&mut world);

    let trauma = world
        .get_resource::<CameraController>()
        .unwrap()
        .shake_trauma;
    assert!((trauma - 0.5).abs() <= 1e-6, "trauma={trauma}");
}

#[test]
fn shake_tick_saturates_at_one() {
    let mut world = make_world(0.0);
    world.insert_resource(CameraController::default());
    send_shake(&mut world, 0.9);
    send_shake(&mut world, 0.9);

    shake_tick_system(&mut world);

    assert_eq!(
        world
            .get_resource::<CameraController>()
            .unwrap()
            .shake_trauma,
        1.0
    );
}

#[test]
fn shake_tick_decays_by_decay_times_dt() {
    let mut world = make_world(0.1);
    let mut controller = CameraController::default();
    controller.shake_decay = 2.0;
    controller.shake_trauma = 1.0;
    world.insert_resource(controller);

    shake_tick_system(&mut world);

    let trauma = world
        .get_resource::<CameraController>()
        .unwrap()
        .shake_trauma;
    assert!((trauma - 0.8).abs() <= 1e-6, "trauma={trauma}");
}

#[test]
fn shake_tick_floors_trauma_at_zero() {
    let mut world = make_world(1.0);
    let mut controller = CameraController::default();
    controller.shake_trauma = 0.2;
    world.insert_resource(controller);

    shake_tick_system(&mut world);

    assert_eq!(
        world
            .get_resource::<CameraController>()
            .unwrap()
            .shake_trauma,
        0.0
    );
}

#[test]
fn shake_tick_no_ops_without_controller() {
    let mut world = make_world(0.016);
    send_shake(&mut world, 0.5);
    shake_tick_system(&mut world);
    assert!(world.get_resource::<CameraController>().is_none());
}

#[test]
fn shake_tick_ignores_previous_window_events() {
    let mut world = make_world(0.0);
    world.insert_resource(CameraController::default());
    send_shake(&mut world, 0.4);
    shake_tick_system(&mut world);

    // Frame boundary: the event rotates into the previous window.
    world
        .get_resource_mut::<EventQueue<ShakeEvent>>()
        .unwrap()
        .flush();
    shake_tick_system(&mut world);

    let trauma = world
        .get_resource::<CameraController>()
        .unwrap()
        .shake_trauma;
    assert!((trauma - 0.4).abs() <= 1e-6, "trauma={trauma}");
}

#[test]
fn squash_trigger_matches_only_its_own_trigger() {
    let mut world = make_world(0.016);
    let entity = spawn_squasher(&mut world, Vec2::ONE, SquashTrigger::OnLand);

    send_squash(&mut world, entity, SquashTrigger::OnHit);
    squash_stretch_trigger_system(&mut world);
    flush(&mut world);
    assert!(world.get::<SquashStretchState>(entity).is_none());

    send_squash(&mut world, entity, SquashTrigger::OnLand);
    squash_stretch_trigger_system(&mut world);
    flush(&mut world);
    assert!(world.get::<SquashStretchState>(entity).is_some());
}

#[test]
fn squash_trigger_ignores_missing_entity() {
    let mut world = make_world(0.016);
    let ghost = world.spawn();
    world.despawn(ghost);

    send_squash(&mut world, ghost, SquashTrigger::OnLand);
    squash_stretch_trigger_system(&mut world);
    flush(&mut world);

    assert!(world.get::<SquashStretchState>(ghost).is_none());
}

#[test]
fn squash_trigger_captures_live_transform_scale() {
    let mut world = make_world(0.016);
    let entity = spawn_squasher(&mut world, Vec2::new(3.0, 5.0), SquashTrigger::OnLand);

    send_squash(&mut world, entity, SquashTrigger::OnLand);
    squash_stretch_trigger_system(&mut world);
    flush(&mut world);

    assert_eq!(
        world.get::<SquashStretchState>(entity).unwrap().base_scale,
        Vec2::new(3.0, 5.0)
    );
}

#[test]
fn squash_tick_restores_base_scale_and_removes_state() {
    let mut world = make_world(0.05);
    let base = Vec2::new(2.0, 2.0);
    let entity = spawn_squasher(&mut world, base, SquashTrigger::OnLand);

    send_squash(&mut world, entity, SquashTrigger::OnLand);
    squash_stretch_trigger_system(&mut world);
    flush(&mut world);

    let mut peaked = false;
    for _ in 0..4 {
        squash_stretch_tick_system(&mut world);
        flush(&mut world);
        if world.get::<Transform>(entity).unwrap().scale != base {
            peaked = true;
        }
    }

    assert!(peaked, "envelope never deviated from base scale");
    assert_eq!(world.get::<Transform>(entity).unwrap().scale, base);
    assert!(world.get::<SquashStretchState>(entity).is_none());
}

#[test]
fn squash_tick_peaks_at_amount_midway() {
    let mut world = make_world(0.1);
    let entity = spawn_squasher(&mut world, Vec2::ONE, SquashTrigger::OnLand);

    send_squash(&mut world, entity, SquashTrigger::OnLand);
    squash_stretch_trigger_system(&mut world);
    flush(&mut world);

    // One 0.1s step into a 0.2s envelope lands exactly on the peak.
    squash_stretch_tick_system(&mut world);
    flush(&mut world);

    let scale = world.get::<Transform>(entity).unwrap().scale;
    let delta = (scale - Vec2::new(1.25, 0.75)).abs();
    assert!(delta.x <= 1e-6 && delta.y <= 1e-6, "scale={scale:?}");
}

#[test]
fn squash_retrigger_mid_flight_does_not_compound_scale() {
    let mut world = make_world(0.05);
    let base = Vec2::new(2.0, 2.0);
    let entity = spawn_squasher(&mut world, base, SquashTrigger::OnLand);

    send_squash(&mut world, entity, SquashTrigger::OnLand);
    squash_stretch_trigger_system(&mut world);
    flush(&mut world);
    squash_stretch_tick_system(&mut world);
    flush(&mut world);
    assert_ne!(world.get::<Transform>(entity).unwrap().scale, base);

    // Re-arm while the squashed scale is live on the Transform.
    world
        .get_resource_mut::<EventQueue<SquashEvent>>()
        .unwrap()
        .flush();
    send_squash(&mut world, entity, SquashTrigger::OnLand);
    squash_stretch_trigger_system(&mut world);
    flush(&mut world);

    assert_eq!(
        world.get::<SquashStretchState>(entity).unwrap().base_scale,
        base
    );

    for _ in 0..5 {
        squash_stretch_tick_system(&mut world);
        flush(&mut world);
    }
    assert_eq!(world.get::<Transform>(entity).unwrap().scale, base);
    assert!(world.get::<SquashStretchState>(entity).is_none());
}

#[test]
fn squash_tick_zero_duration_is_inert_and_self_clearing() {
    let mut world = make_world(0.016);
    let entity = world.spawn();
    world.insert(entity, Transform::default());
    world.insert(
        entity,
        SpriteSquashStretch {
            on: SquashTrigger::Manual,
            amount: Vec2::new(2.0, 0.5),
            duration: 0.0,
            easing: Easing::Linear,
        },
    );

    send_squash(&mut world, entity, SquashTrigger::Manual);
    squash_stretch_trigger_system(&mut world);
    flush(&mut world);
    squash_stretch_tick_system(&mut world);
    flush(&mut world);

    assert_eq!(world.get::<Transform>(entity).unwrap().scale, Vec2::ONE);
    assert!(world.get::<SquashStretchState>(entity).is_none());
}

#[test]
fn squash_tick_no_ops_without_state() {
    let mut world = make_world(0.016);
    let entity = spawn_squasher(&mut world, Vec2::ONE, SquashTrigger::OnLand);
    squash_stretch_tick_system(&mut world);
    flush(&mut world);
    assert_eq!(world.get::<Transform>(entity).unwrap().scale, Vec2::ONE);
}

#[test]
fn squash_retrigger_on_the_completing_frame_survives() {
    // 0.2s envelope at 0.05s steps completes on the fourth tick. A hit landing
    // on that exact frame used to be destroyed by the tick system's buffered
    // removal, because `World::flush` applies commands in queue order.
    let mut world = make_world(0.05);
    let base = Vec2::new(2.0, 2.0);
    let entity = spawn_squasher(&mut world, base, SquashTrigger::OnLand);

    send_squash(&mut world, entity, SquashTrigger::OnLand);
    squash_stretch_trigger_system(&mut world);
    flush(&mut world);

    for frame in 0..4 {
        world
            .get_resource_mut::<EventQueue<SquashEvent>>()
            .unwrap()
            .flush();
        if frame == 3 {
            send_squash(&mut world, entity, SquashTrigger::OnLand);
        }
        squash_stretch_trigger_system(&mut world);
        squash_stretch_tick_system(&mut world);
        flush(&mut world);
    }

    let state = world
        .get::<SquashStretchState>(entity)
        .expect("re-trigger on the completing frame was dropped");
    assert_eq!(state.base_scale, base);
    assert!(state.elapsed < 0.2, "envelope did not restart: {state:?}");
}

#[test]
fn squash_retrigger_restarts_in_place_without_a_flush() {
    let mut world = make_world(0.05);
    let entity = spawn_squasher(&mut world, Vec2::ONE, SquashTrigger::OnLand);

    send_squash(&mut world, entity, SquashTrigger::OnLand);
    squash_stretch_trigger_system(&mut world);
    flush(&mut world);
    squash_stretch_tick_system(&mut world);
    flush(&mut world);
    assert!(world.get::<SquashStretchState>(entity).unwrap().elapsed > 0.0);

    world
        .get_resource_mut::<EventQueue<SquashEvent>>()
        .unwrap()
        .flush();
    send_squash(&mut world, entity, SquashTrigger::OnLand);
    squash_stretch_trigger_system(&mut world);

    // Restart is immediate: no command buffer round trip.
    assert_eq!(
        world.get::<SquashStretchState>(entity).unwrap().elapsed,
        0.0
    );
    assert!(world.get_resource::<CommandBuffer>().unwrap().is_empty());
}

#[test]
fn squash_tick_retires_a_state_orphaned_by_config_removal() {
    let mut world = make_world(0.05);
    let base = Vec2::new(3.0, 3.0);
    let entity = spawn_squasher(&mut world, base, SquashTrigger::OnLand);

    send_squash(&mut world, entity, SquashTrigger::OnLand);
    squash_stretch_trigger_system(&mut world);
    flush(&mut world);
    squash_stretch_tick_system(&mut world);
    flush(&mut world);
    assert_ne!(world.get::<Transform>(entity).unwrap().scale, base);

    world.remove_component::<SpriteSquashStretch>(entity);
    squash_stretch_tick_system(&mut world);
    flush(&mut world);

    assert_eq!(world.get::<Transform>(entity).unwrap().scale, base);
    assert!(world.get::<SquashStretchState>(entity).is_none());
}

#[test]
fn squash_tick_is_inert_on_a_frozen_frame() {
    let mut world = make_world(0.0);
    let entity = spawn_squasher(&mut world, Vec2::ONE, SquashTrigger::OnLand);
    world.insert(
        entity,
        SquashStretchState {
            elapsed: 0.1,
            base_scale: Vec2::ONE,
        },
    );

    squash_stretch_tick_system(&mut world);
    flush(&mut world);

    assert_eq!(
        world.get::<SquashStretchState>(entity).unwrap().elapsed,
        0.1
    );
    assert_eq!(world.get::<Transform>(entity).unwrap().scale, Vec2::ONE);
}
