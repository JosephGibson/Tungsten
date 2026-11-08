//! A seeded random-sequence oracle for the World and its command buffers:
//! each operation runs on a `World` and on a plain model (live entity to
//! component kind to value, the dead handles beside it), and after each one
//! the two agree on every live entity's components, on every dead handle
//! (never live again, and a reused index under a new generation), on the
//! queries, the counters and the live values of a drop-counted component.
//!
//! The generator is the module's own splitmix64: no new crate. A failing
//! sequence shrinks to the smallest one that fails the same way and the test
//! panics with it. `TUNGSTEN_ORACLE_SEED=<n>` runs one seed and
//! `TUNGSTEN_ORACLE_SEEDS=<count>` sets the count; the `#[ignore]` long mode
//! runs other seeds, longer.
//!
//! [`KNOWN`] lists what the generator leaves out because a known issue
//! explains it; [`DESIGN`] what it leaves out because the API refuses it.

use std::any::TypeId;
use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fmt::Write as _;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

use super::{Bundle, CommandBuffer, Entity, PendingEntity, With, Without, World};

/// A case the generators skip and the row that explains it.
pub(crate) struct Known {
    pub(crate) id: &'static str,
    /// The row: a file and line at cad63fa5, or a register row.
    pub(crate) row: &'static str,
    /// Words of a `docs/known-issues.md` row, which
    /// `each_known_row_is_still_open` finds there while the row is open.
    pub(crate) words: Option<&'static str>,
    /// The case left out, and how.
    pub(crate) case: &'static str,
}

/// The cases the generators skip because a known issue explains them
/// (`notes.md` in `perf-runs/20261107-core-hardening/oracle-ecs/`).
pub(crate) const KNOWN: &[Known] = &[
    Known {
        id: "F7-kept",
        row: "docs/known-issues.md:23 (Core F7, remainder)",
        words: Some("Core F7, remainder"),
        case: "a PendingEntity recorded after its buffer's flush: each buffer's handles drop at its flush",
    },
    Known {
        id: "F7-past",
        row: "docs/known-issues.md:23 (Core F7, remainder)",
        words: Some("one past the buffer's spawns"),
        case: "a PendingEntity past the buffer's spawns: handles come only from the buffer's own spawn calls",
    },
    Known {
        id: "F7-other",
        row: "docs/known-issues.md:23 (Core F7, remainder)",
        words: Some("a handle from another buffer"),
        case: "a PendingEntity of another buffer: two buffers record at once, each with its own handles only",
    },
    Known {
        id: "S-59",
        row: "docs/plans/seam-register.md S-59 (the physics step's order and solver cache)",
        words: None,
        case: "a physics step after a restore: the save oracle builds no PhysicsPlugin and never steps",
    },
    Known {
        id: "gone-named",
        row: "docs/known-issues.md:29 (restore_world, plan_area)",
        words: Some("both in an area's `named` records and its `gone` list"),
        case: "a save naming an entity both in `named` and `gone`: every restored save is a capture",
    },
];

/// The cases the generators skip because the API documents them as refused
/// or lossy, not as issues.
pub(crate) const DESIGN: &[(&str, &str, &str)] = &[
    (
        "D-022",
        "ecs/storage.rs `insert` (D-022)",
        "a direct insert or insert_bundle on a dead entity panics by design: direct inserts target live entities",
    ),
    (
        "patch-dash",
        "save/patch.rs module docs",
        "a saved value writing a key `-` of its own cannot be patched apart from it: map keys avoid `-`",
    ),
    (
        "dropped-ref",
        "save/world.rs `WorldSave::dropped_refs`",
        "a reference to a dead or unsaved entity drops at the capture: such targets are not compared",
    ),
    (
        "run-start-names",
        "save/id.rs `name`",
        "names are given at the run's start only: the level builder names, the run never does",
    ),
    (
        "finite-floats",
        "serde_json",
        "a non-finite float writes `null`: saved floats are finite",
    ),
];

/// The row of [`KNOWN`] a failure's text matches, if any.
pub(crate) fn known_of(text: &str) -> Option<&'static str> {
    if text.contains("a handle is valid only for its own buffer") {
        Some("F7-kept or F7-past")
    } else if text.contains("insert on dead entity") && text.contains("restore") {
        Some("gone-named")
    } else {
        None
    }
}

// ---------------------------------------------------------------- generator

/// splitmix64.
pub(crate) struct Rng(u64);

impl Rng {
    pub(crate) fn new(seed: u64) -> Self {
        Self(seed.wrapping_mul(0x2545_f491_4f6c_dd1d) ^ 0x9e37_79b9_7f4a_7c15)
    }

    pub(crate) fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// Uniform below `n`; 0 for `n` 0.
    pub(crate) fn below(&mut self, n: u64) -> u64 {
        if n == 0 { 0 } else { self.next_u64() % n }
    }

    pub(crate) fn small(&mut self) -> u32 {
        self.below(1 << 16) as u32
    }

    pub(crate) fn chance(&mut self, percent: u64) -> bool {
        self.below(100) < percent
    }

    /// An index by weight.
    pub(crate) fn weighted(&mut self, weights: &[u32]) -> usize {
        let total: u64 = weights.iter().map(|&w| u64::from(w)).sum();
        let mut roll = self.below(total);
        for (index, &weight) in weights.iter().enumerate() {
            if roll < u64::from(weight) {
                return index;
            }
            roll -= u64::from(weight);
        }
        weights.len() - 1
    }
}

/// The seeds a run takes: `TUNGSTEN_ORACLE_SEED` alone, else
/// `TUNGSTEN_ORACLE_SEEDS` (or `count`) seeds from `first`.
pub(crate) fn seeds(first: u64, count: u64) -> Vec<u64> {
    if let Ok(one) = std::env::var("TUNGSTEN_ORACLE_SEED") {
        return vec![
            one.trim()
                .parse()
                .expect("TUNGSTEN_ORACLE_SEED is a number"),
        ];
    }
    let count = std::env::var("TUNGSTEN_ORACLE_SEEDS")
        .ok()
        .and_then(|count| count.trim().parse().ok())
        .unwrap_or(count);
    (first..first + count).collect()
}

/// Part `part` of `parts` contiguous parts of [`seeds`]' run, so the default
/// runs split into tests the test harness's threads run side by side.
pub(crate) fn seed_part(first: u64, count: u64, part: usize, parts: usize) -> Vec<u64> {
    let all = seeds(first, count);
    let len = all.len();
    all[len * part / parts..len * (part + 1) / parts].to_vec()
}

/// A failed check: a tag that names what disagreed, and the detail.
#[derive(Debug, Clone)]
pub(crate) struct Fail {
    pub(crate) tag: String,
    pub(crate) detail: String,
    pub(crate) step: usize,
}

pub(crate) fn fail<T>(tag: &str, detail: impl Into<String>) -> Result<T, Fail> {
    Err(Fail {
        tag: tag.to_owned(),
        detail: detail.into(),
        step: 0,
    })
}

/// A panic's message.
pub(crate) fn panic_text(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(text) = payload.downcast_ref::<&str>() {
        (*text).to_owned()
    } else if let Some(text) = payload.downcast_ref::<String>() {
        text.clone()
    } else {
        "a panic without a message".to_owned()
    }
}

/// Runs `run` and turns a panic into a [`Fail`] tagged by its message's
/// first words.
pub(crate) fn guarded(run: impl FnOnce() -> Result<(), Fail>) -> Result<(), Fail> {
    match catch_unwind(AssertUnwindSafe(run)) {
        Ok(result) => result,
        Err(payload) => {
            let text = panic_text(&*payload);
            let tag: String = text
                .split(|c: char| !c.is_ascii_alphabetic() && c != ' ')
                .next()
                .unwrap_or("")
                .chars()
                .take(48)
                .collect();
            fail(&format!("panic: {tag}"), text)
        }
    }
}

