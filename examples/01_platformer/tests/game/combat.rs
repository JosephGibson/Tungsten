//! Combat: the hit builder's base, crit, resistances and health; shocked's
//! half again on every type; the ladders' statuses on the dummy and the
//! player; the missile's blast on the dummy, sparing the player; the damage
//! numbers in their types' colours, a crit's at once and larger in yellow,
//! a type's ticks merged, all following the Damage numbers option; the
//! blood a blow spills; and steam from cold a burning target is immune to.

use example_01_platformer::features::combat::numbers::{
    CRIT_SIZE, FOLLOW_SECS, Numbers, TICK_SIZE,
};
use example_01_platformer::features::combat::{CRIT_CHANCE, Hit, Kind};
use example_01_platformer::features::enemies::components::{EnemyKind, EnemyState};
use example_01_platformer::features::enemies::{self};
use example_01_platformer::features::spells::components::School;
use example_01_platformer::features::{dummy, toys};
use example_01_platformer::shared::{Burst, SaveIds};
use example_01_platformer::shell::Difficulty;
use example_01_platformer::shell::options::GameOptions;
use glam::Vec2;
use std::collections::BTreeSet;
use tungsten::core::{DataRef, Entity, EventQueue, ParticleConfigRegistry, ParticleEmitter};
use tungsten::kit::{
    DamageRequest, Damaged, DamagedPart, EffectRequest, Effects, Health, KitRng, Resist,
    StateMachine, Stats, Tags,
};
use tungsten::testing::UiHarness;

use super::{ground, pin_camera, run, stand};

/// The sky's ground.
pub const TOP: f32 = 1000.0;
/// A seed whose stream's first draw crits, and one whose does not.
pub const CRIT_SEED: u64 = 40;
pub const PLAIN_SEED: u64 = 0;

/// A dummy standing on the sky's ground with its body's centre at `x`.
pub fn dummy_at(harness: &mut UiHarness, x: f32) -> Entity {
    dummy::spawn(harness.world_mut(), Vec2::new(x - 32.0, TOP - 61.5))
}

/// A run on the sky's ground, the player at x 300 and a dummy at x 900.
pub fn with_dummy() -> (UiHarness, Entity, Entity) {
    let (mut harness, player) = run();
    ground(&mut harness, TOP);
    stand(&mut harness, player, 300.0, TOP);
    let dummy = dummy_at(&mut harness, 900.0);
    harness.step(2);
    (harness, player, dummy)
}

/// Applies the effect entry `id` to `target` now, from `source`.
pub fn apply(harness: &mut UiHarness, target: Entity, id: &str, source: Option<Entity>) {
    let mut request = EffectRequest::new(target, DataRef::by_id(id));
    if let Some(source) = source {
        request = request.with_source(source);
    }
    request.apply(harness.world_mut());
}

pub fn has(harness: &UiHarness, entity: Entity, id: &str) -> bool {
    harness
        .world()
        .get::<Effects>(entity)
        .is_some_and(|effects| effects.has(id))
}

pub fn health(harness: &UiHarness, entity: Entity) -> f32 {
    harness.world().get::<Health>(entity).unwrap().current()
}

/// Lands a hit of one `kind` part of `amount` on `target` at once, from
/// nobody, so it never crits.
fn land(harness: &mut UiHarness, target: Entity, kind: Kind, amount: f32) {
    let world = harness.world_mut();
    Hit::new(target, [(kind, amount)])
        .request(world)
        .apply(world);
}

/// Queues a hit of one `kind` part of `amount` on `target` and steps once,
/// so the ladder's feed and the effects resolve.
fn hit(harness: &mut UiHarness, target: Entity, kind: Kind, amount: f32) {
    assert!(Hit::new(target, [(kind, amount)]).queue(harness.world_mut()));
    harness.step(1);
}

