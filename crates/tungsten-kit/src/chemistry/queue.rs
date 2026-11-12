//! Chemistry's queue: one entry per stimulus and receiver, merged, bounded
//! and capped, saved with its receivers and credits by save ID and its tile
//! cells by their keys.

use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};

use serde::{Deserialize, Serialize};
use tungsten_core::save::{RefCodec, RefReader, RefWriter, SaveOptions, Saved};
use tungsten_core::{DataRef, Entity, World};

use crate::chemistry::stimuli::Emission;
use crate::chemistry::tiles::{Cell, CellSave};
use crate::damage::effects::Effect;
use crate::foundation::saved::{Order, order_of};

/// The hasher of the queue's two maps, which every candidate of every
/// emission probes: a rotate, an xor and a multiply a word (the `Fx` mix),
/// where the standard one's SipHash cost more than the rest of a probe.
/// The keys are the World's own entities and cells, no input a player
/// shapes, and neither map is ever walked, so its order is nothing's.
#[derive(Default)]
struct QueueHasher(u64);

impl QueueHasher {
    #[inline]
    fn add(&mut self, word: u64) {
        self.0 = (self.0.rotate_left(5) ^ word).wrapping_mul(0x517c_c1b7_2722_0a95);
    }
}

impl Hasher for QueueHasher {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        for chunk in bytes.chunks(8) {
            let mut word = [0; 8];
            word[..chunk.len()].copy_from_slice(chunk);
            self.add(u64::from_le_bytes(word));
        }
    }

    fn write_u8(&mut self, n: u8) {
        self.add(u64::from(n));
    }

    fn write_u32(&mut self, n: u32) {
        self.add(u64::from(n));
    }

    fn write_u64(&mut self, n: u64) {
        self.add(n);
    }

    fn write_usize(&mut self, n: usize) {
        self.add(n as u64);
    }
}

/// A map on [`QueueHasher`].
type QueueMap<K, V> = HashMap<K, V, BuildHasherDefault<QueueHasher>>;

/// What an entry runs on: an entity, or a placement's tile cell.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum Receiver {
    Entity(Entity),
    Cell(Cell),
}

impl Receiver {
    /// The entity, when it is one.
    pub(crate) fn entity(&self) -> Option<Entity> {
        match self {
            Self::Entity(entity) => Some(*entity),
            Self::Cell(_) => None,
        }
    }
}

/// A receiver's place in a run's order: entities by their save order, then
/// cells by map ID, origin, layer, row and column.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum ReceiverKey {
    Entity((bool, u64)),
    Cell(Cell),
}

/// An entry's place in a run's order (its age, depth, receiver's order and
/// stimulus rank) and its slot.
type TakeKey = (u64, u8, ReceiverKey, u32, usize);

/// The name a save holds the queue under.
pub(crate) const CHEMISTRY_QUEUE: &str = "chemistry_queue";

/// A run's cap, the queue's bound, the depth limit and the light's period
/// by default.
pub(crate) const CAP: usize = 256;
pub(crate) const BOUND: usize = 16_384;
pub(crate) const DEPTH: u8 = 8;
pub(crate) const LIGHT_PERIOD: f32 = 0.25;

/// Chemistry's settings: a run's cap, the queue's bound, the depth limit
/// and the light's period, a World resource
/// [`ChemistryPlugin`](crate::ChemistryPlugin) writes and no save holds, so
/// a World keeps its own through a load, which replaces the saved queue
/// whole.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Settings {
    pub(crate) cap: usize,
    pub(crate) bound: usize,
    pub(crate) depth: u8,
    pub(crate) light_period: f32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            cap: CAP,
            bound: BOUND,
            depth: DEPTH,
            light_period: LIGHT_PERIOD,
        }
    }
}

impl Settings {
    /// The World's, else the defaults.
    pub(crate) fn of(world: &World) -> Self {
        world.get_resource::<Self>().copied().unwrap_or_default()
    }
}

/// A status's own list an entry runs: its `on_end`, or a band's `enter`.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct StatusList {
    pub(crate) effect: DataRef<Effect>,
    /// The band whose `enter` it runs; `None` for the status's `on_end`.
    pub(crate) band: Option<u32>,
}

