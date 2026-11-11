//! Arrival pass (D-092): a body whose velocity the solve changed is clamped
//! against what that velocity reaches within the substep.

use super::pairs::PAIR_REPAIRED;
use super::pairs::PairGrid;
use super::solver::narrow_phase_at;
use super::{Proxy, ProxyLayers, TouchObs};
use crate::physics::PhysicsConfig;
use crate::physics::broadphase::{ProxyId, SpatialGrid};
use crate::physics::collision::Aabb;
use glam::Vec2;

/// Travel change per substep, in units of `linear_slop`, above which a body's
/// solved velocity counts as one the narrow phase did not admit (D-092). Two
/// bodies under it change a pair's closing travel by at most `4·linear_slop`,
/// the admission slack of D-064.
pub(super) const ARRIVAL_SLOPS: f32 = 2.0;

/// Rounds of the arrival pass per substep, and levels of push depth.
const ARRIVAL_ROUNDS: usize = 4;

/// Closing travel per substep, in px, within which an arrival pair counts as
/// tight: its mover is pressed on its target.
const ARRIVAL_TIGHT: f32 = 1.0e-3;

/// `ArrivalScratch::state` bit: the proxy is on the worklist this substep.
const ARRIVAL_LISTED: u8 = 1;
/// `state` bit: the proxy is pressed on another one this substep.
const ARRIVAL_PRESSED: u8 = 1 << 1;
/// `state` bit: the pass changed the proxy's velocity, so its move is redone.
const ARRIVAL_MOVED: u8 = 1 << 2;

/// A listed body and one neighbour its new velocity reaches (D-092).
#[derive(Debug, Clone, Copy)]
struct ArrivalPair {
    /// The halves of what orders the pair in its round ([`Self::order`]),
    /// which keep the pair at 28 bytes.
    order_hi: u32,
    order_lo: u32,
    x: u32,
    y: u32,
    /// Points from `y` to `x`.
    normal: Vec2,
    /// Closing speed that brings the pair to touching within the substep; 0
    /// once it overlaps by more than the slop.
    allowance: f32,
}

/// [`ArrivalPair::order`]'s lowest bit: `y` was the pair's mover when the
/// pair was built. Under the place, which no two pairs share, it orders
/// nothing.
const ORDER_Y_MOVED: u64 = 1;
/// Bits of `order` for the pair's place among the round's pairs as built,
/// above the mover's bit; [`sort_pairs`] sets them.
const ORDER_PLACE_BITS: u32 = 28;
/// Bits of `order` below the target's inverse mass rank.
const ORDER_RANK_SHIFT: u32 = ORDER_PLACE_BITS + 1;
/// Bits of `order` below the mover's push depth.
const ORDER_DEPTH_SHIFT: u32 = ORDER_RANK_SHIFT + 32;

/// `inv`'s rank among inverse masses, larger first: `f32::total_cmp`'s
/// order, inverted, as an unsigned integer. Equal ranks are equal bits.
pub(super) fn inv_rank(inv: f32) -> u32 {
    let bits = inv.to_bits() as i32;
    let ascending = (bits ^ (((bits >> 31) as u32) >> 1) as i32) as u32 ^ (1 << 31);
    !ascending
}

impl ArrivalPair {
    /// What [`sort_pairs`] orders the pair by, from the high bits down: its
    /// mover's push depth, its target's inverse mass rank (heavier later),
    /// its place among the round's pairs as built or its class there, and
    /// [`ORDER_Y_MOVED`].
    fn order(&self) -> u64 {
        (u64::from(self.order_hi) << 32) | u64::from(self.order_lo)
    }

    /// Closing speed beyond the allowance; negative when the pair closes too
    /// fast.
    fn closing(&self, proxies: &[Proxy]) -> f32 {
        (proxies[self.x as usize].velocity - proxies[self.y as usize].velocity).dot(self.normal)
            + self.allowance
    }

