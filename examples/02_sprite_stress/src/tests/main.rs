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

#[test]
fn stress_scene_parses_physics_stress_mode() {
    assert_eq!(
        StressScene::parse(Some("physics-stress")).unwrap(),
        StressScene::PhysicsStress
    );
}

#[test]
fn physics_stress_mode_defaults_to_configured_count() {
    assert_eq!(
        resolve_count(StressScene::PhysicsStress, None),
        DEFAULT_PHYSICS_STRESS_COUNT
    );
}

#[test]
fn physics_sleep_defaults_on_and_parses_off() {
    assert!(parse_physics_sleep(None).unwrap());
    assert!(parse_physics_sleep(Some("on")).unwrap());
    assert!(!parse_physics_sleep(Some("0")).unwrap());
    assert!(!parse_physics_sleep(Some("off")).unwrap());
    assert!(parse_physics_sleep(Some("maybe")).is_err());
}

#[test]
fn ecs_density_requires_explicit_preserve_mode() {
    assert!(!parse_ecs_density(None).unwrap());
    assert!(!parse_ecs_density(Some("fixed")).unwrap());
    assert!(parse_ecs_density(Some("preserve")).unwrap());
    assert!(parse_ecs_density(Some("invalid")).is_err());
}

#[test]
fn render_features_has_a_separate_scene_and_default_count() {
    assert_eq!(
        StressScene::parse(Some("render-features")).unwrap(),
        StressScene::RenderFeatures
    );
    assert_eq!(resolve_count(StressScene::RenderFeatures, None), 4000);
}
