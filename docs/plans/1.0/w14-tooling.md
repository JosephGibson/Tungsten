# W14 Tooling — draft

- **status:** in progress
- **goal:** A headless harness that runs the real stage order, and a project CLI with `check`, `package` and `new`.
- **non-goals:** JSON schemas for config files; live editing in the game (backlog). Moving the physics determinism, containment and tunneling tests, which stay at the core level.
- **files to touch:** W14a: `crates/tungsten/src/app.rs` (a frame body apart from the window, surface and audio), the umbrella's `Cargo.toml` (the `testing` feature) and a harness module in it, `examples/01_platformer/src/tests/`, `docs/LLM_INDEX.md`. W14b: `tools/cli/`, a new `tungsten-cli` package, and the workspace `Cargo.toml`; `scripts/release.py` if `package` shares its steps; `templates/basic/AGENTS.md` and the getting-started guide; `docs/LLM_INDEX.md`. Each: `DECISIONS.md` and `docs/DECISION_INDEX.md`.
- **ordered steps:** (1) W14a, first in Phase 5 Track A. (2) W14b, in Phase 6: `new` after W12a, `package` after W9a and W11a, `check` after W16b ([implementation plan](implementation-plan.md) §3, §4).
- **done-when:** The platformer's tests run through the harness with no hand-written loop; RC-A7 of the [release checklist](release-checklist.md) passes at C3.

Graduated on 2026-10-03 from [criteria](criteria.md) §8.5 after the definition gate ([implementation plan](implementation-plan.md) §11), with implementation plan amendments 5 and 11 folded in. Criteria §8.5 now only places this file.

## Placement

- **Tier:** Harness Must; CLI `check` and `package` Must, `new` Should, as proposed until Q20 settles them.
- **Candidates:** W14a, the headless harness behind a `testing` feature (Phase 5, Track A); W14b, the CLI (Phase 6): `check` after W16b, `package` after W9a and W11a, `new` after W12a. Package `tungsten-cli` in `tools/cli/`, binary `tungsten` (amendment 11).
- **Needs first:** For W14a, an `App` frame body that runs without a window, surface or audio device. For W14b, W12a, W9a, W11a and W16b.
- **Feeds:** Every later test; W1 M1's `UiHarness` (amendment 5); the template's tests and its `tungsten check` rule (W12); RC-A7.
- **Owner questions:** Q20, due when W14b starts.
- **Decisions:** Headless harness behind a `testing` feature; the project CLI and its dependencies (`D-015`).

## Context digest

- W14 gives game code a way to test frames without a window, and a game project a command line. Both were owner-picked on 2026-10-03. The harness is Must; the CLI's `check` and `package` are Must and `new` Should, as proposed, until question 20 settles them when W14b starts.
- Today, tests copy the frame loop by hand (systems, then a manual event flush) and skip particles, tweens, the command flush and event rotation; no headless `App` steps frames (criteria §2.2). The platformer's test file runs to 2,366 lines (a 0.40 follow-up).
- W14a comes first in Phase 5 Track A: every later test stands on it, W1 M1's `UiHarness` builds on it (amendment 5), and W1 M1 cannot start before it. It changes the frame body in `app.rs`, so it captures every CPU row ([workflow](workflow.md) §5).
- W14b, in Phase 6, is a `tungsten-cli` package in `tools/cli/` whose binary is `tungsten` (amendment 11). `new` needs W12a's template, `package` needs W9a's notices and W11a's symbol handling, and `check` needs W16b's prefabs. Its argument parsing and archive writing each need a `D-015` argument.
- The CLI's commands and flags are inside the 1.0 promise (`D-103`), and the harness is public surface the freeze covers ([w04](w04-api-freeze.md)).

## Scope

Moved verbatim from criteria §8.5 on 2026-10-03. Folded in: implementation plan amendment 5 in the harness bullet's last sentence, and amendment 11 in the CLI bullet's package name.