    /// The pair's mover, the member whose own velocity closes it faster, its
    /// target, and the direction the mover travels to close it.
    fn mover(&self, proxies: &[Proxy]) -> (usize, usize, Vec2) {
        let x_velocity = proxies[self.x as usize].velocity;
        let y_velocity = proxies[self.y as usize].velocity;
        if y_moves(x_velocity, y_velocity, self.normal) {
            (self.y as usize, self.x as usize, self.normal)
        } else {
            (self.x as usize, self.y as usize, -self.normal)
        }
    }

    /// The mover and the target as the pair was built ([`Self::mover`]
    /// then). The depth pass and the order read them, before any clamp of
    /// the round changes a velocity; the backward sweep asks again.
    fn built_ends(&self) -> (usize, usize) {
        if self.order() & ORDER_Y_MOVED != 0 {
            (self.y as usize, self.x as usize)
        } else {
            (self.x as usize, self.y as usize)
        }
    }
}

/// Whether `y`, not `x`, is the mover of a pair with `normal` from `y` to
/// `x`: its own velocity closes the pair faster.
fn y_moves(x_velocity: Vec2, y_velocity: Vec2, normal: Vec2) -> bool {
    let toward_y = -x_velocity.dot(normal);
    let toward_x = y_velocity.dot(normal);
    toward_x > toward_y
}

/// The place bits of [`ArrivalPair::order`], in its low half.
const ORDER_PLACE_MASK: u32 = ((1 << ORDER_PLACE_BITS) - 1) << 1;
/// Distinct depths and target masses [`distribute_pairs`] orders a round's
/// pairs by; a pile of like bodies has up to ten, five depths of movable
/// and immovable targets.
const ORDER_CLASSES: usize = 16;
/// A round of at most this many pairs is sorted in place.
const ORDER_SMALL: usize = 128;

/// Order a round's pairs pushers first, walls last: by mover depth, then
/// lighter targets first, and the pairs of one depth and target mass in the
/// order built, as a stable sort leaves them.
fn sort_pairs(pairs: &mut [ArrivalPair], depth: &[u8], reach: &mut ArrivalReach) {
    for pair in pairs.iter_mut() {
        let (mover, _) = pair.built_ends();
        pair.order_hi |= (u64::from(depth[mover]) << (ORDER_DEPTH_SHIFT - 32)) as u32;
    }
    // Tests: every path gives the stable sort's order.
    #[cfg(test)]
    let stable: Vec<(u32, u32)> = {
        let mut stable = pairs.to_vec();
        stable.sort_by_key(|pair| pair.order() >> ORDER_RANK_SHIFT);
        stable.iter().map(|pair| (pair.x, pair.y)).collect()
    };
    let path = if pairs.len() > ORDER_SMALL && distribute_pairs(pairs, &mut reach.unsorted) {
        1
    } else if pairs.len() < 1 << ORDER_PLACE_BITS {
        // No two pairs share an `order` with its place in it, so the
        // unstable sort gives the stable one's order with no buffer.
        for (place, pair) in pairs.iter_mut().enumerate() {
            pair.order_lo = (pair.order_lo & !ORDER_PLACE_MASK) | ((place as u32) << 1);
        }
        pairs.sort_unstable_by_key(ArrivalPair::order);
        usize::from(pairs.len() > ORDER_SMALL) * 2
    } else {
        pairs.sort_by_key(|pair| pair.order() >> ORDER_RANK_SHIFT);
        2
    };
    #[cfg(test)]
    {
        reach.sorts[path] += 1;
        assert!(
            pairs.iter().map(|pair| (pair.x, pair.y)).eq(stable),
            "an arrival round's pairs left the stable sort's order"
        );
    }
    #[cfg(not(test))]
    let _ = path;
}

