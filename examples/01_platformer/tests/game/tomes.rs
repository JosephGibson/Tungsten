//! The tomes (01 (l)) on books a test sets in the level's empty sky: the
//! panel over a book in reach, the take with its burst and sound, and the
//! strip in the order taken; the pact's half health and its shield, which
//! takes a hit first, comes back after one and falls with the kill row;
//! bounding's paid air jumps with their look and sound; and arcane power's
//! dearer spells and their doubled hits, beside the hits it leaves alone,
//! and all three tomes held together.

use example_01_platformer::features::audio::components::{Sound, Sounded};
use example_01_platformer::features::level::KILL_Y;
use example_01_platformer::features::spells::components::Spell;
use example_01_platformer::features::spells::hud::Hud;
use example_01_platformer::features::spells::{self, hud as spell_hud};
use example_01_platformer::features::tomes::{BOOK_CENTRE, INTERACT, Tome, TomeKind, Tomes};
use example_01_platformer::features::toys::{self, IRON_HALF};
use example_01_platformer::features::{gun, lightning};
use example_01_platformer::game::GAMEPLAY;
use example_01_platformer::shell::widgets;
use glam::Vec2;
use tungsten::SceneEntity;
use tungsten::core::ui::Visibility;
use tungsten::core::{DataRegistry, Entity};
use tungsten::kit::{
    Ability, DamageRequest, DamagedPart, Health, KitRng, PlatformerController, Shield, Stats, Tags,
};
use tungsten::testing::UiHarness;

use super::combat::{PLAIN_SEED, TOP as ARENA, with_dummy};
use super::{
    aim_at, ball, body, bursts, centre, choose, ground, kick, pin_camera, run, stand, teleport,
};

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

fn health(harness: &UiHarness, player: Entity) -> (f32, f32) {
    let health = harness.world().get::<Health>(player).expect("a health");
    (health.current(), health.max())
}

fn shield(harness: &UiHarness, player: Entity) -> Option<f32> {
    harness.world().get::<Shield>(player).map(Shield::current)
}

/// The player in the sky takes the pact from beside it.
fn pact(harness: &mut UiHarness, player: Entity) {
    let eye = centre(harness, player);
    book(harness, TomeKind::Pact, eye + Vec2::new(30.0, 0.0));
    harness.step(1);
    assert_eq!(interact(harness, Sound::TomePactTaken), 1);
}

#[test]
fn the_pact_halves_the_health_for_a_full_shield_of_50() {
    let (mut harness, player) = sky();
    assert_eq!(health(&harness, player), (100.0, 100.0));
    pact(&mut harness, player);
    assert_eq!(health(&harness, player), (50.0, 50.0));
    assert_eq!(shield(&harness, player), Some(50.0));
    // Hurt to 80 first, it keeps its share: 40 of 50.
    let (mut harness, player) = sky();
    DamageRequest::new(player, 20.0).apply(harness.world_mut());
    assert_eq!(health(&harness, player), (80.0, 100.0));
    pact(&mut harness, player);
    assert_eq!(health(&harness, player), (40.0, 50.0));
    let full = harness.world().get::<Shield>(player).unwrap();
    assert_eq!((full.current(), full.max()), (50.0, 50.0));
}

#[test]
fn a_hit_in_the_takes_own_step_meets_the_halved_health_behind_the_shield() {
    let (mut harness, player) = sky();
    let eye = centre(&harness, player);
    book(&mut harness, TomeKind::Pact, eye + Vec2::new(30.0, 0.0));
    harness.step(1);
    // The take and a 75-point hit resolve in one step.
    DamageRequest::new(player, 75.0).queue(harness.world_mut());
    harness.harness_mut().press_action(INTERACT);
    harness.step(1);
    harness.harness_mut().release_action(INTERACT);
    assert_eq!(held(&harness, player), [TomeKind::Pact]);
    assert_eq!(health(&harness, player), (25.0, 50.0));
    assert_eq!(shield(&harness, player), Some(0.0));
}

