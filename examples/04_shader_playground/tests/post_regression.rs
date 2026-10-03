//! Opt-in GPU regressions for the stock post stack, on the playground's
//! fixtures.
//!
//! Gate: `TUNGSTEN_VISUAL_REGRESSION=1` (needs GPU/display). Each test compares
//! two captures of this machine with each other, so there is no fixture image.
//! Capture: 8 smoke frames, frame 5 at 1280x720.

use std::path::{Path, PathBuf};

use tungsten::render::compare_png;

/// Runs the playground in `root` with `post_stack` preloaded and captures
/// frame 5 into a file named after `name`. `extra_env` is set last.
fn capture(name: &str, post_stack: &str, root: &Path, extra_env: &[(&str, &str)]) -> PathBuf {
    let actual = std::env::temp_dir().join(format!("tungsten-post-regression-{name}.png"));
    if actual.exists() {
        let _ = std::fs::remove_file(&actual);
    }

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_example-04-shader-playground"))
        .current_dir(root)
        .env("TUNGSTEN_POST_STACK_FIXTURE", post_stack)
        .env("TUNGSTEN_POST_AA_FIXTURE", "off")
        .env_remove("TUNGSTEN_BLOOM_FIXTURE")
        .env_remove("TUNGSTEN_GAME_FEEL_FIXTURE")
        .env_remove("TUNGSTEN_CAPTURE_DIRECT")
        .env_remove("TUNGSTEN_MESH_TRAIL_FIXTURE")
        .env_remove("TUNGSTEN_MATERIAL_PAIR_FIXTURE")
        .env("TUNGSTEN_SMOKE_FRAMES", "8")
        .env("TUNGSTEN_CAPTURE_FRAME", "5")
        .env("TUNGSTEN_CAPTURE_RESOLUTION", "1280x720")
        .env("TUNGSTEN_CAPTURE_PATH", &actual)
        .envs(extra_env.iter().copied())
        .status()
        .expect("run example-04-shader-playground");
    assert!(
        status.success(),
        "example-04-shader-playground exited with {status:?}"
    );
    assert!(
        actual.exists(),
        "capture did not produce {}",
        actual.display()
    );
    actual
}

/// The playground loads its manifests relative to the workspace root.
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

/// A directory the playground can run in: everything it reads is a link into
/// the workspace, except the stock shader `name`, whose source went through
/// `edit`.
#[cfg(unix)]
fn stage_with_edited_stock_shader(name: &str, edit: impl Fn(&str) -> String) -> PathBuf {
    use std::os::unix::fs::symlink;

    let root = workspace_root()
        .canonicalize()
        .expect("canonical workspace root");
    // Tests run in parallel and two may stage the same shader: one directory
    // per call.
    static STAGES: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let stage = std::env::temp_dir().join(format!(
        "tungsten-post-regression-stage-{name}-{}-{}",
        std::process::id(),
        STAGES.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&stage);

    let example_assets = Path::new("examples/04_shader_playground/assets");
    std::fs::create_dir_all(stage.join(example_assets).parent().unwrap())
        .expect("create the staged example directory");
    symlink(root.join(example_assets), stage.join(example_assets))
        .expect("link the example assets");
    for config in ["tungsten.json", "input.json"] {
        symlink(root.join(config), stage.join(config)).expect("link a config file");
    }

    let file = format!("{name}.wgsl");
    mirror(&root.join("assets"), &stage.join("assets"), "shaders");
    mirror(
        &root.join("assets/shaders"),
        &stage.join("assets/shaders"),
        "stock",
    );
    mirror(
        &root.join("assets/shaders/stock"),
        &stage.join("assets/shaders/stock"),
        &file,
    );
    let source = std::fs::read_to_string(root.join("assets/shaders/stock").join(&file))
        .expect("read the stock shader");
    std::fs::write(
        stage.join("assets/shaders/stock").join(&file),
        edit(&source),
    )
    .expect("write the edited stock shader");
    stage
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

    let root = workspace_root();
    let pair = capture("fade-pair", "fade_pair", &root, &[]);
    let twice = capture("fade-twice", "fade_twice", &root, &[]);
    let (apart, total) = pixels_apart(&pair, &twice);
    assert!(
        apart * 2 > total,
        "the first fade's color is lost: {apart} of {total} pixels differ"
    );
}

/// A body edit of a stock post shader reaches its pipeline (`D-091`). The
/// playground started on a tree whose `fade.wgsl` swaps red and blue must not
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
            source.contains("params.v0.rgb"),
            "fade.wgsl no longer reads params.v0.rgb; pick another edit"
        );
        source.replace("params.v0.rgb", "params.v0.bgr")
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
/// shipped pipeline (B2, `D-057`). The playground started on a tree whose
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
        let read = "let p = clamp(params.f.x, 0.0, 1.0);";
        assert!(
            source.contains(params) && source.contains(read),
            "fade.wgsl changed; pick another incompatible edit"
        );
        source
            .replace(
                params,
                &format!("{params}\n@group(2) @binding(0) var<uniform> extra: vec4<f32>;"),
            )
            .replace(read, "let p = clamp(params.f.x + extra.x * 0.0, 0.0, 1.0);")
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
