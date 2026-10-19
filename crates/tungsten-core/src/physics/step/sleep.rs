//! Island sleeping (D-065): the per-body sleep table parallel to the proxies
//! (D-082), union-find over touching contacts, and the frame-start and
//! frame-end sleep passes.

use super::{Layers, PhysicsBuffers, Proxy};
use crate::ecs::World;
use crate::physics::PhysicsConfig;
use crate::physics::collision::Aabb;
use crate::physics::components::Velocity;
use glam::Vec2;

fn hash_key(key: u64) -> u64 {
    let h = key.wrapping_mul(0xD6E8_FEB8_6659_FD93);
    h ^ (h >> 32)
}

/// Per-body sleep bookkeeping (D-065); persists across frames in a side
/// table, never a component.
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct SleepEntry {
    /// Consecutive seconds below the sleep velocity threshold.
    pub(super) timer: f32,
    pub(super) sleeping: bool,
    /// Island tag at sleep time (a member entity key); islands wake as a
    /// unit on despawn or external write.
    pub(super) island: u64,
    /// Collider center frozen at sleep, stored with the exact fp ops the
    /// next gather performs; a mismatch means an external `Position` write.
    center: Vec2,
    /// Layers and mask at sleep; a mismatch is a write that wakes, as a
    /// moved centre is.
    layers: Layers,
}

/// Key of a proxy without sleep state, and the empty index slot; entity keys
/// always set bit 63 (`entity_key`), so 0 is never a real key.
const EMPTY_SLEEP: u64 = 0;

/// Per-body sleep state in arrays parallel to the proxy array (D-082).
///
/// While a frame's proxies are last frame's bodies in last frame's order,
/// every entry already sits at its proxy's index and nothing is rebuilt.
/// When the sequence differs (a spawn, a despawn, a body or collider change),
/// entries carry over by entity key and sleepers that did not carry over wake
/// their island. Never `std::HashMap` in the physics hot path (D-062
/// precedent).
#[derive(Debug, Default)]
pub(super) struct SleepTable {
    /// Entity key of each dynamic entity proxy; `EMPTY_SLEEP` for the rest.
    pub(super) keys: Vec<u64>,
    pub(super) entries: Vec<SleepEntry>,
    /// Open-addressing slots from entity key to index in the arrays above;
    /// refilled only when the key sequence changes.
    slot_keys: Vec<u64>,
    slot_indices: Vec<u32>,
    mask: u64,
    /// Write side of a rebuild; swapped into `keys` and `entries`.
    next_keys: Vec<u64>,
    next_entries: Vec<SleepEntry>,
    /// Scratch: old entries a rebuild carried over.
    carried: Vec<bool>,
    /// The static and tile proxies at the end of the last frame in which an
    /// island slept, in proxy order: key, box and layers. Only dynamic
    /// members carry sleep entries, so a support that went, moved or changed
    /// is found by comparing this with the next frame's (`wake_on_supports`).
    supports: Vec<Support>,
    /// `supports` matches the last frame's proxies; false while no island
    /// sleeps, when nothing is recorded.
    supports_valid: bool,
    /// Scratch: supports whose box or layers changed, by index, with the
    /// new ones.
    moved_supports: Vec<(usize, Support)>,
    /// Scratch: the old and new boxes of the supports that changed.
    changed_supports: Vec<Aabb>,
    /// Scratch for the sorted comparison when supports appear or go.
    sorted_old: Vec<Support>,
    sorted_new: Vec<Support>,
    #[cfg(test)]
    pub(super) rebuilds: usize,
}

/// A proxy sleepers can rest on that holds no sleep entry: a static entity
/// or a tile. Kinematic bodies wake what they press (`D-154`).
fn is_support(proxy: &Proxy) -> bool {
    !proxy.is_dynamic && !proxy.kinematic
}

