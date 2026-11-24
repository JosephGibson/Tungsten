//! The declared frame against the frame the renderer drew before the graph.
//!
//! `reference` is that frame, transcribed: the pass order of the old
//! `default_pass_order`, the post stack's ping-pong ladder, bloom's passes
//! and every load op as the recorder chose it. The declaration must draw
//! the same passes with the same data flowing between them.

use super::*;
use crate::graph::{At, Compiled, Load, Target};

const FORMAT: TextureFormat = TextureFormat::Bgra8UnormSrgb;
const SIZE: (u32, u32) = (1280, 720);

/// A target of the old frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Role {
    SceneColor,
    SceneColorMsaa,
    PostPing,
    PostPong,
    SmaaEdges,
    SmaaBlend,
    PresentSource,
    /// What the old frame called the swapchain: the surface, or a direct
    /// capture's stand-in.
    Swapchain,
    /// The surface behind a direct capture's stand-in.
    Surface,
    /// A level of the bloom pyramid.
    Bloom(u32),
}

#[derive(Debug, Clone, PartialEq)]
struct RefPass {
    label: String,
    timing: String,
    color: Role,
    clears: bool,
    resolve: Option<Role>,
    depth: bool,
    reads: Vec<Role>,
}

fn ref_pass(
    label: impl Into<String>,
    timing: impl Into<String>,
    color: Role,
    clears: bool,
    reads: &[Role],
) -> RefPass {
    RefPass {
        label: label.into(),
        timing: timing.into(),
        color,
        clears,
        resolve: None,
        depth: false,
        reads: reads.to_vec(),
    }
}

/// The old ladder: slot `i` writes ping when even, pong when odd.
fn ladder(slot: usize) -> Role {
    if slot.is_multiple_of(2) {
        Role::PostPing
    } else {
        Role::PostPong
    }
}

/// What the stack's output was called in the old frame, and the overlay's
/// target on a blit frame without SMAA.
fn stack_output(len: usize) -> Role {
    if len == 0 {
        Role::SceneColor
    } else {
        ladder(len - 1)
    }
}

