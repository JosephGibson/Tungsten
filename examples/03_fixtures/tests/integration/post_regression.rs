//! Opt-in GPU regressions for the stock post stack, on the fixtures crate's
//! `post` scene.
//!
//! Gate: `TUNGSTEN_VISUAL_REGRESSION=1` (needs GPU/display). Each test compares
//! two captures of this machine with each other, so there is no fixture image.
//! Capture: 8 smoke frames, frame 5 at 1280x720.

use std::path::{Path, PathBuf};

use tungsten::render::compare_png;

/// Runs the `post` scene in `root` with `post_stack` preloaded and captures
/// frame 5 into a file named after `name`. `extra_env` is set last.
fn capture(name: &str, post_stack: &str, root: &Path, extra_env: &[(&str, &str)]) -> PathBuf {
    capture_logged(name, post_stack, root, extra_env).0
}

/// [`capture`], and the run's log.
fn capture_logged(
    name: &str,
    post_stack: &str,
    root: &Path,
    extra_env: &[(&str, &str)],
) -> (PathBuf, String) {
    let actual = std::env::temp_dir().join(format!("tungsten-post-regression-{name}.png"));
    if actual.exists() {
        let _ = std::fs::remove_file(&actual);
    }

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_example-03-fixtures"))
        .current_dir(root)
        .env("TUNGSTEN_FIXTURE", "post")
        .env("TUNGSTEN_POST_STACK_FIXTURE", post_stack)
        .env("TUNGSTEN_POST_AA_FIXTURE", "off")
        .env_remove("TUNGSTEN_BLOOM_FIXTURE")
        .env_remove("TUNGSTEN_GAME_FEEL_FIXTURE")
        .env_remove("TUNGSTEN_CAPTURE_DIRECT")
        .env_remove("TUNGSTEN_MESH_TRAIL_FIXTURE")
        .env_remove("TUNGSTEN_MATERIAL_PAIR_FIXTURE")
        .env_remove("TUNGSTEN_POST_BACKDROP_FIXTURE")
        .env_remove("TUNGSTEN_POST_TEXT_FIXTURE")
        .env_remove("TUNGSTEN_RENDER_POST_FUSE")
        .env_remove("TUNGSTEN_RENDER_HDR")
        .env_remove("TUNGSTEN_POST_BRIGHT_FIXTURE")
        .env("TUNGSTEN_SMOKE_FRAMES", "8")
        .env("TUNGSTEN_CAPTURE_FRAME", "5")
        .env("TUNGSTEN_CAPTURE_RESOLUTION", "1280x720")
        .env("TUNGSTEN_CAPTURE_PATH", &actual)
        // The fused-against-alone test reads the renderer's lines.
        .env("RUST_LOG", "info")
        .envs(extra_env.iter().copied())
        .output()
        .expect("run example-03-fixtures");
    let log = String::from_utf8_lossy(&output.stderr).into_owned();
    assert!(
        output.status.success(),
        "example-03-fixtures exited with {:?}:\n{log}",
        output.status
    );
    assert!(
        actual.exists(),
        "capture did not produce {}",
        actual.display()
    );
    (actual, log)
}

/// The scene loads its manifests relative to the workspace root.
fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Creates `to` and links every entry of `from` into it, except `skip`.
#[cfg(unix)]
fn mirror(from: &Path, to: &Path, skip: &str) {
    std::fs::create_dir_all(to).expect("create the staged directory");
    for entry in std::fs::read_dir(from).expect("read the workspace directory") {
        let entry = entry.expect("read a directory entry");
        if entry.file_name() != skip {
            std::os::unix::fs::symlink(entry.path(), to.join(entry.file_name()))
                .expect("link a workspace entry into the stage");
        }
    }
}

