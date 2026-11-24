//! Workspace-wide WGSL coverage: every shader under the render crate's
//! sources, `assets/shaders/` and each example's `assets/shaders/` passes Naga
//! validation, a fragment-only material after the material prelude, and every
//! compiled-in shader with a manifest-tracked mirror matches it byte for byte
//! (`D-057`).
//!
//! Naga success is not GPU or pixel correctness; the smoke and visual checks
//! cover that.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use tungsten_render::shader_hot_reload::{MATERIAL_PRELUDE, module_source};
use tungsten_render::validate_wgsl_source;

const RENDER_SRC: &str = "crates/tungsten-render/src";
const ASSET_SHADERS: &str = "assets/shaders";
/// Example-local shaders live in `examples/<name>/assets/shaders`, when present.
const EXAMPLES: &str = "examples";

/// `(under RENDER_SRC, under ASSET_SHADERS)` subtrees or files kept byte-equal.
const MIRRORS: &[(&str, &str)] = &[("shaders/stock", "stock")];

/// Vendored LYGIA snippets are reference fragments, not compiled by the engine.
/// A fragment that calls into a sibling validates with that sibling prepended.
const FRAGMENT_DEPS: &[(&str, &[&str])] = &[("lygia/noise.wgsl", &["hash.wgsl"])];

#[derive(Debug)]
struct Inventory {
    paths: usize,
    distinct: usize,
    pairs: usize,
}

fn wgsl_under(root: &Path, dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = fs::read_dir(dir).unwrap_or_else(|e| panic!("read {}: {e}", dir.display()));
    for entry in entries {
        let path = entry.expect("read dir entry").path();
        if path.is_dir() {
            wgsl_under(root, &path, out);
        } else if path.extension().is_some_and(|ext| ext == "wgsl") {
            out.push(
                path.strip_prefix(root)
                    .expect("path under root")
                    .to_path_buf(),
            );
        }
    }
}

/// A repo-relative path as the checker's messages show it: `/`-separated on
/// every platform, where `Path::display` would print the separators Windows
/// joined with as `\`.
fn shown(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// Map of path-below-`prefix` → repo-relative path for one side of a mirror.
fn mirror_side(files: &[PathBuf], prefix: &Path) -> BTreeMap<PathBuf, PathBuf> {
    files
        .iter()
        .filter_map(|f| Some((f.strip_prefix(prefix).ok()?.to_path_buf(), f.clone())))
        .collect()
}

fn check(repo: &Path) -> Result<Inventory, Vec<String>> {
    let mut errors = Vec::new();
    let mut files = Vec::new();
    for tree in [RENDER_SRC, ASSET_SHADERS] {
        let before = files.len();
        wgsl_under(repo, &repo.join(tree), &mut files);
        if files.len() == before {
            errors.push(format!("no .wgsl files found under {tree}"));
        }
    }
    if let Ok(entries) = fs::read_dir(repo.join(EXAMPLES)) {
        for entry in entries {
            let shaders = entry.expect("read dir entry").path().join(ASSET_SHADERS);
            if shaders.is_dir() {
                wgsl_under(repo, &shaders, &mut files);
            }
        }
    }
    files.sort();

    let mut contents = BTreeMap::new();
    for rel in &files {
        let text = fs::read_to_string(repo.join(rel))
            .unwrap_or_else(|e| panic!("read {}: {e}", rel.display()));
        let mut source = String::new();
        for (fragment, deps) in FRAGMENT_DEPS {
            if rel.ends_with(fragment) {
                for dep in *deps {
                    let dep_path = repo.join(rel).with_file_name(dep);
                    source += &fs::read_to_string(&dep_path)
                        .unwrap_or_else(|e| panic!("read {}: {e}", dep_path.display()));
                }
            }
        }
        source += &module_source(&text);
        if let Err(e) = validate_wgsl_source(&shown(rel), &source) {
            errors.push(e.to_string());
        }
        contents.insert(rel.clone(), text);
    }
    let distinct = contents.values().collect::<BTreeSet<_>>().len();

    let mut pairs = 0;
    for (engine, asset) in MIRRORS {
        let engine_side = mirror_side(&files, &Path::new(RENDER_SRC).join(engine));
        let asset_side = mirror_side(&files, &Path::new(ASSET_SHADERS).join(asset));
        if engine_side.is_empty() && asset_side.is_empty() {
            errors.push(format!("mirror set {engine} <-> {asset} is empty"));
        }
        for (key, engine_path) in &engine_side {
            match asset_side.get(key) {
                None => errors.push(format!("{} has no asset mirror", shown(engine_path))),
                Some(asset_path) if contents[engine_path] != contents[asset_path] => {
                    errors.push(format!(
                        "{} and {} differ",
                        shown(engine_path),
                        shown(asset_path)
                    ));
                }
                Some(_) => pairs += 1,
            }
        }
        for (key, asset_path) in &asset_side {
            if !engine_side.contains_key(key) {
                errors.push(format!("{} has no engine copy", shown(asset_path)));
            }
        }
    }

    if errors.is_empty() {
        Ok(Inventory {
            paths: files.len(),
            distinct,
            pairs,
        })
    } else {
        Err(errors)
    }
}

#[test]
fn every_workspace_shader_validates_and_mirrors_match() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    match check(&repo) {
        Ok(inv) => eprintln!(
            "shader coverage: {} paths, {} distinct contents, {} mirror pairs",
            inv.paths, inv.distinct, inv.pairs
        ),
        Err(errors) => panic!("shader coverage failed:\n  {}", errors.join("\n  ")),
    }
}