#[test]
fn a_crit_at_a_pinned_seed_doubles_the_hit_and_sets_the_flag() {
    let first = |seed: u64| KitRng::seeded(seed).unit();
    assert!(first(CRIT_SEED) < CRIT_CHANCE && first(PLAIN_SEED) >= CRIT_CHANCE);
    let (mut harness, player, dummy) = with_dummy();
    let world = harness.world_mut();
    world.insert_resource(KitRng::seeded(CRIT_SEED));
    let hit = Hit::new(dummy, [(Kind::Physical, 10.0), (Kind::Fire, 4.0)]).from(player);
    let request = hit.request(world);
    assert!(request.is_crit());
    let amounts: Vec<f32> = request.parts().iter().map(|part| part.amount()).collect();
    assert_eq!(amounts, [20.0, 8.0], "every part doubled");
    assert_eq!(world.resource::<KitRng>().draws(), 1, "one draw");
    // From nobody, a hit draws nothing and never crits.
    let plain = Hit::new(dummy, [(Kind::Physical, 10.0)]).request(world);
    assert!(!plain.is_crit());
    assert_eq!(world.resource::<KitRng>().draws(), 1);
    assert!(request.queue(world));
    harness.step(1);
    let parts: Vec<(f32, bool)> = harness
        .world()
        .resource::<EventQueue<DamagedPart>>()
        .iter()
        .filter(|part| part.entity == dummy)
        .map(|part| (part.amount, part.crit))
        .collect();
    assert_eq!(parts, [(20.0, true), (8.0, true)]);
    let hits: Vec<bool> = harness
        .world()
        .resource::<EventQueue<Damaged>>()
        .iter()
        .filter(|hit| hit.entity == dummy)
        .map(|hit| hit.crit)
        .collect();
    assert_eq!(hits, [true]);
}

#[test]
fn a_hit_resolves_base_then_crit_then_resist_then_health() {
    let (mut harness, player, dummy) = with_dummy();
    let world = harness.world_mut();
    world.insert::<Resist>(dummy, Resist::new().with_factor("damage.fire", 0.5));
    world.insert_resource(KitRng::seeded(CRIT_SEED));
    // 10 fire, ×2 on the crit, ×0.5 resisted: 10 off 400.
    let request = Hit::new(dummy, [(Kind::Fire, 10.0)])
        .from(player)
        .request(world);
    request.apply(world);
    assert_eq!(health(&harness, dummy), 390.0);
    let world = harness.world_mut();
    world.insert_resource(KitRng::seeded(PLAIN_SEED));
    let request = Hit::new(dummy, [(Kind::Fire, 10.0)])
        .from(player)
        .request(world);
    assert!(!request.is_crit());
    request.apply(world);
    assert_eq!(health(&harness, dummy), 385.0, "no crit: 10 resisted to 5");
    let landed: Vec<f32> = harness
        .world()
        .resource::<EventQueue<DamagedPart>>()
        .iter()
        .filter(|part| part.entity == dummy)
        .map(|part| part.amount)
        .collect();
    assert_eq!(landed, [10.0, 5.0], "each part after its resistance");
}

#[test]
fn shocked_takes_half_again_from_every_type_and_an_immunity_holds() {
    let (mut harness, _, dummy) = with_dummy();
    apply(&mut harness, dummy, "shocked", None);
    for kind in [Kind::Physical, Kind::Fire, Kind::Cold, Kind::Shock] {
        let before = health(&harness, dummy);
        land(&mut harness, dummy, kind, 10.0);
        assert_eq!(before - health(&harness, dummy), 15.0, "{kind:?}");
    }
    harness
        .world_mut()
        .insert::<Resist>(dummy, Resist::new().with_factor("damage.cold", 0.0));
    apply(&mut harness, dummy, "shocked", None);
    let before = health(&harness, dummy);
    land(&mut harness, dummy, Kind::Cold, 10.0);
    assert_eq!(
        health(&harness, dummy),
        before,
        "immune to cold, shocked or not"
    );
    land(&mut harness, dummy, Kind::Fire, 10.0);
    assert_eq!(before - health(&harness, dummy), 15.0);
}