#[test]
fn the_shield_takes_a_hit_first_and_comes_back_three_seconds_after() {
    let (mut harness, player) = sky();
    pact(&mut harness, player);
    DamageRequest::new(player, 30.0).queue(harness.world_mut());
    harness.step(1);
    assert_eq!(shield(&harness, player), Some(20.0));
    assert_eq!(health(&harness, player), (50.0, 50.0), "health whole");
    // It holds for 3 s, then fills at 20 a second.
    harness.step(179);
    let held = shield(&harness, player).unwrap();
    assert!((held - 20.0).abs() < 1e-3, "held through the wait: {held}");
    harness.step(31);
    let rising = shield(&harness, player).unwrap();
    assert!(
        (rising - 30.0).abs() < 0.5,
        "half a second past the wait: {rising}"
    );
    harness.step(70);
    assert_eq!(shield(&harness, player), Some(50.0), "full again");
}

#[test]
fn the_kill_row_kills_a_player_behind_a_full_shield() {
    let (mut harness, player) = sky();
    pact(&mut harness, player);
    assert_eq!(shield(&harness, player), Some(50.0));
    teleport(&mut harness, player, Vec2::new(400.0, KILL_Y + 20.0));
    harness.step(1);
    let world = harness.world();
    assert!(world.get::<Health>(player).unwrap().is_dead());
    assert_eq!(shield(&harness, player), Some(0.0));
}

fn mana(harness: &UiHarness, player: Entity) -> f32 {
    let stats = harness.world().get::<Stats>(player).expect("stats");
    stats.base("mana").expect("mana")
}

fn set_mana(harness: &mut UiHarness, player: Entity, mana: f32) {
    let stats = harness.world_mut().get_mut::<Stats>(player).expect("stats");
    stats.set_base("mana", mana);
}

/// A player in the sky holding bounding, the mana's regeneration held by
/// the `casting` tag, so each step's mana is the jumps' alone.
fn bounder() -> (UiHarness, Entity) {
    let (mut harness, player) = sky();
    let eye = centre(&harness, player);
    book(&mut harness, TomeKind::Bounding, eye + Vec2::new(30.0, 0.0));
    harness.step(1);
    assert_eq!(interact(&mut harness, Sound::TomeBoundingTaken), 1);
    let world = harness.world_mut();
    world.get_mut::<Tags>(player).unwrap().insert("casting");
    let controller = world.get::<PlatformerController>(player).unwrap();
    assert_eq!(controller.air_jump_cost(), Some(("mana", 25.0)));
    (harness, player)
}

/// Presses jump for a frame and releases it the next, then lets the rise
/// run on for two more: whether it jumped, and the sounds of each kind it
/// made meanwhile.
fn jump(harness: &mut UiHarness, player: Entity, sounds: &[Sound]) -> (bool, Vec<usize>) {
    let mut heard = vec![0; sounds.len()];
    let mut jumped = false;
    harness.harness_mut().press_action("jump");
    for frame in 0..4 {
        if frame == 1 {
            harness.harness_mut().release_action("jump");
        }
        harness.step(1);
        let world = harness.world();
        jumped |= world.get::<PlatformerController>(player).unwrap().jumped();
        for (count, sound) in heard.iter_mut().zip(sounds) {
            *count += harness
                .harness()
                .events::<Sounded>()
                .filter(|sounded| sounded.sound == *sound)
                .count();
        }
    }
    (jumped, heard)
}

#[test]
fn bounding_pays_25_for_each_air_jump_past_the_second_and_refuses_24() {
    let (mut harness, player) = bounder();
    assert_eq!(mana(&harness, player), 100.0);
    for what in ["the ground jump", "the free air jump"] {
        assert!(jump(&mut harness, player, &[]).0, "{what}");
        assert_eq!(mana(&harness, player), 100.0, "{what} is free");
    }
    for left in [75.0, 50.0] {
        assert!(jump(&mut harness, player, &[]).0);
        assert_eq!(mana(&harness, player), left, "a bound costs 25");
    }
    set_mana(&mut harness, player, 25.0);
    assert!(jump(&mut harness, player, &[]).0, "exactly 25 pays");
    assert_eq!(mana(&harness, player), 0.0);
    set_mana(&mut harness, player, 24.0);
    let (jumped, heard) = jump(&mut harness, player, &[Sound::NoMana]);
    assert!(!jumped, "24 is refused");
    assert_eq!(heard, [1], "one refusal a press");
    assert_eq!(mana(&harness, player), 24.0);
}

