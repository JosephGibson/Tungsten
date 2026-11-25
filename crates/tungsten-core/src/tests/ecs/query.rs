//! Tuple queries on a fragmented world, checked against per-entity lookups
//! in the order `query::<Entity>()` walks the archetypes.

use super::super::command_buffer::CommandBuffer;
use super::*;
use crate::ecs::World;

#[derive(Debug, Clone, Copy, PartialEq)]
struct Pos(f32, f32);
#[derive(Debug, Clone, Copy, PartialEq)]
struct Vel(f32, f32);
#[derive(Debug, Clone, Copy, PartialEq)]
struct Health(f32);
#[derive(Debug, Clone, Copy, PartialEq)]
struct Regen(f32);
#[derive(Debug, Clone, Copy, PartialEq)]
struct Stats([f32; 2]);
#[derive(Debug, Clone, Copy, PartialEq)]
struct Follow(u32);
#[derive(Debug, Clone, Copy)]
struct Tag<const N: u8>;
/// Not `Send`, not `Sync`, not `Clone`: the queries carry no bound.
struct Shared(std::rc::Rc<std::cell::RefCell<u32>>);

/// 64 entities over 16 archetypes: tag bits 0–1, `Regen` on every third,
/// `Stats` on every fourth, `Follow` on every fifth; two spawned without
/// `Pos` and one without `Vel`, plus an empty archetype left by a move.
fn fragmented() -> World {
    let mut world = World::new();
    for index in 0..64_u32 {
        let entity = world.spawn();
        if index != 7 && index != 40 {
            world.insert(entity, Pos(index as f32, 2.0 * index as f32));
        }
        if index != 23 {
            world.insert(entity, Vel(1.0, -1.0));
        }
        world.insert(entity, Health(100.0 - index as f32));
        if index % 3 == 0 {
            world.insert(entity, Regen(0.5));
        }
        if index % 4 == 0 {
            world.insert(entity, Stats([1.0, 2.0]));
        }
        if index % 5 == 0 {
            world.insert(entity, Follow(index));
        }
        if index & 1 != 0 {
            world.insert(entity, Tag::<0>);
        }
        if index & 2 != 0 {
            world.insert(entity, Tag::<1>);
        }
        if index == 9 {
            world.insert(entity, Shared(std::rc::Rc::default()));
        }
    }
    // Leave one archetype empty: the entity that held `Shared` moves on.
    let shared = world
        .query_filtered::<Entity, With<Shared>>()
        .next()
        .unwrap();
    world.remove_component::<Shared>(shared);
    world
}

/// Every entity in query order: archetype order, then row order.
fn entities(world: &World) -> Vec<Entity> {
    world.query::<Entity>().collect()
}

/// One entity's row of the five-column read below, through `get`.
type Row = (Entity, Pos, Vel, Health, Option<Regen>, Option<Stats>);

fn row_by_lookup(world: &World, entity: Entity) -> Option<Row> {
    Some((
        entity,
        *world.get::<Pos>(entity)?,
        *world.get::<Vel>(entity)?,
        *world.get::<Health>(entity)?,
        world.get::<Regen>(entity).copied(),
        world.get::<Stats>(entity).copied(),
    ))
}

/// The five-column read through the query.
fn rows(world: &World) -> Vec<Row> {
    world
        .query::<(Entity, &Pos, &Vel, &Health, Option<&Regen>, Option<&Stats>)>()
        .map(|(e, p, v, h, r, s)| (e, *p, *v, *h, r.copied(), s.copied()))
        .collect()
}

#[test]
fn read_tuples_match_per_entity_lookups() {
    let world = fragmented();
    let expected: Vec<_> = entities(&world)
        .into_iter()
        .filter_map(|e| Some((e, *world.get::<Pos>(e)?, *world.get::<Vel>(e)?)))
        .collect();
    let tuple: Vec<_> = world
        .query::<(Entity, &Pos, &Vel)>()
        .map(|(e, p, v)| (e, *p, *v))
        .collect();
    assert_eq!(tuple, expected);
    assert_eq!(tuple.len(), 61);

    let expected: Vec<_> = entities(&world)
        .into_iter()
        .filter_map(|e| row_by_lookup(&world, e))
        .collect();
    let tuple = rows(&world);
    assert_eq!(tuple, expected);
    assert!(tuple.iter().any(|row| row.4.is_some()));
    assert!(tuple.iter().any(|row| row.4.is_none()));
}

