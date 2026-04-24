// LYGIA snippet (MIT) — https://lygia.xyz/ — Rec.709 luma.

fn luma_bt709(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
}
