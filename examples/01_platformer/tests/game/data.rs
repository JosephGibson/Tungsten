//! The data's restated copies: every Rust constant that repeats a prefab's
//! or an ability's value matches the file the game loads, so a change to
//! the data cannot leave its copy behind (I16), and a body moved in its
//! file moves each spawn of it; and every sound event the game sends has a
//! cue rule that plays a registered sound.

use example_01_platformer::features::audio::components::Sound;
use example_01_platformer::features::combat::{
    CHILL_BUILD, CHILL_NEEDS, FREEZE_BUILDS, FREEZE_NEEDS,
};
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
    BOMB_CENTRE, BOMB_RADIUS, BURN_SECS, CRATE_CENTRE, CRATE_HALF, KEG_CENTRE, KEG_HALF,
    PLANK_BURN_SECS, PLANK_CENTRE, PLANK_HALF, ROCK_CENTRE, ROCK_RADIUS,
};
use example_01_platformer::features::spells::hud::ROUNDS;
use example_01_platformer::features::toys::{
    BALL_RADIUS, HOLE_SECS, IRON_HALF, IRON_MASS, MISSILE_SPEED, SCRAP_HALF, SCRAP_MASS,
    SMALL_BALL_RADIUS,
};
use example_01_platformer::shared::{Body, body_centre};
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
    // The toys' sizes and the iron's and its scraps' masses.
    // The walkers' controllers: their step, which the ledge check reads, and
    // their shove by their kind's mass up to the heaviest a walker moves,
    // which the troll's kick reads; lava's layer left out (S-99).
    for kind in EnemyKind::ALL.into_iter().filter(|kind| !kind.info().flyer) {
        let info = kind.info();
        for variant in [Variant::Plain, Variant::Armoured] {
            let prefab = enemies::prefab(kind, variant);
            let value = json(&format!("prefabs/{prefab}.json"));
            let controller = "/components/platformer_controller";
            assert_eq!(
                number(&value, &format!("{controller}/step_height")),
                enemies::WALK_STEP,
                "{prefab}"
            );
            assert_eq!(
                number(&value, &format!("{controller}/shove_mass")),
                info.mass
            );
            assert_eq!(
                number(&value, &format!("{controller}/shove_share")),
                enemies::WALK_SHOVE_MOST / info.mass,
                "{prefab}"
            );
            assert_eq!(
                value.pointer(&format!("{controller}/without")),
                Some(&serde_json::json!([4])),
                "{prefab}: lava's layer"
            );
            assert_eq!(1 << 4, example_01_platformer::features::level::LAVA_LAYER);
        }
    }
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
    // The cold's freeze builds, the freezable's and the troll's, whose band
    // the chill's grade climbs to, each at its intensity's max.
    for id in FREEZE_BUILDS {
        let build = json(&format!("data/effects/{id}.json"));
        assert_eq!(number(&build, "/bands/0/at"), FREEZE_NEEDS, "{id}");
        assert_eq!(number(&build, "/intensity/max"), FREEZE_NEEDS, "{id}");
    }
    // Its chill's build, which the frost climbs to before the chill.
    let chill = json(&format!("data/effects/{CHILL_BUILD}.json"));
    assert_eq!(number(&chill, "/bands/0/at"), CHILL_NEEDS);
    assert_eq!(number(&chill, "/intensity/max"), CHILL_NEEDS);
    // The props' bodies and a goblin's rock and bomb.
    assert_eq!(body("crate"), (CRATE_HALF, CRATE_CENTRE));
    assert_eq!(body("keg"), (KEG_HALF, KEG_CENTRE));
    assert_eq!(body("plank"), (PLANK_HALF, PLANK_CENTRE));
    assert_eq!(body("goblin_bomb"), (Vec2::splat(BOMB_RADIUS), BOMB_CENTRE));
    assert_eq!(body("goblin_rock"), (Vec2::splat(ROCK_RADIUS), ROCK_CENTRE));
    // A crate's and a plank's fire, each prefab's `fuel` stat (7.13a).
    let fuel = |prefab: &str| {
        number(
            &json(&format!("prefabs/{prefab}.json")),
            "/components/stats/fuel",
        )
    };
    assert_eq!(fuel("crate"), BURN_SECS);
    assert_eq!(fuel("plank"), PLANK_BURN_SECS);
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
    let goblin = enemies::spawn(world, EnemyKind::Goblin, corner).expect("its prefab");
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
