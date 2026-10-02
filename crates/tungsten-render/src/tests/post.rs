use super::{ParamSlots, PostStackRenderer, source_slot};
use crate::passes::TargetId;

#[test]
fn plan_empty_stack_produces_no_entries() {
    let plan = PostStackRenderer::plan_targets(0);
    assert!(plan.is_empty());
    assert_eq!(PostStackRenderer::final_target(0), None);
}

#[test]
fn plan_single_pass_reads_scene_writes_ping() {
    let plan = PostStackRenderer::plan_targets(1);
    assert_eq!(plan, vec![(TargetId::SceneColor, TargetId::PostPing)]);
    assert_eq!(PostStackRenderer::final_target(1), Some(TargetId::PostPing));
}

#[test]
fn plan_two_passes_chains_ping_then_pong() {
    let plan = PostStackRenderer::plan_targets(2);
    assert_eq!(
        plan,
        vec![
            (TargetId::SceneColor, TargetId::PostPing),
            (TargetId::PostPing, TargetId::PostPong),
        ]
    );
    assert_eq!(PostStackRenderer::final_target(2), Some(TargetId::PostPong));
}

#[test]
fn plan_three_passes_returns_to_ping() {
    let plan = PostStackRenderer::plan_targets(3);
    assert_eq!(
        plan,
        vec![
            (TargetId::SceneColor, TargetId::PostPing),
            (TargetId::PostPing, TargetId::PostPong),
            (TargetId::PostPong, TargetId::PostPing),
        ]
    );
    assert_eq!(PostStackRenderer::final_target(3), Some(TargetId::PostPing));
}

#[test]
fn plan_seventeen_passes_stays_valid_all_the_way() {
    let plan = PostStackRenderer::plan_targets(17);
    assert_eq!(plan.len(), 17);
    for (i, &(src, dst)) in plan.iter().enumerate() {
        let expected_dst = if i.is_multiple_of(2) {
            TargetId::PostPing
        } else {
            TargetId::PostPong
        };
        assert_eq!(dst, expected_dst, "pass {i}");
        if i == 0 {
            assert_eq!(src, TargetId::SceneColor);
        } else {
            let prev_dst = if (i - 1).is_multiple_of(2) {
                TargetId::PostPing
            } else {
                TargetId::PostPong
            };
            assert_eq!(src, prev_dst, "pass {i} src follows previous dst");
        }
    }
    assert_eq!(
        PostStackRenderer::final_target(17),
        Some(TargetId::PostPing)
    );
}

#[test]
fn every_source_of_the_ladder_has_a_cache_slot_of_its_own() {
    // The three targets a stock pass can sample, as `plan_targets` names them.
    let mut slots: Vec<usize> = PostStackRenderer::plan_targets(17)
        .iter()
        .map(|&(src, _)| source_slot(src))
        .collect();
    slots.sort_unstable();
    slots.dedup();
    assert_eq!(slots, vec![0, 1, 2]);
    assert_ne!(
        source_slot(TargetId::PostPing),
        source_slot(TargetId::PostPong)
    );
}

/// Stages `payload` in `slot`, counting builds in `built`: returns the slot's
/// objects (the build count at the time it was built) and whether a write is
/// due.
fn stage(
    slots: &mut ParamSlots<u32>,
    built: &mut u32,
    slot: usize,
    payload: [u8; 256],
) -> (u32, bool) {
    let (objects, stale) = slots.stage(slot, &payload, || {
        *built += 1;
        *built
    });
    (*objects, stale)
}

#[test]
fn two_passes_of_one_effect_keep_params_of_their_own() {
    // The same effect in slots 0 and 1 with different parameters: each slot
    // gets objects of its own and takes its own write.
    let (red, blue) = ([1u8; 256], [2u8; 256]);
    let mut slots = ParamSlots::default();
    let mut built = 0;

    assert_eq!(stage(&mut slots, &mut built, 0, red), (1, true));
    assert_eq!(stage(&mut slots, &mut built, 1, blue), (2, true));

    // The next frame finds both buffers holding their bytes: nothing is built
    // and nothing is written.
    assert_eq!(stage(&mut slots, &mut built, 0, red), (1, false));
    assert_eq!(stage(&mut slots, &mut built, 1, blue), (2, false));
    assert_eq!(built, 2);
}

#[test]
fn swapped_params_are_written_to_both_slots() {
    let (red, blue) = ([1u8; 256], [2u8; 256]);
    let mut slots = ParamSlots::default();
    let mut built = 0;
    stage(&mut slots, &mut built, 0, red);
    stage(&mut slots, &mut built, 1, blue);

    // Reordering the stack moves the payloads, not the slots' objects.
    assert_eq!(stage(&mut slots, &mut built, 0, blue), (1, true));
    assert_eq!(stage(&mut slots, &mut built, 1, red), (2, true));
    assert_eq!(stage(&mut slots, &mut built, 0, blue), (1, false));
    assert_eq!(built, 2);
}

#[test]
fn a_far_slot_leaves_the_others_alone() {
    // A stack whose only stock pass sits at index 5 (bloom slots before it)
    // builds one entry; an earlier slot used later gets its own.
    let payload = [7u8; 256];
    let mut slots = ParamSlots::default();
    let mut built = 0;

    assert_eq!(stage(&mut slots, &mut built, 5, payload), (1, true));
    assert_eq!(built, 1);
    assert_eq!(stage(&mut slots, &mut built, 2, payload), (2, true));
    assert_eq!(stage(&mut slots, &mut built, 5, payload), (1, false));
    assert_eq!(built, 2);
}
