use super::*;
use tungsten_core::input::{InputState, KeyCode};
use tungsten_core::ActionMap;

#[test]
fn default_hud_is_disabled_and_empty_world_yields_no_rows() {
    let mut hud = DebugHud::new();
    assert!(!hud.enabled);
    let world = World::new();
    let sections = compose_hud_text_sections(&mut hud, &world, (1280, 720), 16.67);
    assert!(sections.is_empty());
}

#[test]
fn toggle_flips_enabled() {
    let mut hud = DebugHud::new();
    assert!(!hud.enabled);
    hud.toggle();
    assert!(hud.enabled);
    hud.toggle();
    assert!(!hud.enabled);
}

#[test]
fn action_map_hud_toggle_uses_engine_action() {
    let mut world = World::new();
    let mut input = InputState::new();
    input.key_down(KeyCode::F4);
    world.insert_resource(input);
    world.insert_resource(ActionMap::default_map());
    world.insert_resource(DebugHud::new());

    hud_toggle_system(&mut world);

    assert!(world.get_resource::<DebugHud>().unwrap().enabled);
}

#[test]
fn hud_excludes_player_state_and_systems_rows() {
    // Regression for the render-focused refactor: the HUD must not
    // surface per-entity or per-system info anymore. Those live in the
    // inspector (F3) and systems overlay (F2) respectively.
    let mut world = World::new();
    let mut ft = FrameTimings::new();
    ft.system_timings = vec![("sys_a".into(), 1.0), ("sys_b".into(), 5.0)];
    world.insert_resource(ft);
    world.insert_resource(HudActiveState("gameplay".into()));

    let mut hud = DebugHud::new();
    hud.enabled = true;
    hud.refresh_interval_ms = 0.0;
    let sections = compose_hud_text_sections(&mut hud, &world, (1280, 720), 16.67);
    let content = &sections.last().unwrap().content;
    assert!(!content.contains("player"));
    assert!(!content.contains("state"));
    assert!(!content.contains("gameplay"));
    assert!(!content.contains("sys_a"));
    assert!(!content.contains("sys_b"));
}

#[test]
fn counts_row_renders_entity_and_sprite_totals() {
    let mut world = World::new();
    world.insert_resource(RenderCounts {
        entities: 42,
        sprite_instances: 128,
    });

    let mut hud = DebugHud::new();
    hud.enabled = true;
    hud.refresh_interval_ms = 0.0;
    let sections = compose_hud_text_sections(&mut hud, &world, (1280, 720), 16.67);
    let content = &sections.last().unwrap().content;
    assert!(
        content.contains("draw ") && content.contains("42 ents") && content.contains("128 spr"),
        "draw row missing entity/sprite totals in:\n{content}"
    );
}

#[test]
fn custom_row_appears_after_built_in() {
    let mut hud = DebugHud::new();
    hud.enabled = true;
    hud.add_row(|_| {
        vec![HudRow {
            label: "extra",
            value: "value".into(),
        }]
    });
    let world = World::new();
    let sections = compose_hud_text_sections(&mut hud, &world, (1280, 720), 16.67);
    assert!(!sections.is_empty());
    let content = &sections.last().unwrap().content;
    // fps row (built-in, emitted first) must precede the custom "extra" row.
    let fps_idx = content.find("fps").expect("fps row missing");
    let extra_idx = content.find("extra").expect("extra row missing");
    assert!(fps_idx < extra_idx);
}

#[test]
fn refresh_throttle_reuses_cached_sections_between_ticks() {
    let mut hud = DebugHud::new();
    hud.enabled = true;
    hud.refresh_interval_ms = 100.0;
    hud.add_row(|_| {
        vec![HudRow {
            label: "tick",
            value: "one".into(),
        }]
    });
    let world = World::new();

    // First call always rebuilds (cache empty, time seeded to +inf).
    let first = compose_hud_text_sections(&mut hud, &world, (1280, 720), 16.67);
    assert!(!first.is_empty());
    let first_content = first.last().unwrap().content.clone();

    // Re-register a different custom row; cache should still win because
    // only 16.67 ms has elapsed of the 100 ms interval.
    hud.add_row(|_| {
        vec![HudRow {
            label: "tick",
            value: "two".into(),
        }]
    });
    let second = compose_hud_text_sections(&mut hud, &world, (1280, 720), 16.67);
    assert_eq!(second[0].content, first_content);

    // Advance past the interval with a large frame_ms; rebuild kicks in.
    let third = compose_hud_text_sections(&mut hud, &world, (1280, 720), 200.0);
    assert_ne!(third[0].content, first_content);
}

#[test]
fn compose_is_empty_when_disabled() {
    let mut hud = DebugHud::new();
    hud.enabled = false;
    let world = World::new();
    let sections = compose_hud_text_sections(&mut hud, &world, (1280, 720), 16.67);
    assert!(sections.is_empty());
}

#[test]
fn ewma_converges() {
    let mut hud = DebugHud::new();
    hud.enabled = true;
    let world = World::new();
    for _ in 0..200 {
        let _ = compose_hud_text_sections(&mut hud, &world, (1280, 720), 16.67);
    }
    assert!(
        (hud.frame_ms_ewma - 16.67).abs() < 0.05,
        "frame_ms_ewma did not converge: {}",
        hud.frame_ms_ewma
    );
}

#[test]
fn outline_emits_extra_sections_behind_main() {
    let mut hud = DebugHud::new();
    hud.enabled = true;
    hud.outline_px = 1.0;
    let world = World::new();
    let sections = compose_hud_text_sections(&mut hud, &world, (1280, 720), 16.67);
    assert_eq!(sections.len(), 5);
    // All outline sections share the main color's content but wear the outline color.
    for s in &sections[..4] {
        assert_eq!(s.color, hud.outline_color);
    }
    let main = sections.last().unwrap();
    assert_eq!(main.color, hud.color);
    // Outline copies sit at +/- outline_px around the main position.
    let [mx, my] = main.position;
    let offsets: Vec<[f32; 2]> = sections[..4].iter().map(|s| s.position).collect();
    assert!(offsets.contains(&[mx - 1.0, my]));
    assert!(offsets.contains(&[mx + 1.0, my]));
    assert!(offsets.contains(&[mx, my - 1.0]));
    assert!(offsets.contains(&[mx, my + 1.0]));
}

#[test]
fn outline_disabled_emits_single_section() {
    let mut hud = DebugHud::new();
    hud.enabled = true;
    hud.outline_px = 0.0;
    let world = World::new();
    let sections = compose_hud_text_sections(&mut hud, &world, (1280, 720), 16.67);
    assert_eq!(sections.len(), 1);
}
