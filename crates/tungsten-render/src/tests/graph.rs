//! The graph's rules on declarations of any shape: what compiles, what is
//! dropped, which textures are shared, each pass's ops and the split.

use super::*;

const COLOR: Spec = Spec {
    size: (64, 32),
    format: TextureFormat::Rgba8UnormSrgb,
    samples: 1,
    mips: 1,
    class: Class::Color,
    twin: None,
};
const CLEAR: Write = Write::Clear(wgpu::Color::BLACK);

fn pass(kind: u8, color: Level, write: Write, reads: &[Level]) -> Pass<u8> {
    Pass {
        kind,
        label: "pass",
        timing: format!("p{kind}"),
        color: (color, write),
        resolve: None,
        depth: None,
        reads: reads.to_vec(),
        buffers: Vec::new(),
    }
}

fn blit() -> Pass<u8> {
    pass(99, Tex(0).base(), Write::Whole, &[])
}

/// Scene into `a`, a full-screen pass from `a` into `b`, an overlay over `b`.
fn two_stages() -> (Graph<u8>, Tex, Tex, Tex) {
    let mut graph = Graph::default();
    let output = graph.import_surface();
    let a = graph.transient(COLOR);
    let b = graph.transient(COLOR);
    graph.pass(pass(0, a.base(), CLEAR, &[]));
    graph.pass(pass(1, b.base(), Write::Whole, &[a.base()]));
    graph.pass(pass(2, b.base(), Write::Over, &[]));
    (graph, output, a, b)
}

fn kinds(compiled: &Compiled<u8>) -> Vec<u8> {
    compiled.passes.iter().map(|pass| pass.kind).collect()
}

#[test]
fn the_last_version_is_the_output_when_nothing_samples_it() {
    let (mut graph, output, _, b) = two_stages();
    graph.present(b, output, blit());
    let compiled = graph.compile().expect("compiles");

    assert_eq!(kinds(&compiled), [0, 1, 2], "no blit");
    // The stage that wrote the last version writes the output, whole, so
    // it starts from nothing; the overlay loads it.
    let stage = &compiled.passes[1];
    assert_eq!(stage.color.at.target, Target::Imported(0));
    assert_eq!(stage.color.load, Load::Nothing);
    assert_eq!(compiled.passes[2].color.at.target, Target::Imported(0));
    assert_eq!(compiled.passes[2].color.load, Load::Keep);
    // The scene's target is the frame's one pooled texture.
    assert_eq!(compiled.slots, [COLOR]);
    assert_eq!(compiled.passes[0].color.at.target, Target::Slot(0));
    assert_eq!(
        compiled.passes[1].reads,
        [At {
            target: Target::Slot(0),
            mip: 0
        }]
    );
    assert_eq!(compiled.split, 1, "the passes before the output's writer");
    assert_eq!(compiled.copied, None);
}

