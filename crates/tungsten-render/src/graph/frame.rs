//! The engine's frame, declared: `Scene → PostStack → [SMAA tail] → Overlay`
//! and its end on the surface (`D-087`, `D-147`, `D-161`).
//!
//! [`declare`] is the one place that knows the frame's shape. Which stage
//! writes the surface, where the overlay draws, what a capture reads and
//! where the surface acquire splits the frame all follow from the
//! declaration when it compiles; none is counted here.

use tungsten_core::post::{PostPass, PostStack};
use wgpu::TextureFormat;

use super::{Class, Graph, Pass, Spec, Tex, Write};
use crate::post::bloom::{
    BLOOM_COMPOSITE_SHADER_NAME, BLOOM_DOWNSAMPLE_SHADER_NAME, BLOOM_SHADERS,
    BLOOM_THRESHOLD_SHADER_NAME,
};
use crate::post::smaa::{
    SMAA_BLEND_WEIGHTS_SHADER_NAME, SMAA_EDGE_SHADER_NAME, SMAA_NEIGHBORHOOD_BLEND_SHADER_NAME,
    SMAA_SHADERS,
};
use crate::post::{STOCK_SHADERS, stock_shader_name};
use crate::targets::{
    BLOOM_PYRAMID_FORMAT, DEPTH_FORMAT, SMAA_BLEND_FORMAT, SMAA_EDGES_FORMAT, bloom_mip_extent_for,
    non_srgb_twin,
};

/// What records a pass of the frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PassKind {
    /// Quads, sprites, mesh particles and debug primitives.
    Scene,
    /// The stock effect in post-stack slot `slot`.
    Post {
        slot: usize,
    },
    /// Bloom's bright pass into the pyramid's first level.
    BloomThreshold {
        slot: usize,
    },
    BloomDown {
        slot: usize,
        level: u32,
    },
    BloomUp {
        slot: usize,
        level: u32,
    },
    /// Bloom's composite; `vignette` when it also draws the stack's
    /// trailing vignette (`D-302`).
    BloomComposite {
        slot: usize,
        vignette: bool,
    },
    SmaaEdges,
    SmaaWeights,
    SmaaNeighborhood,
    /// World labels, the UI paint list, then the text sections.
    Overlay,
    /// The copy of a composed frame onto the surface: capture frames only.
    Blit,
}

/// How a frame reaches the surface (`D-087`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PresentPath {
    /// The last full-screen stage writes the surface and the overlay draws
    /// there.
    Direct,
    /// A capture reads the composed frame, so every stage draws offscreen
    /// and a blit copies the result onto the surface.
    Capture,
    /// A capture of the direct path: the frame draws into a stand-in for
    /// the surface, which the capture reads and a blit copies.
    DirectCapture,
}

/// One slot of the post stack, as far as the frame's shape goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Stage {
    /// A stock effect, by its kind's name; `own` while its shader is the
    /// engine's compiled-in text.
    Stock {
        name: &'static str,
        own: bool,
    },
    Bloom,
}

/// One of bloom's or SMAA's full-screen stages. Bloom's upsample is not one:
/// it draws over what the downsamples left, whatever its shader does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EngineStage {
    BloomThreshold,
    BloomDownsample,
    BloomComposite,
    SmaaEdges,
    SmaaWeights,
    SmaaNeighborhood,
}

impl EngineStage {
    pub(crate) const ALL: [Self; 6] = [
        Self::BloomThreshold,
        Self::BloomDownsample,
        Self::BloomComposite,
        Self::SmaaEdges,
        Self::SmaaWeights,
        Self::SmaaNeighborhood,
    ];

