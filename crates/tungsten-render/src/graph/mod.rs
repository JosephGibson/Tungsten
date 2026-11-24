//! The frame as a graph: passes declared in draw order with the textures
//! they read and write, compiled into each pass's load and store, the pooled
//! textures the frame holds and the pass the surface acquire splits it at
//! (`D-087`, `D-145`).
//!
//! Private to the crate. [`frame`] declares the engine's frame from its
//! inputs and [`pool`] lends the textures; this module knows neither, so
//! its rules are tested on declarations of any shape, without a device.
//! Passes keep their declared order: nothing here reorders, merges or
//! aliases memory.

pub(crate) mod frame;
pub(crate) mod pool;

use wgpu::TextureFormat;

/// A texture a declaration names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct Tex(u16);

/// One mip level of a texture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct Level {
    pub tex: Tex,
    pub mip: u32,
}

impl Tex {
    /// Level `mip` of the texture.
    pub(crate) fn level(self, mip: u32) -> Level {
        Level { tex: self, mip }
    }

    /// The texture's first level.
    pub(crate) fn base(self) -> Level {
        self.level(0)
    }
}

/// A buffer a declaration names: a compute pass writes it and a pass that
/// draws reads it (`D-310`). The declaration owns no buffer; the name is
/// what liveness follows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct Buf(u16);

/// What a pooled texture is for, which fixes its usage flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Class {
    /// A version of the frame's colour: drawn into, sampled, and copied from
    /// by a capture.
    Color,
    /// The multisampled scene colour: drawn into and resolved in its pass.
    Multisampled,
    /// The scene's depth.
    Depth,
    /// A pass's working texture, drawn into and sampled: SMAA's edges and
    /// weights, bloom's pyramid.
    Work,
}

/// What the pool allocates for a transient. Two transients of one spec
/// whose uses do not overlap share a texture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct Spec {
    pub size: (u32, u32),
    pub format: TextureFormat,
    pub samples: u32,
    pub mips: u32,
    pub class: Class,
    /// A second view format: the non-sRGB twin SMAA's edge pass reads
    /// through.
    pub twin: Option<TextureFormat>,
}

/// How a pass writes its colour target.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Write {
    /// Cleared, then drawn into: the pass leaves pixels unwritten.
    Clear(wgpu::Color),
    /// Every pixel written with blending off, so nothing of what the target
    /// held survives the pass. A declaration says so only of a pass it
    /// knows: one full-screen triangle, no blend, every channel written, by
    /// a shader that never discards (`D-306`).
    Whole,
    /// Drawn over what the target holds.
    Over,
}

/// A declared pass. `K` says what records it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Pass<K> {
    pub kind: K,
    /// The wgpu pass label.
    pub label: &'static str,
    /// The GPU timing label.
    pub timing: String,
    pub color: (Level, Write),
    /// The resolve target of a multisampled colour: written whole.
    pub resolve: Option<Level>,
    /// The depth target and the value it clears to.
    pub depth: Option<(Tex, f32)>,
    /// The levels the pass samples.
    pub reads: Vec<Level>,
    /// The buffers the pass reads.
    pub buffers: Vec<Buf>,
}

/// A declared compute pass. It draws into no texture: it writes buffers,
/// and it runs only while a pass that draws reads one of them.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Compute<K> {
    pub kind: K,
    /// The wgpu pass label.
    pub label: &'static str,
    /// The GPU timing label.
    pub timing: String,
    /// The buffers the pass writes.
    pub writes: Vec<Buf>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Decl {
    /// A texture the caller owns and has not drawn into: a direct
    /// capture's stand-in.
    Imported,
    /// The window surface's texture, acquired for the frame.
    Surface,
    Transient(Spec),
}

/// How a frame ends: `last` is what the output shows. Where nothing samples
/// `last` or copies from it, it *is* the output and the pass that writes it
/// writes the output; otherwise `blit` copies it there.
#[derive(Debug, Clone, PartialEq)]
struct Present<K> {
    last: Tex,
    output: Tex,
    blit: Pass<K>,
    /// How many passes were declared before the frame's end: the blit's
    /// place among them, ahead of a pass declared after.
    at: usize,
}