/// One queued reaction: a stimulus on a receiver, merged over the hits that
/// brought it, or a status's own list.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Entry {
    pub(crate) receiver: Receiver,
    /// The stimulus's interned name; a status list's label.
    pub(crate) stimulus: u32,
    pub(crate) status: Option<Box<StatusList>>,
    /// The hits' raw amounts summed, which a rule's `min` reads.
    pub(crate) raw: f64,
    /// Each hit's amount times its status effect, summed: a `scale`
    /// build's.
    pub(crate) weighted: f64,
    /// The largest status effect, a fixed build's and an `apply`'s.
    pub(crate) status_effect: f32,
    /// The largest damage-over-time base.
    pub(crate) dot_base: f32,
    /// The least depth.
    pub(crate) depth: u8,
    /// The first credit.
    pub(crate) credit: Option<Entity>,
    /// The run it was first queued for.
    pub(crate) age: u64,
    /// A loaded entry whose receiver the load's references are to name: one
    /// they leave the placeholder is dropped at the load's rebuild, its
    /// receiver gone from the World. It sits in the entry's padding, so the
    /// queue's size holds.
    pub(crate) awaiting: bool,
}

/// One stimulus to queue, before its merge.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Stimulus {
    pub(crate) amount: f32,
    pub(crate) status_effect: f32,
    pub(crate) dot_base: f32,
    pub(crate) depth: u8,
    pub(crate) credit: Option<Entity>,
}

/// Chemistry's queue and counters, a World resource [`ChemistryPlugin`]
/// inserts and a save holds: each entry a stimulus on a receiver (one per
/// pair, merged) or a status's own list, its receiver and credit by save
/// ID. [`pending`](Self::pending) entries wait for a run;
/// [`dropped`](Self::dropped) counts the adds past the bound,
/// [`cut`](Self::cut) those past the depth limit and [`ran`](Self::ran)
/// the entries a run took.
///
/// [`ChemistryPlugin`]: crate::ChemistryPlugin
///
/// # Examples
///
/// ```
/// use tungsten_core::{Plugin, Schedule, World};
/// use tungsten_kit::{Chemistry, ChemistryPlugin};
///
/// let mut world = World::new();
/// let mut schedule = Schedule::new();
/// ChemistryPlugin::new().with_cap(8).build(&mut schedule, &mut world);
/// let chemistry = world.resource::<Chemistry>();
/// assert_eq!((chemistry.pending(), chemistry.dropped(), chemistry.cut(), chemistry.ran()), (0, 0, 0, 0));
/// ```
#[derive(Debug)]
pub struct Chemistry {
    slots: Vec<Option<Entry>>,
    free: Vec<usize>,
    /// A stimulus entry's slot by receiver and stimulus.
    index: QueueMap<(Receiver, u32), usize>,
    /// Each entity receiver's entries.
    counts: QueueMap<Entity, u32>,
    /// A run's take keys, kept so a warm run allocates nothing.
    keys: Vec<TakeKey>,
    /// The interned stimulus names and status labels, and each one's rank
    /// by name.
    names: Vec<String>,
    ranks: Vec<u32>,
    /// The receivers the kit's despawn holds for their entries: they take
    /// no new one.
    held: Vec<Entity>,
    pending: usize,
    dropped: u64,
    cut: u64,
    ran: u64,
    /// The next run's number.
    pub(crate) run: u64,
    /// An add was dropped this run, warned once.
    pub(crate) dropped_now: bool,
    /// A game's stimulus was refused this run, warned once.
    pub(crate) refused_now: bool,
    /// The entries meet the World's [`Settings`]: unset in a queue a load
    /// restored (the saved form holds no flag), whose first run cuts and
    /// drops what passes them.
    checked: bool,
    /// The emissions since the last run, which it drains; unsaved, since a
    /// step makes and takes them.
    pub(crate) emitted: Vec<Emission>,
    /// Game seconds to the next light sample, while a light rule is loaded.
    pub(crate) light_left: Option<f32>,
}

impl Default for Chemistry {
    fn default() -> Self {
        Self {
            slots: Vec::new(),
            free: Vec::new(),
            index: QueueMap::default(),
            counts: QueueMap::default(),
            keys: Vec::new(),
            names: Vec::new(),
            ranks: Vec::new(),
            held: Vec::new(),
            pending: 0,
            dropped: 0,
            cut: 0,
            ran: 0,
            run: 0,
            dropped_now: false,
            refused_now: false,
            checked: true,
            emitted: Vec::new(),
            light_left: None,
        }
    }
}

