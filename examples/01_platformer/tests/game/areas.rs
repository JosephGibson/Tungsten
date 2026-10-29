//! The areas: walking across each one's start names it on the banner and
//! autosaves once, raises its ambience over the others and, past the
//! light's ease, lights it as a cave or under the moon; and a Retry after a
//! death starts in the last.

use std::path::Path;

use example_01_platformer::features::audio::components::Ambience;
use example_01_platformer::features::level::components::Area;
use example_01_platformer::features::level::{AMBIENT, AMBIENT_INDOOR, LIGHT_EASE, TILE};
use example_01_platformer::features::run::Run;
use example_01_platformer::features::run::banner::{self, BANNER_SECS};
use example_01_platformer::game::GAMEPLAY;
use example_01_platformer::shell::AUTOSAVES;
use example_01_platformer::shell::game_over::GAME_OVER;
use glam::Vec2;
use tungsten::core::{AmbientLight, SoundRegistry, UserFolder};
use tungsten::kit::damage;
use tungsten::testing::UiHarness;

use super::player_half;
use super::{
    calm, centre, fall_dead, harness_with, player_entity, press, settle, start, teleport, top,
    user_folder,
};

/// Each area after the spawn's, its first column, and where the walk into
/// it starts: a column on the platform before its start, that platform's
/// row, and whether the walk jumps. The five transitions' borders.
const CROSSINGS: [(&str, f32, f32, f32, bool); 5] = [
    ("Broken aqueduct", 93.0, 90.5, 36.0, false),
    ("Waterfall ravine", 162.0, 160.0, 40.0, false),
    ("Ember hollow", 211.0, 208.0, 63.0, false),
    ("Iron mine", 281.0, 278.0, 57.0, false),
    ("Troll bridge", 351.0, 348.4, 41.0, true),
];
/// The spawn's area, which a run's first step enters.
const FIRST: &str = "Lantern clearing";

/// Each autosave's bytes, `None` without a file.
fn autosaves(dir: &Path) -> [Option<Vec<u8>>; 3] {
    AUTOSAVES.map(|slot| std::fs::read(dir.join("saves").join(format!("{slot}.json"))).ok())
}

fn area(harness: &UiHarness) -> Option<String> {
    harness.world().resource::<Run>().area.clone()
}

/// Steps one frame and adds the name the banner began to show to `seen`.
fn step(harness: &mut UiHarness, seen: &mut Vec<String>) {
    let before = banner::shown(harness.world()).map(str::to_owned);
    harness.step(1);
    let after = banner::shown(harness.world()).map(str::to_owned);
    if after.is_some() && after != before {
        seen.extend(after);
    }
}

#[test]
fn a_new_area_replaces_the_banner() {
    let mut harness = harness_with(UserFolder::Off);
    start(&mut harness);
    calm(&mut harness);
    let player = player_entity(&harness);
    harness.step(10);
    assert_eq!(banner::shown(harness.world()), Some(FIRST));
    // Into the aqueduct while the clearing's name still shows.
    let (name, column, _, row, _) = CROSSINGS[0];
    let inside = Vec2::new((column + 1.0) * TILE, row * TILE - player_half().y - 0.5);
    teleport(&mut harness, player, inside);
    let mut frames = 0;
    while banner::shown(harness.world()) != Some(name) {
        harness.step(1);
        frames += 1;
        assert!(frames < 30, "the aqueduct's name replaces the clearing's");
    }
    // Its own fade, from its start.
    harness.step(((BANNER_SECS - 0.2) * 60.0) as u32);
    assert_eq!(banner::shown(harness.world()), Some(name));
    harness.step(30);
    assert_eq!(banner::shown(harness.world()), None);
}

#[test]
fn walking_across_each_area_start_autosaves_once_names_it_and_retry_starts_there() {
    let dir = user_folder("areas");
    let mut harness = harness_with(UserFolder::At(dir.clone()));
    start(&mut harness);
    calm(&mut harness);
    let player = player_entity(&harness);
    // The run's first steps, as the curtain lifts, enter the spawn's area.
    harness.step(10);
    let shown = banner::shown(harness.world());
    assert_eq!(shown, Some(FIRST), "the spawn's area names itself");
    assert_eq!(area(&harness).as_deref(), Some(FIRST));
    let mut seen = Vec::new();
    for (name, column, x, row, jump) in CROSSINGS {
        teleport(
            &mut harness,
            player,
            Vec2::new(x * TILE, row * TILE - player_half().y - 0.5),
        );
        for _ in 0..10 {
            harness.step(1);
        }
        seen.clear();
        let (mut bytes, mut writes) = (autosaves(&dir), 0);
        harness.harness_mut().press_action("move_right");
        if jump {
            harness.harness_mut().press_action("jump");
        }
        // In, then half a second for the autosave to land.
        let mut after = None;
        for frame in 0..120 {
            step(&mut harness, &mut seen);
            let now = autosaves(&dir);
            writes += now.iter().zip(&bytes).filter(|(a, b)| a != b).count();
            bytes = now;
            if after.is_none() && area(&harness).as_deref() == Some(name) {
                after = Some(frame);
                for action in ["move_right", "jump"] {
                    harness.harness_mut().release_action(action);
                }
            }
            if after.is_some_and(|entered| frame >= entered + 30) {
                break;
            }
        }
        assert_eq!(area(&harness).as_deref(), Some(name));
        assert!(centre(&harness, player).x + player_half().x > column * TILE);
        assert_eq!(seen, [name], "one name crossing into {name}");
        assert_eq!(writes, 1, "one autosave crossing into {name}");
        // Past the light's ease: the area's ambience up and every other
        // loop down, and its light, a cave's or the moon's.
        harness.step((LIGHT_EASE * 60.0) as u32 + 1);
        let world = harness.world();
        let entered = world
            .query::<&Area>()
            .find(|area| area.name == name)
            .expect("the area")
            .clone();
        let registry = world.resource::<SoundRegistry>();
        for (id, _, level) in &world.resource::<Ambience>().loops {
            let want = if entered.ambience.as_deref() == Some(id.as_str()) {
                registry.get_volume(registry.get_by_id(id).expect("the loop"))
            } else {
                0.0
            };
            assert!((level - want).abs() < 1e-4, "{name}: {id} at {level}");
        }
        let lit = if entered.indoor {
            AMBIENT_INDOOR
        } else {
            AMBIENT
        };
        let ambient = world.resource::<AmbientLight>().0;
        assert!(
            (ambient - lit).abs().max_element() < 1e-4,
            "{name}: ambient {ambient}"
        );
    }
    // A death on the bridge: Retry loads the autosave taken on its start.
    let saved = centre(&harness, player);
    damage(harness.world_mut(), player, 1000.0);
    fall_dead(&mut harness);
    assert_eq!(top(&harness), Some(GAME_OVER));
    press(&mut harness, "Retry");
    settle(&mut harness);
    assert_eq!(top(&harness), Some(GAMEPLAY));
    let player = player_entity(&harness);
    harness.step(5);
    let last = CROSSINGS[CROSSINGS.len() - 1];
    let at = centre(&harness, player);
    // Its body overlaps the bridge's start, where the autosave stood; which
    // area a body astride two names is the save's to say (step 7).
    assert!(
        at.x + player_half().x > last.1 * TILE,
        "{at} is in {}",
        last.0
    );
    assert!(
        at.x <= saved.x,
        "it starts where the bridge's autosave stood"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
