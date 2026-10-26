//! The tomes (01 (l)) on books a test sets in the level's empty sky: the
//! panel over a book in reach, the take with its burst and sound, and the
//! strip in the order taken.

use example_01_platformer::features::audio::components::{Sound, Sounded};
use example_01_platformer::features::spells::hud::Hud;
use example_01_platformer::features::tomes::{BOOK_CENTRE, INTERACT, Tome, TomeKind, Tomes};
use example_01_platformer::game::GAMEPLAY;
use example_01_platformer::shell::widgets;
use glam::Vec2;
use tungsten::SceneEntity;
use tungsten::core::Entity;
use tungsten::core::ui::Visibility;
use tungsten::testing::UiHarness;

use super::{bursts, centre, ground, run, stand};

/// The test's floor's top, in the level's empty sky.
const TOP: f32 = 900.0;

/// A run with the player standing on a floor in the sky at x 400.
fn sky() -> (UiHarness, Entity) {
    let (mut harness, player) = run();
    ground(&mut harness, TOP);
    stand(&mut harness, player, 400.0, TOP);
    harness.step(2);
    (harness, player)
}

/// A book of `kind` from its prefab with its centre at `at`, owned by
/// gameplay.
fn book(harness: &mut UiHarness, kind: TomeKind, at: Vec2) -> Entity {
    let world = harness.world_mut();
    world
        .prepare_prefab(kind.prefab(), (at - BOOK_CENTRE).to_array())
        .expect("the tomes' prefabs load with the manifest")
        .with((SceneEntity { state_id: GAMEPLAY },))
        .spawn(world)
}

fn held(harness: &UiHarness, player: Entity) -> Vec<TomeKind> {
    harness
        .world()
        .get::<Tomes>(player)
        .map(|tomes| tomes.0.clone())
        .unwrap_or_default()
}

fn hud(harness: &UiHarness) -> Hud {
    *harness.world().resource::<Hud>()
}

/// Presses `interact` for a frame and releases it the next: the pickup
/// sounds of `sound` heard meanwhile.
fn interact(harness: &mut UiHarness, sound: Sound) -> usize {
    let heard = |harness: &UiHarness| {
        harness
            .harness()
            .events::<Sounded>()
            .filter(|sounded| sounded.sound == sound)
            .count()
    };
    harness.harness_mut().press_action(INTERACT);
    harness.step(1);
    let mut sounds = heard(harness);
    harness.harness_mut().release_action(INTERACT);
    harness.step(1);
    sounds += heard(harness);
    sounds
}

#[test]
fn a_books_panel_shows_within_reach_and_hides_beyond() {
    let (mut harness, player) = sky();
    let eye = centre(&harness, player);
    book(&mut harness, TomeKind::Pact, eye + Vec2::new(60.0, 0.0));
    harness.step(2);
    let panel = hud(&harness).panel;
    let tree = harness.tree();
    assert_eq!(
        tree.visibility(panel.root),
        Some(Visibility::Visible),
        "at 60 px"
    );
    assert_eq!(widgets::text_of(tree, panel.name), "The Demonic Pact");
    assert_eq!(
        widgets::text_of(tree, panel.gives),
        "A shield of 50 over your health, back 3 s after a hit"
    );
    assert_eq!(
        widgets::text_of(tree, panel.costs),
        "Half your maximum health, for good"
    );
    // 70 px off, it hides; back within reach, it shows again.
    stand(&mut harness, player, 390.0, TOP);
    harness.step(2);
    let root = hud(&harness).panel.root;
    assert_eq!(
        harness.tree().visibility(root),
        Some(Visibility::Hidden),
        "at 70 px"
    );
    stand(&mut harness, player, 410.0, TOP);
    harness.step(2);
    assert_eq!(
        harness.tree().visibility(root),
        Some(Visibility::Visible),
        "at 50 px"
    );
}

#[test]
fn an_interact_press_takes_the_nearest_book_once_with_its_burst_and_sound() {
    let (mut harness, player) = sky();
    let eye = centre(&harness, player);
    let near = book(&mut harness, TomeKind::Pact, eye + Vec2::new(40.0, 0.0));
    let far = book(&mut harness, TomeKind::Arcane, eye - Vec2::new(55.0, 0.0));
    harness.step(1);
    assert_eq!(
        interact(&mut harness, Sound::TomePactTaken),
        1,
        "its sound, once"
    );
    let world = harness.world();
    assert!(!world.is_alive(near), "the nearest book is taken");
    assert!(world.get::<Tome>(far).is_some(), "the other stays");
    assert_eq!(held(&harness, player), [TomeKind::Pact]);
    assert_eq!(bursts(&harness, "ex10_tome_pact_take_sparks"), 1);
    assert_eq!(bursts(&harness, "ex10_tome_arcane_take_sparks"), 0);
    // Out of reach, a press takes nothing; within it, the other.
    harness.step(3);
    assert_eq!(held(&harness, player), [TomeKind::Pact], "once a press");
    stand(&mut harness, player, 250.0, TOP);
    harness.step(2);
    assert_eq!(interact(&mut harness, Sound::TomeArcaneTaken), 0);
    stand(&mut harness, player, 400.0, TOP);
    harness.step(2);
    assert_eq!(interact(&mut harness, Sound::TomeArcaneTaken), 1);
    assert_eq!(held(&harness, player), [TomeKind::Pact, TomeKind::Arcane]);
}

#[test]
fn the_strip_follows_the_order_taken() {
    let (mut harness, player) = sky();
    let strip = hud(&harness).strip;
    assert_eq!(
        harness.tree().visibility(strip.row),
        Some(Visibility::Collapsed),
        "none held"
    );
    let eye = centre(&harness, player);
    for (kind, sound) in [
        (TomeKind::Arcane, Sound::TomeArcaneTaken),
        (TomeKind::Bounding, Sound::TomeBoundingTaken),
    ] {
        book(&mut harness, kind, eye + Vec2::new(30.0, 0.0));
        harness.step(1);
        assert_eq!(interact(&mut harness, sound), 1, "{kind:?}");
    }
    let art = hud(&harness).art;
    let tree = harness.tree();
    assert_eq!(tree.visibility(strip.row), Some(Visibility::Visible));
    assert_eq!(
        tree.image_sprite(strip.tiles[0]),
        Some(art.tomes[TomeKind::Arcane.index()])
    );
    assert_eq!(
        tree.image_sprite(strip.tiles[1]),
        Some(art.tomes[TomeKind::Bounding.index()])
    );
    assert_eq!(tree.visibility(strip.tiles[1]), Some(Visibility::Visible));
    assert_eq!(tree.visibility(strip.tiles[2]), Some(Visibility::Collapsed));
}