impl Chemistry {
    /// The entries waiting for a run.
    #[must_use]
    pub fn pending(&self) -> usize {
        self.pending
    }

    /// The adds dropped past the bound, since the World began.
    #[must_use]
    pub fn dropped(&self) -> u64 {
        self.dropped
    }

    /// The adds dropped past the depth limit, since the World began.
    #[must_use]
    pub fn cut(&self) -> u64 {
        self.cut
    }

    /// The entries the runs took, since the World began.
    #[must_use]
    pub fn ran(&self) -> u64 {
        self.ran
    }

    /// Whether `receiver` has an entry waiting.
    pub(crate) fn holds(&self, receiver: Entity) -> bool {
        self.counts.get(&receiver).is_some_and(|&count| count > 0)
    }

    /// Holds `receiver` for its entries: it takes no new one.
    pub(crate) fn hold(&mut self, receiver: Entity) {
        if !self.held.contains(&receiver) {
            self.held.push(receiver);
        }
    }

    /// The receivers the kit's despawn holds.
    pub(crate) fn held(&self) -> &[Entity] {
        &self.held
    }

    /// Lets go of the held receivers `keep` refuses.
    pub(crate) fn release_held(&mut self, keep: impl Fn(Entity) -> bool) {
        self.held.retain(|&entity| keep(entity));
    }

    /// The interned id of `name`, when it has one.
    pub(crate) fn find(&self, name: &str) -> Option<u32> {
        self.names
            .iter()
            .position(|known| known == name)
            .map(|at| at as u32)
    }

    /// The interned id of `name`.
    pub(crate) fn intern(&mut self, name: &str) -> u32 {
        if let Some(at) = self.find(name) {
            return at;
        }
        self.names.push(name.to_owned());
        let mut order: Vec<u32> = (0..self.names.len() as u32).collect();
        order.sort_by(|&a, &b| self.names[a as usize].cmp(&self.names[b as usize]));
        self.ranks = vec![0; order.len()];
        for (rank, id) in order.into_iter().enumerate() {
            self.ranks[id as usize] = rank as u32;
        }
        (self.names.len() - 1) as u32
    }

    /// The name an entry's interned `id` stands for.
    pub(crate) fn name(&self, id: u32) -> &str {
        &self.names[id as usize]
    }

    /// Queues `stimulus` named `name` on `receiver` for run `age`: merged
    /// into its entry when the pair has one; else a new entry, unless the
    /// depth passes the limit (cut), the receiver is held, or the queue is
    /// at its bound (dropped). Whether it reached an entry.
    pub(crate) fn stimulate(
        &mut self,
        settings: &Settings,
        receiver: Receiver,
        name: &str,
        stimulus: Stimulus,
        age: u64,
    ) -> bool {
        let id = self.intern(name);
        self.stimulate_slot(settings, receiver, id, stimulus, age)
            .is_some()
    }

    /// [`stimulate`](Self::stimulate) by an interned name.
    pub(crate) fn stimulate_id(
        &mut self,
        settings: &Settings,
        receiver: Receiver,
        id: u32,
        stimulus: Stimulus,
        age: u64,
    ) {
        self.stimulate_slot(settings, receiver, id, stimulus, age);
    }

    /// [`stimulate_id`](Self::stimulate_id), answering the slot of the
    /// entry it merged into or made: `None` for a stimulus cut, held or
    /// dropped. The slot stays the pair's while no entry leaves the queue,
    /// so a pass that only adds may keep it for
    /// [`stimulate_kept`](Self::stimulate_kept).
    pub(crate) fn stimulate_slot(
        &mut self,
        settings: &Settings,
        receiver: Receiver,
        id: u32,
        stimulus: Stimulus,
        age: u64,
    ) -> Option<usize> {
        if stimulus.depth > settings.depth {
            self.cut += 1;
            return None;
        }
        if self.is_held(&receiver) {
            return None;
        }
        let key = (receiver, id);
        if let Some(&slot) = self.index.get(&key) {
            self.merge(slot, stimulus);
            return Some(slot);
        }
        let entry = Entry {
            receiver: key.0.clone(),
            stimulus: id,
            status: None,
            raw: f64::from(stimulus.amount),
            weighted: f64::from(stimulus.amount) * f64::from(stimulus.status_effect),
            status_effect: stimulus.status_effect,
            dot_base: stimulus.dot_base,
            depth: stimulus.depth,
            credit: stimulus.credit,
            age,
            awaiting: false,
        };
        let slot = self.insert(settings.bound, entry)?;
        self.index.insert(key, slot);
        Some(slot)
    }