#[test]
fn only_the_surfaces_first_whole_write_starts_from_nothing() {
    let loads = |graph: &Graph<u8>| -> Vec<Load> {
        let compiled = graph.compile().expect("compiles");
        compiled.passes.iter().map(|pass| pass.color.load).collect()
    };

    // A whole write, an overlay, then a second whole write of the surface:
    // the first loads nothing, the others load what is there.
    let mut graph = Graph::default();
    let output = graph.import_surface();
    let scene = graph.transient(COLOR);
    graph.pass(pass(0, scene.base(), CLEAR, &[]));
    graph.pass(pass(1, output.base(), Write::Whole, &[scene.base()]));
    graph.pass(pass(2, output.base(), Write::Over, &[]));
    graph.pass(pass(3, output.base(), Write::Whole, &[scene.base()]));
    assert_eq!(
        loads(&graph),
        [
            Load::Clear(wgpu::Color::BLACK),
            Load::Nothing,
            Load::Keep,
            Load::Keep
        ]
    );

    // A pass that leaves pixels unwritten clears the surface as it clears
    // any target, and an offscreen whole write loads.
    let mut graph = Graph::default();
    let output = graph.import_surface();
    let scene = graph.transient(COLOR);
    let stage = graph.transient(COLOR);
    graph.pass(pass(0, scene.base(), CLEAR, &[]));
    graph.pass(pass(1, stage.base(), Write::Whole, &[scene.base()]));
    graph.pass(pass(2, output.base(), CLEAR, &[stage.base()]));
    assert_eq!(
        loads(&graph),
        [
            Load::Clear(wgpu::Color::BLACK),
            Load::Keep,
            Load::Clear(wgpu::Color::BLACK)
        ]
    );

    // A texture the caller made itself is new: its first whole write clears
    // it, which every backend takes for a new render target, and the
    // surface behind it starts from nothing.
    let mut graph = Graph::default();
    let standin = graph.import();
    let surface = graph.import_surface();
    let scene = graph.transient(COLOR);
    graph.pass(pass(0, scene.base(), CLEAR, &[]));
    graph.pass(pass(1, standin.base(), Write::Whole, &[scene.base()]));
    graph.pass(pass(2, standin.base(), Write::Over, &[]));
    graph.pass(pass(3, surface.base(), Write::Whole, &[standin.base()]));
    assert_eq!(
        loads(&graph),
        [
            Load::Clear(wgpu::Color::BLACK),
            Load::Clear(wgpu::Color::TRANSPARENT),
            Load::Keep,
            Load::Nothing
        ]
    );
    let compiled = graph.compile().expect("compiles");
    assert_eq!(compiled.passes[1].color.at.target, Target::Imported(0));
    assert_eq!(compiled.passes[3].color.at.target, Target::Imported(1));
    assert_eq!(compiled.split, 1, "either kind of import needs the acquire");

    // A resolve counts as a write: a whole write of a surface the scene
    // resolved into loads it.
    let mut graph = Graph::default();
    let surface = graph.import_surface();
    let multisampled = graph.transient(Spec {
        samples: 4,
        class: Class::Multisampled,
        ..COLOR
    });
    let mut scene = pass(0, multisampled.base(), CLEAR, &[]);
    scene.resolve = Some(surface.base());
    graph.pass(scene);
    graph.pass(pass(1, surface.base(), Write::Whole, &[]));
    assert_eq!(loads(&graph), [Load::Clear(wgpu::Color::BLACK), Load::Keep]);
}

#[test]
fn a_capture_keeps_the_last_version_offscreen_and_blits_it() {
    let (mut graph, output, _, b) = two_stages();
    graph.copy_from(b);
    graph.present(b, output, blit());
    let compiled = graph.compile().expect("compiles");

    assert_eq!(kinds(&compiled), [0, 1, 2, 99], "the blit follows");
    let last = compiled.passes[1].color.at;
    assert!(matches!(last.target, Target::Slot(_)), "{last:?}");
    assert_eq!(
        compiled.passes[1].color.load,
        Load::Keep,
        "an offscreen whole write loads"
    );
    assert_eq!(compiled.passes[2].color.at, last, "the overlay draws on it");
    let blit = &compiled.passes[3];
    assert_eq!(blit.reads, [last]);
    assert_eq!(blit.color.at.target, Target::Imported(0));
    assert_eq!(compiled.copied, Some(last.target));
    assert_eq!(compiled.slots.len(), 2);
    assert_eq!(compiled.split, 3, "only the blit needs the surface");
}

#[test]
fn a_sampled_last_version_cannot_be_the_output() {
    // A pass after the last stage samples its result: the surface cannot be
    // sampled, so the frame blits.
    let (mut graph, output, _, b) = two_stages();
    let c = graph.transient(COLOR);
    graph.pass(pass(3, c.base(), Write::Whole, &[b.base()]));
    graph.copy_from(c);
    graph.present(b, output, blit());
    let compiled = graph.compile().expect("compiles");
    assert_eq!(kinds(&compiled), [0, 1, 2, 3, 99]);
}

