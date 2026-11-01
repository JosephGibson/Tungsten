// The stock glow (redo 7.9), the reserved material `engine_glow`: a
// fragment-only module, compiled after the material prelude. A soft round
// glow filling its quad, drawn on the `engine_white` sprite: `material.v0`
// its colour, whose alpha weighs the whole, and `material.f` its falloff
// (the power of the distance from the centre), core boost (white added
// toward the centre), dither (noise against banding) and peak alpha. The
// sprite's tint multiplies in.

fn glow_hash(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2<f32>(12.9898, 78.233))) * 43758.5453);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // 0 at the centre, 1 on the circle the quad holds and past it.
    let d = min(length(in.local - vec2<f32>(0.5, 0.5)) * 2.0, 1.0);
    let t = pow(1.0 - d, max(material.f.x, 0.0001));
    let core = material.f.y * t * t;
    let grain = (glow_hash(in.clip_position.xy) - 0.5) * material.f.z;
    let alpha = clamp(t * material.f.w + grain, 0.0, 1.0) * material.v0.a * in.color.a;
    let rgb = material.v0.rgb * in.color.rgb + vec3<f32>(core);
    return vec4<f32>(rgb, alpha);
}