/// The compiled-in stock files that are not full-screen passes: the stock
/// materials and the lit helpers, drawn by sprite batches.
const NOT_FULL_SCREEN: &[&str] = &[
    "damage_flash.wgsl",
    "emissive_mask.wgsl",
    "glow.wgsl",
    "rim_light.wgsl",
];

/// Whether `block` or a block nested in it holds a `discard`.
fn discards(block: &wgpu::naga::Block) -> bool {
    use wgpu::naga::Statement;
    block.iter().any(|statement| match statement {
        Statement::Kill => true,
        Statement::Block(inner) => discards(inner),
        Statement::If { accept, reject, .. } => discards(accept) || discards(reject),
        Statement::Switch { cases, .. } => cases.iter().any(|case| discards(&case.body)),
        Statement::Loop {
            body, continuing, ..
        } => discards(body) || discards(continuing),
        _ => false,
    })
}

/// The names of the functions of `wgsl` that can discard a fragment: its
/// entry points and every function they could call.
fn discarding_functions(name: &str, wgsl: &str) -> Vec<String> {
    let module = wgpu::naga::front::wgsl::parse_str(wgsl)
        .unwrap_or_else(|error| panic!("{name}: {}", error.emit_to_string(wgsl)));
    let mut found: Vec<String> = module
        .functions
        .iter()
        .filter(|(_, function)| discards(&function.body))
        .map(|(_, function)| function.name.clone().unwrap_or_default())
        .collect();
    found.extend(
        module
            .entry_points
            .iter()
            .filter(|entry| discards(&entry.function.body))
            .map(|entry| entry.name.clone()),
    );
    found
}

/// The engine's full-screen shader files under the render crate's sources,
/// as repo-relative paths: the stock post effects, bloom's four stages,
/// SMAA's three and the present blit.
fn full_screen_files(repo: &Path) -> Vec<PathBuf> {
    let stock = Path::new(RENDER_SRC).join("shaders/stock");
    let mut files: Vec<PathBuf> = fs::read_dir(repo.join(&stock))
        .unwrap_or_else(|e| panic!("read {}: {e}", shown(&stock)))
        .map(|entry| entry.expect("read dir entry").file_name())
        .filter(|name| Path::new(name).extension().is_some_and(|ext| ext == "wgsl"))
        .filter(|name| {
            !name
                .to_str()
                .is_some_and(|name| NOT_FULL_SCREEN.contains(&name))
        })
        .map(|name| stock.join(name))
        .collect();
    files.push(Path::new(RENDER_SRC).join("shaders/present_blit.wgsl"));
    files.sort();
    files
}