/// A sleeper's gathered state differs from the state it froze with: an
/// external `Position` or `Velocity` write, or new layers or a new mask.
fn written(proxy: &Proxy, layers: Layers, entry: &SleepEntry) -> bool {
    proxy.velocity != Vec2::ZERO || proxy.center != entry.center || layers != entry.layers
}

/// A support as the sleep record keeps it: a change to its box or its
/// layers wakes the sleepers near it.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Support {
    key: u64,
    aabb: Aabb,
    layers: Layers,
}

impl Support {
    fn of(proxy: &Proxy, layers: Layers) -> Self {
        Self {
            key: proxy.key,
            aabb: proxy.world_aabb(),
            layers,
        }
    }
}

/// A total order on a support for the sorted comparison: key, then the box's
/// bits, then the layers.
fn support_order(a: &Support, b: &Support) -> std::cmp::Ordering {
    let bits = |support: &Support| {
        (
            support.key,
            support.aabb.center.x.to_bits(),
            support.aabb.center.y.to_bits(),
            support.aabb.half_extents.x.to_bits(),
            support.aabb.half_extents.y.to_bits(),
            support.layers.layers,
            support.layers.mask,
        )
    };
    bits(a).cmp(&bits(b))
}

impl SleepTable {
    /// The key a proxy's entry lives under: only dynamic entity bodies sleep.
    fn key_of(proxy: &Proxy) -> u64 {
        if proxy.is_dynamic && proxy.entity.is_some() {
            proxy.key
        } else {
            EMPTY_SLEEP
        }
    }

    pub(super) fn index_of(&self, key: u64) -> Option<usize> {
        if self.slot_keys.is_empty() {
            return None;
        }
        let mut i = (hash_key(key) & self.mask) as usize;
        loop {
            if self.slot_keys[i] == EMPTY_SLEEP {
                return None;
            }
            if self.slot_keys[i] == key {
                return Some(self.slot_indices[i] as usize);
            }
            i = (i + 1) & self.mask as usize;
        }
    }

    pub(super) fn get(&self, key: u64) -> Option<&SleepEntry> {
        self.index_of(key).map(|i| &self.entries[i])
    }

    /// True when `proxies` are last frame's bodies in last frame's order, so
    /// the entries need no rebuild. Collects the islands of sleepers with an
    /// external write (a sleeper's `Position`/`Velocity` are frozen by the
    /// step, so any mismatch at gather time is one, as is a change to its
    /// layers or mask).
    fn same_bodies(
        &self,
        proxies: &[Proxy],
        layers: &[Layers],
        wake_islands: &mut Vec<u64>,
    ) -> bool {
        if proxies.len() != self.keys.len() {
            return false;
        }
        for (index, ((proxy, &key), entry)) in proxies
            .iter()
            .zip(&self.keys)
            .zip(&self.entries)
            .enumerate()
        {
            if Self::key_of(proxy) != key {
                return false;
            }
            if entry.sleeping && written(proxy, layers[index], entry) {
                wake_islands.push(entry.island);
            }
        }
        true
    }