/// The old frame for `shape`, and the index its acquire split fell on.
fn reference(shape: &Shape) -> (Vec<RefPass>, usize) {
    let len = shape.stages.len();
    let direct = shape.present != PresentPath::Capture;
    let scene_is_last = direct && len == 0 && !shape.smaa;
    let mut passes = Vec::new();

    // The scene: into the swapchain, or resolving into it, when no stage
    // follows; always cleared.
    let (color, resolve) = match (shape.samples > 1, scene_is_last) {
        (true, true) => (Role::SceneColorMsaa, Some(Role::Swapchain)),
        (true, false) => (Role::SceneColorMsaa, Some(Role::SceneColor)),
        (false, true) => (Role::Swapchain, None),
        (false, false) => (Role::SceneColor, None),
    };
    let mut scene = ref_pass("tungsten_scene_pass", "scene", color, true, &[]);
    scene.resolve = resolve;
    scene.depth = shape.depth;
    passes.push(scene);

    // Where each slot's passes begin: the old split was counted in slots.
    let mut slot_start = Vec::new();
    for (slot, stage) in shape.stages.iter().enumerate() {
        slot_start.push(passes.len());
        let source = if slot == 0 {
            Role::SceneColor
        } else {
            ladder(slot - 1)
        };
        let writes_swapchain = direct && !shape.smaa && slot + 1 == len;
        let target = if writes_swapchain {
            Role::Swapchain
        } else {
            ladder(slot)
        };
        let label = if slot < 32 {
            format!("tungsten_post_pass_{slot}")
        } else {
            "tungsten_post_pass_overflow".to_string()
        };
        match stage {
            Stage::Stock(name) => {
                // A stock pass loads offscreen and clears the swapchain.
                passes.push(ref_pass(
                    label,
                    format!("post{slot}_{name}"),
                    target,
                    writes_swapchain,
                    &[source],
                ));
            }
            Stage::Bloom => {
                let mips = shape.bloom_mips;
                let threshold = if shape.bloom_halvings >= 2 {
                    format!("post{slot}_bloom_threshold_quarter")
                } else {
                    format!("post{slot}_bloom_threshold")
                };
                passes.push(ref_pass(
                    "tungsten_bloom_threshold",
                    threshold,
                    Role::Bloom(0),
                    false,
                    &[source],
                ));
                for level in 1..mips {
                    passes.push(ref_pass(
                        "tungsten_bloom_downsample",
                        format!("post{slot}_bloom_down{level}"),
                        Role::Bloom(level),
                        false,
                        &[Role::Bloom(level - 1)],
                    ));
                }
                for level in (0..mips - 1).rev() {
                    passes.push(ref_pass(
                        "tungsten_bloom_upsample",
                        format!("post{slot}_bloom_up{level}"),
                        Role::Bloom(level),
                        false,
                        &[Role::Bloom(level + 1)],
                    ));
                }
                let vignette = shape.vignette && slot + 1 == len;
                let composite = if vignette {
                    format!("post{slot}_bloom_composite_vignette")
                } else {
                    format!("post{slot}_bloom_composite")
                };
                passes.push(ref_pass(
                    "tungsten_bloom_composite",
                    composite,
                    target,
                    writes_swapchain,
                    &[source, Role::Bloom(0)],
                ));
            }
        }
    }
    // The old split (`acquire_split`): SMAA's neighborhood pass; without
    // SMAA the last post slot, whole; with neither the scene.
    let mut split = slot_start.last().copied().unwrap_or(0);

    let source = stack_output(len);
    let mut overlay = if direct { Role::Swapchain } else { source };
    if shape.smaa {
        passes.push(ref_pass(
            "tungsten_smaa_edge_pass",
            "smaa_edges",
            Role::SmaaEdges,
            true,
            &[source],
        ));
        passes.push(ref_pass(
            "tungsten_smaa_blend_weights_pass",
            "smaa_blend_weights",
            Role::SmaaBlend,
            true,
            &[Role::SmaaEdges],
        ));
        split = passes.len();
        let target = if direct {
            Role::Swapchain
        } else {
            Role::PresentSource
        };
        passes.push(ref_pass(
            "tungsten_smaa_neighborhood_pass",
            "smaa_neighborhood",
            target,
            true,
            &[source, Role::SmaaBlend],
        ));
        overlay = target;
    }
    passes.push(ref_pass(
        "tungsten_text_overlay_pass",
        "text",
        overlay,
        false,
        &[],
    ));
    match shape.present {
        PresentPath::Direct => {}
        // The blit loaded the swapchain.
        PresentPath::Capture => {
            passes.push(ref_pass(
                "tungsten_present_pass",
                "present",
                Role::Swapchain,
                false,
                &[overlay],
            ));
        }
        PresentPath::DirectCapture => {
            passes.push(ref_pass(
                "tungsten_present_pass",
                "present",
                Role::Surface,
                false,
                &[Role::Swapchain],
            ));
        }
    }
    (passes, split)
}

/// The pass that last wrote `role` before pass `index`, as colour or
/// resolve target.
fn ref_producer(passes: &[RefPass], index: usize, role: Role) -> Option<usize> {
    passes[..index]
        .iter()
        .rposition(|pass| pass.color == role || pass.resolve == Some(role))
}

fn producer(compiled: &Compiled<PassKind>, index: usize, at: At) -> Option<usize> {
    compiled.passes[..index]
        .iter()
        .rposition(|pass| pass.color.at == at || pass.resolve == Some(at))
}

fn imported(role: Role) -> Option<Target> {
    match role {
        Role::Swapchain => Some(Target::Imported(0)),
        Role::Surface => Some(Target::Imported(1)),
        _ => None,
    }
}

fn shape(stages: &[Stage]) -> Shape {
    Shape {
        output: SIZE,
        format: FORMAT,
        samples: 1,
        depth: false,
        smaa: false,
        present: PresentPath::Direct,
        stages: stages.to_vec(),
        vignette: false,
        bloom_mips: 6,
        bloom_halvings: 1,
    }
}

const V: Stage = Stage::Stock("vignette");
const F: Stage = Stage::Stock("fade");
const D: Stage = Stage::Stock("dither");
const B: Stage = Stage::Bloom;