/// [`sort_pairs`]' order for a round with at most [`ORDER_CLASSES`] distinct
/// depths and target masses, in two passes: count each class, then copy the
/// pairs out and back, each to its class's next place. False, with the
/// pairs in the order built, when the round has more classes.
fn distribute_pairs(pairs: &mut [ArrivalPair], scratch: &mut Vec<ArrivalPair>) -> bool {
    let mut keys = [0u64; ORDER_CLASSES];
    let mut counts = [0usize; ORDER_CLASSES];
    let mut classes = 0;
    for pair in pairs.iter_mut() {
        let key = pair.order() >> ORDER_RANK_SHIFT;
        let class = match keys[..classes].iter().position(|&known| known == key) {
            Some(class) => class,
            None if classes == ORDER_CLASSES => return false,
            None => {
                keys[classes] = key;
                classes += 1;
                classes - 1
            }
        };
        counts[class] += 1;
        // The place bits carry the pair's class to the second pass.
        pair.order_lo = (pair.order_lo & !ORDER_PLACE_MASK) | ((class as u32) << 1);
    }
    let mut next = [0usize; ORDER_CLASSES];
    for class in 0..classes {
        next[class] = (0..classes)
            .filter(|&other| keys[other] < keys[class])
            .map(|other| counts[other])
            .sum();
    }
    scratch.clear();
    scratch.extend_from_slice(pairs);
    for pair in scratch.iter() {
        let class = ((pair.order_lo & ORDER_PLACE_MASK) >> 1) as usize;
        pairs[next[class]] = *pair;
        next[class] += 1;
    }
    true
}

/// What the arrival pass's candidate cut reads of a proxy, in place of the
/// 80-byte [`Proxy`]: a listed body walks whole cells, several candidates
/// for each neighbour in reach.
#[derive(Debug, Clone, Copy, Default)]
struct Reach {
    /// Where the proxy was when the substep began ([`arrival_center`]).
    center: Vec2,
    half: Vec2,
    velocity: Vec2,
    /// The pass that filled the entry ([`ArrivalReach::pass`]).
    stamp: u32,
}

/// The arrival pass's scratch beside [`ArrivalScratch`], kept in the boxed
/// step state.
#[derive(Debug, Default)]
pub(super) struct ArrivalReach {
    /// Each proxy's entry for the cut, filled when a pass first meets it.
    of: Vec<Reach>,
    /// The running pass's stamp, never 0.
    pass: u32,
    /// Where each body of the round's pairs begin in the pairs, by its
    /// place in the round.
    from: Vec<u32>,
    /// A large round's pairs as built, while they are put in order.
    unsorted: Vec<ArrivalPair>,
    /// Tests: the rounds ordered in place as small, by class, and as large
    /// rounds of too many classes.
    #[cfg(test)]
    pub(super) sorts: [usize; 3],
}

impl ArrivalReach {
    /// Begin a pass over `len` proxies: no entry is filled.
    fn begin(&mut self, len: usize) {
        if self.of.len() != len || self.pass == u32::MAX {
            self.of.clear();
            self.of.resize(len, Reach::default());
            self.pass = 0;
        }
        self.pass += 1;
    }
}

/// Scratch of the arrival pass (D-092), kept between substeps and frames.
/// The per-proxy vectors are parallel to the proxies.
#[derive(Debug, Default)]
pub(super) struct ArrivalScratch {
    /// Velocity each awake dynamic proxy had when the narrow phase ran.
    pub(super) admitted: Vec<Vec2>,
    /// Proxies whose velocity the solve changed, in proxy order, then the ones
    /// a clamp pushed, in the order they joined.
    pub(super) list: Vec<u32>,
    /// A listed proxy's position in `list`.
    slot: Vec<u32>,
    pairs: Vec<ArrivalPair>,
    /// `ARRIVAL_*` bits; all zero outside the pass.
    state: Vec<u8>,
    /// Proxies with a nonzero `state`, so the reset touches only them.
    touched: Vec<u32>,
    /// Push depth, valid for the members of `pairs`.
    depth: Vec<u8>,
    /// For a pressed proxy: the direction it is pressed in, the proxy it is
    /// pressed on, and the inverse mass of the chain it heads (0 when the
    /// chain ends at a static or sleeping body).
    pressed_dir: Vec<Vec2>,
    pressed_on: Vec<u32>,
    chain_inv: Vec<f32>,
    /// Bodies listed and clamps applied since the buffers were created.
    #[cfg(test)]
    pub(super) listed: usize,
    #[cfg(test)]
    pub(super) clamps: usize,
    /// Candidates the listed bodies walked, those the cut let through, the
    /// searches for a pair another listed body built, and the pairs built.
    #[cfg(test)]
    pub(super) walked: usize,
    #[cfg(test)]
    pub(super) survivors: usize,
    #[cfg(test)]
    pub(super) searches: usize,
    #[cfg(test)]
    pub(super) built: usize,
}

