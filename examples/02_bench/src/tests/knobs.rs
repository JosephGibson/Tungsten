use super::*;
use crate::physics::BENCH;

fn resolve_physics(
    preset: Option<&str>,
    scale: Option<&str>,
    set: Option<&str>,
) -> anyhow::Result<BenchConfig> {
    resolve(&BENCH, preset, scale, set)
}

fn error_text(result: anyhow::Result<BenchConfig>) -> String {
    result.expect_err("resolution should fail").to_string()
}

#[test]
fn defaults_resolve_without_env() {
    let cfg = resolve_physics(None, None, None).unwrap();
    assert_eq!(cfg.preset, "default");
    assert_eq!(cfg.scale, 1.0);
    assert_eq!(cfg.choice("mode"), "pachinko");
    assert_eq!(cfg.int("balls"), 8_000);
    assert_eq!(cfg.seed(), 1);
    assert_eq!(cfg.row(), "physics");
}

#[test]
fn preset_then_scale_then_overrides() {
    let cfg = resolve_physics(Some("sparse"), Some("2"), Some("speed_max=2000, cell=64")).unwrap();
    assert_eq!(cfg.choice("mode"), "sparse");
    assert_eq!(cfg.row(), "physics-sparse");
    assert_eq!(cfg.int("bodies"), 16_000);
    assert_eq!(cfg.int("balls"), 16_000);
    assert_eq!(cfg.float("speed_max"), 2_000.0);
    assert_eq!(cfg.int("cell"), 64);
    // Scale never touches unscaled knobs; overrides land after scale.
    assert_eq!(cfg.float("density"), 0.01);
    let cfg = resolve_physics(Some("min"), Some("3"), Some("balls=500")).unwrap();
    assert_eq!(cfg.int("balls"), 500);
}

#[test]
fn scale_rounds_and_clamps_integer_knobs() {
    let cfg = resolve_physics(None, Some("0.33337"), None).unwrap();
    assert_eq!(cfg.int("balls"), 2_667);
    let cfg = resolve_physics(None, Some("0.0001"), None).unwrap();
    assert_eq!(cfg.int("balls"), 200);
    assert_eq!(cfg.int("bodies"), 1_000);
    let cfg = resolve_physics(None, Some("1000"), None).unwrap();
    assert_eq!(cfg.int("balls"), 400_000);
    assert_eq!(cfg.int("bodies"), 2_000_000);
}

#[test]
fn every_preset_resolves() {
    for &bench in crate::BENCHES {
        for preset in bench.presets {
            let cfg = resolve(bench, Some(preset.name), None, None).unwrap();
            assert_eq!(cfg.preset, preset.name, "{}", bench.name);
        }
        for row in bench.rows {
            assert_eq!(
                resolve(bench, Some(row.preset), None, None).unwrap().row(),
                row.name
            );
        }
    }
}

#[test]
fn validation_errors_name_the_knob_and_range() {
    let text = error_text(resolve_physics(None, None, Some("bogus=1")));
    assert!(text.contains("unknown knob 'bogus'"), "{text}");
    let text = error_text(resolve_physics(None, None, Some("balls=500000")));
    assert!(
        text.contains("'balls'") && text.contains("200..=400000"),
        "{text}"
    );
    let text = error_text(resolve_physics(None, None, Some("fill=0.9")));
    assert!(
        text.contains("'fill'") && text.contains("0.05..=0.4"),
        "{text}"
    );
    let text = error_text(resolve_physics(None, None, Some("fill=lots")));
    assert!(
        text.contains("'fill'") && text.contains("got 'lots'"),
        "{text}"
    );
    let text = error_text(resolve_physics(None, None, Some("mode=dense")));
    assert!(
        text.contains("'mode'") && text.contains("pachinko, sparse"),
        "{text}"
    );
    let text = error_text(resolve_physics(None, None, Some("balls")));
    assert!(text.contains("not knob=value"), "{text}");
    let text = error_text(resolve_physics(None, None, Some("speed_min=2000")));
    assert!(
        text.contains("speed_min") && text.contains("speed_max"),
        "{text}"
    );
    let text = error_text(resolve_physics(Some("huge"), None, None));
    assert!(
        text.contains("unknown preset 'huge'") && text.contains("sparse-min"),
        "{text}"
    );
    for scale in ["0", "-1", "abc", "inf", "NaN"] {
        let text = error_text(resolve_physics(None, Some(scale), None));
        assert!(text.contains("TUNGSTEN_BENCH_SCALE"), "{text}");
    }
    let text = select(&[&BENCH], Some("gpu"))
        .expect_err("unknown bench")
        .to_string();
    assert!(
        text.contains("unknown benchmark 'gpu'") && text.contains("physics"),
        "{text}"
    );
}
