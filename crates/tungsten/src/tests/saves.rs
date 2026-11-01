//! The kit's saves through `App`: every described component saved or named
//! unsaved, and runs restored into Apps whose entities took other indices.

use std::collections::BTreeSet;

use glam::Vec2;
use tungsten_core::physics::{Position, PrevPosition};
use tungsten_core::save::{self, SaveRegistry, WorldSave, capture_world, restore_world};
use tungsten_core::{Config, DataRef, Entity, Schedule, World};

use crate::App;
use crate::kit::{
    ContactDamage, ContactTouch, Damaged, Effect, EffectRequest, Effects, Health, HitShape, Hitbox,
    Hurtbox, KitDescriptions, KitPlugins, UNSAVED_COMPONENTS,
};
use crate::testing::Harness;

/// What a restored spawn takes, as a state's marker.
#[derive(Debug, Clone, Copy)]
struct Restored;

/// An App with the kit at one fixed step a frame, which spawned and
/// despawned `churn` entities first.
fn harness(churn: usize) -> Harness {
    let mut app = App::new(Config::default()).expect("App::new failed");
    app.add_plugins(KitPlugins::set());
    let mut harness = Harness::new(app);
    harness.set_dt(1.0 / 60.0);
    let world = harness.world_mut();
    let spawned: Vec<Entity> = (0..churn).map(|_| world.spawn()).collect();
    for entity in spawned {
        world.despawn(entity);
    }
    harness
}

fn named(world: &mut World, entity: Entity, name: &str) -> Entity {
    save::name(world, entity, None, name).unwrap();
    entity
}

fn at(x: f32) -> (Position, PrevPosition) {
    (Position(Vec2::new(x, 0.0)), PrevPosition(Vec2::new(x, 0.0)))
}

fn health(harness: &Harness, entity: Entity) -> f32 {
    harness.world().get::<Health>(entity).unwrap().current()
}

fn capture(harness: &Harness) -> WorldSave {
    capture_world(harness.world()).unwrap()
}

fn restore(harness: &mut Harness, saved: &WorldSave) {
    let report = restore_world(harness.world_mut(), saved, &(Restored,)).unwrap();
    assert!(report.is_clean(), "{report:?}");
}

#[test]
fn every_described_kit_component_is_saved_or_named_unsaved() {
    let mut world = World::new();
    KitPlugins::set().build(&mut Schedule::new(), &mut world);
    let described: BTreeSet<&str> = world
        .resource::<KitDescriptions>()
        .iter()
        .map(crate::kit::ComponentDescription::name)
        .collect();
    let registered: BTreeSet<&str> = world.resource::<SaveRegistry>().component_names().collect();
    let saved: BTreeSet<&str> = described.intersection(&registered).copied().collect();
    let unsaved: BTreeSet<&str> = UNSAVED_COMPONENTS.iter().map(|&(name, _)| name).collect();
    assert_eq!(unsaved.len(), UNSAVED_COMPONENTS.len(), "each named once");
    assert!(
        saved.is_disjoint(&unsaved),
        "{:?}",
        saved.intersection(&unsaved)
    );
    assert_eq!(
        saved.union(&unsaved).copied().collect::<BTreeSet<_>>(),
        described,
        "together, every described component"
    );
    assert!(UNSAVED_COMPONENTS.iter().all(|(_, why)| !why.is_empty()));
}

/// A sword whose window is open, a target in its reach and one beyond it.
fn duel(world: &mut World) -> [Entity; 3] {
    let mut sword = Hitbox::new(HitShape::circle(20.0), 1.0).with_window(0.5);
    sword.arm();
    let sword = world.spawn_with((at(0.0).0, at(0.0).1, sword));
    let target = |world: &mut World, x: f32| {
        let (position, prev) = at(x);
        world.spawn_with((
            position,
            prev,
            Hurtbox::new(HitShape::circle(4.0)),
            Health::new(10.0),
        ))
    };
    let near = target(world, 10.0);
    let far = target(world, 100.0);
    [
        named(world, sword, "sword"),
        named(world, near, "near"),
        named(world, far, "far"),
    ]
}

#[test]
fn a_save_mid_hit_loads_in_a_churned_world_without_a_second_hit() {
    let mut a = harness(0);
    let [_, near_a, far_a] = duel(a.world_mut());
    a.step(1);
    assert_eq!((health(&a, near_a), health(&a, far_a)), (9.0, 10.0));
    let saved = capture(&a);

    let mut b = harness(100);
    let [sword, near, far] = duel(b.world_mut());
    assert_ne!(near, near_a, "another entity holds the name");
    restore(&mut b, &saved);
    assert_eq!(
        b.world().get::<Hitbox>(sword).unwrap().hit_targets(),
        [near]
    );
    let (position, prev) = at(12.0);
    b.world_mut().insert(far, position);
    b.world_mut().insert(far, prev);
    b.step(40);
    assert!(
        !b.world().get::<Hitbox>(sword).unwrap().is_armed(),
        "ran out"
    );
    assert_eq!(
        (health(&b, near), health(&b, far)),
        (9.0, 9.0),
        "the first took one hit, the second its first"
    );
}

/// A mage and a golem burning from the mage's effect.
fn burning(world: &mut World) -> [Entity; 2] {
    let mage = world.spawn_with((at(0.0).0,));
    let golem = world.spawn_with((at(50.0).0, Health::new(20.0)));
    [named(world, mage, "mage"), named(world, golem, "golem")]
}

