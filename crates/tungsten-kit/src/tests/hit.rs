use super::*;
use serde_json::json;
use tungsten_core::{ComponentRegistry, Stage};

use crate::foundation::tags::TagsPlugin;
use crate::{DespawnOutsidePlugin, StayInViewPlugin, WrapPlugin};

/// A World with hit shapes and the tags `team.enemy` and `damage.fire`.
fn rig() -> (World, Schedule) {
    let mut world = World::new();
    let mut schedule = Schedule::new();
    TagsPlugin::new()
        .with_tags(["team.enemy", "damage.fire"])
        .build(&mut schedule, &mut world);
    HitPlugin.build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    (world, schedule)
}

fn check(world: &World, name: &str, value: &serde_json::Value) -> Result<(), String> {
    world
        .resource::<ComponentRegistry>()
        .check_in(name, value, world)
        .map_err(|e| e.to_string())
}

#[test]
fn a_hit_shape_reads_one_form_within_its_bounds() {
    assert_eq!(
        serde_json::from_value::<HitShape>(json!({ "aabb": [6, 3] })).unwrap(),
        HitShape::aabb(Vec2::new(6.0, 3.0))
    );
    assert_eq!(
        serde_json::to_value(HitShape::circle(MAX_EXTENT)).unwrap(),
        json!({ "circle": 16384.0 })
    );
    for bad in [
        json!({}),
        json!({ "circle": 0 }),
        json!({ "circle": 16385 }),
        json!({ "aabb": [6, -1] }),
        json!({ "circle": 2, "aabb": [1, 1] }),
        json!({ "kind": "circle", "radius": 2 }),
    ] {
        assert!(
            serde_json::from_value::<HitShape>(bad.clone()).is_err(),
            "{bad}"
        );
    }
    assert_eq!(HitShape::circle(8.0).summary(), "circle 8");
    assert_eq!(HitShape::aabb(Vec2::new(6.0, 3.0)).summary(), "box 6 x 3");
}

#[test]
fn a_hitbox_saves_its_fields_but_not_its_targets() {
    let mut world = World::new();
    let target = world.spawn();
    let mut sword = Hitbox::new(HitShape::circle(4.0), 2.0)
        .with_tags(Tags::new(["damage.fire"]))
        .with_knockback(100.0);
    sword.hit.push(target);
    assert!(sword.has_hit(target));
    let saved = serde_json::to_value(&sword).unwrap();
    assert_eq!(
        saved,
        json!({
            "shape": { "circle": 4.0 }, "offset": [0.0, 0.0], "damage": 2.0,
            "tags": ["damage.fire"], "filter": {}, "knockback": 100.0
        })
    );
    let loaded: Hitbox = serde_json::from_value(saved).unwrap();
    assert!(!loaded.has_hit(target), "a fresh activation");
    sword.rearm();
    assert_eq!(loaded, sword);
    for bad in [
        json!({ "shape": { "circle": 4 }, "damage": -1 }),
        json!({ "shape": { "circle": 4 }, "damage": 1, "knockback": -5 }),
        json!({ "shape": { "circle": 4 }, "damage": 1, "offset": [20000, 0] }),
        json!({ "shape": { "circle": 4 }, "damage": 1, "spin": true }),
    ] {
        assert!(
            serde_json::from_value::<Hitbox>(bad.clone()).is_err(),
            "{bad}"
        );
    }
}

#[test]
fn the_damage_stat_writes_only_a_valid_damage() {
    let mut hitbox = Hitbox::new(HitShape::circle(4.0), 2.0);
    hitbox.set_damage(5.0);
    assert_eq!(hitbox.damage(), 5.0);
    hitbox.set_damage(-1.0);
    hitbox.set_damage(f32::NAN);
    assert_eq!(hitbox.damage(), 5.0);
}