#[test]
fn a_pass_nothing_reads_is_dropped_with_what_only_it_read() {
    let (mut graph, output, a, b) = two_stages();
    // A side chain off the scene's target that nothing consumes.
    let work = Spec {
        class: Class::Work,
        ..COLOR
    };
    let x = graph.transient(work);
    let y = graph.transient(work);
    graph.pass(pass(7, x.base(), Write::Whole, &[a.base()]));
    graph.pass(pass(8, y.base(), Write::Whole, &[x.base()]));
    graph.present(b, output, blit());
    let compiled = graph.compile().expect("compiles");
    assert_eq!(kinds(&compiled), [0, 1, 2], "both side passes are gone");
    assert_eq!(compiled.slots, [COLOR], "and so are their textures");
}

#[test]
fn a_pass_a_live_pass_reads_is_kept() {
    let (mut graph, output, a, b) = two_stages();
    let work = Spec {
        class: Class::Work,
        ..COLOR
    };
    let x = graph.transient(work);
    graph.pass(pass(7, x.base(), Write::Whole, &[a.base()]));
    // A last stage that samples the side texture.
    let c = graph.transient(COLOR);
    graph.pass(pass(3, c.base(), Write::Whole, &[b.base(), x.base()]));
    graph.present(c, output, blit());
    let compiled = graph.compile().expect("compiles");
    assert_eq!(kinds(&compiled), [0, 1, 2, 7, 3]);
    assert!(compiled.slots.contains(&work));
}

#[test]
fn drawing_over_nothing_is_refused() {
    let mut graph = Graph::default();
    let output = graph.import();
    let a = graph.transient(COLOR);
    graph.pass(pass(0, a.base(), Write::Over, &[]));
    graph.present(a, output, blit());
    assert_eq!(graph.compile(), Err(GraphError::OverNothing(0)));
}

#[test]
fn an_imported_depth_is_refused() {
    let mut graph = Graph::default();
    let output = graph.import();
    let a = graph.transient(COLOR);
    let mut scene = pass(0, a.base(), CLEAR, &[]);
    scene.depth = Some((output, 1.0));
    graph.pass(scene);
    graph.present(a, output, blit());
    assert_eq!(graph.compile(), Err(GraphError::ImportedDepth(0)));
}

/// A chain of `stages` full-screen passes after the scene, each reading the
/// version before it; the last version is presented.
fn chain(stages: u8, extra_read_of_first: bool) -> Compiled<u8> {
    let mut graph = Graph::default();
    let output = graph.import();
    let first = graph.transient(COLOR);
    graph.pass(pass(0, first.base(), CLEAR, &[]));
    let mut current = first;
    for stage in 1..=stages {
        let next = graph.transient(COLOR);
        let mut reads = vec![current.base()];
        if extra_read_of_first && stage == stages {
            reads.push(first.base());
        }
        graph.pass(pass(stage, next.base(), Write::Whole, &reads));
        current = next;
    }
    graph.present(current, output, blit());
    graph.compile().expect("compiles")
}

#[test]
fn transients_share_a_texture_only_when_their_uses_do_not_overlap() {
    // scene -> a, 1: a -> b, 2: b -> c, 3: c -> d (the output). `c` takes
    // `a`'s texture, free since pass 1.
    let compiled = chain(3, false);
    assert_eq!(
        compiled.slots,
        [COLOR, COLOR],
        "two textures carry three versions"
    );
    let target = |index: usize| compiled.passes[index].color.at.target;
    assert_eq!(target(0), Target::Slot(0));
    assert_eq!(target(1), Target::Slot(1));
    assert_eq!(target(2), Target::Slot(0), "the scene's texture again");
    assert_eq!(target(3), Target::Imported(0));
    // A pass never samples the texture it draws into.
    for pass in &compiled.passes {
        assert!(!pass.reads.contains(&pass.color.at), "{pass:?}");
    }

    // While the last stage still reads the first version, nothing may take
    // its texture.
    let compiled = chain(3, true);
    assert_eq!(compiled.slots.len(), 3);
}

#[test]
fn a_long_chain_alternates_between_two_textures() {
    let compiled = chain(17, false);
    assert_eq!(compiled.slots, [COLOR, COLOR]);
    for (index, pass) in compiled.passes.iter().enumerate().take(17) {
        assert_eq!(
            pass.color.at.target,
            Target::Slot((index % 2) as u16),
            "pass {index}"
        );
    }
}

