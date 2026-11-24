use super::compose::{self, Between, Class};
use super::{
    InertParam, ParamSlots, STOCK_CLASSES, STOCK_SHADERS, Signature, SourceGroups, effect_section,
    inert_param, stage_info, stock_class, stock_index,
};
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
fn a_stacks_last_pair_fuses_when_it_ends_in_a_bounded_mix() {
    let bloom = PostPass::Bloom(BloomParams::default());
    let vignette = PostPass::Vignette(VignetteParams {
        inner: 0.38,
        outer: 0.78,
        strength: 0.18,
        color: [0.015, 0.013, 0.02, 1.0],
    });
    let fade = PostPass::Fade(FadeParams::default());
    let tonemap = PostPass::Tonemap(TonemapParams::default());
    let dither = PostPass::Dither(DitherParams::default());
    let grain = PostPass::FilmGrain(FilmGrainParams::default());
    let pixelate = PostPass::Pixelate(4.0);
    // The runs of more than one stage, as (first slot, stages), every
    // shader the engine's own.
    let fused = |passes: &[PostPass]| -> Vec<(usize, usize)> {
        let stages: Vec<_> = passes.iter().map(|pass| stage_info(pass, true)).collect();
        compose::runs(&stages, false)
            .into_iter()
            .filter(|run| run.len() > 1)
            .map(|run| (run.start, run.len()))
            .collect()
    };

    // `D-302`'s pair, wherever it ends the stack.
    assert_eq!(fused(&[bloom, vignette]), [(0, 2)]);
    assert_eq!(fused(&[tonemap, bloom, vignette]), [(1, 2)]);
    // The pairs it never reached: a screen transition over example 01's
    // stack, its game-over tail, any stage behind a fade.
    assert_eq!(fused(&[bloom, vignette, fade]), [(1, 2)]);
    assert_eq!(fused(&[bloom, vignette, pixelate, fade]), [(2, 2)]);
    assert_eq!(fused(&[dither, fade]), [(0, 2)]);
    assert_eq!(fused(&[bloom, fade]), [(0, 2)]);
    // Nothing follows a fused pass, and only a `mix` ends one.
    assert_eq!(fused(&[bloom, vignette, dither]), []);
    assert_eq!(fused(&[bloom, vignette, grain]), []);
    assert_eq!(fused(&[vignette, bloom]), []);
    assert_eq!(fused(&[bloom, tonemap]), []);
    assert_eq!(fused(&[vignette]), []);
    assert_eq!(fused(&[bloom]), []);
    assert_eq!(fused(&[]), []);

    // A replaced file's stage has no class, so it draws alone, as the last
    // stage or the one before it; so does a vignette towards a negative
    // colour.
    let own = |passes: &[(PostPass, bool)]| -> usize {
        let stages: Vec<_> = passes
            .iter()
            .map(|(pass, own)| stage_info(pass, *own))
            .collect();
        compose::runs(&stages, false)
            .iter()
            .filter(|run| run.len() > 1)
            .count()
    };
    assert_eq!(own(&[(bloom, true), (vignette, true)]), 1);
    assert_eq!(own(&[(bloom, false), (vignette, true)]), 0);
    assert_eq!(own(&[(bloom, true), (vignette, false)]), 0);
    let negative = PostPass::Vignette(VignetteParams {
        color: [-0.1, 0.0, 0.0, 1.0],
        ..VignetteParams::default()
    });
    assert_eq!(fused(&[bloom, negative]), []);

    // The class the rule reads is the stock table's.
    assert_eq!(stock_class(&vignette), Some(Class::Mix));
    assert_eq!(stock_class(&dither), Some(Class::Steps));
    assert_eq!(stock_class(&bloom), None);
    assert_eq!(stage_info(&bloom, true).class, Some(Class::Head));
    assert_eq!(stage_info(&bloom, false).class, None);
}

#[test]
fn a_fused_pass_is_known_by_its_stages_and_not_their_values() {
    let bloom = PostPass::Bloom(BloomParams::default());
    let vignette = PostPass::Vignette(VignetteParams::default());
    let fade = PostPass::Fade(FadeParams::default());
    let pair = Signature::of(&[bloom, vignette], Between::Clamp).expect("a pair");
    // Other values, the same pass.
    let louder = PostPass::Bloom(BloomParams {
        intensity: 1.5,
        ..BloomParams::default()
    });
    let darker = PostPass::Vignette(VignetteParams {
        strength: 0.9,
        ..VignetteParams::default()
    });
    assert_eq!(Signature::of(&[louder, darker], Between::Clamp), Some(pair));
    // Other stages, another pass.
    let other = Signature::of(&[vignette, fade], Between::Clamp).expect("a pair");
    assert_ne!(pair, other);
    assert_ne!(
        Signature::of(&[fade, vignette], Between::Clamp),
        Some(other),
        "the order is the pass's"
    );
    // A reload names a shader: the passes that hold its effect go.
    assert!(pair.holds("bloom_composite") && pair.holds("vignette"));
    assert!(!pair.holds("fade") && !pair.holds("bloom_threshold"));
    assert!(other.holds("fade") && !other.holds("bloom_composite"));
    // A bloom only leads, and a pass holds eight stages at most.
    assert_eq!(Signature::of(&[vignette, bloom], Between::Clamp), None);
    assert_eq!(Signature::of(&[], Between::Clamp), None);
    assert!(Signature::of(&[fade; 8], Between::Clamp).is_some());
    assert_eq!(Signature::of(&[fade; 9], Between::Clamp), None);
}

#[test]
fn every_effect_of_a_signature_has_its_section_in_the_compiled_in_text() {
    // The 17 stock rows and bloom's composite, each under its class and
    // its own name.
    for effect in 0..=17u8 {
        let section = effect_section(effect).expect("a section");
        let class = STOCK_CLASSES
            .get(usize::from(effect))
            .copied()
            .unwrap_or(Class::Head);
        assert_eq!(section.class, class, "{}", section.name);
        let entry = if class.follows() { "_fx(" } else { "_head(" };
        assert!(
            section
                .text
                .contains(&format!("fn {}{entry}", section.name)),
            "{}",
            section.name
        );
    }
    assert_eq!(
        effect_section(17).expect("a section").name,
        "bloom_composite"
    );
    assert_eq!(
        effect_section(1).expect("a section").name,
        STOCK_SHADERS[1].0
    );
}
