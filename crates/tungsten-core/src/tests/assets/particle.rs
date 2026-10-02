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
        render: ParticleRender::Quad,
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

fn triangle() -> ParticleMesh {
    ParticleMesh {
        vertices: vec![[0.0, -7.0], [6.0, 7.0], [-6.0, 7.0]],
        indices: vec![0, 1, 2],
    }
}

const MESH_CONFIG_JSON: &str = r#"{
    "render": { "kind": "mesh", "mesh": "tri" },
    "max_alive": 4,
    "emission": { "kind": "burst", "count": 1 },
    "lifetime": { "min": 0.5, "max": 1.0 },
    "initial_velocity": { "kind": "radial", "speed": { "min": 10.0, "max": 10.0 } }
}"#;

#[test]
fn particle_mesh_validate_accepts_triangle() {
    triangle().validate().unwrap();
}

#[test]
fn particle_mesh_validate_rejects_bad_index() {
    let mut mesh = triangle();
    mesh.indices = vec![0, 1, 3];
    let err = mesh.validate().unwrap_err();
    assert!(err.contains("out of range"), "got: {err}");
}

#[test]
fn particle_mesh_validate_rejects_non_triangle_index_count() {
    let mut mesh = triangle();
    mesh.indices = vec![0, 1];
    let err = mesh.validate().unwrap_err();
    assert!(err.contains("multiple of 3"), "got: {err}");

    mesh.indices.clear();
    let err = mesh.validate().unwrap_err();
    assert!(err.contains("must not be empty"), "got: {err}");
}

#[test]
fn particle_mesh_validate_rejects_non_finite_vertex() {
    let mut mesh = triangle();
    mesh.vertices[1] = [f32::NAN, 0.0];
    let err = mesh.validate().unwrap_err();
    assert!(err.contains("vertices[1]"), "got: {err}");

    mesh.vertices[1] = [0.0, f32::INFINITY];
    assert!(mesh.validate().is_err());
}

#[test]
fn particle_mesh_validate_rejects_too_few_vertices() {
    let mesh = ParticleMesh {
        vertices: vec![[0.0, 0.0], [1.0, 0.0]],
        indices: vec![0, 1, 0],
    };
    let err = mesh.validate().unwrap_err();
    assert!(err.contains("at least 3"), "got: {err}");
}

#[test]
fn render_defaults_to_quad() {
    let src = r#"{
        "sprite": "spark",
        "max_alive": 4,
        "emission": { "kind": "burst", "count": 1 },
        "lifetime": { "min": 0.5, "max": 1.0 },
        "initial_velocity": { "kind": "radial", "speed": { "min": 10.0, "max": 10.0 } }
    }"#;
    let cfg: ParticleConfig = serde_json::from_str(src).unwrap();
    assert_eq!(cfg.render, ParticleRender::Quad);
    assert_eq!(ParticleRender::default(), ParticleRender::Quad);
}

#[test]
fn render_mesh_parses_from_json() {
    let cfg: ParticleConfig = serde_json::from_str(MESH_CONFIG_JSON).unwrap();
    assert_eq!(cfg.render, ParticleRender::Mesh { mesh: "tri".into() });

    let quad: ParticleRender = serde_json::from_str(r#"{ "kind": "quad" }"#).unwrap();
    assert_eq!(quad, ParticleRender::Quad);
}

#[test]
fn quad_config_requires_sprite() {
    let src = r#"{
        "max_alive": 4,
        "emission": { "kind": "burst", "count": 1 },
        "lifetime": { "min": 0.5, "max": 1.0 },
        "initial_velocity": { "kind": "radial", "speed": { "min": 10.0, "max": 10.0 } }
    }"#;
    let cfg: ParticleConfig = serde_json::from_str(src).unwrap();
    let err = cfg.validate().unwrap_err();
    assert!(err.contains("sprite"), "got: {err}");
}

#[test]
fn mesh_config_needs_no_sprite() {
    let cfg: ParticleConfig = serde_json::from_str(MESH_CONFIG_JSON).unwrap();
    assert!(cfg.sprite.is_empty());
    cfg.validate().unwrap();

    let mut unnamed = cfg;
    unnamed.render = ParticleRender::Mesh {
        mesh: String::new(),
    };
    let err = unnamed.validate().unwrap_err();
    assert!(err.contains("render.mesh"), "got: {err}");
}

#[test]
fn mesh_registry_insert_keeps_id_on_replace() {
    let mut reg = ParticleMeshRegistry::new();
    assert!(reg.is_empty());
    let tri = reg.insert("tri", triangle());
    let quad = reg.insert(
        "quad",
        ParticleMesh {
            vertices: vec![[-1.0, -1.0], [1.0, -1.0], [1.0, 1.0], [-1.0, 1.0]],
            indices: vec![0, 1, 2, 0, 2, 3],
        },
    );
    assert_ne!(tri, quad);
    assert_eq!(reg.len(), 2);

    let mut wider = triangle();
    wider.vertices[1] = [12.0, 7.0];
    let again = reg.insert("tri", wider.clone());
    assert_eq!(again, tri);
    assert_eq!(reg.len(), 2);
    assert_eq!(reg.get(tri), Some(&wider));
    assert_eq!(reg.id_for_name("tri"), Some(tri));
    assert_eq!(reg.name_for_id(quad), Some("quad"));
    assert_eq!(reg.names().collect::<Vec<_>>(), ["tri", "quad"]);
    assert_eq!(reg.id_for_name("absent"), None);
}