#[test]
fn a_copied_texture_is_held_to_the_frames_end() {
    // A capture of an early version: nothing reads `first` after pass 1,
    // but the copy runs after the last pass, so `third`, of the same spec,
    // must not take its texture.
    let mut graph = Graph::default();
    let output = graph.import();
    let first = graph.transient(COLOR);
    let second = graph.transient(COLOR);
    let third = graph.transient(COLOR);
    graph.pass(pass(0, first.base(), CLEAR, &[]));
    graph.pass(pass(1, second.base(), Write::Whole, &[first.base()]));
    graph.pass(pass(2, third.base(), Write::Whole, &[second.base()]));
    graph.pass(pass(3, output.base(), Write::Whole, &[third.base()]));
    let shared = graph.compile().expect("compiles");
    assert_eq!(shared.slots, [COLOR, COLOR], "uncopied, the third takes it");

    graph.copy_from(first);
    let compiled = graph.compile().expect("compiles");
    assert_eq!(compiled.slots, [COLOR, COLOR, COLOR]);
    let copied = compiled.copied.expect("a copy source");
    assert_eq!(copied, compiled.passes[0].color.at.target);
    assert!(compiled.passes[0].color.store);
    for pass in &compiled.passes[1..] {
        assert_ne!(pass.color.at.target, copied, "pass {}", pass.kind);
    }
}

#[test]
fn the_blit_lands_where_the_frames_end_was_declared() {
    // A captured last version cannot be the output, so the blit copies it
    // there: before the pass that then shows the output on a second import,
    // which was declared after the frame's end.
    let mut graph = Graph::default();
    let output = graph.import();
    let surface = graph.import();
    let scene = graph.transient(COLOR);
    graph.pass(pass(0, scene.base(), CLEAR, &[]));
    graph.copy_from(scene);
    graph.present(scene, output, blit());
    graph.pass(pass(50, surface.base(), Write::Whole, &[output.base()]));
    let compiled = graph.compile().expect("compiles");
    assert_eq!(kinds(&compiled), [0, 99, 50]);
    assert_eq!(compiled.passes[2].reads[0].target, Target::Imported(0));
    assert_eq!(compiled.split, 1);
}

#[test]
fn textures_of_different_specs_never_share() {
    let mut graph = Graph::default();
    let output = graph.import();
    let small = Spec {
        size: (32, 16),
        ..COLOR
    };
    let wide_first = graph.transient(COLOR);
    let small_first = graph.transient(small);
    let wide_second = graph.transient(COLOR);
    graph.pass(pass(0, wide_first.base(), CLEAR, &[]));
    graph.pass(pass(
        1,
        small_first.base(),
        Write::Whole,
        &[wide_first.base()],
    ));
    graph.pass(pass(
        2,
        wide_second.base(),
        Write::Whole,
        &[small_first.base()],
    ));
    let small_second = graph.transient(small);
    graph.pass(pass(
        3,
        small_second.base(),
        Write::Whole,
        &[wide_second.base()],
    ));
    let last = graph.transient(COLOR);
    graph.pass(pass(4, last.base(), Write::Whole, &[small_second.base()]));
    graph.present(last, output, blit());
    let compiled = graph.compile().expect("compiles");
    // The second wide one takes the first's texture, the second small one
    // the first small one's; neither takes a texture of the other spec.
    assert_eq!(compiled.slots, [COLOR, small]);
}

#[test]
fn a_result_is_stored_only_while_something_reads_it() {
    let mut graph = Graph::default();
    let output = graph.import();
    let resolved = graph.transient(COLOR);
    let multisampled = graph.transient(Spec {
        samples: 4,
        class: Class::Multisampled,
        ..COLOR
    });
    let depth = graph.transient(Spec {
        format: TextureFormat::Depth32Float,
        samples: 4,
        class: Class::Depth,
        ..COLOR
    });
    let mut scene = pass(0, multisampled.base(), CLEAR, &[]);
    scene.resolve = Some(resolved.base());
    scene.depth = Some((depth, 1.0));
    graph.pass(scene);
    let next = graph.transient(COLOR);
    graph.pass(pass(1, next.base(), Write::Whole, &[resolved.base()]));
    graph.pass(pass(2, next.base(), Write::Over, &[]));
    graph.present(next, output, blit());
    let compiled = graph.compile().expect("compiles");

    let scene = &compiled.passes[0];
    assert!(
        !scene.color.store,
        "the multisampled colour is resolved, not kept"
    );
    assert!(!scene.depth.expect("depth").store, "nothing reads depth");
    assert!(scene.resolve.is_some());
    assert!(compiled.passes[1].color.store, "the output is kept");
    assert!(compiled.passes[2].color.store);
}