#[test]
fn physical_builds_bleed_on_flesh_and_not_while_frozen() {
    let (mut harness, _, flesh) = with_dummy();
    // The threshold is 20 plus a twentieth of 400; bleeding needs 0.4 of it.
    hit(&mut harness, flesh, Kind::Physical, 20.0);
    assert!(has(&harness, flesh, "bleeding"));
    assert!(
        harness
            .world()
            .get::<Tags>(flesh)
            .unwrap()
            .has("status.bleeding")
    );
    let straw = super::combat::dummy_at(&mut harness, 1200.0);
    harness.step(1);
    harness
        .world_mut()
        .get_mut::<Tags>(straw)
        .unwrap()
        .remove("flesh");
    hit(&mut harness, straw, Kind::Physical, 20.0);
    assert!(!has(&harness, straw, "bleeding"), "no flesh, no bleed");
    let frozen = super::combat::dummy_at(&mut harness, 1500.0);
    harness.step(1);
    apply(&mut harness, frozen, "frozen", None);
    hit(&mut harness, frozen, Kind::Physical, 20.0);
    assert!(
        !has(&harness, frozen, "bleeding"),
        "frozen blocks the bleed"
    );
}

#[test]
fn fire_burns_and_burning_holds_the_dummys_regeneration() {
    let (mut harness, _, dummy) = with_dummy();
    DamageRequest::new(dummy, 100.0).apply(harness.world_mut());
    let hurt = health(&harness, dummy);
    harness.step(30);
    let healed = health(&harness, dummy);
    assert!(healed > hurt, "it regenerates: {hurt} to {healed}");
    // A quarter of the threshold of 40 burns.
    hit(&mut harness, dummy, Kind::Fire, 10.0);
    assert!(has(&harness, dummy, "burning"));
    let lit = health(&harness, dummy);
    let mut low = lit;
    for _ in 0..60 {
        harness.step(1);
        let now = health(&harness, dummy);
        assert!(
            now <= low,
            "burning holds the regeneration: {now} after {low}"
        );
        low = now;
    }
    assert!(low <= lit - 6.0, "two ticks of 3: {lit} to {low}");
}

#[test]
fn cold_chills_then_freezes_the_freezable_and_the_player_only_chills() {
    let (mut harness, player, dummy) = with_dummy();
    // A little past 0.15 of the threshold of 40, its f32 product.
    hit(&mut harness, dummy, Kind::Cold, 6.5);
    assert!(has(&harness, dummy, "chilled") && !has(&harness, dummy, "frozen"));
    hit(&mut harness, dummy, Kind::Cold, 20.5);
    assert!(
        has(&harness, dummy, "frozen"),
        "half the threshold, chilled"
    );
    // The player's threshold is 30 plus a fifth of 100.
    let speed = |harness: &UiHarness| {
        harness
            .world()
            .get::<Stats>(player)
            .unwrap()
            .get("move_speed")
            .unwrap()
    };
    assert_eq!(speed(&harness), 560.0);
    hit(&mut harness, player, Kind::Cold, 8.0);
    harness.step(1);
    assert!(has(&harness, player, "chilled"));
    assert!(
        (speed(&harness) - 364.0).abs() < 1e-3,
        "{}",
        speed(&harness)
    );
    // Past the window, the freeze's whole rung more: never frozen.
    harness.step(70);
    hit(&mut harness, player, Kind::Cold, 25.0);
    assert!(has(&harness, player, "chilled") && !has(&harness, player, "frozen"));
}

#[test]
fn shock_shocks_then_stuns() {
    let (mut harness, _, dummy) = with_dummy();
    hit(&mut harness, dummy, Kind::Shock, 13.0);
    assert!(has(&harness, dummy, "shocked") && !has(&harness, dummy, "stunned"));
    hit(&mut harness, dummy, Kind::Shock, 30.0);
    assert!(has(&harness, dummy, "stunned"), "45 shocked, past 40");
}