/// The frame's graph takes a full-screen pass for one that writes every
/// pixel, and loads the surface with `DontCare` under it (`D-306`). That
/// holds while no fragment of these files discards and each pipeline that
/// draws them blends nothing and writes every channel.
#[test]
fn the_engines_fullscreen_fragments_write_every_pixel() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let files = full_screen_files(&repo);
    // 17 stock post effects, 4 bloom stages, 3 SMAA stages, the blit: a new
    // file is classed here, as full-screen or in `NOT_FULL_SCREEN`.
    assert_eq!(files.len(), 25, "{files:?}");
    for path in &files {
        let wgsl =
            fs::read_to_string(repo.join(path)).unwrap_or_else(|e| panic!("{}: {e}", shown(path)));
        assert_eq!(
            discarding_functions(&shown(path), &wgsl),
            Vec::<String>::new(),
            "{} discards: its pass no longer writes every pixel",
            shown(path)
        );
    }

    // The pipelines: every builder of a full-screen pipeline takes its
    // colour target from `whole_target`, which blends nothing and writes
    // every channel. The stock effects' builder still spells the same state
    // out, the one `ColorTargetState` of its file.
    let source = |rel: &str| {
        let path = repo.join(RENDER_SRC).join(rel);
        fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", shown(&path)))
    };
    let helper = source("post/mod.rs");
    let body = helper
        .split("pub(crate) fn whole_target(")
        .nth(1)
        .expect("post/mod.rs defines whole_target");
    let body = &body[..body.find("\n}\n").expect("the function ends")];
    assert!(body.contains("blend: None,"), "{body}");
    assert!(
        body.contains("write_mask: wgpu::ColorWrites::ALL,"),
        "{body}"
    );
    // (file, its calls of `whole_target`, its own `ColorTargetState`s, its
    // pipeline builders): bloom's threshold and downsample share one
    // builder and the additive upsample keeps a state of its own; SMAA's
    // three stages; the blit. Each builder culls nothing and tests no
    // depth, so the one triangle is drawn whichever way it winds.
    for (rel, calls, own, builders) in [
        ("post/bloom.rs", 2, 1, 2),
        ("post/smaa.rs", 3, 0, 3),
        ("renderer.rs", 1, 0, 1),
        ("post/fullscreen.rs", 0, 1, 1),
    ] {
        let text = source(rel);
        assert_eq!(text.matches("whole_target(").count(), calls, "{rel}");
        assert_eq!(text.matches("ColorTargetState {").count(), own, "{rel}");
        assert_eq!(
            text.matches("create_render_pipeline(").count(),
            builders,
            "{rel}"
        );
        assert_eq!(text.matches("cull_mode: None,").count(), builders, "{rel}");
        assert_eq!(
            text.matches("depth_stencil: None,").count(),
            builders,
            "{rel}"
        );
    }
    let stock = source("post/fullscreen.rs");
    assert!(
        stock.contains("blend: None,\n                write_mask: wgpu::ColorWrites::ALL,"),
        "the stock effects' pipeline blends nothing and writes every channel"
    );
}

#[test]
fn a_discard_in_any_branch_is_found() {
    let nested = "@fragment\nfn fs_main(@builtin(position) at: vec4<f32>) -> @location(0) vec4<f32> {\n    \
        if at.x > 4.0 {\n        loop {\n            if at.y > 2.0 {\n                discard;\n            }\n            \
        break;\n        }\n    }\n    return vec4<f32>(1.0);\n}\n";
    assert_eq!(discarding_functions("nested", nested), ["fs_main"]);
    let helper = "fn cut(x: f32) {\n    if x < 0.5 {\n        discard;\n    }\n}\n\
        @fragment\nfn fs_main(@builtin(position) at: vec4<f32>) -> @location(0) vec4<f32> {\n    \
        cut(at.x);\n    return vec4<f32>(1.0);\n}\n";
    assert_eq!(discarding_functions("helper", helper), ["cut"]);
    assert!(discarding_functions("valid", VALID).is_empty());
}

// Negative cases run against a disposable tree so the checker itself stays honest.

const VALID: &str =
    "@fragment\nfn fs_main() -> @location(0) vec4<f32> {\n    return vec4<f32>(1.0);\n}\n";

fn fixture(name: &str, files: &[(&str, &str)]) -> PathBuf {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("shader_coverage")
        .join(name);
    if root.exists() {
        fs::remove_dir_all(&root).expect("reset fixture");
    }
    fs::create_dir_all(root.join(RENDER_SRC)).expect("create render src");
    fs::create_dir_all(root.join(ASSET_SHADERS)).expect("create asset shaders");
    for (rel, text) in files {
        let path = root.join(rel);
        fs::create_dir_all(path.parent().expect("parent")).expect("create parent");
        fs::write(path, text).expect("write fixture");
    }
    root
}

