use super::*;

const BLACK: [f32; 4] = [0.0, 0.0, 0.0, 1.0];
const EDGE: [f32; 4] = [1.0, 0.5, 0.0, 1.0];

fn wipe(center: [f32; 2], softness: f32, cover: f32) -> WipeRadialParams {
    match (TransitionEffect::WipeRadial { center, softness }).pass_at(cover) {
        PostPass::WipeRadial(params) => params,
        other => panic!("expected a radial wipe, got {other:?}"),
    }
}

fn fade(secs: f32) -> Transition {
    Transition::new(TransitionEffect::Fade { color: BLACK }, secs)
}

#[test]
fn fade_progress_equals_cover() {
    let effect = TransitionEffect::Fade { color: BLACK };
    for cover in [0.0, 0.25, 1.0] {
        match effect.pass_at(cover) {
            PostPass::Fade(params) => {
                assert_eq!(params.progress, cover);
                assert_eq!(params.color, BLACK);
            }
            other => panic!("expected a fade, got {other:?}"),
        }
    }
}

#[test]
fn wipe_radial_is_inverted() {
    // The shader shows the frame inside `progress * 1.5`: an uncovered frame
    // needs the largest progress and a covered one needs 0.
    let open = wipe([0.5, 0.5], 0.05, 0.0);
    let half = wipe([0.5, 0.5], 0.05, 0.5);
    let closed = wipe([0.5, 0.5], 0.05, 1.0);
    assert!(open.progress > half.progress);
    assert!(half.progress > closed.progress);
    assert_eq!(closed.progress, 0.0);
    assert_eq!(open.center, [0.5, 0.5]);
}

#[test]
fn wipe_radial_starts_at_the_far_corner() {
    // At cover 0 the circle's inner edge (`radius - softness`) sits on the
    // farthest corner, so that corner darkens as soon as cover rises.
    for (center, far) in [
        ([0.5, 0.5], 0.5_f32.hypot(0.5)),
        ([0.25, 0.5], 0.75_f32.hypot(0.5)),
        ([0.0, 0.0], 1.0_f32.hypot(1.0)),
    ] {
        let softness = 0.05;
        let params = wipe(center, softness, 0.0);
        let inner_edge = params.progress * 1.5 - params.softness;
        assert!(
            (inner_edge - far).abs() < 1.0e-5,
            "center {center:?}: inner edge {inner_edge}, farthest corner {far}"
        );
    }
}

#[test]
fn wipe_radial_softness_vanishes_at_full_cover() {
    assert_eq!(wipe([0.5, 0.5], 0.2, 0.0).softness, 0.2);
    assert!((wipe([0.5, 0.5], 0.2, 0.5).softness - 0.1).abs() < 1.0e-6);
    assert_eq!(wipe([0.5, 0.5], 0.2, 1.0).softness, 0.0);
    // A negative or non-finite softness counts as none.
    assert_eq!(wipe([0.5, 0.5], -1.0, 0.0).softness, 0.0);
    assert_eq!(wipe([0.5, 0.5], f32::NAN, 0.0).softness, 0.0);
}

#[test]
fn dissolve_progress_equals_cover() {
    let effect = TransitionEffect::Dissolve {
        noise_scale: 12.0,
        edge_color: EDGE,
    };
    for cover in [0.0, 0.6, 1.0] {
        match effect.pass_at(cover) {
            PostPass::Dissolve(params) => {
                assert_eq!(params.progress, cover);
                assert_eq!(params.noise_scale, 12.0);
                assert_eq!(params.edge_color, EDGE);
            }
            other => panic!("expected a dissolve, got {other:?}"),
        }
    }
}

#[test]
fn pixelate_block_runs_from_one_to_max() {
    let block = |max_block_px: f32, cover: f32| match (TransitionEffect::Pixelate { max_block_px })
        .pass_at(cover)
    {
        PostPass::Pixelate(px) => px,
        other => panic!("expected pixelate, got {other:?}"),
    };
    assert_eq!(block(33.0, 0.0), 1.0);
    assert_eq!(block(33.0, 0.5), 17.0);
    assert_eq!(block(33.0, 1.0), 33.0);
    // A maximum below one pixel never shrinks the block under 1.
    assert_eq!(block(0.25, 1.0), 1.0);
}

