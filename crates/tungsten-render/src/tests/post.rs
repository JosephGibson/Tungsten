use super::{
    InertParam, ParamSlots, STOCK_SHADERS, SourceGroups, VIGNETTE_SHADER_NAME, inert_param,
    stock_index, trailing_vignette,
};
use tungsten_core::post::PostStack;
use tungsten_core::post::{
    BloomParams, ColorAdjustParams, CrtParams, DissolveParams, DitherParams, FadeParams,
    FilmGrainParams, FogParams, GlitchParams, GodRaysParams, LutParams, PixelOutlineParams,
    PostPass, ToneMonoParams, TonemapParams, VignetteParams, WipeRadialParams,
};

#[test]
fn a_source_group_is_built_once_a_texture_and_goes_with_it() {
    // A stack's passes sample two or three pooled textures in turn: each
    // gets one bind group, found again every frame.
    let mut groups: SourceGroups<u32> = SourceGroups::default();
    let mut built = 0;
    let mut get = |groups: &mut SourceGroups<u32>, id| {
        *groups.get_or_build(id, || {
            built += 1;
            built
        })
    };
    for _ in 0..3 {
        assert_eq!(get(&mut groups, 4), 1);
        assert_eq!(get(&mut groups, 9), 2);
        assert_eq!(get(&mut groups, 4), 1);
    }
    assert_eq!(groups.len(), 2);
    // A freed texture's group goes; the other texture's stays.
    groups.forget(&[4]);
    assert_eq!(groups.len(), 1);
    assert_eq!(get(&mut groups, 9), 2);
    assert_eq!(get(&mut groups, 4), 3);
}

/// Stages `payload` in `slot`, counting builds in `built`: returns the slot's
/// objects (the build count at the time it was built) and whether a write is
/// due.
fn stage(
    slots: &mut ParamSlots<u32>,
    built: &mut u32,
    slot: usize,
    payload: [u8; 256],
) -> (u32, bool) {
    let (objects, stale) = slots.stage(slot, &payload, || {
        *built += 1;
        *built
    });
    (*objects, stale)
}

#[test]
fn two_passes_of_one_effect_keep_params_of_their_own() {
    // The same effect in slots 0 and 1 with different parameters: each slot
    // gets objects of its own and takes its own write.
    let (red, blue) = ([1u8; 256], [2u8; 256]);
    let mut slots = ParamSlots::default();
    let mut built = 0;

    assert_eq!(stage(&mut slots, &mut built, 0, red), (1, true));
    assert_eq!(stage(&mut slots, &mut built, 1, blue), (2, true));

    // The next frame finds both buffers holding their bytes: nothing is built
    // and nothing is written.
    assert_eq!(stage(&mut slots, &mut built, 0, red), (1, false));
    assert_eq!(stage(&mut slots, &mut built, 1, blue), (2, false));
    assert_eq!(built, 2);
}

#[test]
fn swapped_params_are_written_to_both_slots() {
    let (red, blue) = ([1u8; 256], [2u8; 256]);
    let mut slots = ParamSlots::default();
    let mut built = 0;
    stage(&mut slots, &mut built, 0, red);
    stage(&mut slots, &mut built, 1, blue);

    // Reordering the stack moves the payloads, not the slots' objects.
    assert_eq!(stage(&mut slots, &mut built, 0, blue), (1, true));
    assert_eq!(stage(&mut slots, &mut built, 1, red), (2, true));
    assert_eq!(stage(&mut slots, &mut built, 0, blue), (1, false));
    assert_eq!(built, 2);
}

#[test]
fn a_far_slot_leaves_the_others_alone() {
    // A stack whose only stock pass sits at index 5 (bloom slots before it)
    // builds one entry; an earlier slot used later gets its own.
    let payload = [7u8; 256];
    let mut slots = ParamSlots::default();
    let mut built = 0;

    assert_eq!(stage(&mut slots, &mut built, 5, payload), (1, true));
    assert_eq!(built, 1);
    assert_eq!(stage(&mut slots, &mut built, 2, payload), (2, true));
    assert_eq!(stage(&mut slots, &mut built, 5, payload), (1, false));
    assert_eq!(built, 2);
}

