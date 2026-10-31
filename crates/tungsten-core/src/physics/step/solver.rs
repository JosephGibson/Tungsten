//! Warm-started soft contact solver (D-063) and the signed-distance narrow
//! phase that admits speculative contacts (D-064).

use super::slope;
use super::{ContactConstraint, FrictionConstraint, NO_SPIN, Proxy, RollingConstraint, SpinState};
use crate::physics::PhysicsConfig;
use crate::physics::collision::{
    Aabb, Contact, FACE_ONE_WAY, aabb_vs_aabb_speculative, aabb_vs_circle_speculative,
    circle_vs_circle_speculative,
};
use crate::physics::components::Shape;
use glam::Vec2;

/// Soft-constraint coefficients derived from (hertz, damping ratio, sub_dt).
#[derive(Debug, Clone, Copy)]
pub(super) struct Softness {
    bias_rate: f32,
    mass_scale: f32,
    impulse_scale: f32,
}

pub(super) fn soft_params(hertz: f32, damping_ratio: f32, h: f32) -> Softness {
    // Quarter of the substep rate is the stability ceiling.
    let hertz = hertz.min(0.25 / h);
    if hertz <= 0.0 {
        return Softness {
            bias_rate: 0.0,
            mass_scale: 1.0,
            impulse_scale: 0.0,
        };
    }
    let omega = std::f32::consts::TAU * hertz;
    let a1 = 2.0 * damping_ratio + h * omega;
    let c = h * omega * a1;
    Softness {
        bias_rate: omega / a1,
        mass_scale: c / (1.0 + c),
        impulse_scale: 1.0 / (1.0 + c),
    }
}

/// Apply a contact impulse through the inverse masses captured at contact
/// build; statics and sides asleep at build time carry 0 and never move.
pub(super) fn apply_impulse(proxies: &mut [Proxy], contact: &ContactConstraint, impulse: Vec2) {
    proxies[contact.a as usize].velocity += impulse * contact.inv_a;
    proxies[contact.b as usize].velocity -= impulse * contact.inv_b;
}

/// One Gauss-Seidel pass over the contact buffer. With `use_bias`, deep
/// contacts push out at the soft-constraint rate (capped by max push speed);
/// contacts inside the slop band limit approach instead. The bias-free relax
/// pass reuses the same accumulated impulses.
pub(super) fn solve_contacts(
    proxies: &mut [Proxy],
    contacts: &mut [ContactConstraint],
    config: &PhysicsConfig,
    soft: Softness,
    soft_static: Softness,
    inv_h: f32,
    use_bias: bool,
) {
    for contact in contacts.iter_mut() {
        let a = contact.a as usize;
        let b = contact.b as usize;
        let vn = (proxies[a].velocity - proxies[b].velocity).dot(contact.normal);

        // Separation relative to the slop band; negative = must push out.
        let s = config.linear_slop - contact.penetration;
        let (bias, mass_scale, impulse_scale) = if s > 0.0 {
            // Within slop: allow approach to close the band, never push.
            (s * inv_h, 1.0, 0.0)
        } else if use_bias {
            let softness = if contact.vs_static { soft_static } else { soft };
            (
                (softness.bias_rate * s).max(-config.max_push_speed),
                softness.mass_scale,
                softness.impulse_scale,
            )
        } else {
            (0.0, 1.0, 0.0)
        };

        let raw = -contact.normal_mass * mass_scale * (vn + bias) - impulse_scale * contact.impulse;
        let new_impulse = (contact.impulse + raw).max(0.0);
        let delta = new_impulse - contact.impulse;
        contact.impulse = new_impulse;
        if delta != 0.0 {
            apply_impulse(proxies, contact, contact.normal * delta);
        }
    }
}

/// A contact's tangent: its normal turned a quarter, `(−n.y, n.x)`.
#[inline(always)]
pub(super) fn tangent(normal: Vec2) -> Vec2 {
    Vec2::new(-normal.y, normal.x)
}

