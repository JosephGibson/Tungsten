use glam::Vec2;
use std::f32::consts::FRAC_PI_2;
use tungsten_core::camera::{self, CameraAim};
use tungsten_core::{
    CameraBounds, CameraController, CameraRig, CameraState, Easing, Entity, FixedRig, FollowRig,
    RigBlend, RigKind, RigLead, Time, Transform, Velocity, With, World,
};

use crate::WindowSize;
use crate::camera::camera_update_system;

const DT: f32 = 1.0 / 60.0;

fn world_at([width, height]: [u32; 2]) -> World {
    let mut world = World::new();
    world.insert_resource(Time::new());
    world.insert_resource(CameraState::new());
    world.insert_resource(CameraController::default());
    world.insert_resource(WindowSize { width, height });
    world
}

/// One frame of `dt`, then the camera's update.
fn frame(world: &mut World, dt: f32) {
    world.resource_mut::<Time>().advance_frame(dt);
    camera_update_system(world);
}

fn base(world: &mut World) -> &mut CameraRig {
    &mut world.resource_mut::<CameraController>().rig
}

fn winner(world: &World) -> Option<Entity> {
    world.resource::<CameraController>().winner()
}

fn position(world: &World) -> Vec2 {
    world.resource::<CameraState>().position
}

/// The drawn view's centre and height, without its shake.
fn view(world: &World) -> (Vec2, f32) {
    let camera = world.resource::<CameraState>().unshaken();
    let window = world.resource::<WindowSize>();
    let size = Vec2::new(window.width as f32, window.height as f32) / camera.zoom;
    (camera.position + size * 0.5, size.y)
}

fn close(actual: Vec2, expected: Vec2, within: f32) -> bool {
    (actual - expected).length() <= within
}

/// A fixed rig with its view's top-left at (`x`, 0).
fn corner(x: f32) -> CameraRig {
    CameraRig::fixed(FixedRig::Corner(Vec2::new(x, 0.0)))
}

#[test]
fn follow_mode_recenters_target_without_dead_zone() {
    let mut world = world_at([800, 600]);
    let target = world.spawn_with((Transform::from_position(Vec2::new(1000.0, 500.0)),));
    base(&mut world).kind = RigKind::Follow(FollowRig::of(target));
    frame(&mut world, 0.016);
    // Top-left anchor: target lands at the viewport center.
    assert_eq!(position(&world), Vec2::new(600.0, 200.0));
}

#[test]
fn dead_zone_holds_camera_until_target_leaves_it() {
    let mut world = world_at([800, 600]);
    let target = world.spawn_with((Transform::from_position(Vec2::new(420.0, 310.0)),));
    base(&mut world).kind =
        RigKind::Follow(FollowRig::of(target).with_dead_zone(Vec2::new(200.0, 150.0)));
    frame(&mut world, 0.016);
    assert_eq!(position(&world), Vec2::ZERO);

    world.get_mut::<Transform>(target).unwrap().position = Vec2::new(900.0, 310.0);
    frame(&mut world, 0.016);
    assert_eq!(position(&world).x, 900.0 - 300.0 - 200.0);
}

#[test]
fn bounds_clamp_still_applies_in_follow_mode() {
    let mut world = world_at([800, 600]);
    let target = world.spawn_with((Transform::from_position(Vec2::new(5000.0, 5000.0)),));
    *base(&mut world) = CameraRig::follow(FollowRig::of(target))
        .with_bounds(CameraBounds::new(Vec2::ZERO, Vec2::new(1000.0, 800.0)));
    frame(&mut world, 0.016);
    assert_eq!(position(&world), Vec2::new(200.0, 200.0));
}

#[test]
fn shake_offset_reaches_camera_state() {
    let mut world = world_at([800, 600]);
    {
        let controller = world.resource_mut::<CameraController>();
        controller.shake_max_offset = Vec2::new(12.0, 0.0);
        controller.shake_trauma = 1.0;
        controller.shake_phase = FRAC_PI_2;
    }
    frame(&mut world, 0.0);
    let position = position(&world);
    assert!((position.x - 12.0).abs() <= 1e-5, "position={position:?}");
}

#[test]
fn shake_does_not_accumulate_into_base_position_across_frames() {
    let mut world = world_at([800, 600]);
    {
        let controller = world.resource_mut::<CameraController>();
        controller.shake_amplitude = Vec2::new(10.0, 10.0);
        controller.shake_max_offset = Vec2::new(20.0, 20.0);
        controller.shake_trauma = 1.0;
        controller.shake_phase = FRAC_PI_2;
    }
    // dt == 0 freezes the phase, so every frame emits the same offset; the
    // rig's view never takes it in.
    frame(&mut world, 0.0);
    let first = position(&world);
    for _ in 0..8 {
        frame(&mut world, 0.0);
    }
    assert_eq!(position(&world), first);
}

