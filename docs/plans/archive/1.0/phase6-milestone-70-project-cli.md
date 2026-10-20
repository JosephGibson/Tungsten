# Phase 6 milestone 70: the project command line (W14b) — draft

- **status:** done
- **goal:** A `tungsten` command line for a game project, the `tungsten-cli` package in `tools/cli/`: `new` starts a game from `templates/basic` with the engine pinned to the release tag, `check` loads every file a game reads with the engine's own loaders, inside the game's own build, and reports every error with its file, and `package` builds a Linux release archive with the licence notices and the debug file kept apart. The outside check runs on `new`'s output with both, so RC-A7 runs at every release.
- **non-goals:** Packaging on Windows or macOS hosts (zip, PDB, dSYM) and CPU levels with a launcher for games; a check API for a game's tests; checking shader code, font files or IDs written in Rust; `tungsten run --script` ([backlog](../../1.0/backlog-1.x.md)); moving `release.py`'s notices onto the CLI; publishing the CLI to crates.io (criteria Q2); a manifest section for scenes; JSON schemas for config files ([w14](../../1.0/w14-tooling.md) non-goal); any change to the frame body.
- **files to touch:** Step 1: `Cargo.toml`, `Cargo.lock`, `tools/cli/Cargo.toml`, `tools/cli/src/main.rs`, `tools/cli/src/args.rs`, `tools/cli/src/tests/args.rs`, `tools/cli/tests/cli.rs`, `crates/tungsten-core/src/assets/manifest.rs`, `crates/tungsten-core/src/assets/mod.rs`, `crates/tungsten-core/src/tests/assets/manifest.rs`, `crates/tungsten-core/src/assets/data.rs`, `crates/tungsten-core/src/tests/assets/data.rs`, `scripts/public-api.sh`, `api/tungsten-core.txt`, `api/tungsten-cli.txt`, `templates/basic/.gitignore`, `templates/basic/AGENTS.md`, `templates/basic/README.md`, `DECISIONS.md`, `docs/DECISION_INDEX.md`, `docs/plans/1.0/w04-api-freeze.md`. Step 2: `crates/tungsten/src/asset_loader/check.rs`, `crates/tungsten/src/asset_loader/mod.rs`, `crates/tungsten/src/asset_loader/atlas.rs`, `crates/tungsten/src/tests/check.rs`, `crates/tungsten/src/app.rs`, `crates/tungsten/src/tests/app.rs`, `Cargo.toml`, `Cargo.lock`, `tools/cli/Cargo.toml`, `tools/cli/src/main.rs`, `tools/cli/src/check.rs`, `tools/cli/src/cargo.rs`, `tools/cli/src/tests/check.rs`. Step 3: `tools/cli/build.rs`, `tools/cli/src/main.rs`, `tools/cli/src/package.rs`, `tools/cli/src/notices.rs`, `tools/cli/src/elf.rs`, `tools/cli/src/tests/package.rs`, `tools/cli/src/tests/notices.rs`, `tools/cli/src/tests/elf.rs`. Step 4: `tools/cli/build.rs`, `tools/cli/src/main.rs`, `tools/cli/src/new.rs`, `tools/cli/src/tests/new.rs`, `tools/cli/tests/cli.rs`. Step 5: `scripts/template-check.sh`, `justfile`, `scripts/check-agent-context.py`, `scripts/check-repo.py`, `scripts/test-check-repo.py`, `docs/getting-started.md`, `docs/validation.md`, `docs/releases.md`, `docs/LLM_INDEX.md`. Step 6: `DESIGN.md`, `docs/known-issues.md`, `docs/plans/1.0/backlog-1.x.md`, `docs/plans/1.0/w14-tooling.md`, `docs/plans/1.0/criteria.md`, `docs/plans/1.0/roadmap.json`. Step 7: `CHANGELOG.md`, `DESIGN.md`, `Cargo.toml`, `Cargo.lock`, the register row in `docs/plans/1.0/implementation-plan.md`, the README's four owned Now fields, this plan (moved to `docs/plans/archive/1.0/`).
- **ordered steps:** (1) The API: the CLI's commands and flags, the check's two core functions, the template's text and both decisions; then the step 1 API critique. (2) `check`. (3) `package`. (4) `new`. (5) The outside check on `new`'s output, the template's `just ctx` budget, the guide and the routes. (6) Records. (7) Release 0.90.
- **done-when:** every step's evidence row quoted; workflow §5 closed (§8 here); the release checks, plus `just deps` and `just udeps` from the steps that change dependencies, pass after the cut; the preflight prints the command block and the post-merge block.

Candidate W14b ([implementation plan](../../1.0/implementation-plan.md) §4 card, level C), milestone M70, release 0.90. Written 2026-10-19 at `301f95c`. Runs under the [tungsten-milestone skill](../../../../.claude/skills/tungsten-milestone/SKILL.md) and [workflow](../../1.0/workflow.md) §4–§6; this plan records only where it differs. Scoping source: [w14](../../1.0/w14-tooling.md), W14b's step and follow-ups 1 and 2.

## Context digest

- **Why now.** W14b is the first open stop. What it needs has landed: W12a's template and outside-copy check and W9a's notices (M36), W11a's symbols (M35), W16b's prefabs (M51), W13f's two-phase load (M61) and M69's template shell. The acceptance game ships with `package` (RC-A2), and RC-A7 needs `new`, `check` and `package` together.
- **Promise.** `D-103` puts the CLI's commands and flags inside 1.0's semver promise. Level C, so step 1 lands that surface and a critique reads it before the rest runs.
- **Q20** (criteria §10) is open. This plan takes its default: `check` and `package` Must, `new` Should (`D-150`). Argument parsing by hand and archives through the platform's `tar` (§4 Q2, Q3).
- **The hard part** (w14 follow-up 2, from `D-153`): `check` needs the game's registered components and data kinds, which only the game's code holds. Here the content check runs inside the game's own build. `tungsten check` runs `cargo run` with `TUNGSTEN_CHECK=<report>`, and `App::run`, called after every plugin has built, then loads every file without a window, keeping every error.
- **Every loader stops at its first error** (§1 A6–A10). Two core functions gain a reporting form; the per-file loaders are reused as they are.
- **`package`** ports `release.py`'s notice rules to Rust (the release workflow keeps its Python), lists the engine's crates under its `LICENSE`, splits the debug file as `D-120` does and archives with GNU `tar`. Linux hosts only.
- **Later work** uses it: the acceptance game is made with `new` and shipped with `package`, `just template-check` runs RC-A7 from now on, and W12b and the template audit inherit `tungsten check` in the template's rules.

## 1. Audit

Read at `301f95c`.

| # | Finding | Evidence |
| --- | --- | --- |
| A1 | No `tools/cli/`. The workspace's tools are `tools/crash-probe` and `tools/launcher`, std-only, with the workspace's version, edition, MSRV, licence and lints | `Cargo.toml:9-22`, `tools/launcher/Cargo.toml:1-11` |
| A2 | The template depends on the engine by path and carries its own names: package `tungsten-template-basic`, crate `tungsten_template_basic` in `src/main.rs` and seven `tests/game/` files, `CARGO_BIN_EXE_tungsten-template-basic`, game ID and title in `tungsten.json`, the title again in a test's `GameInfo`. No `.gitignore` | `templates/basic/Cargo.toml:2,12,15`, `templates/basic/tungsten.json:2-3`, `templates/basic/src/main.rs:10-11`, `templates/basic/tests/game/visual.rs:21`, `templates/basic/tests/game/main.rs:39` |
| A3 | A member's `[profile.*]` tables are ignored, so the template has none. The outside check appends the workspace's four profile tables and copies its `Cargo.lock` | `Cargo.toml:99-123`, `scripts/template-check.sh:63-68` |
| A4 | The guide starts a game by hand: clone at the tag, `cp -R`, both engine lines onto the tag, rename package, crate and game ID, add a dev profile | `docs/getting-started.md:12-55` |
| A5 | The outside check: `cp -R`, the engine path rewritten to this checkout, profiles and lock appended, then an offline build, test and 3-frame smoke into this checkout's target folder, after a 3 GiB free-space floor | `scripts/template-check.sh:13,20,32-36,53-97` |
| A6 | The manifest loader stops at its first error: the first missing file in map order, the smallest failing cross reference, the first duplicate. `Missing*File` names the missing file, not the manifest | `crates/tungsten-core/src/assets/manifest.rs:10-82,569-573,593-600,821-831,838-852` |
| A7 | The staged load's check stops at the first failing entry and takes every staged entry back | `crates/tungsten-core/src/assets/data.rs:691-721,839-874` |
| A8 | A scene load and a prefab load stop at the file's first error. Neither checks its sprite ID at load (the extract and the spawn intern it). A map's objects stop at the first | `crates/tungsten-core/src/assets/scene.rs:1,288-327`, `crates/tungsten-core/src/assets/prefab.rs:65-68,89-116,584-598` |
| A9 | `load_all` returns at the first animation frame, tileset entry or particle render reference that names no registered asset, and needs the renderer | `crates/tungsten/src/asset_loader/mod.rs:428-497` |
| A10 | Each headless section loader loops over a per-file loader (`AnimationData::load`, `ThemeFile::load`, `SoundData::decode`, `TilemapData::load`, `ParticleConfig::load`), stops at the first failure, then inserts its registry. An unknown theme font token only warns | `crates/tungsten/src/asset_loader/mod.rs:61-79,111-131,180-230,400-421` |
| A11 | `Harness::load_manifest` loads the headless sections in the window startup's order, the materials as metadata, then the prefabs and data entries. Sprites, fonts, shaders and particle meshes are skipped | `crates/tungsten/src/testing.rs:93-131` |
| A12 | The atlas decodes each sprite and its sibling maps with `image::open` | `crates/tungsten/src/asset_loader/atlas.rs:72-80,359` |
| A13 | `App::new` validates the config, installs the logger, resolves the user folder (none in smoke mode) and loads `input.json`, failing on a bad one. Plugins build when added. `App::run` resolves the schedule, starts the watcher, then the event loop. The reload roots fall back to `manifest_path` | `crates/tungsten/src/app.rs:179-246,408-414,541-543,566-579,1850-1856`, `crates/tungsten/src/user_dir.rs:62-70` |
| A14 | The template's `main` loads `tungsten.json` before `App::new`, so a bad config ends the run before any engine code | `templates/basic/src/main.rs:13-29` |
| A15 | `Config::load` and `ActionMap::load` stop at a file's first error. A missing `tungsten.json` or `input.json` falls back to defaults. `GameConfig::validate` is the game ID rule | `crates/tungsten-core/src/config.rs:347-384,503-570`, `crates/tungsten-core/src/input/action_map.rs:370`, `crates/tungsten/src/app.rs:2057-2070` |
| A16 | `release.py package` archives the examples: CPU levels, one launcher per example, the shared runtime files, a README, the notices from a build job's `licenses.json`, the debug split (`objcopy`, then build-id and `.debug_line` checks), `gztar` or `zip` | `scripts/release.py:394-468,844-907` |
| A17 | The notices: crates from `cargo tree -e normal` and `cargo metadata`, rooted at the examples and the launcher, workspace members left out; a text per preference and markers; clarifications by version; the engine's embedded works; an intro that names the examples | `scripts/release.py:93-128,595-628,631-822` |
| A18 | The engine's crates ship no licence file of their own; the root `LICENSE` covers them, so a game's notices need it | `fd -d 1 LICENSE crates/tungsten crates/tungsten-core crates/tungsten-render crates/tungsten-kit` → none; `LICENSE` |
| A19 | `just api` lists four crates and runs the kit's components example into a fifth file under the same `--check` | `scripts/public-api.sh:24,76-81` |
| A20 | `just ctx` budgets three files. The template's `AGENTS.md` (11,951 B) has none, a known issue that names W14b. The self-test's tree must hold every budgeted file | `scripts/check-agent-context.py:22-27,141-145,185-200`, `docs/known-issues.md:104` |
| A21 | The LLM index is 12,275 B of its 12,288 B budget | `just ctx` output; `docs/LLM_INDEX.md:85` |
| A22 | `just checks` routes a `tools/` path to a review note and leaves it unclassified | `scripts/check-repo.py:565-567` |
| A23 | The release build compiles every workspace binary but the probe's and the template's, so the CLI builds there too, unshipped | `.github/workflows/release.yml:150-156` |
| A24 | cargo-shear needs each `#[path]` test file included from a subdirectory listed | `Cargo.toml:24-33` |
| A25 | `D-103` promises the CLI's commands and flags; Q20 is `open` in the catalog | `DECISIONS.md:1248`, `docs/plans/1.0/roadmap.json` (q20) |
| A26 | Known issues the CLI touches: the launcher's Windows errors (home "W7b or W14b"), the outside check's lock, tileset references | `docs/known-issues.md:101,106,112` |
| A27 | DESIGN describes the outside check's copy step by step | `DESIGN.md:371` |

