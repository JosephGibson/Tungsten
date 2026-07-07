// M29 emissive-mask helper.
//
// Standalone helper exposing a fragment-only contribution function. M29 does
// not bind this as a pipeline; it is manifest-tracked so material authors and
// future milestones can compose `emissive_contribution` into their own shaders.

fn emissive_contribution(
    uv: vec2<f32>,
    mask_tex: texture_2d<f32>,
    samp: sampler,
    strength: f32,
) -> vec3<f32> {
    let m = textureSample(mask_tex, samp, uv).rgb;
    return m * strength;
}
