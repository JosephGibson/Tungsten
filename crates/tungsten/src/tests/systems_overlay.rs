use super::*;
use tungsten_core::input::KeyCode;

fn world_with_timings(systems: &[(&str, f32)]) -> World {
    let mut world = World::new();
    let mut ft = FrameTimings::new();
    ft.system_timings = systems.iter().map(|(n, v)| (n.to_string(), *v)).collect();
    world.insert_resource(ft);
    world
}

#[test]
fn compose_returns_empty_when_disabled() {
    let mut overlay = SystemTimingOverlay::default();
    let world = world_with_timings(&[("a", 1.0)]);
    let out = compose_systems_overlay_text_section(&mut overlay, &world, (1280, 720), 16.0);
    assert!(out.is_empty());
}

#[test]
fn ewma_converges_on_constant_input() {
    let mut overlay = SystemTimingOverlay {
        enabled: true,
        refresh_interval_ms: 0.0,
        ..Default::default()
    };
    let world = world_with_timings(&[("a", 2.0), ("b", 4.0)]);

    for _ in 0..200 {
        let _ = compose_systems_overlay_text_section(&mut overlay, &world, (1280, 720), 16.0);
    }
    let a = overlay.ewma_for("a").unwrap();
    let b = overlay.ewma_for("b").unwrap();
    assert!((a - 2.0).abs() < 0.01, "a = {a}");
    assert!((b - 4.0).abs() < 0.01, "b = {b}");
}

#[test]
fn stale_system_name_is_dropped() {
    let mut overlay = SystemTimingOverlay {
        enabled: true,
        refresh_interval_ms: 0.0,
        ..Default::default()
    };

    let world_a = world_with_timings(&[("alpha", 1.0), ("beta", 1.0)]);
    let _ = compose_systems_overlay_text_section(&mut overlay, &world_a, (1280, 720), 16.0);
    assert!(overlay.ewma_for("alpha").is_some());
    assert!(overlay.ewma_for("beta").is_some());

    let world_b = world_with_timings(&[("alpha", 1.0)]);
    let _ = compose_systems_overlay_text_section(&mut overlay, &world_b, (1280, 720), 16.0);
    assert!(overlay.ewma_for("alpha").is_some());
    assert!(overlay.ewma_for("beta").is_none());
}

#[test]
fn overlay_y_is_pushed_below_hud_when_hud_is_left_anchored() {
    use crate::debug_hud::{compose_hud_text_sections, DebugHud};

    let mut world = world_with_timings(&[("alpha", 1.0)]);
    let mut hud = DebugHud::new();
    hud.enabled = true;
    hud.corner = HudCorner::TopLeft;
    hud.refresh_interval_ms = 0.0;
    // Prime the HUD so its cached_sections populate, then reinsert so
    // the overlay can read the height back out of the world.
    let _ = compose_hud_text_sections(&mut hud, &world, (1280, 720), 16.0);
    let hud_bottom = hud.rendered_height_px();
    assert!(hud_bottom > 0.0);
    world.insert_resource(hud);

    let mut overlay = SystemTimingOverlay {
        enabled: true,
        refresh_interval_ms: 0.0,
        corner: HudCorner::TopLeft,
        ..Default::default()
    };
    let sections = compose_systems_overlay_text_section(&mut overlay, &world, (1280, 720), 16.0);
    assert!(!sections.is_empty());
    let main_y = sections.last().unwrap().position[1];
    assert!(
        main_y >= hud_bottom,
        "overlay y={main_y} should be >= hud_bottom={hud_bottom}"
    );
}

#[test]
fn toggle_on_f2_action_flips_enabled() {
    let mut world = World::new();
    let mut input = InputState::new();
    input.key_down(KeyCode::F2);
    world.insert_resource(input);
    world.insert_resource(ActionMap::default_map());
    world.insert_resource(SystemTimingOverlay::default());

    systems_overlay_toggle_system(&mut world);

    assert!(world.get_resource::<SystemTimingOverlay>().unwrap().enabled);
}