/// The smallest sequence (by chunk removal, then single ops) that still
/// fails with `tag`, within `budget` runs.
pub(crate) fn shrink<O: Clone>(
    mut ops: Vec<O>,
    tag: &str,
    mut budget: usize,
    run: impl Fn(&[O]) -> Result<(), Fail>,
) -> (Vec<O>, Fail) {
    let mut last = run(&ops).expect_err("a shrink starts from a failing sequence");
    let mut chunk = ops.len().div_ceil(2).max(1);
    loop {
        let mut progress = false;
        let mut start = 0;
        while start < ops.len() && budget > 0 {
            let end = (start + chunk).min(ops.len());
            let mut candidate = ops.clone();
            candidate.drain(start..end);
            budget -= 1;
            match run(&candidate) {
                Err(err) if err.tag == tag => {
                    ops = candidate;
                    last = err;
                    progress = true;
                }
                _ => start += chunk,
            }
        }
        if budget == 0 || (chunk == 1 && !progress) {
            break;
        }
        if !progress {
            chunk = chunk.div_ceil(2);
        }
    }
    (ops, last)
}

/// One shrunk failure of a seed.
pub(crate) struct Found<O> {
    pub(crate) seed: u64,
    pub(crate) ops: Vec<O>,
    pub(crate) fail: Fail,
    pub(crate) known: Option<&'static str>,
}

/// Runs each seed's sequence, shrinks each first failure of a new tag, and
/// panics listing them all.
pub(crate) fn drive<O: Clone + std::fmt::Debug>(
    name: &str,
    seeds: &[u64],
    generate: impl Fn(u64) -> Vec<O>,
    run: impl Fn(&[O]) -> Result<(), Fail>,
) {
    let mut found: Vec<Found<O>> = Vec::new();
    let mut tags = BTreeSet::new();
    let mut failing_seeds = 0;
    for &seed in seeds {
        let ops = generate(seed);
        let Err(err) = run(&ops) else {
            continue;
        };
        failing_seeds += 1;
        if !tags.insert(err.tag.clone()) || found.len() >= 8 {
            continue;
        }
        let (ops, fail) = shrink(ops, &err.tag, 4_000, &run);
        let known = known_of(&fail.detail);
        found.push(Found {
            seed,
            ops,
            fail,
            known,
        });
    }
    if found.is_empty() {
        return;
    }
    let mut report = format!(
        "{name}: {failing_seeds} of {} seeds failed, {} distinct\n",
        seeds.len(),
        found.len()
    );
    for one in &found {
        let _ = writeln!(
            report,
            "\nseed {} [{}] ({}), at op {} of {}: {}\n  ops: {:?}",
            one.seed,
            one.fail.tag,
            one.known.unwrap_or("new"),
            one.fail.step,
            one.ops.len(),
            one.fail.detail,
            one.ops
        );
    }
    panic!("{report}");
}

// ------------------------------------------------------------ the components

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Kind {
    A,
    B,
    C,
    D,
    T,
}

const KINDS: [Kind; 5] = [Kind::A, Kind::B, Kind::C, Kind::D, Kind::T];

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Val {
    A(u32),
    B(String),
    C,
    D([u64; 3]),
    T(u64),
}

impl Val {
    fn kind(&self) -> Kind {
        match self {
            Val::A(_) => Kind::A,
            Val::B(_) => Kind::B,
            Val::C => Kind::C,
            Val::D(_) => Kind::D,
            Val::T(_) => Kind::T,
        }
    }

