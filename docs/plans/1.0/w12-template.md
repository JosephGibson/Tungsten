# W12 Template project — draft

- **status:** in progress
- **goal:** `templates/basic`, the layout every game follows, built, tested and smoke-run in the workspace and from a copy outside it; examples 01, 03 and 04 on the same layout.
- **non-goals:** `02_bench`, which keeps its harness and digests; a template past a few hundred lines; moving the sprite shaders inside `tungsten-render`, which waits for Q2 (W9c).
- **files to touch:** W12a: `crates/tungsten` (the engine font embedded for the HUD, inspector and overlays, and no read of the root manifest), the workspace `Cargo.toml`, `templates/basic/**` with its `AGENTS.md` and `CLAUDE.md`, `scripts/smoke-examples.sh`, an outside-copy recipe in the `justfile`, `docs/LLM_INDEX.md`, `docs/known-issues.md`. W12b: `examples/01_platformer`, `examples/03_scene_state` and `examples/04_shader_playground`, each with its own `tungsten.json`, `input.json` and `assets/`; the root `tungsten.json`, `input.json` and `assets/`; `scripts/smoke-examples.sh`, `tools/launcher/` and `scripts/release.py` for the archive layout. Each: `DECISIONS.md`, `docs/DECISION_INDEX.md` and the getting-started guide.
- **ordered steps:** (1) W12a, in Phase 5 Track A after W11a and W9a. (2) Template upkeep: every later milestone that changes how a game is written updates the template in the same step. (3) W12b with W1 M6, in Phase 6 after W13 and W1 M5 ([implementation plan](implementation-plan.md) §3, §4).
- **done-when:** The template builds, tests and smoke-runs in the workspace and from a copy outside it; its `main.rs` names no engine system once W15a has landed; examples 01, 03 and 04 run on its layout from their own folders; `02_bench` digests are unchanged; RC-A9 of the [release checklist](release-checklist.md) passes at C3.

Graduated on 2026-10-03 from [criteria](criteria.md) §8.3 after the definition gate ([implementation plan](implementation-plan.md) §11), with implementation plan amendments 6, 14 and 16 folded in. Criteria §8.3 now only places this file.

## Placement

- **Tier:** Must.
- **Candidates:** W12a, self-containment, the template skeleton and the outside-copy check (Phase 5, Track A; the skeleton registers engine systems by hand until W15a); template changes that land with each workstream that changes how a game is written; W12b, example migration in one pass per example with W1 M6 (Phase 6, amendment 6).
- **Needs first:** W11a, for logging; W9a, for the font's notice.
- **Feeds:** W14b `new`; the acceptance game; W9b's guide.
- **Owner questions:** Q18 and Q19, both answered. Q2 decides only whether the sprite shaders move later (W9c).
- **Decisions:** Standalone games: the engine owns its assets, no engine feature reads the root manifest, and the template's layout is the supported one; amends the workspace-root wording of `D-008`, `D-013` and `D-045` (criteria §11). The same entry settles shader ownership: the sprite shaders stay in `assets/shaders/` until Q2, and the stock shaders' two copies are kept or merged (`D-059`, amendment 14).

## Context digest

- W12 makes a game in its own repository, on a git dependency, the supported way to use Tungsten. `templates/basic` shows the layout, games and the migrated examples follow it, and the engine stops relying on the repository root. Questions 18 (examples run from their own folders) and 19 (embed JetBrains Mono) are answered.
- What an outside repository lacked before W12a (criteria §2.2): the debug HUD asks the shared manifest for its `mono` font; every example runs from the root on the shared `tungsten.json`, `input.json` and `assets/manifest.json`; there is no template and no guide. Logging left `main.rs` in 0.48 (W11a, `D-119`). `tungsten-render` includes `sprite.wgsl` and `lit_sprite.wgsl` from `../../../assets/shaders/`, which a git dependency tolerates, so they stay (amendment 14).
- W12a, in Phase 5 Track A after W11a and W9a: self-containment, the template skeleton with its `AGENTS.md` for agent sessions (amendment 16), the outside-copy check, and the getting-started guide's first draft (W9b). The template registers engine systems by hand until W15a.
- From W12a on, a milestone that changes how a game is written updates the template, and the guide, in the same step ([workflow](workflow.md) §5 item 6). W12b, in Phase 6 with W1 M6, moves examples 01, 03 and 04 onto the layout in one pass each (amendment 6); `02_bench` keeps its harness and digests.
- The acceptance game starts from the template when W15a lands (`D-102`), and RC-A9 tests the template's `AGENTS.md` at C3. A cold standalone build wrote 973 MB, so the outside-copy check shares the workspace's target folder and checks free space first ([workflow](workflow.md) §6).

