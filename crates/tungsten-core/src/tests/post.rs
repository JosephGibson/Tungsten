use super::*;

#[test]
fn default_post_stack_is_empty() {
    let s = PostStack::new();
    assert_eq!(s.len(), 0);
    assert!(s.is_empty());
    assert!(s.as_slice().is_empty());
}

#[test]
fn push_and_reorder_preserve_length() {
    let mut s = PostStack::new();
    s.push(PostPass::Tonemap(TonemapParams::default()));
    s.push(PostPass::Vignette(VignetteParams::default()));
    s.push(PostPass::FilmGrain(FilmGrainParams::default()));
    assert_eq!(s.len(), 3);

    s.reorder(0, 2);
    assert_eq!(s.len(), 3);
    assert!(matches!(s.as_slice()[2], PostPass::Tonemap(_)));
    assert!(matches!(s.as_slice()[0], PostPass::Vignette(_)));
    assert!(matches!(s.as_slice()[1], PostPass::FilmGrain(_)));
}

#[test]
fn reorder_on_empty_stack_is_noop() {
    let mut s = PostStack::new();
    s.reorder(0, 5);
    assert!(s.is_empty());
}

#[test]
fn clear_empties_the_stack() {
    let mut s = PostStack::new();
    s.push(PostPass::Fade(FadeParams::default()));
    assert!(!s.is_empty());
    s.clear();
    assert!(s.is_empty());
}

fn round_trip(pass: PostPass) {
    let json = serde_json::to_string(&pass).expect("serialize");
    let back: PostPass = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(pass, back, "round-trip failed for {}", pass.kind_name());
}

#[test]
fn all_eighteen_variants_round_trip_through_json() {
    round_trip(PostPass::Tonemap(TonemapParams::default()));
    round_trip(PostPass::Vignette(VignetteParams::default()));
    round_trip(PostPass::Lut(LutParams::default()));
    round_trip(PostPass::ChromaticAberration(0.5));
    round_trip(PostPass::ColorAdjust(ColorAdjustParams::default()));
    round_trip(PostPass::ToneMono(ToneMonoParams::default()));
    round_trip(PostPass::Crt(CrtParams::default()));
    round_trip(PostPass::FilmGrain(FilmGrainParams::default()));
    round_trip(PostPass::Dither(DitherParams::default()));
    round_trip(PostPass::PixelOutline(PixelOutlineParams::default()));
    round_trip(PostPass::Fade(FadeParams::default()));
    round_trip(PostPass::WipeRadial(WipeRadialParams::default()));
    round_trip(PostPass::Dissolve(DissolveParams::default()));
    round_trip(PostPass::Glitch(GlitchParams::default()));
    round_trip(PostPass::Pixelate(2.0));
    round_trip(PostPass::Fog(FogParams::default()));
    round_trip(PostPass::GodRays(GodRaysParams::default()));
    round_trip(PostPass::Bloom(BloomParams::default()));
}

#[test]
fn bloom_serde_round_trip_with_non_default_params() {
    let pass = PostPass::Bloom(BloomParams {
        threshold: 1.2,
        knee: 0.4,
        intensity: 0.6,
        radius: 0.85,
    });
    let json = serde_json::to_string(&pass).unwrap();
    let back: PostPass = serde_json::from_str(&json).unwrap();
    assert_eq!(pass, back);
    assert!(json.contains("\"kind\":\"bloom\""));
}

#[test]
fn bloom_kind_name() {
    assert_eq!(PostPass::Bloom(BloomParams::default()).kind_name(), "bloom");
}

#[test]
fn bloom_default_matches_reference() {
    let p = BloomParams::default();
    assert_eq!(p.threshold, 1.0);
    assert_eq!(p.knee, 0.5);
    assert_eq!(p.intensity, 0.7);
    assert_eq!(p.radius, 1.0);
}

#[test]
fn kind_name_matches_serde_tag() {
    assert_eq!(
        PostPass::Tonemap(TonemapParams::default()).kind_name(),
        "tonemap"
    );
    assert_eq!(
        PostPass::ChromaticAberration(1.0).kind_name(),
        "chromatic_aberration"
    );
    assert_eq!(
        PostPass::GodRays(GodRaysParams::default()).kind_name(),
        "god_rays"
    );
}

/// The pass of effect `id` with every lane zero.
fn custom(id: u32) -> CustomPost {
    CustomPost::new(
        crate::assets::PostEffectId(id),
        crate::assets::MaterialUniformDefaults::default(),
    )
}