#[test]
fn a_level_drawn_over_later_is_stored() {
    // A pyramid level written whole, then added to: the first write is kept
    // for the second.
    let mut graph = Graph::default();
    let output = graph.import();
    let a = graph.transient(COLOR);
    let pyramid = graph.transient(Spec {
        mips: 2,
        class: Class::Work,
        ..COLOR
    });
    graph.pass(pass(0, a.base(), CLEAR, &[]));
    graph.pass(pass(1, pyramid.level(0), Write::Whole, &[a.base()]));
    graph.pass(pass(2, pyramid.level(1), Write::Whole, &[pyramid.level(0)]));
    graph.pass(pass(3, pyramid.level(0), Write::Over, &[pyramid.level(1)]));
    let b = graph.transient(COLOR);
    graph.pass(pass(
        4,
        b.base(),
        Write::Whole,
        &[a.base(), pyramid.level(0)],
    ));
    graph.present(b, output, blit());
    let compiled = graph.compile().expect("compiles");
    assert_eq!(kinds(&compiled), [0, 1, 2, 3, 4]);
    assert!(compiled.passes.iter().all(|pass| pass.color.store));
    assert_eq!(compiled.passes[3].color.load, Load::Keep);
    assert_eq!(compiled.passes[3].color.at.mip, 0);
    assert_eq!(compiled.passes[2].color.at.mip, 1);
}

#[test]
fn the_split_is_the_first_pass_that_writes_an_import() {
    // A scene that resolves straight into the output splits at 0.
    let mut graph = Graph::default();
    let output = graph.import();
    let resolved = graph.transient(COLOR);
    let multisampled = graph.transient(Spec {
        samples: 4,
        class: Class::Multisampled,
        ..COLOR
    });
    let mut scene = pass(0, multisampled.base(), CLEAR, &[]);
    scene.resolve = Some(resolved.base());
    graph.pass(scene);
    graph.pass(pass(1, resolved.base(), Write::Over, &[]));
    graph.present(resolved, output, blit());
    let compiled = graph.compile().expect("compiles");
    assert_eq!(compiled.split, 0);
    assert_eq!(
        compiled.passes[0].resolve,
        Some(At {
            target: Target::Imported(0),
            mip: 0
        })
    );
    assert_eq!(
        compiled.slots.len(),
        1,
        "only the multisampled colour is pooled"
    );

    // A frame with no pass on an import never splits.
    let mut graph: Graph<u8> = Graph::default();
    let a = graph.transient(COLOR);
    graph.pass(pass(0, a.base(), CLEAR, &[]));
    graph.copy_from(a);
    let compiled = graph.compile().expect("compiles");
    assert_eq!(compiled.split, 1);
}

#[test]
fn a_pass_that_samples_an_import_records_after_the_acquire() {
    // An imported texture a pass only samples needs the surface as much as
    // one it draws into.
    let mut graph = Graph::default();
    let output = graph.import();
    let input = graph.import();
    let scene = graph.transient(COLOR);
    graph.pass(pass(0, scene.base(), CLEAR, &[input.base()]));
    graph.pass(pass(1, output.base(), Write::Whole, &[scene.base()]));
    let compiled = graph.compile().expect("compiles");
    assert_eq!(kinds(&compiled), [0, 1]);
    assert_eq!(compiled.split, 0);
}

