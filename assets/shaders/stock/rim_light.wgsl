// M29 rim-light helper.
//
// Standalone helper exposing an additive rim contribution. M29 does not bind
// this as a pipeline; it is manifest-tracked so material authors can fold
// `rim_term` into their own lit/material WGSL.

fn rim_term(n: vec3<f32>, view_dir: vec3<f32>, color: vec3<f32>, power: f32) -> vec3<f32> {
    let n_dot_v = max(0.0, dot(normalize(n), normalize(view_dir)));
    let rim = pow(1.0 - n_dot_v, max(power, 0.0));
    return color * rim;
}
