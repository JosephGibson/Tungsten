use super::*;

#[test]
fn stress_scene_defaults_to_baseline() {
    assert_eq!(StressScene::parse(None).unwrap(), StressScene::Baseline);
}

#[test]
fn stress_scene_parses_high_load_mode() {
    assert_eq!(
        StressScene::parse(Some("ecs-high-load")).unwrap(),
        StressScene::EcsHighLoad
    );
}

#[test]
fn high_load_mode_defaults_to_configured_count() {
    assert_eq!(
        resolve_count(StressScene::EcsHighLoad, None),
        DEFAULT_HIGH_LOAD_COUNT
    );
}