#[test]
fn single_items_need_no_tuple() {
    let mut world = fragmented();
    let expected: Vec<Health> = entities(&world)
        .into_iter()
        .filter_map(|e| world.get::<Health>(e).copied())
        .collect();
    let single: Vec<Health> = world.query::<&Health>().copied().collect();
    assert_eq!(single, expected);
    assert_eq!(single.len(), 64);
    for health in world.query_mut::<&mut Health>() {
        health.0 += 1.0;
    }
    let bumped: Vec<Health> = world.query::<&Health>().copied().collect();
    assert!(bumped.iter().zip(&expected).all(|(b, a)| b.0 == a.0 + 1.0));
    assert_eq!(world.query::<Entity>().count(), 64);
    let optional: Vec<Option<Regen>> = world
        .query::<Option<&Regen>>()
        .map(Option::<&Regen>::copied)
        .collect();
    assert_eq!(optional.len(), 64);
    assert_eq!(optional.iter().filter(|r| r.is_some()).count(), 22);
}

#[test]
fn mutable_tuples_write_every_matching_row() {
    let mut expected = fragmented();
    let mut world = fragmented();
    // The same writes through `get_mut`, entity by entity.
    for entity in entities(&expected) {
        let Some(&vel) = expected.get::<Vel>(entity) else {
            continue;
        };
        if let Some(pos) = expected.get_mut::<Pos>(entity) {
            pos.0 += vel.0;
            pos.1 += vel.1;
        }
    }
    for (pos, vel) in world.query_mut::<(&mut Pos, &Vel)>() {
        pos.0 += vel.0;
        pos.1 += vel.1;
    }
    for entity in entities(&expected) {
        let (Some(&health), true) = (expected.get::<Health>(entity), expected.has::<Vel>(entity))
        else {
            continue;
        };
        if let Some(&regen) = expected.get::<Regen>(entity) {
            expected.get_mut::<Vel>(entity).unwrap().0 += regen.0 * health.0;
        }
        if let Some(stats) = expected.get_mut::<Stats>(entity) {
            stats.0[0] = health.0;
        }
    }
    for (health, vel, regen, stats) in
        world.query_mut::<(&Health, &mut Vel, Option<&Regen>, Option<&mut Stats>)>()
    {
        if let Some(regen) = regen {
            vel.0 += regen.0 * health.0;
        }
        if let Some(stats) = stats {
            stats.0[0] = health.0;
        }
    }
    let mut picked = Vec::new();
    for (entity, health) in world.query_mut::<(Entity, &mut Health)>() {
        if health.0 < 41.0 {
            picked.push(entity);
        }
    }
    let low: Vec<Entity> = entities(&expected)
        .into_iter()
        .filter(|&e| expected.get::<Health>(e).is_some_and(|h| h.0 < 41.0))
        .collect();
    assert_eq!(picked, low);

    let right = rows(&world);
    assert_eq!(rows(&expected), right);
    assert!(
        right
            .iter()
            .any(|row| row.5.is_some_and(|s| s.0[0] == row.3.0))
    );
    // The two entities without `Pos` are outside `rows`.
    let vels = |world: &World| -> Vec<(Entity, Vel)> {
        world
            .query::<(Entity, &Vel)>()
            .map(|(e, v)| (e, *v))
            .collect()
    };
    assert_eq!(vels(&expected), vels(&world));
}

#[test]
fn filters_narrow_by_archetype() {
    let mut expected = fragmented();
    let mut world = fragmented();
    for entity in entities(&expected) {
        if expected.has::<Follow>(entity) {
            continue;
        }
        let (Some(_), Some(&vel), Some(_)) = (
            expected.get::<Pos>(entity),
            expected.get::<Vel>(entity),
            expected.get::<Health>(entity),
        ) else {
            continue;
        };
        expected.get_mut::<Pos>(entity).unwrap().0 -= vel.0;
        expected.get_mut::<Health>(entity).unwrap().0 -= 1.0;
    }
    for (pos, vel, health) in
        world.query_mut_filtered::<(&mut Pos, &Vel, &mut Health), Without<Follow>>()
    {
        pos.0 -= vel.0;
        health.0 -= 1.0;
    }
    let followers: Vec<Entity> = entities(&world)
        .into_iter()
        .filter(|&e| world.has::<Follow>(e))
        .collect();
    let filtered: Vec<Entity> = world.query_filtered::<Entity, With<Follow>>().collect();
    assert_eq!(filtered, followers);
    assert_eq!(filtered.len(), 13);
    let both: Vec<Entity> = world
        .query_filtered::<Entity, (With<Follow>, Without<Stats>, ())>()
        .collect();
    assert_eq!(both.len(), 13 - 4);
    assert_eq!(rows(&expected), rows(&world));
}