#[test]
fn the_missile_blast_burns_the_dummy_and_spares_the_player() {
    let (mut harness, player, dummy) = with_dummy();
    let at = super::body(&harness, dummy);
    super::teleport(&mut harness, player, at + Vec2::new(-40.0, 0.0));
    harness
        .world_mut()
        .insert_resource(KitRng::seeded(PLAIN_SEED));
    let (dummy_before, player_before) = (health(&harness, dummy), health(&harness, player));
    toys::explode(harness.world_mut(), at);
    harness.step(1);
    let fire: Vec<f32> = harness
        .world()
        .resource::<EventQueue<DamagedPart>>()
        .iter()
        .filter(|part| part.entity == dummy)
        .map(|part| part.amount)
        .collect();
    assert_eq!(fire, [20.0], "its whole fire at the centre");
    assert!(health(&harness, dummy) < dummy_before);
    assert!(has(&harness, dummy, "burning"));
    assert_eq!(
        health(&harness, player),
        player_before,
        "the caster is spared"
    );
}

/// A label the last frame drew: its text, its size and its colour's RGB,
/// which keeps while its alpha fades.
type Label = (String, f32, [u8; 3]);

fn labels(harness: &UiHarness) -> Vec<Label> {
    harness.harness().draw().map_or_else(Vec::new, |draw| {
        draw.world_labels
            .iter()
            .map(|label| {
                let [r, g, b, _] = label.color;
                (label.content.clone(), label.font_size, [r, g, b])
            })
            .collect()
    })
}

/// Steps `frames` frames: each label they floated, with the frame it first
/// showed in.
fn floated(harness: &mut UiHarness, frames: usize) -> Vec<(usize, Label)> {
    let mut before = labels(harness);
    let mut fresh = Vec::new();
    for frame in 0..frames {
        harness.step(1);
        let now = labels(harness);
        let mut old = before;
        for label in &now {
            match old.iter().position(|seen| seen == label) {
                Some(i) => {
                    old.remove(i);
                }
                None => fresh.push((frame, label.clone())),
            }
        }
        before = now;
    }
    fresh
}

/// Whether `rgb` is within a step of the palette's `color`.
fn near(rgb: [u8; 3], color: [u8; 4]) -> bool {
    rgb.iter().zip(color).all(|(&a, b)| a.abs_diff(b) <= 3)
}

/// The palette's crit yellow.
const YELLOW: [u8; 4] = [255, 214, 10, 255];

/// A run with the dummy and the camera held over both.
fn watched() -> (UiHarness, Entity, Entity) {
    let (mut harness, player, dummy) = with_dummy();
    pin_camera(&mut harness, Vec2::new(0.0, 400.0));
    harness.step(1);
    (harness, player, dummy)
}

#[test]
fn numbers_float_in_their_types_colour_and_a_crit_larger_in_yellow() {
    let (mut harness, player, dummy) = watched();
    // Fire and cold from nobody, below their ladders' first rungs: each
    // plain part floats at once in its type's colour.
    let plain = Hit::new(dummy, [(Kind::Fire, 6.0), (Kind::Cold, 8.0)]);
    assert!(plain.queue(harness.world_mut()));
    let floated_plain = floated(&mut harness, 50);
    assert_eq!(floated_plain.len(), 2, "{floated_plain:?}");
    for (frame, (_, size, _)) in &floated_plain {
        assert!(*frame <= 1, "at once: {frame}");
        assert_eq!(*size, 16.0);
    }
    let colour = |text: &str| {
        floated_plain
            .iter()
            .find(|(_, label)| label.0 == text)
            .map(|(_, label)| label.2)
            .expect(text)
    };
    assert!(near(colour("6"), School::Fire.color()), "{:?}", colour("6"));
    assert!(near(colour("8"), School::Cold.color()), "{:?}", colour("8"));
    // A crit from the player floats at once, larger, in the crit yellow.
    harness
        .world_mut()
        .insert_resource(KitRng::seeded(CRIT_SEED));
    let crit = Hit::new(dummy, [(Kind::Physical, 5.0)]).from(player);
    assert!(crit.queue(harness.world_mut()));
    let floated_crit = floated(&mut harness, 3);
    assert_eq!(floated_crit.len(), 1, "{floated_crit:?}");
    let (frame, (text, size, rgb)) = &floated_crit[0];
    assert_eq!((*frame, text.as_str(), *size), (0, "10", CRIT_SIZE));
    assert!(near(*rgb, YELLOW), "{rgb:?}");
}