## 2. Design

### 2.1 Commands and flags

The promised surface (`D-103`). `tungsten --help` prints exactly this, which `api/tungsten-cli.txt` records (§2.10):

```text
Usage: tungsten <command>

Commands:
  new <path>  Make a game in the new folder <path> from the engine's
              template, named after the folder, with the engine pinned to
              this command line's release tag
  check       Load the game's tungsten.json, input.json and every file its
              manifests name with the engine's loaders, in the game's own
              build, and report every error with its file
  package     Build the game for release, check it, and write its players'
              archive and its debug archive to dist/ (Linux)

Options:
  -h, --help     Print this help
  -V, --version  Print the version

Run check and package in the game's folder. They need the game's engine
to be the version of this command line.
```

`-h` or `--help` anywhere prints the usage and exits 0; `-V` or `--version` prints `tungsten <version>` and exits 0. An unknown command, a missing or extra argument, or an unknown option prints the usage to stderr and exits 2. A command exits 0 when it succeeds and 1 when it fails or finds errors. The version is the workspace's; the help text names no version, so a release cut leaves the snapshot as it is.

### 2.2 Version coupling

`check` and `package` read `cargo metadata --format-version 1` in the game's folder. Exactly one package named `tungsten` must be in the graph, at the CLI's own version; otherwise they exit 1, naming both versions and the `cargo install` line for the game's (Q5). The CLI, the `TUNGSTEN_CHECK` handshake and the embedded notice data then always match the engine the game builds, so the handshake stays outside the promise. `new` pins the tag of its own version, `v<version>`.

### 2.3 `check`

**Config stage, in the CLI.** `Config::load("tungsten.json")` and `ActionMap::load("input.json")`, the engine's own loaders, each at most one error, its first. A missing file is no error, as at startup (`D-008`). Either error ends the command after this stage, since the game cannot start without both files: its lines, then the summary naming the skipped stage.

**Content stage, in the game's build.** The CLI deletes `<tmp>/tungsten-check-<pid>.txt`, runs `cargo run --quiet` in the game's folder with `TUNGSTEN_CHECK` set to that path, with stdin, stdout and stderr inherited, then reads the file. The game's `main` runs as written. `App::new` sees the variable and resolves no user folder, as in smoke mode, so nothing of the player's is read or written. `App::run` sees it first of all and calls `run_check` in place of the schedule, the watcher and the event loop: no window, renderer, audio device, startup hook or frame. `run_check` checks the reload roots, writes one `<file>\t<message>` line per error, and returns an error when there is a line, so the process exits 1. No report file after the run means the game stopped before `App::run`; its stderr says why, and the CLI says the content stage did not run, exit 1.

What the content stage loads, in the harness's order (A11), each registry inserted as its section loader inserts it, before the prefabs and data that check against them (`D-157`):

| # | Loads | With | An error names |
| --- | --- | --- | --- |
| 1 | The manifest roots, merged, each path held inside the game's folder | `ResolvedManifest::load_for_check` with the current directory as the folder (§2.4) | the manifest that declares the entry, or the root that did not read or parse |
| 2 | Each sprite, then its normal map and emissive mask against the albedo's size; each sprite that decodes is registered | `image::open` and the atlas's own `decode_sprite_siblings` (A12), then `AssetRegistry::register_sprite` with its size, filter and paths and a placeholder handle (`TextureHandle(u32::MAX)`, `UvRect::FULL`), since no renderer allocates one (`D-048`) | the image |
| 3 | Animations; each frame's sprite against the registered sprites, as `load_all` checks it | `AnimationData::load` | the animation file |
| 4 | UI themes; each font token against `font_families` and the engine font | `ThemeFile::load`, `unknown_font_tokens` | the theme |
| 5 | Materials, as metadata | `MaterialRegistry::allocate`, as the harness does | none |
| 6 | Sounds | `SoundData::decode` | the sound |
| 7 | Tilemaps; each tileset entry against the registered sprites | `TilemapData::load` | the map |
| 8 | Particle configs; a quad's sprite against the registered sprites, a mesh's against the manifest's `particle_meshes` | `ParticleConfig::load` | the config |
| 9 | Data entries, then prefabs; each prefab's sprite against the registered sprites | `StagedLoad::add_data` and `add_prefab` per file, then `StagedLoad::commit_for_check` (§2.4) | the entry's file |
| 10 | Each map's objects | `World::check_map_objects` | the map |
| 11 | Scenes; each entry's sprite against the registered sprites | `SceneData::load`, as `load_scene` does | the scene |

So a game's own asset check (`World::register_component_with_assets`) that looks a manifest sprite up in `AssetRegistry` finds it, as at a window's startup, where the harness finds none.

**Every error, defined.** A file that does not read or parse, or fails its loader's checks, is one error, the loader's own first message for that file: a scene, a prefab, a data entry, an animation, a theme, a sound, a map, a particle config, a map's objects. Every reference to a missing file or ID is an error of its own: each missing file a manifest names, each root or entry whose file resolves outside the game's folder, each particle mesh that fails its check, each duplicate ID, each failing material shader, family face and fallback family, each file backing two entries, each normal map or emissive mask that does not decode or differs in size from its albedo, each animation frame, tileset entry, particle reference, prefab sprite and scene sprite, and each data or prefab entry's first missing ID. An entry whose file does not read, parse or decode is left out, so the files that name it report it missing as well. One that loads but fails a check stays in place, as `commit_for_check` leaves it, so references to it pass (Q12).

**Scenes** (Q6). Every `.json` file under a manifest root's folder, searched recursively with hidden folders skipped, that is neither a manifest root nor named by a manifest entry loads as a scene (`D-046`), so a scene with a misspelled top-level key fails as the game's own load would. The template's rules name every asset by its manifest ID, so an unlisted JSON file under `assets/` is a scene or a mistake; its error reads `listed in no manifest, so loaded as a scene: <the scene error>`.