#[test]
fn zero_trauma_leaves_position_at_the_base() {
    let mut world = world_at([800, 600]);
    frame(&mut world, 0.016);
    assert_eq!(position(&world), Vec2::ZERO);
}

#[test]
fn missing_controller_leaves_camera_untouched() {
    let mut world = world_at([800, 600]);
    world.remove_resource::<CameraController>();
    world.resource_mut::<CameraState>().position = Vec2::new(5.0, 7.0);
    frame(&mut world, 0.016);
    assert_eq!(position(&world), Vec2::new(5.0, 7.0));
}

#[test]
fn the_highest_live_rig_wins_and_a_tie_goes_to_the_latest_raised() {
    let mut world = world_at([1280, 720]);
    let a = world.spawn_with((corner(100.0).with_priority(1),));
    let b = world.spawn_with((corner(200.0).with_priority(1),));
    let low = world.spawn_with((corner(300.0),));
    frame(&mut world, DT);
    assert_eq!(
        winner(&world),
        Some(b),
        "live from their spawns: b stamped later"
    );
    assert_eq!(position(&world), Vec2::new(200.0, 0.0));
    assert!(camera::raise(&mut world, a));
    frame(&mut world, DT);
    assert_eq!(winner(&world), Some(a), "raised last");
    assert_eq!(position(&world), Vec2::new(100.0, 0.0));
    world.get_mut::<CameraRig>(low).unwrap().priority = 2;
    frame(&mut world, DT);
    assert_eq!(winner(&world), Some(low), "the highest priority");
    assert!(camera::lower(&mut world, low));
    frame(&mut world, DT);
    assert_eq!(winner(&world), Some(a), "lowered, it is no candidate");
    // Raised in Rust, a rig takes its stamp at the next update: the latest.
    {
        let rig = world.get_mut::<CameraRig>(low).unwrap();
        rig.priority = 1;
        rig.raise();
    }
    frame(&mut world, DT);
    assert_eq!(winner(&world), Some(low));
}

#[test]
fn the_base_rig_wins_when_no_other_is_live() {
    let mut world = world_at([1280, 720]);
    base(&mut world).kind = RigKind::Fixed(FixedRig::Corner(Vec2::new(-50.0, 25.0)));
    let rig = world.spawn_with((corner(400.0).lowered(),));
    frame(&mut world, DT);
    assert_eq!(winner(&world), None);
    assert_eq!(position(&world), Vec2::new(-50.0, 25.0));
    camera::raise(&mut world, rig);
    frame(&mut world, DT);
    assert_eq!(winner(&world), Some(rig));
    base(&mut world).priority = 1;
    frame(&mut world, DT);
    assert_eq!(winner(&world), None, "a base rig above it");
    base(&mut world).priority = 0;
    frame(&mut world, DT);
    assert_eq!(
        winner(&world),
        Some(rig),
        "a tie: the base was raised first"
    );
    world.despawn(rig);
    frame(&mut world, DT);
    assert_eq!(winner(&world), None, "its entity gone");
    assert_eq!(position(&world), Vec2::new(-50.0, 25.0));
    assert!(
        !camera::raise(&mut world, rig),
        "a dead entity holds no rig"
    );
}

#[test]
fn two_raises_in_one_step_keep_their_order() {
    let mut world = world_at([1280, 720]);
    let a = world.spawn_with((corner(100.0).lowered(),));
    let b = world.spawn_with((corner(200.0).lowered(),));
    frame(&mut world, DT);
    let met: Vec<Entity> = world.query_filtered::<Entity, With<CameraRig>>().collect();
    assert_eq!(met, [a, b], "the query meets a first");
    // In one step, b raised and then a: stamped as raised, a is the later.
    camera::raise(&mut world, b);
    camera::raise(&mut world, a);
    frame(&mut world, DT);
    assert_eq!(winner(&world), Some(a));
    assert_eq!(position(&world), Vec2::new(100.0, 0.0));
}

/// Two fixed rigs 600 px apart, from a view height of 720 to 1,008, over
/// 0.5 s of `sine_in_out` at `dt`: the view before the switch, then each
/// frame's from it on, about 0.7 s of them.
fn switch(dt: f32) -> Vec<(Vec2, f32)> {
    let mut world = world_at([1280, 720]);
    *base(&mut world) =
        CameraRig::fixed(FixedRig::At(Vec2::new(640.0, 360.0))).with_view_height(720.0);
    let incoming = world.spawn_with((CameraRig::fixed(FixedRig::At(Vec2::new(1240.0, 360.0)))
        .with_view_height(1008.0)
        .with_blend(RigBlend::over(0.5))
        .lowered(),));
    frame(&mut world, dt);
    let mut views = vec![view(&world)];
    camera::raise(&mut world, incoming);
    for _ in 0..(0.7 / dt).round() as usize {
        frame(&mut world, dt);
        views.push(view(&world));
    }
    views
}