Owner-picked on 2026-10-03: a headless test harness and a project CLI. JSON schemas for the config files, and live editing in the game (inspector edits, a tweak panel or a console on W1's widgets), wait for 1.x.

- **Headless harness.** An `App` with no window, surface or audio device that runs the real stage order: `step(n)` at a pinned dt, injected actions and cursor positions, events and resources readable between steps, and the CPU extracts runnable, so a test can check what a frame would draw. It lives in the umbrella behind a `testing` feature. Example tests that copy the frame loop move onto it; the physics determinism, containment and tunneling tests stay at the core level. W1 M1's `UiHarness` is built on it, not beside it.
- **Project CLI**, a `tungsten` binary in `tools/`: the umbrella package is already `tungsten`, so the package is `tungsten-cli` in `tools/cli/`, beside `tools/launcher/`, with `[[bin]] name = "tungsten"`. Cargo can report a doc output collision when a library and a binary share a name, so the binary sets `doc = false` before W4 gates rustdoc.
  - `new <name>` copies `templates/basic`, sets the game identifier and window title, and pins the engine as a git dependency on the current tag.
  - `check` loads `tungsten.json`, `input.json`, every manifest, scene and prefab with the engine's own loaders, checks that each referenced ID exists, and reports every error, not just the first. It cannot see string IDs in game code; R0's interned IDs and a once-per-ID warning for an unknown sprite (W8) cover that side.
  - `package` makes a release build and an archive for a standalone game: binary, assets and licence notices (W9), with the debug file kept apart (W11). It reuses the steps `release.yml` runs for the examples.
- **Dependencies.** The CLI is a tool, not a runtime dependency, but `D-015` still applies. Argument parsing (by hand or a crate) and archive writing (a crate, or the platform's `tar` and `zip`) each need their argument in a decision entry.
- **Done-when sketch:** the platformer's tests run through the harness with no hand-written loop; the output of `tungsten new` passes `tungsten check`, then builds, tests and packages outside the workspace; `check` reports every error in a seeded set of broken files; the packaged archive runs on the Linux reference machine.

## Steps

The workstream's candidates in order. Each candidate's milestone plan holds its execution steps.

1. **W14a: the headless harness.** First in Phase 5, Track A; level B ([workflow](workflow.md) §4). A frame body in `app.rs` apart from the window, surface and audio device, which `App::run` and the harness both call; the harness behind the umbrella's `testing` feature; example 01's hand-written test loops moved onto it, splitting its 2,366-line test file as they go.
   - **Done-when:** A harness test steps frames at a pinned dt and observes particles, tweens, the command flush and event rotation, which the hand-written loops skipped, and another runs the CPU extracts and checks what a frame would draw. Example 01's tests run through the harness, and `rg` finds no hand-written frame loop or manual event flush left in them. The core physics determinism, containment and tunneling tests are untouched. Every CPU row reads not `regressed`, `just smoke` passes with digests unchanged, and every new public item has rustdoc.
2. **W14b: the project CLI.** Phase 6; Q20 is answered before the plan is approved. The `tungsten-cli` package in `tools/cli/` with the `tungsten` binary and `doc = false`: `new` once W12a has landed, `package` once W9a and W11a have, `check` once W16b has. One milestone, or one per command if the plan splits it.
   - **Done-when:** `cargo doc --workspace` reports no output collision. A test seeds a set of broken config, manifest, scene and prefab files, and `tungsten check` reports every error in it, each naming its file. The output of `tungsten new` passes `tungsten check`, then builds, tests and packages outside the workspace on the workspace's target folder. The packaged archive carries the licence notices, keeps the debug file out, and runs on the Linux reference machine. The decision entry gives argument parsing and archive writing their `D-015` arguments, and the template's `AGENTS.md` and the guide name `tungsten check`.

## Done-when

The platformer's tests run through the harness with no hand-written loop, and RC-A7 passes at C3: the output of `tungsten new` passes `tungsten check`, then builds, tests and packages outside the workspace.

## Follow-ups

1. **RC-A7 needs `tungsten new`, which is proposed as Should.** If Q20 keeps it Should and it is cut, RC-A7 and C3's outside-workspace check need another way to make the project, such as W12a's outside-copy recipe. Settle it with Q20 when W14b starts.