impl ArrivalScratch {
    /// Size the per-proxy vectors for `len` proxies.
    pub(super) fn fit(&mut self, len: usize) {
        if self.admitted.len() != len {
            self.admitted.resize(len, Vec2::ZERO);
            self.slot.resize(len, 0);
            self.state.resize(len, 0);
            self.depth.resize(len, 0);
            self.pressed_dir.resize(len, Vec2::ZERO);
            self.pressed_on.resize(len, 0);
            self.chain_inv.resize(len, 0.0);
        }
    }

    /// Set `bit` on `index`, remembering the proxy for the reset.
    fn mark(state: &mut [u8], touched: &mut Vec<u32>, index: usize, bit: u8) {
        if state[index] == 0 {
            touched.push(index as u32);
        }
        state[index] |= bit;
    }
}

/// Inverse mass in the arrival pass: a static, kinematic or sleeping body
/// does not give way.
fn arrival_inv(proxy: &Proxy) -> f32 {
    if proxy.is_dynamic && !proxy.sleeping {
        proxy.inv_mass
    } else {
        0.0
    }
}

/// Whether the round's `pairs` hold the one that the body at `place` in the
/// round built with `body`. Each body pushes its pairs, itself first, as it
/// is reached in list order, so they are the run from its entry of `from`
/// to the next body's.
fn built_pair(pairs: &[ArrivalPair], from: &[u32], place: usize, body: u32) -> bool {
    pairs[from[place] as usize..from[place + 1] as usize]
        .iter()
        .any(|pair| pair.y == body)
}

/// Where a proxy was when the substep began. The pass runs after position
/// integration, which moved the awake dynamic and the kinematic ones.
fn arrival_center(proxy: &Proxy) -> Vec2 {
    if proxy.moves() {
        proxy.prev_center
    } else {
        proxy.center
    }
}

