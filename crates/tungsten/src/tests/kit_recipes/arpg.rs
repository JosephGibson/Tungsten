//! The arpg fixture, the ARPG design's genre (`D-218`): §9.1's caster in
//! data, Cinder Volley with its three supports' and a ring's skill-local
//! lines, on a holder whose stats carry the passives', the wand's and the
//! status's lines, beside a chilled fen brute. The fixture's damage types
//! are `elemental.fire` and `elemental.cold`, names the other fixtures
//! declare, so coverage's set holds. 7.10g's adept casts cinder in the aim
//! pose nearest its aim's elevation, and cinder names its icon.

use tungsten_core::Entity;

use super::{DTS, recipe, spawn};
use crate::kit::{Damaged, DamagedPart, KitPlugins, KitRng, Stats};
use crate::testing::Harness;

const GENRE: &str = "arpg";

/// §9.1's pinned seeds, as the kit's sheet test pins them: the first draw
/// within 10⁻⁴ of 0.5 at both, the second missing a 0.09 crit chance at the
/// first and making it at the second.
const PLAIN: u64 = 7228;
const CRIT: u64 = 2638;

/// The volley's hit on the brute, its parts' amounts, and the caster, after
/// `attack` is pressed under `seed` with the brute in the caster's hitbox.
fn volley(dt: f32, seed: u64) -> (Harness, Entity, Damaged, Vec<f32>) {
    let mut harness = recipe(GENRE, KitPlugins::set(), dt);
    let caster = spawn(&mut harness, "caster", [0.0, 0.0]);
    let brute = spawn(&mut harness, "brute", [40.0, 0.0]);
    harness.step(1);
    harness.world_mut().insert_resource(KitRng::seeded(seed));
    harness.press_action("attack");
    let mut hits = Vec::new();
    let mut parts = Vec::new();
    for _ in 0..4 {
        harness.step(1);
        hits.extend(
            harness
                .events::<Damaged>()
                .filter(|hit| hit.entity == brute)
                .copied(),
        );
        parts.extend(
            harness
                .events::<DamagedPart>()
                .filter(|part| part.entity == brute)
                .map(|part| part.amount),
        );
    }
    assert_eq!(hits.len(), 1, "dt {dt}, seed {seed}: one hit");
    (harness, caster, hits[0], parts)
}

#[test]
fn nine_ones_volley_hits_32_07_and_crits_60_94_from_data() {
    for dt in DTS {
        for (seed, crit, want) in [(PLAIN, false, 32.07), (CRIT, true, 60.94)] {
            let (_, _, hit, mut parts) = volley(dt, seed);
            assert!(
                (hit.amount - want).abs() < 0.005 && hit.crit == crit,
                "dt {dt}, seed {seed}: {} (crit {})",
                hit.amount,
                hit.crit
            );
            // Half the fire to cold: 10 of fire at fire's 0.6 and the
            // ring's 0.10, 10 of cold at cold's 0.75, a crit's 1.9 on both.
            let scale = if crit { 1.9 } else { 1.0 };
            parts.sort_by(f32::total_cmp);
            assert!(
                parts.len() == 2
                    && (parts[0] - 14.8456 * scale).abs() < 0.01
                    && (parts[1] - 17.226 * scale).abs() < 0.01,
                "dt {dt}, seed {seed}: {parts:?}"
            );
        }
    }
}

#[test]
fn nine_ones_volley_spends_its_sheets_cost() {
    for dt in DTS {
        let (harness, caster, ..) = volley(dt, PLAIN);
        let mana = harness
            .world()
            .get::<Stats>(caster)
            .unwrap()
            .base("mana")
            .unwrap();
        assert!(
            (mana - (100.0 - 20.592)).abs() < 0.005,
            "dt {dt}: 12 × 1.3 × 1.2 × 1.1 off 100: {mana}"
        );
    }
}

/// 7.10g: the adept, aimed by its game at the nearest enemy, casts cinder
/// in the aim pose nearest the brute's elevation, flipped on the left, for
/// the pose's 200 ms; cinder names its icon for a bar.
#[test]
fn the_caster_casts_cinder_in_its_aim_pose() {
    use glam::Vec2;
    use tungsten_core::components::SpriteFlip;
    use tungsten_core::{AnimationState, DataRegistry, Transform};

    use super::frames_for;
    use crate::kit::{Abilities, Aim, TagFilter, Target};

    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let adept = spawn(&mut harness, "adept", [0.0, 0.0]);
        let brute = spawn(&mut harness, "brute", [24.0, -32.0]);
        let enemy = Target::nearest(TagFilter::new().all(["team.enemy"]));
        harness.world_mut().insert(adept, Aim::at(enemy));
        harness.step(1);
        let shown = |harness: &Harness| {
            let world = harness.world();
            let flipped = world.get::<SpriteFlip>(adept).is_some_and(|flip| flip.0);
            let state = world.get::<AnimationState>(adept).unwrap();
            (state.animation_id.clone(), flipped)
        };
        {
            let world = harness.world();
            let abilities = world.get::<Abilities>(adept).unwrap();
            let data = world.resource::<DataRegistry>();
            assert_eq!(abilities.icon(data, "cinder"), Some("cinder_icon"));
        }
        assert_eq!(shown(&harness), ("adept_idle".into(), false), "dt {dt}");
        let cast = |harness: &mut Harness| {
            harness.press_action("attack");
            harness.step(1);
            harness.release_action("attack");
        };
        // 53 degrees up and ahead: the up-forward pose of five.
        cast(&mut harness);
        assert_eq!(
            shown(&harness),
            ("adept_cast_up_fwd".into(), false),
            "dt {dt}"
        );
        for _ in 1..frames_for(0.2, dt) {
            harness.step(1);
            assert_eq!(shown(&harness).0, "adept_cast_up_fwd", "dt {dt}");
        }
        harness.step(1);
        assert_eq!(
            shown(&harness).0,
            "adept_idle",
            "dt {dt}: its pose's 200 ms"
        );
        // Level and behind: the forward pose, flipped.
        harness
            .world_mut()
            .get_mut::<Transform>(brute)
            .unwrap()
            .position = Vec2::new(-40.0, 0.0);
        harness.step(1);
        cast(&mut harness);
        assert_eq!(shown(&harness), ("adept_cast_fwd".into(), true), "dt {dt}");
    }
}