/// A frame's declaration.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Graph<K> {
    textures: Vec<Decl>,
    passes: Vec<Pass<K>>,
    /// How many buffers the declaration names.
    buffers: u16,
    /// The compute passes, each with how many passes were declared before
    /// it: its place among them.
    computes: Vec<(usize, Compute<K>)>,
    copied: Option<Tex>,
    present: Option<Present<K>>,
}

/// Why a declaration does not compile. Each is a bug in the declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum GraphError {
    /// The pass at this index draws over a level no earlier pass wrote.
    OverNothing(usize),
    /// The pass at this index attaches an imported texture as its depth.
    ImportedDepth(usize),
}

/// Where a compiled pass's attachment or source lives.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Target {
    /// An imported texture, by its order of import.
    Imported(u16),
    /// A pooled texture, by its slot in [`Compiled::slots`].
    Slot(u16),
}

/// One level of a [`Target`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct At {
    pub target: Target,
    pub mip: u32,
}

/// What a colour attachment starts a pass with.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Load {
    Clear(wgpu::Color),
    /// What the target holds.
    Keep,
    /// Neither a clear nor what the target holds (wgpu's `DontCare`): the
    /// pass writes every pixel of the surface before any other pass has
    /// (`D-306`).
    Nothing,
}

/// A compiled colour attachment.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Attach {
    pub at: At,
    pub load: Load,
    /// Whether the pass's result is kept: a later pass reads it, or it is an
    /// imported or copied-from texture.
    pub store: bool,
}

/// A compiled depth attachment, cleared at the pass's start.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct DepthAttach {
    pub slot: u16,
    pub clear: f32,
    pub store: bool,
}

/// A pass as the renderer records it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CompiledPass<K> {
    pub kind: K,
    pub label: &'static str,
    pub timing: String,
    pub color: Attach,
    pub resolve: Option<At>,
    pub depth: Option<DepthAttach>,
    pub reads: Vec<At>,
}

/// A compute pass as the renderer records it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CompiledCompute<K> {
    pub kind: K,
    pub label: &'static str,
    pub timing: String,
    /// The index in [`Compiled::passes`] of the pass it records ahead of.
    pub before: usize,
}

/// A compiled frame: the passes that draw, in declared order.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Compiled<K> {
    pub passes: Vec<CompiledPass<K>>,
    /// The compute passes that run: those whose buffers a pass that draws
    /// reads, in declared order.
    pub computes: Vec<CompiledCompute<K>>,
    /// The first pass that writes or samples an imported texture: the
    /// passes before it need no surface, and record before the acquire
    /// (`D-145`).
    pub split: usize,
    /// The pooled textures the frame holds, by slot.
    pub slots: Vec<Spec>,
    /// What a capture copies from.
    pub copied: Option<Target>,
}

impl<K: Copy> Default for Graph<K> {
    fn default() -> Self {
        Self {
            textures: Vec::new(),
            passes: Vec::new(),
            buffers: 0,
            computes: Vec::new(),
            copied: None,
            present: None,
        }
    }
}

impl<K: Copy> Graph<K> {
    /// A texture the caller owns: the surface's, or a capture's stand-in.
    /// A pass that writes one is never dropped.
    pub(crate) fn import(&mut self) -> Tex {
        self.declare(Decl::Imported)
    }

    /// The window surface's texture, imported: its first write, when it is
    /// whole, loads nothing (`D-306`). A texture the caller made itself is
    /// [`import`](Self::import)ed, and a first whole write clears it: a new
    /// texture must be initialised on every backend, where a surface's is
    /// each frame's to overwrite.
    pub(crate) fn import_surface(&mut self) -> Tex {
        self.declare(Decl::Surface)
    }

    /// Whether `tex` is the caller's, of either kind, and not the pool's.
    fn imported(&self, tex: Tex) -> bool {
        !matches!(self.textures[tex.0 as usize], Decl::Transient(_))
    }