/// A tangent impulse `impulse` of `row`'s contact: along the tangent on the
/// two bodies, and on a spinning side's spin, `−2 · impulse · inv_m / r`
/// (its contact point sits `r` from its centre against the normal, its
/// moment of inertia is `m r² / 2`).
#[inline]
pub(super) fn apply_tangent(
    proxies: &mut [Proxy],
    spins: &mut [SpinState],
    contact: &ContactConstraint,
    row: &FrictionConstraint,
    impulse: f32,
) {
    apply_impulse(proxies, contact, tangent(contact.normal) * impulse);
    for (slot, inv) in [(row.spin_a, contact.inv_a), (row.spin_b, contact.inv_b)] {
        if slot != NO_SPIN && inv > 0.0 {
            let spin = &mut spins[slot as usize];
            spin.angular_velocity -= 2.0 * impulse * inv / spin.radius;
        }
    }
}

/// A side's rim speed along its contact's tangent, `r · ω`, which a spin of
/// `slot` subtracts from the tangent relative velocity; 0 for no spin.
#[inline]
fn rim(spins: &[SpinState], slot: u32) -> f32 {
    if slot == NO_SPIN {
        0.0
    } else {
        let spin = &spins[slot as usize];
        spin.radius * spin.angular_velocity
    }
}

/// One Gauss-Seidel pass of Coulomb friction after a normal pass: each
/// constraint's impulse removes its contact's tangent relative velocity, a
/// kinematic side's included and each spinning side's rim speed, its
/// accumulated value within `±mu` times the contact's normal impulse. An
/// empty list does nothing.
pub(super) fn solve_friction(
    proxies: &mut [Proxy],
    spins: &mut [SpinState],
    contacts: &[ContactConstraint],
    friction: &mut [FrictionConstraint],
) {
    for row in friction.iter_mut() {
        let contact = &contacts[row.contact as usize];
        let t = tangent(contact.normal);
        let a = contact.a as usize;
        let b = contact.b as usize;
        let vt = (proxies[a].velocity - proxies[b].velocity).dot(t)
            - rim(spins, row.spin_a)
            - rim(spins, row.spin_b);
        let bound = row.mu * contact.impulse;
        let new_impulse = (row.impulse - row.tangent_mass * vt).clamp(-bound, bound);
        let delta = new_impulse - row.impulse;
        row.impulse = new_impulse;
        if delta != 0.0 {
            apply_tangent(proxies, spins, contact, row, delta);
        }
    }
}

/// One Gauss-Seidel pass of rolling resistance after the friction pass:
/// each constraint's angular impulse opposes its spinning side's spin
/// relative to the other side's, its accumulated value within `±bound`
/// times the contact's normal impulse. An empty list does nothing.
pub(super) fn solve_rolling(
    spins: &mut [SpinState],
    contacts: &[ContactConstraint],
    rolling: &mut [RollingConstraint],
) {
    let spin_of = |spins: &[SpinState], slot: u32| {
        if slot == NO_SPIN {
            0.0
        } else {
            spins[slot as usize].angular_velocity
        }
    };
    for row in rolling.iter_mut() {
        let contact = &contacts[row.contact as usize];
        let relative = spin_of(spins, row.spin_a) - spin_of(spins, row.spin_b);
        let bound = row.bound * contact.impulse;
        let new_impulse = (row.impulse - row.mass * relative).clamp(-bound, bound);
        let delta = new_impulse - row.impulse;
        row.impulse = new_impulse;
        if delta == 0.0 {
            continue;
        }
        for (slot, inv, sign) in [
            (row.spin_a, contact.inv_a, 1.0),
            (row.spin_b, contact.inv_b, -1.0),
        ] {
            if slot != NO_SPIN && inv > 0.0 {
                let spin = &mut spins[slot as usize];
                spin.angular_velocity += sign * delta * 2.0 * inv / (spin.radius * spin.radius);
            }
        }
    }
}

/// [`masked_contact`] against a slope tile: a box's or a circle's contact
/// with its polygon.
#[cold]
#[inline(never)]
fn slope_contact(
    a: &Proxy,
    a_center: Vec2,
    b: &Proxy,
    b_center: Vec2,
    margin: f32,
) -> Option<Contact> {
    let poly = slope::polygon(b.slope, b_center, b.half_extents());
    match a.shape {
        Shape::Circle { radius } => {
            slope::circle_contact(a_center, radius, &poly, b.face_mask, margin)
        }
        Shape::Aabb { half_extents } => {
            slope::box_contact(a_center, half_extents, &poly, b.face_mask, margin)
        }
    }
}