#[test]
fn a_custom_pass_is_written_and_never_read() {
    // The nineteenth variant holds an ID of the session's (`D-308`). It is
    // written, so a room built in code that holds one keeps its canonical
    // form and two such rooms their own hashes; it is never read.
    let pass = PostPass::Custom(custom(0));
    assert_eq!(pass.kind_name(), "custom");
    let written = serde_json::to_value(pass).expect("a custom pass serializes");
    assert_eq!(written["kind"], "custom");
    assert_eq!(written["params"]["effect"], 0);
    let mut other = custom(0);
    other.set_f32(0, 0.5);
    assert_ne!(
        serde_json::to_value(PostPass::Custom(other)).unwrap(),
        written
    );
    assert_ne!(
        serde_json::to_value(PostPass::Custom(custom(1))).unwrap(),
        written
    );
    // What it wrote does not read back.
    assert!(serde_json::from_value::<PostPass>(written).is_err());
    let named = serde_json::from_str::<PostPass>(r#"{"kind": "custom", "params": {"effect": 0}}"#);
    let refused = named.unwrap_err().to_string();
    assert!(refused.contains("unknown variant `custom`"), "{refused}");
    // A room's `post` is a list of passes: one that names `custom` is
    // refused, where the stock kinds beside it read.
    let stack = r#"[{"kind": "pixelate", "params": 4.0}, {"kind": "custom", "params": {}}]"#;
    assert!(serde_json::from_str::<Vec<PostPass>>(stack).is_err());
    let stock = r#"[{"kind": "pixelate", "params": 4.0}]"#;
    assert_eq!(
        serde_json::from_str::<Vec<PostPass>>(stock).unwrap(),
        [PostPass::Pixelate(4.0)]
    );
}

#[test]
fn a_custom_pass_sets_its_lanes_and_ignores_one_past_the_block() {
    let mut pass = custom(2);
    pass.set_vec4(3, [1.0, 2.0, 3.0, 4.0])
        .set_f32(1, 0.5)
        .set_i32(0, -7);
    assert_eq!(pass.vec4[3], [1.0, 2.0, 3.0, 4.0]);
    assert_eq!((pass.f32s[1], pass.i32s[0]), (0.5, -7));
    let before = pass;
    pass.set_vec4(4, [9.0; 4]).set_f32(4, 9.0).set_i32(4, 9);
    assert_eq!(pass, before);
    // Its size: the ID and the block's 96 bytes of lanes, which is the
    // stack's cost a pass, where the largest stock pass took 44 bytes.
    assert_eq!(std::mem::size_of::<CustomPost>(), 100);
    assert_eq!(std::mem::size_of::<PostPass>(), 104);
    assert_eq!(std::mem::size_of::<ToneMonoParams>(), 40);
}

#[test]
fn the_registry_builds_a_pass_with_the_manifests_defaults() {
    use crate::assets::{MaterialUniformDefaults, PostEffectRegistry};
    use std::path::Path;

    let mut effects = PostEffectRegistry::new();
    let defaults = MaterialUniformDefaults {
        vec4: [[0.1, 0.2, 0.3, 1.0], [0.0; 4], [0.0; 4], [0.0; 4]],
        f32s: [0.5, 0.0, 0.0, 0.0],
        i32s: [3, 0, 0, 0],
    };
    let heat = effects.allocate(
        "heat",
        "assets/manifest.json".into(),
        "heat_shader".into(),
        defaults,
    );
    let ripple = effects.allocate(
        "ripple",
        "assets/manifest.json".into(),
        "ripple".into(),
        MaterialUniformDefaults::default(),
    );
    assert_ne!(heat, ripple);
    assert_eq!(effects.get("heat"), Some(heat));
    assert_eq!(effects.name_for_id(heat), Some("heat"));
    assert_eq!(effects.shader_name_for_id(heat), Some("heat_shader"));
    assert_eq!(
        effects.path_for_id(heat),
        Some(Path::new("assets/manifest.json"))
    );
    let pass = effects.pass("heat").expect("a declared effect");
    assert_eq!(pass.effect, heat);
    assert_eq!(pass.vec4[0], [0.1, 0.2, 0.3, 1.0]);
    assert_eq!((pass.f32s, pass.i32s), ([0.5, 0.0, 0.0, 0.0], [3, 0, 0, 0]));
    assert!(effects.pass("smoke").is_none());

    // A manifest reload: the effect keeps its ID and takes the new shader
    // and defaults, which the next pass holds; a pass a game already holds
    // keeps its lanes.
    let mut held = pass;
    held.set_f32(0, 0.9);
    let again = effects.allocate(
        "heat",
        "assets/other.json".into(),
        "heat_v2".into(),
        MaterialUniformDefaults {
            f32s: [0.25, 0.0, 0.0, 0.0],
            ..MaterialUniformDefaults::default()
        },
    );
    assert_eq!(again, heat);
    assert_eq!(effects.shader_name_for_id(heat), Some("heat_v2"));
    assert_eq!(
        effects.path_for_id(heat),
        Some(Path::new("assets/other.json"))
    );
    assert_eq!(
        effects
            .defaults_for_id(heat)
            .map(|defaults| defaults.f32s[0]),
        Some(0.25)
    );
    assert_eq!(
        effects.pass("heat").expect("a declared effect").f32s[0],
        0.25
    );
    assert_eq!(held.f32s[0], 0.9);
    let mut names: Vec<&str> = effects.iter().map(|(name, _)| name).collect();
    names.sort_unstable();
    assert_eq!(names, ["heat", "ripple"]);
}
