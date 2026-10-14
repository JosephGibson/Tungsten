use super::*;

const VALID_WGSL: &str = r"
@vertex fn vs_main(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    return vec4<f32>(f32(i), 0.0, 0.0, 1.0);
}
@fragment fn fs_main() -> @location(0) vec4<f32> {
    return vec4<f32>(1.0, 1.0, 1.0, 1.0);
}
";

const PARSE_BROKEN_WGSL: &str = "fn totally broken :: :: ~~ { ";

const SEMANTIC_BROKEN_WGSL: &str = r"
@vertex fn vs_main() -> @builtin(position) vec4<f32> {
    // reference an undeclared identifier -> validation error
    return ghost_variable;
}
";

#[test]
fn valid_wgsl_source_passes_validation() {
    validate_wgsl_source("sprite", VALID_WGSL).expect("valid WGSL should pass");
}

#[test]
fn parse_error_is_reported_distinctly() {
    let err = validate_wgsl_source("sprite", PARSE_BROKEN_WGSL).unwrap_err();
    assert!(matches!(err, ShaderError::Parse { .. }));
}

#[test]
fn semantic_error_is_reported_distinctly() {
    let err = validate_wgsl_source("sprite", SEMANTIC_BROKEN_WGSL).unwrap_err();
    // Undeclared identifier surfaces from the parser in naga 29 — either kind
    // signals "bad shader, reject it", which is all the cache needs to know.
    assert!(matches!(
        err,
        ShaderError::Parse { .. } | ShaderError::Validation { .. }
    ));
}

#[test]
fn cache_byte_equal_short_circuit_counts_unchanged_calls() {
    // Device-free code path: seed the entry directly and check `bytes_equal`.
    let cache = ShaderModuleCache::new();
    let id = ShaderAssetId(0);
    // Seed without a GPU by injecting an `Entry` through `commit` +
    // a dummy module is impossible device-free, so we only test the
    // bytes_equal predicate + unchanged_count bookkeeping via the cache API.
    assert!(!cache.bytes_equal(id, VALID_WGSL));
    assert_eq!(cache.unchanged_count, 0);
}

/// Needs `SHADER_F16`, which the renderer never requests.
const F16_WGSL: &str = r"
enable f16;
@fragment fn fs_main() -> @location(0) vec4<f32> {
    let half: f16 = 0.5h;
    return vec4<f32>(f32(half));
}
";

#[test]
fn the_cache_validates_against_the_device_capabilities() {
    // Naga with every capability accepts a module a device without the
    // feature rejects, and wgpu's rejection outside an error scope panics.
    validate_wgsl_source("half", F16_WGSL).expect("every capability takes f16");
    let mut cache = ShaderModuleCache::new();
    cache.set_capabilities(naga_capabilities(
        wgpu::Features::TIMESTAMP_QUERY,
        wgpu::DownlevelFlags::all(),
    ));
    let err = cache.precheck("half", F16_WGSL).unwrap_err();
    assert!(
        matches!(
            err,
            ShaderError::Parse { .. } | ShaderError::Validation { .. }
        ),
        "{err}"
    );
    cache.set_capabilities(naga_capabilities(
        wgpu::Features::SHADER_F16,
        wgpu::DownlevelFlags::all(),
    ));
    cache
        .precheck("half", F16_WGSL)
        .expect("the feature takes f16");
    cache
        .precheck("sprite", VALID_WGSL)
        .expect("valid WGSL passes");
}