    /// Carry entries over by entity key into the order of `proxies`. Sleeping
    /// entries that do not carry over (despawn or component removal) wake
    /// their island: bodies above a removed support must fall.
    fn rebuild(&mut self, proxies: &[Proxy], layers: &[Layers], wake_islands: &mut Vec<u64>) {
        self.next_keys.clear();
        self.next_entries.clear();
        self.carried.clear();
        self.carried.resize(self.entries.len(), false);
        let mut members = 0_usize;
        for (proxy, &proxy_layers) in proxies.iter().zip(layers) {
            let key = Self::key_of(proxy);
            let mut entry = SleepEntry::default();
            if key != EMPTY_SLEEP {
                members += 1;
                if let Some(old) = self.index_of(key) {
                    self.carried[old] = true;
                    entry = self.entries[old];
                    if entry.sleeping && written(proxy, proxy_layers, &entry) {
                        wake_islands.push(entry.island);
                    }
                }
            }
            self.next_keys.push(key);
            self.next_entries.push(entry);
        }
        for (entry, &carried) in self.entries.iter().zip(&self.carried) {
            if entry.sleeping && !carried {
                wake_islands.push(entry.island);
            }
        }
        std::mem::swap(&mut self.keys, &mut self.next_keys);
        std::mem::swap(&mut self.entries, &mut self.next_entries);

        // Refill the index at <= 0.5 load factor.
        let capacity = (members * 2).next_power_of_two().max(64);
        self.mask = capacity as u64 - 1;
        self.slot_keys.clear();
        self.slot_keys.resize(capacity, EMPTY_SLEEP);
        self.slot_indices.clear();
        self.slot_indices.resize(capacity, 0);
        for (index, &key) in self.keys.iter().enumerate() {
            if key == EMPTY_SLEEP {
                continue;
            }
            let mut i = (hash_key(key) & self.mask) as usize;
            while self.slot_keys[i] != EMPTY_SLEEP {
                i = (i + 1) & self.mask as usize;
            }
            self.slot_keys[i] = key;
            self.slot_indices[i] = index as u32;
        }
        #[cfg(test)]
        {
            self.rebuilds += 1;
        }
    }

    /// Wake every sleeping member of the given island tags.
    pub(super) fn wake_islands(&mut self, islands: &[u64]) {
        for entry in &mut self.entries {
            if entry.sleeping && islands.contains(&entry.island) {
                entry.sleeping = false;
                entry.timer = 0.0;
            }
        }
    }

    /// Rewrite island tag `from` to `to` (sleeping-island merge).
    fn retag(&mut self, from: u64, to: u64) {
        for entry in &mut self.entries {
            if entry.sleeping && entry.island == from {
                entry.island = to;
            }
        }
    }

    pub(super) fn sleeping_count(&self) -> usize {
        self.entries.iter().filter(|entry| entry.sleeping).count()
    }

    /// Brings the support record up to `proxies` and collects into
    /// `changed_supports` the old and new box of every support that moved,
    /// changed shape or changed its layers or mask, and the box of every one
    /// that went or appeared. While the supports keep their keys in order,
    /// they compare in place.
    fn diff_supports(&mut self, proxies: &[Proxy], layers: &[Layers]) {
        self.changed_supports.clear();
        self.moved_supports.clear();
        let mut count = 0;
        let mut same_keys = true;
        for (proxy, &proxy_layers) in proxies
            .iter()
            .zip(layers)
            .filter(|(proxy, _)| is_support(proxy))
        {
            match self.supports.get(count) {
                Some(old) if old.key == proxy.key => {
                    let new = Support::of(proxy, proxy_layers);
                    if *old != new {
                        self.moved_supports.push((count, new));
                    }
                }
                _ => {
                    same_keys = false;
                    break;
                }
            }
            count += 1;
        }
        if same_keys && count == self.supports.len() {
            for &(index, new) in &self.moved_supports {
                self.changed_supports.push(self.supports[index].aabb);
                self.changed_supports.push(new.aabb);
                self.supports[index] = new;
            }
            return;
        }
        // A support appeared or went: every entry in only one of the two
        // sorted lists changed.
        self.sorted_old.clear();
        self.sorted_old.extend_from_slice(&self.supports);
        self.record_supports(proxies, layers);
        self.sorted_new.clear();
        self.sorted_new.extend_from_slice(&self.supports);
        self.sorted_old.sort_unstable_by(support_order);
        self.sorted_new.sort_unstable_by(support_order);
        let (mut i, mut j) = (0, 0);
        while i < self.sorted_old.len() || j < self.sorted_new.len() {
            let order = match (self.sorted_old.get(i), self.sorted_new.get(j)) {
                (Some(old), Some(new)) => support_order(old, new),
                (Some(_), None) => std::cmp::Ordering::Less,
                _ => std::cmp::Ordering::Greater,
            };
            match order {
                std::cmp::Ordering::Less => {
                    self.changed_supports.push(self.sorted_old[i].aabb);
                    i += 1;
                }
                std::cmp::Ordering::Greater => {
                    self.changed_supports.push(self.sorted_new[j].aabb);
                    j += 1;
                }
                std::cmp::Ordering::Equal => {
                    i += 1;
                    j += 1;
                }
            }
        }
    }