#[test]
fn an_effect_restored_in_a_churned_world_keeps_its_source_and_time() {
    let mut a = harness(0);
    let [mage_a, golem_a] = burning(a.world_mut());
    let burn = DataRef::inline(
        Effect::new()
            .with_duration(2.0)
            .with_period(0.5)
            .with_damage(1.0),
    );
    EffectRequest::new(golem_a, burn)
        .with_source(mage_a)
        .apply(a.world_mut());
    a.step(20);
    let saved = capture(&a);

    let mut b = harness(100);
    let [mage_b, golem_b] = burning(b.world_mut());
    restore(&mut b, &saved);
    let instance = |harness: &Harness, golem: Entity| {
        harness
            .world()
            .get::<Effects>(golem)
            .and_then(|effects| effects.instances().next().cloned())
    };
    let (kept, then) = (
        instance(&b, golem_b).unwrap(),
        instance(&a, golem_a).unwrap(),
    );
    assert_eq!(kept.source(), Some(mage_b));
    assert_eq!(kept.left(), then.left(), "its time");
    for frame in 0..120 {
        a.step(1);
        b.step(1);
        assert_eq!(health(&a, golem_a), health(&b, golem_b), "frame {frame}");
        let sources = |harness: &Harness, mage: Entity| {
            harness
                .events::<Damaged>()
                .map(|damaged| damaged.source == Some(mage))
                .collect::<Vec<_>>()
        };
        assert_eq!(sources(&a, mage_a), sources(&b, mage_b), "frame {frame}");
    }
    assert!(instance(&b, golem_b).is_none(), "it ran out on time");
    assert!(health(&b, golem_b) < 20.0);
}

/// Spikes touching a hero.
fn spikes(world: &mut World) -> [Entity; 2] {
    let spikes = world.spawn_with((
        at(0.0).0,
        at(0.0).1,
        ContactDamage::new(1.0)
            .with_interval(0.5)
            .with_shape(HitShape::circle(8.0), Vec2::ZERO),
    ));
    let hero = world.spawn_with((
        at(4.0).0,
        at(4.0).1,
        Hurtbox::new(HitShape::circle(4.0)),
        Health::new(10.0),
    ));
    [named(world, spikes, "spikes"), named(world, hero, "hero")]
}

#[test]
fn a_touch_restored_in_a_churned_world_keeps_its_schedule() {
    let mut a = harness(0);
    let [spikes_a, hero_a] = spikes(a.world_mut());
    a.step(12);
    assert_eq!(health(&a, hero_a), 9.0, "hurt at once");
    let touch: Vec<ContactTouch> = a
        .world()
        .get::<ContactDamage>(spikes_a)
        .unwrap()
        .touches()
        .collect();
    assert_eq!(touch.len(), 1);
    let saved = capture(&a);

    let mut b = harness(100);
    let [spikes_b, hero_b] = spikes(b.world_mut());
    restore(&mut b, &saved);
    let restored: Vec<ContactTouch> = b
        .world()
        .get::<ContactDamage>(spikes_b)
        .unwrap()
        .touches()
        .collect();
    assert_eq!(restored.len(), 1);
    assert_eq!(restored[0].target(), hero_b);
    assert_eq!(
        (restored[0].since_hit(), restored[0].since_met()),
        (touch[0].since_hit(), touch[0].since_met())
    );
    for frame in 0..60 {
        a.step(1);
        b.step(1);
        assert_eq!(health(&a, hero_a), health(&b, hero_b), "frame {frame}");
    }
    assert_eq!(health(&b, hero_b), 7.0, "two more hits, on the schedule");
}

/// Two hazards, each touching both heroes on its own interval.
fn hazards(world: &mut World) {
    for (name, interval) in [("brambles", 0.25), ("embers", 0.4)] {
        let hazard = world.spawn_with((
            at(0.0).0,
            at(0.0).1,
            ContactDamage::new(1.0)
                .with_interval(interval)
                .with_shape(HitShape::circle(24.0), Vec2::ZERO),
        ));
        named(world, hazard, name);
    }
    for (name, x) in [("ana", 6.0), ("bo", -6.0)] {
        let hero = world.spawn_with((
            at(x).0,
            at(x).1,
            Hurtbox::new(HitShape::circle(4.0)),
            Health::new(1000.0),
        ));
        named(world, hero, name);
    }
}

/// Each frame's hits: the hurt entity's and the source's save IDs, and the
/// amount, in the order the step resolved them.
fn hits_over(harness: &mut Harness, frames: u32) -> Vec<Vec<(u64, Option<u64>, f32)>> {
    let id = |world: &World, entity: Entity| {
        world
            .get::<tungsten_core::Saved>(entity)
            .map(|saved| saved.id().0)
    };
    (0..frames)
        .map(|_| {
            harness.step(1);
            let world = harness.world();
            harness
                .events::<Damaged>()
                .map(|damaged| {
                    (
                        id(world, damaged.entity).unwrap(),
                        damaged.source.and_then(|source| id(world, source)),
                        damaged.amount,
                    )
                })
                .collect()
        })
        .collect()
}

#[test]
fn a_loaded_run_resolves_its_steps_hits_as_the_uninterrupted_run() {
    let mut a = harness(0);
    hazards(a.world_mut());
    a.step(30);
    let saved = capture(&a);

    let mut b = harness(100);
    hazards(b.world_mut());
    restore(&mut b, &saved);
    let uninterrupted = hits_over(&mut a, 120);
    let loaded = hits_over(&mut b, 120);
    assert!(
        uninterrupted.iter().filter(|hits| hits.len() > 1).count() > 3,
        "steps with several hits"
    );
    assert_eq!(loaded, uninterrupted);
    for name in ["ana", "bo"] {
        let health = |harness: &Harness| {
            let entity = harness
                .world()
                .resource::<save::SaveState>()
                .named_entity(save::SaveId::authored(name))
                .unwrap();
            health(harness, entity)
        };
        assert_eq!(health(&b), health(&a), "{name}");
    }
}
