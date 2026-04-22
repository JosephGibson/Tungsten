use super::*;

#[test]
fn curve_scalar_endpoints() {
    let c = Curve::<f32> {
        points: vec![(0.0, 10.0), (1.0, 20.0)],
    };
    assert_eq!(c.sample(-1.0), 10.0);
    assert_eq!(c.sample(0.0), 10.0);
    assert_eq!(c.sample(1.0), 20.0);
    assert_eq!(c.sample(2.0), 20.0);
}

#[test]
fn curve_scalar_midpoint_interpolates() {
    let c = Curve::<f32> {
        points: vec![(0.0, 0.0), (1.0, 10.0)],
    };
    assert!((c.sample(0.5) - 5.0).abs() < 1.0e-5);
}

#[test]
fn curve_scalar_multi_segment() {
    let c = Curve::<f32> {
        points: vec![(0.0, 0.0), (0.5, 10.0), (1.0, 0.0)],
    };
    assert!((c.sample(0.25) - 5.0).abs() < 1.0e-5);
    assert!((c.sample(0.75) - 5.0).abs() < 1.0e-5);
}

#[test]
fn curve_rgba_interpolates_componentwise() {
    let c = Curve::<[f32; 4]> {
        points: vec![(0.0, [1.0, 0.0, 0.0, 1.0]), (1.0, [0.0, 1.0, 0.0, 1.0])],
    };
    let mid = c.sample(0.5);
    assert!((mid[0] - 0.5).abs() < 1.0e-5);
    assert!((mid[1] - 0.5).abs() < 1.0e-5);
    assert!((mid[2] - 0.0).abs() < 1.0e-5);
    assert!((mid[3] - 1.0).abs() < 1.0e-5);
}

#[test]
fn config_round_trip() {
    let src = r#"{
        "sprite": "spark",
        "max_alive": 32,
        "emission": { "kind": "burst", "count": 8 },
        "lifetime": { "min": 0.5, "max": 1.0 },
        "initial_velocity": { "kind": "radial", "speed": { "min": 10.0, "max": 20.0 } }
    }"#;
    let cfg: ParticleConfig = serde_json::from_str(src).unwrap();
    cfg.validate().unwrap();
    assert_eq!(cfg.sprite, "spark");
    assert!(matches!(cfg.emission, EmissionKind::Burst { count: 8, .. }));
    assert!(matches!(
        cfg.initial_velocity,
        InitialVelocity::Radial { .. }
    ));
    assert_eq!(cfg.blend, BlendMode::Alpha);
}

#[test]
fn config_rejects_unsorted_curve() {
    let src = r#"{
        "sprite": "s",
        "max_alive": 4,
        "emission": { "kind": "burst", "count": 1 },
        "lifetime": { "min": 0.5, "max": 1.0 },
        "initial_velocity": { "kind": "radial", "speed": { "min": 10.0, "max": 10.0 } },
        "alpha_over_life": [[0.0, 1.0], [0.3, 0.5], [0.2, 0.0]]
    }"#;
    let cfg: ParticleConfig = serde_json::from_str(src).unwrap();
    let err = cfg.validate().unwrap_err();
    assert!(err.contains("sorted"), "expected sort error, got: {err}");
}

#[test]
fn config_rejects_zero_max_alive() {
    let src = r#"{
        "sprite": "s",
        "max_alive": 0,
        "emission": { "kind": "burst", "count": 1 },
        "lifetime": { "min": 0.5, "max": 1.0 },
        "initial_velocity": { "kind": "radial", "speed": { "min": 10.0, "max": 10.0 } }
    }"#;
    let cfg: ParticleConfig = serde_json::from_str(src).unwrap();
    assert!(cfg.validate().is_err());
}

#[test]
fn config_rejects_negative_lifetime() {
    let src = r#"{
        "sprite": "s",
        "max_alive": 1,
        "emission": { "kind": "burst", "count": 1 },
        "lifetime": { "min": -0.1, "max": 1.0 },
        "initial_velocity": { "kind": "radial", "speed": { "min": 10.0, "max": 10.0 } }
    }"#;
    let cfg: ParticleConfig = serde_json::from_str(src).unwrap();
    assert!(cfg.validate().is_err());
}

#[test]
fn world_rng_seed_is_deterministic_and_post_increments() {
    let mut a = WorldRngSeed::new(0);
    let mut b = WorldRngSeed::new(0);
    assert_eq!(a.derive_seed(), b.derive_seed());
    assert_eq!(a.next, 1);
    let s0 = WorldRngSeed::new(0).derive_seed();
    let s1 = WorldRngSeed::new(1).derive_seed();
    assert_ne!(
        s0, s1,
        "consecutive start values must produce distinct seeds"
    );
}

#[test]
fn particle_budget_default_is_10k() {
    assert_eq!(ParticleBudget::default().global_cap, 10_000);
}

#[test]
fn registry_register_and_replace_preserves_id() {
    let cfg_a = Arc::new(ParticleConfig {
        sprite: "s".into(),
        max_alive: 4,
        seed: None,
        blend: BlendMode::Alpha,
        emission: EmissionKind::Burst {
            count: 1,
            once: true,
        },
        lifetime: Range { min: 0.1, max: 0.2 },
        initial_velocity: InitialVelocity::Radial {
            speed: Range::single(1.0),
        },
        gravity: [0.0, 0.0],
        drag_per_sec: 0.0,
        angular_velocity: Range::single(0.0),
        start_scale: Range::single(1.0),
        scale_over_life: None,
        color_over_life: None,
        alpha_over_life: None,
        tint: [1.0, 1.0, 1.0, 1.0],
    });
    let mut reg = ParticleConfigRegistry::new();
    let id = reg.register(
        "spark".into(),
        PathBuf::from("/p/spark.json"),
        cfg_a.clone(),
    );
    assert_eq!(reg.id_for_name("spark"), Some(id));
    assert_eq!(reg.id_for_path(Path::new("/p/spark.json")), Some(id));
    assert_eq!(reg.name_for_id(id), Some("spark"));
    assert!(Arc::ptr_eq(reg.get(id).unwrap(), &cfg_a));

    let cfg_b = Arc::new((*cfg_a).clone());
    reg.replace(id, cfg_b.clone());
    assert!(Arc::ptr_eq(reg.get(id).unwrap(), &cfg_b));
    assert_eq!(reg.id_for_name("spark"), Some(id));
}

#[test]
fn config_rejects_nonfinite_field() {
    let src = r#"{
        "sprite": "s",
        "max_alive": 1,
        "emission": { "kind": "continuous", "rate_hz": 100.0 },
        "lifetime": { "min": 0.5, "max": 1.0 },
        "initial_velocity": { "kind": "radial", "speed": { "min": 1.0, "max": 2.0 } },
        "drag_per_sec": -1.0
    }"#;
    let cfg: ParticleConfig = serde_json::from_str(src).unwrap();
    assert!(cfg.validate().is_err());
}