/// Every shape the old order's arguments spanned, and bloom's.
fn matrix() -> Vec<Shape> {
    let seventeen: Vec<Stage> = (0..17)
        .map(|slot| if slot % 3 == 0 { F } else { V })
        .collect();
    let stacks: Vec<(Vec<Stage>, bool)> = vec![
        (vec![], false),
        (vec![V], false),
        (vec![V, F], false),
        (vec![V, F, D], false),
        (seventeen, false),
        (vec![B], false),
        (vec![B], true),
        (vec![B, V], false),
        (vec![V, B], false),
        (vec![V, B], true),
        (vec![F, B, V], false),
        (vec![B, B], false),
        (vec![B, F, B], true),
    ];
    let mut shapes = Vec::new();
    for (stages, vignette) in &stacks {
        for samples in [1, 4] {
            for depth in [false, true] {
                for smaa in [false, true] {
                    for present in [
                        PresentPath::Direct,
                        PresentPath::Capture,
                        PresentPath::DirectCapture,
                    ] {
                        for (bloom_mips, bloom_halvings) in [(6, 1), (5, 2), (1, 1)] {
                            // The fold never applied under an SMAA tail.
                            if *vignette && smaa {
                                continue;
                            }
                            shapes.push(Shape {
                                samples,
                                depth,
                                smaa,
                                present,
                                vignette: *vignette,
                                bloom_mips,
                                bloom_halvings,
                                ..shape(stages)
                            });
                        }
                    }
                }
            }
        }
    }
    shapes
}