#[test]
fn optional_only_queries_are_bounded_by_rows() {
    let world = fragmented();
    let rows: Vec<(Entity, Option<Regen>)> = world
        .query::<(Entity, Option<&Regen>)>()
        .map(|(e, r)| (e, r.copied()))
        .collect();
    assert_eq!(rows.len(), 64);
    let only_optional: Vec<Option<Stats>> = world
        .query::<(Option<&Stats>,)>()
        .map(|(s,)| s.copied())
        .collect();
    assert_eq!(only_optional.len(), 64);
    assert_eq!(only_optional.iter().filter(|s| s.is_some()).count(), 16);
    assert_eq!(World::new().query::<(Option<&Stats>,)>().count(), 0);
}

#[test]
fn slices_cover_each_archetype_once() {
    let mut world = fragmented();
    let mut rows_seen = 0;
    for (rows, (entities, pos, regen)) in world.query_slices::<(Entity, &Pos, Option<&Regen>)>() {
        assert_eq!(entities.len(), rows);
        assert_eq!(pos.len(), rows);
        if let Some(regen) = regen {
            assert_eq!(regen.len(), rows);
        }
        rows_seen += rows;
    }
    assert_eq!(rows_seen, 62);
    let unfollowed: usize = world
        .query_slices_filtered::<&Pos, Without<Follow>>()
        .map(|(rows, pos)| {
            assert_eq!(pos.len(), rows);
            rows
        })
        .sum();
    assert_eq!(
        unfollowed,
        world.query_filtered::<&Pos, Without<Follow>>().count()
    );
    for (rows, (vel, stats)) in world.query_mut_slices::<(&mut Vel, Option<&mut Stats>)>() {
        assert_eq!(vel.len(), rows);
        for vel in vel.iter_mut() {
            vel.1 = 0.0;
        }
        if let Some(stats) = stats {
            for stats in stats.iter_mut() {
                stats.0 = [0.0; 2];
            }
        }
    }
    assert!(world.query::<&Vel>().all(|v| v.1 == 0.0));
    assert!(world.query::<&Stats>().all(|s| s.0 == [0.0; 2]));
    for (_, vel) in world.query_mut_slices_filtered::<&mut Vel, With<Follow>>() {
        for vel in vel {
            vel.0 = -1.0;
        }
    }
    assert!(
        world
            .query::<(&Vel, Option<&Follow>)>()
            .all(|(v, f)| (v.0 == -1.0) == f.is_some())
    );
}

#[test]
fn unbounded_component_types_query() {
    let mut world = fragmented();
    let shared = world.spawn();
    world.insert(shared, Shared(std::rc::Rc::default()));
    world.insert(shared, Health(1.0));
    for (cell, health) in world.query_mut::<(&Shared, &mut Health)>() {
        *cell.0.borrow_mut() += 1;
        health.0 += 1.0;
    }
    assert_eq!(world.query::<&Shared>().count(), 1);
    assert_eq!(world.get::<Health>(shared).map(|h| h.0), Some(2.0));
}