**Not checked:** IDs written in Rust (`D-113` interning and W8's once-per-ID warnings cover that side); sprites, fonts or shaders a game registers in code; WGSL; font file contents; the schedule, which a game's tests catch (`Harness::new` panics); the player's files in the user folder.

**Output**, on stdout: one `<file>: <message>` line per error, sorted by file, then in the order found, with every path, the ones inside a message too, relative to the game's folder and `/`-separated. Then one summary line: `tungsten check: no errors`, `tungsten check: <n> errors in <m> files`, or, after a failed config stage, `tungsten check: <n> errors; the game's assets are checked once tungsten.json and input.json load`. Cargo's build output stays on stderr.

### 2.4 Core API

All are public in `tungsten-core`, which the umbrella re-exports whole as `tungsten::core` (`crates/tungsten/src/lib.rs:69`), so they are promised surface the freeze lists (`D-103`), each with rustdoc:

```rust
// crates/tungsten-core/src/assets/manifest.rs, re-exported from `assets`

/// An error [`ResolvedManifest::load_for_check`] found, with the manifest
/// to fix: the root that declares the entry it is about, or the root that
/// did not read or parse (M70).
#[derive(Debug)]
#[non_exhaustive]
pub struct ManifestIssue {
    /// The manifest file, as the roots name it.
    pub manifest: PathBuf,
    /// What is wrong, as `load_and_merge_many` would say it.
    pub error: ManifestError,
}

impl ResolvedManifest {
    /// Loads `roots` as [`Self::load_and_merge_many`] does, past every error,
    /// for `tungsten check`: a root that does not read or parse, or lies
    /// outside `folder`, is one issue and adds nothing; an entry whose file is
    /// missing or resolves outside `folder` (both canonicalized), a particle
    /// mesh that fails its check and an ID an earlier root declared are an
    /// issue each and are left out, the first declaration kept; then every
    /// material shader, family face and fallback family the graph lacks, and
    /// every file that backs two entries, is an issue. Returns the graph of
    /// what resolved and the issues, root by root, then the graph's sorted
    /// by ID.
    #[must_use]
    pub fn load_for_check(roots: &[impl AsRef<Path>], folder: &Path) -> (Self, Vec<ManifestIssue>);
}

// `ManifestError` gains `OutsideFolder { id: String, path: String }`, whose
// message names the entry and the path outside the game's folder. A new
// variant breaks an exhaustive match outside core, so step 1 adds its break
// ledger row, as the variants `D-115` and `D-157` added have theirs; nothing
// in the tree matches the enum exhaustively (`matches!` only).

// crates/tungsten-core/src/assets/data.rs

impl StagedLoad {
    /// Puts every staged entry in place as [`Self::commit`] does, then checks
    /// every entry the registries hold from a file and returns every failure,
    /// in the order `commit` checks them, rather than the first. A staged
    /// entry whose kind `world` registers for another type is a failure and
    /// stays out. Nothing is taken back, so the World is one a check throws
    /// away, as `tungsten check`'s is (M70).
    #[must_use]
    pub fn commit_for_check(self, world: &mut World) -> Vec<StagedLoadError>;
}
```

`load`, `load_and_merge_many`, `merge` and `commit` keep their results. `load_unvalidated`, `merge_entries`, `validate_cross_refs` and `check_file_backed` share one body with the new functions, reporting through a sink that the first-error callers stop at, so the two paths cannot drift.

### 2.5 Engine side, crate-private

```rust
// crates/tungsten/src/app.rs
/// Names the report file `tungsten check` reads. Set, `App::run` checks the
/// game's files in place of opening a window (M70).
const CHECK_ENV: &str = "TUNGSTEN_CHECK";
fn check_report_from_env() -> Option<PathBuf>; // set and not empty
impl App {
    pub(crate) fn run_check(mut self, report: &Path) -> anyhow::Result<()>;
}

// crates/tungsten/src/asset_loader/check.rs
pub(crate) struct CheckError {
    pub(crate) file: PathBuf,
    pub(crate) message: String,
}
pub(crate) fn check_files(world: &mut World, roots: &[PathBuf], game_dir: &Path) -> Vec<CheckError>;
```

`App::with_plugins` passes `smoke || checking` to `user_dir::resolve`. `run_check` lowers the log level to warnings (`log::set_max_level`) so the loaders' info lines stay out, takes `manifest_reload_roots(&self.manifest_roots, self.manifest_path.as_deref())` and the current directory, and writes the report. `App::run`'s rustdoc gains a paragraph on `TUNGSTEN_CHECK`. The section loaders, `load_all` and `Harness::load_manifest` are not changed; `atlas.rs`'s `decode_sprite_siblings` and its return type `SpriteSiblings` become `pub(crate)`, unchanged otherwise, so `check.rs` can call it without a private type in its interface. `check_files` calls the same per-file loaders (A10) and repeats only their few lines of registration, so a new section loader gains its line in `check_files` in the same change.

### 2.6 `package`

1. The game's folder (a `Cargo.toml` and a `tungsten.json`), the version gate (§2.2) and a Linux x86-64 host, from `rustc -vV`'s `host` line (Q7). Elsewhere it exits 1, naming the backlog row. `RUSTFLAGS` or `CARGO_ENCODED_RUSTFLAGS` already in the environment also stops it, naming the variable, since step 3 sets `RUSTFLAGS` itself.
2. The config stage of `check`; an error stops it.
3. **Build** for the portable baseline (Q17): `cargo build --release --bin <bin> --message-format=json-render-diagnostics` with `RUSTFLAGS="-C target-cpu=x86-64 -C link-arg=-Wl,--build-id"`. A set `RUSTFLAGS` replaces every config file's flags, so no `target-cpu=native` or `target-feature` from a game's or the user's config reaches the build: the effective flags are exactly these, as `release.yml` builds the examples (`D-072`, `D-120`). The binary carries a GNU build-id whatever the linker's default. The command prints the flags it built with. `<bin>` is the package's one binary or its `default-run`; the path comes from the `compiler-artifact` message. A game's own config flags, such as a linker choice, do not apply to `package`; the guide says so.
4. **Stage** under `<target-dir>/tungsten-package/<name>-<version>-<target>/`: the binary, `tungsten.json`, `input.json`, `assets/`, and the game's root `LICENSE*`, `COPYING*` and `NOTICE*` files, copied as files, following symbolic links, so the stage holds everything itself. No `README.txt` (Q13).
5. **Split** the staged binary as `D-120` does: `objcopy --only-keep-debug --compress-debug-sections=zlib` into the debug stage, then `objcopy --strip-debug --remove-section=.debug_gdb_scripts --add-gnu-debuglink`. The binary's build-id must equal the debug file's, and the debug file must hold `.debug_line`, both read from the ELF in Rust. A build without line tables fails, telling the game to set `debug = 1` under `[profile.release]`, which `new` writes.
6. **Check what ships:** the staged, stripped binary runs from the stage with `TUNGSTEN_CHECK`. The stage is then the game's folder, and `load_for_check` holds every root and entry inside it (§2.4), so what the content stage passes is the staged set alone. An entry outside it fails here with the check's error, as it would fail the player: a relative `../shared/player.png` is missing from the stage, and an absolute path into the source folder lies outside it. Any error stops it, with the report printed.
7. **Notices**, by `D-122`'s rules ported to Rust (Q8). The crates come from `cargo tree -e normal --target <host> --prefix none -f {p} -p <package>`, with details from `cargo metadata --format-version 1 --filter-platform <host>`. The game's own workspace members are left out. The engine's four crates are always listed, under MIT with the engine's root `LICENSE` (A18), citing their source in place of a crates.io page. Every other crate takes the first alternative, in `release.py`'s preference order, whose licences each match one of its files by the same marker phrases; `NOTICE` files are kept; the font guard applies. Clarifications come from the engine's `licenses/`, then from the game's own `licenses/clarifications.json` when it exists (same format and version check). The engine's embedded works (SMAA, JetBrains Mono's OFL) and the standard library's notices from `rustc --print sysroot` complete them. A crate with no matching text fails, every such crate named. The heading reads `Third-party notices: <name> <version> for <target>`, and the intro names the game, the engine version and the files. `THIRD-PARTY-NOTICES.txt` and `THIRD-PARTY-NOTICES-rust-std.html` join the stage.
8. **Archive** with `tar -czf` (Q3): `dist/<name>-<version>-<target>.tar.gz`, and the debug file under the same top folder in `dist/<name>-<version>-<target>-debug.tar.gz`, so it extracts over the player archive. Each is replaced if present; both paths are printed. A missing `cargo`, `rustc`, `tar` or `objcopy` is named in the error.

### 2.7 `new`

1. The name is `<path>`'s last component: 1–64 characters of lowercase ASCII letters, digits, `-` and `_`, starting with a letter, passing `GameConfig::validate` (the game ID rule, which refuses Windows device names). Refused as well: Rust keywords, `test`, `build`, `deps`, `examples`, `incremental`, the template's dependencies (`anyhow`, `log`, `serde`, `tungsten`) and the engine's packages (`tungsten-core`, `tungsten-render`, `tungsten-kit`, `tungsten-cli`, `tungsten-template-basic`). `<path>` must not exist and its parent must; `new` never overwrites.
2. Every file of `templates/basic`, embedded at the CLI's build (Q9). `build.rs` walks the folder, skipping `target/` and `dist/`; a test compares the list with `git ls-files templates/basic` when git is there. In UTF-8 files, `tungsten-template-basic` becomes the name, `tungsten_template_basic` the crate name (`-` as `_`) and `Tungsten template` the name, so the window title and the tests' `GameInfo` follow. Other files are copied byte for byte.
3. `Cargo.toml`: both `path = "../../crates/tungsten"` become `git = "https://github.com/JosephGibson/Tungsten", tag = "v<version>"`, and a `path = ` left over fails. The workspace's `[profile.*]` tables, embedded as the outside check appends them today (A3), follow under a comment line, so `package` finds line tables and the game builds as the engine is tested.
4. The workspace's `Cargo.lock`, embedded, with its one `name = "tungsten-template-basic"` entry renamed as the other files are, so no template name is left and the entry becomes the game's own: a new game starts on the versions the engine tested, which the embedded clarifications are checked at. Cargo drops the workspace's other members from it at the first build.
5. It prints the folder and the next commands, `cargo run` and `tungsten check`. No git repository (Q15). The title screen now shows the name, so `tests/fixtures/title.png` is stale until recaptured (Q14).

### 2.8 Dependencies (`D-015`)

No new crate. The CLI depends on `tungsten-core` (`Config`, `ActionMap`, `GameConfig`), `serde_json` (`cargo metadata`, the embedded clarifications) and `anyhow`, all locked workspace dependencies, each added in the step that first uses it, so `just udeps` holds after every step. Arguments are parsed by hand (Q2): none of `D-015`'s three rules admits an argument-parsing crate, and three commands with two options need none. The external tools are the platform's: `cargo` and `rustc` from the game's toolchain, GNU `tar` and `gzip`, and binutils' `objcopy`, which `release.yml`'s publish job already uses (`D-120`). `build.rs` uses std alone.

### 2.9 Template and guide

- `templates/basic/.gitignore` (new): `/target` and `/dist`.
- `templates/basic/AGENTS.md`: a Layout bullet, ``- `tungsten check`, the engine's command line, loads `tungsten.json`, `input.json` and every file the manifest names as the game does, and reports every error with its file; `tungsten package` makes the players' archive in `dist/`.``, and the commit line becomes ``- Before a commit: `cargo test`, `cargo clippy --all-targets -- -D warnings` and `tungsten check`.``. About 12,200 B after, under the 12,288 B budget step 5 adds (Q11).
- `templates/basic/README.md`: the start paragraph opens with `tungsten new <name>`, the engine's command line, which the getting-started guide installs.
- `src/` is unchanged, so `D-181`'s line budget and the title fixture stand.
- `docs/getting-started.md`: the start section installs the CLI (`cargo install --locked --git https://github.com/JosephGibson/Tungsten --tag vX.Y.Z tungsten-cli`) and runs `tungsten new my-game`, says what `new` writes and the name rule, and drops the manual copy, rename and profile steps. A new section, "Check and package", covers what `check` loads, the scene rule and the game's folder as the bound on every path, what it cannot see, the exit codes, and `package`'s two archives, their contents, its own build flags (a game's config flags do not apply, and a set `RUSTFLAGS` stops it), the debug file, `licenses/clarifications.json` for a crate without a text (linking [releases](../../../releases.md#license-notices)), and recapturing the title fixture. Line 3's "packaging joins it" becomes present tense.

### 2.10 The outside check, the snapshot and the budget

`scripts/template-check.sh` (Q10): the free-space floor's default rises from 3 to 6 GiB for the release build. It builds the CLI (`cargo build --quiet --locked -p tungsten-cli`) and makes `$copy_root/outside-game` with `tungsten new`, keeping the guard against game files above the copy. It checks that the copy's `Cargo.toml` pins `tag = "v<workspace version>"` twice, then rewrites both pins to `path = "<checkout>/crates/tungsten"` and fails on a `git = ` left over. It copies the checkout's `.cargo/config.toml` into the copy and exports `CARGO_TARGET_DIR=<checkout>/target` and `CARGO_NET_OFFLINE=true`, so the commands the CLI spawns share this checkout's artifacts offline. Then, in the copy:

1. `tungsten check` exits 0 with `tungsten check: no errors`.
2. Build, test and the 3-frame smoke run, as today.
3. `tungsten package` exits 0. Every `tungsten package` run in the script, the failing ones of item 4 too, runs under `env -u RUSTFLAGS -u CARGO_ENCODED_RUSTFLAGS` with `CARGO_TARGET_DIR=<checkout>/target/package-check`, so a shell's flags cannot stop it, and its portable release build keeps a folder of its own across releases and never rebuilds `target/release`, which `just perf` builds with other flags. The player archive lists the binary, `tungsten.json`, `input.json`, `assets/manifest.json` and both notice files and no `.debug` file; its notices list `tungsten <version>` and JetBrains Mono. The debug archive holds `<top>/outside-game.debug`. Both extract into `$copy_root/extract`, and the extracted binary smoke-runs 3 frames from its folder.
4. Two files outside the stage, one at a time. First, a sprite entry naming `../outside.png`, which exists beside `assets/`, inside the game's folder. Second, a sprite entry naming the absolute path of the copy's own `assets/sprites/player.png`. For each, `tungsten check` exits 0, and `tungsten package` exits 1 with a line naming `assets/manifest.json`, the first as missing from the stage and the second as outside it. Each entry is removed after its run.
5. Five seeded content errors, one file each: a manifest sprite whose file is missing (`assets/manifest.json`), an unknown component (`assets/prefabs/gem.json`), an unknown sprite ID (`assets/prefabs/hazard.json`), text that is not JSON (`assets/data/cue_rules/labels.json`, which no file names) and a scene with an unknown component (a new `assets/broken_scene.json`). `tungsten check` exits 1, prints a line starting with each file, and ends `tungsten check: 5 errors in 5 files`.
6. Two seeded config errors, `render.msaa` 3 in `tungsten.json` and `input.json` not JSON: exit 1, one line each, and the skipped-stage summary.

JSON edits use `python3 -I` from a heredoc. The script prints one `OK` line per stage and ends `Template check passed: ...`.

`scripts/public-api.sh` also writes `cargo run -q --locked -p tungsten-cli -- --help` to `api/tungsten-cli.txt`, under the same `cmp` and `--check` rule as the components file. `scripts/check-agent-context.py` budgets `templates/basic/AGENTS.md` at 12,288 B and 100 lines and lists `templates/basic/CLAUDE.md` as import-only. Its self-test tree gains both files and an oversize-template case.

### 2.11 Seam

Core gains two reporting functions, `ManifestIssue` and one `ManifestError` variant, and no render or `wgpu` type (`D-007`, `D-016`). The umbrella's check reads and writes the World at startup and never in a frame, so the extract seam (`D-018`) and the frame body are untouched. The CLI depends on core alone, never on render or the umbrella.

## 3. Steps

### Step 1: The API, the template's text and the decisions

- **Files:** `Cargo.toml`, `Cargo.lock`, `tools/cli/Cargo.toml`, `tools/cli/src/main.rs`, `tools/cli/src/args.rs`, `tools/cli/src/tests/args.rs`, `tools/cli/tests/cli.rs`, `crates/tungsten-core/src/assets/manifest.rs`, `crates/tungsten-core/src/assets/mod.rs`, `crates/tungsten-core/src/tests/assets/manifest.rs`, `crates/tungsten-core/src/assets/data.rs`, `crates/tungsten-core/src/tests/assets/data.rs`, `scripts/public-api.sh`, `api/tungsten-core.txt`, `api/tungsten-cli.txt`, `templates/basic/.gitignore`, `templates/basic/AGENTS.md`, `templates/basic/README.md`, `DECISIONS.md`, `docs/DECISION_INDEX.md`, `docs/plans/1.0/w04-api-freeze.md`
- **Change:**
  1. The decisions first, with the tungsten-decision skill (§5): 182 and 183, their index rows, the markers on `D-106`, `D-107` and `D-123` and those rows.
  2. `tools/cli/Cargo.toml`: `tungsten-cli`, the workspace's version, edition, `rust-version` and licence, a description, `publish = false`, `[[bin]] name = "tungsten"`, `path = "src/main.rs"`, `doc = false`, `[lints] workspace = true` and no dependency yet. The workspace's members gain `tools/cli`.
  3. `args.rs`: §2.1's usage as one constant and a parser over `std::env::args_os()` into `Help`, `Version`, `New(PathBuf)`, `Check` and `Package`. `main.rs` dispatches; `new`, `check` and `package` print `tungsten <command>: not built in this tree yet` and exit 1 until their steps. Unit tests cover each form and each usage error, and `tools/cli/tests/cli.rs` runs `CARGO_BIN_EXE_tungsten --help` and an unknown command.
  4. Core: `ManifestIssue`, `ManifestError::OutsideFolder`, `ResolvedManifest::load_for_check` and `StagedLoad::commit_for_check` (§2.4), with the shared bodies. Tests, each name holding `for_check`: one root with a missing sprite file, a missing prefab file, a missing data file and an invalid particle mesh gives four issues naming it, and its other entries resolve; a second root redeclaring an ID gives one issue naming the second root, and the first entry stays; a material shader, a family face and a fallback family the graph lacks give three issues, each naming its declaring root; a root that does not parse gives one issue and adds nothing; an entry naming an existing file outside the folder, by an absolute path and by `../`, gives an `OutsideFolder` issue each, and both entries are left out. For the staged load, two prefabs each naming a missing data ID and a data entry naming a missing prefab give three failures in `commit`'s order, all left in place. The existing manifest and staged-load tests pass unchanged.
  5. `scripts/public-api.sh` gains the CLI file (§2.10); `just api`.
  6. The [break ledger](../../1.0/w04-api-freeze.md#break-ledger): a row for `ManifestError` gaining `OutsideFolder` (an exhaustive match outside core needs the arm), and a row of additions for the freeze, as M49's row lists its own: `ManifestIssue`, `ResolvedManifest::load_for_check`, `StagedLoad::commit_for_check`, and the CLI's commands and flags in `api/tungsten-cli.txt` (`D-103`).
  7. The template's text (§2.9): `.gitignore`, `AGENTS.md`, `README.md`.
  8. The step 1 API critique, as the skill says, on the `api/` and `templates/` diff, recorded in §9 as `C.<n>`. A finding that holds changes this step before step 2.
- **Done-when:**
  - `cargo fmt --all -- --check` → exit 0
  - `cargo test -p tungsten-core --locked --lib -- for_check` → at least 7 passed, 0 failed
  - `cargo test -p tungsten-core --locked --lib -- assets::manifest::tests assets::data::tests` and `cargo test -p tungsten-core --locked --test manifests` → 0 failed
  - `cargo test -p tungsten --locked` → 0 failed: the umbrella's loads and its hot reload, whose manifest reload and staged commits run the shared bodies inside a frame (`crates/tungsten/src/asset_loader/reload.rs`)
  - `cargo test -p tungsten-cli --locked` → at least 8 passed, 0 failed
  - `cargo run -q --locked -p tungsten-cli -- --help` → §2.1's text verbatim; after `just api`, `cmp api/tungsten-cli.txt <(cargo run -q --locked -p tungsten-cli -- --help)` → exit 0
  - `cargo run -q --locked -p tungsten-cli -- frobnicate; echo "exit $?"` → the usage on stderr, then `exit 2`
  - `cargo doc --workspace --no-deps --locked 2>&1 | rg -i collision` → no line (rg exits 1)
  - `just lint` → exit 0; `just deps` → exit 0
  - `git --no-pager diff --name-only -- api/` → `api/tungsten-core.txt` alone; `git ls-files -o --exclude-standard -- api/` → `api/tungsten-cli.txt`
  - `rg -c 'ResolvedManifest::load_for_check\(' api/tungsten-core.txt` equals `rg -c 'ResolvedManifest::load_and_merge_many\(' api/tungsten-core.txt` (3 at `301f95c`), and `rg -c 'StagedLoad::commit_for_check\(' api/tungsten-core.txt` equals `rg -c 'StagedLoad::commit\(' api/tungsten-core.txt` (3)
  - `rg -n '^## D-18[23] ' DECISIONS.md` → 2 lines; `rg -c '^\| .D-18[23]. \|' docs/DECISION_INDEX.md` → 2; `rg -c '^\*\*Amended by D-18[3]:' DECISIONS.md` → 3
  - `rg -c 'OutsideFolder' docs/plans/1.0/w04-api-freeze.md` → at least 1; `rg -c 'load_for_check.*tungsten-cli|tungsten-cli.*load_for_check' docs/plans/1.0/w04-api-freeze.md` → 1 (the additions row)
  - `rg -c 'tungsten check' templates/basic/AGENTS.md` → 2; `rg -c 'tungsten new' templates/basic/README.md` → 1; `templates/basic/.gitignore` holds `/target` and `/dist`
  - `cargo test -p tungsten-template-basic --locked` → 0 failed
  - `just repo-check` and `just ctx` → exit 0
- **Moves:** `api/tungsten-core.txt` (four items, each on every public path) and the new `api/tungsten-cli.txt`.

### Step 2: `check`

- **Files:** `crates/tungsten/src/asset_loader/check.rs`, `crates/tungsten/src/asset_loader/mod.rs`, `crates/tungsten/src/asset_loader/atlas.rs`, `crates/tungsten/src/tests/check.rs`, `crates/tungsten/src/app.rs`, `crates/tungsten/src/tests/app.rs`, `Cargo.toml`, `Cargo.lock`, `tools/cli/Cargo.toml`, `tools/cli/src/main.rs`, `tools/cli/src/check.rs`, `tools/cli/src/cargo.rs`, `tools/cli/src/tests/check.rs`
- **Change:**
  1. `asset_loader/check.rs` (§2.3, §2.5), declared `pub(crate) mod check` in `asset_loader/mod.rs`; `decode_sprite_siblings` and `SpriteSiblings` in `atlas.rs` become `pub(crate)`. The tests in `crates/tungsten/src/tests/check.rs` run as `asset_loader::check::tests`. The workspace's cargo-shear `ignored-paths` line becomes `crates/tungsten/src/tests/{asset_loader,testing_ui,check}.rs` (A24).
  2. `app.rs`: `CHECK_ENV`, `check_report_from_env`, the user folder off while checking, `run_check`, and the branch at the top of `App::run` with its rustdoc. `run_frame` is not touched.
  3. The seeded test: a World from `App::with_plugins` with `DefaultPlugins`, `KitPlugins` and one test component, over a temp folder with two manifest roots and fourteen errors, one per file but the two roots' three. The errors: a sprite file missing and a sprite named by an absolute path to a file outside the folder (both root 1), an ID redeclared (root 2), a normal map of another size than its albedo, a theme font token naming no family, an animation frame naming no sprite, a tileset entry naming no sprite, a particle config naming no mesh, a prefab with an unknown component, a prefab naming no sprite, a data entry naming no prefab, a scene with a bad transform, a scene whose `entities` key is misspelled, and an unlisted file that is not JSON. A second data entry names the prefab with the unknown component and adds no error, since that prefab stays in place (Q12). `check_files` reports 14 errors, each naming its file relative to the folder, and no manifest root is loaded as a scene; with each error fixed, none. A second test registers a component through `World::register_component_with_assets` whose asset check looks the sprite its value names up in `AssetRegistry`: a prefab whose component names a manifest sprite passes, and one whose component names no sprite fails. Two `app::tests` tests named `run_check_…`: `run_check` writes one line naming the manifest and fails on a missing sprite file, and writes an empty report and passes on clean files.
  4. The CLI: `cargo.rs` (running `cargo metadata` and reading the `tungsten` version), `check.rs` (the config stage, the content stage, the output), dispatched from `main.rs`. `tools/cli/Cargo.toml` adds `tungsten-core`, `serde_json` and `anyhow`. Tests: a broken `tungsten.json` and a broken `input.json` give two lines naming them; missing files give none; a `cargo metadata` sample at the CLI's version passes the gate, and one at another version, or with no `tungsten`, fails it; a report file's tab lines print as `<file>: <message>` with the file count.
- **Done-when:**
  - `cargo fmt --all -- --check` → exit 0
  - `cargo test -p tungsten --locked --lib -- asset_loader::check::tests` → at least 3 passed (the seeded set, its clean twin and the sprite lookup), 0 failed
  - `cargo test -p tungsten --locked --lib -- app::tests::run_check` → 2 passed, 0 failed
  - `cargo test -p tungsten --locked` → 0 failed (the section loaders, `load_all` and the harness unchanged)
  - `cargo test -p tungsten-cli --locked` → 0 failed
  - `cargo build -q --locked -p tungsten-cli -p tungsten-template-basic && (cd templates/basic && ../../target/debug/tungsten check); echo "exit $?"` → last line `tungsten check: no errors`, then `exit 0`
  - A seeded copy through the template's binary: `repo="$PWD" && tmp="$(mktemp -d)" && cp -R templates/basic/tungsten.json templates/basic/input.json templates/basic/assets "$tmp"/`, seed §2.10's five content errors there, then `(cd "$tmp" && TUNGSTEN_CHECK="$tmp/report.txt" "$repo/target/debug/tungsten-template-basic"); echo "exit $?"` → `exit 1`, and `cut -f1 "$tmp/report.txt" | sort -u` → the five seeded files
  - `rg -c 'TUNGSTEN_CHECK' crates/tungsten/src/app.rs` → at least 2; `git --no-pager diff -- crates/tungsten/src/app.rs | rg '^[-+].*fn run_frame'` → no line
  - `cargo test -p tungsten-template-basic --locked` → 0 failed
  - `just lint` → exit 0; `just udeps` → exit 0
- **Moves:** none. No perf row: startup and the CLI only (§7).

### Step 3: `package`

- **Files:** `tools/cli/build.rs`, `tools/cli/src/main.rs`, `tools/cli/src/package.rs`, `tools/cli/src/notices.rs`, `tools/cli/src/elf.rs`, `tools/cli/src/tests/package.rs`, `tools/cli/src/tests/notices.rs`, `tools/cli/src/tests/elf.rs`
- **Change:**
  1. `build.rs`: embeds `licenses/` (every file), the root `LICENSE` and the two embedded works' licence files (`crates/tungsten-render/src/assets/smaa/ATTRIBUTION.md`, `crates/tungsten/assets/fonts/OFL.txt`), each with `cargo:rerun-if-changed`, into `$OUT_DIR`.
  2. `notices.rs`: §2.6 step 7. The rules ported from `release.py` (A17), with the engine's crates and the game's clarifications added. Tests ported from `scripts/test-release.py`'s notice cases: each expression form, the preference order, the GPL alternative left out, `NOTICE` kept, a clarification and its version check, a bundled work, the font guard, every crate without an entry named in one error. Then the engine's crates under its `LICENSE`, and a game's own clarification. A drift test reads `scripts/release.py` and fails when its `PREFERENCE` order, its `MARKERS` phrases or its `EMBEDDED_WORKS` paths differ from the CLI's.
  3. `elf.rs`: the section-header reader, the GNU build-id note and the `.debug_line` check, ported from `release.py` (A16), tested on synthetic ELF bytes (a build-id, no build-id, a truncated header).
  4. `package.rs`: §2.6, dispatched from `main.rs`. Tests: the stage's file list and the two archive names from a sample `cargo metadata`; the build's command and environment, `RUSTFLAGS` exactly `-C target-cpu=x86-64 -C link-arg=-Wl,--build-id`; the refusals off Linux and with `RUSTFLAGS` or `CARGO_ENCODED_RUSTFLAGS` set (as functions of the host string and the environment); the missing-line-tables message. Cargo's precedence was probed on 2026-10-20 with a scratch crate whose config file set `target-cpu=native`. A `--config` rustflags array was joined after it (`-C target-cpu=native -C target-cpu=x86-64`), so a config's `target-feature` would have stayed. With `RUSTFLAGS` set, neither array reached `rustc`, which is why `package` sets `RUSTFLAGS`.
- **Done-when:**
  - `cargo fmt --all -- --check` → exit 0
  - `cargo test -p tungsten-cli --locked` → 0 failed, with at least 18 tests in `notices`, `elf` and `package` modules
  - `v="$(python3 -B scripts/release.py version)" && cargo build -q --locked -p tungsten-cli && (cd templates/basic && CARGO_TARGET_DIR=../../target/package-check ../../target/debug/tungsten package); echo "exit $?"` → a line naming `RUSTFLAGS="-C target-cpu=x86-64 -C link-arg=-Wl,--build-id"`, two archive paths under `templates/basic/dist/`, then `exit 0`; `target/release` is left alone (§2.10)
  - `(cd templates/basic && RUSTFLAGS="-C target-cpu=native" ../../target/debug/tungsten package); echo "exit $?"` → names `RUSTFLAGS` before building, then `exit 1`
  - `tar -tzf templates/basic/dist/tungsten-template-basic-0.1.0-x86_64-unknown-linux-gnu.tar.gz` → one top folder holding `tungsten-template-basic`, `tungsten.json`, `input.json`, `assets/manifest.json`, `THIRD-PARTY-NOTICES.txt` and `THIRD-PARTY-NOTICES-rust-std.html`, and `rg '\.debug$'` on the list finds nothing; the `-debug.tar.gz` lists `<top>/tungsten-template-basic.debug` alone
  - From the extracted notices: `rg -c "^tungsten $v\$|^tungsten-core $v\$" THIRD-PARTY-NOTICES.txt` → 2; `rg -c 'JetBrains Mono' THIRD-PARTY-NOTICES.txt` → at least 1; the first line reads `Third-party notices: tungsten-template-basic 0.1.0 for x86_64-unknown-linux-gnu`
  - The extracted binary, from its folder: `TUNGSTEN_SMOKE_FRAMES=3 timeout 90 ./tungsten-template-basic; echo "exit $?"` → `exit 0` (GPU and display)
  - `rm -rf templates/basic/dist && git status --short --untracked-files=all templates/basic` → step 1's `.gitignore`, `AGENTS.md` and `README.md` alone
  - `just lint` → exit 0; `just udeps` → exit 0
- **Moves:** none.

### Step 4: `new`

- **Files:** `tools/cli/build.rs`, `tools/cli/src/main.rs`, `tools/cli/src/new.rs`, `tools/cli/src/tests/new.rs`, `tools/cli/tests/cli.rs`
- **Change:** `build.rs` also embeds the template (skipping `target/` and `dist/`), the workspace's `Cargo.lock` and its `[profile.*]` tables. `new.rs` follows §2.7, dispatched from `main.rs`. Tests: a game made in a temp folder holds every template file; no token is left, in the lock either; it has both pins, the profiles, the lock equal to the workspace's but for the renamed entry, and `.gitignore`. An existing folder is refused with nothing written. Refused names: an uppercase letter, a leading digit, a `.`, 65 characters, `con`, `fn`, `test`, `tungsten`, `serde`. Accepted: `my-game`, `my_game`, `g`. The embedded list equals `git ls-files templates/basic`. `tools/cli/tests/cli.rs` runs `new` end to end.
- **Done-when:**
  - `cargo fmt --all -- --check` → exit 0
  - `cargo test -p tungsten-cli --locked` → 0 failed
  - `v="$(python3 -B scripts/release.py version)" && tmp="$(mktemp -d)" && cargo build -q --locked -p tungsten-cli && target/debug/tungsten new "$tmp/my-game" && rg -c "tag = \"v$v\"" "$tmp/my-game/Cargo.toml"` → `2`
  - `rg -l 'tungsten_template_basic|tungsten-template-basic|Tungsten template' "$tmp/my-game"` → no file (rg exits 1)
  - `sed 's/^name = "tungsten-template-basic"$/name = "my-game"/' Cargo.lock | cmp - "$tmp/my-game/Cargo.lock"` → exit 0 (one entry renamed, at `Cargo.lock:3067` at `301f95c`)
  - `diff -rq templates/basic "$tmp/my-game"` → `Cargo.toml`, `tungsten.json`, `src/main.rs` and the `tests/game/` files that name the crate, the binary or the title differ, and `Cargo.lock` is only in the new game
  - `target/debug/tungsten new "$tmp/my-game"; echo "exit $?"` → names the existing folder, then `exit 1`
  - `just lint` → exit 0; `just udeps` → exit 0
- **Moves:** none.

### Step 5: The outside check, the template's budget, the guide and the routes

- **Files:** `scripts/template-check.sh`, `justfile`, `scripts/check-agent-context.py`, `scripts/check-repo.py`, `scripts/test-check-repo.py`, `docs/getting-started.md`, `docs/validation.md`, `docs/releases.md`, `docs/LLM_INDEX.md`
- **Change:**
  1. `scripts/template-check.sh` (§2.10); the `justfile`'s `template-check` comment names `new`, `check` and `package`.
  2. `scripts/check-agent-context.py`: the template budget, the import-only file and the self-test case (§2.10).
  3. `scripts/check-repo.py`: `tools/cli/` paths recommend `cargo test -p tungsten-cli --locked` and `just template-check`, and the template's paths now also `cargo test -p tungsten-cli --locked`, since `new` embeds them; `scripts/test-check-repo.py` gains a case.
  4. `docs/getting-started.md` (§2.9). `docs/validation.md`: the template row's hardware cell names `new`, `check` and `package`, and a CLI row follows it. `docs/releases.md:72`: the outside check's sentence names `new`, `check` and `package` and the 6 GiB floor.
  5. `docs/LLM_INDEX.md`: the template row becomes the template and CLI row. Its paths are `templates/basic/src/`, `templates/basic/AGENTS.md`, `tools/cli/src/`, `tungsten/asset_loader/check.rs`, `scripts/template-check.sh` and `docs/getting-started.md`, and its task text "Game template (shell, sample, seam) and the `tungsten` CLI" with the decision range from `D-181` to 183. Measured at `301f95c`, that row leaves the index at 12,262 B.
- **Done-when:**
  - `just template-check` → exit 0: an `OK` line for the copy, the pin, the clean check, the build, the tests, the smoke run, the package, the archive's smoke run, the two files outside the stage, the five seeded content errors and the two seeded config errors, then `Template check passed: ...`
  - `just script-test` → exit 0
  - `just ctx` → exit 0; `python3 -B scripts/check-agent-context.py --self-test` → `self-test: OK`, with an oversize-template case among the cases
  - `wc -c < docs/LLM_INDEX.md` → at most 12288; `rg -c 'tools/cli/src/' docs/LLM_INDEX.md` → 1
  - `just checks tools/cli/src/main.rs` → recommends `cargo test -p tungsten-cli --locked` and `just template-check`, and prints no `UNCLASSIFIED` line
  - `rg -n 'cargo install --locked --git' docs/getting-started.md` → 1 line; `rg -c 'tungsten new|tungsten check|tungsten package' docs/getting-started.md` → at least 3
  - `just repo-check` → exit 0
- **Moves:** `just template-check`'s flow and its floor's default (3 → 6 GiB).

### Step 6: Records

- **Files:** `DESIGN.md`, `docs/known-issues.md`, `docs/plans/1.0/backlog-1.x.md`, `docs/plans/1.0/w14-tooling.md`, `docs/plans/1.0/criteria.md`, `docs/plans/1.0/roadmap.json`
- **Change:**
  1. `DESIGN.md`: `### Project CLI — M70` after M69's section: the three commands, the check's two stages and its handshake, the every-error contract, the version coupling, `package`'s archives and the notices port. The outside-check sentence at `DESIGN.md:371` names `new`, `check` and `package`.
  2. Known issues: the template budget entry (A20) leaves. The tileset entry keeps only its extract-warning half, for W8b. The launcher entry's home becomes W7b. The outside check's lock entry becomes: the outside check points `new`'s pin at this checkout's path and builds offline, so fetching the engine from its tag stays untested until W9b's final pass or C3.
  3. Backlog rows: `tungsten package` on Windows and macOS hosts (zip, PDB, dSYM, CPU levels), back with Q3's tiers or a host for the acceptance game; one notice implementation for `release.py` and the CLI, back when a notice rule changes or the release workflow moves onto the CLI.
  4. `w14-tooling.md`: W14b's status line (M70, both decisions by bare ID, `v0.90.0`), and follow-ups 1 and 2 closed in italics, as W14a's step reads.
  5. Q20: criteria §10 reads answered on the run's date by its default, which M70 took (`D-150`), naming decision 182; `roadmap.json`'s `q20` gets `"state": "default"` and the answer.
- **Done-when:**
  - `rg -n '^### Project CLI — M70' DESIGN.md` → 1 line; `rg -c 'template-check.*tungsten new' DESIGN.md` → at least 1 (0 at `301f95c`)
  - `rg -c "have no .just ctx. budget|W14b's .check. covers|W7b or W14b" docs/known-issues.md` → 0
  - `rg -c 'tungsten package' docs/plans/1.0/backlog-1.x.md` → at least 2
  - `rg -n 'done in M70' docs/plans/1.0/w14-tooling.md` → 1 line
  - `jq -r '.questions[] | select(.id == "q20") | .state' docs/plans/1.0/roadmap.json` → `default`
  - `just repo-check` and `just ctx` → exit 0
- **Moves:** none.

### Step 7: Release 0.90

- **Files:** `CHANGELOG.md`, `DESIGN.md`, `Cargo.toml`, `Cargo.lock`, the register row, the README's four owned Now fields (`roadmap.py summary`), this plan (moved to `docs/plans/archive/`)
- **Change:** close workflow §5 (§8) and this step's evidence row with `status: done`, then [tungsten-finalize](../../../../.claude/skills/tungsten-finalize/SKILL.md) and [tungsten-release](../../../../.claude/skills/tungsten-release/SKILL.md) ([releases](../../../releases.md)): the plan archived, `just release-cut 0.90.0` once, every release check once after it (`just template-check` among them runs `new`, `check` and `package` on the cut tree, so the pin reads `v0.90.0`), then the preflight with `--message`. The `CHANGELOG.md` line names the three commands, the check's handshake, the Linux archive and both decisions.
- **Done-when:** [releases](../../../releases.md) step 1's command chain → every check passes, quoted in the report (the plan is archived before the cut); `just release-preflight 0.90.0 --repo JosephGibson/Tungsten --message 'Update 0.90: M70 project command line'` → passes and prints the command block and the post-merge block
- **Ends with:** the changed-file list, the command block, the post-merge block, then the next prompt: the `qa-plan` prompt for the Template audit (`w12qa`, a QA stop), the next register row ([tungsten-next's prompts](../../../../.claude/skills/tungsten-next/prompts.md))

## 4. Open questions

| # | Question | Default |
| --- | --- | --- |
| Q1 | Q20's tiers | `check` and `package` Must, `new` Should (`D-150`). All three land here; a skipped `new` does not stop the release (§7) |
| Q2 | Argument parsing: by hand or a crate? | By hand: three commands and two options, and `D-015` admits no argument-parsing crate |
| Q3 | Archive writing: a crate or the platform's tools? | GNU `tar` with gzip, on Linux, beside binutils' `objcopy` (`D-120`). A crate (`flate2` with a hand-written writer, or `zip`) comes with Windows packaging |
| Q4 | How does `check` learn the game's components and data kinds (w14 follow-up 2)? | The game's own build: `cargo run` with `TUNGSTEN_CHECK`, checked in `App::run` after every plugin has built. No registry file for the tool |
| Q5 | CLI and engine versions | Equal, read from `cargo metadata`, for `check` and `package`; `new` pins its own tag |
| Q6 | Which files are scenes? | Every unlisted `.json` under a manifest root's folder, since the template names every other asset by its ID; a misspelled scene stays checked. A manifest section for scenes would be a format change for a later candidate |
| Q7 | Which hosts does `package` serve? | Linux x86-64 alone; Windows and macOS are a backlog row |
| Q8 | One notices implementation or two? | A Rust port in the CLI, held to `release.py`'s tables by a drift test. One implementation is a backlog row |
| Q9 | Where does `new` get the template, lock and profiles? | Embedded at the CLI's build from this checkout: no network and no git at `new` time |
| Q10 | Does the outside check move onto `new`'s output at every release? | Yes, with `check` (clean, then seeded) and `package`, so RC-A7 runs at every release; the floor's default rises from 3 to 6 GiB for the release build |
| Q11 | The template's `AGENTS.md` budget | 12,288 B and 100 lines, the index's byte figure; about 90 B stay free |
| Q12 | Cascades from a broken file | Only from a file that does not read, parse or decode: that entry is missing for the files that name it, and their errors are reported too. An entry that loads but fails a check stays in place, so references to it pass |
| Q13 | A `README.txt` in the game's archive | None: the archive is the game's, and the game writes its own |
| Q14 | The title fixture after `new` | Stale until recaptured, since the title follows the name. The guide says how; `new` keeps the visual test |
| Q15 | Does `new` make a git repository? | No; the guide says `git init` |
| Q16 | A public check for a game's tests | Not now: the check stays crate-private behind `TUNGSTEN_CHECK`; a follow-up if the acceptance game wants it in `cargo test` |
| Q17 | The packaged binary's CPU baseline | x86-64, through a `RUSTFLAGS` that `package` sets, which replaces every config file's flags as `release.yml` does, so no native CPU or target feature from a config reaches the build; a `RUSTFLAGS` already set is refused. One portable binary, no CPU levels or launcher (`D-072` stays the examples') |

Each question takes its stated default; the owner reviews the plan on their own time (`D-150`).

## 5. Decisions expected

Next free IDs at `301f95c`, checked again when step 1 runs:

- **182 — the project command line.** `tungsten-cli` in `tools/cli/`, binary `tungsten` with `doc = false`. Q20's tiers. The commands, options and exit codes of §2.1. The version coupling (§2.2). `check`'s two stages, the `TUNGSTEN_CHECK` handshake (outside the promise, since the CLI and the engine move together) and the every-error contract (§2.3). `package`'s Linux archives on the portable x86-64 baseline, checked as staged, with its notices port and its debug split (§2.6). `new`'s copy, pins, profiles and lock (§2.7). No new crate (§2.8). Extends `D-120` and `D-122` to a game's archive, and settles `D-153`'s open point on the check's components.
- **183 — the tools around it.** The outside check runs on `new`'s output with `check` and `package` (amends `D-123`'s outside-copy clause). `api/tungsten-cli.txt` holds the CLI's help under `just api` and `just api-check` (amends `D-107`'s snapshot files). `templates/basic/AGENTS.md` joins `just ctx` at 12 KiB and 100 lines (amends `D-106`'s budget list).

## 6. Invariants

- **May move:** `api/tungsten-core.txt` (step 1: `ManifestIssue`, `ManifestError::OutsideFolder`, `load_for_check`, `commit_for_check`); the new `api/tungsten-cli.txt` (step 1); `just template-check`'s flow and floor (step 5). Each in its evidence row and the `CHANGELOG.md` line.
- **Must not move:** the determinism hash, the pinned containment hash, the row digests, `gpu-visual.png`, the post and transition regressions, the kit's determinism hash, `DefaultPlugins`' schedule, `api/tungsten.txt`, `api/tungsten-render.txt`, `api/tungsten-kit.txt`, `api/tungsten-kit-components.txt`, the template's `src/` and `tests/fixtures/title.png`, `just notices`' crate counts, `release.yml` and `release.py`.

## 7. Stop conditions

- A done-when check fails: restore the step's files from their copies, mark the step skipped, and continue with steps that don't depend on it.
- Step 2 or 3 skipped: both are Must, so the run finishes the steps that do not depend on it and ends before step 7, which a release session takes after the owner has seen why (workflow §3). The register row says so.
- Step 4 skipped: `new` is Should. Before the release, `new` leaves the parser, the usage, `api/tungsten-cli.txt`, decision 182 and the template's and guide's text. Step 5 keeps the outside check's `cp -R`, path rewrite, profiles and lock, adding `check` and `package` after them. RC-A7 and w14's follow-up 1 stay open, and the register row says so.
- Step 1's API critique finds a defect in the API: step 1 changes before step 2 starts, as the skill says.
- A step would change what a frame runs when no watched file has changed (the stages, the command flush, extract, render) or `App::run_frame` itself: stop that step. This plan captures no perf row because only startup, loads and the CLI change. Hot reload's manifest reload and staged commits do run inside a frame (`crates/tungsten/src/app.rs:1679`, `crates/tungsten/src/asset_loader/reload.rs:336,488`), and step 1 changes their shared bodies on purpose, held by `cargo test -p tungsten --locked`. No perf row edits a file.
- A step needs a crate outside the lockfile: stop that step; it needs a `D-015` entry this plan does not hold.
- The LLM index cannot hold the CLI's route without dropping a path or a decision: skip that edit and record a follow-up.
- A must-not-move invariant changes: restore that step.
- The outside check fails on its free-space floor alone: delete `target/debug/incremental` as its message says, never another session's files, and rerun once.

## 8. What the milestone owes

| Workflow §5 line | Step | Note |
| --- | --- | --- |
| 1 Evidence | every | |
| 2 Gates by change | 1–5, release | `just deps` in step 1; `just udeps` in steps 2–4; `just template-check` in step 5. No perf row: the frame body is untouched (§7). The release runs `just check`, `just smoke`, `just script-test`, `just notices`, `just template-check` and the rest once, after the cut |
| 3 Invariants | 1–5, release | §6; smoke and its digests at the release |
| 4 Decisions | 1 | 182 and 183 with their index rows and the three markers, before any doc cites them |
| 5 API ([break ledger](../../1.0/w04-api-freeze.md#break-ledger), rustdoc, `just api` snapshot) | 1 | One break, `ManifestError::OutsideFolder`, with its ledger row; a row of additions for the freeze; the core items with rustdoc; the CLI's surface in `api/tungsten-cli.txt` |
| 6 Template and guide | 1, 5 | The template's text in step 1, the guide in step 5 |
| 7 Routes (`docs/LLM_INDEX.md`, `just ctx`) | 5 | The template and CLI row; the selector rule; the template's budget |
| 8 Design | 6 | `### Project CLI — M70`, and DESIGN's outside-check sentence |
| 9 Records (known issues, backlog, gap log, register, workstream file, README's four owned Now fields) | 6, release | Known issues and backlog rows, w14, Q20 in criteria and the catalog. The gap log has no rows (the game has not started). `python3 -B scripts/roadmap.py summary` supplies the owned fields |
| 10 Close (one `CHANGELOG.md` line, `status: done`, the plan archived) | release | Then the cut, the checks and the preflight; the session ends with both blocks and the next prompt |

## 9. Critique

Reviewer: the `critique` skill with `--repo`, `gpt-6-astra`, on this plan with §9 and the evidence log cut by `sed`, the tree at `301f95c` plus this file.

Round 1, 2026-10-20 01:22 UTC: effort max, context 872,000, output cap 4,000, 482.8 s, 1,261,422 tokens in (1,147,648 cached), 16,592 out (11,457 reasoning), 40 commands, 390.4 KB output (largest 48.8 KB), 5h window 0% → 31%, week 46% → 50%. Seven findings, each checked against the lines it cites, all accepted. They changed done-when checks in steps 1–5, so round 2 runs.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 1 | [HIGH] Packaging checks the source tree, not the files it ships: a manifest path outside `assets/` passes the check and is left out of the archive | `crates/tungsten-core/src/assets/manifest.rs:589` resolves an entry against its manifest's folder with no bound; §2.6 staged `assets/` alone | accepted | §2.6 reordered: stage, split, then the staged binary checks the stage (its step 6), so a file outside it fails the package; §2.10 and step 5 add a `../outside.png` case to the outside check |
| 2 | [MEDIUM] A scene whose `entities` key is misspelled is skipped as no scene, though `SceneData` refuses it and example 03 panics on it | `crates/tungsten-core/src/assets/scene.rs:48` (`deny_unknown_fields`), `examples/03_scene_state/src/states.rs:115-116` (`expect`) | accepted | §2.3's scenes and Q6: every unlisted `.json` under a root's folder loads as a scene; the template's seven JSON files are all listed, so it stays clean; step 2's seeded set gains a misspelled key |
| 3 | [MEDIUM] Headless, no sprite is registered, so a game's asset check that looks a manifest sprite up in `AssetRegistry` fails where the window's startup passes | `crates/tungsten/src/asset_loader/atlas.rs:322-330` (`register_sprite` at startup), `crates/tungsten-core/src/component_registry.rs:598-602` (`register_component_with_assets` gets the World), `crates/tungsten-core/src/assets/registry.rs:98-102` (`sprite` is `None` until registered) | accepted | §2.3 row 2 registers each decoded sprite with a placeholder handle (`TextureHandle(u32::MAX)`, `UvRect::FULL`), and the sprite references read the registry as `load_all` does; step 2 gains the custom-check test |
| 4 | [MEDIUM] The package inherits a config file's `target-cpu=native`, which a smoke run on the build machine cannot catch | `.cargo/config.toml:5-6,16-19`; `.github/workflows/release.yml:153` sets the level per build | accepted | §2.6 step 3 builds with a `--config` array (`target-cpu=x86-64` and the build-id) that cargo joins after the game's flags, probed with a scratch crate (step 3); step 1 of §2.6 refuses `RUSTFLAGS`; Q17; §2.10 and step 3 build into `target/package-check`, away from `just perf`'s `target/release` |
| 5 | [MEDIUM] `image::open` alone misses the atlas's sibling-size check | `crates/tungsten/src/asset_loader/atlas.rs:72-99` (`decode_sprite_siblings`), `:363-368` (logged as an error, the sprite drawn unlit) | accepted | §2.3 row 2 and its error list use `decode_sprite_siblings`, made `pub(crate)`; `atlas.rs` joins step 2's files; the seeded set gains a normal map of another size |
| 6 | [MEDIUM] Copying the lock byte for byte leaves `tungsten-template-basic` in it, so step 4's token search cannot pass | `Cargo.lock:3067`, the name's one occurrence | accepted | §2.7 item 4 renames that entry as the other files are; step 4 compares the lock with the workspace's after that one rename |
| 7 | [MEDIUM] The stop condition on anything a frame runs blocks step 1, since hot reload runs the shared loader bodies inside a frame | `crates/tungsten/src/app.rs:1679`, `crates/tungsten/src/asset_loader/reload.rs:336,488` | accepted | §7 narrows the condition to what a frame runs when no watched file changed and names the reload path step 1 changes on purpose; step 1's done-when adds `cargo test -p tungsten --locked` |

Round 2, 2026-10-20 01:36 UTC, with a focus note naming round 1's changed sections and asking how its fixes hold together: effort xhigh, `--force` (62,266 characters after the cut, past the 60,000 guard, confirmed by the skill's step 7), 250.1 s, 496,676 tokens in (418,944 cached), 8,906 out (5,268 reasoning), 19 commands, 210.2 KB output (largest 57.9 KB), 5h window 31% → 48%, week 50% → 53%. Six findings, each checked, all accepted; every one is a gap a round-1 fix left. No third round runs (the skill's step 7); the fixes below were checked against each other before the records step.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 2.1 | [HIGH] The scene rule loads the manifest roots themselves as scenes, so the template's clean check fails on `assets/manifest.json` | `templates/basic/src/game.rs:12` (`MANIFEST`), `crates/tungsten-core/src/assets/scene.rs:48`; round 1's rule in §2.3 excluded nothing | accepted | §2.3's scenes leave out every manifest root; step 2's seeded test asserts that no root loads as a scene |
| 2.2 | [HIGH] Running from the stage bounds nothing: an absolute path into the source folder still loads there | `crates/tungsten-core/src/assets/manifest.rs:593,600` (`join`, then `canonicalize`, with no bound) | accepted | §2.4: `load_for_check` takes the folder and reports a root or entry outside it as `ManifestError::OutsideFolder`, a new variant as M61 added two; §2.3's row 1 and error list; §2.6 steps 4 (copies follow links) and 6; §2.10 item 4 adds the absolute case; steps 1 and 2 test both forms |
| 2.3 | [HIGH] A config's `target-feature` survives the joined `--config` array, so the binary is not portable | §2.6 step 3 kept "a target feature among them"; the round-1 probe showed the join; `DECISIONS.md:534-540` (`D-072`'s CPU levels) | accepted | §2.6 step 3 sets `RUSTFLAGS`, which replaces every config file's flags (the same probe: a set `RUSTFLAGS` dropped both arrays), as `release.yml` does; §2.6 step 1 refuses one already set; Q17; step 3's tests and done-when; §2.10 unsets both variables for its package runs |
| 2.4 | [MEDIUM] The cascade rule says a failed entry is left out, but `commit_for_check` keeps a prefab that fails its checks, so references to it pass | `crates/tungsten-core/src/assets/data.rs:587-597` (`add_prefab` only parses), `:1020-1029` (`check_prefab_id` checks existence) | accepted | §2.3 and Q12: cascades come only from files that do not read, parse or decode; an entry that fails a check stays; step 2 adds a data entry naming the failing prefab, which adds no error |
| 2.5 | [MEDIUM] `decode_sprite_siblings`, made `pub(crate)`, returns the private `SpriteSiblings` | `crates/tungsten/src/asset_loader/atlas.rs:63-66,72-77` | accepted | §2.5 and step 2: `SpriteSiblings` becomes `pub(crate)` as well |
| 2.6 | [MEDIUM] The failing package runs of the outside check rebuild `target/release` | §2.10 item 4 ran `tungsten package` without item 3's target folder | accepted | §2.10: every package run uses `target/package-check` |

Checking the fixes against each other after round 2 changed three more places. §2.10 now unsets `RUSTFLAGS` for its package runs, which 2.3's refusal would otherwise turn on a shell's flags. Step 3's refusal check now runs from `templates/basic`. And §2.4 had placed the core items outside the promise: the umbrella re-exports core whole (`crates/tungsten/src/lib.rs:69`), so `OutsideFolder` is a break. Step 1 now adds its ledger row and a row of additions, as `D-115`'s and `D-157`'s variants and M49's additions have, and §8 says so.

### Step 1 API critique

Run 2026-10-19 on the `api/` and `templates/` diff, untracked files included (`critique --raw --repo --force`, `--force` confirmed by the skill): effort xhigh, context 872,000, output cap 4,000, 76.2 s, 245,957 tokens in (197,760 cached), 2,622 out (864 reasoning), 17 commands, 130.5 KB output (largest 21.8 KB), 5h window 48% → 56%, week 53% → 54%. Two findings, each checked; neither is an API defect, so step 1 stands.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| C.1 | [HIGH] The template's text and the help name commands that are still stubs | `tools/cli/src/main.rs` routes `new`, `check` and `package` to `not_built` | rejected | By the plan's order: steps 2–4 build the three commands before the release, and §7 removes the template's and guide's `new` text if step 4 is skipped; the release ships no stub |
| C.2 | [MEDIUM] The README points at a getting-started guide that does not install the CLI | `docs/getting-started.md:12-55` still copies by hand | rejected | Step 5 rewrites the guide's start section with the `cargo install` line (§2.9) before the release |

## Evidence log

| Step | Date | Verdict | Key numbers | Paths |
| --- | --- | --- | --- | --- |
| 1 | 2026-10-19 | pass | `cargo fmt --all -- --check` exit 0; `for_check` 8 passed, 0 failed; `assets::manifest::tests assets::data::tests` 89 passed, `--test manifests` 5 passed; `cargo test -p tungsten --locked` 0 failed (679 lib); `tungsten-cli` 12 passed (9 unit, 3 binary); `--help` equals §2.1 and `cmp api/tungsten-cli.txt` exit 0; `frobnicate` → usage on stderr, `exit 2`; `cargo doc` collision: no line; `just lint`, `just deps` exit 0; `api/` diff: `api/tungsten-core.txt` alone, untracked `api/tungsten-cli.txt`; `load_for_check(` 3 = `load_and_merge_many(` 3, `commit_for_check(` 3 = `commit(` 3; `D-18[23]` headings 2, index rows 2, `Amended by D-183` 3; ledger `OutsideFolder` 1, additions row 1; `tungsten check` 2 in `AGENTS.md` (12,201 B), `tungsten new` 1 in `README.md`, `.gitignore` `/target` `/dist`; template tests 0 failed; `just repo-check`, `just ctx` exit 0. API critique C.1–C.2 rejected (§9) | `tools/cli/`, `crates/tungsten-core/src/assets/{manifest,data}.rs`, `api/tungsten-{core,cli}.txt`, `DECISIONS.md` D-182–183 |
| 2 | 2026-10-19 | pass | fmt exit 0; `asset_loader::check::tests` 3 passed; `app::tests::run_check` 2 passed; `cargo test -p tungsten --locked` 0 failed (684 lib); `tungsten-cli` 17 passed; the template: `tungsten check: no errors`, `exit 0`; seeded copy through the template's binary: `exit 1`, report files `assets/broken_scene.json`, `assets/data/cue_rules/labels.json`, `assets/manifest.json`, `assets/prefabs/gem.json`, `assets/prefabs/hazard.json`; `TUNGSTEN_CHECK` 3 in `app.rs`, no `run_frame` diff line; template tests 0 failed; `just lint`, `just udeps` exit 0. `run_check` takes the game's folder as a parameter (`App::run` passes the current folder) so its tests need no `set_current_dir`; a check run also caps `App::new`'s log level at warnings; scenes exclude the files each root names alone, so an entry the merge dropped as a repeated ID is not read as a scene | `crates/tungsten/src/asset_loader/check.rs`, `crates/tungsten/src/app.rs`, `tools/cli/src/{cargo,check}.rs` |
| 3 | 2026-10-19 | pass | fmt exit 0; `tungsten-cli` 39 + 3 passed, 25 in `notices`, `elf` and `package` (the drift test read `release.py`'s `PREFERENCE`, `MARKERS` and `EMBEDDED_WORKS` through `python3` and matched); the template's `tungsten package` printed `RUSTFLAGS="-C target-cpu=x86-64 -C link-arg=-Wl,--build-id"`, built in 1m 41s into `target/package-check`, checked the stage clean, wrote both archives under `templates/basic/dist/`, `exit 0`; with `RUSTFLAGS` set: refused before building, `exit 1`; player archive: one top folder with the binary, `tungsten.json`, `input.json`, `assets/manifest.json` and both notice files, no `.debug`; debug archive: `<top>/tungsten-template-basic.debug` alone; notices: `^tungsten 0.89.0$\|^tungsten-core 0.89.0$` 2, `JetBrains Mono` 2, first line `Third-party notices: tungsten-template-basic 0.1.0 for x86_64-unknown-linux-gnu`, 210 crates; extracted binary `TUNGSTEN_SMOKE_FRAMES=3` `exit 0`; `target/release` holds no template binary; after `rm -rf templates/basic/dist` the template shows step 1's three files alone; `just lint`, `just udeps` exit 0 | `tools/cli/{build.rs,src/{notices,elf,package}.rs}`, `target/package-check/` |
| 4 | 2026-10-19 | pass | fmt exit 0; `tungsten-cli` 44 + 4 passed (`new` tests: every template file under the game's names, both pins, the profiles, `.gitignore`, the lock, an existing folder refused with nothing written, the name rule, the embedded list equal to `git ls-files --cached --others --exclude-standard templates/basic`, since step 1's `.gitignore` is not yet tracked); `tungsten new "$tmp/my-game"` then `rg -c 'tag = "v0.89.0"'` → 2; token search: no file (rg exit 1); the lock `cmp` against the workspace's with one entry renamed: exit 0; `diff -rq`: `Cargo.toml`, `tungsten.json`, `src/main.rs` and the ten `tests/game/` files differ, `Cargo.lock` only in the game; a second `new` names the existing folder, `exit 1`; `just lint`, `just udeps` exit 0 | `tools/cli/{build.rs,src/new.rs,tests/cli.rs}` |
| 5 | 2026-10-19 | pass | `just template-check` exit 0: `Free space … 200.8 GiB (floor 6 GiB)`, then `OK` for the copy (`tungsten new`, nothing above it), the pin (tag v0.89.0, twice), the clean check, the build, the tests, the smoke run, the package (both archives, the notices), the archive's smoke run, the two files outside the stage, the five seeded content errors and the two seeded config errors, then `Template check passed: …`; `just script-test` exit 0 (`test-check-repo.py` 51 tests); `just ctx` exit 0, self-test `OK` with `oversize template rules` and `template rules too long` among its cases; `docs/LLM_INDEX.md` 12,262 B, `tools/cli/src/` 1; `just checks tools/cli/src/main.rs` recommends `cargo test -p tungsten-cli --locked` and `just template-check`, no `UNCLASSIFIED`; `cargo install --locked --git` 1 line and `tungsten new|check|package` 4 in the guide; `just repo-check` exit 0 | `scripts/template-check.sh`, `scripts/check-agent-context.py`, `scripts/check-repo.py`, `docs/{getting-started,validation,releases,LLM_INDEX}.md` |
| 6 | 2026-10-19 | pass | `### Project CLI — M70` at `DESIGN.md:613`; `template-check.*tungsten new` 1; known issues: the three phrases 0 (the template budget entry gone, the launcher's home W7b, the tileset entry W8b's extract warning alone, the outside check's lock entry now the offline path pin); backlog `tungsten package` 2; `done in M70` at `w14-tooling.md:52`, follow-ups 1 and 2 closed in italics; Q20 answered by its default in criteria §10, `q20` state `default`; `just repo-check`, `just ctx` exit 0 | `DESIGN.md`, `docs/known-issues.md`, `docs/plans/1.0/{backlog-1.x,w14-tooling,criteria}.md`, `docs/plans/1.0/roadmap.json` |
| 7 | 2026-10-19 | pass | Workflow §5 closed (§8): every step's evidence row above; decisions 182 and 183 with their index rows and markers (step 1); the break ledger's two rows, rustdoc on the four core items and `just api` (step 1); the template's text (step 1) and the guide (step 5); the routes and budgets (step 5); `DESIGN.md`'s M70 section and status line; records (step 6), the register row and the `CHANGELOG.md` `[Unreleased]` entry; `status: done`. The plan is archived before the cut, so the cut, every release check and the preflight are quoted in the session's report, not here | `CHANGELOG.md`, `docs/plans/1.0/implementation-plan.md` |

## Follow-ups

- The launcher's invisible Windows errors (A26) stay with W7b; this plan only rewords their home.
