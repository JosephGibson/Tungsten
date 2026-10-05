# Phase 5 milestone 35: logs, crash reports and a Windows test job (W11a with W7a) — done

- **status:** done
- **goal:** A game names itself in `tungsten.json` and gets a per-user folder. `App::new` installs one engine logger, which applies `logging.level`, writes one file per run and is the logger W1 M5's console will read, and a panic hook whose crash file symbolizes from per-tag debug files. Windows release builds open no console, and CI gains a CPU-only Windows test job (W7a).
- **non-goals:** Settings, the shared atomic write and corrupt-file fallback (W11b); save slots (W11c); the log console and its channel sink (W1 M5); crash files for signals or aborts below Rust (1.x, w11); resolving assets relative to the executable; the template and the guide (W12a, W9b); platform tiers and a macOS build (W7b, Q3); licence notices (W9a); `tungsten package` (W14b).
- **files to touch:** Step 1: `crates/tungsten-core/src/config.rs`, `crates/tungsten-core/src/tests/config.rs`, `crates/tungsten-core/src/lib.rs`, `crates/tungsten/src/user_dir.rs` (new), `crates/tungsten/src/tests/user_dir.rs` (new), `crates/tungsten/src/lib.rs`, `DECISIONS.md`, `docs/DECISION_INDEX.md`. Step 2: `crates/tungsten/src/logging.rs` (new), `crates/tungsten/src/tests/logging.rs` (new), `crates/tungsten/src/app.rs`, `crates/tungsten/src/tests/app.rs`, `crates/tungsten/src/lib.rs`, `crates/tungsten/Cargo.toml`, `crates/tungsten-core/src/config.rs`, `crates/tungsten-core/src/display.rs`, `crates/tungsten-core/src/tests/config.rs`, `crates/tungsten-core/src/tests/display.rs`, `Cargo.lock`. Step 3: `crates/tungsten/src/crash.rs` (new), `crates/tungsten/src/tests/crash.rs` (new), `crates/tungsten/build.rs` (new), `crates/tungsten/src/app.rs`, `crates/tungsten/src/lib.rs`, `Cargo.toml`, `Cargo.lock`, `tools/crash-probe/Cargo.toml`, `tools/crash-probe/src/main.rs` and `tools/crash-probe/tests/probe.rs` (new). Step 4: `examples/0{1,2,3,4}_*/src/main.rs` and `Cargo.toml`, `tools/launcher/src/main.rs`, `tungsten.json`, `Cargo.lock`. Step 5: `scripts/release.py`, `scripts/test-release.py`, `docs/releases.md`, `DECISIONS.md`, `docs/DECISION_INDEX.md`. Step 6: `scripts/crash-report.py` and `scripts/test-crash-report.py` (new), `justfile`, `.github/workflows/release.yml`, `docs/releases.md`. Step 7: `.github/workflows/ci.yml`, `DECISIONS.md`, `docs/DECISION_INDEX.md`. Step 8: `DESIGN.md`, `docs/LLM_INDEX.md`, `docs/known-issues.md`, `docs/perf/profiling-workflow.md`, `docs/plans/1.0/w11-shipping-basics.md`, `w07-platforms.md`, `w12-template.md`, `w04-api-freeze.md`, `criteria.md` (§10, Q14), `roadmap.json` (`q14`), `api/tungsten-core.txt`. Step 9: the skeleton's release list.
- **ordered steps:** (1) Game identifier and user folder, `D-119`. (2) Engine logger; config-load warnings kept. (3) Crash report, the crash probe and the test panic. (4) Examples, launcher and captures. (5) Packaging keeps debug files apart, `D-120`. (6) Crash symbolization and the release probe. (7) W7a: CPU-only Windows test job, `D-121`. (8) Docs, routes, API snapshot and records. (9) Release 0.48.
- **done-when:** Every step's evidence row quoted; workflow §5 closed; the release checks pass after the cut, `just visual` having run in step 4; the preflight prints the command block and the post-merge block; the owner's checks (§4 Q8–Q10) are listed with the hand-off.

Candidates W11a and W7a, a pair under `D-108` ([implementation plan](../../1.0/implementation-plan.md) §3 card, level B), milestone M35, release 0.48. Written 2026-10-04 at `71b00be`. Runs under the [tungsten-milestone skill](../../../../.claude/skills/tungsten-milestone/SKILL.md) and [workflow](../../1.0/workflow.md) §4–§6; this plan records only where it differs.

## Context digest

- W11a is next in Track A, after W1 M0a and before W9a with W12a. W12a needs its game identifier and a `main.rs` without logging setup, W1 M5 its logger (amendment 4), and W14b's `package` its debug-file handling. W7a's CPU-only Windows job tests the folder paths and the crash hook beside it (amendment 9). Scope: [w11](../../1.0/w11-shipping-basics.md) step 1 and criteria §8.1 (W7); W7 has not graduated, so §3 step 7 holds W7a's done-when.
- Owner question Q14 (logs on by default, or one opt-in call) is due now; §4 Q1 carries it with a default.
- It stands on `D-008` (config: an invalid file is fatal), `D-015` (no new crate: env_logger 0.11 is already locked), `D-070` (CI informational), `D-071`, `D-072` and `D-079` (release archives, levels, triggers), `D-105` (no commits) and `D-110` (the headless harness builds its `App` with `App::new`).
- The hard parts are ordering and isolation. `log` holds one logger per process and std one panic hook, both set by `App::new`. Tests build `App`s, captures must stay byte-identical, and `Config::load` logs before any `App` exists. §2 settles each.

## 1. Audit

Read at `71b00be`.