    /// The shader that draws the stage, by its manifest name.
    pub(crate) fn shader(self) -> &'static str {
        match self {
            Self::BloomThreshold => BLOOM_THRESHOLD_SHADER_NAME,
            Self::BloomDownsample => BLOOM_DOWNSAMPLE_SHADER_NAME,
            Self::BloomComposite => BLOOM_COMPOSITE_SHADER_NAME,
            Self::SmaaEdges => SMAA_EDGE_SHADER_NAME,
            Self::SmaaWeights => SMAA_BLEND_WEIGHTS_SHADER_NAME,
            Self::SmaaNeighborhood => SMAA_NEIGHBORHOOD_BLEND_SHADER_NAME,
        }
    }
}

/// The [`EngineStage`]s drawn by a text other than the engine's compiled-in
/// one. A manifest's file or a reload that replaces a stage's shader may
/// discard, so its pass is no longer known to write every pixel (`D-306`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct ReplacedStages(u8);

impl ReplacedStages {
    /// Every stage on its compiled-in text: a renderer whose manifest
    /// replaces none.
    pub(crate) const NONE: Self = Self(0);

    /// The set with `stage` in it or out of it.
    #[must_use]
    pub(crate) fn with(self, stage: EngineStage, replaced: bool) -> Self {
        let bit = 1 << stage as u8;
        Self(if replaced {
            self.0 | bit
        } else {
            self.0 & !bit
        })
    }

    pub(crate) fn holds(self, stage: EngineStage) -> bool {
        self.0 & (1 << stage as u8) != 0
    }
}

/// The engine's full-screen shaders that draw with a text other than the
/// compiled-in one, by shader name: a manifest's file or a reload replaced
/// it. The renderer records each text it commits, and the frame's shape is
/// read from the record, so a replacement, or a file put back, compiles the
/// declaration again.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Replaced {
    names: Vec<&'static str>,
}

impl Replaced {
    /// Records the text shader `name` draws with from now on: `own` when it
    /// is the compiled-in text.
    pub(crate) fn record(&mut self, name: &'static str, own: bool) {
        self.names.retain(|held| *held != name);
        if !own {
            self.names.push(name);
        }
    }

    /// Whether shader `name` draws with a replacement.
    pub(crate) fn holds(&self, name: &str) -> bool {
        self.names.contains(&name)
    }

    /// Records that shader `name` draws with `text` from now on, when it is
    /// one of the engine's full-screen shaders: replaced unless `text` is
    /// the compiled-in bytes.
    pub(crate) fn commit(&mut self, name: &str, text: &str) {
        if let Some((key, source)) = fullscreen_source(name) {
            self.record(key, text == source);
        }
    }

    /// Whether the stock effect that draws `pass` is on the engine's own
    /// text. Bloom's stages are [`Self::stages`]'.
    pub(crate) fn own(&self, pass: &PostPass) -> bool {
        !stock_shader_name(pass).is_some_and(|name| self.holds(name))
    }

    /// The stages of bloom and SMAA the record holds.
    pub(crate) fn stages(&self) -> ReplacedStages {
        EngineStage::ALL
            .into_iter()
            .fold(ReplacedStages::NONE, |stages, stage| {
                stages.with(stage, self.holds(stage.shader()))
            })
    }
}

/// The compiled-in text of the engine's full-screen shader `name`, under the
/// name's static: a stock post effect's, a bloom stage's or an SMAA stage's.
pub(crate) fn fullscreen_source(name: &str) -> Option<(&'static str, &'static str)> {
    STOCK_SHADERS
        .iter()
        .chain(&BLOOM_SHADERS)
        .chain(&SMAA_SHADERS)
        .find(|(held, _)| *held == name)
        .copied()
}

/// Everything the frame's declaration depends on. Two frames of one shape
/// compile to the same passes over the same textures.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Shape {
    /// The surface's size.
    pub output: (u32, u32),
    /// The surface's format.
    pub format: TextureFormat,
    pub samples: u32,
    /// Whether the scene pass attaches depth (`gpu_depth`).
    pub depth: bool,
    pub smaa: bool,
    pub present: PresentPath,
    pub stages: Vec<Stage>,
    /// Whether the stack's last pass, a bloom, draws the trailing vignette
    /// in its composite (`D-302`).
    pub vignette: bool,
    /// The pyramid's level count and how many times the output is halved
    /// for its first level (`D-301`).
    pub bloom_mips: u32,
    pub bloom_halvings: u32,
    /// The stages of bloom and SMAA a file replaced.
    pub replaced: ReplacedStages,
}