    /// [`stimulate_id`](Self::stimulate_id) on the pair whose entry
    /// [`stimulate_slot`](Self::stimulate_slot) answered in `slot`, with
    /// no entry taken and no receiver held since: the same cut and the same
    /// merge, without the index's probe. The receiver was not held when
    /// the slot was answered, so the hold has nothing to refuse.
    pub(crate) fn stimulate_kept(&mut self, settings: &Settings, slot: usize, stimulus: Stimulus) {
        if stimulus.depth > settings.depth {
            self.cut += 1;
            return;
        }
        self.merge(slot, stimulus);
    }

    /// Merges `stimulus` into the stimulus entry in `slot`.
    fn merge(&mut self, slot: usize, stimulus: Stimulus) {
        let entry = self.slots[slot]
            .as_mut()
            .expect("an indexed slot holds its entry");
        let limit = f64::from(f32::MAX);
        let weighted = f64::from(stimulus.amount) * f64::from(stimulus.status_effect);
        entry.raw = (entry.raw + f64::from(stimulus.amount)).min(limit);
        entry.weighted = (entry.weighted + weighted).min(limit);
        entry.status_effect = entry.status_effect.max(stimulus.status_effect);
        entry.dot_base = entry.dot_base.max(stimulus.dot_base);
        entry.depth = entry.depth.min(stimulus.depth);
        if entry.credit.is_none() {
            entry.credit = stimulus.credit;
        }
    }

    /// The entries in a stable order, for a test's comparison: by age,
    /// depth, receiver (entities by index, then cells) and name.
    #[cfg(test)]
    pub(crate) fn entries(&self) -> Vec<Entry> {
        let receiver = |entry: &Entry| match &entry.receiver {
            Receiver::Entity(entity) => (0, entity.id(), None),
            Receiver::Cell(cell) => (1, 0, Some(cell.clone())),
        };
        let mut entries: Vec<&Entry> = self.slots.iter().flatten().collect();
        entries.sort_by(|a, b| {
            (a.age, a.depth, receiver(a), self.name(a.stimulus)).cmp(&(
                b.age,
                b.depth,
                receiver(b),
                self.name(b.stimulus),
            ))
        });
        entries.into_iter().cloned().collect()
    }

    /// Whether `receiver` is an entity the kit's despawn holds.
    fn is_held(&self, receiver: &Receiver) -> bool {
        receiver
            .entity()
            .is_some_and(|entity| self.held.contains(&entity))
    }

    /// Queues a status's own list on `receiver` for run `age`, unmerged,
    /// under the same depth limit, hold and bound.
    pub(crate) fn queue_list(
        &mut self,
        settings: &Settings,
        receiver: Receiver,
        list: StatusList,
        depth: u8,
        credit: Option<Entity>,
        age: u64,
    ) {
        if depth > settings.depth {
            self.cut += 1;
            return;
        }
        if self.is_held(&receiver) {
            return;
        }
        let label = match list.band {
            None => format!("{}#on_end", list.effect.id().unwrap_or("")),
            Some(band) => format!("{}#enter.{band}", list.effect.id().unwrap_or("")),
        };
        let id = self.intern(&label);
        self.insert(
            settings.bound,
            Entry {
                receiver,
                stimulus: id,
                status: Some(Box::new(list)),
                raw: 0.0,
                weighted: 0.0,
                status_effect: 1.0,
                dot_base: 0.0,
                depth,
                credit,
                age,
                awaiting: false,
            },
        );
    }

    /// Puts `entry` in a free slot, or drops it past `bound`.
    fn insert(&mut self, bound: usize, entry: Entry) -> Option<usize> {
        if self.pending >= bound {
            self.dropped += 1;
            self.dropped_now = true;
            return None;
        }
        if let Some(entity) = entry.receiver.entity() {
            *self.counts.entry(entity).or_insert(0) += 1;
        }
        self.pending += 1;
        if let Some(slot) = self.free.pop() {
            self.slots[slot] = Some(entry);
            Some(slot)
        } else {
            self.slots.push(Some(entry));
            Some(self.slots.len() - 1)
        }
    }