#[test]
fn stock_shader_table_covers_every_post_pass() {
    let passes = [
        PostPass::Tonemap(TonemapParams::default()),
        PostPass::Vignette(VignetteParams::default()),
        PostPass::Lut(LutParams::default()),
        PostPass::ChromaticAberration(1.0),
        PostPass::ColorAdjust(ColorAdjustParams::default()),
        PostPass::ToneMono(ToneMonoParams::default()),
        PostPass::Crt(CrtParams::default()),
        PostPass::FilmGrain(FilmGrainParams::default()),
        PostPass::Dither(DitherParams::default()),
        PostPass::PixelOutline(PixelOutlineParams::default()),
        PostPass::Fade(FadeParams::default()),
        PostPass::WipeRadial(WipeRadialParams::default()),
        PostPass::Dissolve(DissolveParams::default()),
        PostPass::Glitch(GlitchParams::default()),
        PostPass::Pixelate(4.0),
        PostPass::Fog(FogParams::default()),
        PostPass::GodRays(GodRaysParams::default()),
        PostPass::Bloom(BloomParams::default()),
    ];

    let mut rows = Vec::new();
    for pass in &passes {
        // No wildcard: a new `PostPass` variant stops this test compiling
        // until it is listed here and in `passes`.
        let stock = match pass {
            PostPass::Bloom(_) => false,
            PostPass::Tonemap(_)
            | PostPass::Vignette(_)
            | PostPass::Lut(_)
            | PostPass::ChromaticAberration(_)
            | PostPass::ColorAdjust(_)
            | PostPass::ToneMono(_)
            | PostPass::Crt(_)
            | PostPass::FilmGrain(_)
            | PostPass::Dither(_)
            | PostPass::PixelOutline(_)
            | PostPass::Fade(_)
            | PostPass::WipeRadial(_)
            | PostPass::Dissolve(_)
            | PostPass::Glitch(_)
            | PostPass::Pixelate(_)
            | PostPass::Fog(_)
            | PostPass::GodRays(_) => true,
        };
        let row = stock_index(pass);
        assert_eq!(row.is_some(), stock, "{}", pass.kind_name());
        if let Some(row) = row {
            // The table's shader ID is the manifest's, which is the pass's
            // kind name: a reload of that file finds this pass's pipeline.
            assert_eq!(STOCK_SHADERS[row].0, pass.kind_name());
            rows.push(row);
        }
    }

    // Every row is used by exactly one variant.
    rows.sort_unstable();
    assert_eq!(rows, (0..STOCK_SHADERS.len()).collect::<Vec<_>>());
}

#[test]
fn settings_that_draw_nothing_or_not_as_named_are_found() {
    use tungsten_core::post::DitherMode;
    let dither = |mode| {
        PostPass::Dither(DitherParams {
            mode,
            ..DitherParams::default()
        })
    };
    assert_eq!(inert_param(&dither(DitherMode::Bayer4), 1.0), None);
    assert_eq!(
        inert_param(&dither(DitherMode::Bayer8), 1.0),
        Some(InertParam::DitherMode)
    );
    assert_eq!(
        inert_param(&dither(DitherMode::BlueNoise), 1.0),
        Some(InertParam::DitherMode)
    );

    assert_eq!(inert_param(&PostPass::Crt(CrtParams::default()), 1.0), None);
    let bleeding = CrtParams {
        bleed: 0.1,
        ..CrtParams::default()
    };
    assert_eq!(
        inert_param(&PostPass::Crt(bleeding), 1.0),
        Some(InertParam::CrtBleed)
    );

    let outline = PostPass::PixelOutline(PixelOutlineParams::default());
    assert_eq!(
        inert_param(&outline, 1.0),
        Some(InertParam::OutlineClearAlpha),
        "the default clear's alpha"
    );
    assert_eq!(inert_param(&outline, 0.0), None);
}

#[test]
fn a_vignette_folds_into_bloom_only_as_the_stacks_last_pass_after_a_bloom() {
    let bloom = PostPass::Bloom(BloomParams::default());
    let own = VignetteParams {
        inner: 0.38,
        outer: 0.78,
        strength: 0.18,
        color: [0.015, 0.013, 0.02, 1.0],
    };
    let vignette = PostPass::Vignette(own);
    let other = PostPass::Vignette(VignetteParams::default());
    let tonemap = PostPass::Tonemap(TonemapParams::default());
    let dither = PostPass::Dither(DitherParams::default());
    let folds = |passes: &[PostPass]| trailing_vignette(&PostStack(passes.to_vec()), false, true);

    // Bloom then vignette, ending the stack: the composite draws it, with
    // its own values.
    assert_eq!(folds(&[bloom, vignette]), Some(own));
    assert_eq!(folds(&[tonemap, bloom, vignette]), Some(own));
    // Of two vignettes after a bloom the last keeps its pass: it does not
    // follow the bloom. A vignette that does, with a pass after it, keeps
    // its own too: the pass after could turn one rounding step into more.
    assert_eq!(folds(&[bloom, vignette, other]), None);
    assert_eq!(folds(&[bloom, vignette, dither]), None);
    // Any other order plans as before.
    assert_eq!(folds(&[vignette, bloom]), None);
    assert_eq!(folds(&[bloom, tonemap, vignette]), None);
    assert_eq!(folds(&[vignette]), None);
    assert_eq!(folds(&[bloom]), None);
    assert_eq!(folds(&[]), None);

    // SMAA reads the stack's output: its edge test keeps the vignette's own
    // pass. A game's own vignette or composite shader does as well.
    let stack = PostStack(vec![bloom, vignette]);
    assert_eq!(trailing_vignette(&stack, true, true), None);
    assert_eq!(trailing_vignette(&stack, false, false), None);
    assert_eq!(trailing_vignette(&stack, false, true), Some(own));
}

#[test]
fn the_vignettes_shader_name_is_its_stock_row() {
    // A reload names the shader it replaced; the fold ends on this name.
    let index = stock_index(&PostPass::Vignette(VignetteParams::default())).unwrap();
    assert_eq!(STOCK_SHADERS[index].0, VIGNETTE_SHADER_NAME);
}
