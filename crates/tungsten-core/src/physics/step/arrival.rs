//! Arrival pass (D-092): a body whose velocity the solve changed is clamped
//! against what that velocity reaches within the substep.

use super::Proxy;
use super::pairs::PAIR_REPAIRED;
use super::solver::narrow_phase_at;
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
    x: u32,
    y: u32,
    /// Points from `y` to `x`.
    normal: Vec2,
    /// Closing speed that brings the pair to touching within the substep; 0
    /// once it overlaps by more than the slop.
    allowance: f32,
    /// Push depth of the pair's mover.
    depth: u8,
    /// Inverse mass of the pair's target; 0 for a static or sleeping one.
    target_inv: f32,
}

impl ArrivalPair {
    /// Closing speed beyond the allowance; negative when the pair closes too
    /// fast.
    fn closing(&self, proxies: &[Proxy]) -> f32 {
        (proxies[self.x as usize].velocity - proxies[self.y as usize].velocity).dot(self.normal)
            + self.allowance
    }

    /// The pair's mover, the member whose own velocity closes it faster, its
    /// target, and the direction the mover travels to close it.
    fn mover(&self, proxies: &[Proxy]) -> (usize, usize, Vec2) {
        let toward_y = -proxies[self.x as usize].velocity.dot(self.normal);
        let toward_x = proxies[self.y as usize].velocity.dot(self.normal);
        if toward_x > toward_y {
            (self.y as usize, self.x as usize, self.normal)
        } else {
            (self.x as usize, self.y as usize, -self.normal)
        }
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

/// Inverse mass in the arrival pass: a static or sleeping body does not give
/// way.
fn arrival_inv(proxy: &Proxy) -> f32 {
    if proxy.is_dynamic && !proxy.sleeping {
        proxy.inv_mass
    } else {
        0.0
    }
}

/// Where a proxy was when the substep began. The pass runs after position
/// integration, which moved the awake dynamic ones.
fn arrival_center(proxy: &Proxy) -> Vec2 {
    if proxy.is_dynamic && !proxy.sleeping {
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
///    repair grid) that the narrow phase accepts at the D-064 margin, with
///    both bodies where the substep began.
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
/// wake test fires at the next narrow phase. Every loop runs in proxy, grid
/// or sorted order, so the pass is deterministic.
#[allow(clippy::too_many_arguments)]
pub(super) fn arrival_pass(
    config: &PhysicsConfig,
    sub_dt: f32,
    proxies: &mut [Proxy],
    grid: &mut SpatialGrid,
    repair_grid: &mut SpatialGrid,
    pair_flags: &[u8],
    repaired: &[u32],
    candidates: &mut Vec<ProxyId>,
    arrival: &mut ArrivalScratch,
) {
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
    } = arrival;
    let inv_h = 1.0 / sub_dt;
    let slack = 4.0 * config.linear_slop;
    let threshold = ARRIVAL_SLOPS * config.linear_slop * inv_h;
    let threshold_sq = threshold * threshold;
    let tight = ARRIVAL_TIGHT * inv_h;

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

        // The neighbours each body of this round can reach.
        pairs.clear();
        for (at, &body) in list[..round_end].iter().enumerate().skip(round_start) {
            let body = body as usize;
            let velocity = proxies[body].velocity;
            let center = proxies[body].prev_center;
            let half = proxies[body].half_extents();
            let mut aabb = Aabb::new(center, half);
            aabb.half_extents += Vec2::splat(velocity.length() * sub_dt + slack);
            // As in a pair repair: the pair grid holds a repaired proxy under
            // the cells of its old AABB, so those come from the repair grid.
            candidates.clear();
            grid.for_each_in(&aabb, Some(body as ProxyId), |id| {
                if pair_flags[id as usize] & PAIR_REPAIRED == 0 {
                    candidates.push(id);
                }
            });
            if !repaired.is_empty() {
                repair_grid.for_each_in(&aabb, Some(body as ProxyId), |id| candidates.push(id));
            }
            for &other_id in candidates.iter() {
                let other = other_id as usize;
                // Two bodies of one round share a pair: the one listed first
                // has it.
                if state[other] & ARRIVAL_LISTED != 0
                    && (round_start..at).contains(&(slot[other] as usize))
                {
                    continue;
                }
                let target = &proxies[other];
                let target_center = arrival_center(target);
                let relative = velocity - target.velocity;
                // A cell holds far more neighbours than the margin admits:
                // bounding boxes farther apart than it are dropped before
                // the square root and the narrow phase.
                let apart = ((target_center - center).abs() - half - target.half_extents())
                    .max_element()
                    - slack;
                if apart > 0.0 && apart * apart > relative.length_squared() * sub_dt * sub_dt {
                    continue;
                }
                let margin = relative.length() * sub_dt + slack;
                let Some(contact) =
                    narrow_phase_at(&proxies[body], center, target, target_center, margin)
                else {
                    continue;
                };
                let separation = config.linear_slop - contact.penetration;
                pairs.push(ArrivalPair {
                    x: body as u32,
                    y: other_id,
                    normal: contact.normal,
                    allowance: if separation > 0.0 {
                        separation * inv_h
                    } else {
                        0.0
                    },
                    depth: 0,
                    target_inv: 0.0,
                });
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
                let (mover, target, _) = pair.mover(proxies);
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
        for pair in pairs.iter_mut() {
            let (mover, target, _) = pair.mover(proxies);
            pair.depth = depth[mover];
            pair.target_inv = arrival_inv(&proxies[target]);
        }
        // Stable: pairs of one depth and target mass keep query order.
        pairs.sort_by(|p, q| {
            p.depth
                .cmp(&q.depth)
                .then(q.target_inv.total_cmp(&p.target_inv))
        });

        // Forward: each pair arrives, mass-weighted like the solver.
        for pair in pairs.iter() {
            let (x, y) = (pair.x as usize, pair.y as usize);
            let (inv_x, inv_y) = (arrival_inv(&proxies[x]), arrival_inv(&proxies[y]));
            let closing = pair.closing(proxies);
            if closing >= 0.0 || inv_x + inv_y <= 0.0 {
                continue;
            }
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