    fn random(kind: Kind, rng: &mut Rng) -> Self {
        match kind {
            Kind::A => Val::A(rng.small()),
            Kind::B => Val::B(format!("b{}", rng.below(1000))),
            Kind::C => Val::C,
            Kind::D => Val::D([rng.next_u64(), rng.below(9), rng.next_u64()]),
            Kind::T => Val::T(rng.below(1 << 20)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct A(u32);

#[derive(Debug, Clone, PartialEq, Eq)]
struct B(String);

/// Zero-sized.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct C;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct D([u64; 3]);

/// Drop-counted: `live` counts the values alive anywhere, in the World or
/// queued in a buffer.
#[derive(Debug)]
struct T {
    value: u64,
    live: Rc<Cell<i64>>,
}

impl T {
    fn new(value: u64, live: &Rc<Cell<i64>>) -> Self {
        live.set(live.get() + 1);
        Self {
            value,
            live: Rc::clone(live),
        }
    }
}

impl Drop for T {
    fn drop(&mut self) {
        self.live.set(self.live.get() - 1);
    }
}

fn type_of(kind: Kind) -> TypeId {
    match kind {
        Kind::A => TypeId::of::<A>(),
        Kind::B => TypeId::of::<B>(),
        Kind::C => TypeId::of::<C>(),
        Kind::D => TypeId::of::<D>(),
        Kind::T => TypeId::of::<T>(),
    }
}

fn get_val(world: &World, entity: Entity, kind: Kind) -> Option<Val> {
    match kind {
        Kind::A => world.get::<A>(entity).map(|a| Val::A(a.0)),
        Kind::B => world.get::<B>(entity).map(|b| Val::B(b.0.clone())),
        Kind::C => world.get::<C>(entity).map(|_| Val::C),
        Kind::D => world.get::<D>(entity).map(|d| Val::D(d.0)),
        Kind::T => world.get::<T>(entity).map(|t| Val::T(t.value)),
    }
}

fn has_kind(world: &World, entity: Entity, kind: Kind) -> bool {
    match kind {
        Kind::A => world.has::<A>(entity),
        Kind::B => world.has::<B>(entity),
        Kind::C => world.has::<C>(entity),
        Kind::D => world.has::<D>(entity),
        Kind::T => world.has::<T>(entity),
    }
}

fn has_any_kind(world: &World, kind: Kind) -> bool {
    match kind {
        Kind::A => world.has_any::<A>(),
        Kind::B => world.has_any::<B>(),
        Kind::C => world.has_any::<C>(),
        Kind::D => world.has_any::<D>(),
        Kind::T => world.has_any::<T>(),
    }
}

fn insert_val(world: &mut World, entity: Entity, val: &Val, live: &Rc<Cell<i64>>) {
    match val {
        Val::A(x) => world.insert(entity, A(*x)),
        Val::B(x) => world.insert(entity, B(x.clone())),
        Val::C => world.insert(entity, C),
        Val::D(x) => world.insert(entity, D(*x)),
        Val::T(x) => world.insert(entity, T::new(*x, live)),
    }
}

fn remove_val(world: &mut World, entity: Entity, kind: Kind) -> Option<Val> {
    match kind {
        Kind::A => world.remove_component::<A>(entity).map(|a| Val::A(a.0)),
        Kind::B => world.remove_component::<B>(entity).map(|b| Val::B(b.0)),
        Kind::C => world.remove_component::<C>(entity).map(|_| Val::C),
        Kind::D => world.remove_component::<D>(entity).map(|d| Val::D(d.0)),
        Kind::T => world.remove_component::<T>(entity).map(|t| Val::T(t.value)),
    }
}

/// The write `get_mut` makes, on the model's value.
fn mutate_val(val: &mut Val, x: u32) {
    match val {
        Val::A(a) => *a = a.wrapping_add(x),
        Val::B(b) => b.push(char::from(b'a' + (x % 26) as u8)),
        Val::C => {}
        Val::D(d) => d[(x % 3) as usize] ^= u64::from(x),
        Val::T(t) => *t = t.wrapping_add(u64::from(x)),
    }
}

/// The same write through `get_mut`; whether the World had the value.
fn mutate_world(world: &mut World, entity: Entity, kind: Kind, x: u32) -> bool {
    match kind {
        Kind::A => world
            .get_mut::<A>(entity)
            .map(|a| a.0 = a.0.wrapping_add(x)),
        Kind::B => world
            .get_mut::<B>(entity)
            .map(|b| b.0.push(char::from(b'a' + (x % 26) as u8))),
        Kind::C => world.get_mut::<C>(entity).map(|_| ()),
        Kind::D => world
            .get_mut::<D>(entity)
            .map(|d| d.0[(x % 3) as usize] ^= u64::from(x)),
        Kind::T => world
            .get_mut::<T>(entity)
            .map(|t| t.value = t.value.wrapping_add(u64::from(x))),
    }
    .is_some()
}

fn buffer_insert(buffer: &mut CommandBuffer, entity: Entity, val: &Val, live: &Rc<Cell<i64>>) {
    match val {
        Val::A(x) => buffer.insert(entity, A(*x)),
        Val::B(x) => buffer.insert(entity, B(x.clone())),
        Val::C => buffer.insert(entity, C),
        Val::D(x) => buffer.insert(entity, D(*x)),
        Val::T(x) => buffer.insert(entity, T::new(*x, live)),
    }
}

fn buffer_insert_pending(
    buffer: &mut CommandBuffer,
    pending: PendingEntity,
    val: &Val,
    live: &Rc<Cell<i64>>,
) {
    match val {
        Val::A(x) => buffer.insert_pending(pending, A(*x)),
        Val::B(x) => buffer.insert_pending(pending, B(x.clone())),
        Val::C => buffer.insert_pending(pending, C),
        Val::D(x) => buffer.insert_pending(pending, D(*x)),
        Val::T(x) => buffer.insert_pending(pending, T::new(*x, live)),
    }
}

fn buffer_remove(buffer: &mut CommandBuffer, entity: Entity, kind: Kind) {
    match kind {
        Kind::A => buffer.remove_component::<A>(entity),
        Kind::B => buffer.remove_component::<B>(entity),
        Kind::C => buffer.remove_component::<C>(entity),
        Kind::D => buffer.remove_component::<D>(entity),
        Kind::T => buffer.remove_component::<T>(entity),
    }
}

/// A recorded call: `A` plus one where the entity has one.
fn bump_a(world: &mut World, entity: Entity) {
    if let Some(a) = world.get_mut::<A>(entity) {
        a.0 = a.0.wrapping_add(1);
    }
}

/// A recorded call that despawns every entity holding `C`, pending spawns
/// of the same flush included.
fn purge_c(world: &mut World, _: Entity) {
    let doomed: Vec<Entity> = world.query_filtered::<Entity, With<C>>().collect();
    for entity in doomed {
        world.despawn(entity);
    }
}

// ------------------------------------------------------------------ bundles

/// Each bundle shape's kinds, in insertion order: duplicates and chains
/// included, so a later value of a kind wins.
const SHAPES: &[&[Kind]] = &[
    &[Kind::A],
    &[Kind::A, Kind::B],
    &[Kind::B, Kind::C, Kind::A],
    &[Kind::D, Kind::T],
    &[Kind::A, Kind::B, Kind::C, Kind::D, Kind::T],
    &[Kind::A, Kind::A],
    &[Kind::T, Kind::B, Kind::T],
    &[Kind::A, Kind::B, Kind::C],
    &[Kind::D, Kind::D, Kind::A],
    &[Kind::C],
    &[Kind::T],
];

fn shape_vals(shape: u8, rng: &mut Rng) -> Vec<Val> {
    SHAPES[shape as usize]
        .iter()
        .map(|&kind| Val::random(kind, rng))
        .collect()
}

/// Where a bundle goes.
trait Sink {
    type Out;
    fn take<Bn: Bundle>(self, bundle: Bn) -> Self::Out;
}

/// The bundle of `shape` from `vals`, handed to `sink`.
fn bundle<S: Sink>(shape: u8, vals: &[Val], live: &Rc<Cell<i64>>, sink: S) -> S::Out {
    let a = |i: usize| match &vals[i] {
        Val::A(x) => A(*x),
        other => panic!("shape {shape}: {other:?} is no A"),
    };
    let b = |i: usize| match &vals[i] {
        Val::B(x) => B(x.clone()),
        other => panic!("shape {shape}: {other:?} is no B"),
    };
    let d = |i: usize| match &vals[i] {
        Val::D(x) => D(*x),
        other => panic!("shape {shape}: {other:?} is no D"),
    };
    let t = |i: usize| match &vals[i] {
        Val::T(x) => T::new(*x, live),
        other => panic!("shape {shape}: {other:?} is no T"),
    };
    match shape {
        0 => sink.take((a(0),)),
        1 => sink.take((a(0), b(1))),
        2 => sink.take((b(0), C, a(2))),
        3 => sink.take((d(0), t(1))),
        4 => sink.take((a(0), b(1), C, d(3), t(4))),
        5 => sink.take((a(0), a(1))),
        6 => sink.take((t(0), b(1), t(2))),
        7 => sink.take((a(0), b(1)).with((C,))),
        8 => sink.take((d(0),).with((d(1), a(2)))),
        9 => sink.take((C,)),
        10 => sink.take((t(0),)),
        _ => unreachable!("shape {shape}"),
    }
}

struct SpawnWith<'a>(&'a mut World);

impl Sink for SpawnWith<'_> {
    type Out = Entity;
    fn take<Bn: Bundle>(self, bundle: Bn) -> Entity {
        self.0.spawn_with(bundle)
    }
}

struct InsertBundle<'a>(&'a mut World, Entity);

impl Sink for InsertBundle<'_> {
    type Out = ();
    fn take<Bn: Bundle>(self, bundle: Bn) {
        self.0.insert_bundle(self.1, bundle);
    }
}

struct BufSpawnWith<'a>(&'a mut CommandBuffer);

impl Sink for BufSpawnWith<'_> {
    type Out = PendingEntity;
    fn take<Bn: Bundle>(self, bundle: Bn) -> PendingEntity {
        self.0.spawn_with(bundle)
    }
}

struct BufInsertBundle<'a>(&'a mut CommandBuffer, Entity);

impl Sink for BufInsertBundle<'_> {
    type Out = ();
    fn take<Bn: Bundle>(self, bundle: Bn) {
        self.0.insert_bundle(self.1, bundle);
    }
}

struct BufInsertBundlePending<'a>(&'a mut CommandBuffer, PendingEntity);

impl Sink for BufInsertBundlePending<'_> {
    type Out = ();
    fn take<Bn: Bundle>(self, bundle: Bn) {
        self.0.insert_bundle_pending(self.1, bundle);
    }
}

// --------------------------------------------------------------- operations

/// An entity an operation names, resolved against the model when it runs:
/// a live one, a dead one, a live index under the next generation, or an
/// index never allocated.
#[derive(Clone, Copy, Debug)]
enum Pick {
    Live(u32),
    Dead(u32),
    Future(u32),
    Beyond(u32),
}