#[test]
fn a_bound_sounds_bounded_and_bursts_its_sparks_never_the_air_jump() {
    let (mut harness, player) = bounder();
    let sounds = [Sound::AirJumped, Sound::Bounded];
    assert_eq!(
        jump(&mut harness, player, &sounds).1,
        [0, 0],
        "the ground jump"
    );
    assert_eq!(
        jump(&mut harness, player, &sounds).1,
        [1, 0],
        "the double jump"
    );
    let doubled = bursts(&harness, "ex10_double_jump");
    assert_eq!(bursts(&harness, "ex10_bound_sparks"), 0);
    assert_eq!(jump(&mut harness, player, &sounds).1, [0, 1], "the bound");
    assert_eq!(bursts(&harness, "ex10_bound_sparks"), 1);
    assert_eq!(bursts(&harness, "ex10_bound_trail"), 1);
    assert_eq!(
        bursts(&harness, "ex10_double_jump"),
        doubled,
        "no double jump's burst"
    );
}

/// The player in `harness` takes `kind` from beside it.
fn take(harness: &mut UiHarness, player: Entity, kind: TomeKind) {
    let eye = centre(harness, player);
    book(harness, kind, eye + Vec2::new(30.0, 0.0));
    harness.step(1);
    assert_eq!(interact(harness, kind.sound()), 1, "{kind:?}");
}

#[test]
fn arcane_power_doubles_the_ten_slots_costs_and_the_huds_payable_follows() {
    let (mut harness, player) = sky();
    assert_eq!(
        spells::cost(harness.world(), player, Spell::FireMissile),
        Some(4.0)
    );
    take(&mut harness, player, TomeKind::Arcane);
    let world = harness.world();
    assert_eq!(spells::cost(world, player, Spell::FireMissile), Some(8.0));
    assert_eq!(spells::cost(world, player, Spell::Summon), Some(20.0));
    let staff = world
        .resource::<DataRegistry>()
        .get::<Ability>("staff_light")
        .unwrap();
    assert_eq!(
        staff.cost_for(world.get::<Stats>(player)),
        Some(("mana", 0.05)),
        "the staff light's step unscaled"
    );
    // The mana's regeneration held, the missile's slot greys under 8.
    harness
        .world_mut()
        .get_mut::<Tags>(player)
        .unwrap()
        .insert("casting");
    set_mana(&mut harness, player, 7.9);
    assert!(!spell_hud::payable(
        harness.world(),
        player,
        Spell::FireMissile
    ));
    set_mana(&mut harness, player, 8.0);
    assert!(spell_hud::payable(
        harness.world(),
        player,
        Spell::FireMissile
    ));
}

/// The first part `dummy` takes after `act`, within `frames` frames: its
/// amount with a crit's ×2 taken out, none of a tick's.
fn first_part(
    harness: &mut UiHarness,
    dummy: Entity,
    frames: u32,
    act: impl FnOnce(&mut UiHarness),
) -> f32 {
    harness
        .world_mut()
        .insert_resource(KitRng::seeded(PLAIN_SEED));
    act(harness);
    for _ in 0..frames {
        harness.step(1);
        let part = harness
            .harness()
            .events::<DamagedPart>()
            .find(|part| part.entity == dummy && !part.periodic)
            .map(|part| part.amount / if part.crit { 2.0 } else { 1.0 });
        if let Some(amount) = part {
            return amount;
        }
    }
    panic!("no hit on the dummy within {frames} frames");
}

