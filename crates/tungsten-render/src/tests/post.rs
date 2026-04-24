use super::PostStackRenderer;
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
    for i in 0..17 {
        let (src, dst) = plan[i];
        let expected_dst = if i % 2 == 0 {
            TargetId::PostPing
        } else {
            TargetId::PostPong
        };
        assert_eq!(dst, expected_dst, "pass {i}");
        if i == 0 {
            assert_eq!(src, TargetId::SceneColor);
        } else {
            let prev_dst = if (i - 1) % 2 == 0 {
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