#[test]
fn the_declaration_draws_the_old_orders_frame() {
    let shapes = matrix();
    assert_eq!(shapes.len(), 828);
    for shape in &shapes {
        let compiled = declare(shape)
            .compile()
            .unwrap_or_else(|error| panic!("{shape:?}: {error:?}"));
        let (old, old_split) = reference(shape);
        let context = format!("{shape:?}");
        assert_eq!(
            compiled.passes.len(),
            old.len(),
            "no pass dropped or added: {context}"
        );

        for (index, (new, old_pass)) in compiled.passes.iter().zip(&old).enumerate() {
            let at = format!("pass {index} ({}) of {context}", old_pass.timing);
            assert_eq!(new.label, old_pass.label, "{at}");
            assert_eq!(new.timing, old_pass.timing, "{at}");

            // The surface and the stand-in are where they were.
            assert_eq!(
                imported(old_pass.color),
                match new.color.at.target {
                    Target::Imported(_) => Some(new.color.at.target),
                    Target::Slot(_) => None,
                },
                "{at}"
            );
            assert_eq!(
                old_pass.resolve.map(imported),
                new.resolve.map(|resolve| match resolve.target {
                    Target::Imported(_) => Some(resolve.target),
                    Target::Slot(_) => None,
                }),
                "{at}"
            );
            assert_eq!(new.depth.is_some(), old_pass.depth, "{at}");

            // The ops: a clear where the old pass cleared, a load where it
            // loaded. A capture's blit is the one change, on either capture
            // path: it loaded the surface, which made wgpu clear it in a
            // pass of its own, and now clears it itself.
            let clears = matches!(new.color.load, Load::Clear(_));
            if new.kind == PassKind::Blit {
                assert_ne!(shape.present, PresentPath::Direct, "{at}");
                assert!(clears && !old_pass.clears, "{at}");
            } else {
                assert_eq!(clears, old_pass.clears, "{at}");
            }
            if let Load::Clear(color) = new.color.load {
                assert_eq!(color, wgpu::Color::TRANSPARENT, "{at}");
            }

            // Every result is stored but the multisampled colour, which its
            // resolve keeps, and depth, which nothing reads.
            assert_eq!(new.color.store, new.resolve.is_none(), "{at}");
            assert!(
                new.depth
                    .is_none_or(|depth| !depth.store && depth.clear == 1.0),
                "{at}"
            );

            // The data: each source was last written by the pass that wrote
            // it in the old frame.
            assert_eq!(new.reads.len(), old_pass.reads.len(), "{at}");
            for (read, role) in new.reads.iter().zip(&old_pass.reads) {
                assert_eq!(
                    producer(&compiled, index, *read),
                    ref_producer(&old, index, *role),
                    "{at}: the source {role:?}"
                );
                assert!(
                    ref_producer(&old, index, *role).is_some(),
                    "{at}: {role:?} was written"
                );
            }
            // And a pass that draws over its target draws over the same
            // pass's result.
            if !clears && new.reads.is_empty() {
                assert_eq!(
                    producer(&compiled, index, new.color.at),
                    ref_producer(&old, index, old_pass.color),
                    "{at}: what it draws over"
                );
            }

            // What each pass draws into is the kind of texture it was.
            if let Target::Slot(slot) = new.color.at.target {
                let spec = compiled.slots[slot as usize];
                let (format, class, samples) = match old_pass.color {
                    Role::SceneColorMsaa => (FORMAT, Class::Multisampled, shape.samples),
                    Role::SmaaEdges => (SMAA_EDGES_FORMAT, Class::Work, 1),
                    Role::SmaaBlend => (SMAA_BLEND_FORMAT, Class::Work, 1),
                    Role::Bloom(_) => (BLOOM_PYRAMID_FORMAT, Class::Work, 1),
                    _ => (FORMAT, Class::Color, 1),
                };
                assert_eq!(
                    (spec.format, spec.class, spec.samples),
                    (format, class, samples),
                    "{at}"
                );
                match old_pass.color {
                    Role::Bloom(level) => {
                        assert_eq!(new.color.at.mip, level, "{at}");
                        assert_eq!(spec.mips, shape.bloom_mips, "{at}");
                        assert_eq!(
                            spec.size,
                            bloom_mip_extent_for(SIZE.0, SIZE.1, shape.bloom_halvings, 0),
                            "{at}"
                        );
                    }
                    _ => assert_eq!(
                        (spec.size, spec.mips, new.color.at.mip),
                        (SIZE, 1, 0),
                        "{at}"
                    ),
                }
            }
        }

        // The split is the first pass on an import. It is the old split
        // but in two cases: a stack that ends in bloom on the surface now
        // records its pyramid passes before the acquire (the old split fell
        // on the whole slot), and a capture frame, whose stages are all
        // offscreen, now splits at its blit.
        let on_import = |pass: &crate::graph::CompiledPass<PassKind>| {
            matches!(pass.color.at.target, Target::Imported(_))
                || pass
                    .resolve
                    .is_some_and(|resolve| matches!(resolve.target, Target::Imported(_)))
        };
        let first = compiled
            .passes
            .iter()
            .position(on_import)
            .expect("a frame reaches the surface");
        assert_eq!(compiled.split, first, "{context}");
        let ends_in_bloom = !shape.smaa && shape.stages.last() == Some(&B);
        if shape.present == PresentPath::Capture {
            assert_eq!(compiled.split, compiled.passes.len() - 1, "{context}");
        } else if ends_in_bloom {
            assert_eq!(
                compiled.split,
                old_split + 2 * shape.bloom_mips as usize - 1,
                "{context}"
            );
            assert!(matches!(
                compiled.passes[compiled.split].kind,
                PassKind::BloomComposite { .. }
            ));
        } else {
            assert_eq!(compiled.split, old_split, "{context}");
        }

        // A capture reads what the overlay drew on.
        let overlay = compiled
            .passes
            .iter()
            .find(|pass| pass.kind == PassKind::Overlay)
            .expect("an overlay");
        match shape.present {
            PresentPath::Direct => assert_eq!(compiled.copied, None, "{context}"),
            PresentPath::Capture => {
                assert_eq!(compiled.copied, Some(overlay.color.at.target), "{context}");
                assert!(
                    matches!(overlay.color.at.target, Target::Slot(_)),
                    "{context}"
                );
            }
            PresentPath::DirectCapture => {
                assert_eq!(compiled.copied, Some(Target::Imported(0)), "{context}");
                assert_eq!(overlay.color.at.target, Target::Imported(0), "{context}");
            }
        }
    }
}

/// The specs of the pooled textures a frame of `shape` holds.
fn held(shape: &Shape) -> Vec<Spec> {
    declare(shape).compile().expect("compiles").slots
}