/// Restitution from the approach velocity stored at contact build; contacts
/// slower than the threshold stay inelastic so piles can settle.
pub(super) fn apply_restitution(
    proxies: &mut [Proxy],
    contacts: &mut [ContactConstraint],
    threshold: f32,
) {
    for contact in contacts.iter_mut() {
        if contact.restitution == 0.0 || contact.approach > -threshold || contact.impulse <= 0.0 {
            continue;
        }
        let a = contact.a as usize;
        let b = contact.b as usize;
        let vn = (proxies[a].velocity - proxies[b].velocity).dot(contact.normal);
        let raw = -contact.normal_mass * (vn + contact.restitution * contact.approach);
        let new_impulse = (contact.impulse + raw).max(0.0);
        let delta = new_impulse - contact.impulse;
        contact.impulse = new_impulse;
        if delta != 0.0 {
            apply_impulse(proxies, contact, contact.normal * delta);
        }
    }
}

/// Signed-distance narrow phase (D-064): separated pairs with a gap under
/// `margin` return contacts with negative penetration for speculative CCD.
/// Always inlined: the substep's pair loop is the hottest loop of the step,
/// and a second caller must not turn it into a call.
#[inline(always)]
pub(super) fn narrow_phase(a: &Proxy, b: &Proxy, margin: f32) -> Option<Contact> {
    narrow_phase_at(a, a.center, b, b.center, margin)
}

/// `narrow_phase` with the two bodies placed at `a_center` and `b_center`.
/// A one-way tile `b` gives a contact only from above: one pushing `a` up
/// with its bottom at most a quarter cell below the tile's top face.
#[inline(always)]
pub(super) fn narrow_phase_at(
    a: &Proxy,
    a_center: Vec2,
    b: &Proxy,
    b_center: Vec2,
    margin: f32,
) -> Option<Contact> {
    let contact = masked_contact(a, a_center, b, b_center, margin)?;
    if b.face_mask & FACE_ONE_WAY != 0
        && !one_way_holds(
            a,
            a_center,
            b,
            b_center,
            contact.normal,
            0.5 * b.half_extents().y,
        )
    {
        return None;
    }
    Some(contact)
}

/// Whether the one-way tile `b` at `b_center` holds `a` at `a_center` with a
/// contact along `normal`: one pushing it up, its bottom at most `depth`
/// below the tile's top face.
#[inline(always)]
pub(super) fn one_way_holds(
    a: &Proxy,
    a_center: Vec2,
    b: &Proxy,
    b_center: Vec2,
    normal: Vec2,
    depth: f32,
) -> bool {
    let bottom = a_center.y + a.half_extents().y;
    let top = b_center.y - b.half_extents().y;
    normal.y < 0.0 && bottom <= top + depth
}

/// The signed-distance contact of `a` at `a_center` against `b` at
/// `b_center`, a tile by its face mask and a slope tile by its polygon.
#[inline(always)]
fn masked_contact(
    a: &Proxy,
    a_center: Vec2,
    b: &Proxy,
    b_center: Vec2,
    margin: f32,
) -> Option<Contact> {
    if b.slope != 0 {
        return slope_contact(a, a_center, b, b_center, margin);
    }
    match (a.shape, b.shape) {
        (
            Shape::Aabb {
                half_extents: ha, ..
            },
            Shape::Aabb {
                half_extents: hb, ..
            },
        ) => aabb_vs_aabb_speculative(
            &Aabb::new(a_center, ha),
            &Aabb::new(b_center, hb),
            b.face_mask,
            margin,
        ),
        (Shape::Circle { radius: ra }, Shape::Circle { radius: rb }) => {
            circle_vs_circle_speculative(a_center, ra, b_center, rb, margin)
        }
        (Shape::Aabb { half_extents }, Shape::Circle { radius }) => aabb_vs_circle_speculative(
            &Aabb::new(a_center, half_extents),
            b_center,
            radius,
            a.face_mask,
            margin,
        ),
        (Shape::Circle { radius }, Shape::Aabb { half_extents }) => {
            // Helper returns AABB escape normal; circle needs the opposite.
            aabb_vs_circle_speculative(
                &Aabb::new(b_center, half_extents),
                a_center,
                radius,
                b.face_mask,
                margin,
            )
            .map(|c| Contact {
                normal: -c.normal,
                penetration: c.penetration,
            })
        }
    }
}