#[test]
fn a_switch_never_jumps_at_30_60_or_144_hz() {
    let rates = [1.0 / 30.0, 1.0 / 60.0, 1.0 / 144.0];
    let runs: Vec<Vec<(Vec2, f32)>> = rates.iter().map(|&dt| switch(dt)).collect();
    for (&dt, views) in rates.iter().zip(&runs) {
        let centre_bound = 600.0 * FRAC_PI_2 / 0.5 * dt + 0.01;
        let height_bound = 1008.0 * 1.4_f32.ln() * FRAC_PI_2 / 0.5 * dt + 0.01;
        for (k, pair) in views.windows(2).enumerate() {
            let step = (pair[1].0 - pair[0].0).length();
            let rise = (pair[1].1 - pair[0].1).abs();
            assert!(
                step <= centre_bound,
                "dt {dt}, frame {k}: a centre step of {step} past {centre_bound}"
            );
            assert!(
                rise <= height_bound,
                "dt {dt}, frame {k}: a height step of {rise} past {height_bound}"
            );
        }
        // views[k] is k frames after the switch, k·dt into the blend.
        let on = (0.5 / dt).round() as usize;
        for &(centre, height) in &views[on..] {
            assert!(
                close(centre, Vec2::new(1240.0, 360.0), 1e-3) && (height - 1008.0).abs() < 1e-3,
                "dt {dt}: on the incoming view at 0.5 s, not {centre} at {height}"
            );
        }
        assert_eq!(views[on + 1], *views.last().unwrap(), "dt {dt}: held");
    }
    for t in [1.0 / 6.0, 1.0 / 3.0] {
        let at: Vec<(Vec2, f32)> = rates
            .iter()
            .zip(&runs)
            .map(|(&dt, views)| views[(t / dt).round() as usize])
            .collect();
        for &(centre, height) in &at[1..] {
            assert!(
                close(centre, at[0].0, 0.01) && (height - at[0].1).abs() <= 0.01,
                "at {t} s: {at:?}"
            );
        }
    }
}

#[test]
fn an_overshooting_ease_keeps_the_height_positive() {
    // Heights lerped from 100 to 2,000 under back_in would pass under 0.
    let dip = (0..=100)
        .map(|i| Easing::BackIn.apply(i as f32 / 100.0))
        .fold(f32::INFINITY, f32::min);
    assert!(100.0 + 1900.0 * dip < 0.0, "back_in dips to {dip}");
    let mut world = world_at([1280, 720]);
    *base(&mut world) = CameraRig::fixed(FixedRig::At(Vec2::ZERO)).with_view_height(100.0);
    let tall = world.spawn_with((CameraRig::fixed(FixedRig::At(Vec2::ZERO))
        .with_view_height(2000.0)
        .with_blend(RigBlend::over(0.5).with_ease(Easing::BackIn))
        .lowered(),));
    frame(&mut world, DT);
    camera::raise(&mut world, tall);
    let mut lowest = f32::INFINITY;
    for _ in 0..40 {
        frame(&mut world, DT);
        let zoom = world.resource::<CameraState>().zoom;
        assert!(zoom.is_finite() && zoom > 0.0, "zoom {zoom}");
        lowest = lowest.min(view(&world).1);
    }
    assert!(
        lowest > 0.0 && lowest < 100.0,
        "under the start, above 0: {lowest}"
    );
    assert!((view(&world).1 - 2000.0).abs() < 0.01);
}

#[test]
fn a_cut_lands_in_one_frame() {
    let mut world = world_at([1280, 720]);
    *base(&mut world) = CameraRig::fixed(FixedRig::At(Vec2::new(640.0, 360.0)));
    let cut = world.spawn_with((CameraRig::fixed(FixedRig::At(Vec2::new(5000.0, -200.0)))
        .with_view_height(360.0)
        .lowered(),));
    frame(&mut world, DT);
    camera::raise(&mut world, cut);
    frame(&mut world, DT);
    let camera = *world.resource::<CameraState>();
    assert_eq!(camera.zoom, 2.0);
    assert_eq!(camera.position, Vec2::new(5000.0 - 320.0, -200.0 - 180.0));
    assert_eq!(world.resource::<CameraController>().frame().blend, None);
}