#[test]
fn a_frame_holds_only_the_targets_it_uses() {
    let color = Spec {
        size: SIZE,
        format: FORMAT,
        samples: 1,
        mips: 1,
        class: Class::Color,
        twin: None,
    };
    let depth = Spec {
        format: DEPTH_FORMAT,
        class: Class::Depth,
        ..color
    };
    let pyramid = Spec {
        size: bloom_mip_extent_for(SIZE.0, SIZE.1, 1, 0),
        format: BLOOM_PYRAMID_FORMAT,
        mips: 6,
        class: Class::Work,
        ..color
    };

    // An empty stack on the direct path: the scene draws on the surface.
    assert_eq!(held(&shape(&[])), []);
    // Under `cpu_stable` no pass attaches depth, so none is held.
    assert_eq!(held(&shape(&[V])), [color]);
    // Example 01's frame: bloom, its vignette folded, under `gpu_depth`.
    let e01 = Shape {
        depth: true,
        vignette: true,
        ..shape(&[B])
    };
    assert_eq!(held(&e01), [color, depth, pyramid]);
    // A stack of any length alternates between two colour textures.
    assert_eq!(held(&shape(&[V, F, D, V, F])), [color, color]);
    // A capture adds the texture the overlay draws on.
    let capture = Shape {
        present: PresentPath::Capture,
        ..e01.clone()
    };
    assert_eq!(held(&capture), [color, depth, pyramid, color]);
    // Two blooms in one stack share one pyramid.
    assert_eq!(held(&shape(&[B, B])), [color, pyramid, color]);
    // SMAA's working set, the colour versions carrying the twin its edge
    // pass reads through.
    let twin = Spec {
        twin: Some(TextureFormat::Bgra8Unorm),
        ..color
    };
    let smaa = Shape {
        smaa: true,
        ..shape(&[V])
    };
    assert_eq!(
        held(&smaa),
        [
            twin,
            twin,
            Spec {
                format: SMAA_EDGES_FORMAT,
                class: Class::Work,
                ..color
            },
            Spec {
                format: SMAA_BLEND_FORMAT,
                class: Class::Work,
                ..color
            },
        ]
    );
    // MSAA: the multisampled colour, and its resolve only when a stage
    // reads it.
    let msaa = Shape {
        samples: 4,
        ..shape(&[])
    };
    assert_eq!(
        held(&msaa),
        [Spec {
            samples: 4,
            class: Class::Multisampled,
            ..color
        }]
    );
}

#[test]
fn no_frame_of_the_matrix_drops_a_pass() {
    for shape in matrix() {
        let graph = declare(&shape);
        let compiled = graph.compile().expect("compiles");
        let (old, _) = reference(&shape);
        assert_eq!(compiled.passes.len(), old.len(), "{shape:?}");
    }
}

#[test]
fn a_folded_vignette_takes_no_pass_of_the_frame() {
    // Bloom then vignette: 2 * 6 bloom passes and the vignette's, with the
    // scene and the overlay. Where bloom's composite draws the vignette
    // (`D-302`) the frame plans the stack without it: one pass fewer, the
    // composite under a label of its own.
    let timings = |shape: &Shape| -> Vec<String> {
        let compiled = declare(shape).compile().expect("compiles");
        compiled
            .passes
            .iter()
            .map(|pass| pass.timing.clone())
            .collect()
    };
    let apart = timings(&shape(&[B, V]));
    let folded = timings(&Shape {
        vignette: true,
        ..shape(&[B])
    });
    assert_eq!(apart.len(), 15);
    assert_eq!(
        apart[12..],
        ["post0_bloom_composite", "post1_vignette", "text"]
    );
    assert_eq!(folded.len(), 14);
    assert_eq!(folded[12..], ["post0_bloom_composite_vignette", "text"]);
}

#[test]
fn the_shape_follows_the_stack() {
    use tungsten_core::post::{BloomParams, FadeParams, VignetteParams};
    let stack = PostStack(vec![
        PostPass::Fade(FadeParams::default()),
        PostPass::Bloom(BloomParams::default()),
        PostPass::Vignette(VignetteParams::default()),
    ]);
    let mut shape = shape(&[V]);
    shape.set_stages(&stack);
    assert_eq!(shape.stages, [F, B, V]);
    shape.set_stages(&PostStack::default());
    assert!(shape.stages.is_empty());
}

#[test]
fn post_labels_run_out_into_one_shared_label() {
    assert_eq!(post_pass_label(0), "tungsten_post_pass_0");
    assert_eq!(post_pass_label(31), "tungsten_post_pass_31");
    assert_eq!(post_pass_label(32), "tungsten_post_pass_overflow");
}

#[test]
fn bloom_labels_name_the_quarter_base_and_the_folded_vignette() {
    assert_eq!(threshold_label(0, 1), "post0_bloom_threshold");
    assert_eq!(threshold_label(1, 2), "post1_bloom_threshold_quarter");
    assert_eq!(composite_label(0, false), "post0_bloom_composite");
    assert_eq!(composite_label(2, true), "post2_bloom_composite_vignette");
}