#[derive(Clone, Debug)]
enum Op {
    Spawn,
    SpawnWith(u8, Vec<Val>),
    /// On a live entity (`D-022`).
    InsertBundle(u32, u8, Vec<Val>),
    /// On a live entity (`D-022`): a new kind or a replacement.
    Insert(u32, Val),
    Despawn(Pick),
    Remove(Pick, Kind),
    Get(Pick, Kind),
    Write(Pick, Kind, u32),
    Query(u8),
    QueryMut(u8),
    Record(u8, Rec),
    Flush(u8, bool),
}

#[derive(Clone, Debug)]
enum Rec {
    Spawn,
    SpawnWith(u8, Vec<Val>),
    InsertPending(u32, Val),
    InsertBundlePending(u32, u8, Vec<Val>),
    Insert(Pick, Val),
    InsertBundle(Pick, u8, Vec<Val>),
    Remove(Pick, Kind),
    Despawn(Pick),
    Bump(Pick),
    PurgeC,
}

const READ_QUERIES: u8 = 13;
const MUT_QUERIES: u8 = 5;

fn pick(rng: &mut Rng) -> Pick {
    let k = rng.small();
    match rng.weighted(&[80, 12, 4, 4]) {
        0 => Pick::Live(k),
        1 => Pick::Dead(k),
        2 => Pick::Future(k),
        _ => Pick::Beyond(k),
    }
}

fn kind(rng: &mut Rng) -> Kind {
    KINDS[rng.below(KINDS.len() as u64) as usize]
}

fn shape(rng: &mut Rng) -> u8 {
    rng.below(SHAPES.len() as u64) as u8
}

fn generate(seed: u64, len: usize) -> Vec<Op> {
    let mut rng = Rng::new(seed);
    // Each seed leans its own way: some churn, some grow.
    let despawn_weight = 2 + rng.below(10) as u32;
    let record_weight = 4 + rng.below(30) as u32;
    (0..len)
        .map(|_| {
            match rng.weighted(&[6, 8, 5, 10, despawn_weight, 6, 4, 6, 6, 3, record_weight, 3]) {
                0 => Op::Spawn,
                1 => {
                    let s = shape(&mut rng);
                    Op::SpawnWith(s, shape_vals(s, &mut rng))
                }
                2 => {
                    let s = shape(&mut rng);
                    Op::InsertBundle(rng.small(), s, shape_vals(s, &mut rng))
                }
                3 => {
                    let k = kind(&mut rng);
                    Op::Insert(rng.small(), Val::random(k, &mut rng))
                }
                4 => Op::Despawn(pick(&mut rng)),
                5 => Op::Remove(pick(&mut rng), kind(&mut rng)),
                6 => Op::Get(pick(&mut rng), kind(&mut rng)),
                7 => Op::Write(pick(&mut rng), kind(&mut rng), rng.small()),
                8 => Op::Query(rng.below(u64::from(READ_QUERIES)) as u8),
                9 => Op::QueryMut(rng.below(u64::from(MUT_QUERIES)) as u8),
                10 => {
                    let buf = rng.below(2) as u8;
                    let rec = match rng.weighted(&[6, 6, 8, 4, 8, 4, 4, 4, 3, 1]) {
                        0 => Rec::Spawn,
                        1 => {
                            let s = shape(&mut rng);
                            Rec::SpawnWith(s, shape_vals(s, &mut rng))
                        }
                        2 => {
                            let k = kind(&mut rng);
                            Rec::InsertPending(rng.small(), Val::random(k, &mut rng))
                        }
                        3 => {
                            let s = shape(&mut rng);
                            Rec::InsertBundlePending(rng.small(), s, shape_vals(s, &mut rng))
                        }
                        4 => {
                            let k = kind(&mut rng);
                            Rec::Insert(pick(&mut rng), Val::random(k, &mut rng))
                        }
                        5 => {
                            let s = shape(&mut rng);
                            Rec::InsertBundle(pick(&mut rng), s, shape_vals(s, &mut rng))
                        }
                        6 => Rec::Remove(pick(&mut rng), kind(&mut rng)),
                        7 => Rec::Despawn(pick(&mut rng)),
                        8 => Rec::Bump(pick(&mut rng)),
                        _ => Rec::PurgeC,
                    };
                    Op::Record(buf, rec)
                }
                _ => Op::Flush(rng.below(2) as u8, rng.chance(30)),
            }
        })
        .collect()
}

// -------------------------------------------------------------------- model

type Comps = BTreeMap<Kind, Val>;

fn key(entity: Entity) -> (u32, u32) {
    (entity.index, entity.generation)
}

fn entity_at((index, generation): (u32, u32)) -> Entity {
    Entity { index, generation }
}

#[derive(Clone, Debug)]
enum Target {
    Live(Entity),
    Pending(usize),
}

#[derive(Clone, Debug)]
enum Cmd {
    Insert(Target, Val),
    Remove(Entity, Kind),
    Despawn(Entity),
    Bump(Entity),
    PurgeC,
}

#[derive(Default)]
struct ModelBuffer {
    pending: Vec<PendingEntity>,
    cmds: Vec<Cmd>,
    queued_t: i64,
}

#[derive(Default)]
struct Model {
    live: BTreeMap<(u32, u32), Comps>,
    dead: Vec<Entity>,
    dead_set: HashSet<(u32, u32)>,
    spawned: u64,
    despawned: u64,
    buffers: [ModelBuffer; 2],
}

impl Model {
    fn nth_live(&self, k: u32) -> Option<Entity> {
        if self.live.is_empty() {
            return None;
        }
        let n = k as usize % self.live.len();
        self.live.keys().nth(n).copied().map(entity_at)
    }

    fn resolve(&self, pick: Pick) -> Entity {
        let beyond = |k: u32| Entity {
            index: 1_000_000 + k,
            generation: 0,
        };
        match pick {
            Pick::Live(k) => self.nth_live(k).unwrap_or_else(|| beyond(k)),
            Pick::Dead(k) if !self.dead.is_empty() => self.dead[k as usize % self.dead.len()],
            Pick::Future(k) => self.nth_live(k).map_or_else(
                || beyond(k),
                |e| Entity {
                    index: e.index,
                    generation: e.generation.wrapping_add(1),
                },
            ),
            Pick::Dead(k) | Pick::Beyond(k) => beyond(k),
        }
    }

    fn is_live(&self, entity: Entity) -> bool {
        self.live.contains_key(&key(entity))
    }

    /// A spawn the World allocated: never a live or dead handle before.
    fn born(&mut self, entity: Entity) -> Result<(), Fail> {
        if self.is_live(entity) {
            return fail("spawn-live", format!("spawn returned live {entity:?}"));
        }
        if self.dead_set.contains(&key(entity)) {
            return fail(
                "spawn-reused-generation",
                format!(
                    "spawn returned {entity:?}, a dead handle: its index's generation did not move"
                ),
            );
        }
        self.live.insert(key(entity), Comps::new());
        self.spawned += 1;
        Ok(())
    }

    fn kill(&mut self, entity: Entity) {
        if self.live.remove(&key(entity)).is_some() {
            self.dead.push(entity);
            self.dead_set.insert(key(entity));
            self.despawned += 1;
        }
    }

    fn set(&mut self, entity: Entity, val: Val) -> bool {
        match self.live.get_mut(&key(entity)) {
            Some(comps) => {
                comps.insert(val.kind(), val);
                true
            }
            None => false,
        }
    }

    fn live_t(&self) -> i64 {
        let held = self
            .live
            .values()
            .filter(|comps| comps.contains_key(&Kind::T))
            .count() as i64;
        held + self.buffers.iter().map(|b| b.queued_t).sum::<i64>()
    }
}

fn t_count(vals: &[Val]) -> i64 {
    vals.iter().filter(|v| v.kind() == Kind::T).count() as i64
}

