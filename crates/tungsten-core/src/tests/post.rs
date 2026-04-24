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
fn all_seventeen_variants_round_trip_through_json() {
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