#[test]
fn a_second_import_is_addressed_in_order() {
    // A direct capture: the frame ends on a stand-in, which a blit shows on
    // the surface and the capture copies.
    let (mut graph, output, _, b) = two_stages();
    let surface = graph.import();
    graph.present(b, output, blit());
    let mut show = pass(98, surface.base(), Write::Whole, &[output.base()]);
    show.label = "show";
    graph.pass(show);
    graph.copy_from(output);
    let compiled = graph.compile().expect("compiles");
    assert_eq!(kinds(&compiled), [0, 1, 2, 98]);
    assert_eq!(compiled.passes[1].color.at.target, Target::Imported(0));
    let show = &compiled.passes[3];
    assert_eq!(show.color.at.target, Target::Imported(1));
    assert_eq!(
        show.reads,
        [At {
            target: Target::Imported(0),
            mip: 0
        }]
    );
    assert_eq!(compiled.copied, Some(Target::Imported(0)));
    assert_eq!(compiled.split, 1);
}

fn compute(kind: u8, writes: &[Buf]) -> Compute<u8> {
    Compute {
        kind,
        label: "compute",
        timing: format!("c{kind}"),
        writes: writes.to_vec(),
    }
}

/// `pass` reading `buffers` as well.
fn reading(mut pass: Pass<u8>, buffers: &[Buf]) -> Pass<u8> {
    pass.buffers = buffers.to_vec();
    pass
}

/// A compute pass runs while a pass that draws reads a buffer it writes
/// (`D-310`), and records ahead of the pass declared after it.
#[test]
fn a_compute_pass_a_live_pass_reads_is_kept() {
    let mut graph = Graph::default();
    let output = graph.import_surface();
    let a = graph.transient(COLOR);
    let (tiles, spare) = (graph.buffer(), graph.buffer());
    graph.compute(compute(50, &[tiles, spare]));
    graph.pass(reading(pass(0, a.base(), CLEAR, &[]), &[tiles]));
    graph.pass(pass(1, a.base(), Write::Over, &[]));
    graph.present(a, output, blit());
    let compiled = graph.compile().expect("compiles");

    assert_eq!(kinds(&compiled), [0, 1]);
    assert_eq!(compiled.computes.len(), 1);
    let bins = &compiled.computes[0];
    assert_eq!((bins.kind, bins.before), (50, 0), "ahead of the scene");
    assert_eq!(bins.timing, "c50");
    // It holds no texture and moves no pass's ops or the split.
    let mut plain = Graph::default();
    let output = plain.import_surface();
    let a = plain.transient(COLOR);
    plain.pass(pass(0, a.base(), CLEAR, &[]));
    plain.pass(pass(1, a.base(), Write::Over, &[]));
    plain.present(a, output, blit());
    let plain = plain.compile().expect("compiles");
    assert_eq!(compiled.passes, plain.passes);
    assert_eq!(compiled.slots, plain.slots);
    assert_eq!(compiled.split, plain.split);
}

/// A compute pass whose buffers no pass that draws reads does not run:
/// nothing reads them, a pass declared before it reads them, or the one
/// pass that reads them is itself dropped.
#[test]
fn a_compute_pass_nothing_reads_is_dropped() {
    // Nothing reads the buffer.
    let mut graph = Graph::default();
    let output = graph.import_surface();
    let a = graph.transient(COLOR);
    let tiles = graph.buffer();
    graph.compute(compute(50, &[tiles]));
    graph.pass(pass(0, a.base(), CLEAR, &[]));
    graph.present(a, output, blit());
    let compiled = graph.compile().expect("compiles");
    assert_eq!(kinds(&compiled), [0]);
    assert!(compiled.computes.is_empty(), "nothing reads its buffer");

    // The reader was declared ahead of it: it read last frame's buffer.
    let mut graph = Graph::default();
    let output = graph.import_surface();
    let a = graph.transient(COLOR);
    let tiles = graph.buffer();
    graph.pass(reading(pass(0, a.base(), CLEAR, &[]), &[tiles]));
    graph.compute(compute(50, &[tiles]));
    graph.pass(pass(1, a.base(), Write::Over, &[]));
    graph.present(a, output, blit());
    let compiled = graph.compile().expect("compiles");
    assert!(compiled.computes.is_empty(), "its reader drew before it");

    // The reader draws into a texture nothing shows, so it is dropped, and
    // the compute pass with it.
    let mut graph = Graph::default();
    let output = graph.import_surface();
    let a = graph.transient(COLOR);
    let unseen = graph.transient(COLOR);
    let tiles = graph.buffer();
    graph.compute(compute(50, &[tiles]));
    graph.pass(pass(0, a.base(), CLEAR, &[]));
    graph.pass(reading(pass(1, unseen.base(), CLEAR, &[]), &[tiles]));
    graph.present(a, output, blit());
    let compiled = graph.compile().expect("compiles");
    assert_eq!(kinds(&compiled), [0], "the reader is dropped");
    assert!(compiled.computes.is_empty(), "its one reader is dropped");
}