// --------------------------------------------------------------- the queries

/// One row: the entity (none for a query without one) and each column.
type Row = (Option<(u32, u32)>, Vec<Option<Val>>);

/// What a query reads, for the model: its columns (kind, required), and
/// its filters.
struct Spec {
    entity: bool,
    cols: &'static [(Kind, bool)],
    with: &'static [Kind],
    without: &'static [Kind],
}

const READ_SPECS: [Spec; READ_QUERIES as usize] = [
    Spec {
        entity: true,
        cols: &[],
        with: &[],
        without: &[],
    },
    Spec {
        entity: true,
        cols: &[(Kind::A, true)],
        with: &[],
        without: &[],
    },
    Spec {
        entity: true,
        cols: &[(Kind::A, true), (Kind::B, true)],
        with: &[],
        without: &[],
    },
    Spec {
        entity: true,
        cols: &[(Kind::B, true), (Kind::D, false), (Kind::T, true)],
        with: &[],
        without: &[],
    },
    Spec {
        entity: true,
        cols: &[(Kind::A, true)],
        with: &[Kind::C],
        without: &[],
    },
    Spec {
        entity: true,
        cols: &[(Kind::B, true)],
        with: &[],
        without: &[Kind::A],
    },
    Spec {
        entity: true,
        cols: &[(Kind::A, true), (Kind::D, true)],
        with: &[Kind::B],
        without: &[Kind::C],
    },
    Spec {
        entity: true,
        cols: &[(Kind::A, false), (Kind::C, false)],
        with: &[],
        without: &[],
    },
    Spec {
        entity: true,
        cols: &[(Kind::C, true)],
        with: &[],
        without: &[Kind::T],
    },
    Spec {
        entity: true,
        cols: &[(Kind::B, false)],
        with: &[Kind::D, Kind::A],
        without: &[Kind::C],
    },
    Spec {
        entity: false,
        cols: &[(Kind::A, true)],
        with: &[],
        without: &[],
    },
    Spec {
        entity: true,
        cols: &[(Kind::A, true)],
        with: &[],
        without: &[],
    },
    Spec {
        entity: true,
        cols: &[],
        with: &[],
        without: &[Kind::A, Kind::B, Kind::C, Kind::D],
    },
];

const MUT_SPECS: [Spec; MUT_QUERIES as usize] = [
    Spec {
        entity: true,
        cols: &[(Kind::A, true)],
        with: &[],
        without: &[],
    },
    Spec {
        entity: true,
        cols: &[(Kind::B, true), (Kind::A, true)],
        with: &[],
        without: &[Kind::C],
    },
    Spec {
        entity: true,
        cols: &[(Kind::T, true), (Kind::D, false), (Kind::C, true)],
        with: &[],
        without: &[],
    },
    Spec {
        entity: true,
        cols: &[(Kind::A, false)],
        with: &[Kind::D],
        without: &[],
    },
    Spec {
        entity: true,
        cols: &[(Kind::A, true)],
        with: &[],
        without: &[],
    },
];

fn model_rows(model: &Model, spec: &Spec) -> Vec<Row> {
    let mut rows: Vec<Row> = model
        .live
        .iter()
        .filter(|(_, comps)| {
            spec.cols
                .iter()
                .all(|&(kind, required)| !required || comps.contains_key(&kind))
                && spec.with.iter().all(|kind| comps.contains_key(kind))
                && spec.without.iter().all(|kind| !comps.contains_key(kind))
        })
        .map(|(&k, comps)| {
            (
                spec.entity.then_some(k),
                spec.cols
                    .iter()
                    .map(|(kind, _)| comps.get(kind).cloned())
                    .collect(),
            )
        })
        .collect();
    rows.sort();
    rows
}

fn world_rows(world: &World, query: u8) -> Result<Vec<Row>, Fail> {
    let of_a = |x: &A| Some(Val::A(x.0));
    let of_b = |x: &B| Some(Val::B(x.0.clone()));
    let of_c = |_: &C| Some(Val::C);
    let of_d = |x: &D| Some(Val::D(x.0));
    let of_t = |x: &T| Some(Val::T(x.value));
    let rows: Vec<Row> = match query {
        0 => world
            .query::<Entity>()
            .map(|x| (Some(key(x)), vec![]))
            .collect(),
        1 => world
            .query::<(Entity, &A)>()
            .map(|(x, va)| (Some(key(x)), vec![of_a(va)]))
            .collect(),
        2 => world
            .query::<(Entity, &A, &B)>()
            .map(|(x, va, vb)| (Some(key(x)), vec![of_a(va), of_b(vb)]))
            .collect(),
        3 => world
            .query::<(Entity, &B, Option<&D>, &T)>()
            .map(|(x, vb, vd, vt)| (Some(key(x)), vec![of_b(vb), vd.and_then(of_d), of_t(vt)]))
            .collect(),
        4 => world
            .query_filtered::<(Entity, &A), With<C>>()
            .map(|(x, va)| (Some(key(x)), vec![of_a(va)]))
            .collect(),
        5 => world
            .query_filtered::<(Entity, &B), Without<A>>()
            .map(|(x, vb)| (Some(key(x)), vec![of_b(vb)]))
            .collect(),
        6 => world
            .query_filtered::<(Entity, &A, &D), (With<B>, Without<C>)>()
            .map(|(x, va, vd)| (Some(key(x)), vec![of_a(va), of_d(vd)]))
            .collect(),
        7 => world
            .query::<(Entity, Option<&A>, Option<&C>)>()
            .map(|(x, va, vc)| (Some(key(x)), vec![va.and_then(of_a), vc.and_then(of_c)]))
            .collect(),
        8 => world
            .query_filtered::<(Entity, &C), (Without<T>,)>()
            .map(|(x, vc)| (Some(key(x)), vec![of_c(vc)]))
            .collect(),
        9 => world
            .query_filtered::<(Entity, Option<&B>), (With<D>, With<A>, Without<C>)>()
            .map(|(x, vb)| (Some(key(x)), vec![vb.and_then(of_b)]))
            .collect(),
        10 => world
            .query::<&A>()
            .map(|va| (None, vec![of_a(va)]))
            .collect(),
        11 => {
            let mut rows = Vec::new();
            for (count, (entities, values)) in world.query_slices::<(Entity, &A)>() {
                if entities.len() != count || values.len() != count {
                    return fail(
                        "query-slices-len",
                        format!(
                            "{count} rows with {} entities and {} values",
                            entities.len(),
                            values.len()
                        ),
                    );
                }
                rows.extend(
                    entities
                        .iter()
                        .zip(values)
                        .map(|(&x, va)| (Some(key(x)), vec![of_a(va)])),
                );
            }
            rows
        }
        12 => world
            .query_filtered::<Entity, (Without<A>, Without<B>, Without<C>, Without<D>)>()
            .map(|x| (Some(key(x)), vec![]))
            .collect(),
        _ => unreachable!("query {query}"),
    };
    Ok(rows)
}