#[test]
fn two_hits_in_a_step_float_apart_and_only_the_crits_in_yellow() {
    let (mut harness, player, dummy) = watched();
    let world = harness.world_mut();
    world.insert_resource(KitRng::seeded(CRIT_SEED));
    // One step: a crit of two parts from the player, and a plain cold hit.
    let crit = Hit::new(dummy, [(Kind::Physical, 3.0), (Kind::Fire, 2.0)]).from(player);
    assert!(crit.queue(world));
    assert!(Hit::new(dummy, [(Kind::Cold, 5.0)]).queue(world));
    let floated = floated(&mut harness, 50);
    assert_eq!(floated.len(), 2, "{floated:?}");
    let (crit_frame, (crit_text, crit_size, crit_rgb)) = &floated[0];
    assert_eq!(
        (*crit_frame, crit_text.as_str()),
        (0, "10"),
        "one sum, at once"
    );
    assert_eq!(*crit_size, CRIT_SIZE);
    assert!(near(*crit_rgb, YELLOW));
    let (cold_frame, (cold_text, cold_size, cold_rgb)) = &floated[1];
    assert!(*cold_frame <= 1, "apart, and at once too");
    assert_eq!((cold_text.as_str(), *cold_size), ("5", 16.0));
    assert!(near(*cold_rgb, School::Cold.color()) && !near(*cold_rgb, YELLOW));
}

#[test]
fn burn_ticks_float_at_once_and_smaller() {
    let (mut harness, _, dummy) = watched();
    apply(&mut harness, dummy, "burning", None);
    // Every tick and every label over two and a half seconds: each tick
    // half a second apart, so each floats at once, at the tick size.
    let mut ticks: Vec<usize> = Vec::new();
    let mut shown: Vec<(usize, f32)> = Vec::new();
    let mut before = labels(&harness);
    for frame in 0..150 {
        harness.step(1);
        let world = harness.world();
        ticks.extend(
            world
                .resource::<EventQueue<DamagedPart>>()
                .iter()
                .filter(|part| part.entity == dummy && part.periodic)
                .map(|_| frame),
        );
        let now = labels(&harness);
        let mut old = before;
        for label in &now {
            match old.iter().position(|seen| seen == label) {
                Some(i) => {
                    old.remove(i);
                }
                None => shown.push((frame, label.1)),
            }
        }
        before = now;
    }
    assert!(ticks.len() >= 2, "{ticks:?}");
    let sizes: Vec<f32> = shown.iter().map(|&(_, size)| size).collect();
    assert!(sizes.iter().all(|&size| size == TICK_SIZE), "{shown:?}");
    let at: Vec<usize> = shown.iter().map(|&(frame, _)| frame).collect();
    assert_eq!(at, ticks, "each tick at once");
}

#[test]
fn a_hit_floats_its_number_at_once() {
    // The first part floats at once; parts that follow within 0.25 s merge
    // into one number when that window closes.
    let (mut harness, _, dummy) = watched();
    assert!(Hit::new(dummy, [(Kind::Cold, 4.0)]).queue(harness.world_mut()));
    let first = floated(&mut harness, 2);
    let texts: Vec<&str> = first.iter().map(|(_, l)| l.0.as_str()).collect();
    assert_eq!(texts, ["4"], "at once: {first:?}");
    assert!(Hit::new(dummy, [(Kind::Cold, 3.0)]).queue(harness.world_mut()));
    harness.step(1);
    assert!(Hit::new(dummy, [(Kind::Cold, 2.0)]).queue(harness.world_mut()));
    let rest = floated(&mut harness, 40);
    let texts: Vec<&str> = rest.iter().map(|(_, l)| l.0.as_str()).collect();
    assert_eq!(texts, ["5"], "the follow-ups' sum: {rest:?}");
    let window = (FOLLOW_SECS * 60.0) as usize;
    assert!(rest[0].0 <= window, "when the window closes: {rest:?}");
}