    /// A texture the pool lends for as long as the frame's passes use it.
    pub(crate) fn transient(&mut self, spec: Spec) -> Tex {
        self.declare(Decl::Transient(spec))
    }

    fn declare(&mut self, decl: Decl) -> Tex {
        let tex = Tex(u16::try_from(self.textures.len()).expect("a frame names few textures"));
        self.textures.push(decl);
        tex
    }

    /// Declares the next pass in draw order.
    pub(crate) fn pass(&mut self, pass: Pass<K>) {
        self.passes.push(pass);
    }

    /// A buffer a compute pass writes for a pass that draws.
    pub(crate) fn buffer(&mut self) -> Buf {
        self.buffers += 1;
        Buf(self.buffers - 1)
    }

    /// Declares a compute pass at this place among the frame's passes: it
    /// records ahead of the next pass declared.
    pub(crate) fn compute(&mut self, compute: Compute<K>) {
        self.computes.push((self.passes.len(), compute));
    }

    /// A capture copies `tex` out after the frame: its last writer is kept,
    /// and it cannot become the output.
    pub(crate) fn copy_from(&mut self, tex: Tex) {
        self.copied = Some(tex);
    }

    /// Ends the frame: `output` shows `last`. `blit` is the pass that copies
    /// one to the other where they cannot be one texture; its colour target
    /// and its source are set here.
    pub(crate) fn present(&mut self, last: Tex, output: Tex, mut blit: Pass<K>) {
        blit.color = (output.base(), Write::Whole);
        blit.reads = vec![last.base()];
        self.present = Some(Present {
            last,
            output,
            blit,
            at: self.passes.len(),
        });
    }