| # | Finding | Evidence |
| --- | --- | --- |
| A1 | Each example installs its own logger first: `env_logger::init()`, then `Config::load`, `App::new`, `run`; each has an `env_logger` dependency. A logger set before `App::new` wins under w11's order, so while these calls stay, the engine's logger never runs in an example | `examples/01_platformer/src/main.rs:38-45`, `02_bench/src/main.rs:38-71`, `03_scene_state/src/main.rs:70-110`, `04_shader_playground/src/main.rs:128-235`; their `Cargo.toml:12-13` |
| A2 | `logging.level` is parsed (default `"info"`) and read nowhere | `crates/tungsten-core/src/config.rs:291-307`; no reader outside that file |
| A3 | `Config` has `window`, `display`, `render` and `logging`, no game identifier; `WindowConfig` holds title, size and vsync. `Config` derives `Default` and `Deserialize`, has only public fields, and nothing builds it by literal | `config.rs:60-70`, `309-320`; core re-exports it at `crates/tungsten-core/src/lib.rs:41` |
| A4 | `Config::load` logs before any `App` exists: a missing file, two legacy conflicts and about ten display fallbacks, which `DisplayConfig`'s own `Deserialize` raises while parsing | `config.rs:324-369` (`358`, `577-602`); `crates/tungsten-core/src/display.rs:173-206`, warnings `175-337` |
| A5 | `App::new` resolves the display and loads the action map first, and both log; smoke mode is `TUNGSTEN_SMOKE_FRAMES` parsed to n > 0 | `crates/tungsten/src/app.rs:160-163`, `258-261`, `1396-1430` |
| A6 | Tests build `App` with `App::new(Config::default())` at 17 sites; the harness wraps an `App` built that way | `crates/tungsten/src/tests/app.rs:20` and 11 more, `crates/tungsten/src/tests/testing.rs:19`, `examples/01_platformer/src/tests/{main.rs:75,246, ball_pit.rs:156, presentation.rs:613}`; `crates/tungsten/src/testing.rs:54` |
| A7 | Perf lines are `log::debug!` records in `run_frame`. The bench logs `bench-config` after `App::new` and before `run`. The runner sets `RUST_LOG=tungsten::app=debug,bench=debug`, merges stdout and stderr into the run log and strips env_logger's `[…]` prefix. A capture without `bench-config` is invalid | `app.rs:1155-1187`; `02_bench/src/main.rs:67-71`, `counters.rs:35-37`; `scripts/bench.py:41-44`, `263-279`, `307`; `scripts/bench_report.py:27-28`, `371` |
| A8 | Smoke, the pixel tests and every capture set `TUNGSTEN_SMOKE_FRAMES`. Smoke runs debug builds through `cargo run` and checks only exit codes and time | `scripts/smoke-examples.sh:63-65`; `examples/02_bench/tests/visual_regression.rs:26-38`; `03_scene_state/tests/transition_regression.rs:22-26`; `04_shader_playground/tests/post_regression.rs:20-29`; `bench.py:269` |
| A9 | No binary sets `windows_subsystem`. On Windows the launcher waits on the example (`Command::status`) and prints its level on stderr; on Unix it `exec`s, so the example's `/proc/self/exe` is the `bin/<level>/` build | `tools/launcher/src/main.rs:33-49`, `127-146` |
| A10 | Release runs strip with `CARGO_PROFILE_RELEASE_STRIP=debuginfo` over a profile with `debug = 1`, `strip = "none"` and `panic = "abort"`. The artifact takes `example-*` and the launcher, not PDBs. `package` copies binaries verbatim, and its README promises a console line | `.github/workflows/release.yml:112-116`, `151-157`; `Cargo.toml:94-100`; `scripts/release.py:276-298` (`293`), `307-349` |
| A11 | Release runs build only (`D-071`): two CPU levels, so 8 example binaries per target plus the launcher. `SHA256SUMS` covers `dist/*`. The docs expect two archives and a console line | `release.yml:10`, `112`, `189`; `docs/releases.md:114-125` |
| A12 | A rehearsal tag on a cut tree passes `check` and gets "No unreleased changes recorded" as notes | `scripts/release.py:92-95`, `191-206`, `236-243` |
| A13 | `test-release.py` packages fake text binaries, so any real `objcopy` call in `package` must be injectable | `scripts/test-release.py:249-302` |
| A14 | CI is one Linux job; no `.gitattributes`. Only the Linux target is installed on the reference machine | `.github/workflows/ci.yml:33-104`; `~/.rustup/toolchains/1.98.1-*/lib/rustlib/` |
| A15 | Library crates hold no `unsafe` and no build script; tests set environment variables in `unsafe` blocks (edition 2024) | `crates/tungsten-core/tests/display.rs:53`, `181-184` |
| A16 | Every package takes the workspace version (0.47.0), so an example's version is the engine's | `Cargo.toml:2`; `examples/01_platformer/Cargo.toml:3` |
| A17 | env_logger 0.11.11 is locked. `Target::Pipe` gets one `write_all` and `flush` per record under env_logger's mutex; `is_test` writes through `eprint!`; with no directive the filter is `error` | `Cargo.lock:746-747`; env_logger `src/writer/buffer.rs:84-100`, `src/logger.rs:484-498`; env_filter 1.0.1 `src/filter.rs:144-148` |
| A18 | Errors that `App::new` and `App::run` return are not logged: the action map's parse error, `EventLoop::new` and `run_app`. The fatal errors stored in `fatal_error` are logged where they are set | `app.rs:1412`, `390-391`; `1563-1564`, `1626-1627`, `1641-1642` |
| A19 | Records: `DESIGN.md` names env_logger in the stack and says `logging.level` is unapplied; known issues carry that follow-up and the hosts-missing list; w01 §11 sketches the console's channel sink | `DESIGN.md:42`, `113-148`; `docs/known-issues.md:26-34`, `48`; `docs/plans/1.0/w01-ui-text-suite.md:471`, `530` |
| A20 | On the reference machine, debug and release binaries carry a 20-byte GNU build-id: the system gcc that drives the link is built with `--enable-linker-build-id`. The runner's compiler is unchecked, hence §2.7's explicit flag | `readelf -n target/debug/example-01-platformer`, `target/release/example-02-bench`; `gcc -v` |

## 2. Design

### 2.1 The game section

`Config.game: GameConfig { id: Option<String>, version: Option<String> }`, `#[non_exhaustive]` per the break ledger, `#[serde(default)]`, with rustdoc. `GameConfig::validate()` rejects a `game.id` that is not 1–64 ASCII letters, digits, `.`, `_` or `-` starting with a letter or digit, or that is a Windows device name (`CON`, `PRN`, `AUX`, `NUL`, `COM1`–`COM9`, `LPT1`–`LPT9`, any case, any extension). The rejection is `ConfigError::InvalidValue { field: "game.id" }`, fatal under `D-008`. `Config::load` calls it, and so does `App::new`, because a config built in code never passes through `Config::load`; `user_dir` also refuses to build a path from an id that fails it. `game.version` is free text that crash files print. `logging.level` must parse as `log::LevelFilter` (`off` … `trace`, any case), else `InvalidValue { field: "logging.level" }`. The root `tungsten.json` gains `"game": { "id": "tungsten-examples" }` in step 4: the four examples share it until W12a gives each its own folder. `Config::default()` has no id.

### 2.2 The user folder

`crates/tungsten/src/user_dir.rs` (private): `UserDirs { settings, saves, logs }` from `resolve(game_id, smoke, platform, env)`, a pure function. `env` is a lookup closure, so Linux tests cover all three platforms without `set_var`. The rules, in order:

1. In smoke mode, no folder, whatever else is set. Captures, smoke runs and the pixel tests then neither read nor write user files (RC-S3) with no script change (A8).
2. `TUNGSTEN_USER_DIR` set: empty means no folder; otherwise `<dir>/settings`, `<dir>/saves` and `<dir>/logs`, a relative path against the working directory.
3. With no `game.id`, no folder. Tests (A6) stay as they are today.
4. Linux and other Unix: `$XDG_CONFIG_HOME/<id>`, `$XDG_DATA_HOME/<id>/saves` and `$XDG_STATE_HOME/<id>/logs`. An unset, empty or relative variable falls back to `$HOME/.config`, `.local/share` and `.local/state`; with no usable `HOME`, no folder. Windows: `%APPDATA%\<id>`, `%APPDATA%\<id>\saves` and `%LOCALAPPDATA%\<id>\logs`. macOS: `~/Library/Application Support/<id>`, `…/<id>/saves` and `~/Library/Logs/<id>`.

Only the logs folder is created here; W11b and W11c create theirs. A game with an id but no folder gets one warning once the logger is up.

### 2.3 The engine logger

`crates/tungsten/src/logging.rs` (private). It is the first thing `App::new` does, since its own records (A5) and the bench's `bench-config` line (A7) must reach it.

