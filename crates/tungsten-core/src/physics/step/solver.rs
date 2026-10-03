//! Warm-started soft contact solver (D-063) and the signed-distance narrow
//! phase that admits speculative contacts (D-064).

use super::{ContactConstraint, Proxy};
use crate::physics::PhysicsConfig;
use crate::physics::collision::{
    Aabb, Contact, aabb_vs_aabb_speculative, aabb_vs_circle_speculative,
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
#[inline(always)]
pub(super) fn narrow_phase_at(
    a: &Proxy,
    a_center: Vec2,
    b: &Proxy,
    b_center: Vec2,
    margin: f32,
) -> Option<Contact> {
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