/// Runs mutable query `query` on the World, writing each row, and returns
/// the rows as read before the write.
fn world_rows_mut(world: &mut World, query: u8) -> Vec<Row> {
    let mut rows = Vec::new();
    match query {
        0 => {
            for (x, va) in world.query_mut::<(Entity, &mut A)>() {
                rows.push((Some(key(x)), vec![Some(Val::A(va.0))]));
                va.0 = va.0.wrapping_add(1);
            }
        }
        1 => {
            for (x, vb, va) in world.query_mut_filtered::<(Entity, &mut B, &A), Without<C>>() {
                rows.push((
                    Some(key(x)),
                    vec![Some(Val::B(vb.0.clone())), Some(Val::A(va.0))],
                ));
                vb.0.push('q');
            }
        }
        2 => {
            for (x, vt, vd, _) in world.query_mut::<(Entity, &mut T, Option<&mut D>, &C)>() {
                rows.push((
                    Some(key(x)),
                    vec![
                        Some(Val::T(vt.value)),
                        vd.as_ref().map(|vd| Val::D(vd.0)),
                        Some(Val::C),
                    ],
                ));
                vt.value = vt.value.wrapping_add(1);
                if let Some(vd) = vd {
                    vd.0[1] = vd.0[1].wrapping_add(1);
                }
            }
        }
        3 => {
            for (x, va) in world.query_mut_filtered::<(Entity, Option<&mut A>), With<D>>() {
                rows.push((Some(key(x)), vec![va.as_ref().map(|va| Val::A(va.0))]));
                if let Some(va) = va {
                    va.0 = va.0.wrapping_mul(3);
                }
            }
        }
        4 => {
            for (_, (values, entities)) in world.query_mut_slices::<(&mut A, Entity)>() {
                for (va, &x) in values.iter_mut().zip(entities) {
                    rows.push((Some(key(x)), vec![Some(Val::A(va.0))]));
                    va.0 = va.0.wrapping_add(7);
                }
            }
        }
        _ => unreachable!("query_mut {query}"),
    }
    rows
}

/// The write mutable query `query` makes, on the model.
fn model_write(model: &mut Model, query: u8) {
    let spec = &MUT_SPECS[query as usize];
    let keys: Vec<(u32, u32)> = model_rows(model, spec)
        .into_iter()
        .filter_map(|(k, _)| k)
        .collect();
    for k in keys {
        let comps = model.live.get_mut(&k).expect("a row's entity is live");
        match query {
            0 => {
                if let Some(Val::A(a)) = comps.get_mut(&Kind::A) {
                    *a = a.wrapping_add(1);
                }
            }
            1 => {
                if let Some(Val::B(b)) = comps.get_mut(&Kind::B) {
                    b.push('q');
                }
            }
            2 => {
                if let Some(Val::T(t)) = comps.get_mut(&Kind::T) {
                    *t = t.wrapping_add(1);
                }
                if let Some(Val::D(d)) = comps.get_mut(&Kind::D) {
                    d[1] = d[1].wrapping_add(1);
                }
            }
            3 => {
                if let Some(Val::A(a)) = comps.get_mut(&Kind::A) {
                    *a = a.wrapping_mul(3);
                }
            }
            4 => {
                if let Some(Val::A(a)) = comps.get_mut(&Kind::A) {
                    *a = a.wrapping_add(7);
                }
            }
            _ => unreachable!(),
        }
    }
}

fn compare_rows(tag: &str, mut got: Vec<Row>, want: &[Row]) -> Result<(), Fail> {
    let entities: Vec<_> = got.iter().filter_map(|(k, _)| *k).collect();
    let distinct: HashSet<_> = entities.iter().collect();
    if distinct.len() != entities.len() {
        return fail(tag, format!("a row's entity appears twice: {entities:?}"));
    }
    got.sort();
    if got != want {
        return fail(tag, format!("world rows {got:?}\n  model rows {want:?}"));
    }
    Ok(())
}

// ---------------------------------------------------------------- the check

struct Sut {
    world: World,
    buffers: [CommandBuffer; 2],
    live: Rc<Cell<i64>>,
}

/// The World and the model agree on everything the World answers.
fn check(sut: &Sut, model: &Model) -> Result<(), Fail> {
    let world = &sut.world;
    if world.entity_count() as usize != model.live.len() {
        return fail(
            "entity-count",
            format!(
                "World {} live, model {}",
                world.entity_count(),
                model.live.len()
            ),
        );
    }
    for (&k, comps) in &model.live {
        let entity = entity_at(k);
        if !world.is_alive(entity) {
            return fail("live-reads-dead", format!("{entity:?} reads dead"));
        }
        for kind in KINDS {
            let got = get_val(world, entity, kind);
            let want = comps.get(&kind).cloned();
            if got != want {
                return fail(
                    "get",
                    format!("{entity:?} {kind:?}: World {got:?}, model {want:?}"),
                );
            }
            if has_kind(world, entity, kind) != want.is_some() {
                return fail(
                    "has",
                    format!("{entity:?} {kind:?}: has disagrees with get"),
                );
            }
        }
        let types = world.component_types(entity);
        let got: BTreeSet<TypeId> = types.iter().map(|&(t, _)| t).collect();
        let want: BTreeSet<TypeId> = comps.keys().map(|&kind| type_of(kind)).collect();
        if got != want || types.len() != got.len() {
            return fail(
                "component-types",
                format!("{entity:?}: World {types:?}, model {:?}", comps.keys()),
            );
        }
        if !types.windows(2).all(|w| w[0].1 <= w[1].1) {
            return fail("component-types-order", format!("{entity:?}: {types:?}"));
        }
        // The same index under the next generation reads dead.
        let next = Entity {
            index: entity.index,
            generation: entity.generation.wrapping_add(1),
        };
        if world.is_alive(next) || get_val(world, next, Kind::A).is_some() {
            return fail("future-reads-live", format!("{next:?} reads live"));
        }
    }
    for &entity in &model.dead {
        if model.is_live(entity) {
            continue;
        }
        if world.is_alive(entity) {
            return fail("dead-reads-live", format!("dead {entity:?} reads live"));
        }
        for kind in KINDS {
            if get_val(world, entity, kind).is_some() || has_kind(world, entity, kind) {
                return fail(
                    "dead-holds",
                    format!("dead {entity:?} still reads a {kind:?}"),
                );
            }
        }
        if !world.component_types(entity).is_empty() {
            return fail("dead-types", format!("dead {entity:?} lists components"));
        }
    }
    for kind in KINDS {
        let want = model.live.values().any(|comps| comps.contains_key(&kind));
        if has_any_kind(world, kind) != want {
            return fail("has-any", format!("has_any::<{kind:?}> is not {want}"));
        }
    }
    let counters = world.counters();
    if (counters.spawned, counters.despawned) != (model.spawned, model.despawned) {
        return fail(
            "counters",
            format!(
                "World spawned {} despawned {}, model {} {}",
                counters.spawned, counters.despawned, model.spawned, model.despawned
            ),
        );
    }
    if sut.live.get() != model.live_t() {
        return fail(
            "drops",
            format!(
                "{} drop-counted values alive, the model holds {}",
                sut.live.get(),
                model.live_t()
            ),
        );
    }
    for (index, buffer) in sut.buffers.iter().enumerate() {
        let want = model.buffers[index].pending.len() + model.buffers[index].cmds.len();
        if buffer.is_empty() != (want == 0) {
            return fail(
                "buffer-empty",
                format!(
                    "buffer {index}: is_empty {} with {want} recorded",
                    buffer.is_empty()
                ),
            );
        }
    }
    Ok(())
}

// ------------------------------------------------------------------ the run