#[test]
fn a_switch_mid_blend_starts_from_the_drawn_view() {
    let mut world = world_at([1280, 720]);
    *base(&mut world) = CameraRig::fixed(FixedRig::At(Vec2::new(640.0, 360.0)));
    let blended = |x: f32| {
        CameraRig::fixed(FixedRig::At(Vec2::new(x, 360.0)))
            .with_blend(RigBlend::over(0.5))
            .lowered()
    };
    let b = world.spawn_with((blended(1240.0),));
    let c = world.spawn_with((blended(-1000.0),));
    frame(&mut world, DT);
    camera::raise(&mut world, b);
    for _ in 0..15 {
        frame(&mut world, DT);
    }
    let drawn = view(&world).0;
    assert!((drawn.x - 940.0).abs() < 0.1, "half way at 0.25 s: {drawn}");
    camera::raise(&mut world, c);
    frame(&mut world, DT);
    // C's first frame: its ease over one frame of the way from the drawn view.
    let first = (-1000.0 - drawn.x) * Easing::SineInOut.apply(DT / 0.5);
    let next = view(&world).0;
    assert!(
        (next.x - (drawn.x + first)).abs() < 0.01,
        "{next} from {drawn}"
    );
    for _ in 0..30 {
        frame(&mut world, DT);
    }
    assert!(close(view(&world).0, Vec2::new(-1000.0, 360.0), 1e-3));
    assert_eq!(winner(&world), Some(c));
}

#[test]
fn bounds_and_view_height_ride_the_blend() {
    let mut world = world_at([1280, 720]);
    *base(&mut world) = corner(0.0);
    // Its point lies past its bounds, so its view stays in them at 360 tall:
    // 640 by 360 with its corner at (2360, 640).
    let bounds = CameraBounds::new(Vec2::ZERO, Vec2::new(3000.0, 1000.0));
    let boxed = world.spawn_with((CameraRig::fixed(FixedRig::At(Vec2::new(9000.0, 9000.0)))
        .with_bounds(bounds)
        .with_view_height(360.0)
        .with_blend(RigBlend::over(0.5).with_ease(Easing::Linear))
        .lowered(),));
    frame(&mut world, DT);
    camera::raise(&mut world, boxed);
    let live = Vec2::new(2680.0, 820.0);
    for k in 1..=30 {
        frame(&mut world, DT);
        let (centre, height) = view(&world);
        let s = (k as f32 * DT / 0.5).min(1.0);
        let expected = Vec2::new(640.0, 360.0).lerp(live, s);
        assert!(
            close(centre, expected, 0.05),
            "frame {k}: {centre} for {expected}"
        );
        let tall = 720.0 * 0.5_f32.powf(s);
        assert!(
            (height - tall).abs() < 0.05,
            "frame {k}: {height} for {tall}"
        );
    }
    assert!(close(view(&world).0, live, 1e-3));
    assert!((view(&world).1 - 360.0).abs() < 1e-3);
    assert_eq!(
        world.resource::<CameraController>().frame().bounds,
        Some(bounds)
    );
}

#[test]
fn a_gone_winner_hands_over_without_a_jump() {
    let mut world = world_at([1280, 720]);
    *base(&mut world) =
        CameraRig::fixed(FixedRig::At(Vec2::new(640.0, 360.0))).with_blend(RigBlend::over(0.5));
    let gone = world.spawn_with((CameraRig::fixed(FixedRig::At(Vec2::new(1840.0, 360.0))),));
    frame(&mut world, DT);
    assert_eq!(winner(&world), Some(gone), "cut in: nothing drawn before");
    let mut last = view(&world).0;
    assert!(close(last, Vec2::new(1840.0, 360.0), 1e-3));
    world.despawn(gone);
    let bound = 1200.0 * FRAC_PI_2 / 0.5 * DT + 0.01;
    for _ in 0..40 {
        frame(&mut world, DT);
        let now = view(&world).0;
        assert!(
            (now - last).length() <= bound,
            "a step from {last} to {now}"
        );
        last = now;
    }
    assert_eq!(winner(&world), None);
    assert!(close(last, Vec2::new(640.0, 360.0), 1e-3));
}

#[test]
fn a_follow_starts_at_the_last_view_and_smooths_each_axis() {
    let mut world = world_at([1280, 720]);
    *base(&mut world) = CameraRig::fixed(FixedRig::Corner(Vec2::new(100.0, 50.0)));
    let target = world.spawn_with((Transform::from_position(Vec2::new(2000.0, 1000.0)),));
    let follow = world
        .spawn_with((
            CameraRig::follow(FollowRig::of(target).with_smoothing(Vec2::new(1.0, 0.5))).lowered(),
        ));
    frame(&mut world, DT);
    camera::raise(&mut world, follow);
    frame(&mut world, DT);
    // From (100, 50): x snaps to centre the target, y covers half the way
    // in 1/60 s.
    let desired = Vec2::new(2000.0 - 640.0, 1000.0 - 360.0);
    let expected = Vec2::new(desired.x, 50.0 + (desired.y - 50.0) * 0.5);
    assert!(
        close(position(&world), expected, 1e-3),
        "{}",
        position(&world)
    );
    // A follow at another height starts at the last view's centre, which
    // a smoothing of 0 keeps.
    let centre = view(&world).0;
    let still = world.spawn_with((CameraRig::follow(
        FollowRig::of(target).with_smoothing(Vec2::ZERO),
    )
    .with_view_height(360.0)
    .lowered(),));
    camera::raise(&mut world, still);
    frame(&mut world, DT);
    let (now, height) = view(&world);
    assert!(close(now, centre, 1e-3), "{now} for {centre}");
    assert!((height - 360.0).abs() < 1e-3);
}