    /// The frame's passes with their ops, the pooled textures they need and
    /// the acquire's split.
    pub(crate) fn compile(&self) -> Result<Compiled<K>, GraphError> {
        let passes = self.resolved();
        for (index, pass) in passes.iter().enumerate() {
            if let Some((depth, _)) = pass.depth
                && self.imported(depth)
            {
                return Err(GraphError::ImportedDepth(index));
            }
        }
        let live = self.live(&passes);
        // A compute pass runs while a pass that draws, declared after it,
        // reads a buffer it writes. Its place is among the declared passes;
        // a blit put in ahead of it moves it one on.
        let blit_at = match &self.present {
            Some(present) if !self.last_is_output() => Some(present.at.min(self.passes.len())),
            _ => None,
        };
        let computes: Vec<CompiledCompute<K>> = self
            .computes
            .iter()
            .filter_map(|(declared, compute)| {
                let place = declared + usize::from(blit_at.is_some_and(|blit| blit < *declared));
                let read = passes.iter().zip(&live).skip(place).any(|(pass, live)| {
                    *live
                        && pass
                            .buffers
                            .iter()
                            .any(|read| compute.writes.contains(read))
                });
                read.then(|| CompiledCompute {
                    kind: compute.kind,
                    label: compute.label,
                    timing: compute.timing.clone(),
                    before: live[..place.min(live.len())]
                        .iter()
                        .filter(|live| **live)
                        .count(),
                })
            })
            .collect();
        let passes: Vec<&Pass<K>> = passes
            .iter()
            .zip(&live)
            .filter_map(|(pass, live)| live.then_some(pass))
            .collect();
        check_over(&passes)?;

        // Each transient's first and last use among the passes that draw.
        let mut span: Vec<Option<(usize, usize)>> = vec![None; self.textures.len()];
        for (index, pass) in passes.iter().enumerate() {
            for tex in touched(pass) {
                let entry = &mut span[tex.0 as usize];
                *entry = Some(entry.map_or((index, index), |(first, _)| (first, index)));
            }
        }

        // A capture copies its texture out after the last pass, so that
        // texture is in use to the frame's end and no later transient takes
        // it.
        if let Some(copied) = self.copied.map(|tex| self.substituted(tex))
            && let Some((first, _)) = span[copied.0 as usize]
        {
            span[copied.0 as usize] = Some((first, usize::MAX));
        }

        // Slots in order of first use; a slot is free again after its
        // texture's last use, for a transient of the same spec.
        let mut order: Vec<(usize, Tex)> = span
            .iter()
            .enumerate()
            .filter_map(|(index, span)| span.map(|(first, _)| (first, Tex(index as u16))))
            .filter(|(_, tex)| matches!(self.textures[tex.0 as usize], Decl::Transient(_)))
            .collect();
        order.sort_by_key(|&(first, tex)| (first, tex.0));
        let mut slots: Vec<Spec> = Vec::new();
        let mut free_after: Vec<usize> = Vec::new();
        let mut slot_of: Vec<Option<u16>> = vec![None; self.textures.len()];
        for (first, tex) in order {
            let Decl::Transient(spec) = self.textures[tex.0 as usize] else {
                continue;
            };
            let last = span[tex.0 as usize].map_or(first, |(_, last)| last);
            let reuse =
                (0..slots.len()).find(|&slot| slots[slot] == spec && free_after[slot] < first);
            let slot = if let Some(slot) = reuse {
                free_after[slot] = last;
                slot
            } else {
                slots.push(spec);
                free_after.push(last);
                slots.len() - 1
            };
            slot_of[tex.0 as usize] = Some(slot as u16);
        }

        let imports: Vec<Tex> = (0..self.textures.len())
            .filter(|&index| !matches!(self.textures[index], Decl::Transient(_)))
            .map(|index| Tex(index as u16))
            .collect();
        let target = |tex: Tex| match slot_of[tex.0 as usize] {
            Some(slot) => Target::Slot(slot),
            None => Target::Imported(
                imports
                    .iter()
                    .position(|import| *import == tex)
                    .expect("a texture without a slot is imported") as u16,
            ),
        };
        let at = |level: Level| At {
            target: target(level.tex),
            mip: level.mip,
        };
        let imported = |tex: Tex| self.imported(tex);
        let surface = |tex: Tex| self.textures[tex.0 as usize] == Decl::Surface;
        let kept = |tex: Tex| imported(tex) || self.copied == Some(tex);

        let compiled: Vec<CompiledPass<K>> = passes
            .iter()
            .enumerate()
            .map(|(index, pass)| {
                let (level, write) = pass.color;
                let later = &passes[index + 1..];
                let unwritten = !passes[..index]
                    .iter()
                    .any(|earlier| earlier.color.0 == level || earlier.resolve == Some(level));
                let load = match write {
                    Write::Clear(color) => Load::Clear(color),
                    // The surface's first write, when it is whole, starts
                    // from nothing: a load of a newly acquired surface makes
                    // wgpu clear it in a pass of its own (`D-087`), and a
                    // clear is work the pass's own draw covers (`D-306`).
                    Write::Whole if unwritten && surface(level.tex) => Load::Nothing,
                    // Another imported texture is new, and a new texture's
                    // first use must initialise it on every backend.
                    Write::Whole if unwritten && imported(level.tex) => {
                        Load::Clear(wgpu::Color::TRANSPARENT)
                    }
                    Write::Whole | Write::Over => Load::Keep,
                };
                CompiledPass {
                    kind: pass.kind,
                    label: pass.label,
                    timing: pass.timing.clone(),
                    color: Attach {
                        at: at(level),
                        load,
                        store: kept(level.tex) || needed_later(level, later),
                    },
                    resolve: pass.resolve.map(at),
                    depth: pass.depth.map(|(tex, clear)| DepthAttach {
                        slot: slot_of[tex.0 as usize].expect("a depth target is pooled"),
                        clear,
                        // Nothing samples depth, and a later pass that
                        // attached it would clear it.
                        store: false,
                    }),
                    reads: pass.reads.iter().copied().map(at).collect(),
                }
            })
            .collect();
        let split = passes
            .iter()
            .position(|pass| touched(pass).any(imported))
            .unwrap_or(passes.len());
        Ok(Compiled {
            passes: compiled,
            computes,
            split,
            slots,
            copied: self.copied.map(|tex| self.substituted(tex)).map(target),
        })
    }