#[test]
fn numbers_follow_the_damage_numbers_option() {
    let (mut harness, player, dummy) = watched();
    harness
        .world_mut()
        .resource_mut::<GameOptions>()
        .damage_numbers = false;
    let world = harness.world_mut();
    world.insert_resource(KitRng::seeded(CRIT_SEED));
    assert!(
        Hit::new(dummy, [(Kind::Physical, 5.0)])
            .from(player)
            .queue(world)
    );
    assert!(Hit::new(dummy, [(Kind::Fire, 4.0)]).queue(world));
    assert!(floated(&mut harness, 50).is_empty(), "off: nothing floats");
    assert!(harness.world().get_resource::<Numbers>().is_none());
    harness
        .world_mut()
        .resource_mut::<GameOptions>()
        .damage_numbers = true;
    let world = harness.world_mut();
    world.insert_resource(KitRng::seeded(CRIT_SEED));
    assert!(
        Hit::new(dummy, [(Kind::Physical, 5.0)])
            .from(player)
            .queue(world)
    );
    let back = floated(&mut harness, 3);
    assert_eq!(back.len(), 1, "on again: {back:?}");
}

/// Adds to `seen` the bursts of the particle config `name` now alive, by
/// their order: every such burst a run of frames spawned.
fn note_bursts(harness: &UiHarness, name: &str, seen: &mut BTreeSet<u64>) {
    let world = harness.world();
    let config = world.resource::<ParticleConfigRegistry>().id_for_name(name);
    seen.extend(
        world
            .query::<(&Burst, &ParticleEmitter)>()
            .filter(|(_, emitter)| Some(emitter.config) == config)
            .map(|(burst, _)| burst.order),
    );
}

#[test]
fn a_blow_on_flesh_or_the_courier_spurts_blood_and_a_tick_does_not() {
    let (mut harness, player, dummy) = with_dummy();
    let mut blood = BTreeSet::new();
    // A blow on the dummy, which is flesh, past its bleed's rung, and one
    // on the courier.
    hit(&mut harness, dummy, Kind::Physical, 20.0);
    note_bursts(&harness, "ex10_blood_burst", &mut blood);
    assert_eq!(blood.len(), 1, "the dummy bleeds");
    hit(&mut harness, player, Kind::Physical, 5.0);
    note_bursts(&harness, "ex10_blood_burst", &mut blood);
    assert_eq!(blood.len(), 2, "the courier bleeds");
    // The dummy's bleeding ticks spill none.
    let mut ticks = 0;
    for _ in 0..90 {
        harness.step(1);
        ticks += harness
            .harness()
            .events::<Damaged>()
            .filter(|hit| hit.entity == dummy && hit.periodic)
            .count();
        note_bursts(&harness, "ex10_blood_burst", &mut blood);
    }
    assert!(ticks > 0, "it bled");
    assert_eq!(blood.len(), 2, "no tick spurts");
}

#[test]
fn cold_on_a_burning_frost_crawler_bursts_into_steam() {
    let (mut harness, _, _) = with_dummy();
    let kind = EnemyKind::Crawler;
    let centre = Vec2::new(1300.0, TOP - kind.info().half.y - 0.5);
    let world = harness.world_mut();
    let id = world.resource_mut::<SaveIds>().take();
    let crawler = enemies::spawn(world, kind, centre - kind.info().centre, id).unwrap();
    enemies::scale(world, crawler, Difficulty::Normal);
    world.remove_component::<StateMachine<EnemyState>>(crawler);
    harness.step(1);
    apply(&mut harness, crawler, "burning", None);
    harness.step(1);
    assert!(has(&harness, crawler, "burning"), "burning");
    // It takes no cold, but the cold its immunity drops still steams.
    let before = health(&harness, crawler);
    hit(&mut harness, crawler, Kind::Cold, 10.0);
    let mut steam = BTreeSet::new();
    for _ in 0..3 {
        harness.step(1);
        note_bursts(&harness, "ex10_steam_burst", &mut steam);
    }
    assert_eq!(steam.len(), 1, "one steam burst");
    assert!(!has(&harness, crawler, "burning"), "put out");
    assert!(health(&harness, crawler) <= before);
}