/// The winner's lead this frame: its led point less its anchor.
fn lead(world: &World) -> Vec2 {
    let frame = world.resource::<CameraController>().frame();
    frame.lead.unwrap() - frame.anchor.unwrap()
}

#[test]
fn a_lead_runs_ahead_capped_and_eased() {
    let mut world = world_at([1280, 720]);
    let runner = world.spawn_with((
        Transform::from_position(Vec2::new(640.0, 360.0)),
        Velocity(Vec2::new(400.0, 0.0)),
    ));
    let ahead = RigLead::velocity(96.0, 0.5).with_smoothing(0.5);
    base(&mut world).kind = RigKind::Follow(FollowRig::of(runner).with_lead(ahead));
    frame(&mut world, DT);
    // v · time is 200 px, capped at 96, half of it after one 1/60 s frame.
    assert_eq!(
        world.resource::<CameraController>().frame().anchor,
        Some(Vec2::new(640.0, 360.0))
    );
    assert!(
        close(lead(&world), Vec2::new(48.0, 0.0), 1e-3),
        "{}",
        lead(&world)
    );
    for _ in 0..60 {
        frame(&mut world, DT);
    }
    assert!(close(lead(&world), Vec2::new(96.0, 0.0), 1e-3), "capped");
    let led = world.resource::<CameraController>().frame().lead.unwrap();
    assert!(
        close(view(&world).0, led, 1e-3),
        "the view keeps the led point"
    );
    // Without a Velocity, the drawn point's change: 2 px a frame at 60 Hz
    // is 120 px/s, 60 px ahead at half a second.
    world.remove_component::<Velocity>(runner);
    for _ in 0..120 {
        world.get_mut::<Transform>(runner).unwrap().position.y += 2.0;
        frame(&mut world, DT);
    }
    assert!(
        close(lead(&world), Vec2::new(0.0, 60.0), 0.05),
        "{}",
        lead(&world)
    );
    // A pause holds it.
    let held = lead(&world);
    frame(&mut world, 0.0);
    assert_eq!(lead(&world), held);
}

#[test]
fn an_aim_lead_reads_the_aim_source() {
    let mut world = world_at([1280, 720]);
    let gunner = world.spawn_with((Transform::from_position(Vec2::new(640.0, 360.0)),));
    base(&mut world).kind =
        RigKind::Follow(FollowRig::of(gunner).with_lead(RigLead::aim(96.0).with_smoothing(1.0)));
    frame(&mut world, DT);
    assert_eq!(lead(&world), Vec2::ZERO, "no aim source");
    world.insert_resource(CameraAim(|_, _| Some(Vec2::Y)));
    frame(&mut world, DT);
    assert_eq!(lead(&world), Vec2::new(0.0, 96.0));
    assert_eq!(position(&world), Vec2::new(0.0, 96.0), "the view led down");
    world.insert_resource(CameraAim(|_, _| None));
    frame(&mut world, DT);
    assert_eq!(lead(&world), Vec2::ZERO, "no aim for it");
}

#[test]
fn a_fixed_rig_takes_a_point_a_corner_or_a_rect() {
    let mut world = world_at([1280, 720]);
    base(&mut world).kind = RigKind::Fixed(FixedRig::At(Vec2::new(1000.0, 500.0)));
    frame(&mut world, DT);
    assert_eq!(position(&world), Vec2::new(360.0, 140.0));
    base(&mut world).kind = RigKind::Fixed(FixedRig::Corner(Vec2::new(-30.0, 12.5)));
    frame(&mut world, DT);
    assert_eq!(position(&world), Vec2::new(-30.0, 12.5));
    // A box wider than the window's aspect shows its width, centred.
    let wide = CameraBounds::new(Vec2::ZERO, Vec2::new(2560.0, 400.0));
    base(&mut world).kind = RigKind::Fixed(FixedRig::Rect(wide));
    frame(&mut world, DT);
    let camera = *world.resource::<CameraState>();
    assert_eq!(camera.zoom, 0.5);
    assert_eq!(camera.position, Vec2::new(0.0, 200.0 - 720.0));
    // A taller one its height.
    let tall = CameraBounds::new(Vec2::ZERO, Vec2::new(400.0, 2000.0));
    base(&mut world).kind = RigKind::Fixed(FixedRig::Rect(tall));
    frame(&mut world, DT);
    let (centre, height) = view(&world);
    assert!(close(centre, Vec2::new(200.0, 1000.0), 1e-2) && (height - 2000.0).abs() < 1e-2);
    // At the rig's own view height, centred on the box.
    base(&mut world).kind = RigKind::Fixed(FixedRig::Rect(wide));
    base(&mut world).view_height = Some(360.0);
    frame(&mut world, DT);
    assert_eq!(position(&world), Vec2::new(1280.0 - 320.0, 200.0 - 180.0));
}