/// A compute pass's place counts the passes that draw: a dropped pass ahead
/// of it does not, and the passes ahead of it that do, do.
#[test]
fn a_compute_pass_records_ahead_of_the_next_pass_that_draws() {
    let mut graph = Graph::default();
    let output = graph.import_surface();
    let a = graph.transient(COLOR);
    let unseen = graph.transient(COLOR);
    let (first, second) = (graph.buffer(), graph.buffer());
    graph.pass(pass(0, unseen.base(), CLEAR, &[]));
    graph.pass(pass(1, a.base(), CLEAR, &[]));
    graph.compute(compute(50, &[first]));
    graph.pass(reading(pass(2, a.base(), Write::Over, &[]), &[first]));
    graph.compute(compute(51, &[second]));
    graph.pass(reading(
        pass(3, a.base(), Write::Over, &[]),
        &[second, first],
    ));
    graph.present(a, output, blit());
    let compiled = graph.compile().expect("compiles");

    assert_eq!(kinds(&compiled), [1, 2, 3], "the unseen pass is dropped");
    let places: Vec<(u8, usize)> = compiled
        .computes
        .iter()
        .map(|compute| (compute.kind, compute.before))
        .collect();
    assert_eq!(places, [(50, 1), (51, 2)]);
}

/// A blit put in where the frame's end was declared moves the passes
/// declared after it one place on, and a compute pass declared among them
/// with them: it still records ahead of the pass declared after it, and a
/// pass declared before it, the blit's neighbour, does not keep it.
#[test]
fn a_compute_pass_declared_after_a_blit_keeps_its_place() {
    let build = |reader_before: bool| {
        let mut graph = Graph::default();
        let output = graph.import_surface();
        let a = graph.transient(COLOR);
        let tiles = graph.buffer();
        graph.pass(pass(0, a.base(), CLEAR, &[]));
        // A capture copies the last version, so a blit shows it.
        graph.copy_from(a);
        graph.present(a, output, blit());
        let over = pass(1, output.base(), Write::Over, &[]);
        graph.pass(if reader_before {
            reading(over, &[tiles])
        } else {
            over
        });
        graph.compute(compute(50, &[tiles]));
        graph.pass(reading(pass(2, output.base(), Write::Over, &[]), &[tiles]));
        graph.compile().expect("compiles")
    };
    let compiled = build(false);
    assert_eq!(kinds(&compiled), [0, 99, 1, 2], "the blit after the scene");
    let places: Vec<(u8, usize)> = compiled
        .computes
        .iter()
        .map(|compute| (compute.kind, compute.before))
        .collect();
    assert_eq!(places, [(50, 3)], "ahead of the pass declared after it");

    // With its one reader gone, a pass declared before it that reads the
    // buffer does not keep it.
    let mut graph = Graph::default();
    let output = graph.import_surface();
    let a = graph.transient(COLOR);
    let tiles = graph.buffer();
    graph.pass(pass(0, a.base(), CLEAR, &[]));
    graph.copy_from(a);
    graph.present(a, output, blit());
    graph.pass(reading(pass(1, output.base(), Write::Over, &[]), &[tiles]));
    graph.compute(compute(50, &[tiles]));
    graph.pass(pass(2, output.base(), Write::Over, &[]));
    let compiled = graph.compile().expect("compiles");
    assert_eq!(kinds(&compiled), [0, 99, 1, 2]);
    assert!(
        compiled.computes.is_empty(),
        "its reader was declared before it"
    );
    let _ = build(true);
}