#[test]
fn a_query_shape_sees_the_archetypes_added_after_its_first_run() {
    // Each shape runs once, so its matching archetypes are worked out; then
    // new archetypes appear, and each run again matches per-entity lookups.
    let mut world = fragmented();
    let filtered = |world: &World| -> Vec<Entity> {
        world
            .query_filtered::<Entity, (With<Regen>, Without<Stats>)>()
            .collect()
    };
    let before = (rows(&world).len(), filtered(&world).len());
    for index in 0..5_u32 {
        let entity = world.spawn();
        world.insert(entity, Pos(index as f32, 0.0));
        world.insert(entity, Vel(0.0, 1.0));
        world.insert(entity, Health(1.0));
        world.insert(entity, Regen(0.25));
        match index {
            0 => world.insert(entity, Tag::<7>),
            1 => {
                world.insert(entity, Tag::<7>);
                world.insert(entity, Tag::<8>);
            }
            _ => world.insert(entity, Follow(index)),
        }
    }
    let expected: Vec<_> = entities(&world)
        .into_iter()
        .filter_map(|e| row_by_lookup(&world, e))
        .collect();
    assert_eq!(rows(&world), expected);
    assert_eq!(rows(&world).len(), before.0 + 5);
    let expected: Vec<Entity> = entities(&world)
        .into_iter()
        .filter(|&e| world.has::<Regen>(e) && !world.has::<Stats>(e))
        .collect();
    assert_eq!(filtered(&world), expected);
    assert_eq!(expected.len(), before.1 + 5);
    // A write through the same shape reaches the new rows too.
    for (_, regen) in world.query_mut_filtered::<(&Health, &mut Regen), With<Tag<7>>>() {
        regen.0 = 9.0;
    }
    let tagged: Vec<f32> = world
        .query_filtered::<&Regen, With<Tag<7>>>()
        .map(|regen| regen.0)
        .collect();
    assert_eq!(tagged, [9.0, 9.0]);
}

#[test]
fn structural_changes_between_queries_are_seen() {
    let mut world = fragmented();
    let mut buffer = CommandBuffer::new();
    let spawned = buffer.spawn();
    buffer.insert_pending(spawned, Pos(0.0, 0.0));
    buffer.insert_pending(spawned, Vel(0.0, 0.0));
    world.flush(buffer);
    assert_eq!(world.query::<(&Pos, &Vel)>().count(), 62);
    let first = world
        .query::<(Entity, &Pos)>()
        .next()
        .map(|(e, _)| e)
        .unwrap();
    world.despawn(first);
    assert_eq!(world.query::<(&Pos, &Vel)>().count(), 61);
}

#[test]
#[should_panic(
    expected = "(&mut tungsten_core::ecs::query::tests::Pos, &mut tungsten_core::ecs::query::tests::Pos): component types must be distinct"
)]
fn two_mutable_items_of_one_type_panic() {
    let mut world = fragmented();
    let _ = world.query_mut::<(&mut Pos, &mut Pos)>().count();
}

#[test]
#[should_panic(
    expected = "(&tungsten_core::ecs::query::tests::Pos, &mut tungsten_core::ecs::query::tests::Pos): component types must be distinct"
)]
fn a_read_and_a_write_of_one_type_panic() {
    let mut world = fragmented();
    let _ = world.query_mut::<(&Pos, &mut Pos)>().count();
}

#[test]
#[should_panic(
    expected = "(&tungsten_core::ecs::query::tests::Pos, core::option::Option<&mut tungsten_core::ecs::query::tests::Pos>): component types must be distinct"
)]
fn an_optional_write_of_a_read_type_panics() {
    let mut world = fragmented();
    let _ = world.query_mut::<(&Pos, Option<&mut Pos>)>().count();
}

#[test]
fn two_shared_reads_of_one_type_are_allowed_read_only() {
    let world = fragmented();
    assert_eq!(world.query::<(&Pos, &Pos)>().count(), 62);
}

// A query's collection against a walk of every archetype (phase 7's C4):
// the collection reads its shape's kept bits and gives no entry for an
// archetype with no rows, and the rows are the ones the old walk gave.

/// The old collection, kept as the reference: every archetype walked with
/// its bit tested, one `(index, rows, slices)` entry for each match whether
/// or not it has rows.
fn every_archetype<'w, Q: ReadOnlyQueryData<'w>, F: QueryFilter>(
    world: &'w World,
) -> Vec<(usize, usize, Q::Slices)> {
    let matched = world.archetypes.matching(Q::matches, F::matches);
    world
        .archetypes
        .archetypes
        .iter()
        .enumerate()
        .filter(|&(index, _)| matched[index / 64] >> (index % 64) & 1 == 1)
        .map(|(index, arch)| (index, arch.entities.len(), Q::slices(Columns::of(arch))))
        .collect()
}

/// [`every_archetype`] for a query that writes.
fn every_archetype_mut<'w, Q: QueryData<'w>, F: QueryFilter>(
    world: &'w mut World,
) -> Vec<(usize, usize, Q::Slices)> {
    Q::check_access();
    let matched = world.archetypes.matching(Q::matches, F::matches);
    world
        .archetypes
        .archetypes
        .iter_mut()
        .enumerate()
        .filter(|&(index, _)| matched[index / 64] >> (index % 64) & 1 == 1)
        .map(|(index, arch)| {
            let rows = arch.entities.len();
            (index, rows, Q::slices_mut(ColumnsMut::of(arch)))
        })
        .collect()
}