fn step(sut: &mut Sut, model: &mut Model, op: &Op) -> Result<(), Fail> {
    match op {
        Op::Spawn => {
            let entity = sut.world.spawn();
            model.born(entity)?;
        }
        Op::SpawnWith(s, vals) => {
            let entity = bundle(*s, vals, &sut.live, SpawnWith(&mut sut.world));
            model.born(entity)?;
            for val in vals {
                model.set(entity, val.clone());
            }
        }
        Op::InsertBundle(k, s, vals) => {
            let Some(entity) = model.nth_live(*k) else {
                return Ok(());
            };
            bundle(*s, vals, &sut.live, InsertBundle(&mut sut.world, entity));
            for val in vals {
                model.set(entity, val.clone());
            }
        }
        Op::Insert(k, val) => {
            let Some(entity) = model.nth_live(*k) else {
                return Ok(());
            };
            insert_val(&mut sut.world, entity, val, &sut.live);
            model.set(entity, val.clone());
        }
        Op::Despawn(at) => {
            let entity = model.resolve(*at);
            sut.world.despawn(entity);
            model.kill(entity);
        }
        Op::Remove(at, kind) => {
            let entity = model.resolve(*at);
            let got = remove_val(&mut sut.world, entity, *kind);
            let want = model
                .live
                .get_mut(&key(entity))
                .and_then(|comps| comps.remove(kind));
            if got != want {
                return fail(
                    "remove",
                    format!("remove {kind:?} from {entity:?}: World {got:?}, model {want:?}"),
                );
            }
        }
        Op::Get(at, kind) => {
            let entity = model.resolve(*at);
            let got = get_val(&sut.world, entity, *kind);
            let want = model
                .live
                .get(&key(entity))
                .and_then(|comps| comps.get(kind).cloned());
            if got != want {
                return fail(
                    "get",
                    format!("get {kind:?} of {entity:?}: World {got:?}, model {want:?}"),
                );
            }
        }
        Op::Write(at, kind, x) => {
            let entity = model.resolve(*at);
            let got = mutate_world(&mut sut.world, entity, *kind, *x);
            let want = match model
                .live
                .get_mut(&key(entity))
                .and_then(|comps| comps.get_mut(kind))
            {
                Some(val) => {
                    mutate_val(val, *x);
                    true
                }
                None => false,
            };
            if got != want {
                return fail(
                    "get-mut",
                    format!("get_mut {kind:?} of {entity:?}: World {got}, model {want}"),
                );
            }
        }
        Op::Query(q) => {
            let got = world_rows(&sut.world, *q)?;
            compare_rows(
                &format!("query-{q}"),
                got,
                &model_rows(model, &READ_SPECS[*q as usize]),
            )?;
        }
        Op::QueryMut(q) => {
            let want = model_rows(model, &MUT_SPECS[*q as usize]);
            let got = world_rows_mut(&mut sut.world, *q);
            compare_rows(&format!("query-mut-{q}"), got, &want)?;
            model_write(model, *q);
        }
        Op::Record(buf, rec) => record(sut, model, *buf as usize, rec),
        Op::Flush(buf, by_value) => flush(sut, model, *buf as usize, *by_value)?,
    }
    Ok(())
}

fn record(sut: &mut Sut, model: &mut Model, buf: usize, rec: &Rec) {
    let resolve = |model: &Model, at: Pick| model.resolve(at);
    let live = Rc::clone(&sut.live);
    match rec {
        Rec::Spawn => {
            let pending = sut.buffers[buf].spawn();
            model.buffers[buf].pending.push(pending);
        }
        Rec::SpawnWith(s, vals) => {
            let pending = bundle(*s, vals, &live, BufSpawnWith(&mut sut.buffers[buf]));
            let mb = &mut model.buffers[buf];
            mb.pending.push(pending);
            let index = mb.pending.len() - 1;
            mb.cmds.extend(
                vals.iter()
                    .map(|v| Cmd::Insert(Target::Pending(index), v.clone())),
            );
            mb.queued_t += t_count(vals);
        }
        Rec::InsertPending(k, val) => {
            let mb = &mut model.buffers[buf];
            if mb.pending.is_empty() {
                return;
            }
            let index = *k as usize % mb.pending.len();
            buffer_insert_pending(&mut sut.buffers[buf], mb.pending[index], val, &live);
            mb.cmds
                .push(Cmd::Insert(Target::Pending(index), val.clone()));
            mb.queued_t += t_count(std::slice::from_ref(val));
        }
        Rec::InsertBundlePending(k, s, vals) => {
            let mb = &mut model.buffers[buf];
            if mb.pending.is_empty() {
                return;
            }
            let index = *k as usize % mb.pending.len();
            let pending = mb.pending[index];
            bundle(
                *s,
                vals,
                &live,
                BufInsertBundlePending(&mut sut.buffers[buf], pending),
            );
            mb.cmds.extend(
                vals.iter()
                    .map(|v| Cmd::Insert(Target::Pending(index), v.clone())),
            );
            mb.queued_t += t_count(vals);
        }
        Rec::Insert(at, val) => {
            let entity = resolve(model, *at);
            buffer_insert(&mut sut.buffers[buf], entity, val, &live);
            let mb = &mut model.buffers[buf];
            mb.cmds.push(Cmd::Insert(Target::Live(entity), val.clone()));
            mb.queued_t += t_count(std::slice::from_ref(val));
        }
        Rec::InsertBundle(at, s, vals) => {
            let entity = resolve(model, *at);
            bundle(
                *s,
                vals,
                &live,
                BufInsertBundle(&mut sut.buffers[buf], entity),
            );
            let mb = &mut model.buffers[buf];
            mb.cmds.extend(
                vals.iter()
                    .map(|v| Cmd::Insert(Target::Live(entity), v.clone())),
            );
            mb.queued_t += t_count(vals);
        }
        Rec::Remove(at, kind) => {
            let entity = resolve(model, *at);
            buffer_remove(&mut sut.buffers[buf], entity, *kind);
            model.buffers[buf].cmds.push(Cmd::Remove(entity, *kind));
        }
        Rec::Despawn(at) => {
            let entity = resolve(model, *at);
            sut.buffers[buf].despawn(entity);
            model.buffers[buf].cmds.push(Cmd::Despawn(entity));
        }
        Rec::Bump(at) => {
            let entity = resolve(model, *at);
            sut.buffers[buf].call(entity, bump_a);
            model.buffers[buf].cmds.push(Cmd::Bump(entity));
        }
        Rec::PurgeC => {
            let nobody = Entity {
                index: u32::MAX,
                generation: 0,
            };
            sut.buffers[buf].call(nobody, purge_c);
            model.buffers[buf].cmds.push(Cmd::PurgeC);
        }
    }
}

fn flush(sut: &mut Sut, model: &mut Model, buf: usize, by_value: bool) -> Result<(), Fail> {
    let spawned: Vec<Entity> = if by_value && model.buffers[buf].pending.is_empty() {
        let buffer = std::mem::take(&mut sut.buffers[buf]);
        sut.world.flush(buffer);
        Vec::new()
    } else {
        sut.world.flush_reusing(&mut sut.buffers[buf]);
        sut.buffers[buf].pending_entities.clone()
    };
    let mb = std::mem::take(&mut model.buffers[buf]);
    if spawned.len() != mb.pending.len() {
        return fail(
            "flush-spawns",
            format!(
                "{} pending spawns, {} allocated",
                mb.pending.len(),
                spawned.len()
            ),
        );
    }
    for &entity in &spawned {
        model.born(entity)?;
    }
    for cmd in mb.cmds {
        match cmd {
            Cmd::Insert(target, val) => {
                let entity = match target {
                    Target::Live(entity) => entity,
                    Target::Pending(index) => spawned[index],
                };
                model.set(entity, val);
            }
            Cmd::Remove(entity, kind) => {
                if let Some(comps) = model.live.get_mut(&key(entity)) {
                    comps.remove(&kind);
                }
            }
            Cmd::Despawn(entity) => model.kill(entity),
            Cmd::Bump(entity) => {
                if let Some(Val::A(a)) = model
                    .live
                    .get_mut(&key(entity))
                    .and_then(|comps| comps.get_mut(&Kind::A))
                {
                    *a = a.wrapping_add(1);
                }
            }
            Cmd::PurgeC => {
                let doomed: Vec<Entity> = model
                    .live
                    .iter()
                    .filter(|(_, comps)| comps.contains_key(&Kind::C))
                    .map(|(&k, _)| entity_at(k))
                    .collect();
                for entity in doomed {
                    model.kill(entity);
                }
            }
        }
    }
    let buffer = &sut.buffers[buf];
    if !buffer.is_empty() {
        return fail(
            "flush-leaves",
            format!("buffer {buf} holds {} after its flush", buffer.len()),
        );
    }
    Ok(())
}