    /// Records this frame's supports, in proxy order.
    fn record_supports(&mut self, proxies: &[Proxy], layers: &[Layers]) {
        self.supports.clear();
        self.supports.extend(
            proxies
                .iter()
                .zip(layers)
                .filter(|(proxy, _)| is_support(proxy))
                .map(|(proxy, &proxy_layers)| Support::of(proxy, proxy_layers)),
        );
    }
}

/// Union-find root with path halving; serial and deterministic (D-065).
fn uf_find(parent: &mut [u32], mut i: u32) -> u32 {
    while parent[i as usize] != i {
        let grandparent = parent[parent[i as usize] as usize];
        parent[i as usize] = grandparent;
        i = grandparent;
    }
    i
}

/// True when the island of `a`, as this frame's unions have built it so far,
/// holds one of `members`.
pub(super) fn island_holds_any(parent: &mut [u32], a: u32, members: &[u32]) -> bool {
    let root = uf_find(parent, a);
    members
        .iter()
        .any(|&member| uf_find(parent, member) == root)
}

/// Min-index root keeps island roots independent of union order.
pub(super) fn uf_union(parent: &mut [u32], a: u32, b: u32) {
    let root_a = uf_find(parent, a);
    let root_b = uf_find(parent, b);
    if root_a != root_b {
        let (lo, hi) = if root_a < root_b {
            (root_a, root_b)
        } else {
            (root_b, root_a)
        };
        parent[hi as usize] = lo;
    }
}

/// Frame-start sleep bookkeeping (D-065): line the persistent sleep state up
/// with this frame's proxies (D-082), waking islands whose members despawned
/// (or lost their dynamic body/collider), islands with an external write or
/// a member whose layers or mask changed, and islands within `margin` of a
/// static or tile support that went, moved or changed, its layers and mask
/// included, since the last frame.
pub(super) fn sleep_frame_start(buffers: &mut PhysicsBuffers, margin: f32) {
    let PhysicsBuffers {
        proxies,
        layers,
        sleep,
        wake_islands,
        ..
    } = buffers;
    let layers = &layers.of;

    wake_islands.clear();
    if !sleep.same_bodies(proxies, layers, wake_islands) {
        wake_islands.clear();
        sleep.rebuild(proxies, layers, wake_islands);
    }
    if sleep.supports_valid {
        sleep.diff_supports(proxies, layers);
        if !sleep.changed_supports.is_empty() {
            let reach = Vec2::splat(margin);
            for (proxy, entry) in proxies.iter().zip(&sleep.entries) {
                if !entry.sleeping {
                    continue;
                }
                let body = proxy.world_aabb();
                if sleep.changed_supports.iter().any(|support| {
                    Aabb::new(support.center, support.half_extents + reach).overlaps(&body)
                }) {
                    wake_islands.push(entry.island);
                }
            }
        }
    }
    if !wake_islands.is_empty() {
        sleep.wake_islands(wake_islands);
    }

    // Proxies persist across the frame's substeps (D-066), so the flag set
    // here carries through until a contact wake flips it. Statics and tiles
    // hold a default entry, which never sleeps.
    for (proxy, entry) in proxies.iter_mut().zip(&sleep.entries) {
        proxy.sleeping = entry.sleeping;
    }
}