    /// The take order's key of the entry in `slot`: its age, depth,
    /// receiver's save order and stimulus rank.
    fn key(&self, order: Order<'_>, slot: usize, entry: &Entry) -> TakeKey {
        let receiver = match &entry.receiver {
            Receiver::Entity(entity) => ReceiverKey::Entity(order.key(*entity)),
            Receiver::Cell(cell) => ReceiverKey::Cell(cell.clone()),
        };
        (
            entry.age,
            entry.depth,
            receiver,
            self.ranks[entry.stimulus as usize],
            slot,
        )
    }

    /// Takes the entry out of `slot`, which holds one.
    fn remove(&mut self, slot: usize) -> Entry {
        let entry = self.slots[slot]
            .take()
            .expect("a chosen slot holds its entry");
        self.free.push(slot);
        self.pending -= 1;
        if entry.status.is_none() {
            self.index.remove(&(entry.receiver.clone(), entry.stimulus));
        }
        if let Some(entity) = entry.receiver.entity()
            && let Some(count) = self.counts.get_mut(&entity)
        {
            *count -= 1;
            if *count == 0 {
                self.counts.remove(&entity);
            }
        }
        entry
    }

    /// Whether a load restored it and no run has checked it yet.
    pub(crate) fn unchecked(&self) -> bool {
        !self.checked
    }

    /// The first run's check after a load: cuts the entries past the depth
    /// limit, then drops the youngest past the bound (the last in the take
    /// order), each counted, so a queue restored from looser settings meets
    /// the World's before it runs; later runs skip it.
    pub(crate) fn enforce(&mut self, world: &World, settings: &Settings) {
        self.checked = true;
        for slot in 0..self.slots.len() {
            if self.slots[slot]
                .as_ref()
                .is_some_and(|entry| entry.depth > settings.depth)
            {
                self.remove(slot);
                self.cut += 1;
            }
        }
        if self.pending <= settings.bound {
            return;
        }
        let order = order_of(world);
        let mut keys: Vec<TakeKey> = self
            .slots
            .iter()
            .enumerate()
            .filter_map(|(slot, entry)| Some(self.key(order, slot, entry.as_ref()?)))
            .collect();
        keys.sort_unstable();
        for (.., slot) in keys.drain(settings.bound..) {
            self.remove(slot);
            self.dropped += 1;
            self.dropped_now = true;
        }
    }

    /// Takes into `out` at most `cap` of the entries queued for run `run`
    /// or before, oldest first: by age, then depth, then the receiver's save
    /// order, then the stimulus's name.
    pub(crate) fn take(&mut self, world: &World, cap: usize, run: u64, out: &mut Vec<Entry>) {
        out.clear();
        let order = order_of(world);
        let mut chosen = std::mem::take(&mut self.keys);
        chosen.clear();
        chosen.extend(self.slots.iter().enumerate().filter_map(|(slot, entry)| {
            let entry = entry.as_ref()?;
            (entry.age <= run).then(|| self.key(order, slot, entry))
        }));
        if chosen.len() > cap {
            chosen.select_nth_unstable(cap);
            chosen.truncate(cap);
        }
        chosen.sort_unstable();
        for (.., slot) in chosen.drain(..) {
            out.push(self.remove(slot));
        }
        self.keys = chosen;
    }

    /// Counts `count` entries run.
    pub(crate) fn count_ran(&mut self, count: usize) {
        self.ran += count as u64;
    }

    /// The queue as a save writes it: its entries in the run's order with
    /// each receiver keyed by its save ID, not its index (age, depth, a
    /// named receiver by save ID, then an unnamed one, then cells, then the
    /// stimulus's name; ties in slot order), and each entity receiver
    /// without a save ID, despawned or never saved, as the placeholder, a
    /// dead entry. So a World whose entities took other indices writes the
    /// same queue, and a loaded one, whose dead entries share the
    /// placeholder in their saved order, writes what it loaded.
    fn saved_copy(&self, world: &World) -> Chemistry {
        let mut keyed: Vec<(TakeKey, Entry)> = self
            .slots
            .iter()
            .enumerate()
            .filter_map(|(slot, entry)| {
                let mut entry = entry.clone()?;
                let receiver = match &entry.receiver {
                    Receiver::Entity(entity) => {
                        if let Some(saved) = world.get::<Saved>(*entity) {
                            ReceiverKey::Entity((false, saved.id().0))
                        } else {
                            entry.receiver = Receiver::Entity(Entity::PLACEHOLDER);
                            ReceiverKey::Entity((true, 0))
                        }
                    }
                    Receiver::Cell(cell) => ReceiverKey::Cell(cell.clone()),
                };
                let rank = self.ranks[entry.stimulus as usize];
                Some(((entry.age, entry.depth, receiver, rank, slot), entry))
            })
            .collect();
        keyed.sort_unstable_by(|a, b| a.0.cmp(&b.0));
        Chemistry {
            slots: keyed.into_iter().map(|(_, entry)| Some(entry)).collect(),
            names: self.names.clone(),
            ranks: self.ranks.clone(),
            pending: self.pending,
            dropped: self.dropped,
            cut: self.cut,
            ran: self.ran,
            run: self.run,
            checked: self.checked,
            light_left: self.light_left,
            ..Chemistry::default()
        }
    }
}