#[test]
fn cover_is_clamped() {
    let effect = TransitionEffect::Fade { color: BLACK };
    let progress = |cover: f32| match effect.pass_at(cover) {
        PostPass::Fade(params) => params.progress,
        other => panic!("expected a fade, got {other:?}"),
    };
    assert_eq!(progress(-0.5), 0.0);
    assert_eq!(progress(1.5), 1.0);
    assert_eq!(progress(f32::NAN), 0.0);
}

#[test]
fn out_reaches_boundary_at_out_secs() {
    let transition = fade(0.5);
    let mut state = TransitionState::START;
    assert_eq!(transition.cover(state), 0.0);

    assert_eq!(
        transition.advance(&mut state, 0.25),
        TransitionStep::Running
    );
    assert_eq!(state.phase, TransitionPhase::Out);
    assert!((transition.cover(state) - 0.5).abs() < 1.0e-6);

    assert_eq!(
        transition.advance(&mut state, 0.125),
        TransitionStep::Running
    );
    assert_eq!(
        transition.advance(&mut state, 0.125),
        TransitionStep::Boundary
    );
    assert_eq!(transition.cover(state), 1.0);
}

#[test]
fn in_starts_fully_covered_and_drops_leftover_time() {
    let transition = fade(0.5);
    let mut state = TransitionState::START;
    // One long step overshoots `Out` by 0.4 s.
    assert_eq!(
        transition.advance(&mut state, 0.9),
        TransitionStep::Boundary
    );

    assert_eq!(transition.begin_in(&mut state), TransitionStep::Running);
    assert_eq!(state.phase, TransitionPhase::In);
    assert_eq!(state.elapsed, 0.0, "the overshoot is dropped");
    assert_eq!(
        transition.cover(state),
        1.0,
        "the boundary frame is covered"
    );

    assert_eq!(
        transition.advance(&mut state, 0.25),
        TransitionStep::Running
    );
    assert!((transition.cover(state) - 0.5).abs() < 1.0e-6);
    assert_eq!(
        transition.advance(&mut state, 0.25),
        TransitionStep::Finished
    );
    assert_eq!(transition.cover(state), 0.0);
}

#[test]
fn zero_durations_finish_on_the_first_step() {
    for secs in [0.0, -1.0, f32::NAN, f32::INFINITY] {
        let transition = fade(secs);
        let mut state = TransitionState::START;
        assert_eq!(
            transition.advance(&mut state, 0.0),
            TransitionStep::Boundary,
            "out_secs {secs}"
        );
        assert_eq!(
            transition.begin_in(&mut state),
            TransitionStep::Finished,
            "in_secs {secs}"
        );
    }

    // A negative or non-finite time step does not move the clock.
    let transition = fade(0.5);
    let mut state = TransitionState::START;
    assert_eq!(
        transition.advance(&mut state, -1.0),
        TransitionStep::Running
    );
    assert_eq!(
        transition.advance(&mut state, f32::NAN),
        TransitionStep::Running
    );
    assert_eq!(state.elapsed, 0.0);
}

#[test]
fn easing_shapes_cover() {
    let transition = fade(1.0).with_easing(Easing::QuadIn);
    let out_half = TransitionState {
        phase: TransitionPhase::Out,
        elapsed: 0.5,
    };
    let in_half = TransitionState {
        phase: TransitionPhase::In,
        elapsed: 0.5,
    };
    assert!((transition.cover(out_half) - 0.25).abs() < 1.0e-6);
    assert!((transition.cover(in_half) - 0.75).abs() < 1.0e-6);

    // An overshooting easing still yields a cover in [0, 1].
    let back = fade(1.0).with_easing(Easing::BackOut);
    for elapsed in [0.0, 0.3, 0.6, 0.9, 1.0] {
        for phase in [TransitionPhase::Out, TransitionPhase::In] {
            let cover = back.cover(TransitionState { phase, elapsed });
            assert!((0.0..=1.0).contains(&cover), "cover {cover}");
        }
    }

    // The phases draw their own effect.
    let mixed = fade(1.0).with_in_effect(TransitionEffect::Pixelate { max_block_px: 9.0 });
    assert!(matches!(mixed.pass(out_half), PostPass::Fade(_)));
    assert!(matches!(mixed.pass(in_half), PostPass::Pixelate(_)));
}