/// What the dummy takes first from each of the player's hits, with and
/// without arcane power: the player at x 300 on the sky's ground, a dummy
/// at x 900, the camera held over them.
fn arcane_and_plain(act: impl Fn(&mut UiHarness, Entity, Entity)) -> (f32, f32) {
    let mut amounts = [0.0; 2];
    for (amount, arcane) in amounts.iter_mut().zip([true, false]) {
        let (mut harness, player, dummy) = with_dummy();
        if arcane {
            take(&mut harness, player, TomeKind::Arcane);
            stand(&mut harness, player, 300.0, ARENA);
            harness.step(1);
        }
        pin_camera(&mut harness, Vec2::new(0.0, 500.0));
        *amount = first_part(&mut harness, dummy, 120, |harness| {
            act(harness, player, dummy);
        });
    }
    (amounts[0], amounts[1])
}

#[test]
fn arcane_power_doubles_a_spells_own_hits_and_leaves_the_gun_and_a_ball_alone() {
    let blast = arcane_and_plain(|harness, _, dummy| {
        let at = body(harness, dummy);
        toys::explode(harness.world_mut(), at);
    });
    let strike = arcane_and_plain(|harness, player, dummy| {
        let at = body(harness, dummy);
        lightning::strike(harness.world_mut(), player, at);
    });
    let jump = arcane_and_plain(|harness, player, _| {
        // An iron block 200 px from the dummy, struck: the chain's first
        // jump is the dummy's.
        let iron = toys::place_iron(harness.world_mut(), Vec2::new(700.0, ARENA - IRON_HALF.y));
        harness.step(2);
        let at = body(harness, iron);
        lightning::strike(harness.world_mut(), player, at);
    });
    let pulse = arcane_and_plain(|harness, player, dummy| {
        stand(harness, player, 820.0, ARENA);
        harness.step(1);
        let at = body(harness, dummy);
        aim_at(harness, at);
        choose(harness, 1);
        harness.harness_mut().press_action("cast");
    });
    for (what, (arcane, plain)) in [
        ("the blast", blast),
        ("the strike", strike),
        ("the chain's first jump", jump),
        ("a spray's pulse", pulse),
    ] {
        println!("{what}: {arcane} under arcane power, {plain} without");
        assert!(plain > 0.0, "{what}");
        assert!(
            (arcane - 2.0 * plain).abs() < 1e-3,
            "{what}: {arcane} for twice {plain}"
        );
    }
    let shot = arcane_and_plain(|harness, _, dummy| {
        let at = body(harness, dummy);
        aim_at(harness, at);
        harness.harness_mut().press_action(gun::SHOOT);
    });
    let impact = arcane_and_plain(|harness, _, dummy| {
        let at = body(harness, dummy) - Vec2::new(150.0, 0.0);
        let bowling = ball(harness, at, false);
        kick(harness, bowling, Vec2::new(1500.0, 0.0));
    });
    // The same within a hundredth: the ball's closing speed moves a little
    // with the steps the take adds, which a doubling would dwarf.
    for (what, (arcane, plain)) in [("a shot", shot), ("a bowling ball", impact)] {
        println!("{what}: {arcane} under arcane power, {plain} without");
        assert!(plain > 0.0, "{what}");
        assert!(
            (arcane / plain - 1.0).abs() < 0.01,
            "{what}: {arcane} for {plain}"
        );
    }
}

#[test]
fn all_three_tomes_together_hold_their_own_changes() {
    let (mut harness, player) = sky();
    for kind in TomeKind::ALL {
        take(&mut harness, player, kind);
    }
    assert_eq!(held(&harness, player), TomeKind::ALL);
    assert_eq!(health(&harness, player), (50.0, 50.0));
    assert_eq!(shield(&harness, player), Some(50.0));
    let world = harness.world();
    assert_eq!(spells::cost(world, player, Spell::FireMissile), Some(8.0));
    let controller = world.get::<PlatformerController>(player).unwrap();
    assert_eq!(
        controller.air_jump_cost(),
        Some(("mana", 25.0)),
        "a bound at 25"
    );
}