impl Shape {
    /// Refills `stages` from the stack a frame draws, keeping the
    /// allocation. `own` says whether a stock pass's shader is the engine's
    /// compiled-in text.
    pub(crate) fn set_stages(&mut self, stack: &PostStack, own: impl Fn(&PostPass) -> bool) {
        self.stages.clear();
        self.stages.extend(stack.0.iter().map(|pass| match pass {
            PostPass::Bloom(_) => Stage::Bloom,
            other => Stage::Stock {
                name: other.kind_name(),
                own: own(other),
            },
        }));
    }
}

/// How a full-screen pass writes its target. On the engine's own text it is
/// known to write every pixel: one triangle over the whole target, no blend,
/// every channel, a fragment that never discards (the shader coverage test
/// holds the files to it). A replaced file may discard, so its pass clears
/// first, as every pass did before the graph knew (`D-306`).
fn full_screen(own: bool) -> Write {
    if own {
        Write::Whole
    } else {
        Write::Clear(wgpu::Color::TRANSPARENT)
    }
}

/// The scene pass's clear in the declaration. The renderer clears with its
/// own clear colour, which a game may change between frames of one shape.
pub(crate) const SCENE_CLEAR: wgpu::Color = wgpu::Color::TRANSPARENT;

/// Labels for the post stack's passes: static, so a pass keeps a
/// `&'static str` label without an allocation a frame.
const POST_PASS_LABELS: [&str; 32] = [
    "tungsten_post_pass_0",
    "tungsten_post_pass_1",
    "tungsten_post_pass_2",
    "tungsten_post_pass_3",
    "tungsten_post_pass_4",
    "tungsten_post_pass_5",
    "tungsten_post_pass_6",
    "tungsten_post_pass_7",
    "tungsten_post_pass_8",
    "tungsten_post_pass_9",
    "tungsten_post_pass_10",
    "tungsten_post_pass_11",
    "tungsten_post_pass_12",
    "tungsten_post_pass_13",
    "tungsten_post_pass_14",
    "tungsten_post_pass_15",
    "tungsten_post_pass_16",
    "tungsten_post_pass_17",
    "tungsten_post_pass_18",
    "tungsten_post_pass_19",
    "tungsten_post_pass_20",
    "tungsten_post_pass_21",
    "tungsten_post_pass_22",
    "tungsten_post_pass_23",
    "tungsten_post_pass_24",
    "tungsten_post_pass_25",
    "tungsten_post_pass_26",
    "tungsten_post_pass_27",
    "tungsten_post_pass_28",
    "tungsten_post_pass_29",
    "tungsten_post_pass_30",
    "tungsten_post_pass_31",
];

/// The wgpu label of post-stack slot `slot`'s pass; one shared label past
/// the table.
fn post_pass_label(slot: usize) -> &'static str {
    POST_PASS_LABELS
        .get(slot)
        .copied()
        .unwrap_or("tungsten_post_pass_overflow")
}

/// The threshold pass's timing label in post-stack slot `slot`: its own
/// where the pyramid starts at a quarter of its target (`D-301`). The pass
/// then reads each texel's 4x4 footprint, the half-size pyramid's first
/// downsample with its threshold, and its time is no measure of the
/// half-size threshold's.
pub(crate) fn threshold_label(slot: usize, halvings: u32) -> String {
    if halvings >= 2 {
        format!("post{slot}_bloom_threshold_quarter")
    } else {
        format!("post{slot}_bloom_threshold")
    }
}

/// The composite pass's timing label in post-stack slot `slot`: its own
/// where it also draws the stack's trailing vignette (`D-302`), so the two
/// passes' work is not read as the composite's alone.
pub(crate) fn composite_label(slot: usize, with_vignette: bool) -> String {
    if with_vignette {
        format!("post{slot}_bloom_composite_vignette")
    } else {
        format!("post{slot}_bloom_composite")
    }
}