fn baseline() -> Vec<(&'static str, &'static str)> {
    vec![
        ("crates/tungsten-render/src/shaders/stock/fade.wgsl", VALID),
        ("assets/shaders/stock/fade.wgsl", VALID),
    ]
}

fn expect_error(name: &str, files: &[(&str, &str)], needle: &str) {
    let errors = check(&fixture(name, files)).expect_err("fixture should fail");
    assert!(
        errors.iter().any(|e| e.contains(needle)),
        "expected an error containing {needle:?}, got {errors:?}"
    );
}

#[test]
fn checker_accepts_valid_fixture() {
    let inv = check(&fixture("valid", &baseline())).expect("valid fixture");
    assert_eq!((inv.paths, inv.distinct, inv.pairs), (2, 1, 1));
}

#[test]
fn checker_rejects_malformed_wgsl() {
    let mut files = baseline();
    files.push(("assets/shaders/broken.wgsl", "fn broken( {"));
    expect_error("malformed", &files, "broken.wgsl");
}

#[test]
fn checker_validates_example_shaders() {
    let mut files = baseline();
    files.push(("examples/demo/assets/shaders/broken.wgsl", "fn broken( {"));
    expect_error(
        "example",
        &files,
        "examples/demo/assets/shaders/broken.wgsl",
    );
}

#[test]
fn checker_rejects_missing_mirrors_in_both_directions() {
    let mut files = baseline();
    files.push(("assets/shaders/stock/orphan.wgsl", VALID));
    expect_error(
        "missing-engine",
        &files,
        "stock/orphan.wgsl has no engine copy",
    );

    let mut files = baseline();
    files.push((
        "crates/tungsten-render/src/shaders/stock/orphan.wgsl",
        VALID,
    ));
    expect_error(
        "missing-asset",
        &files,
        "stock/orphan.wgsl has no asset mirror",
    );
}

#[test]
fn checker_rejects_mismatched_mirror() {
    let mut files = baseline();
    files[1] = (
        "assets/shaders/stock/fade.wgsl",
        "// drift\n@fragment\nfn fs_main() -> @location(0) vec4<f32> {\n    return vec4<f32>(0.0);\n}\n",
    );
    expect_error("mismatch", &files, "differ");
}

#[test]
fn messages_show_windows_paths_with_forward_slashes() {
    assert_eq!(
        shown(Path::new(r"examples\demo\assets/shaders\broken.wgsl")),
        "examples/demo/assets/shaders/broken.wgsl"
    );
    assert_eq!(
        shown(Path::new("assets/shaders/stock/orphan.wgsl")),
        "assets/shaders/stock/orphan.wgsl"
    );
}

#[test]
fn checker_rejects_empty_discovery() {
    let files = [("crates/tungsten-render/src/quad.wgsl", VALID)];
    expect_error("empty", &files, "no .wgsl files found under assets/shaders");
}

#[test]
fn the_prelude_validates_alone_and_before_a_fragment() {
    validate_wgsl_source("material_prelude.wgsl", MATERIAL_PRELUDE).unwrap();
    let fragment = "@fragment\nfn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {\n    \
        return textureSample(sprite_tex, sprite_sampler, in.uv) * in.color * material.v0;\n}\n";
    // Alone it names what only the prelude declares; after it, it validates.
    assert!(validate_wgsl_source("fragment", fragment).is_err());
    let compiled = module_source(fragment);
    assert!(compiled.starts_with(MATERIAL_PRELUDE));
    validate_wgsl_source("fragment", &compiled).unwrap();
    // A whole module, and a helper with no entry point, compile as written.
    let whole = "@vertex fn vs_main() -> @builtin(position) vec4<f32> { return vec4<f32>(0.0); }\n\
        @fragment fn fs_main() -> @location(0) vec4<f32> { return vec4<f32>(1.0); }\n";
    assert_eq!(module_source(whole), whole);
    let helper = "fn half(x: f32) -> f32 { return x * 0.5; }\n";
    assert_eq!(module_source(helper), helper);
}