#[test]
fn the_scene_forms_check_their_fields_and_tags() {
    let (world, _) = rig();
    assert!(
        check(
            &world,
            HITBOX,
            &json!({ "shape": { "aabb": [6, 3] }, "offset": [8, 0], "damage": 2, "tags": ["damage.fire"], "filter": { "all": ["team.enemy"] }, "knockback": 300 })
        )
        .is_ok()
    );
    assert_eq!(
        check(
            &world,
            HITBOX,
            &json!({ "shape": { "circle": 4 }, "damage": 1, "tags": ["damage.acid"] })
        )
        .unwrap_err(),
        "component `hitbox`, field `tags.0`: unknown tag `damage.acid`"
    );
    assert_eq!(
        check(
            &world,
            HITBOX,
            &json!({ "shape": { "circle": 4 }, "damage": 1, "filter": { "all": ["team.foe"] } })
        )
        .unwrap_err(),
        "component `hitbox`, field `filter.all.0`: unknown tag `team.foe`"
    );
    for bad in [
        json!({ "damage": 1 }),
        json!({ "shape": { "circle": 4 } }),
        json!({ "shape": { "circle": 4 }, "damage": 1, "tags": ["Fire"] }),
        json!({ "shape": { "circle": -4 }, "damage": 1 }),
    ] {
        assert!(check(&world, HITBOX, &bad).is_err(), "{bad}");
    }
    assert!(
        check(
            &world,
            HURTBOX,
            &json!({ "shape": { "circle": 8 }, "offset": [0, -4] })
        )
        .is_ok()
    );
    assert!(
        check(
            &world,
            HURTBOX,
            &json!({ "shape": { "circle": 8 }, "radius": 3 })
        )
        .is_err()
    );
}

#[test]
fn the_plugin_registers_the_queue_the_grid_and_the_tick_before_the_corrections() {
    let (world, schedule) = rig();
    assert!(world.has_event::<Struck>());
    assert_eq!(schedule.stage_of(HIT_GRID), Some(Stage::FixedUpdate));
    assert_eq!(
        schedule.len(),
        9,
        "the seven anchors, the grid and the tick"
    );
    let mut world = World::new();
    let mut schedule = Schedule::new();
    DespawnOutsidePlugin.build(&mut schedule, &mut world);
    WrapPlugin.build(&mut schedule, &mut world);
    StayInViewPlugin.build(&mut schedule, &mut world);
    HitPlugin.build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    let order: Vec<&str> = schedule.names(Stage::FixedUpdate).collect();
    let at = |name: &str| order.iter().position(|n| *n == name).unwrap();
    for later in ["despawn_outside", "wrap", "stay_in_view"] {
        assert!(
            at(HIT_GRID) < at(HITBOX_TICK) && at(HITBOX_TICK) < at(later),
            "{order:?}"
        );
    }
}

#[test]
fn a_hurtbox_and_a_struck_read_back_what_they_were_built_with() {
    let mut world = World::new();
    let (hitter, target) = (world.spawn(), world.spawn());
    let body = Hurtbox::new(HitShape::circle(8.0)).with_offset(Vec2::new(0.0, -4.0));
    assert_eq!(
        (body.shape(), body.offset()),
        (HitShape::circle(8.0), Vec2::new(0.0, -4.0))
    );
    let struck = Struck::new(hitter, target, Vec2::ONE, 2.0);
    assert_eq!(
        (struck.entity, struck.target, struck.position, struck.amount),
        (hitter, target, Vec2::ONE, 2.0)
    );
}

#[test]
#[should_panic(expected = "radius must be finite and in (0, 16384]")]
fn a_zero_circle_panics() {
    let _ = HitShape::circle(0.0);
}

#[test]
#[should_panic(expected = "each half extent must be finite and in (0, 16384]")]
fn a_huge_box_panics() {
    let _ = HitShape::aabb(Vec2::new(1.0, 20_000.0));
}

#[test]
#[should_panic(expected = "offset must be finite and within ±16384")]
fn a_far_offset_panics() {
    let _ = Hurtbox::new(HitShape::circle(1.0)).with_offset(Vec2::new(16_385.0, 0.0));
}

#[test]
#[should_panic(expected = "damage must be finite and >= 0")]
fn a_negative_hitbox_damage_panics() {
    let _ = Hitbox::new(HitShape::circle(1.0), -1.0);
}

#[test]
#[should_panic(expected = "speed must be finite and >= 0")]
fn a_nan_hitbox_knockback_panics() {
    let _ = Hitbox::new(HitShape::circle(1.0), 1.0).with_knockback(f32::NAN);
}