/// The test's own stream (splitmix64).
fn draw(stream: &mut u64) -> u64 {
    *stream = stream.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *stream;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// A seeded World whose rows come and go over a few hundred archetypes: a
/// mask's nine bits name an entity's components, inserted one at a time, so
/// each prefix of a run is an archetype of its own, most of them empty.
struct Churn {
    world: World,
    stream: u64,
    /// Each live entity with the mask it wears.
    live: Vec<(Entity, u32)>,
    /// Masks whose archetype a round emptied, for a later round to refill.
    emptied: Vec<u32>,
    spawned: u32,
    refilled: u32,
}

impl Churn {
    const REGEN: u32 = 4;
    const NO_POS: u32 = 256;

    fn new(seed: u64) -> Self {
        let mut churn = Self {
            world: World::new(),
            stream: seed,
            live: Vec::new(),
            emptied: Vec::new(),
            spawned: 0,
            refilled: 0,
        };
        for _ in 0..400 {
            let mask = (draw(&mut churn.stream) % 512) as u32;
            churn.spawn(mask);
        }
        churn
    }

    fn spawn(&mut self, mask: u32) {
        let world = &mut self.world;
        let entity = world.spawn();
        self.spawned += 1;
        let n = self.spawned;
        if mask & Self::NO_POS == 0 {
            world.insert(entity, Pos(n as f32, 0.5 * n as f32));
        }
        if mask & 1 != 0 {
            world.insert(entity, Vel(n as f32, -1.0));
        }
        if mask & 2 != 0 {
            world.insert(entity, Health(n as f32));
        }
        if mask & Self::REGEN != 0 {
            world.insert(entity, Regen(0.5));
        }
        if mask & 8 != 0 {
            world.insert(entity, Stats([n as f32, 2.0]));
        }
        if mask & 16 != 0 {
            world.insert(entity, Follow(n));
        }
        if mask & 32 != 0 {
            world.insert(entity, Tag::<0>);
        }
        if mask & 64 != 0 {
            world.insert(entity, Tag::<1>);
        }
        if mask & 128 != 0 {
            world.insert(entity, Tag::<2>);
        }
        self.live.push((entity, mask));
    }

    fn pick(&mut self) -> Option<(Entity, u32)> {
        if self.live.is_empty() {
            return None;
        }
        let at = (draw(&mut self.stream) % self.live.len() as u64) as usize;
        Some(self.live[at])
    }

    /// One round: an archetype emptied by despawns, one by a remove that
    /// moves its rows on, an emptied one refilled, and a few loose spawns
    /// and despawns.
    fn round(&mut self) {
        if let Some((_, mask)) = self.pick() {
            let world = &mut self.world;
            self.live.retain(|&(entity, worn)| {
                if worn == mask {
                    world.despawn(entity);
                }
                worn != mask
            });
            self.emptied.push(mask);
        }
        if let Some((_, mask)) = self.pick()
            && mask & Self::REGEN != 0
        {
            for (entity, worn) in &mut self.live {
                if *worn == mask {
                    self.world.remove_component::<Regen>(*entity);
                    *worn &= !Self::REGEN;
                }
            }
            self.emptied.push(mask);
        }
        if self.emptied.len() > 2 {
            let at = (draw(&mut self.stream) % self.emptied.len() as u64) as usize;
            let mask = self.emptied.swap_remove(at);
            for _ in 0..=draw(&mut self.stream) % 3 {
                self.spawn(mask);
            }
            self.refilled += 1;
        }
        for _ in 0..3 {
            let mask = (draw(&mut self.stream) % 512) as u32;
            self.spawn(mask);
            if let Some((entity, _)) = self.pick() {
                self.world.despawn(entity);
                self.live.retain(|&(live, _)| live != entity);
            }
        }
    }

    fn clear(&mut self) {
        for (entity, _) in self.live.drain(..) {
            self.world.despawn(entity);
        }
    }
}

type Read<'w> = (Entity, &'w Pos, Option<&'w Regen>);
type ReadRow = (Entity, Pos, Option<Regen>);
type Write<'w> = (Entity, &'w mut Health, Option<&'w mut Stats>);

/// Every entity's components by lookup, in a walk of every archetype: the
/// archetypes in order, each one's rows in order.
type Whole = (
    Entity,
    Option<Pos>,
    Option<Vel>,
    Option<Health>,
    Option<Regen>,
    Option<Stats>,
    Option<Follow>,
);

fn whole(world: &World) -> Vec<Whole> {
    world
        .archetypes
        .archetypes
        .iter()
        .flat_map(|arch| arch.entities.iter().copied())
        .map(|e| {
            (
                e,
                world.get::<Pos>(e).copied(),
                world.get::<Vel>(e).copied(),
                world.get::<Health>(e).copied(),
                world.get::<Regen>(e).copied(),
                world.get::<Stats>(e).copied(),
                world.get::<Follow>(e).copied(),
            )
        })
        .collect()
}

fn read_rows<'w>(
    parts: impl IntoIterator<Item = (usize, <Read<'w> as QueryData<'w>>::Slices)>,
) -> Vec<ReadRow> {
    parts
        .into_iter()
        .flat_map(|(rows, slices)| <Read<'w> as QueryData<'w>>::iter(slices, rows))
        .map(|(e, p, r)| (e, *p, r.copied()))
        .collect()
}

