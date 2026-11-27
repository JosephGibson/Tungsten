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

/// The slots that lead a pass: every stage but those joined to the stage
/// before them. The old frame folded one such stage, a vignette into
/// bloom's composite (`D-302`); it drew the stack without it.
fn leads(shape: &Shape) -> Vec<usize> {
    (0..shape.stages.len())
        .filter(|&slot| {
            slot == 0 || !matches!(shape.stages[slot], Stage::Stock { joined: true, .. })
        })
        .collect()
}

/// The names of the stages joined to the stage in `slot`.
fn joined_names(shape: &Shape, slot: usize) -> Vec<&'static str> {
    shape.stages[slot + 1..]
        .iter()
        .map_while(|stage| match stage {
            Stage::Stock {
                name, joined: true, ..
            } => Some(*name),
            _ => None,
        })
        .collect()
}

/// What the stack's output was called in the old frame, and the overlay's
/// target on a blit frame without SMAA: the target of its last pass.
fn stack_output(passes: usize) -> Role {
    if passes == 0 {
        Role::SceneColor
    } else {
        ladder(passes - 1)
    }
}

/// The old frame for `shape`, and the index its acquire split fell on. A
/// run of joined stages is one pass of it, as the folded vignette was, under
/// its first slot's label.
fn reference(shape: &Shape) -> (Vec<RefPass>, usize) {
    let leads = leads(shape);
    let direct = shape.present != PresentPath::Capture;
    let scene_is_last = direct && leads.is_empty() && !shape.smaa;
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

    // Where each stack pass's passes begin: the old split was counted in
    // slots.
    let mut slot_start = Vec::new();
    for (index, &slot) in leads.iter().enumerate() {
        slot_start.push(passes.len());
        let source = if index == 0 {
            Role::SceneColor
        } else {
            ladder(index - 1)
        };
        let writes_swapchain = direct && !shape.smaa && index + 1 == leads.len();
        let target = if writes_swapchain {
            Role::Swapchain
        } else {
            ladder(index)
        };
        let label = if slot < 32 {
            format!("tungsten_post_pass_{slot}")
        } else {
            "tungsten_post_pass_overflow".to_string()
        };
        let joined = joined_names(shape, slot)
            .iter()
            .fold(String::new(), |all, name| all + "_" + name);
        match shape.stages[slot] {
            Stage::Stock { name, .. } => {
                // A stock pass loads offscreen and clears the swapchain.
                passes.push(ref_pass(
                    label,
                    format!("post{slot}_{name}{joined}"),
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
                passes.push(ref_pass(
                    "tungsten_bloom_composite",
                    format!("post{slot}_bloom_composite{joined}"),
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

    let source = stack_output(leads.len());
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

/// Where the window surface is among a frame's imports: behind the
/// stand-in on a direct capture, the frame's output otherwise.
fn surface_of(shape: &Shape) -> Target {
    if shape.present == PresentPath::DirectCapture {
        Target::Imported(1)
    } else {
        Target::Imported(0)
    }
}

/// The kinds that draw one triangle over their whole target with blending
/// off: every kind but the scene, bloom's additive upsample and the overlay.
fn full_screen(kind: PassKind) -> bool {
    !matches!(
        kind,
        PassKind::Scene | PassKind::BloomUp { .. } | PassKind::Overlay
    )
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
        bloom_mips: 6,
        bloom_halvings: 1,
        replaced: ReplacedStages::NONE,
        float: None,
        tail_alone: false,
        light_tiles: false,
    }
}

const fn stock(name: &'static str) -> Stage {
    Stage::Stock {
        name,
        own: true,
        joined: false,
    }
}

/// A stage that draws in the pass of the stage before it.
const fn joined(name: &'static str) -> Stage {
    Stage::Stock {
        name,
        own: true,
        joined: true,
    }
}

const V: Stage = stock("vignette");
const F: Stage = stock("fade");
const D: Stage = stock("dither");
const B: Stage = Stage::Bloom;
const VJ: Stage = joined("vignette");
const FJ: Stage = joined("fade");

/// Every shape the old order's arguments spanned, bloom's, and stacks with
/// stages joined to the stage before them.
fn matrix() -> Vec<Shape> {
    let seventeen: Vec<Stage> = (0..17)
        .map(|slot| if slot % 3 == 0 { F } else { V })
        .collect();
    let stacks: Vec<Vec<Stage>> = vec![
        vec![],
        vec![V],
        vec![V, F],
        vec![V, F, D],
        seventeen,
        vec![B],
        vec![B, VJ],
        vec![B, V],
        vec![V, B],
        vec![V, B, VJ],
        vec![F, B, V],
        vec![B, B],
        vec![B, F, B, VJ],
        vec![V, FJ],
        vec![D, V, FJ],
        vec![B, V, FJ],
        vec![D, FJ, VJ, B, FJ],
    ];
    let mut shapes = Vec::new();
    for stages in &stacks {
        for samples in [1, 4] {
            for depth in [false, true] {
                for smaa in [false, true] {
                    for present in [
                        PresentPath::Direct,
                        PresentPath::Capture,
                        PresentPath::DirectCapture,
                    ] {
                        for (bloom_mips, bloom_halvings) in [(6, 1), (5, 2), (1, 1)] {
                            shapes.push(Shape {
                                samples,
                                depth,
                                smaa,
                                present,
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
    assert_eq!(shapes.len(), 17 * 72);
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

            // The ops. The old frame cleared the scene, SMAA's three
            // targets and the swapchain's first write, and loaded the rest;
            // a capture's blit loaded the surface, which made wgpu clear it
            // in a pass of its own. Now an imported texture's first write
            // loads nothing when a full-screen pass makes it, and SMAA's
            // stages, which write every pixel, load (`D-306`).
            let on_import = matches!(new.color.at.target, Target::Imported(_));
            let on_surface = new.color.at.target == surface_of(shape);
            let first_write = producer(&compiled, index, new.color.at).is_none();
            let smaa = matches!(
                new.kind,
                PassKind::SmaaEdges | PassKind::SmaaWeights | PassKind::SmaaNeighborhood
            );
            // A direct capture's stand-in is the renderer's own new
            // texture: its first whole write clears it, as before.
            let expected = if on_surface && first_write && full_screen(new.kind) {
                Load::Nothing
            } else if on_import && first_write && full_screen(new.kind) {
                Load::Clear(wgpu::Color::TRANSPARENT)
            } else if smaa || !old_pass.clears {
                Load::Keep
            } else {
                Load::Clear(wgpu::Color::TRANSPARENT)
            };
            assert_eq!(new.color.load, expected, "{at}");
            let clears = matches!(new.color.load, Load::Clear(_));
            if new.kind == PassKind::Blit {
                assert_ne!(shape.present, PresentPath::Direct, "{at}");
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
        let ends_in_bloom = !shape.smaa
            && leads(shape)
                .last()
                .is_some_and(|&slot| shape.stages[slot] == B);
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

/// The passes of `compiled` that load nothing, by index.
fn dont_cares(compiled: &Compiled<PassKind>) -> Vec<usize> {
    (0..compiled.passes.len())
        .filter(|&index| compiled.passes[index].color.load == Load::Nothing)
        .collect()
}

#[test]
fn dont_care_loads_only_the_outputs_first_whole_write() {
    for shape in matrix() {
        let compiled = declare(&shape).compile().expect("compiles");
        // The surface's first writer, as colour or resolve.
        let surface = surface_of(&shape);
        let first = compiled.passes.iter().position(|pass| {
            pass.color.at.target == surface
                || pass
                    .resolve
                    .is_some_and(|resolve| resolve.target == surface)
        });
        // It loads nothing when it is a full-screen pass that attaches the
        // surface as its colour: never the scene, which clears or resolves.
        let expected: Vec<usize> = first
            .into_iter()
            .filter(|&index| {
                let pass = &compiled.passes[index];
                full_screen(pass.kind) && pass.color.at.target == surface
            })
            .collect();
        assert_eq!(dont_cares(&compiled), expected, "{shape:?}");
        for index in dont_cares(&compiled) {
            let pass = &compiled.passes[index];
            assert!(
                matches!(
                    pass.kind,
                    PassKind::Post { .. }
                        | PassKind::BloomComposite { .. }
                        | PassKind::SmaaNeighborhood
                        | PassKind::Blit
                ),
                "{:?} of {shape:?}",
                pass.kind
            );
            assert!(pass.color.store, "{shape:?}");
        }
        // A frame whose scene writes the surface has none; every other
        // frame has one, a capture's on its blit. No pass on a direct
        // capture's stand-in or on a pooled texture has one.
        let scene_ends = shape.stages.is_empty() && !shape.smaa;
        let count = match shape.present {
            PresentPath::Direct => usize::from(!scene_ends),
            PresentPath::Capture | PresentPath::DirectCapture => 1,
        };
        assert_eq!(dont_cares(&compiled).len(), count, "{shape:?}");
        for pass in &compiled.passes {
            if pass.color.at.target != surface {
                assert_ne!(pass.color.load, Load::Nothing, "{shape:?}");
            }
        }
    }
}

#[test]
fn a_replaced_shaders_pass_clears() {
    let clear = Load::Clear(wgpu::Color::TRANSPARENT);
    let load_of = |shape: &Shape, kind: fn(&PassKind) -> bool| -> Vec<Load> {
        let compiled = declare(shape).compile().expect("compiles");
        compiled
            .passes
            .iter()
            .filter(|pass| kind(&pass.kind))
            .map(|pass| pass.color.load)
            .collect()
    };
    let replaced = Stage::Stock {
        name: "vignette",
        own: false,
        joined: false,
    };

    // A stock pass: on the output and offscreen.
    let post = |kind: &PassKind| matches!(kind, PassKind::Post { slot: 0, .. });
    assert_eq!(load_of(&shape(&[V]), post), [Load::Nothing]);
    assert_eq!(load_of(&shape(&[replaced]), post), [clear]);
    assert_eq!(load_of(&shape(&[V, F]), post), [Load::Keep]);
    assert_eq!(load_of(&shape(&[replaced, F]), post), [clear]);
    assert_ne!(shape(&[V]), shape(&[replaced]), "the shape moves with it");
    // A pass that draws a replaced file's stage beside its own clears too:
    // the rule joins no such stage, and the declaration does not rely on it.
    let joined_replaced = Stage::Stock {
        name: "fade",
        own: false,
        joined: true,
    };
    assert_eq!(load_of(&shape(&[V, FJ]), post), [Load::Nothing]);
    assert_eq!(load_of(&shape(&[V, joined_replaced]), post), [clear]);
    let composite = |kind: &PassKind| matches!(kind, PassKind::BloomComposite { .. });
    assert_eq!(load_of(&shape(&[B, VJ]), composite), [Load::Nothing]);
    assert_eq!(load_of(&shape(&[B, joined_replaced]), composite), [clear]);

    // Each of bloom's and SMAA's stages, on the output where it can be and
    // offscreen (a capture frame draws every stage offscreen).
    let kind_of = |stage: EngineStage| -> fn(&PassKind) -> bool {
        match stage {
            EngineStage::BloomThreshold => |kind| matches!(kind, PassKind::BloomThreshold { .. }),
            EngineStage::BloomDownsample => |kind| matches!(kind, PassKind::BloomDown { .. }),
            EngineStage::BloomComposite => |kind| matches!(kind, PassKind::BloomComposite { .. }),
            EngineStage::SmaaEdges => |kind| matches!(kind, PassKind::SmaaEdges),
            EngineStage::SmaaWeights => |kind| matches!(kind, PassKind::SmaaWeights),
            EngineStage::SmaaNeighborhood => |kind| matches!(kind, PassKind::SmaaNeighborhood),
        }
    };
    let mut checked = 0;
    for stage in EngineStage::ALL {
        let kind = kind_of(stage);
        for present in [PresentPath::Direct, PresentPath::Capture] {
            // Bloom ends the stack and SMAA follows it, so the composite
            // and the neighborhood pass each reach the output in one of
            // the two frames below.
            for smaa in [false, true] {
                let own = Shape {
                    smaa,
                    present,
                    ..shape(&[B])
                };
                let other = Shape {
                    replaced: ReplacedStages::NONE.with(stage, true),
                    ..own.clone()
                };
                let before = load_of(&own, kind);
                let after = load_of(&other, kind);
                assert_eq!(before.len(), after.len(), "{stage:?}");
                if before.is_empty() {
                    continue;
                }
                checked += 1;
                assert!(
                    before.iter().all(|load| *load != clear),
                    "{stage:?}: on its own text the stage is whole"
                );
                assert!(
                    after.iter().all(|load| *load == clear),
                    "{stage:?}: a replaced file's pass clears"
                );
                assert_ne!(own, other, "{stage:?}: the shape moves with it");
                // Marked compiled-in again, it is the first frame's.
                let back = Shape {
                    replaced: other.replaced.with(stage, false),
                    ..other.clone()
                };
                assert_eq!(back, own, "{stage:?}");
            }
        }
    }
    // Bloom's three stages in all four frames, SMAA's three in two.
    assert_eq!(checked, 3 * 4 + 3 * 2);
    // The upsample draws over what the downsamples left, whatever is
    // replaced.
    let all = EngineStage::ALL
        .into_iter()
        .fold(ReplacedStages::NONE, |stages, stage| {
            stages.with(stage, true)
        });
    assert!(EngineStage::ALL.into_iter().all(|stage| all.holds(stage)));
    let none = Shape {
        replaced: all,
        ..shape(&[B])
    };
    let ups = |kind: &PassKind| matches!(kind, PassKind::BloomUp { .. });
    assert_eq!(load_of(&none, ups), [Load::Keep; 5]);
}

#[test]
fn a_committed_text_marks_its_own_pass_and_no_other() {
    use tungsten_core::post::{DitherParams, FadeParams, VignetteParams};
    let own_text = |name: &str| fullscreen_source(name).expect("a full-screen shader").1;
    let stock = [
        ("vignette", PostPass::Vignette(VignetteParams::default())),
        ("fade", PostPass::Fade(FadeParams::default())),
        ("dither", PostPass::Dither(DitherParams::default())),
        ("pixelate", PostPass::Pixelate(4.0)),
    ];

    // Each stage of bloom and SMAA is drawn by a shader of the two tables,
    // and a text other than the compiled-in one marks that stage alone.
    let tables: Vec<&str> = BLOOM_SHADERS
        .iter()
        .chain(&SMAA_SHADERS)
        .map(|(name, _)| *name)
        .collect();
    for stage in EngineStage::ALL {
        assert!(tables.contains(&stage.shader()), "{stage:?}");
        let mut replaced = Replaced::default();
        replaced.commit(stage.shader(), "// a game's own text");
        assert_eq!(
            replaced.stages(),
            ReplacedStages::NONE.with(stage, true),
            "{stage:?}"
        );
        for (name, pass) in &stock {
            assert!(replaced.own(pass), "{stage:?} leaves {name} its own");
        }
        // The compiled-in bytes are the engine's own again.
        replaced.commit(stage.shader(), own_text(stage.shader()));
        assert_eq!(replaced, Replaced::default(), "{stage:?}");
    }
    // The six stages are six shaders; the seventh of the two tables is
    // bloom's upsample, which draws over what its level holds either way.
    let mut shaders: Vec<&str> = EngineStage::ALL
        .iter()
        .map(|stage| stage.shader())
        .collect();
    shaders.sort_unstable();
    shaders.dedup();
    assert_eq!(shaders.len(), 6);
    let mut replaced = Replaced::default();
    replaced.commit("bloom_upsample", "// a game's own text");
    assert_eq!(replaced.stages(), ReplacedStages::NONE);

    // A stock effect's text marks the passes of its kind and no stage.
    for (name, pass) in &stock {
        let mut replaced = Replaced::default();
        replaced.commit(name, "// a game's own text");
        assert!(!replaced.own(pass), "{name}");
        assert_eq!(replaced.stages(), ReplacedStages::NONE, "{name}");
        for (other, other_pass) in &stock {
            assert_eq!(
                replaced.own(other_pass),
                other != name,
                "{name} and {other}"
            );
        }
        replaced.commit(name, own_text(name));
        assert!(replaced.own(pass), "{name}");
    }
    // A shader that draws no full-screen pass records nothing, and a bloom
    // is never a stock pass.
    let mut replaced = Replaced::default();
    replaced.commit("sprite", "// a game's own text");
    replaced.commit("glow", "// a game's own text");
    assert_eq!(replaced, Replaced::default());
    assert!(fullscreen_source("sprite").is_none());
    assert_eq!(
        STOCK_SHADERS.len() + BLOOM_SHADERS.len() + SMAA_SHADERS.len(),
        24
    );
    for (name, source) in STOCK_SHADERS
        .iter()
        .chain(&BLOOM_SHADERS)
        .chain(&SMAA_SHADERS)
    {
        assert_eq!(fullscreen_source(name), Some((*name, *source)), "{name}");
    }
}

#[test]
fn the_record_holds_a_replaced_shader_until_its_own_text_is_back() {
    let mut replaced = Replaced::default();
    assert!(!replaced.holds("vignette"));
    replaced.record("vignette", false);
    replaced.record("smaa_edge", false);
    assert!(replaced.holds("vignette") && replaced.holds("smaa_edge"));
    assert!(!replaced.holds("fade"));
    // Another replacement of the same file is still one.
    replaced.record("vignette", false);
    replaced.record("vignette", true);
    assert!(
        !replaced.holds("vignette"),
        "the compiled-in bytes are back"
    );
    assert!(replaced.holds("smaa_edge"));
    // A text that was never replaced records nothing.
    replaced.record("fade", true);
    replaced.record("smaa_edge", true);
    assert_eq!(replaced, Replaced::default());
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
    // Example 01's frame: bloom and the vignette its composite draws, under
    // `gpu_depth`.
    let e01 = Shape {
        depth: true,
        ..shape(&[B, VJ])
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

/// The format of the texture `at` names: a pooled texture's, or the
/// output's for an imported one.
fn format_at(shape: &Shape, compiled: &Compiled<PassKind>, at: At) -> TextureFormat {
    match at.target {
        Target::Slot(slot) => compiled.slots[usize::from(slot)].format,
        Target::Imported(_) => shape.format,
    }
}

#[test]
fn a_float_scene_reaches_the_8_bit_frame_through_the_tail() {
    // `render.hdr` (`D-309`). A format the pyramid never has, so a float
    // version is told from a bloom level.
    const FLOAT: TextureFormat = TextureFormat::Rg11b10Ufloat;
    let mut frames = 0;
    for base in matrix() {
        // Off: the flag for the tail's own pass means nothing.
        let off = Shape {
            tail_alone: true,
            ..base.clone()
        };
        assert_eq!(
            declare(&off).compile().expect("compiles"),
            declare(&base).compile().expect("compiles"),
            "{base:?}"
        );
        for tail_alone in [true, false] {
            // The stack's last pass ends in the tail only where there is one.
            if !tail_alone && base.stages.is_empty() {
                continue;
            }
            let shape = Shape {
                float: Some(FLOAT),
                tail_alone,
                ..base.clone()
            };
            let compiled = declare(&shape)
                .compile()
                .unwrap_or_else(|error| panic!("{shape:?}: {error:?}"));
            frames += 1;
            let format = |at: At| format_at(&shape, &compiled, at);

            // One pass ends in the tail: its own, or the stack's last.
            let tails: Vec<usize> = compiled
                .passes
                .iter()
                .enumerate()
                .filter(|(_, pass)| {
                    matches!(
                        pass.kind,
                        PassKind::Tail { .. }
                            | PassKind::Post { tail: true, .. }
                            | PassKind::BloomComposite { tail: true, .. }
                    )
                })
                .map(|(index, _)| index)
                .collect();
            assert_eq!(tails.len(), 1, "{shape:?}");
            let tail = tails[0];
            assert_eq!(
                matches!(compiled.passes[tail].kind, PassKind::Tail { .. }),
                tail_alone,
                "{shape:?}"
            );
            if let PassKind::Tail { slot } = compiled.passes[tail].kind {
                assert_eq!(slot, shape.stages.len());
                assert_eq!(compiled.passes[tail].timing, "tail");
            }

            for (index, pass) in compiled.passes.iter().enumerate() {
                let at = format!("pass {index} ({}) of {shape:?}", pass.timing);
                let target = format(pass.color.at);
                let float_reads = pass.reads.iter().filter(|read| format(**read) == FLOAT);
                match pass.kind {
                    PassKind::LightTiles => unreachable!("a compute pass attaches no texture"),
                    // The scene draws in the float format, resolve and all.
                    PassKind::Scene => {
                        assert_eq!(target, FLOAT, "{at}");
                        assert!(pass.resolve.is_none_or(|to| format(to) == FLOAT), "{at}");
                        assert_eq!(index, 0, "{at}");
                    }
                    // The stack's passes draw float versions, up to the
                    // one that ends in the tail, which writes the 8-bit
                    // frame from a float colour.
                    PassKind::Post { .. }
                    | PassKind::BloomComposite { .. }
                    | PassKind::Tail { .. } => {
                        assert!(index <= tail, "nothing of the stack follows the tail: {at}");
                        assert_eq!(target, if index == tail { FORMAT } else { FLOAT }, "{at}");
                        assert_eq!(float_reads.count(), 1, "{at}");
                    }
                    // The pyramid is its own; its threshold reads a float
                    // version.
                    PassKind::BloomThreshold { .. } => {
                        assert_eq!(target, BLOOM_PYRAMID_FORMAT, "{at}");
                        assert_eq!(float_reads.count(), 1, "{at}");
                    }
                    PassKind::BloomDown { .. } | PassKind::BloomUp { .. } => {
                        assert_eq!(target, BLOOM_PYRAMID_FORMAT, "{at}");
                    }
                    // SMAA, the overlay and a capture's blit never touch a
                    // float target: they follow the tail and read and draw
                    // 8-bit textures.
                    PassKind::SmaaEdges
                    | PassKind::SmaaWeights
                    | PassKind::SmaaNeighborhood
                    | PassKind::Overlay
                    | PassKind::Blit => {
                        assert!(index > tail, "{at}");
                        assert_ne!(target, FLOAT, "{at}");
                        assert_eq!(float_reads.count(), 0, "{at}");
                    }
                }
            }
            // The tail's own pass writes every pixel, so it loads nothing
            // on the surface; where it is the frame's only full-screen
            // pass the scene alone comes before it.
            if tail_alone && shape.stages.is_empty() {
                assert_eq!(tail, 1, "{shape:?}");
            }
            // What follows the tail is declared as `off` declares it: the
            // 8-bit texture SMAA reads, with its view in the other
            // encoding, and the one a capture copies.
            let base_compiled = declare(&base).compile().expect("compiles");
            let spec = |frame: &Compiled<PassKind>, at: Target| match at {
                Target::Slot(slot) => Some(frame.slots[slot as usize]),
                Target::Imported(_) => None,
            };
            let edges = |frame: &Compiled<PassKind>| {
                frame
                    .passes
                    .iter()
                    .find(|pass| pass.kind == PassKind::SmaaEdges)
                    .map(|pass| spec(frame, pass.reads[0].target))
            };
            assert_eq!(edges(&compiled), edges(&base_compiled), "{shape:?}");
            let copied =
                |frame: &Compiled<PassKind>| frame.copied.map(|target| spec(frame, target));
            assert_eq!(copied(&compiled), copied(&base_compiled), "{shape:?}");
        }
    }
    // Every shape with its tail alone, and every shape with a stack with
    // the tail in its last pass.
    assert_eq!(frames, 17 * 72 + 16 * 72);

    // The frame of an empty stack on the direct path: the scene in a float
    // texture, then the tail onto the surface, which it need not load.
    let direct = Shape {
        float: Some(FLOAT),
        tail_alone: true,
        ..shape(&[])
    };
    let compiled = declare(&direct).compile().expect("compiles");
    let kinds: Vec<PassKind> = compiled.passes.iter().map(|pass| pass.kind).collect();
    assert_eq!(
        kinds,
        [
            PassKind::Scene,
            PassKind::Tail { slot: 0 },
            PassKind::Overlay
        ]
    );
    assert_eq!(compiled.passes[1].color.load, Load::Nothing);
    assert_eq!(compiled.passes[1].color.at.target, Target::Imported(0));
    assert_eq!(compiled.slots.len(), 1, "the float scene alone");
}

#[test]
fn a_games_effect_is_a_pass_that_clears_under_the_label_custom() {
    // The record never takes a game's effect for the engine's own text
    // (`D-308`): its module may discard, so its pass clears, on the output
    // and offscreen, and a pass that draws it beside a stock stage does.
    let custom = PostPass::Custom(tungsten_core::post::CustomPost::new(
        tungsten_core::assets::PostEffectId(3),
        tungsten_core::assets::MaterialUniformDefaults::default(),
    ));
    let record = Replaced::default();
    assert!(!record.own(&custom));
    assert!(record.own(&PostPass::Vignette(
        tungsten_core::post::VignetteParams::default()
    )));
    let mut declared = shape(&[]);
    declared.set_stages(
        &PostStack(vec![
            custom,
            PostPass::Fade(tungsten_core::post::FadeParams::default()),
        ]),
        |pass| record.own(pass),
    );
    assert_eq!(
        declared.stages,
        [
            Stage::Stock {
                name: "custom",
                own: false,
                joined: false
            },
            F
        ]
    );
    let passes = |shape: &Shape| -> Vec<(String, Load)> {
        let compiled = declare(shape).compile().expect("compiles");
        compiled
            .passes
            .iter()
            .filter(|pass| matches!(pass.kind, PassKind::Post { .. }))
            .map(|pass| (pass.timing.clone(), pass.color.load))
            .collect()
    };
    let clear = Load::Clear(wgpu::Color::TRANSPARENT);
    assert_eq!(
        passes(&declared),
        [
            ("post0_custom".to_string(), clear),
            ("post1_fade".to_string(), Load::Nothing)
        ]
    );
    declared.join(0..2);
    assert_eq!(
        passes(&declared),
        [("post0_custom_fade".to_string(), clear)]
    );
}

#[test]
fn a_run_of_joined_stages_is_one_pass_under_its_stages_names() {
    let passes = |shape: &Shape| -> Vec<(PassKind, String)> {
        let compiled = declare(shape).compile().expect("compiles");
        compiled
            .passes
            .iter()
            .map(|pass| (pass.kind, pass.timing.clone()))
            .collect()
    };
    // Bloom then vignette: 2 * 6 bloom passes and the vignette's, with the
    // scene and the overlay. Where the composite draws the vignette the
    // frame has one pass fewer, the composite under the label `D-302` gave
    // it.
    let apart = passes(&shape(&[B, V]));
    assert_eq!(apart.len(), 15);
    assert_eq!(
        apart[12..],
        [
            (
                PassKind::BloomComposite {
                    slot: 0,
                    stages: 1,
                    tail: false
                },
                "post0_bloom_composite".to_string()
            ),
            (
                PassKind::Post {
                    slot: 1,
                    stages: 1,
                    tail: false
                },
                "post1_vignette".to_string()
            ),
            (PassKind::Overlay, "text".to_string()),
        ]
    );
    let fused = passes(&shape(&[B, VJ]));
    assert_eq!(fused.len(), 14);
    assert_eq!(
        fused[12..],
        [
            (
                PassKind::BloomComposite {
                    slot: 0,
                    stages: 2,
                    tail: false
                },
                "post0_bloom_composite_vignette".to_string()
            ),
            (PassKind::Overlay, "text".to_string()),
        ]
    );
    // A stock pair under a screen transition, behind example 01's bloom:
    // the pass keeps its first stage's slot.
    let transition = passes(&shape(&[B, V, FJ]));
    assert_eq!(transition.len(), 15);
    assert_eq!(
        transition[12..14],
        [
            (
                PassKind::BloomComposite {
                    slot: 0,
                    stages: 1,
                    tail: false
                },
                "post0_bloom_composite".to_string()
            ),
            (
                PassKind::Post {
                    slot: 1,
                    stages: 2,
                    tail: false
                },
                "post1_vignette_fade".to_string()
            ),
        ]
    );
    // A stage after a run leads the next pass, under its own slot.
    let after = passes(&shape(&[D, FJ, VJ, B, FJ]));
    assert_eq!(
        after[1],
        (
            PassKind::Post {
                slot: 0,
                stages: 3,
                tail: false
            },
            "post0_dither_fade_vignette".to_string()
        )
    );
    assert_eq!(after.last().map(|(kind, _)| *kind), Some(PassKind::Overlay));
    assert_eq!(
        after[after.len() - 2],
        (
            PassKind::BloomComposite {
                slot: 3,
                stages: 2,
                tail: false
            },
            "post3_bloom_composite_fade".to_string()
        )
    );
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
    shape.set_stages(&stack, |_| true);
    assert_eq!(shape.stages, [F, B, V]);
    // A stock pass whose shader a file replaced says so; bloom's stages are
    // the shape's own record.
    shape.set_stages(&stack, |pass| !matches!(pass, PostPass::Vignette(_)));
    assert_eq!(
        shape.stages,
        [
            F,
            B,
            Stage::Stock {
                name: "vignette",
                own: false,
                joined: false,
            }
        ]
    );
    // The composer's runs join the stages after their first.
    shape.set_stages(&stack, |_| true);
    assert_eq!((shape.run_len(0), shape.run_len(1)), (1, 1));
    shape.join(1..3);
    assert_eq!(shape.stages, [F, B, VJ]);
    assert_eq!((shape.run_len(0), shape.run_len(1)), (1, 2));
    // A bloom leads a pass whatever a run says, and a refill starts over.
    shape.join(0..2);
    assert_eq!(shape.stages, [F, B, VJ]);
    shape.set_stages(&stack, |_| true);
    assert_eq!(shape.stages, [F, B, V]);
    shape.set_stages(&PostStack::default(), |_| true);
    assert!(shape.stages.is_empty());
}

#[test]
fn post_labels_run_out_into_one_shared_label() {
    assert_eq!(post_pass_label(0), "tungsten_post_pass_0");
    assert_eq!(post_pass_label(31), "tungsten_post_pass_31");
    assert_eq!(post_pass_label(32), "tungsten_post_pass_overflow");
}

#[test]
fn bloom_labels_name_the_quarter_base_and_the_joined_stages() {
    assert_eq!(threshold_label(0, 1), "post0_bloom_threshold");
    assert_eq!(threshold_label(1, 2), "post1_bloom_threshold_quarter");
    assert_eq!(composite_label(0, &[]), "post0_bloom_composite");
    assert_eq!(
        composite_label(2, &["vignette"]),
        "post2_bloom_composite_vignette"
    );
    assert_eq!(
        composite_label(1, &["fog", "fade"]),
        "post1_bloom_composite_fog_fade"
    );
}

/// The lights' tiles are binned only in a frame that walks them (`D-310`):
/// the compute pass runs ahead of the scene, under the timing label
/// `light_tiles`, and any other frame drops it and compiles the passes it
/// compiled before the tiles.
#[test]
fn a_frame_that_walks_tiles_keeps_the_tile_pass_and_another_drops_it() {
    for mut frame in matrix() {
        frame.light_tiles = false;
        let unlit = declare(&frame).compile().expect("compiles");
        assert!(
            unlit.computes.is_empty(),
            "{frame:?}: a frame that walks no tiles bins no lights"
        );
        frame.light_tiles = true;
        let lit = declare(&frame).compile().expect("compiles");
        assert_eq!(lit.computes.len(), 1, "{frame:?}");
        let bins = &lit.computes[0];
        assert_eq!(bins.kind, PassKind::LightTiles, "{frame:?}");
        assert_eq!(bins.timing, "light_tiles", "{frame:?}");
        assert_eq!(bins.before, 0, "{frame:?}: ahead of the scene");
        assert_eq!(lit.passes[0].kind, PassKind::Scene, "{frame:?}");
        // The tiles are a buffer: no pass's target, op or place moves, and
        // the acquire splits where it did.
        assert_eq!(lit.passes, unlit.passes, "{frame:?}");
        assert_eq!(lit.slots, unlit.slots, "{frame:?}");
        assert_eq!(lit.split, unlit.split, "{frame:?}");
    }
}
