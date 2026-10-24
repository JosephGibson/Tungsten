//! The data's restated copies: every Rust constant that repeats a prefab's
//! or an ability's value matches the file the game loads, so a change to
//! the data cannot leave its copy behind (I16); and every sound event the
//! game sends has a cue rule that plays a registered sound.

use example_01_platformer::features::audio::components::Sound;
use example_01_platformer::features::combat::FREEZE_NEEDS;
use example_01_platformer::features::enemies::bat::DIVE_SECS;
use example_01_platformer::features::enemies::beetle::CHARGE_SECS;
use example_01_platformer::features::enemies::components::EnemyKind;
use example_01_platformer::features::enemies::drops::ORB_SECS;
use example_01_platformer::features::enemies::{crawler, troll};
use example_01_platformer::features::gun::{
    BULLET_CENTRE, BULLET_RADIUS, MAX_RICOCHETS, MUZZLE_OFFSET,
};
use example_01_platformer::features::props::{
    BOMB_CENTRE, BOMB_RADIUS, CRATE_CENTRE, CRATE_HALF, KEG_CENTRE, KEG_HALF, ROCK_CENTRE,
    ROCK_RADIUS,
};
use example_01_platformer::features::spells::hud::ROUNDS;
use example_01_platformer::features::toys::{
    BALL_RADIUS, HOLE_SECS, IRON_HALF, IRON_MASS, MISSILE_SPEED, SCRAP_HALF, SCRAP_MASS,
    SMALL_BALL_RADIUS,
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
        // Its knockback's resist, which the pushes read (the kit's has no
        // getter): 0 unless the prefab sets one.
        let prefab = json(&format!("prefabs/{}.json", info.prefab));
        let resist = prefab
            .pointer("/components/knockback/resist")
            .and_then(Value::as_f64)
            .unwrap_or(0.0) as f32;
        assert_eq!(resist, info.resist, "{kind:?}");
    }
    // The toys' sizes and the iron's and its scraps' masses.
    assert_eq!(body("iron_block").0, IRON_HALF);
    let iron = json("prefabs/iron_block.json");
    assert_eq!(number(&iron, "/components/rigid_body/mass"), IRON_MASS);
    let scrap = json("prefabs/iron_scrap.json");
    assert_eq!(number(&scrap, "/components/rigid_body/mass"), SCRAP_MASS);
    // The courier's jump, which its shove's retry restates, and its health,
    // the same at every difficulty, which a save's migration restates.
    let courier = json("prefabs/player.json");
    for pointer in ["/components/stats/max_health", "/components/health/max"] {
        assert_eq!(number(&courier, pointer), player::MAX_HEALTH, "{pointer}");
    }
    // Its reach, which a save's migration restates.
    assert_eq!(
        number(&courier, "/components/stats/pickup_radius"),
        player::PICKUP_RADIUS
    );
    assert_eq!(
        number(&courier, "/components/platformer_controller/jump_speed"),
        player::JUMP_SPEED
    );
    // The troll's walk, its `move_speed` stat and its run speed, which a
    // save's migration restates.
    let walker = json("prefabs/troll.json");
    for pointer in [
        "/components/stats/move_speed",
        "/components/platformer_controller/run_speed",
    ] {
        assert_eq!(number(&walker, pointer), troll::MOVE_SPEED, "{pointer}");
    }
    // The crawler's touch: its `damage` stat and its cold, which a save's
    // migration restates.
    let frost = json("prefabs/crawler.json");
    for pointer in [
        "/components/stats/damage",
        "/components/contact_damage/damage/damage.cold",
    ] {
        assert_eq!(number(&frost, pointer), crawler::DAMAGE, "{pointer}");
    }
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
    // The magazine the HUD shows, the gun's charges.
    assert_eq!(number(&gun, "/charges/max"), ROUNDS as f32);
    // A black hole's life.
    let hole = json("prefabs/black_hole.json");
    assert_eq!(number(&hole, "/components/black_hole/left"), HOLE_SECS);
    // The cold ladder's freeze rungs, the freezable's and the troll's, which
    // the chill's grade climbs to.
    let types = json("data/damage_types/game.json");
    let rungs = types["damage.cold"]["ailments"]
        .as_array()
        .expect("cold's ladder");
    let freezes: Vec<&Value> = rungs
        .iter()
        .filter(|rung| {
            rung["requires"]
                .as_array()
                .is_some_and(|tags| tags.iter().any(|tag| tag == "freezable"))
        })
        .collect();
    assert_eq!(freezes.len(), 2, "two freeze rungs");
    for rung in freezes {
        assert_eq!(number(rung, "/needs"), FREEZE_NEEDS);
    }
    // The props' bodies and a goblin's rock and bomb.
    assert_eq!(body("crate"), (CRATE_HALF, CRATE_CENTRE));
    assert_eq!(body("keg"), (KEG_HALF, KEG_CENTRE));
    assert_eq!(body("goblin_bomb"), (Vec2::splat(BOMB_RADIUS), BOMB_CENTRE));
    assert_eq!(body("goblin_rock"), (Vec2::splat(ROCK_RADIUS), ROCK_CENTRE));
}

#[test]
fn every_sound_event_has_a_rule_and_its_sound_is_in_the_manifest() {
    let rules = json("data/cue_rules/sounds.json");
    let rules = rules.as_array().expect("a list of rules");
    let manifest = json("manifest.json");
    for sound in Sound::ALL {
        assert!(
            rules.iter().any(|rule| rule["on"] == sound.event()),
            "{sound:?}: no rule on `{}`",
            sound.event()
        );
    }
    for rule in rules {
        for action in rule["do"].as_array().expect("its actions") {
            if let Some(id) = action.pointer("/sound/id").and_then(Value::as_str) {
                assert!(
                    manifest["sounds"].get(id).is_some(),
                    "{}: `{id}` is not in the manifest",
                    rule["on"]
                );
            }
        }
    }
}