/// What a round's read checks saw of the cases the collection must handle.
#[derive(Default)]
struct Seen {
    /// Matching archetypes with no rows between two that had some.
    skipped_between: usize,
    /// The highest index of a matching archetype with rows.
    top: usize,
    /// Rounds where a walk that ends at the first matching archetype with
    /// no rows gives other rows than the collection.
    ending_at_an_empty_differs: usize,
    /// Rounds where a walk of the bits' first word alone does.
    first_word_alone_differs: usize,
}

/// The read-only forms under `F` against the old collection of `world`.
fn check_reads<F: QueryFilter>(world: &World, seen: &mut Seen) {
    let old = every_archetype::<Read<'_>, F>(world);
    let new: Vec<_> = world.query_slices_filtered::<Read<'_>, F>().collect();
    let with_rows: Vec<_> = old.iter().filter(|part| part.1 > 0).collect();
    assert_eq!(
        new.len(),
        with_rows.len(),
        "one entry an archetype with rows"
    );
    for (new, old) in new.iter().zip(&with_rows) {
        assert!(new.0 > 0, "no entry for an archetype with no rows");
        assert_eq!(new.0, old.1);
        assert!(
            std::ptr::eq(new.1.0, old.2.0),
            "the archetype's own entities"
        );
        assert!(std::ptr::eq(new.1.1, old.2.1));
        assert_eq!(
            new.1.2.map(<[Regen]>::as_ptr),
            old.2.2.map(<[Regen]>::as_ptr)
        );
    }
    let expected = read_rows(old.iter().map(|part| (part.1, part.2)));
    assert_eq!(read_rows(new), expected, "the slices form's rows");
    let rows: Vec<ReadRow> = world
        .query_filtered::<Read<'_>, F>()
        .map(|(e, p, r)| (e, *p, r.copied()))
        .collect();
    assert_eq!(rows, expected, "the row form's rows");

    let first = old.iter().position(|part| part.1 > 0);
    let last = old.iter().rposition(|part| part.1 > 0);
    if let (Some(first), Some(last)) = (first, last) {
        seen.skipped_between += old[first..last].iter().filter(|part| part.1 == 0).count();
        seen.top = seen.top.max(old[last].0);
    }
    let ending_at_an_empty = read_rows(
        old.iter()
            .take_while(|part| part.1 > 0)
            .map(|part| (part.1, part.2)),
    );
    seen.ending_at_an_empty_differs += usize::from(ending_at_an_empty != expected);
    let first_word_alone = read_rows(
        old.iter()
            .filter(|part| part.0 < 64)
            .map(|part| (part.1, part.2)),
    );
    seen.first_word_alone_differs += usize::from(first_word_alone != expected);
}