/// What a run did, for `TUNGSTEN_ORACLE_STATS=1`: the most entities live
/// at once, spawns, despawns, flushes and the commands they replayed, the
/// archetypes made and the query rows compared.
#[derive(Default, Debug)]
struct Stats {
    max_live: usize,
    spawned: u64,
    despawned: u64,
    flushes: u64,
    flushed_spawns: u64,
    replayed: u64,
    archetypes: usize,
    rows: u64,
}

fn run(ops: &[Op]) -> Result<(), Fail> {
    run_with(ops, &mut Stats::default())
}

fn run_with(ops: &[Op], stats: &mut Stats) -> Result<(), Fail> {
    guarded(|| {
        let mut sut = Sut {
            world: World::new(),
            buffers: [CommandBuffer::new(), CommandBuffer::new()],
            live: Rc::new(Cell::new(0)),
        };
        let mut model = Model::default();
        for (index, op) in ops.iter().enumerate() {
            let at = |mut err: Fail| {
                err.step = index;
                err
            };
            match op {
                Op::Flush(buf, _) => {
                    let mb = &model.buffers[*buf as usize];
                    stats.flushes += 1;
                    stats.flushed_spawns += mb.pending.len() as u64;
                    stats.replayed += mb.cmds.len() as u64;
                }
                Op::Query(q) => {
                    stats.rows += model_rows(&model, &READ_SPECS[*q as usize]).len() as u64;
                }
                _ => {}
            }
            step(&mut sut, &mut model, op).map_err(at)?;
            check(&sut, &model).map_err(at)?;
            stats.max_live = stats.max_live.max(model.live.len());
        }
        stats.spawned += model.spawned;
        stats.despawned += model.despawned;
        stats.archetypes = stats.archetypes.max(sut.world.archetypes.archetypes.len());
        // Everything flushed and dropped: no value outlives the World.
        for buf in 0..2 {
            flush(&mut sut, &mut model, buf, false).map_err(|mut err| {
                err.step = ops.len();
                err
            })?;
        }
        check(&sut, &model)?;
        let live = Rc::clone(&sut.live);
        drop(sut);
        if live.get() != 0 {
            return fail(
                "drop-world",
                format!("{} values outlive the World", live.get()),
            );
        }
        Ok(())
    })
}

/// The default run's seeds and sequence length (`notes.md` has the debug
/// and release times).
const SEEDS: u64 = 2_000;
const LEN: usize = 400;

/// `TUNGSTEN_ORACLE_STATS=1 cargo test -p tungsten-core --lib oracle_stats
/// -- --nocapture` prints what the default seeds exercise.
#[test]
fn oracle_stats() {
    if std::env::var_os("TUNGSTEN_ORACLE_STATS").is_none() {
        return;
    }
    let mut stats = Stats::default();
    let seeds = seeds(0, SEEDS);
    for &seed in &seeds {
        let _ = run_with(&generate(seed, LEN), &mut stats);
    }
    println!("ecs oracle, {} seeds of {LEN} ops: {stats:?}", seeds.len());
}

/// The default seeds' eighth `part` (0 to 7).
fn agrees_on_default_eighth(part: usize) {
    drive(
        "ecs oracle",
        &seed_part(0, SEEDS, part, 8),
        |seed| generate(seed, LEN),
        run,
    );
}

#[test]
fn world_and_command_buffers_agree_with_the_model_1() {
    agrees_on_default_eighth(0);
}

#[test]
fn world_and_command_buffers_agree_with_the_model_2() {
    agrees_on_default_eighth(1);
}

#[test]
fn world_and_command_buffers_agree_with_the_model_3() {
    agrees_on_default_eighth(2);
}

#[test]
fn world_and_command_buffers_agree_with_the_model_4() {
    agrees_on_default_eighth(3);
}

#[test]
fn world_and_command_buffers_agree_with_the_model_5() {
    agrees_on_default_eighth(4);
}

#[test]
fn world_and_command_buffers_agree_with_the_model_6() {
    agrees_on_default_eighth(5);
}

#[test]
fn world_and_command_buffers_agree_with_the_model_7() {
    agrees_on_default_eighth(6);
}

#[test]
fn world_and_command_buffers_agree_with_the_model_8() {
    agrees_on_default_eighth(7);
}

#[test]
#[ignore = "long mode: run by hand, about ten minutes in debug"]
fn world_and_command_buffers_agree_with_the_model_long() {
    drive(
        "ecs oracle (long)",
        &seeds(1_000_000, SEEDS * 20),
        // Twice as long: a sequence's checks grow with its live and dead
        // handles, so its cost with the square of its length.
        |seed| generate(seed, LEN * 2),
        run,
    );
}

#[test]
fn each_known_row_is_still_open() {
    // A fixed row leaves its case to the generators: drop its entry here.
    let issues =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/known-issues.md");
    let issues = std::fs::read_to_string(&issues).expect("docs/known-issues.md reads");
    for known in KNOWN {
        if let Some(words) = known.words {
            assert!(
                issues.contains(words),
                "KNOWN `{}` ({}): no row of docs/known-issues.md holds `{words}` now; \
                 let the oracle generate {}",
                known.id,
                known.row,
                known.case
            );
        }
    }
    let mut ids: Vec<&str> = KNOWN.iter().map(|known| known.id).collect();
    ids.extend(DESIGN.iter().map(|&(id, _, _)| id));
    ids.extend(crate::save::oracle::FOUND.iter().map(|&(id, _, _)| id));
    let count = ids.len();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), count, "each skipped case has its own ID");
}

#[test]
fn the_oracle_catches_a_wrong_model() {
    // A model that forgets a removal must fail, so a pass means something.
    let ops = vec![
        Op::SpawnWith(1, vec![Val::A(1), Val::B("x".into())]),
        Op::Remove(Pick::Live(0), Kind::A),
    ];
    assert!(run(&ops).is_ok());
    let result = guarded(|| {
        let mut sut = Sut {
            world: World::new(),
            buffers: [CommandBuffer::new(), CommandBuffer::new()],
            live: Rc::new(Cell::new(0)),
        };
        let mut model = Model::default();
        step(&mut sut, &mut model, &ops[0])?;
        let entity = model.nth_live(0).unwrap();
        sut.world.remove_component::<A>(entity);
        check(&sut, &model)
    });
    assert_eq!(result.unwrap_err().tag, "get");
}

#[test]
fn the_shrinker_finds_the_smallest_failing_sequence() {
    let failing = |ops: &[u32]| {
        if ops.contains(&3) && ops.contains(&7) {
            fail("both", "3 and 7")
        } else {
            Ok(())
        }
    };
    let (ops, err) = shrink((0..40).collect(), "both", 1_000, failing);
    assert_eq!(ops, [3, 7]);
    assert_eq!(err.tag, "both");
}