/// Arrival pass (D-092): a body whose velocity the solve just changed is
/// clamped against whatever that velocity reaches in this substep, and its
/// move is redone with the clamped velocity.
///
/// The narrow phase admits contacts for the velocities it sees. A body the
/// solver then accelerates, such as a resting ball hit by a fast one, may
/// have no pair with the wall behind it, or a contact with it that was solved
/// before the push arrived: with one velocity iteration the contact solved
/// last has the last word. Position integration would then carry the body
/// through the wall, at once or a few pixels per substep.
///
/// 1. Position integration lists each awake body whose velocity differs
///    from the admitted one by more than `ARRIVAL_SLOPS · linear_slop` of
///    travel. The pass runs only when it listed one.
/// 2. Each listed body is paired with every neighbour within reach of its
///    new velocity (static, sleeping or dynamic; from the pair grid and the
///    repair grid) whose layers meet its own and that the narrow phase
///    accepts at the D-064 margin, with both bodies where the substep began.
/// 3. The pairs are ordered pushers first, walls last: by the push depth of
///    each pair's mover, then lighter targets first.
/// 4. Forward sweep: a pair closing faster than its gap allows takes the
///    impulse that removes the excess, split by inverse mass. A neighbour
///    this pushes past the threshold joins the list for the next round.
/// 5. Backward sweep: a mover left pressed on its target moves with it, so a
///    pusher still closing on a pressed body is clamped against the pressed
///    chain as one body. A chain that ends at a static or sleeping body
///    takes no impulse, and the pusher stops behind it.
///
/// The pair list, the contacts, their impulses and the events are left
/// alone; the regular repair (D-081) picks a new pair up before the next
/// narrow phase. A clamp is inelastic, and a sleeper is immovable here: the
/// wake test fires at the next narrow phase. With `touches`, the contact
/// record's, each clamp is a touch of its pair, at the closing speed it
/// found. Every loop runs in proxy, grid or sorted order, so the pass is
/// deterministic.
#[allow(clippy::too_many_arguments)]
pub(super) fn arrival_pass(
    config: &PhysicsConfig,
    sub_dt: f32,
    proxies: &mut [Proxy],
    grid: &mut PairGrid,
    repair_grid: &mut SpatialGrid,
    pair_flags: &[u8],
    layers: &ProxyLayers,
    repaired: &[u32],
    oversize: &[u32],
    candidates: &mut Vec<ProxyId>,
    arrival: &mut ArrivalScratch,
    reach: &mut ArrivalReach,
    mut touches: Option<&mut Vec<TouchObs>>,
) {
    let mut touch = |pair: &ArrivalPair, proxies: &[Proxy]| {
        if let Some(touches) = touches.as_deref_mut() {
            let approach = (proxies[pair.x as usize].velocity - proxies[pair.y as usize].velocity)
                .dot(pair.normal);
            touches.push(TouchObs {
                a: pair.x,
                b: pair.y,
                normal: pair.normal,
                speed: (-approach).max(0.0),
            });
        }
    };
    let ArrivalScratch {
        admitted,
        list,
        slot,
        pairs,
        state,
        touched,
        depth,
        pressed_dir,
        pressed_on,
        chain_inv,
        #[cfg(test)]
        listed,
        #[cfg(test)]
        clamps,
        #[cfg(test)]
        walked,
        #[cfg(test)]
        survivors,
        #[cfg(test)]
        searches,
        #[cfg(test)]
        built,
    } = arrival;
    let inv_h = 1.0 / sub_dt;
    let slack = 4.0 * config.linear_slop;
    let threshold = ARRIVAL_SLOPS * config.linear_slop * inv_h;
    let threshold_sq = threshold * threshold;
    let tight = ARRIVAL_TIGHT * inv_h;
    let layered = layers.layered;
    reach.begin(proxies.len());
    let pass = reach.pass;

    for (at, &index) in list.iter().enumerate() {
        slot[index as usize] = at as u32;
        ArrivalScratch::mark(state, touched, index as usize, ARRIVAL_LISTED);
    }

    let mut round_start = 0;
    for _ in 0..ARRIVAL_ROUNDS {
        let round_end = list.len();
        if round_start == round_end {
            break;
        }
        #[cfg(test)]
        {
            *listed += round_end - round_start;
        }

        // The cut's entries follow the velocities the earlier rounds' clamps
        // changed.
        if round_start > 0 {
            for &index in touched.iter() {
                if state[index as usize] & ARRIVAL_MOVED != 0 {
                    reach.of[index as usize].velocity = proxies[index as usize].velocity;
                }
            }
        }

        // The neighbours each body of this round can reach.
        pairs.clear();
        reach.from.clear();
        for (at, &body) in list[..round_end].iter().enumerate().skip(round_start) {
            reach.from.push(pairs.len() as u32);
            let body = body as usize;
            let velocity = proxies[body].velocity;
            let center = proxies[body].prev_center;
            let half = proxies[body].half_extents();
            let mut aabb = Aabb::new(center, half);
            aabb.half_extents += Vec2::splat(velocity.length() * sub_dt + slack);
            // As in a pair repair: the pair grid holds a repaired proxy under
            // the cells of its old AABB, so those come from the repair grid.
            candidates.clear();
            grid.for_each_run_in(
                &aabb,
                #[inline(always)]
                |mut run| {
                    run.each(Some(body as ProxyId), |id| {
                        if pair_flags[id as usize] & PAIR_REPAIRED == 0 {
                            candidates.push(id);
                        }
                    });
                },
            );
            if !repaired.is_empty() {
                repair_grid.for_each_run_in(
                    &aabb,
                    0,
                    #[inline(always)]
                    |mut run| {
                        run.each(Some(body as ProxyId), |id| candidates.push(id));
                    },
                );
            }
            // An oversize proxy is in no grid (D-081): every one is a
            // candidate.
            candidates.extend(oversize.iter().filter(|&&id| id as usize != body));
            #[cfg(test)]
            {
                *walked += candidates.len();
            }
            // A cell holds far more neighbours than the margin admits:
            // bounding boxes farther apart than it are dropped before the
            // square root and the narrow phase, and so is a neighbour the
            // body's layers do not meet. Which candidates stay is data the
            // branch predictor cannot learn, so the cut marks a bit for
            // each of 64 and the rest runs over the bits set, in order.
            for chunk in candidates.chunks(64) {
                let mut keep = 0u64;
                for (bit, &other_id) in chunk.iter().enumerate() {
                    let other = other_id as usize;
                    let entry = &mut reach.of[other];
                    if entry.stamp != pass {
                        let target = &proxies[other];
                        *entry = Reach {
                            center: arrival_center(target),
                            half: target.half_extents(),
                            velocity: target.velocity,
                            stamp: pass,
                        };
                    }
                    let relative = velocity - entry.velocity;
                    let apart =
                        ((entry.center - center).abs() - half - entry.half).max_element() - slack;
                    let cut = ((apart > 0.0)
                        & (apart * apart > relative.length_squared() * sub_dt * sub_dt))
                        | (layered && !layers.of[body].meets(layers.of[other]));
                    keep |= u64::from(!cut) << bit;
                }
                #[cfg(test)]
                {
                    *survivors += keep.count_ones() as usize;
                }
                while keep != 0 {
                    let other_id = chunk[keep.trailing_zeros() as usize];
                    keep &= keep - 1;
                    let other = other_id as usize;
                    let entry = &reach.of[other];
                    let target_center = entry.center;
                    let target_velocity = entry.velocity;
                    let relative = velocity - target_velocity;
                    // Two bodies of one round share a pair: the one listed
                    // first has it, when its own reach found this one; a
                    // slower body's smaller box can miss it. The search for
                    // that pair comes after the cheap cuts.
                    if state[other] & ARRIVAL_LISTED != 0
                        && (round_start..at).contains(&(slot[other] as usize))
                    {
                        #[cfg(test)]
                        {
                            *searches += 1;
                        }
                        let place = slot[other] as usize - round_start;
                        if built_pair(pairs, &reach.from, place, body as u32) {
                            continue;
                        }
                    }
                    let margin = relative.length() * sub_dt + slack;
                    let target = &proxies[other];
                    let Some(contact) =
                        narrow_phase_at(&proxies[body], center, target, target_center, margin)
                    else {
                        continue;
                    };
                    let separation = config.linear_slop - contact.penetration;
                    // No clamp runs before the round's pairs are ordered, so
                    // the mover and its target's inverse mass are settled
                    // here.
                    let y_moved = y_moves(velocity, target_velocity, contact.normal);
                    let target_inv = arrival_inv(if y_moved { &proxies[body] } else { target });
                    let order = (u64::from(inv_rank(target_inv)) << ORDER_RANK_SHIFT)
                        | if y_moved { ORDER_Y_MOVED } else { 0 };
                    pairs.push(ArrivalPair {
                        order_hi: (order >> 32) as u32,
                        order_lo: order as u32,
                        x: body as u32,
                        y: other_id,
                        normal: contact.normal,
                        allowance: if separation > 0.0 {
                            separation * inv_h
                        } else {
                            0.0
                        },
                    });
                    #[cfg(test)]
                    {
                        *built += 1;
                    }
                }
            }
        }
        round_start = round_end;
        if pairs.is_empty() {
            continue;
        }

        // Pushers first, walls last. A target sits one level deeper than its
        // mover; the bound on the levels also ends a cycle.
        for pair in pairs.iter() {
            depth[pair.x as usize] = 0;
            depth[pair.y as usize] = 0;
        }
        for _ in 0..ARRIVAL_ROUNDS {
            let mut changed = false;
            for pair in pairs.iter() {
                let (mover, target) = pair.built_ends();
                let next = depth[mover] + 1;
                if depth[target] < next && next as usize <= ARRIVAL_ROUNDS {
                    depth[target] = next;
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        // Pairs of one depth and target mass keep query order.
        sort_pairs(pairs, depth, reach);

        // Forward: each pair arrives, mass-weighted like the solver.
        for pair in pairs.iter() {
            let (x, y) = (pair.x as usize, pair.y as usize);
            let (inv_x, inv_y) = (arrival_inv(&proxies[x]), arrival_inv(&proxies[y]));
            let closing = pair.closing(proxies);
            if closing >= 0.0 || inv_x + inv_y <= 0.0 {
                continue;
            }
            touch(pair, proxies);
            let lambda = -closing / (inv_x + inv_y);
            proxies[x].velocity += pair.normal * (lambda * inv_x);
            proxies[y].velocity -= pair.normal * (lambda * inv_y);
            #[cfg(test)]
            {
                *clamps += 1;
            }
            for body in [x, y] {
                if arrival_inv(&proxies[body]) == 0.0 {
                    continue;
                }
                ArrivalScratch::mark(state, touched, body, ARRIVAL_MOVED);
                // The clamp can hand a neighbour a velocity nobody admitted.
                if state[body] & ARRIVAL_LISTED == 0
                    && (proxies[body].velocity - admitted[body]).length_squared() > threshold_sq
                {
                    slot[body] = list.len() as u32;
                    ArrivalScratch::mark(state, touched, body, ARRIVAL_LISTED);
                    list.push(body as u32);
                }
            }
        }

        // Backward, walls first: a pair that is tight, or still closing too
        // fast, presses its mover on its target.
        for pair in pairs.iter().rev() {
            let closing = pair.closing(proxies);
            if closing > tight {
                continue;
            }
            let (mover, target, dir) = pair.mover(proxies);
            let inv_mover = arrival_inv(&proxies[mover]);
            if inv_mover == 0.0 {
                continue;
            }
            // What the mover is clamped against: the target alone, or the
            // chain the target heads when it is pressed the same way.
            let inv_target = arrival_inv(&proxies[target]);
            let pressed_along = |index: usize, state: &[u8], pressed_dir: &[Vec2]| {
                state[index] & ARRIVAL_PRESSED != 0 && pressed_dir[index].dot(dir) > 0.5
            };
            let inv_chain = if inv_target == 0.0 {
                0.0
            } else if pressed_along(target, state, pressed_dir) {
                chain_inv[target]
            } else {
                inv_target
            };
            if closing < 0.0 {
                touch(pair, proxies);
                let lambda = -closing / (inv_mover + inv_chain);
                proxies[mover].velocity -= dir * (lambda * inv_mover);
                ArrivalScratch::mark(state, touched, mover, ARRIVAL_MOVED);
                if inv_chain > 0.0 {
                    // Every member takes the same change, so the chain stays
                    // pressed together.
                    let gain = dir * (lambda * inv_chain);
                    let mut member = target;
                    for _ in 0..=touched.len() {
                        proxies[member].velocity += gain;
                        ArrivalScratch::mark(state, touched, member, ARRIVAL_MOVED);
                        let next = pressed_on[member] as usize;
                        if !pressed_along(member, state, pressed_dir)
                            || arrival_inv(&proxies[next]) == 0.0
                        {
                            break;
                        }
                        member = next;
                    }
                }
                #[cfg(test)]
                {
                    *clamps += 1;
                }
            }
            ArrivalScratch::mark(state, touched, mover, ARRIVAL_PRESSED);
            pressed_dir[mover] = dir;
            pressed_on[mover] = target as u32;
            chain_inv[mover] = if inv_chain == 0.0 {
                0.0
            } else {
                1.0 / (1.0 / inv_mover + 1.0 / inv_chain)
            };
        }
    }

    // Redo the move of every body the pass slowed or pushed.
    for &index in touched.iter() {
        let index = index as usize;
        if state[index] & ARRIVAL_MOVED != 0 {
            let proxy = &mut proxies[index];
            proxy.center = proxy.prev_center + proxy.velocity * sub_dt;
        }
        state[index] = 0;
    }
    touched.clear();
}
