//! Workspace-wide WGSL coverage: every shader under the render crate's
//! sources and `assets/shaders/` passes Naga validation, and every compiled-in
//! shader with a manifest-tracked mirror matches it byte for byte (`D-057`).
//!
//! Naga success is not GPU or pixel correctness; the smoke and visual checks
//! cover that.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use tungsten_render::validate_wgsl_source;

const RENDER_SRC: &str = "crates/tungsten-render/src";
const ASSET_SHADERS: &str = "assets/shaders";

/// `(under RENDER_SRC, under ASSET_SHADERS)` subtrees or files kept byte-equal.
const MIRRORS: &[(&str, &str)] = &[("shaders/stock", "stock"), ("sprite.wgsl", "sprite.wgsl")];

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
        source += &text;
        if let Err(e) = validate_wgsl_source(&rel.display().to_string(), &source) {
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
                None => errors.push(format!("{} has no asset mirror", engine_path.display())),
                Some(asset_path) if contents[engine_path] != contents[asset_path] => {
                    errors.push(format!(
                        "{} and {} differ",
                        engine_path.display(),
                        asset_path.display()
                    ));
                }
                Some(_) => pairs += 1,
            }
        }
        for (key, asset_path) in &asset_side {
            if !engine_side.contains_key(key) {
                errors.push(format!("{} has no engine copy", asset_path.display()));
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
        ("crates/tungsten-render/src/sprite.wgsl", VALID),
        ("assets/shaders/sprite.wgsl", VALID),
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
    assert_eq!((inv.paths, inv.distinct, inv.pairs), (4, 1, 2));
}

#[test]
fn checker_rejects_malformed_wgsl() {
    let mut files = baseline();
    files.push(("assets/shaders/broken.wgsl", "fn broken( {"));
    expect_error("malformed", &files, "broken.wgsl");
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
    files[3] = (
        "assets/shaders/stock/fade.wgsl",
        "// drift\n@fragment\nfn fs_main() -> @location(0) vec4<f32> {\n    return vec4<f32>(0.0);\n}\n",
    );
    expect_error("mismatch", &files, "differ");
}

#[test]
fn checker_rejects_empty_discovery() {
    let files = [("crates/tungsten-render/src/quad.wgsl", VALID)];
    expect_error("empty", &files, "no .wgsl files found under assets/shaders");
}
