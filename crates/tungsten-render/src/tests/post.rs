use super::{ParamSlots, PostStackRenderer, STOCK_SHADERS, source_slot, stock_index};
use crate::passes::TargetId;
use tungsten_core::post::{
    BloomParams, ColorAdjustParams, CrtParams, DissolveParams, DitherParams, FadeParams,
    FilmGrainParams, FogParams, GlitchParams, GodRaysParams, LutParams, PixelOutlineParams,
    PostPass, ToneMonoParams, TonemapParams, VignetteParams, WipeRadialParams,
};

#[test]
fn plan_empty_stack_produces_no_entries() {
    let plan = PostStackRenderer::plan_targets(0);
    assert!(plan.is_empty());
}

#[test]
fn plan_single_pass_reads_scene_writes_ping() {
    let plan = PostStackRenderer::plan_targets(1);
    assert_eq!(plan, vec![(TargetId::SceneColor, TargetId::PostPing)]);
}

#[test]
fn plan_two_passes_chains_ping_then_pong() {
    let plan = PostStackRenderer::plan_targets(2);
    assert_eq!(
        plan,
        vec![
            (TargetId::SceneColor, TargetId::PostPing),
            (TargetId::PostPing, TargetId::PostPong),
        ]
    );
}

#[test]
fn plan_three_passes_returns_to_ping() {
    let plan = PostStackRenderer::plan_targets(3);
    assert_eq!(
        plan,
        vec![
            (TargetId::SceneColor, TargetId::PostPing),
            (TargetId::PostPing, TargetId::PostPong),
            (TargetId::PostPong, TargetId::PostPing),
        ]
    );
}

#[test]
fn plan_seventeen_passes_stays_valid_all_the_way() {
    let plan = PostStackRenderer::plan_targets(17);
    assert_eq!(plan.len(), 17);
    for (i, &(src, dst)) in plan.iter().enumerate() {
        let expected_dst = if i.is_multiple_of(2) {
            TargetId::PostPing
        } else {
            TargetId::PostPong
        };
        assert_eq!(dst, expected_dst, "pass {i}");
        if i == 0 {
            assert_eq!(src, TargetId::SceneColor);
        } else {
            let prev_dst = if (i - 1).is_multiple_of(2) {
                TargetId::PostPing
            } else {
                TargetId::PostPong
            };
            assert_eq!(src, prev_dst, "pass {i} src follows previous dst");
        }
    }
}

#[test]
fn every_source_of_the_ladder_has_a_cache_slot_of_its_own() {
    // The three targets a stock pass can sample, as `plan_targets` names them.
    let mut slots: Vec<usize> = PostStackRenderer::plan_targets(17)
        .iter()
        .map(|&(src, _)| source_slot(src))
        .collect();
    slots.sort_unstable();
    slots.dedup();
    assert_eq!(slots, vec![0, 1, 2]);
    assert_ne!(
        source_slot(TargetId::PostPing),
        source_slot(TargetId::PostPong)
    );
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
