//! Safety-net slab sweep against statics (D-064) and the statics-only grid
//! that answers it once a frame's queries outnumber the statics (D-080).

use super::Proxy;
use crate::physics::PhysicsConfig;
use crate::physics::broadphase::{ProxyId, SpatialGrid};
use crate::physics::collision::{FACE_BOTTOM, FACE_LEFT, FACE_RIGHT, FACE_TOP, sweep_aabb_vs_aabb};
use crate::physics::components::Shape;
use crate::physics::events::CollisionEvent;
use glam::Vec2;

/// Statics-only grid for the safety-net sweep (D-080).
///
/// It returns the static candidates a query of the pair grid returns, in the
/// same order, so which grid answers a sweep query is a matter of cost alone.
/// Staging one static costs about as much as one query of the pair grid, so
/// the sweep rents the pair grid until the frame's sweep queries reach the
/// static count and then stages this grid for the rest of the frame. A tile
/// map with a few fast bodies never stages it; thousands of fast bodies
/// between a few walls stage it after a handful of queries.
#[derive(Debug, Default)]
pub(super) struct SweepGrid {
    pub(super) grid: SpatialGrid,
    pub(super) staged: bool,
    /// Sweep queries the pair grid answered this frame.
    pub(super) pair_queries: usize,
    /// Static proxies, counted by every pair build.
    pub(super) statics: usize,
}

impl SweepGrid {
    /// Gather changes the proxy set, so staged ids are stale.
    pub(super) fn begin_frame(&mut self) {
        self.staged = false;
        self.pair_queries = 0;
    }

    /// True when this grid answers the next sweep query, staging it first if
    /// the pair grid has answered as many queries as there are statics.
    fn answers_query(&mut self, config: &PhysicsConfig, proxies: &[Proxy]) -> bool {
        if !self.staged {
            if self.pair_queries < self.statics {
                self.pair_queries += 1;
                return false;
            }
            self.stage(config, proxies);
        }
        true
    }

    /// Stage every static proxy with the `2·linear_slop` inflation a pair
    /// build gives it, in proxy order. Statics never move within a frame, so
    /// one staging serves every substep.
    pub(super) fn stage(&mut self, config: &PhysicsConfig, proxies: &[Proxy]) {
        let grid = &mut self.grid;
        if (grid.cell_size() - config.broadphase_cell_size).abs() > f32::EPSILON {
            grid.set_cell_size(config.broadphase_cell_size);
        }
        grid.clear();
        let inflation = Vec2::splat(2.0 * config.linear_slop);
        for (id, proxy) in proxies.iter().enumerate() {
            if proxy.is_dynamic {
                continue;
            }
            let mut aabb = proxy.world_aabb();
            aabb.half_extents += inflation;
            grid.insert(id as ProxyId, &aabb);
        }
        self.staged = true;
    }
}

/// Safety-net slab sweep (D-064): clamp fast dynamics to the first static
/// sweep hit. Speculative contacts are the primary CCD; this pass only
/// catches trajectories the pair admission never saw — velocity injected by
/// the solver after contacts were built (impulse chains, deep-recovery bias).
/// Static circles promote conservatively to their bounding squares.
///
/// Only statics can be hit. The pair grid answers the query until the
/// statics-only grid is worth staging (D-080); both return the same statics
/// in the same order, so the first hit is the same.
pub(super) fn speculative_pass(
    config: &PhysicsConfig,
    proxies: &mut [Proxy],
    grid: &mut SpatialGrid,
    static_grid: &mut SweepGrid,
    candidates: &mut Vec<ProxyId>,
    events: &mut Vec<CollisionEvent>,
) {
    const BACKOFF: f32 = 1.0e-3;

    for a_idx in 0..proxies.len() {
        let proxy = &proxies[a_idx];
        if !proxy.is_dynamic {
            continue;
        }
        let min_extent = proxy.min_half_extent();
        if min_extent <= 0.0 {
            continue;
        }
        let travel_sq = (proxy.center - proxy.prev_center).length_squared();
        // Travel beyond the smallest half-extent can skip a thin wall
        // (`D-064`); compared squared.
        if travel_sq <= min_extent * min_extent {
            continue;
        }

        let swept = proxy.swept_aabb();
        if static_grid.answers_query(config, proxies) {
            static_grid.grid.query(&swept, None, candidates);
        } else {
            grid.query(&swept, Some(a_idx as ProxyId), candidates);
        }

        let a_prev = proxy.prev_center;
        let a_cur = proxy.center;
        let a_half = proxy.half_extents();

        let mut best: Option<(f32, Vec2, usize)> = None;
        for &b_id in candidates.iter() {
            let b_idx = b_id as usize;
            let target = &proxies[b_idx];
            if target.is_dynamic {
                continue;
            }
            let b_half = match target.shape {
                Shape::Aabb { half_extents } => half_extents,
                // Conservative circle->AABB promotion: the bounding square
                // can clamp a diagonal pass slightly early; acceptable for a
                // last-resort net.
                Shape::Circle { radius } => Vec2::splat(radius),
            };
            if let Some((t, n)) = sweep_aabb_vs_aabb(a_prev, a_cur, a_half, target.center, b_half) {
                // Skip hits on internal tile faces.
                let face_bit = if n.x < 0.0 {
                    FACE_LEFT
                } else if n.x > 0.0 {
                    FACE_RIGHT
                } else if n.y < 0.0 {
                    FACE_TOP
                } else {
                    FACE_BOTTOM
                };
                if target.face_mask & face_bit == 0 {
                    continue;
                }
                if best.is_none_or(|(best_t, _, _)| t < best_t) {
                    best = Some((t, n, b_idx));
                }
            }
        }

        let Some((t_hit, normal, b_idx)) = best else {
            continue;
        };
        let t_safe = (t_hit - BACKOFF).max(0.0);
        let clamped = a_prev + (a_cur - a_prev) * t_safe;
        proxies[a_idx].center = clamped;

        // Reflect closing velocity along contact normal.
        let v = proxies[a_idx].velocity;
        let v_along_normal = v.dot(normal);
        if v_along_normal < 0.0 {
            let restitution = proxies[a_idx].restitution;
            let impulse = -(1.0 + restitution) * v_along_normal;
            proxies[a_idx].velocity = v + normal * impulse;
        }

        if let Some(a_entity) = proxies[a_idx].entity {
            events.push(CollisionEvent {
                a: a_entity,
                b: proxies[b_idx].entity,
                normal,
                penetration: 0.0,
            });
        }
    }
}