#[test]
fn a_scripted_rig_follows_its_point() {
    let mut world = world_at([1280, 720]);
    let rig =
        world.spawn_with((CameraRig::scripted(Vec2::new(100.0, 100.0)).with_view_height(360.0),));
    frame(&mut world, DT);
    assert_eq!(position(&world), Vec2::new(100.0 - 320.0, 100.0 - 180.0));
    world
        .get_mut::<CameraRig>(rig)
        .unwrap()
        .set_point(Vec2::new(-400.0, 50.0));
    frame(&mut world, DT);
    assert_eq!(position(&world), Vec2::new(-400.0 - 320.0, 50.0 - 180.0));
    assert_eq!(world.get::<CameraRig>(rig).unwrap().kind.name(), "scripted");
}

#[test]
fn the_view_height_sets_the_zoom_at_every_window() {
    for [width, height] in [[1280, 720], [1920, 1080], [2560, 1080], [800, 600]] {
        let mut world = world_at([width, height]);
        *base(&mut world) = corner(0.0).with_view_height(720.0);
        frame(&mut world, DT);
        let zoom = |world: &World| world.resource::<CameraState>().zoom;
        assert_eq!(zoom(&world), height as f32 / 720.0, "{width}x{height}");
        base(&mut world).zoom = 2.0;
        frame(&mut world, DT);
        assert_eq!(zoom(&world), height as f32 / 720.0 * 2.0);
        assert!((view(&world).1 - 360.0).abs() < 1e-3, "half the height");
        base(&mut world).view_height = None;
        frame(&mut world, DT);
        assert_eq!(zoom(&world), 2.0, "the window's height at zoom 1, scaled");
    }
}

#[test]
fn the_shake_comes_after_the_winner_and_scales() {
    let mut world = world_at([1280, 720]);
    // Clamped from (-500, 0) to (0, 0) first; the shake then moves the view.
    *base(&mut world) =
        corner(-500.0).with_bounds(CameraBounds::new(Vec2::ZERO, Vec2::new(1280.0, 720.0)));
    {
        let controller = world.resource_mut::<CameraController>();
        controller.shake_max_offset = Vec2::new(12.0, 8.0);
        controller.shake_trauma = 1.0;
        controller.shake_phase = FRAC_PI_2;
    }
    frame(&mut world, 0.0);
    let camera = *world.resource::<CameraState>();
    assert!(
        close(camera.shake, Vec2::new(12.0, 0.0), 1e-5),
        "{}",
        camera.shake
    );
    assert_eq!(camera.position, camera.shake, "the corner plus the shake");
    assert_eq!(camera.unshaken().position, Vec2::ZERO);
    world.resource_mut::<CameraController>().shake_scale = 0.5;
    frame(&mut world, 0.0);
    let camera = *world.resource::<CameraState>();
    assert!(close(camera.shake, Vec2::new(6.0, 0.0), 1e-5), "scaled");
    // It shakes whichever rig wins.
    world.spawn_with((CameraRig::fixed(FixedRig::Corner(Vec2::new(300.0, 40.0))),));
    frame(&mut world, 0.0);
    let camera = *world.resource::<CameraState>();
    assert!(close(camera.position, Vec2::new(306.0, 40.0), 1e-4));
    assert!(close(
        camera.unshaken().position,
        Vec2::new(300.0, 40.0),
        1e-4
    ));
}

#[test]
fn a_write_to_the_camera_state_is_never_read_back() {
    let mut world = world_at([1280, 720]);
    *base(&mut world) =
        CameraRig::fixed(FixedRig::Corner(Vec2::new(40.0, 20.0))).with_view_height(360.0);
    frame(&mut world, DT);
    {
        let camera = world.resource_mut::<CameraState>();
        camera.position = Vec2::new(900.0, 900.0);
        camera.zoom = 7.0;
    }
    frame(&mut world, DT);
    assert_eq!(position(&world), Vec2::new(40.0, 20.0));
    assert_eq!(world.resource::<CameraState>().zoom, 2.0);
    // A zoom written each frame never compounds with the rig's.
    for _ in 0..3 {
        world.resource_mut::<CameraState>().zoom = 2.0;
        frame(&mut world, DT);
        assert_eq!(world.resource::<CameraState>().zoom, 2.0);
    }
}