/// An entry as a save writes it, its receiver and credit by reference.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct EntrySave {
    /// A tile cell's receiver; an entity's is a reference.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    cell: Option<CellSave>,
    /// The stimulus's name; for a status's list, the status.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    stimulus: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    status: Option<DataRef<Effect>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    band: Option<u32>,
    #[serde(default)]
    raw: f64,
    #[serde(default)]
    weighted: f64,
    #[serde(default = "one")]
    status_effect: f32,
    #[serde(default)]
    dot_base: f32,
    #[serde(default)]
    depth: u8,
    #[serde(default)]
    age: u64,
    /// Its receiver was despawned or never saved at the capture: the load
    /// keeps it dead, so a run takes it under the cap and skips it, as the
    /// uninterrupted run does.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    dead: bool,
}

fn one() -> f32 {
    1.0
}

/// The queue as a save writes it.
#[derive(Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct ChemistrySave {
    entries: Vec<EntrySave>,
    run: u64,
    dropped: u64,
    cut: u64,
    ran: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    light_left: Option<f32>,
}

impl From<&Chemistry> for ChemistrySave {
    /// The entries in slot order: a capture saves the queue's
    /// [`saved_copy`](Chemistry::saved_copy) ([`queue_saved`]).
    fn from(chemistry: &Chemistry) -> Self {
        Self {
            entries: chemistry
                .slots
                .iter()
                .flatten()
                .map(|entry| {
                    let (stimulus, status, band) = match &entry.status {
                        None => (Some(chemistry.name(entry.stimulus).to_owned()), None, None),
                        Some(list) => (None, Some(list.effect.clone()), list.band),
                    };
                    let cell = match &entry.receiver {
                        Receiver::Entity(_) => None,
                        Receiver::Cell(cell) => Some(cell.save()),
                    };
                    EntrySave {
                        cell,
                        stimulus,
                        status,
                        band,
                        raw: entry.raw,
                        weighted: entry.weighted,
                        status_effect: entry.status_effect,
                        dot_base: entry.dot_base,
                        depth: entry.depth,
                        age: entry.age,
                        dead: entry.receiver == Receiver::Entity(Entity::PLACEHOLDER),
                    }
                })
                .collect(),
            run: chemistry.run,
            dropped: chemistry.dropped,
            cut: chemistry.cut,
            ran: chemistry.ran,
            light_left: chemistry.light_left,
        }
    }
}

impl From<ChemistrySave> for Chemistry {
    /// The entries in their saved order under placeholder receivers, which
    /// the reference pass names or drops, a dead one's kept
    /// ([`queue_saved`]).
    fn from(saved: ChemistrySave) -> Self {
        let mut chemistry = Chemistry {
            run: saved.run,
            dropped: saved.dropped,
            cut: saved.cut,
            ran: saved.ran,
            checked: false,
            light_left: saved
                .light_left
                .filter(|left| left.is_finite())
                .map(|left| left.max(0.0)),
            ..Chemistry::default()
        };
        for entry in saved.entries {
            let (status, label) = match (entry.status, entry.stimulus) {
                (Some(effect), _) => {
                    let label = match entry.band {
                        None => format!("{}#on_end", effect.id().unwrap_or("")),
                        Some(band) => format!("{}#enter.{band}", effect.id().unwrap_or("")),
                    };
                    let list = StatusList {
                        effect,
                        band: entry.band,
                    };
                    (Some(Box::new(list)), label)
                }
                (None, Some(name)) => (None, name),
                // Its index stays, so the later entries keep their keys.
                (None, None) => {
                    chemistry.slots.push(None);
                    continue;
                }
            };
            let stimulus = chemistry.intern(&label);
            let receiver = entry
                .cell
                .as_ref()
                .map_or(Receiver::Entity(Entity::PLACEHOLDER), |cell| {
                    Receiver::Cell(cell.cell())
                });
            let awaiting = entry.cell.is_none() && !entry.dead;
            chemistry.slots.push(Some(Entry {
                receiver,
                stimulus,
                status,
                raw: entry.raw.max(0.0),
                weighted: entry.weighted.max(0.0),
                status_effect: entry.status_effect.max(0.0),
                dot_base: entry.dot_base.max(0.0),
                depth: entry.depth,
                credit: None,
                age: entry.age,
                awaiting,
            }));
        }
        // Whole without a reference pass too: a restore runs one only for a
        // save that wrote a reference.
        chemistry.rebuild(false);
        chemistry
    }
}