/// A directory the scene can run in: everything it reads is a link into
/// the workspace, except `file`, a path from the workspace root under one of
/// the scene's two asset trees, whose text went through `edit`.
#[cfg(unix)]
fn stage_with_edited_file(file: &str, edit: impl Fn(&str) -> String) -> PathBuf {
    use std::os::unix::fs::symlink;

    let root = workspace_root()
        .canonicalize()
        .expect("canonical workspace root");
    // Tests run in parallel and two may stage the same file: one directory
    // per call.
    static STAGES: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let stem = Path::new(file)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .expect("the staged file's name");
    let stage = std::env::temp_dir().join(format!(
        "tungsten-post-regression-stage-{stem}-{}-{}",
        std::process::id(),
        STAGES.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&stage);
    std::fs::create_dir_all(&stage).expect("create the stage");
    for config in ["tungsten.json", "input.json"] {
        symlink(root.join(config), stage.join(config)).expect("link a config file");
    }

    let mut staged = false;
    for tree in ["assets", "examples/03_fixtures/assets"] {
        let (from, to) = (root.join(tree), stage.join(tree));
        std::fs::create_dir_all(to.parent().expect("a tree has a parent"))
            .expect("create the staged tree's parent");
        let Ok(inside) = Path::new(file).strip_prefix(tree) else {
            symlink(&from, &to).expect("link an asset tree");
            continue;
        };
        // Each directory down to the file is a directory of links, less the
        // entry the next level stands in for.
        let mut below = PathBuf::new();
        for part in inside.components() {
            mirror(
                &from.join(&below),
                &to.join(&below),
                part.as_os_str().to_str().expect("a UTF-8 path"),
            );
            below.push(part);
        }
        staged = true;
    }
    assert!(staged, "{file} is under neither asset tree");

    let source = std::fs::read_to_string(root.join(file)).expect("read the file to stage");
    std::fs::write(stage.join(file), edit(&source)).expect("write the edited file");
    stage
}

/// [`stage_with_edited_file`] for the stock shader `name`.
#[cfg(unix)]
fn stage_with_edited_stock_shader(name: &str, edit: impl Fn(&str) -> String) -> PathBuf {
    stage_with_edited_file(&format!("assets/shaders/stock/{name}.wgsl"), edit)
}

/// Pixels of `lhs` and `rhs` further apart than 8, and the frame's pixel count.
fn pixels_apart(lhs: &Path, rhs: &Path) -> (u32, u32) {
    let report = compare_png(lhs, rhs, 8).expect("compare the two captures");
    (report.pixels_above_tolerance, report.width * report.height)
}

/// Two passes of one stock effect each draw with their own parameters
/// (`D-090`). A red half fade under a blue one must not look like two blue
/// ones, which is what a params buffer shared by the variant drew.
#[test]
fn repeated_effect_keeps_its_own_params() {
    if std::env::var("TUNGSTEN_VISUAL_REGRESSION").is_err() {
        return;
    }

    // Each fade in a pass of its own, as `D-090` drew them, and both in the
    // one pass the composer generates (`D-307`), which holds a block a
    // stage.
    let root = workspace_root();
    for fuse in ["false", "true"] {
        let env = [("TUNGSTEN_RENDER_POST_FUSE", fuse)];
        let (pair, log) = capture_logged(&format!("fade-pair-{fuse}"), "fade_pair", &root, &env);
        let twice = capture(&format!("fade-twice-{fuse}"), "fade_twice", &root, &env);
        assert_eq!(
            log.contains(FUSED_LINE),
            fuse == "true",
            "fusing {fuse}:\n{log}"
        );
        let (apart, total) = pixels_apart(&pair, &twice);
        assert!(
            apart * 2 > total,
            "fusing {fuse}: the first fade's color is lost: {apart} of {total} pixels differ"
        );
    }
}

/// A body edit of a stock post shader reaches its pipeline (`D-091`). The
/// scene started on a tree whose `fade.wgsl` swaps red and blue must not
/// draw what the shipped shader draws; a pipeline left on its compiled-in
/// source does.
#[cfg(unix)]
#[test]
fn stock_shader_body_edit_changes_the_frame() {
    if std::env::var("TUNGSTEN_VISUAL_REGRESSION").is_err() {
        return;
    }

    let shipped = capture("fade-shipped", "fade_twice", &workspace_root(), &[]);
    let stage = stage_with_edited_stock_shader("fade", |source| {
        assert!(
            source.contains("p.v0.rgb"),
            "fade.wgsl no longer reads p.v0.rgb; pick another edit"
        );
        source.replace("p.v0.rgb", "p.v0.bgr")
    });
    let edited = capture("fade-edited", "fade_twice", &stage, &[]);
    let _ = std::fs::remove_dir_all(&stage);

    let (apart, total) = pixels_apart(&shipped, &edited);
    assert!(
        apart * 2 > total,
        "the edited fade.wgsl did not reach its pipeline: {apart} of {total} pixels differ"
    );
}

/// A stock shader edit that passes Naga but not pipeline validation keeps the
/// shipped pipeline (B2, `D-057`). The scene started on a tree whose
/// `fade.wgsl` also reads a `@group(2)` uniform, which the post pipeline layout
/// does not have, must exit cleanly and draw what the shipped shader draws;
/// without an error scope the default handler panicked.
#[cfg(unix)]
#[test]
fn incompatible_stock_shader_edit_keeps_the_shipped_pipeline() {
    if std::env::var("TUNGSTEN_VISUAL_REGRESSION").is_err() {
        return;
    }

    let shipped = capture("fade-shipped-b2", "fade_twice", &workspace_root(), &[]);
    let stage = stage_with_edited_stock_shader("fade", |source| {
        let params = "@group(1) @binding(0) var<uniform> params: Params;";
        let read = "in.uv, params);";
        assert!(
            source.contains(params) && source.contains(read),
            "fade.wgsl changed; pick another incompatible edit"
        );
        source
            .replace(
                params,
                &format!("{params}\n@group(2) @binding(0) var<uniform> extra: vec4<f32>;"),
            )
            .replace(read, "in.uv, params) + extra * 0.0;")
    });
    let edited = capture("fade-incompatible", "fade_twice", &stage, &[]);
    let _ = std::fs::remove_dir_all(&stage);

    let (apart, total) = pixels_apart(&shipped, &edited);
    assert_eq!(
        apart, 0,
        "the incompatible fade.wgsl replaced the shipped pipeline: {apart} of {total} pixels differ"
    );
}

/// Two batches of one material each draw with their own uniforms (B1). A red
/// and a blue `damage_flash` quad must not look like two blue ones, which a
/// single UBO per material drew: every batch's write landed before the frame
/// ran. Each quad covers 64x64 pixels of the capture.
#[test]
fn material_batches_keep_their_own_uniforms() {
    if std::env::var("TUNGSTEN_VISUAL_REGRESSION").is_err() {
        return;
    }

    let root = workspace_root();
    let pair = capture(
        "material-pair",
        "empty",
        &root,
        &[("TUNGSTEN_MATERIAL_PAIR_FIXTURE", "pair")],
    );
    let same = capture(
        "material-same",
        "empty",
        &root,
        &[("TUNGSTEN_MATERIAL_PAIR_FIXTURE", "same")],
    );
    let (apart, total) = pixels_apart(&pair, &same);
    assert!(
        apart > 64 * 64 / 2,
        "the first quad's red is lost: {apart} of {total} pixels differ"
    );
}

/// The bullet trail's mesh particles reach the frame (M31, `D-093`). With no
/// post pass, the frame with the trail must differ from the frame without it
/// by more than a stray pixel: the trail draws instanced triangles over the
/// first bouncer.
#[test]
fn mesh_trail_draws_instanced_triangles() {
    if std::env::var("TUNGSTEN_VISUAL_REGRESSION").is_err() {
        return;
    }

    let root = workspace_root();
    let with_trail = capture("mesh-trail-on", "empty", &root, &[]);
    let without_trail = capture(
        "mesh-trail-off",
        "empty",
        &root,
        &[("TUNGSTEN_MESH_TRAIL_FIXTURE", "off")],
    );
    let (apart, total) = pixels_apart(&with_trail, &without_trail);
    assert!(
        apart > 100,
        "the mesh trail did not draw: {apart} of {total} pixels differ"
    );
}

/// The five effects that may end a fused pair, by kind.
const MIXES: [&str; 5] = ["vignette", "fade", "wipe_radial", "dissolve", "fog"];

/// What the renderer logs when it builds a fused pass.
const FUSED_LINE: &str = "post: the fused pass";

/// The stacks of [`fused_stacks`] whose last two stages never draw in one
/// pass: the last is no `mix`.
const ALONE: [&str; 6] = [
    "dreamy",
    "retro_arcade",
    "glitch_boss",
    "all",
    "pair:vignette:film_grain",
    "pair:vignette:duotone",
];

/// The stacks the fused-against-alone test draws: each `mix` behind a stage
/// of each class, the named stacks whose last two stages fuse or do not,
/// and two pairs that must never fuse.
fn fused_stacks() -> Vec<String> {
    let mut stacks: Vec<String> = Vec::new();
    for first in ["bloom", "vignette", "film_grain", "tonemap", "crt"] {
        for mix in MIXES {
            stacks.push(format!("pair:{first}:{mix}"));
        }
    }
    for named in [
        "bloom_vignette_fade",
        "fog_fade",
        // Bloom's composite leading a pair at a slot past the first.
        "fog_bloom_vignette",
        "dreamy",
        "retro_arcade",
        "glitch_boss",
        "all",
        // A subtraction near black and a tone that falls as luma rises are
        // no `mix`: fused behind another stage they would pass one step.
        "pair:vignette:film_grain",
        "pair:vignette:duotone",
    ] {
        stacks.push(named.to_string());
    }
    stacks
}

/// A stack's last two stages drawn in one pass stay within one 8-bit step a
/// channel of the two passes (`D-307`). The scene draws a ramp of every
/// channel from 0 to 1 and no text, so the capture is the stack's output,
/// dark values included, with no overlay blended over it.
#[test]
fn a_fused_pair_stays_within_one_step_of_its_passes() {
    if std::env::var("TUNGSTEN_VISUAL_REGRESSION").is_err() {
        return;
    }

    let root = workspace_root();
    let draw = |stack: &str, fuse: &str| {
        let name = format!("fused-{fuse}-{}", stack.replace(':', "-"));
        capture_logged(
            &name,
            stack,
            &root,
            &[
                ("TUNGSTEN_POST_BACKDROP_FIXTURE", "gradient"),
                ("TUNGSTEN_POST_TEXT_FIXTURE", "off"),
                ("TUNGSTEN_RENDER_POST_FUSE", fuse),
            ],
        )
    };
    let mut differing = Vec::new();
    for stack in fused_stacks() {
        let (fused, fused_log) = draw(&stack, "true");
        let (alone, alone_log) = draw(&stack, "false");
        // The pass was built where the rule joins the pair, and nowhere
        // else: a pair that silently drew alone would pass every pixel
        // check below.
        assert_eq!(
            fused_log.contains(FUSED_LINE),
            !ALONE.contains(&stack.as_str()),
            "{stack}:\n{fused_log}"
        );
        assert!(
            !alone_log.contains(FUSED_LINE),
            "{stack}, with fusing off:\n{alone_log}"
        );
        let report = compare_png(&fused, &alone, 1).expect("compare the two captures");
        assert_eq!(
            report.pixels_above_tolerance, 0,
            "{stack}: {} pixels more than one step from the stages drawn alone, the furthest {}",
            report.pixels_above_tolerance, report.max_delta
        );
        if report.max_delta > 0 {
            differing.push(stack);
        }
    }
    // A stack that fuses loses one rounding, so some pixel of some pair
    // moves a step: with none the test would not be seeing fused passes.
    assert!(!differing.is_empty(), "no stack drew differently fused");
    // The stacks whose last two stages do not fuse draw the same bytes.
    for alone in ALONE {
        assert!(
            !differing.iter().any(|stack| stack == alone),
            "{alone} must not fuse"
        );
    }
}

/// A game's own post effects draw (`D-308`): the fixtures' manifest declares
/// `tint`, whose `mix` section fuses behind a vignette, and `wave`, which
/// marks no section. Each changes the frame; the fused `tint` stays within
/// one step of its own pass, and `wave` draws the same bytes either way.
#[test]
fn a_games_effect_draws() {
    if std::env::var("TUNGSTEN_VISUAL_REGRESSION").is_err() {
        return;
    }

    let root = workspace_root();
    let draw_logged = |stack: &str, fuse: &str| {
        let name = format!("game-{fuse}-{}", stack.replace(':', "-"));
        capture_logged(
            &name,
            stack,
            &root,
            &[
                ("TUNGSTEN_POST_BACKDROP_FIXTURE", "gradient"),
                ("TUNGSTEN_POST_TEXT_FIXTURE", "off"),
                ("TUNGSTEN_RENDER_POST_FUSE", fuse),
            ],
        )
    };
    let draw = |stack: &str, fuse: &str| draw_logged(stack, fuse).0;
    let plain = draw("empty", "true");

    // `wave` shifts rows sideways: many pixels move.
    let wave = draw("game_wave", "true");
    let (moved, total) = pixels_apart(&plain, &wave);
    assert!(
        moved * 50 > total,
        "the wave drew nothing: {moved} of {total} pixels differ"
    );
    // A module without a section draws alone, a fade behind it too, though
    // a fade ends a fused pair behind any section.
    let (waved, waved_log) = draw_logged("game_wave_fade", "true");
    assert!(
        !waved_log.contains(FUSED_LINE),
        "a module without a section fused:\n{waved_log}"
    );
    let report =
        compare_png(&waved, &draw("game_wave_fade", "false"), 0).expect("compare the two captures");
    assert_eq!(
        report.pixels_above_tolerance, 0,
        "a module without a section draws alone either way"
    );

    // `tint` darkens green and blue over the whole frame, in the vignette's
    // pass when the stack fuses.
    let (fused, fused_log) = draw_logged("game_tint", "true");
    let (alone, alone_log) = draw_logged("game_tint", "false");
    assert!(
        fused_log.contains(FUSED_LINE),
        "the tint's section did not fuse behind the vignette:\n{fused_log}"
    );
    assert!(
        !alone_log.contains(FUSED_LINE),
        "the tint fused with fusing off:\n{alone_log}"
    );
    let (tinted, total) = pixels_apart(&plain, &fused);
    assert!(
        tinted * 2 > total,
        "the tint drew nothing: {tinted} of {total} pixels differ"
    );
    let report = compare_png(&fused, &alone, 1).expect("compare the two captures");
    assert_eq!(
        report.pixels_above_tolerance, 0,
        "the fused tint is {} pixels more than one step from its own pass, the furthest {}",
        report.pixels_above_tolerance, report.max_delta
    );
    assert!(
        report.max_delta > 0,
        "the tint did not fuse behind the vignette"
    );
}

/// A game's effect whose module cannot draw as a post pass is left out of the
/// frame, which draws the rest of its stack, and the log says so (`D-308`).
/// Two modules that pass Naga: one reads a block of 272 bytes where a pass
/// binds 256, which would build and then fail its draw; one has no `fs_main`,
/// which fails its pipeline inside the validation scope.
#[cfg(unix)]
#[test]
fn a_games_effect_that_cannot_draw_is_left_out() {
    if std::env::var("TUNGSTEN_VISUAL_REGRESSION").is_err() {
        return;
    }

    let root = workspace_root();
    let env = [
        ("TUNGSTEN_POST_BACKDROP_FIXTURE", "gradient"),
        ("TUNGSTEN_POST_TEXT_FIXTURE", "off"),
    ];
    let without = capture("game-none", "game_none", &root, &env);
    let with = capture("game-tint-whole", "game_tint", &root, &env);
    let report = compare_png(&with, &without, 0).expect("compare the two captures");
    assert!(
        report.pixels_above_tolerance > 0,
        "the tint draws nothing, so leaving it out shows nothing"
    );

    let edits: [(&str, &str, &str, &str); 2] = [
        (
            "block",
            "    i: vec4<i32>,\n};",
            "    i: vec4<i32>,\n    more: array<vec4<f32>, 11>,\n};",
            "is 272 bytes",
        ),
        (
            "entry",
            "fn fs_main(",
            "fn fs_other(",
            "does not draw as a post pass",
        ),
    ];
    for (name, old, new, reason) in edits {
        let stage = stage_with_edited_file(
            "examples/03_fixtures/assets/shaders/game_tint.wgsl",
            |source| {
                assert_eq!(
                    source.matches(old).count(),
                    1,
                    "game_tint.wgsl no longer holds {old:?} once; pick another edit"
                );
                source.replace(old, new)
            },
        );
        let (left_out, log) =
            capture_logged(&format!("game-left-out-{name}"), "game_tint", &stage, &env);
        let _ = std::fs::remove_dir_all(&stage);
        assert!(
            log.contains(reason),
            "{name}: the load did not say {reason:?}:\n{log}"
        );
        assert!(
            log.contains("post effect 'tint' has no pipeline"),
            "{name}: the frame did not say it left the effect out:\n{log}"
        );
        let report = compare_png(&left_out, &without, 0).expect("compare the two captures");
        assert_eq!(
            report.pixels_above_tolerance, 0,
            "{name}: the stack without its effect is {} pixels from the vignette alone",
            report.pixels_above_tolerance
        );
    }
}

/// Under a float scene (`render.hdr`, `D-309`) the float rule fuses whole
/// runs, and the frame's last pass ends in the tail. With fusing off every
/// stage draws alone and the tail is a pass of its own: under `rgba16f`,
/// over the ramp with no text, the two frames stay within one 8-bit step,
/// a stack with a quantising stage in its middle among them. Under
/// `rg11b10` a stage drawn alone rounds to 6- and 5-bit mantissas a pass,
/// which a fused run does not: its counts are printed, a measurement and no
/// bound.
#[test]
fn hdr_fuses_within_one_step_under_rgba16f() {
    if std::env::var("TUNGSTEN_VISUAL_REGRESSION").is_err() {
        return;
    }

    let root = workspace_root();
    let draw = |stack: &str, hdr: &str, fuse: &str| {
        capture_logged(
            &format!("hdr-{hdr}-{fuse}-{stack}"),
            stack,
            &root,
            &[
                ("TUNGSTEN_POST_BACKDROP_FIXTURE", "gradient"),
                ("TUNGSTEN_POST_TEXT_FIXTURE", "off"),
                ("TUNGSTEN_RENDER_HDR", hdr),
                ("TUNGSTEN_RENDER_POST_FUSE", fuse),
            ],
        )
    };
    // The passes a run's log says were generated, the tail's own left out.
    let generated = |log: &str| -> usize {
        log.lines()
            .filter(|line| line.contains(FUSED_LINE) && !line.contains("the fused pass tail built"))
            .count()
    };
    for stack in [
        "bloom_vignette_fade",
        "dreamy",
        "retro_arcade",
        "all",
        "vignette_dither_fade",
    ] {
        let (fused, fused_log) = draw(stack, "rgba16f", "true");
        let (alone, alone_log) = draw(stack, "rgba16f", "false");
        // Fused, the stack's last pass at least is a generated one, with
        // the tail; alone, only the tail's own pass is.
        assert!(generated(&fused_log) > 0, "{stack}:\n{fused_log}");
        assert_eq!(generated(&alone_log), 0, "{stack}:\n{alone_log}");
        assert!(
            alone_log.contains("the fused pass tail built"),
            "{stack}:\n{alone_log}"
        );
        let report = compare_png(&fused, &alone, 1).expect("compare the two captures");
        assert_eq!(
            report.pixels_above_tolerance, 0,
            "{stack} under rgba16f: {} pixels more than one step from the stages drawn alone, the furthest {}",
            report.pixels_above_tolerance, report.max_delta
        );

        let (fused, _) = draw(stack, "rg11b10", "true");
        let (alone, _) = draw(stack, "rg11b10", "false");
        let report = compare_png(&fused, &alone, 1).expect("compare the two captures");
        println!(
            "hdr rg11b10 {stack}: {} of {} pixels more than one step between fused and alone, the furthest {}, mean {:.4}",
            report.pixels_above_tolerance,
            report.width * report.height,
            report.max_delta,
            report.mean_delta
        );
    }
}

/// A float scene keeps today's picture where nothing passes 1, and gives
/// bloom what does (`D-309`). With the mesh trail off, which adds light, the
/// scene under `rgba16f` stays within 3 steps of `off`, with no stack and
/// under bloom, and so do quads four times white without bloom: the tail
/// clamps them to the white `off` draws. Under bloom those quads glow far
/// past `off`'s, whose scene held them at 1.
#[test]
fn hdr_keeps_a_dim_scene_and_blooms_a_bright_one() {
    if std::env::var("TUNGSTEN_VISUAL_REGRESSION").is_err() {
        return;
    }

    let root = workspace_root();
    let draw = |stack: &str, hdr: &str, bright: &str| {
        capture(
            &format!("hdr-scene-{hdr}-{bright}-{stack}"),
            stack,
            &root,
            &[
                ("TUNGSTEN_POST_TEXT_FIXTURE", "off"),
                ("TUNGSTEN_MESH_TRAIL_FIXTURE", "off"),
                ("TUNGSTEN_POST_BRIGHT_FIXTURE", bright),
                ("TUNGSTEN_RENDER_HDR", hdr),
            ],
        )
    };
    for (stack, bright) in [("empty", "off"), ("bloom_only", "off"), ("empty", "on")] {
        let report = compare_png(
            &draw(stack, "off", bright),
            &draw(stack, "rgba16f", bright),
            3,
        )
        .expect("compare the two captures");
        assert_eq!(
            report.pixels_above_tolerance, 0,
            "{stack}, bright quads {bright}: {} pixels more than 3 steps from the 8-bit scene, the furthest {}",
            report.pixels_above_tolerance, report.max_delta
        );
    }
    // The quads under bloom: brighter around each by far more than a dim
    // scene moves.
    let report = compare_png(
        &draw("bloom_only", "off", "on"),
        &draw("bloom_only", "rgba16f", "on"),
        8,
    )
    .expect("compare the two captures");
    let total = report.width * report.height;
    assert!(
        report.pixels_above_tolerance * 5 > total && report.max_delta > 64,
        "values above 1 did not reach bloom: {} of {total} pixels more than 8 steps apart, the furthest {}",
        report.pixels_above_tolerance,
        report.max_delta
    );
}

/// A colour 8 bits hold comes through a float scene's tail as it is
/// (`D-309`): over quads of every 8-bit value in each channel, with no
/// stack, the frame under `rgba16f` is `off`'s byte for byte, the tail's
/// noise staying short of what the float store and the conversions round
/// by. `rg11b10` holds 6 and 5 bits of mantissa a channel, so there a
/// colour lands within 4 steps, and its counts are printed.
#[test]
fn hdr_keeps_the_colours_8_bits_hold() {
    if std::env::var("TUNGSTEN_VISUAL_REGRESSION").is_err() {
        return;
    }

    let root = workspace_root();
    let draw_on = |hdr: &str, direct: &str| {
        capture(
            &format!("hdr-codes-{hdr}-{direct}"),
            "empty",
            &root,
            &[
                ("TUNGSTEN_POST_TEXT_FIXTURE", "off"),
                ("TUNGSTEN_POST_BRIGHT_FIXTURE", "codes"),
                ("TUNGSTEN_RENDER_HDR", hdr),
                ("TUNGSTEN_CAPTURE_DIRECT", direct),
            ],
        )
    };
    let draw = |hdr: &str| draw_on(hdr, "0");
    let off = draw("off");
    // The scene is the 256 quads: sixteen columns of 80 pixels, each its
    // own colour, and no pixel of anything else.
    let plain = compare_png(&off, &capture("hdr-codes-none", "empty", &root, &[]), 0)
        .expect("compare the two captures");
    assert!(
        plain.pixels_above_tolerance * 10 > plain.width * plain.height * 9,
        "the code quads do not cover the view"
    );

    let report = compare_png(&off, &draw("rgba16f"), 0).expect("compare the two captures");
    assert_eq!(
        report.pixels_above_tolerance, 0,
        "rgba16f moved {} pixels of colours 8 bits hold, the furthest {} steps",
        report.pixels_above_tolerance, report.max_delta
    );
    // The direct path's frame, the tail drawn onto a stand-in for the
    // surface, is the capture path's.
    let report = compare_png(&draw("rgba16f"), &draw_on("rgba16f", "1"), 0)
        .expect("compare the two captures");
    assert_eq!(
        report.pixels_above_tolerance, 0,
        "the direct path's tail draws another frame"
    );
    let report = compare_png(&off, &draw("rg11b10"), 4).expect("compare the two captures");
    let moved = compare_png(&off, &draw("rg11b10"), 0).expect("compare the two captures");
    println!(
        "hdr rg11b10 codes: {} of {} pixels differ from off, the furthest {} steps",
        moved.pixels_above_tolerance,
        moved.width * moved.height,
        moved.max_delta
    );
    assert_eq!(
        report.pixels_above_tolerance, 0,
        "rg11b10 moved {} pixels more than 4 steps, the furthest {}",
        report.pixels_above_tolerance, report.max_delta
    );
}