## Scope

Moved verbatim from criteria §8.3 on 2026-10-03; the section reference now names criteria. Folded in: implementation plan amendment 14 in the self-containment bullet's shader sentences, amendment 16 in the layout's `AGENTS.md` and `CLAUDE.md` lines and the agent-rules bullet, and amendment 6 in the examples bullet's last sentence.

Owner-added on 2026-10-03. A game lives in its own repository on a git dependency, `templates/basic` shows how, and examples 01, 03 and 04 move onto its layout. `02_bench` keeps its harness so its digests stay comparable.

- **Self-containment first.** What an outside repository lacks today (criteria §2.2): (1) the engine's own assets. The HUD's `mono` font, JetBrains Mono Regular (115 KB), moves into the umbrella crate as embedded bytes for all engine text: HUD, inspector and overlays. Every game then ships its OFL notice (W9; owner, 2026-10-03). `sprite.wgsl` and `lit_sprite.wgsl` stay in `assets/shaders/`: a git dependency checks out the whole repository, so `include_str!` from `../../../assets/shaders/` already works for a game in its own repository, and only a crates.io package (Q2) needs them inside `tungsten-render`. A yes on Q2 moves them in W9c as an internal change, with `assets/shaders/` keeping copies for hot reload, as the stock post shaders already do (`D-091`). The same decision settles the stock shaders' two copies (`D-059`). (2) No engine feature reads the shared root manifest. (3) Logging and the crash hook move into the umbrella (W11), so `main.rs` stops calling `env_logger::init()`. *Done in 0.48 (M35, `D-119`).* (4) W11's game identifier names the user folder.
- **Layout.**

  ```text
  templates/basic/
    Cargo.toml            tungsten and tungsten-kit: a path dependency in the workspace, a git tag once copied
    rust-toolchain.toml   the engine's pinned toolchain (D-069)
    tungsten.json         game identifier, window, render
    input.json            named actions; game code reads actions, never keys
    assets/manifest.json  the game's own fonts, sprites, sounds and prefabs; no shared root
    src/main.rs           about ten lines: App::new, DefaultPlugins, GamePlugin, run
    src/game.rs           GamePlugin: resources, events, systems by stage
    src/states.rs         title → gameplay → pause on the state stack (D-046)
    src/components.rs     game components, registered for prefabs (W16)
    tests/                headless harness tests (W14)
    README.md             run, test, hot reload, package
    AGENTS.md             the game repository's rules for agent sessions (below)
    CLAUDE.md             imports AGENTS.md
  ```

- **Practices it shows.** Stages and plugins in place of a hand-ordered list (W15); kit components before game code (W13); registry IDs and prefabs in place of paths and spawn code (`D-046`, W16); settings and one save slot (W11); actions in place of keys; `anyhow` in `main`, `thiserror` in modules and `log` for output (`AGENTS.md` conventions); asset hot reload in debug builds; a headless test per system.
- **Rules for agent sessions.** This engine is built in Claude Code sessions, and games made from the template will be too. The template's `AGENTS.md`, with a `CLAUDE.md` that imports it, holds a game repository's rules: the public API and the kit only, registry IDs not paths, actions not keys, systems by stage, a harness test per system, `tungsten check` before a commit, and where the engine's documentation lives. The acceptance game starts from it, so its sessions test it, and C3 checks it (RC-A9): a fresh session in a copy of the template adds a scripted feature using only the template, the guide and rustdoc.
- **It grows with the engine.** Every workstream that changes how a game is written lands its template change in the same step, so the template stays the current practice. It stays basic: a feature that would take it past a few hundred lines belongs in the acceptance game.
- **Kept honest.** A workspace member, built and tested by `just check` and smoke-run by `just smoke`. A second check copies it to a folder outside the workspace, points its dependency at this checkout, and builds, tests and smoke-runs it from there, so any hidden reliance on the repository root fails. That check shares the workspace's target folder and profiles. On 2026-10-03, a cold standalone build with a target folder of its own wrote 973 MB before it filled a disk that was already close to full.
- **Examples.** 01, 03 and 04 adopt the layout, plugins and kit, and drop their own copies of kit features. Each runs from its own folder with its own `tungsten.json`, `input.json` and `assets/`, as a copied template does (owner, 2026-10-03). The smoke script, the release launcher and the archive layout change to match. The shared root `assets/` keeps only what `02_bench` and the engine's hot-reload copies need. Each example moves once, in one pass with its W1 M6 UI migration (W12b), not twice.
- **Done-when sketch:** the template builds, tests and smoke-runs in the workspace; the outside copy does the same from a folder with no `tungsten.json`, `input.json` or `assets/` above it; its `main.rs` names no engine system; examples 01, 03 and 04 on the layout with their smoke matrices green and the transition and post-stack regressions byte-equal; `02_bench` digests unchanged.