/// Frame-end sleep pass (D-065): update per-body low-velocity timers from
/// end-of-frame velocities, then sleep every island whose slowest member has
/// been below the threshold for `time_to_sleep`, and this frame at least, so
/// a `time_to_sleep` of 0 or less never sleeps a moving body. A member that touched a
/// moving kinematic body this frame restarts its timer and keeps its island
/// awake, whatever `time_to_sleep` is. Members freeze with zeroed
/// velocity. An island falling asleep while resting on an already sleeping
/// island adopts that island's tag, so a later island wake (despawn deep in a
/// pile) also lifts bodies that settled on top after it slept; bridging two
/// sleeping islands merges their tags.
pub(super) fn sleep_frame_end(
    world: &mut World,
    config: &PhysicsConfig,
    dt: f32,
    buffers: &mut PhysicsBuffers,
) {
    let PhysicsBuffers {
        proxies,
        layers,
        contacts,
        sleep,
        island_parent,
        island_min_timer,
        island_tag,
        ..
    } = buffers;
    let layers = &layers.of;

    debug_assert_eq!(
        sleep.entries.len(),
        proxies.len(),
        "sleep table not lined up"
    );
    let threshold_sq = config.sleep_threshold * config.sleep_threshold;
    island_min_timer.clear();
    island_min_timer.resize(proxies.len(), f32::INFINITY);
    island_tag.clear();
    island_tag.resize(proxies.len(), 0);

    for (index, proxy) in proxies.iter().enumerate() {
        if !proxy.is_dynamic || proxy.sleeping || proxy.entity.is_none() {
            continue;
        }
        let entry = &mut sleep.entries[index];
        entry.timer = if proxy.velocity.length_squared() <= threshold_sq && !proxy.kept_awake {
            entry.timer + dt
        } else {
            0.0
        };
        let root = uf_find(island_parent, index as u32) as usize;
        let timer = if proxy.kept_awake {
            f32::NEG_INFINITY
        } else {
            entry.timer
        };
        island_min_timer[root] = island_min_timer[root].min(timer);
    }

    // Tag adoption from the final substep's resting contacts vs sleepers.
    for contact in contacts.iter() {
        if contact.penetration <= 0.0 {
            continue;
        }
        let b_idx = contact.b as usize;
        if !proxies[b_idx].sleeping || proxies[b_idx].entity.is_none() {
            continue;
        }
        let root = uf_find(island_parent, contact.a) as usize;
        if !ready_to_sleep(island_min_timer[root], config) {
            continue;
        }
        let tag = sleep.entries[b_idx].island;
        let current = island_tag[root];
        if current == 0 {
            island_tag[root] = tag;
        } else if current != tag {
            sleep.retag(tag, current);
            for slot in island_tag.iter_mut() {
                if *slot == tag {
                    *slot = current;
                }
            }
        }
    }

    for index in 0..proxies.len() {
        let proxy = proxies[index];
        if !proxy.is_dynamic || proxy.sleeping {
            continue;
        }
        let Some(entity) = proxy.entity else { continue };
        let root = uf_find(island_parent, index as u32) as usize;
        if !ready_to_sleep(island_min_timer[root], config) {
            continue;
        }
        let tag = if island_tag[root] != 0 {
            island_tag[root]
        } else {
            proxies[root].key
        };
        let entry = &mut sleep.entries[index];
        entry.sleeping = true;
        entry.island = tag;
        // Same fp ops the next gather performs on the written-back
        // Position, so an untouched body compares bit-equal.
        entry.center = (proxy.center - proxy.offset) + proxy.offset;
        entry.layers = layers[index];
        if let Some(vel) = world.get_mut::<Velocity>(entity) {
            vel.0 = Vec2::ZERO;
        }
    }

    // The supports the sleepers rest among, for the next frame's start; the
    // start's comparison already brought a valid record up to date.
    if sleep.entries.iter().any(|entry| entry.sleeping) {
        if !sleep.supports_valid {
            sleep.record_supports(proxies, layers);
            sleep.supports_valid = true;
        }
    } else {
        sleep.supports_valid = false;
    }
}

/// Whether an island whose slowest member's timer is `min_timer` sleeps: every
/// member below the threshold this frame (a timer above 0) and for
/// `time_to_sleep`.
fn ready_to_sleep(min_timer: f32, config: &PhysicsConfig) -> bool {
    min_timer > 0.0 && min_timer >= config.time_to_sleep
}