impl Chemistry {
    /// Builds the index, the counts, the free slots and `pending` from the
    /// slots, a dead entry (its receiver the placeholder) out of the index
    /// and the counts but pending; after a load's reference pass
    /// (`resolved`), first drops each entry whose receiver it was to name
    /// and did not.
    fn rebuild(&mut self, resolved: bool) {
        self.index.clear();
        self.counts.clear();
        self.free.clear();
        self.pending = 0;
        for slot in 0..self.slots.len() {
            let Some(entry) = &mut self.slots[slot] else {
                self.free.push(slot);
                continue;
            };
            let unnamed = entry.receiver == Receiver::Entity(Entity::PLACEHOLDER);
            if resolved && std::mem::take(&mut entry.awaiting) && unnamed {
                self.slots[slot] = None;
                self.free.push(slot);
                continue;
            }
            if unnamed {
                self.pending += 1;
                continue;
            }
            if entry.status.is_none() {
                self.index
                    .insert((entry.receiver.clone(), entry.stimulus), slot);
            }
            if let Some(entity) = entry.receiver.entity() {
                *self.counts.entry(entity).or_insert(0) += 1;
            }
            self.pending += 1;
        }
    }
}

/// Each saved entry's receiver, but a dead one's, and credit, by key, in
/// slot order: a capture's [`saved_copy`](Chemistry::saved_copy) holds the
/// save's.
fn queue_refs(chemistry: &Chemistry) -> impl Iterator<Item = (String, Entity)> + '_ {
    chemistry
        .slots
        .iter()
        .flatten()
        .enumerate()
        .flat_map(|(index, entry)| {
            entry
                .receiver
                .entity()
                .filter(|&receiver| receiver != Entity::PLACEHOLDER)
                .map(|receiver| (format!("entries.{index}.receiver"), receiver))
                .into_iter()
                .chain(
                    entry
                        .credit
                        .map(|credit| (format!("entries.{index}.credit"), credit)),
                )
        })
}

/// The queue, saved so a reaction waiting behind the cap runs once after a
/// load: its [`saved_copy`](Chemistry::saved_copy), each receiver and
/// credit by save ID; an entry whose receiver was despawned or never saved
/// at the capture loads dead, one whose receiver the loaded World lacks is
/// dropped, and a gone credit credits no one.
pub(crate) fn queue_saved() -> SaveOptions<Chemistry> {
    SaveOptions::<Chemistry>::form::<ChemistrySave>()
        .capture_as(|world, _, chemistry| Some(chemistry.saved_copy(world)))
        .refs(RefCodec {
            save: |chemistry, out: &mut RefWriter<'_>| {
                for (key, entity) in queue_refs(chemistry) {
                    out.put(&key, entity);
                }
            },
            load: |chemistry, refs: &RefReader<'_>| {
                for (index, slot) in chemistry.slots.iter_mut().enumerate() {
                    let Some(entry) = slot else {
                        continue;
                    };
                    if matches!(entry.receiver, Receiver::Entity(_))
                        && let Some(receiver) = refs.first(&format!("entries.{index}.receiver"))
                    {
                        entry.receiver = Receiver::Entity(receiver);
                    }
                    entry.credit = refs.first(&format!("entries.{index}.credit"));
                }
                chemistry.rebuild(true);
            },
            keys: |chemistry| queue_refs(chemistry).map(|(key, _)| key).collect(),
        })
}