/// One order-dependent write through the old collection of `old` and through
/// the mutable slices form of `new`, under `F`: the entries' row counts and
/// the Worlds they leave are equal.
fn check_writes<F: QueryFilter>(old: &mut World, new: &mut World, stamp: f32) {
    let mut old_rows = Vec::new();
    let mut ordinal = stamp;
    for (_, rows, slices) in every_archetype_mut::<Write<'_>, F>(old) {
        if rows > 0 {
            old_rows.push(rows);
        }
        for (_, health, stats) in <Write<'_> as QueryData<'_>>::iter(slices, rows) {
            ordinal += 1.0;
            health.0 = ordinal;
            if let Some(stats) = stats {
                stats.0[1] = ordinal;
            }
        }
    }
    let mut new_rows = Vec::new();
    let mut ordinal = stamp;
    for (rows, slices) in new.query_mut_slices_filtered::<Write<'_>, F>() {
        new_rows.push(rows);
        for (_, health, stats) in <Write<'_> as QueryData<'_>>::iter(slices, rows) {
            ordinal += 1.0;
            health.0 = ordinal;
            if let Some(stats) = stats {
                stats.0[1] = ordinal;
            }
        }
    }
    assert_eq!(new_rows, old_rows);
    assert_eq!(whole(new), whole(old));
}

#[test]
fn a_collection_gives_the_rows_a_walk_of_every_archetype_does() {
    let mut seen = Seen::default();
    let mut refilled = 0;
    for seed in [1_u64, 7, 20_261_121] {
        // Twin Worlds: `old` takes its writes through the old collection,
        // `new` through the queries, and each round's structural changes are
        // the same on both.
        let mut old = Churn::new(seed);
        let mut new = Churn::new(seed);
        for round in 0..120_u32 {
            old.round();
            new.round();
            assert_eq!(whole(&new.world), whole(&old.world));

            check_reads::<()>(&new.world, &mut seen);
            check_reads::<Without<Follow>>(&new.world, &mut seen);
            check_reads::<(With<Vel>, Without<Tag<0>>)>(&new.world, &mut seen);
            // The plain forms are the filtered ones under `()`.
            let plain = read_rows(new.world.query_slices::<Read<'_>>());
            let by_lookup: Vec<ReadRow> = whole(&new.world)
                .into_iter()
                .filter_map(|row| Some((row.0, row.1?, row.4)))
                .collect();
            assert_eq!(plain, by_lookup, "against per-entity lookups");

            let stamp = (round * 1_000) as f32;
            check_writes::<()>(&mut old.world, &mut new.world, stamp);
            check_writes::<Without<Follow>>(&mut old.world, &mut new.world, stamp + 0.25);
            check_writes::<(With<Vel>, Without<Tag<0>>)>(
                &mut old.world,
                &mut new.world,
                stamp + 0.5,
            );
            // The row forms and the unfiltered slices form, against the old
            // collection's writes.
            for (_, rows, slices) in every_archetype_mut::<Write<'_>, ()>(&mut old.world) {
                for (_, health, _) in <Write<'_> as QueryData<'_>>::iter(slices, rows) {
                    health.0 += 3.0;
                }
            }
            for (_, health, _) in new.world.query_mut::<Write<'_>>() {
                health.0 += 1.0;
            }
            for (_, (_, health, _)) in new.world.query_mut_slices::<Write<'_>>() {
                for health in health {
                    health.0 += 1.0;
                }
            }
            for (_, health, _) in new.world.query_mut_filtered::<Write<'_>, ()>() {
                health.0 += 1.0;
            }
            assert_eq!(whole(&new.world), whole(&old.world));
        }
        refilled += new.refilled;

        // With every row gone, each form reads nothing from the archetypes
        // its shape still matches.
        new.clear();
        assert!(!every_archetype::<Read<'_>, ()>(&new.world).is_empty());
        assert_eq!(new.world.query_slices::<Read<'_>>().count(), 0);
        assert_eq!(new.world.query::<Read<'_>>().count(), 0);
        assert_eq!(new.world.query_mut_slices::<Write<'_>>().count(), 0);
        assert_eq!(new.world.query_mut::<Write<'_>>().count(), 0);
        assert_eq!(
            new.world
                .query_mut_slices_filtered::<Write<'_>, Without<Follow>>()
                .count(),
            0
        );
    }
    // The scene held the cases a wrong walk would lose: empty archetypes
    // between ones with rows, matches past the bits' first two words,
    // archetypes emptied and filled again, and rounds where a walk that
    // drops either case reads other rows.
    assert!(seen.skipped_between > 1_000, "{}", seen.skipped_between);
    assert!(seen.top >= 128, "{}", seen.top);
    assert!(refilled > 100, "{refilled}");
    assert!(seen.ending_at_an_empty_differs > 300);
    assert!(seen.first_word_alone_differs > 300);
}