    /// The texture `tex` names once the output stands in for the frame's
    /// last version.
    fn substituted(&self, tex: Tex) -> Tex {
        match &self.present {
            Some(present) if present.last == tex && self.last_is_output() => present.output,
            _ => tex,
        }
    }

    /// Whether the frame's last version can be the output itself: nothing
    /// samples it and no capture copies from it.
    fn last_is_output(&self) -> bool {
        let Some(present) = &self.present else {
            return false;
        };
        let sampled = self
            .passes
            .iter()
            .any(|pass| pass.reads.iter().any(|level| level.tex == present.last));
        !sampled && self.copied != Some(present.last)
    }

    /// The declared passes with the frame's end applied: the output in the
    /// last version's place, or the blit where the end was declared.
    fn resolved(&self) -> Vec<Pass<K>> {
        let mut passes = self.passes.clone();
        let Some(present) = &self.present else {
            return passes;
        };
        if self.last_is_output() {
            let swap = |level: &mut Level| {
                if level.tex == present.last {
                    level.tex = present.output;
                }
            };
            for pass in &mut passes {
                swap(&mut pass.color.0);
                if let Some(resolve) = &mut pass.resolve {
                    swap(resolve);
                }
            }
        } else {
            passes.insert(present.at.min(passes.len()), present.blit.clone());
        }
        passes
    }

    /// Which passes draw: one that writes an imported or copied-from
    /// texture, or whose result a pass that draws reads or draws over.
    fn live(&self, passes: &[Pass<K>]) -> Vec<bool> {
        let kept = |tex: Tex| self.imported(tex) || self.copied == Some(tex);
        let mut needed: Vec<Level> = Vec::new();
        let mut live = vec![false; passes.len()];
        for (index, pass) in passes.iter().enumerate().rev() {
            let (level, write) = pass.color;
            let wanted =
                |level: Level, needed: &[Level]| kept(level.tex) || needed.contains(&level);
            let draws = wanted(level, &needed)
                || pass.resolve.is_some_and(|resolve| wanted(resolve, &needed));
            if !draws {
                continue;
            }
            live[index] = true;
            // A whole write ends the need for what the level held; a pass
            // that draws over it needs that too.
            if let Some(resolve) = pass.resolve {
                needed.retain(|other| *other != resolve);
            }
            match write {
                Write::Clear(_) | Write::Whole => needed.retain(|other| *other != level),
                Write::Over => {
                    if !needed.contains(&level) {
                        needed.push(level);
                    }
                }
            }
            for read in &pass.reads {
                if !needed.contains(read) {
                    needed.push(*read);
                }
            }
        }
        live
    }
}

/// The textures a pass attaches or samples.
fn touched<K>(pass: &Pass<K>) -> impl Iterator<Item = Tex> + '_ {
    std::iter::once(pass.color.0.tex)
        .chain(pass.resolve.map(|level| level.tex))
        .chain(pass.depth.map(|(tex, _)| tex))
        .chain(pass.reads.iter().map(|level| level.tex))
}

/// Whether a pass after this one samples `level` or draws over it before a
/// pass writes it whole.
fn needed_later<K>(level: Level, later: &[&Pass<K>]) -> bool {
    for pass in later {
        if pass.reads.contains(&level) {
            return true;
        }
        if pass.resolve == Some(level) {
            return false;
        }
        if pass.color.0 == level {
            return matches!(pass.color.1, Write::Over);
        }
    }
    false
}

/// Refuses a pass that draws over a level nothing wrote before it.
fn check_over<K>(passes: &[&Pass<K>]) -> Result<(), GraphError> {
    let mut written: Vec<Level> = Vec::new();
    for (index, pass) in passes.iter().enumerate() {
        let (level, write) = pass.color;
        if matches!(write, Write::Over) && !written.contains(&level) {
            return Err(GraphError::OverNothing(index));
        }
        written.push(level);
        if let Some(resolve) = pass.resolve {
            written.push(resolve);
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "../tests/graph.rs"]
mod tests;
