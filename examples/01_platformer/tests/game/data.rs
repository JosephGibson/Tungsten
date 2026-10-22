//! The data's restated copies: every Rust constant that repeats a prefab's
//! or an ability's value matches the file the game loads, so a change to
//! the data cannot leave its copy behind (I16).

use example_01_platformer::features::enemies::bat::DIVE_SECS;
use example_01_platformer::features::enemies::beetle::CHARGE_SECS;
use example_01_platformer::features::enemies::components::EnemyKind;
use example_01_platformer::features::enemies::drops::ORB_SECS;
use example_01_platformer::features::gun::{
    BULLET_CENTRE, BULLET_RADIUS, MAX_RICOCHETS, MUZZLE_OFFSET,
};
use example_01_platformer::features::toys::{
    BALL_RADIUS, IRON_HALF, IRON_MASS, MISSILE_SPEED, SCRAP_HALF, SMALL_BALL_RADIUS,
};
use example_01_platformer::features::{dummy, player};
use glam::Vec2;
use serde_json::Value;

/// The file under `assets/` at `path`, as JSON.
fn json(path: &str) -> Value {
    let text = std::fs::read_to_string(format!("assets/{path}"))
        .unwrap_or_else(|error| panic!("{path}: {error}"));
    serde_json::from_str(&text).unwrap_or_else(|error| panic!("{path}: {error}"))
}

/// The number at `pointer` in `value`.
fn number(value: &Value, pointer: &str) -> f32 {
    value
        .pointer(pointer)
        .and_then(Value::as_f64)
        .unwrap_or_else(|| panic!("no number at {pointer}")) as f32
}

/// The pair at `pointer` in `value`.
fn pair(value: &Value, pointer: &str) -> Vec2 {
    Vec2::new(
        number(value, &format!("{pointer}/0")),
        number(value, &format!("{pointer}/1")),
    )
}

/// A prefab's body: its half size, or its radius as a square's, and its
/// offset.
fn body(prefab: &str) -> (Vec2, Vec2) {
    let value = json(&format!("prefabs/{prefab}.json"));
    let shape = "/components/rigid_body/shape";
    let half = match value.pointer(&format!("{shape}/radius")) {
        Some(_) => Vec2::splat(number(&value, &format!("{shape}/radius"))),
        None => pair(&value, &format!("{shape}/half_extents")),
    };
    (half, pair(&value, "/components/rigid_body/offset"))
}

#[test]
fn every_restated_constant_matches_its_data() {
    // The bodies: the player's, the dummy's and each enemy's, their half
    // sizes and centres from the sprite's top-left.
    assert_eq!(body("player"), (player::PLAYER_HALF, player::BODY_OFFSET));
    assert_eq!(body("dummy"), (dummy::DUMMY_HALF, dummy::DUMMY_CENTRE));
    for kind in EnemyKind::ALL {
        let info = kind.info();
        assert_eq!(body(info.prefab), (info.half, info.centre), "{kind:?}");
    }
    // The toys' sizes and the iron's mass.
    assert_eq!(body("iron_block").0, IRON_HALF);
    let iron = json("prefabs/iron_block.json");
    assert_eq!(number(&iron, "/components/rigid_body/mass"), IRON_MASS);
    assert_eq!(body("iron_scrap").0, SCRAP_HALF);
    assert_eq!(body("bowling_ball").0, Vec2::splat(BALL_RADIUS));
    assert_eq!(body("small_ball").0, Vec2::splat(SMALL_BALL_RADIUS));
    // The missile's speed, the orbs' lifetime, the beetle's charge and the
    // bat's dive.
    let missile = json("data/abilities/fire_missile.json");
    assert_eq!(number(&missile, "/payload/1/spawn/speed"), MISSILE_SPEED);
    for orb in ["health_orb", "mana_orb"] {
        let value = json(&format!("prefabs/{orb}.json"));
        assert_eq!(
            number(&value, "/components/lifetime/seconds"),
            ORB_SECS,
            "{orb}"
        );
    }
    let charge = json("data/abilities/beetle_charge.json");
    assert_eq!(number(&charge, "/payload/0/hitbox"), CHARGE_SECS);
    let dive = json("data/abilities/bat_dive.json");
    assert_eq!(number(&dive, "/payload/0/hitbox"), DIVE_SECS);
    // The gun's bullet and muzzle.
    let bullet = json("prefabs/bullet.json");
    assert_eq!(pair(&bullet, "/components/hitbox/offset"), BULLET_CENTRE);
    assert_eq!(
        number(&bullet, "/components/hitbox/shape/circle"),
        BULLET_RADIUS
    );
    assert_eq!(
        number(&bullet, "/components/projectile/bounces"),
        MAX_RICOCHETS as f32
    );
    let gun = json("data/abilities/gun.json");
    assert_eq!(number(&gun, "/payload/0/spawn/offset"), MUZZLE_OFFSET);
}