fn pass(
    kind: PassKind,
    label: &'static str,
    timing: impl Into<String>,
    color: Tex,
    write: Write,
) -> Pass<PassKind> {
    Pass {
        kind,
        label,
        timing: timing.into(),
        color: (color.base(), write),
        resolve: None,
        depth: None,
        reads: Vec::new(),
    }
}

/// The frame of `shape`: the scene, the post stack's passes with bloom's
/// own, SMAA's tail, the overlay and the frame's end.
pub(crate) fn declare(shape: &Shape) -> Graph<PassKind> {
    let mut graph = Graph::default();
    // The texture the frame's stages end on: the surface's, or a direct
    // capture's stand-in for it, a texture of the renderer's own.
    let output = if shape.present == PresentPath::DirectCapture {
        graph.import()
    } else {
        graph.import_surface()
    };

    // A version of the frame's colour. While SMAA is on it carries the
    // non-sRGB twin its edge pass reads through.
    let color = Spec {
        size: shape.output,
        format: shape.format,
        samples: 1,
        mips: 1,
        class: Class::Color,
        twin: if shape.smaa {
            non_srgb_twin(shape.format)
        } else {
            None
        },
    };

    // The scene writes the first version, through its resolve under MSAA.
    let mut current = graph.transient(color);
    let mut scene = if shape.samples > 1 {
        let multisampled = graph.transient(Spec {
            samples: shape.samples,
            class: Class::Multisampled,
            twin: None,
            ..color
        });
        let mut scene = pass(
            PassKind::Scene,
            "tungsten_scene_pass",
            "scene",
            multisampled,
            Write::Clear(SCENE_CLEAR),
        );
        scene.resolve = Some(current.base());
        scene
    } else {
        pass(
            PassKind::Scene,
            "tungsten_scene_pass",
            "scene",
            current,
            Write::Clear(SCENE_CLEAR),
        )
    };
    if shape.depth {
        let depth = graph.transient(Spec {
            format: DEPTH_FORMAT,
            samples: shape.samples,
            class: Class::Depth,
            twin: None,
            ..color
        });
        scene.depth = Some((depth, 1.0));
    }
    graph.pass(scene);

    // Each post pass reads a version and writes the next.
    let last_slot = shape.stages.len().saturating_sub(1);
    for (slot, stage) in shape.stages.iter().enumerate() {
        let next = graph.transient(color);
        match stage {
            Stage::Stock { name, own } => {
                let mut post = pass(
                    PassKind::Post { slot },
                    post_pass_label(slot),
                    format!("post{slot}_{name}"),
                    next,
                    full_screen(*own),
                );
                post.reads = vec![current.base()];
                graph.pass(post);
            }
            Stage::Bloom => {
                let vignette = shape.vignette && slot == last_slot;
                declare_bloom(&mut graph, shape, slot, current, next, vignette);
            }
        }
        current = next;
    }

    if shape.smaa {
        let work = |format| Spec {
            format,
            class: Class::Work,
            twin: None,
            ..color
        };
        let edges = graph.transient(work(SMAA_EDGES_FORMAT));
        let weights = graph.transient(work(SMAA_BLEND_FORMAT));
        // The tail's output is sampled by nothing SMAA reads, so it carries
        // no twin.
        let next = graph.transient(Spec {
            twin: None,
            ..color
        });
        // Each stage writes every pixel of its target: an edge or a weight
        // of zero is written, not left.
        let mut edge = pass(
            PassKind::SmaaEdges,
            "tungsten_smaa_edge_pass",
            "smaa_edges",
            edges,
            full_screen(!shape.replaced.holds(EngineStage::SmaaEdges)),
        );
        edge.reads = vec![current.base()];
        graph.pass(edge);
        let mut blend = pass(
            PassKind::SmaaWeights,
            "tungsten_smaa_blend_weights_pass",
            "smaa_blend_weights",
            weights,
            full_screen(!shape.replaced.holds(EngineStage::SmaaWeights)),
        );
        blend.reads = vec![edges.base()];
        graph.pass(blend);
        let mut neighborhood = pass(
            PassKind::SmaaNeighborhood,
            "tungsten_smaa_neighborhood_pass",
            "smaa_neighborhood",
            next,
            full_screen(!shape.replaced.holds(EngineStage::SmaaNeighborhood)),
        );
        neighborhood.reads = vec![current.base(), weights.base()];
        graph.pass(neighborhood);
        current = next;
    }

    // The overlay draws over the last version, after presentation AA.
    graph.pass(pass(
        PassKind::Overlay,
        "tungsten_text_overlay_pass",
        "text",
        current,
        Write::Over,
    ));

    let blit = |target: Tex| {
        pass(
            PassKind::Blit,
            "tungsten_present_pass",
            "present",
            target,
            Write::Whole,
        )
    };
    match shape.present {
        PresentPath::Direct => graph.present(current, output, blit(output)),
        PresentPath::Capture => {
            graph.copy_from(current);
            graph.present(current, output, blit(output));
        }
        PresentPath::DirectCapture => {
            // The frame ends on the stand-in as a direct frame ends on the
            // surface; the capture reads it and a blit shows it.
            let surface = graph.import_surface();
            graph.present(current, output, blit(output));
            let mut show = blit(surface);
            show.reads = vec![output.base()];
            graph.pass(show);
            graph.copy_from(output);
        }
    }
    graph
}