#[test]
fn a_cut_to_restarts_the_winner_there_with_no_blend() {
    let mut world = world_at([1280, 720]);
    *base(&mut world) = CameraRig::fixed(FixedRig::At(Vec2::new(640.0, 360.0)));
    let target = world.spawn_with((Transform::from_position(Vec2::new(2000.0, 360.0)),));
    // A dead zone of the whole view: it moves only to keep the target seen.
    let follow = world.spawn_with((CameraRig::follow(
        FollowRig::of(target).with_dead_zone(Vec2::new(1280.0, 720.0)),
    )
    .with_blend(RigBlend::over(1.0))
    .lowered(),));
    frame(&mut world, DT);
    camera::raise(&mut world, follow);
    for _ in 0..10 {
        frame(&mut world, DT);
    }
    assert!(world.resource::<CameraController>().frame().blend.is_some());
    world
        .resource_mut::<CameraController>()
        .cut_to(Vec2::new(1900.0, 400.0));
    for _ in 0..3 {
        frame(&mut world, DT);
        assert_eq!(world.resource::<CameraController>().frame().blend, None);
        assert!(close(view(&world).0, Vec2::new(1900.0, 400.0), 1e-3));
        assert_eq!(winner(&world), Some(follow));
    }
}

#[test]
fn a_pause_holds_a_blends_progress() {
    let mut world = world_at([1280, 720]);
    *base(&mut world) = CameraRig::scripted(Vec2::new(640.0, 360.0)).with_view_height(720.0);
    let incoming = world.spawn_with((CameraRig::scripted(Vec2::new(1240.0, 360.0))
        .with_view_height(1008.0)
        .with_blend(RigBlend::over(0.5))
        .lowered(),));
    frame(&mut world, DT);
    camera::raise(&mut world, incoming);
    for _ in 0..15 {
        frame(&mut world, DT);
    }
    let share = world.resource::<CameraController>().frame().blend.unwrap();
    assert!((share - 0.5).abs() < 1e-3, "half way: {share}");
    let held = view(&world);
    for _ in 0..5 {
        frame(&mut world, 0.0);
        assert_eq!(view(&world), held, "paused, the view holds");
    }
    // Under the pause the incoming point moves 100 px and its height to
    // 1,200: the view moves by the frozen weight's share alone.
    {
        let rig = world.get_mut::<CameraRig>(incoming).unwrap();
        rig.set_point(Vec2::new(1340.0, 360.0));
        rig.view_height = Some(1200.0);
    }
    frame(&mut world, 0.0);
    let weight = Easing::SineInOut.apply(share);
    let (moved, grown) = view(&world);
    let expected = Vec2::new(640.0 + (1340.0 - 640.0) * weight, 360.0);
    assert!(close(moved, expected, 0.01), "{moved} for {expected}");
    let tall = 720.0 * (1200.0_f32 / 720.0).powf(weight);
    assert!((grown - tall).abs() < 0.01, "{grown} for {tall}");
    assert_eq!(
        world.resource::<CameraController>().frame().blend,
        Some(share)
    );
    // Resumed, the blend goes on from there with no step past its bound.
    let bound = (1340.0 - 640.0) * FRAC_PI_2 / 0.5 * DT + 0.01;
    let height_bound = 1200.0 * (1200.0_f32 / 720.0).ln() * FRAC_PI_2 / 0.5 * DT + 0.01;
    let mut last = (moved, grown);
    for _ in 0..20 {
        frame(&mut world, DT);
        let now = view(&world);
        assert!((now.0 - last.0).length() <= bound, "{last:?} to {now:?}");
        assert!((now.1 - last.1).abs() <= height_bound);
        last = now;
    }
    assert!(close(last.0, Vec2::new(1340.0, 360.0), 1e-3));
    assert!((last.1 - 1200.0).abs() < 1e-3);
}

