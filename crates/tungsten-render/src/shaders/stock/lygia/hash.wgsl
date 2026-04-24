// LYGIA snippet (MIT) — https://lygia.xyz/ — pcg-style 2D hash.
// Included by other stock effects via Rust-side source concatenation.

fn hash12(p: vec2<f32>) -> f32 {
    var p3 = fract(vec3<f32>(p.xyx) * 0.1031);
    p3 = p3 + dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

fn hash22(p: vec2<f32>) -> vec2<f32> {
    var p3 = fract(vec3<f32>(p.xyx) * vec3<f32>(0.1031, 0.1030, 0.0973));
    p3 = p3 + dot(p3, p3.yzx + 33.33);
    return fract(vec2<f32>((p3.x + p3.y) * p3.z, (p3.x + p3.z) * p3.y));
}