- **Filter.** When `RUST_LOG` is set, it alone is parsed, as `env_logger::init()` parses it today. Otherwise the filter is `logging.level`.
- **Sinks.** Stderr in debug builds. In release builds, stderr only while `RUST_LOG` is set (amendment 19: the runner keeps reading a release build's stderr). The file whenever a folder exists. With neither, no logger is installed.
- **Wiring.** Stderr only: `Target::Stderr` with the default format, as today, plus `is_test(cfg!(debug_assertions))` so test output is captured. With a file: `Target::Pipe(Box<Tee>)`. env_logger hands the `Tee` one whole record per `write_all` (A17), and the `Tee` writes it to the file and, when enabled, to stderr. The format stays env_logger's default, whose prefix the perf parser strips. A capture's logging is therefore what it is today: release, `RUST_LOG`, stderr only.
- **Install.** `Builder::try_init()`. A `SetLoggerError` means a logger is already set, by the game or an earlier `App`; the engine leaves it, opens no file and deletes nothing.
- **File.** Opened only after a successful install, through an `Arc<OnceLock<File>>` that the `Tee` and the crash hook share: `<logs>/<stem>-<YYYYMMDDTHHMMSS.mmmZ>-<pid>.log`, where `<stem>` is the executable's file stem and the stamp is UTC from `SystemTime` (days-from-civil, std only). It is unbuffered, so a crash loses no record. After opening, only the 10 newest log files of this stem are kept, by name. A file counts only when the rest of its name after `<stem>-` is exactly a stamp, `-`, digits and `.log`, so `game-helper-…` never counts as `game`; crash files are never touched. A folder or file that cannot be created gets one stderr line and no file, never an error.
- **Returned errors.** `App::new` logs any error it returns at `error` before returning it, an invalid `input.json` among them (`app.rs:1412`). An invalid `game.id` has no folder to log into: `App::new` installs its logger without a file, logs the error (on stderr in debug builds, or with `RUST_LOG`), then returns it. `App::run` does the same for the errors of `EventLoop::new` and `run_app`; its fatal errors are logged where they arise (`app.rs:1563`, `1626`, `1641`). A Windows release build's returned errors then reach the log file, not only the stderr it lacks.
- **Console seam.** The `Tee` is the one fan-out point. W1 M5 adds a bounded `sync_channel` sink fed with `try_send` (drops counted), drained on the main thread by the console resource (w01 §11). `D-119` records that design, so M5 adds a sink, not a logger.

### 2.4 Config-load warnings

Moving the logger from `main` into `App::new` would drop `Config::load`'s warnings (A4). `Config` gains a private `#[serde(skip)] load_warnings: Vec<String>`. `Config::load` pushes the missing-file, conflict and display-fallback messages there instead of logging them, because the display parse helpers now take a `&mut Vec<String>`. `DisplayConfig`'s own `Deserialize` still logs what it collects, for direct deserialization. `pub fn take_load_warnings(&mut self) -> Vec<String>` (rustdoc) lets `App::new` log each at `warn` with target `tungsten_core::config` once its logger exists, so they reach the engine's logger or the game's alike.

### 2.5 The crash report

`crates/tungsten/src/crash.rs` (private), installed by `App::new` after the logger, when a folder exists. It installs once per process, guarded by a `std::sync::Once` (Q6): `take_hook`, then `set_hook` with a closure that writes the report and then calls the previous hook. A game hook set later replaces it unless it chains through `take_hook` (the W9b guide shows how). The hook ignores every I/O error, never panics and never logs through `log`, since the panicking thread may hold env_logger's mutex.

- **File.** `<logs>/<stem>-<stamp>-<pid>-crash.txt`, created with `create_new` and a counter suffix on a collision. One line naming it is appended to the run's log through the shared `OnceLock<File>`.
- **Contents.** Lines that `scripts/crash-report.py` parses:
  - `game`, `game_version`, `engine_version` (`CARGO_PKG_VERSION`), `target` (exported by a three-line `build.rs` from `TARGET`), `executable` (`current_exe`), `time`, `thread`, `message` (`payload_as_str`, else `Box<dyn Any>`) and `location`;
  - `anchor: 0x… tungsten::crash::anchor`, the runtime address of an `#[inline(never)]` function, for Windows (A15: no `unsafe` to ask for the module base);
  - `build_id`, the running executable's identity, read once at install from its file's headers with plain std reads: the GNU build-id note on Linux, and on Windows the CodeView record's PDB GUID, age and name;
  - `backtrace:` with `format!("{:#}", Backtrace::force_capture())`, which prints each frame's address;
  - on Linux, `modules:` with the file-backed lines of `/proc/self/maps`.
- **Limits.** The hook runs before `panic = "abort"` ends a release process. Only panics leave a file.
- **`TUNGSTEN_TEST_PANIC`** (Q7). When it is set and not empty, `App::new` registers an engine system `__test_panic` that panics in the first frame, for checking a shipped build. Unset, nothing is registered, so no frame does more work.
- **The probe.** `tools/crash-probe`, package `tungsten-crash-probe` (`publish = false`, depends on `tungsten` and `log`). Mode `panic`, the default, sets a hook that prints a marker, builds `Config::default()` with `game.id = "tungsten-crash-probe"` and `game.version` set to its package version, calls `App::new`, then panics two `#[inline(never)]` calls deep. Mode `log <level>` logs one record per level and exits 0. Mode `own-logger` installs a minimal logger first, then calls `App::new` and logs a marker. Mode `bad-input` calls `App::new` in a working folder whose `input.json` is malformed and exits with its error. Mode `bad-id` sets `game.id` to `../x` in code, then calls `App::new`. Its integration tests spawn it with `Command::env` and `current_dir`, so they need no `set_var`.

### 2.6 Examples, launcher and captures

The examples drop `env_logger::init()` and the dependency (A1) and gain `#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]`, as does the launcher. On Windows the launcher's stderr line then goes nowhere, since std ignores the missing handle. This lands w12's item (3), logging leaving `main.rs`, ahead of W12a. Captures keep stderr-only logging (§2.3) and no folder (§2.2), so `scripts/bench.py` and the pixel tests stay unedited.

### 2.7 Release symbols

- **Workflow.** `release.yml` drops `CARGO_PROFILE_RELEASE_STRIP`. The Linux row adds `-C link-arg=-Wl,--build-id`, so the build-id does not depend on the runner's compiler defaults, and the Windows artifact adds `release/example_*.pdb`.
- **Linux packaging.** In the publish job, `release.py package` splits each placed binary: `objcopy --only-keep-debug --compress-debug-sections=zlib`, then `objcopy --strip-debug --add-gnu-debuglink`. The binary's GNU build-id must equal its debug file's, read from the ELF notes in Python, and the debug file must hold a `.debug_line` section, else `ValueError`. A strip left in the workflow therefore fails packaging instead of shipping debug files with no lines. The launcher copies are stripped and their debug info dropped.
- **Windows packaging.** Each example's PDB, under the file name its CodeView record names (PE debug directory, read in Python), must match the record's GUID and age (PDB info stream, read from the MSF file), else `ValueError`.
- **Output.** The player archive keeps its layout and holds no `.debug` or `.pdb` file. `tungsten-debug-<tag>-<target>.tar.gz` or `.zip` holds `bin/<level>/<example>.debug` or `bin/<level>/<record name>.pdb` for both levels, under the player archive's top folder, so it extracts over the player archive and puts each debug file beside its binary. `SHA256SUMS` covers it. The prefix keeps `--pattern 'tungsten-examples-*'` downloads to the player archives, and `docs/releases.md`'s verification becomes `sha256sum -c --ignore-missing SHA256SUMS`, which still fails when no listed file is present, with the debug download as a separate command.
- **Splitter.** It is a parameter of `package`, so the tests inject a fake (A13). `release.py split-debug <binary> <debug-out>` exposes the same split.
- **README.** `README.txt` says where logs and crash files go on each platform, and its console sentence becomes Linux-only.

### 2.8 Symbolization

- **`symbolize`.** `scripts/crash-report.py symbolize <crash file> (--root <dir> | --binary <file> --debug <file>) [--expect <path fragment>]`. `--root` is a folder holding a player archive with its debug archive extracted over it; the crash file's `executable` path belongs to the crashing machine, so only its `bin/<level>/<name>` tail is used, to pick the binary and its debug file under the root.
  - Linux: a frame's module base is the start of its file's offset-0 mapping. Every frame but the first is a return address, so `addr − base − 1` goes to one `addr2line -C -f -i -e <debug>` call per module.
  - Windows: the binary is required. The anchor's public symbol in the PDB and the binary's section table give the anchor's RVA, and `llvm-symbolizer --relative-address --obj <binary>` takes `addr − anchor + anchor RVA`, finding the PDB beside the binary under the record's name.
  - It first checks identity: the crash file's `build_id` must equal the debug file's build-id (Linux) or the PDB's GUID and age (Windows), read with step 5's readers. A mismatch or a missing identity exits 1 before anything is resolved, so another build's symbols are never used.
  - It prints `N: function at file:line` and exits 1 when no executable frame resolves or `--expect` matches none.
- **`probe`.** `crash-report.py probe <release dir> --target <triple>` copies the probe and its PDB into a temp folder laid out like an archive, away from their build paths, splits the Linux copy, runs it with `TUNGSTEN_USER_DIR` in another temp folder, and symbolizes with `--root` and `--expect tools/crash-probe/src/main.rs`.
- **In `release.yml`.** The build job excludes the probe from the level loop, builds it once at the portable level, and runs `probe` on both runners with `continue-on-error: true` (Q8). `D-120` amends `D-071`'s "build only" to allow this CPU-only run.

### 2.9 W7a

A `ci.yml` job `windows-tests` on `windows-2025` runs `git config --global core.autocrlf false` before checkout (A14), then the Linux job's pinned checkout and toolchain actions, then `cargo test --workspace --locked` in the dev profile. The workflow's `RUSTFLAGS` already pins `x86-64`; the timeout is 90 minutes. It is informational: no branch protection (`D-070`, amended by `D-121`). It runs the folder, logger, crash-hook and probe tests on Windows.

### 2.10 Surface and seam

- **Core.** `GameConfig`, `GameConfig::validate`, `Config.game` and `Config::take_load_warnings` change `api/tungsten-core.txt`. A break-ledger row records that `Config` can no longer be built by literal outside core.
- **Umbrella.** No public change: the new modules are private, and `build.rs` only exports `TARGET`.
- **Seam.** Unchanged (`D-007`, `D-018`).
- **Template and guide.** Not applicable before W12a and W9b.

## 3. Steps

Each step's done-when quotes command output into the evidence log. `$S` is a fresh scratchpad folder.

### Step 1: Game identifier and user folder

- **Files:** step 1's list in the header
- **Change:** First write `D-119` with the tungsten-decision skill, from §2.1–§2.5 and the approved answers. Then add §2.1's `GameConfig`, `game.id` and `logging.level` validation and tests, and §2.2's `user_dir.rs` and tests.
- **Done-when:**
  - `cargo test -p tungsten-core --lib config` → passes. New cases, through `Config::load` and `GameConfig::validate` alike: ids `my-game` and `com.example.game` pass; empty, 65 characters, `../x`, `/abs`, `.x`, `a/b` and `con.txt` fail with field `game.id`; `logging.level` `"verbose"` fails with field `logging.level`; `"WARN"` loads.
  - `cargo test -p tungsten --lib user_dir` → passes for: Linux with XDG set, unset, empty and relative; no `HOME`; Windows; macOS; an override that wins; an empty override; smoke mode with an override; no id; an invalid id, which builds no path.
  - `rg -c '^## D-119' DECISIONS.md` → 1, and ``rg -c '^\| `D-119`' docs/DECISION_INDEX.md`` → 1.
  - `just repo-check` → passes.
- **Moves:** none

### Step 2: Engine logger; config-load warnings kept

- **Files:** step 2's list
- **Change:** §2.3 and §2.4. `App::new` begins by validating `config.game`, resolving the folder from it and smoke mode (none when the id is invalid), and installing the logger. An invalid id is then logged and returned. Otherwise it logs `take_load_warnings()` and the missing-folder warning, and only then runs today's body. `App::new` and `App::run` log the errors they return (§2.3).
- **Done-when:**
  - `cargo test -p tungsten --lib logging` → passes, covering:
    - the filter: `RUST_LOG` replaces the level; the level applies when it is unset;
    - the sinks for debug and release, with and without `RUST_LOG` and a folder;
    - the file name;
    - pruning: the 10 newest of a stem are kept, while files of `game-helper` beside `game`, files of other stems and crash files are left alone;
    - the UTC stamp at three known instants.
  - `cargo test -p tungsten --lib app` → passes, including `App::new` with `game.id` set in code to `../x`, which returns an error naming `game.id`.
  - `cargo test -p tungsten-core --lib` → passes; a missing file, a display fallback and a legacy conflict each come back from `take_load_warnings()`.
  - `just check` → passes.
- **Moves:** none

### Step 3: Crash report, crash probe and the test panic

- **Files:** step 3's list
- **Change:** §2.5: the hook, `build.rs`, `TUNGSTEN_TEST_PANIC`, and the probe with its tests, added to the workspace members.
- **Done-when:**
  - `cargo test -p tungsten --lib crash` → passes: the report is formatted from plain fields; the identity readers return the build-id of a synthetic ELF and the GUID, age and name of a synthetic PE, and nothing for files without them; the `TUNGSTEN_TEST_PANIC` parse is checked.
  - `cargo test -p tungsten-crash-probe` → passes:
    - Mode `panic` leaves one `*-crash.txt` under `<dir>/logs`. Its header has `game`, `game_version`, `engine_version` equal to the workspace version, `target` equal to the host triple, `executable`, `thread: main`, the message and `location: tools/crash-probe/src/main.rs:<line>`, plus an `anchor:` line and, on Linux and Windows, a non-empty `build_id:` line. The backtrace has a frame naming `tungsten_crash_probe`, and on Linux the `modules:` section lists the probe's path. The previous hook's marker is on stderr.
    - Mode `log info` writes one `.log` with the info and warn records and no debug record; with `RUST_LOG=warn`, only the warn record.
    - Twelve runs leave ten log files.
    - Mode `own-logger` prints the marker through the probe's logger and creates no log file.
    - Mode `bad-input`, run in a temp folder whose `input.json` is `{`, exits non-zero, and the run's log file holds the parse error.
    - Mode `bad-id` exits non-zero, its stderr names `game.id`, and no folder is created under the override.
    - `TUNGSTEN_SMOKE_FRAMES=1` leaves the folder empty.
  - `just check` → passes.
- **Moves:** none

### Step 4: Examples, launcher and captures

- **Files:** step 4's list
- **Change:** First, on the tree as step 3 left it, run `just perf run physics --repeat 2 --allow-background` and note its digest D0. The examples still install their own logger then, so the engine's stands aside. Then make §2.6's edits and add the root `game.id`.
- **Done-when:**
  - `rg -c 'env_logger' examples` → no matches, and `rg -l 'windows_subsystem' examples/*/src/main.rs tools/launcher/src/main.rs` → 5 files.
  - `TUNGSTEN_USER_DIR=$S TUNGSTEN_TEST_PANIC=1 cargo run -p example-03-scene-state` → exits non-zero. `$S/logs/example-03-scene-state-*.log` holds `Loaded action map`, and the `*-crash.txt` beside it has a `message:` line naming `TUNGSTEN_TEST_PANIC`.
  - `just perf run physics --repeat 2 --allow-background` → exit 0. `jq -c '{valid, d: .determinism.digests, f: [.runs[].frames_logged]}'` on its `capture.json` shows `valid: true`, both digests equal to D0, and the frame counts of the D0 capture.
  - `just smoke` → passes; `just visual` → passes.
  - `fd -I --changed-within 6h . "${XDG_STATE_HOME:-$HOME/.local/state}/tungsten-examples"` → no output, or no such folder: the runs above wrote nothing to the real user folder.
- **Moves:** none. A digest other than D0 is a stop condition.

### Step 5: Packaging keeps debug files apart

- **Files:** step 5's list
- **Change:** First write `D-120` from §2.7 and §2.8. Then add §2.7's readers, split, `package` outputs, `split-debug` subcommand, README text and tests, and update `docs/releases.md`: the assets, the console line, the debug archive, and the checksum command with `--ignore-missing`.
- **Done-when:**
  - `python3 -B scripts/test-release.py` → passes. New cases:
    - the build-id read from a synthetic ELF, a mismatch that raises naming the binary, and a debug file without `.debug_line` that raises;
    - the PDB GUID and age read from a synthetic PE and MSF pair, matching and not;
    - a Linux `package` with a fake splitter: the player archive holds no `.debug` or `.pdb`, and the debug archive holds `bin/<level>/<example>.debug` for both levels;
    - the Windows debug zip under the record's PDB names;
    - the README naming the log folders.
  - `cargo build --release -p tungsten-crash-probe`, a plain `cp` of the binary to `$S/p`, then `python3 -B scripts/release.py split-debug $S/p $S/p.debug` → exit 0. `readelf -S $S/p | rg -c '\.debug_'` → 0, and the `Build ID` lines of `readelf -n` on `$S/p` and `$S/p.debug` are equal. Both sizes are recorded.
  - `just script-test` → passes; `just repo-check` → passes.
- **Moves:** none

### Step 6: Crash symbolization and the release probe

- **Files:** step 6's list
- **Change:** §2.8: `crash-report.py` and its tests, the `just script-test` line, and `release.yml`'s strip removal, flags, artifact paths and probe step, with its header comment. `docs/releases.md` gains a crash-report check: download the Linux player and debug archives, extract both into one folder, run an example with `TUNGSTEN_TEST_PANIC=1`, then symbolize with `--root` and `--expect crates/tungsten/src/`.
- **Done-when:**
  - `python3 -B scripts/test-crash-report.py` → passes. Cases: a Linux crash file is parsed, with its module bases and the −1 rule; a fake `addr2line` runs once per module, and its output is parsed; an `executable` path from another machine maps to the binary and debug file under `--root`; a `build_id` that differs from the debug file's, or is missing, exits 1 before resolving; the Windows anchor arithmetic, and Windows refusing to run without a binary; `--expect` both matching and missing.
  - `cargo build --release -p tungsten-crash-probe && python3 -B scripts/crash-report.py probe target/release --target x86_64-unknown-linux-gnu` → exit 0. The crash file's `build_id` matches the split debug file's, and the output prints a frame `… at …/tools/crash-probe/src/main.rs:<line>` resolved from that debug file.
  - `rg -c 'CARGO_PROFILE_RELEASE_STRIP' .github/workflows/release.yml` → no matches.
  - `just script-test` → passes; actionlint reads the workflow.
- **Moves:** none

### Step 7: W7a, CPU-only Windows test job

- **Files:** step 7's list
- **Change:** First write `D-121`. Then add §2.9's job. Per Q10, install the MSVC target and cross-check the build.
- **Done-when:**
  - `just script-test` → passes.
  - `rustup target add x86_64-pc-windows-msvc && cargo check --workspace --all-targets --locked --target x86_64-pc-windows-msvc` → passes. If either cannot run, record it as not run, with the error.
  - `D-121` heading and index row, counted as in step 1.
  - The job's own done-when: its first run on the release pull request passes, or each failure is filed in known issues before the merge. The owner reads it (§4 Q10).
- **Moves:** none

### Step 8: Docs, routes, API snapshot and records

- **Files:** step 8's list
- **Change:**
  - `DESIGN.md`: the stack row, the config example with `game`, `logging.level` applied, and a "Logs and Crash Reports — M35" section.
  - Index rows:
    - logs, user folder and crash reports (`D-119`): `logging.rs`, `user_dir.rs`, `crash.rs`, `app.rs` and `tools/crash-probe/`;
    - the release row gains `scripts/crash-report.py` and `D-120`;
    - a CI row: `ci.yml`, `D-070` and `D-121`.
  - Known issues:
    - the follow-up line keeps only `display.scale_mode`;
    - the no-host list gains the Windows console and Windows PDB symbolization, plus the step 6 result if CI cannot be read yet;
    - new follow-ups: the launcher's errors are invisible on Windows, and a `Config::load` error before `App::new` reaches only stderr.
  - The profiling workflow's telemetry section: stderr in release builds while `RUST_LOG` is set, and no folder in smoke mode.
  - w11: step 1 landed in 0.48, its probe checks pending the rehearsal run (Q8, Q9). w07: W7a landed, with its done-when and its first run pending (Q10). w12: item (3) is done. w04: the ledger row.
  - criteria §10: Q14 moves to Answered. In `roadmap.json`, `q14` takes `state` `default` (or `answered`, if the approval changed Q1) and the answer, citing `D-119`.
  - `just api`.
- **Done-when:**
  - `just api`, then `git --no-pager diff --stat -- api/` → only `api/tungsten-core.txt`. `git --no-pager diff -U0 -- api/ | rg -c '^-pub'` → no matches.
  - `just ctx` → passes, with the index under 12 KiB.
  - `just repo-check` → passes.
  - `rg -c 'logging\.level' docs/known-issues.md` → no matches.
- **Moves:** none

### Step 9: Release 0.48

- **Files:** `CHANGELOG.md`, `DESIGN.md`, `Cargo.toml`, `Cargo.lock`, the register row, the README "Now" lines, this plan (moved to `docs/plans/archive/1.0/`)
- **Change:** Close workflow §5 and this step's evidence row with `status: done`. Then run [tungsten-finalize](../../../../.claude/skills/tungsten-finalize/SKILL.md) and [tungsten-release](../../../../.claude/skills/tungsten-release/SKILL.md) ([releases](../../../releases.md)): archive the plan, run `just release-cut 0.48.0` once, run every release check once after it (no further gate is owed: step 4 ran `just visual`, and `just physics-release` does not apply), then run the preflight with `--message`.
- **Done-when:**
  - The command chain of [releases](../../../releases.md) step 1 → every check passes, quoted in the report.
  - `just release-preflight 0.48.0 --repo JosephGibson/Tungsten --message 'Update 0.48: logs, crash reports and a Windows test job (W11a, W7a, D-119–D-121)'` → passes and prints both blocks.
- **Ends with:** the changed-file list, the command block, then the owner's checks in order:
  1. Q9's rehearsal block: tag `v0.48.0-test.<UTC date>.g<short sha>` at the release commit and push it.
  2. The run's two probe steps, decided per Q8.
  3. The crash-report check from `docs/releases.md` on its Linux archive.
  4. Q10's Windows job.
  5. The merge, or a hold with the reason. Results that differ from the records in step 8 go into known issues and w11 in the next session.

  Then the post-merge block, and then the plan prompt for W9a with W12a (M36).

## 4. Open questions

| # | Question | Default |
| --- | --- | --- |
| Q1 | Owner Q14: logs and crash reports on by default in `App`, or one opt-in call? | **On by default.** `App::new` installs the engine logger unless one is set. Where a user folder exists, it also installs the crash hook. A game opts out by installing its logger before `App::new` and its hook after it. No new API: an opt-in call would also need `Config::load`'s warnings handled before it, and every game would have to remember it |
| Q2 | The game section's shape | **`game.id` and `game.version`** (§2.1). Both are optional, and without an id there is no user folder. The examples share `tungsten-examples` and leave `version` unset, since theirs is the engine's |
| Q3 | Folder layout, override and captures | **§2.2:** XDG, `%APPDATA%`/`%LOCALAPPDATA%` and `~/Library` paths. `TUNGSTEN_USER_DIR` overrides them, and empty means none. Smoke mode always means no folder, which keeps captures and smoke runs out of user files without script edits |
| Q4 | Logger rules | **§2.3.** Stderr in debug builds, and in release builds only while `RUST_LOG` is set (amendment 19's first option). `RUST_LOG` replaces `logging.level`; `logging.level` takes level names only, and anything else is fatal. The file is `<stem>-<stamp>-<pid>.log`, 10 kept per executable, and crash files are never pruned. A release build run from a Linux terminal shows no engine errors unless `RUST_LOG` is set |
| Q5 | `Config::load`'s warnings | **Kept in `Config`** and logged by `App::new` (§2.4). Without this they would be lost, because the logger starts after the load |
| Q6 | Installing the hook once per process | **A `static std::sync::Once`**, which `D-119` records as the guard on std's hook slot and the only process-wide state the engine adds, against the no-global-state rule. The alternative is to install at every `App::new` that has a folder, which chains a second report for a second `App` |
| Q7 | A deliberate panic in a shipped build | **`TUNGSTEN_TEST_PANIC`** registers a first-frame panicking system only when set (§2.5) |
| Q8 | Debug assets and the probe | **One `tungsten-debug-<tag>-<target>` asset per target**, compressed, holding both levels' debug files (§2.7). The probe step is informational (`continue-on-error`, `D-070`), but W11a's done-when in w11 wants it to pass on both runners. Its verdicts arrive only with the rehearsal run (Q9), so the owner reads them before merging. If the Windows half fails (`llvm-symbolizer` missing, or frames unresolved), the owner either holds the merge or merges with the exception recorded in w11, known issues and §4, as a W7b follow-up. A failed Linux half holds the merge |
| Q9 | When the owner checks a shipped crash file | **On a rehearsal prerelease**, pushed after the command block and before the merge. Its run also gives both probes' verdicts, and a failed packaging check (build-id, PDB) then blocks no published release |
| Q10 | W7a's scope and the local cross-check | **`cargo test --workspace --locked` on `windows-2025`** (§2.9). Step 7 may run `rustup target add x86_64-pc-windows-msvc` on the reference machine for a `cargo check`. The owner reads the job's first run on the release pull request; failures become known-issue rows, not fixes in this milestone |

Approved 2026-10-05 with the stated defaults.

## 5. Decisions expected

- `D-119`: Per-user folder, log file and crash report, with the one engine logger that W1 M5's console reads. Extends `D-008`. It records §2.1–§2.5, `TUNGSTEN_USER_DIR`, `TUNGSTEN_TEST_PANIC` and how the logger and hook sit with the no-global-state rule (Q6).
- `D-120`: Release symbols. Extends `D-071`; amends its "build only" for the CPU-only probe.
- `D-121`: CPU-only Windows test job in CI (W7a). Amends `D-070`.

These are the next free IDs at `71b00be`; check again when the step runs.

## 6. Invariants

- **May move:** none.
- **Must not move:** the determinism hash, the pinned containment hash, the row digests (`physics` checked in step 4), `gpu-visual.png` (`just visual`, step 4), the post and transition regressions (smoke), and a capture's logging: release build, filter from `RUST_LOG`, stderr only, default format.

## 7. Stop conditions

- A done-when check fails: restore the step's files from their copies, mark the step skipped, and continue with steps that don't depend on it.
- Any of steps 2–6 skipped: stop before the release, since the milestone's deliverable depends on them. A skipped step 6 would also leave the workflow's strip in place.
- In step 4, the perf capture is invalid, a digest differs from D0, or the example run writes no log or crash file: restore step 4, then stop before the release.
- A new crate (`D-015`) or `unsafe` code would be needed, or env_logger 0.11.11 does not behave as A17 records: stop that step.
- A file outside the step's list needs a change: stop the step and record it.
- A Windows-only failure (step 7's cross-check, or the probe) never stops the run: record it under Q8 and Q10.

## 8. What the milestone owes

| Workflow §5 line | Step | Note |
| --- | --- | --- |
| 1 Evidence | every | |
| 2 Gates by change | 3, 4, 5, 6, 7, release | Smoke, visual and the perf parse check in step 4; `just script-test` in steps 5–7. The release step runs the release checks once, after the cut. `just physics-release` does not apply |
| 3 Invariants | 4, release | |
| 4 Decisions | 1, 5, 7 | `D-119`, `D-120` and `D-121`, each before the docs that cite it |
| 5 API ([break ledger](../../1.0/w04-api-freeze.md#break-ledger), rustdoc, `just api` snapshot) | 1, 2, 8 | Rustdoc on `GameConfig`, `GameConfig::validate`, `Config.game` and `take_load_warnings`; the ledger row and the snapshot in step 8 |
| 6 Template and guide | — | Not applicable before W12a and W9b; step 8 marks w12's item (3) done |
| 7 Routes (`docs/LLM_INDEX.md`, `just ctx`) | 8 | |
| 8 Design | 8 | |
| 9 Records (known issues, backlog, gap log, register, workstream file, README "Now") | 8, release | No backlog cut, no gap rows |
| 10 Close (one `CHANGELOG.md` line, `status: done`, the plan archived) | release | Then the cut, the checks and the preflight; the session ends with both blocks, the owner's checks and the next prompt |

## 9. Critique

Round 1: the `critique` skill with `--repo` (gpt-6.1-sol through Codex, another model family) on 2026-10-05, reading `71b00be` with this plan uncommitted; effort xhigh, the skill's default, since the stop's `plan` stage recommends Opus 5.5, not Fable 5.1 (`scripts/roadmap.py catalog`); 367.3 s, 638,149 tokens in (540,800 cached), 10,573 out (7,172 reasoning). It changed done-when checks in steps 1, 2, 3 and 6, so a second round runs.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 1 | [MEDIUM] Windows symbolization needs an executable the command cannot locate | §2.7–§2.8: `symbolize` took only a crash file and debug files, the Windows debug archive holds only PDBs, and `executable` is the crashing machine's path | Accepted | §2.7: the debug archive extracts over the player archive. §2.8: `--root`, or `--binary` with `--debug`; only the `bin/<level>/<name>` tail of `executable` is used; Windows requires the binary; the probe relocates both. Step 6 adds the cases |
| 2 | [MEDIUM] Removing the Windows console hides errors raised after logging starts | `app.rs:1412` returns the `input.json` error unlogged, as `app.rs:390-391` do for the event loop; fatal errors are logged at `1563`, `1626`, `1641`; `examples/03_scene_state/src/main.rs:77` | Accepted | A18. §2.3 "Returned errors"; step 2's change; probe mode `bad-input` with its step 3 case |
| 3 | [MEDIUM] Programmatic configs bypass the game identifier's filesystem restrictions | `app.rs:160` takes any `Config`; §2.5's probe builds one in code; `Path::join` with an absolute id replaces the base | Accepted | §2.1: `GameConfig::validate`, called by `Config::load` and `App::new`, and `user_dir` refuses an invalid id. Steps 1 and 2 add the cases; §2.10 and §8 list the method |
| 4 | [MEDIUM] The retention pattern can delete another executable's logs | §2.3's `<stem>-*.log` matches `game-helper-…` for `game` | Accepted | §2.3 counts a file only when the rest of its name is a stamp, a pid and `.log`; step 2's pruning case adds `game-helper` |
| 5 | [MEDIUM] Player-only downloads will fail the existing checksum verification | `docs/releases.md:119-122` downloads `tungsten-examples-*` and `SHA256SUMS`, then runs `sha256sum -c`; GNU `sha256sum -c --ignore-missing`, run in the scratchpad, exits 0 with a listed file present and 1 ("no file was verified") with none | Accepted | §2.7: `sha256sum -c --ignore-missing`; step 5's docs change |

Round 2, on the revised plan, the same day and tree: effort xhigh, 485.8 s, 670,273 tokens in (570,752 cached), 15,195 out (11,681 reasoning). There is no third round (skill step 7); every finding was applied, so none became an open question.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 2.1 | [MEDIUM] Symbolization cannot verify that the supplied symbols belong to the crashing build | §2.5 recorded versions and a path, no build identity; §2.8 picked files by path tail; local builds use `target-cpu=native` (`.cargo/config.toml:18`) and release builds a portable level | Accepted | §2.5 adds a `build_id` line (ELF note on Linux, CodeView GUID, age and name on Windows). §2.8 refuses a mismatched or missing identity. Steps 3 and 6 add the cases, step 6's mismatch case being the negative test |
| 2.2 | [MEDIUM] The completion rules do not consistently require working release symbols | §7 blocked the release for steps 2–4 only; restoring step 6 keeps `release.yml:116`'s strip; Q8 allowed a failed Windows probe against `w11-shipping-basics.md:52` | Accepted | §7 covers steps 2–6. §2.7: packaging refuses a debug file without `.debug_line`, with a step 5 case. Q8: the owner reads both probes on the rehearsal and holds the merge or records a Windows exception. Step 8 records the probes as pending; step 9 lists the decision |
| 2.3 | [MEDIUM] Invalid game identifiers fail before the logger that supposedly reports them | Step 2 validated before installing the logger, so the error had no logger to reach | Accepted | §2.3 and step 2: an invalid id leaves no folder, the logger is installed without a file, and the error is logged, then returned. Probe mode `bad-id` checks it in a fresh process (step 3) |

## Evidence log

| Step | Date | Verdict | Key numbers | Paths |
| --- | --- | --- | --- | --- |
| 1 | 2026-10-05 | Pass | `cargo test -p tungsten-core --lib config` → 52 passed, 6 of them new: `my-game` and `com.example.game` pass, and empty, 65 characters, `../x`, `/abs`, `.x`, `a/b` and `con.txt` fail with field `game.id` through `GameConfig::validate` (empty path) and `Config::load` (the file's path); device names in any case and extension, the 64-character limit; `"verbose"` fails with field `logging.level`, `"WARN"` loads. `cargo test -p tungsten --lib user_dir` → 12 passed (XDG set, unset, empty, relative; no `HOME`; Windows; macOS; the override on all three; empty override; smoke with an override; no id; invalid ids with and without the override). `rg -c '^## D-119' DECISIONS.md` → 1; ``rg -c '^\| `D-119`' docs/DECISION_INDEX.md`` → 1. `just repo-check` → exit 0, "Repository QA: 0 error(s)" | `crates/tungsten-core/src/config.rs`, `crates/tungsten/src/user_dir.rs`; `D-119`, `D-008` marker |
| 2 | 2026-10-05 | Pass | `cargo test -p tungsten --lib logging` → 13 passed: `RUST_LOG` replaces the level (`warn`; the runner's `tungsten::app=debug,bench=debug` drops other targets), the level applies without it (`debug`, `off`); the eight sink cases; `example-03-scene-state-20000229T123456.789Z-4242.log`; pruning keeps the 10 newest `game` logs and leaves `game-helper`, `other` and crash files; `open` creates the folder, prunes to 10 counting the new file, and leaves no file under a path that is a file; the `Tee` writes each record; stamps `19700101T000000.000Z`, `20000229T123456.789Z`, `19991231T235959.999Z`, `20261005T000000.000Z`. `cargo test -p tungsten --lib app` → 38 passed, including `game.id` `../x` and `logging.level` `verbose` set in code, each an error naming its field. `cargo test -p tungsten-core --lib` → 511 passed; a missing file, a display fallback and a legacy conflict (before the fallback) come back from `take_load_warnings()`. `just check` → exit 0, 27 test binaries, 1,033 passed. A17 holds for env_logger 0.11.11 (one `write_all` and `flush` per record under the pipe's mutex; `is_test` prints through `eprint!`), but the locked `env_filter` is 2.0.0, not 1.0.1; its `build` still defaults to `error` with no directive. `Cargo.lock` gains only `env_logger` under `tungsten` | `crates/tungsten/src/logging.rs`, `app.rs`; `crates/tungsten-core/src/display.rs` |
| 3 | 2026-10-05 | Pass | `cargo test -p tungsten --lib crash` → 11 passed: the report laid out from plain fields (escaped message, `(none)` for a missing identity, no `modules:` without them); the ELF reader returns a synthetic build-id after another note and nothing for another name, a non-`PT_NOTE` segment or a truncated file; the PE reader returns the synthetic CodeView GUID, age 3 and name (`00112233445566778899AABBCCDDEEFF3 D:\build\release\deps\game.pdb`) and nothing for `NB10` or a bad signature; other files have none; `TUNGSTEN_TEST_PANIC` counts only set and not empty, and the system's panic names it; a second report in one millisecond takes `-2-crash.txt`. `cargo test -p tungsten-crash-probe` → 7 passed: `panic` leaves one `*-crash.txt` with `game: tungsten-crash-probe`, `game_version` and `engine_version` 0.47.0, `target` equal to `rustc -vV`'s host, `executable`, `thread: main`, the message, `location: tools/crash-probe/src/main.rs:70:5`, an `anchor:` line, a non-empty `build_id:`, frames naming `tungsten_crash_probe` and the probe's path under `modules:`, the run's log naming the file and the previous hook's marker on stderr; `log info` writes one `.log` with the warn and info records and no debug or trace; with `RUST_LOG=warn`, the warn record alone; twelve runs leave ten; `own-logger` prints its marker through its own logger and no `.log` appears; `bad-input` exits non-zero with `invalid action map in 'input.json'` at `ERROR` in its log; `bad-id` exits non-zero naming `game.id` with no folder created; `TUNGSTEN_SMOKE_FRAMES=1` leaves the folder empty. The `log` mode logs warn to trace, not error, so the `RUST_LOG=warn` case can read "only the warn record"; `bad-input` covers `error`. `just check` → exit 0, 29 test binaries, 1,051 passed, after its first run flagged two pedantic lints in the new code (`chunks_exact` with a constant size, a `[u8; 4]` by reference), fixed in `crash.rs` and its tests | `crates/tungsten/src/crash.rs`, `build.rs`; `tools/crash-probe/` |
| 4 | 2026-10-05 | Pass | D0, on step 3's tree: `just perf run physics --repeat 2 --allow-background` → valid, digests `c10885dddd6115a4` ×2, frames 420 ×2, no notes. `rg -c 'env_logger' examples` → no matches; `rg -l 'windows_subsystem' …` → 5 files. `TUNGSTEN_USER_DIR=$S TUNGSTEN_TEST_PANIC=1 cargo run -p example-03-scene-state` → exit 101; `example-03-scene-state-20261005T035008.165Z-1855103.log` holds `Loaded action map` (1 line), and the crash file beside it reads `message: TUNGSTEN_TEST_PANIC is set: panicking in the first frame`. After the edits: `just perf run physics --repeat 2 --allow-background` → exit 0, `{"valid":true,"d":["c10885dddd6115a4","c10885dddd6115a4"],"f":[420,420]}`, equal to D0, no notes; each run's `telemetry.log` keeps 2,103 lines in env_logger's default `[… INFO  tungsten::app]` format, as D0's. `just smoke` → exit 0 (4/4 examples, matrix 4/4, post-stack 2/2, post-AA, bloom, lighting, game-feel 2/2, mesh/transition 5/5, benchmarks 15/15, frame cap). `just visual` → exit 0 (`gpu_visual_matches_fixture` and the visual test 2 passed, post regression 5, transition regression 1). `fd -I --changed-within 6h . ~/.local/state/tungsten-examples` → no such folder; neither are `~/.config` or `~/.local/share` ones. `nxcodec.bin` absent before and after every run | `perf-runs/20261005T034846Z-physics` (D0), `perf-runs/20261005T035038Z-physics` |
| 5 | 2026-10-05 | Pass | `D-120` written first (`^## D-120` 1, index row 1). `python3 -B scripts/test-release.py` → 28 passed, 5 new: a synthetic ELF's build-id, a mismatch raising `split/game: build-id 000102… differs from game.debug's`, a debug file without `.debug_line` raising, and a binary without a build-id; the CodeView GUID, age and path of a synthetic PE and the GUID and age of a synthetic MSF info stream (512- and 4096-byte blocks), matching and not (age 4, zero GUID); the Linux `package` with a fake splitter (player archive without `.debug`/`.pdb`, `tungsten-debug-v0.26.0-…tar.gz` with `bin/<level>/<example>.debug` for both levels and examples, each launcher copy stripped with no debug file); the Windows debug zip under the records' `example_01_demo.pdb` names, with a mismatched or missing PDB failing; the README naming `~/.local/state/my-game/logs` and `%LOCALAPPDATA%\my-game\logs`, the console sentence Linux-only. `cargo build --release -p tungsten-crash-probe`, `cp` to `$S/p`, `release.py split-debug $S/p $S/p.debug` → exit 0; `readelf -n` Build IDs `99346cf97eff80112f5479daf0ef517469e2ee76` on both; sizes 16,084,112 → 2,618,040 (binary) + 3,800,816 (debug file). The first run left one `.debug_` match, `.debug_gdb_scripts`: an allocated (`A`) 0x11a0-byte section rustc emits to hint GDB's Rust pretty-printers, which `--strip-debug` keeps by design. The split now also passes `--remove-section=.debug_gdb_scripts` (in `D-120`); `readelf -l` shows the same LOAD segments with and without the header, the stripped probe runs (`log info`, exit 0), and the flag is a no-op where the section is absent. Rerun from a fresh copy: `readelf -S $S/p \| rg -c '\.debug_'` → no matches. `just script-test` → exit 0 (bench 35, check-repo 25, roadmap 13, release 28, preflight 22; shellcheck, actionlint, smoke-script tests). `just repo-check` → exit 0 | `scripts/release.py`, `scripts/test-release.py`, `docs/releases.md` |
| 6 | 2026-10-05 | Pass | `python3 -B scripts/test-crash-report.py` → 7 passed: a Linux crash file's fields, module bases (`/…/bin/x86-64-v3/game` at `0x560000000000`, libc apart) and the −1 rule (the IP's frame keeps `0xb0010`, return addresses lose one); a fake `addr2line -a -C -f -i -e <debug>` called once, for the executable, with each address once, its inline pairs and discriminators parsed; a `/home/player/Downloads/…/bin/x86-64-v3/game` path and a `C:\Users\player\…\bin\x86-64\game.exe` path mapped under `--root` to the binary and its `.debug` or record-named PDB, and paths without the tail or binary refused; a differing, `(none)` or empty `build_id` exiting 1 with the fake never called (Linux and Windows); the Windows anchor arithmetic (`0x7ff612341010` − RVA `0x1010`: `0x2000`, `0x3004`, a frame outside the image dropped), the anchor found as a v0-mangled public or a module's procedure record, its RVA from the section table, and Windows refusing without a binary; `--expect` matching, missing, and nothing resolved. `cargo build --release -p tungsten-crash-probe && python3 -B scripts/crash-report.py probe target/release --target x86_64-unknown-linux-gnu` → exit 0: the probe exited −6 (abort), `build_id 99346cf97eff80112f5479daf0ef517469e2ee76 matches …/archive/bin/x86-64/tungsten-crash-probe.debug`, and `12: tungsten_crash_probe::inner at …/tools/crash-probe/src/main.rs:70`, then `outer` (65), `panic_mode` (59), `main` (28). `rg -c 'CARGO_PROFILE_RELEASE_STRIP' .github/workflows/release.yml` → no matches. `just script-test` → exit 0, actionlint included (crash report 7, release 28, the others as in step 5). Built first, the symbolizer resolved one address per backtrace line and printed each shared address's inline chain once per line (777 lines); it now resolves each address once and prints its chain under the first line's index (27 lines) | `scripts/crash-report.py`, `scripts/test-crash-report.py`, `.github/workflows/release.yml`, `docs/releases.md`, `justfile` |
| 7 | 2026-10-05 | Pass; cross-check not run | `D-121` written first: `rg -c '^## D-121' DECISIONS.md` → 1, ``rg -c '^\| `D-121`' docs/DECISION_INDEX.md`` → 1. `ci.yml` gains `windows-tests` (`windows-2025`, `core.autocrlf false` before checkout, the pinned checkout and toolchain actions, `cargo test --workspace --locked`, 90 minutes); actionlint passes. `just script-test` → exit 0 (bench 35, check-repo 25, roadmap 13, release 28, crash report 7, preflight 22). `rustup target add x86_64-pc-windows-msvc` → installed; `cargo check --workspace --all-targets --locked --target x86_64-pc-windows-msvc` → **not run** to completion: exit 101, `error: failed to run custom build command for alloca v0.4.0` … `error occurred in cc-rs: failed to find tool "lib.exe"`, after 123 crates checked; `alloca` is a dependency of `criterion` 0.8.2, a dev-dependency, and needs MSVC's tools, which this Linux host lacks. Extra, without dev-dependencies: `cargo check --workspace --locked --target x86_64-pc-windows-msvc` → exit 0, no warnings. The job's own check, its first run on the release pull request, is the owner's (Q10) | `.github/workflows/ci.yml`; `D-121`, `D-070` marker |
| 8 | 2026-10-05 | Pass | `DESIGN.md`: the stack row, `game` in the config example, `logging.level` applied, and "Logs and Crash Reports — M35". Index rows: logs, user folder and crash reports (`D-119`), the release row with `scripts/crash-report.py` and `D-120`, and a CI row (`D-070`, `D-121`). Known issues: the follow-up keeps only `display.scale_mode`; the no-host list gains the Windows console, Windows PDB symbolization and the unread probe steps and Windows job; three follow-ups (the launcher's errors on Windows, a `Config::load` error before `App::new`, the bench's dropped knob warning). Profiling workflow, w11, w07, w12, w04 (ledger row), criteria §10 (Q14 answered, revision line) and `roadmap.json` (`q14` `default`, citing `D-119`). `just api` → exit 0; `git --no-pager diff --stat -- api/` → `api/tungsten-core.txt \| 44 +` only; `git --no-pager diff -U0 -- api/ \| rg -c '^-pub'` → no matches (the run's two rustdoc warnings are `sprite_extract.rs`'s, from before M35). `just ctx` → exit 0, `LLM_INDEX.md` 9,398 B. `just repo-check` → exit 0, 0 errors. `rg -c 'logging\.level' docs/known-issues.md` → no matches | `DESIGN.md`, `docs/LLM_INDEX.md`, `docs/known-issues.md`, `api/tungsten-core.txt` |
| 9 | 2026-10-05 | Closed; the release checks are in the run's report | Workflow §5: (1) evidence rows 1–8 quote every done-when, step 7's cross-check recorded as not run with its error; (2) gates by change: `just check` (steps 2, 3), layer 1 inside `just repo-check` (steps 1, 5, 8), `just smoke` and `just visual` (step 4), the perf parse check (step 4) and `just script-test` (steps 5–7), with `just check`, `just smoke`, `just script-test` and the rest run again after the cut; (3) invariants: the `physics` digest equals D0 (`c10885dddd6115a4`), `gpu-visual.png` and the post and transition regressions pass `just visual`, and a capture's logging is unchanged (2,103 lines in the default format); no physics code changed, so the determinism and containment hashes need no rerun (§8), though the release checks run `just physics-release` too; (4) `D-119`–`D-121` with index rows and markers on `D-008`, `D-070` and `D-071`, each before the docs citing it; (5) the break-ledger row, rustdoc on `GameConfig`, its fields, `GameConfig::validate`, `Config.game`, `Config::take_load_warnings`, `App::new` and `App::run`, and `api/tungsten-core.txt` regenerated; (6) template n/a before W12a, w12's item (3) marked done; (7) three index rows, `just ctx` passing; (8) `DESIGN.md`'s "Logs and Crash Reports — M35"; (9) known issues, w11, w07, w12, w04, criteria §10, `roadmap.json`, the register row and the README "Now" lines; (10) one `CHANGELOG.md` entry, `status: done`, the plan archived before the cut. Owner's checks before the merge: Q9's rehearsal tag, the run's two probe steps (Q8), the crash-report check on its Linux archive, CI's Windows job (Q10) | `CHANGELOG.md`, `DESIGN.md`, `docs/plans/1.0/implementation-plan.md`, `docs/plans/1.0/README.md` |

## Follow-ups

- The launcher's errors are invisible on Windows once it has no console: an error dialog or a log line, for W7b or W14b. Step 8 files it in known issues.
- A `Config::load` error before `App::new` reaches only stderr, which a Windows release build lacks. Step 8 files it in known issues.
- Found in step 4: the bench's `knobs::unless_env` warning (an environment variable overriding a knob) is logged before `App::new` installs the logger, so it is now dropped; before, it reached stderr only with `RUST_LOG` set. `examples/02_bench/src/knobs.rs` is outside the plan's files.