/// Bloom's passes in slot `slot`: the threshold into the pyramid's first
/// level, the downsamples, the additive upsamples, and the composite of
/// `source` and the pyramid into `target`.
fn declare_bloom(
    graph: &mut Graph<PassKind>,
    shape: &Shape,
    slot: usize,
    source: Tex,
    target: Tex,
    vignette: bool,
) {
    let mips = shape.bloom_mips.max(1);
    let pyramid = graph.transient(Spec {
        size: bloom_mip_extent_for(shape.output.0, shape.output.1, shape.bloom_halvings, 0),
        format: BLOOM_PYRAMID_FORMAT,
        samples: 1,
        mips,
        class: Class::Work,
        twin: None,
    });
    let level = |kind, label, timing: String, mip: u32, write, read| Pass {
        kind,
        label,
        timing,
        color: (pyramid.level(mip), write),
        resolve: None,
        depth: None,
        reads: vec![read],
    };
    graph.pass(level(
        PassKind::BloomThreshold { slot },
        "tungsten_bloom_threshold",
        threshold_label(slot, shape.bloom_halvings),
        0,
        full_screen(!shape.replaced.holds(EngineStage::BloomThreshold)),
        source.base(),
    ));
    for mip in 1..mips {
        graph.pass(level(
            PassKind::BloomDown { slot, level: mip },
            "tungsten_bloom_downsample",
            format!("post{slot}_bloom_down{mip}"),
            mip,
            full_screen(!shape.replaced.holds(EngineStage::BloomDownsample)),
            pyramid.level(mip - 1),
        ));
    }
    // Each upsample adds the level below into its own.
    for mip in (0..mips - 1).rev() {
        graph.pass(level(
            PassKind::BloomUp { slot, level: mip },
            "tungsten_bloom_upsample",
            format!("post{slot}_bloom_up{mip}"),
            mip,
            Write::Over,
            pyramid.level(mip + 1),
        ));
    }
    let mut composite = pass(
        PassKind::BloomComposite { slot, vignette },
        "tungsten_bloom_composite",
        composite_label(slot, vignette),
        target,
        full_screen(!shape.replaced.holds(EngineStage::BloomComposite)),
    );
    composite.reads = vec![source.base(), pyramid.base()];
    graph.pass(composite);
}

#[cfg(test)]
#[path = "../tests/graph_frame.rs"]
mod tests;