#[test]
fn the_update_draws_its_shapes_while_asked() {
    use tungsten_core::{DebugDraw, DebugShape};
    let mut world = world_at([1280, 720]);
    world.insert_resource(DebugDraw::new());
    let runner = world.spawn_with((
        Transform::from_position(Vec2::new(640.0, 360.0)),
        Velocity(Vec2::new(300.0, 0.0)),
    ));
    let bounds = CameraBounds::new(Vec2::new(-1000.0, -1000.0), Vec2::new(3000.0, 2000.0));
    *base(&mut world) = CameraRig::follow(
        FollowRig::of(runner)
            .with_dead_zone(Vec2::new(200.0, 100.0))
            .with_lead(RigLead::velocity(60.0, 0.1).with_smoothing(1.0)),
    )
    .with_bounds(bounds);
    frame(&mut world, DT);
    assert!(world.resource::<DebugDraw>().is_empty(), "off by default");
    world.resource_mut::<CameraController>().draw_shapes = true;
    // Moved this frame: the shapes stand at this frame's view.
    world.get_mut::<Transform>(runner).unwrap().position.x += 400.0;
    frame(&mut world, DT);
    let camera = world.resource::<CameraState>().unshaken();
    let drawn = world.resource::<CameraController>().frame();
    assert_eq!(drawn.corner, camera.position, "this frame's view");
    let commands = world.resource::<DebugDraw>().commands().to_vec();
    let boxes: Vec<(Vec2, Vec2)> = commands
        .iter()
        .filter_map(|command| match command.shape {
            DebugShape::Aabb { min, max } => Some((min, max)),
            _ => None,
        })
        .collect();
    let zone = drawn.dead_zone.unwrap();
    let size = Vec2::new(1280.0, 720.0);
    assert!(close(
        zone.min,
        camera.position + (size - Vec2::new(200.0, 100.0)) * 0.5,
        1e-3
    ));
    assert_eq!(boxes, [(zone.min, zone.max), (bounds.min, bounds.max)]);
    let lines: Vec<(Vec2, Vec2)> = commands
        .iter()
        .filter_map(|command| match command.shape {
            DebugShape::Line { a, b } => Some((a, b)),
            _ => None,
        })
        .collect();
    assert_eq!(lines.len(), 4, "two crosses");
    let anchor = Vec2::new(1040.0, 360.0);
    let led = anchor + Vec2::new(30.0, 0.0);
    for (pair, at) in lines.chunks(2).zip([anchor, led]) {
        for (a, b) in pair {
            assert!(close((*a + *b) * 0.5, at, 1e-3), "a cross at {at}");
        }
    }
    assert_eq!((drawn.anchor, drawn.lead), (Some(anchor), Some(led)));
    // Off: nothing.
    world.resource_mut::<DebugDraw>().clear();
    world.resource_mut::<CameraController>().draw_shapes = false;
    frame(&mut world, DT);
    assert!(world.resource::<DebugDraw>().is_empty());
}

/// A world whose camera sits at the origin, holding a laid-out 40 by 20
/// root anchored at `point` moved by `offset`: (world, root).
fn anchored(point: Vec2, offset: Vec2) -> (World, tungsten_core::ui::WidgetId) {
    use tungsten_core::ui::{FixedAdvanceMeasure, LayoutStyle, Sizing, UiLayer, UiTree};
    let mut world = world_at([800, 600]);
    let mut tree = UiTree::new();
    let root = tree.add_root(
        UiLayer::Game,
        LayoutStyle::default().with_size(Sizing::Px(40.0), Sizing::Px(20.0)),
    );
    tree.layout(Vec2::new(800.0, 600.0), &mut FixedAdvanceMeasure::default());
    tree.anchor_to_world(root, point, offset);
    world.insert_resource(tree);
    (world, root)
}

fn translation(world: &World, root: tungsten_core::ui::WidgetId) -> Option<Vec2> {
    world
        .resource::<tungsten_core::ui::UiTree>()
        .translation(root)
}

#[test]
fn a_world_anchored_root_follows_its_point() {
    let (mut world, root) = anchored(Vec2::new(300.0, 200.0), Vec2::new(5.0, -10.0));
    frame(&mut world, 0.016);
    assert_eq!(translation(&world, root), Some(Vec2::new(305.0, 190.0)));
    *base(&mut world) = CameraRig::fixed(FixedRig::Corner(Vec2::new(100.0, 50.0)));
    frame(&mut world, 0.016);
    assert_eq!(
        translation(&world, root),
        Some(Vec2::new(205.0, 140.0)),
        "it follows the view"
    );
    world
        .resource_mut::<tungsten_core::ui::UiTree>()
        .clear_world_anchor(root);
    *base(&mut world) = corner(0.0);
    frame(&mut world, 0.016);
    assert_eq!(
        translation(&world, root),
        Some(Vec2::new(205.0, 140.0)),
        "cleared, it stays"
    );
}

#[test]
fn a_world_anchored_root_stays_in_the_view() {
    let (mut world, root) = anchored(Vec2::new(790.0, 595.0), Vec2::ZERO);
    frame(&mut world, 0.016);
    assert_eq!(
        translation(&world, root),
        Some(Vec2::new(760.0, 580.0)),
        "its last size inside the viewport"
    );
    world
        .resource_mut::<tungsten_core::ui::UiTree>()
        .anchor_to_world(root, Vec2::new(-50.0, -50.0), Vec2::ZERO);
    frame(&mut world, 0.016);
    assert_eq!(translation(&world, root), Some(Vec2::ZERO));
}