## Steps

The workstream's candidates in order. Each candidate's milestone plan holds its execution steps.

1. **W12a: self-containment, the template skeleton and the outside-copy check.** Phase 5, Track A, after W11a and W9a; level B ([workflow](workflow.md) §4). The engine font embedded and read by the HUD, inspector and overlays in place of the manifest's `mono`; no engine read of the root manifest; the template as a workspace member, registering engine systems by hand until W15a, with its `AGENTS.md` and `CLAUDE.md` holding the rules that exist so far (`tungsten check` joins with W14b); the outside-copy recipe; the getting-started guide's first draft (W9b); the decision on standalone games and shader ownership.
   - **Landed in 0.49** (M36, with W9a; plan archived at `docs/plans/archive/1.0/phase5-milestone-36-licence-notices-template.md`; `D-123`): the engine font, the action-map path, the template skeleton, `just template-check` and the guide's first draft. Its sprite is generated, and its `src/lib.rs` lets `tests/` reach the game's code.
   - **Done-when:** Layer 1 passes. `just check` builds and tests the template and `just smoke` runs it. The outside-copy recipe copies it to a folder with no `tungsten.json`, `input.json` or `assets/` above it, after a free-space check, and builds, tests and smoke-runs it there on the workspace's target folder. The HUD draws with the embedded font when the loaded manifests have no `mono`, and no engine code reads the root manifest. `just visual` is byte-equal and smoke passes for every example. `docs/LLM_INDEX.md` routes to the template, and `just ctx` passes. The decision entry amends the workspace-root wording of `D-008`, `D-013` and `D-045`, and the stock-shader follow-up in known issues is closed or points at it.
2. **Template upkeep.** From W12a until 1.0, every milestone that changes how a game is written updates `templates/basic`, its `AGENTS.md` and the guide in the same step ([workflow](workflow.md) §5 item 6). W15a, for example, removes the hand-registered engine systems from its `main.rs`.
   - **Done-when:** Each such release's diff carries its template change, the template stays a few hundred lines, and the outside-copy check passes in that release's checks.
3. **W12b: examples 01, 03 and 04 on the template layout, with W1 M6.** Phase 6, after W13 and W1 M5: one pass per example covering the layout, plugins, kit and UI. Each example runs from its own folder; the smoke script, the release launcher and the archive layout follow; the root `assets/` keeps only what `02_bench` and the engine's hot-reload copies need.
   - **Done-when:** Each migrated example runs from its own folder with its own `tungsten.json`, `input.json` and `assets/`, lists no engine system, and has no copy of a kit feature. Its smoke matrix is green, and the transition and post-stack regressions are byte-equal. `02_bench` digests are unchanged. A rehearsal prerelease's archives start each example through the launcher from its folder.

## Done-when

The template builds, tests and smoke-runs in the workspace and from the outside copy, and its `main.rs` names no engine system after W15a. Examples 01, 03 and 04 are on the layout with their smoke matrices green and the transition and post-stack regressions byte-equal, and `02_bench` digests are unchanged. RC-A9 passes at C3.

## Follow-ups

1. **The `input.json` watcher follow-up has two homes.** Implementation plan §3 sends known issues' "any `input.json` under a watched directory reloads as the action map" to W12a "when each example gets its own folder", but examples get their own folders in W12b (amendment 6), and W11b moves bindings to the user folder. W12a's plan settles where the fix lands. Default: W12a, if the template's folder is the first one the watcher sees with an `input.json` of its own; otherwise W12b. *Settled in M36 step 3: the action map reloads only from its canonical path (`D-123`).*
