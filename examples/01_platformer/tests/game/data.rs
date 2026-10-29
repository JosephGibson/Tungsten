//! The data's restated copies: every Rust constant that repeats a prefab's
//! or an ability's value matches the file the game loads, so a change to
//! the data cannot leave its copy behind (I16), and a body moved in its
//! file moves each spawn of it; and every sound event the game sends has a
//! cue rule that plays a registered sound.

use example_01_platformer::features::audio::components::Sound;
use example_01_platformer::features::combat::{CHILL_NEEDS, FREEZE_NEEDS};
use example_01_platformer::features::enemies::bat::DIVE_SECS;
use example_01_platformer::features::enemies::beetle::CHARGE_SECS;
use example_01_platformer::features::enemies::components::{EnemyKind, Variant};
use example_01_platformer::features::enemies::drops::ORB_SECS;
use example_01_platformer::features::enemies::{self, crawler, goblin, snowball, troll};
use example_01_platformer::features::gun::{
    BULLET_CENTRE, BULLET_RADIUS, MAX_RICOCHETS, MUZZLE_OFFSET,
};
use example_01_platformer::features::player;
use example_01_platformer::features::props::{
    BOMB_CENTRE, BOMB_RADIUS, CRATE_CENTRE, CRATE_HALF, KEG_CENTRE, KEG_HALF, PLANK_CENTRE,
    PLANK_HALF, ROCK_CENTRE, ROCK_RADIUS,
};
use example_01_platformer::features::spells::hud::ROUNDS;
use example_01_platformer::features::toys::{
    BALL_RADIUS, HOLE_SECS, IRON_HALF, IRON_MASS, MISSILE_SPEED, SCRAP_HALF, SCRAP_MASS,
    SMALL_BALL_RADIUS,
};
use example_01_platformer::shared::{Body, SaveIds, body_centre};
use glam::Vec2;
use serde_json::Value;
use tungsten::core::{Collider, DataRegistry, Shape};
use tungsten::kit::Hurtbox;

use super::actor_body;

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
    // The bodies: each actor's prefabs' rigid body and hurtbox are its body
    // file's, which a spawn takes over them (`shared::fit_body`); the
    // armoured troll's (e01flask) are the troll's.
    let armoured = enemies::prefab(EnemyKind::Troll, Variant::Armoured);
    let actors = EnemyKind::ALL.map(|kind| (kind.info().prefab, kind.info().prefab));
    for (prefab, actor) in [
        ("player", "player"),
        ("dummy", "dummy"),
        (armoured, "troll"),
    ]
    .into_iter()
    .chain(actors)
    {
        let file = actor_body(actor);
        assert_eq!(body(prefab), (file.half, file.centre), "{prefab}");
        let hurtbox = json(&format!("prefabs/{prefab}.json"))["components"]["hurtbox"].clone();
        let hurtbox: Hurtbox = serde_json::from_value(hurtbox).expect("a hurtbox");
        assert_eq!(hurtbox, file.hurtbox, "{prefab}");
    }
    for kind in EnemyKind::ALL {
        let info = kind.info();
        // Its knockback's resist, which the pushes read (the kit's has no
        // getter): 0 unless the prefab sets one.
        let prefab = json(&format!("prefabs/{}.json", info.prefab));
        let resist = prefab
            .pointer("/components/knockback/resist")
            .and_then(Value::as_f64)
            .unwrap_or(0.0) as f32;
        assert_eq!(resist, info.resist, "{kind:?}");
    }
    // The armoured troll's knockback's resist, its kind's table entry.
    let info = EnemyKind::Troll.info();
    let prefab = json(&format!("prefabs/{armoured}.json"));
    let resist = prefab
        .pointer("/components/knockback/resist")
        .and_then(Value::as_f64)
        .unwrap_or(0.0) as f32;
    assert_eq!(resist, info.resist, "the armoured troll's knockback");
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
    // And its run, its `move_speed` stat and its run speed, likewise.
    for pointer in [
        "/components/stats/move_speed",
        "/components/platformer_controller/run_speed",
    ] {
        assert_eq!(number(&frost, pointer), crawler::MOVE_SPEED, "{pointer}");
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
    // Its chill rung, which the frost climbs to before the chill.
    let chills: Vec<&Value> = rungs
        .iter()
        .filter(|rung| rung["effect"] == "chilled")
        .collect();
    assert_eq!(chills.len(), 1, "one chill rung");
    assert_eq!(number(chills[0], "/needs"), CHILL_NEEDS);
    // The props' bodies and a goblin's rock and bomb.
    assert_eq!(body("crate"), (CRATE_HALF, CRATE_CENTRE));
    assert_eq!(body("keg"), (KEG_HALF, KEG_CENTRE));
    assert_eq!(body("plank"), (PLANK_HALF, PLANK_CENTRE));
    assert_eq!(body("goblin_bomb"), (Vec2::splat(BOMB_RADIUS), BOMB_CENTRE));
    assert_eq!(body("goblin_rock"), (Vec2::splat(ROCK_RADIUS), ROCK_CENTRE));
    // A crawler's snowball's.
    assert_eq!(
        body(snowball::SNOWBALL_PREFAB),
        (
            Vec2::splat(snowball::SNOWBALL_RADIUS),
            snowball::SNOWBALL_CENTRE
        )
    );
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

#[test]
fn a_body_moved_in_its_data_file_moves_the_spawned_body() {
    // The goblin's file as a reload reads it, its body and hurtbox raised
    // and widened, its hand moved: no Rust names any of it.
    let path = "assets/data/bodies/goblin.json";
    let text = std::fs::read_to_string(path).expect("the goblin's body");
    let mut file: Value = serde_json::from_str(&text).expect("its JSON");
    file["centre"] = serde_json::json!([30, 37]);
    file["half"] = serde_json::json!([12, 24]);
    file["hurtbox"] = serde_json::json!({ "offset": [30, 36], "shape": { "aabb": [12, 23] } });
    file["points"]["hand"] = serde_json::json!({ "left": [6, 26], "right": [54, 26] });
    let moved: Body = serde_json::from_value(file).expect("a body");
    let (mut harness, _) = super::run();
    let world = harness.world_mut();
    world
        .resource_mut::<DataRegistry>()
        .insert("goblin", moved.clone());
    let corner = Vec2::new(600.0, 500.0);
    let id = world.resource_mut::<SaveIds>().take();
    let goblin = enemies::spawn(world, EnemyKind::Goblin, corner, id).expect("its prefab");
    let collider = world.get::<Collider>(goblin).expect("a collider");
    assert_eq!(collider.offset, Vec2::new(30.0, 37.0));
    assert_eq!(
        collider.shape,
        Shape::Aabb {
            half_extents: Vec2::new(12.0, 24.0)
        }
    );
    assert_eq!(world.get::<Hurtbox>(goblin), Some(&moved.hurtbox));
    assert_eq!(moved.hurtbox.offset(), Vec2::new(30.0, 36.0));
    assert_eq!(
        enemies::centre(world, goblin),
        Some(corner + Vec2::new(30.0, 37.0))
    );
    assert_eq!(
        goblin::hand(world, goblin),
        Some(corner + Vec2::new(54.0, 26.0))
    );
    // A summon places the moved body's centre where it is cast.
    let at = Vec2::new(900.0, 500.0);
    let summoned = enemies::summon(world, EnemyKind::Goblin, at).expect("its prefab");
    assert_eq!(body_centre(world, summoned), Some(at));
}
