# Roadmap redo

The plan of record from Phase 6's tail to 1.0, adopted on 2026-10-25 (`D-215`); [ROADMAP.md](../../ROADMAP.md) holds the status. It replaces the 1.0 roadmap after Phase 6. It brings the workflow cutover forward into Phase 6's tail and the cheap, high-leverage fixes and agent tooling to the head of Phase 7. It reworks the benchmark suite in two parts, and replaces today's Phase 7 with three phases (Seams, Genres, Release), defining 1.0 as a fully featured engine, tooling and kit rather than a frozen API. Written from the owner's review session of 2026-10-22 and 2026-10-23 at `6773e75` (0.101). The transition session of 2026-10-25 rechecked it at `bb1e4c0` (0.107), after M81–M85 and 0.107 had landed under the old flow, and folded in the [paper designs](paper-designs.md). Facts marked `@be16369` were measured at that commit; facts marked `@bb1e4c0` moved since the draft, with the draft's figure beside them. The same session adopted it and put H1 in place.

- **status:** in progress
- **goal:** Re-aim 1.0 at the bottleneck example 01 measured, the cost of making a game, not engine speed or API stability, and do the work that pays back soonest first:
  1. Health in two parts: the cutover's essentials (H1) before 01's remaining stops, the rest (H2) once the quick wins and tooling give it real tasks.
  2. Phase 7, Seams: quick wins, agent tooling and the benchmark runner first; then the analysis pass and the paper designs; then the engine, kit, template and level work they scope, with physics and slopes before saves, a chemistry-style interaction model, and saves the engine does for the game; then a CPU and a GPU performance pass.
  3. Phase 8, Genres: the systems a story-driven RPG and a shooter with complex enemy AI need, then 01 refreshed on them.
  4. Phase 9, Release: the benchmark expansion pass with budgets, performance, quality, docs and tests, release candidates, 1.0.
- **non-goals:**
  - Edits outside this file before adoption: the register, `roadmap.json`, decisions, workstream files, the Phase 7 rewrite, skills.
  - Changes to the example stops from `e01quick2` on, beyond carrying them into `ROADMAP.md`.
  - Dates and time estimates.
  - Each step's mechanisms: its own plan specifies them from the tree of its day.
  - Implementation.
- **files to touch:** This file while it is a draft. At adoption (ordered step 3): `docs/plans/1.0/implementation-plan.md` (§2, §5, §6, §10), `docs/plans/1.0/roadmap.json`, `docs/plans/1.0/README.md`, `docs/plans/1.0/acceptance-game.md`, `docs/plans/1.0/w04-api-freeze.md`, `docs/plans/1.0/workflow.md`, `docs/plans/1.0/release-checklist.md`, `docs/plans/1.0/w13-kit.md` (§14), `docs/plans/1.0/backlog-1.x.md` (the `--genre` note), `docs/plans/phase7-rewrite.md` (superseded and archived), `DECISIONS.md` and `docs/DECISION_INDEX.md` (one entry, §9). H1 then creates `ROADMAP.md` and edits what §4's rows name.
- **ordered steps:**
  1. The owner reads this draft and answers or accepts the open questions (§10). Done 2026-10-23 and 2026-10-25.
  2. M81 (`e01g`) releases. Done, with M82–M85 and 0.107 after it.
  3. The transition session, 2026-10-25: recheck every `@be16369` and `@6773e75` fact and §9's list against the tree; fold in the paper designs; one critique round; write the decision entry; adopt §3's order; archive the Phase 7 rewrite. H1 follows in the same session if it fits, otherwise in the next. Done 2026-10-25 (`D-215`), H1 included, ending with 0.108.
  4. `e01quick2`, `e01tomes` and the later example stops run under H1; the first that draws new art is the first trial of §6's asset sessions.
  5. Phases 7, 8 and 9 in §3's order, re-cut after each game finding (H1's habits).
- **done-when:**
  - Every open question in §10 has an owner answer or a stated default.
  - The transition session has rechecked the facts, written the decision entry and adopted, changed or rejected §3's order.
  - H1 is in place before `e01quick2`, and 0.108 is the old pipeline's last release.

## Context digest

- On 2026-10-25, 0.107 is out (M81–M85 landed as 0.102–0.106 under the old flow). Six example stops remain: `e01quick2`, `e01tomes`, `e01foes`, `e01sfx`, `e01hollow` and `e01flask`, the first and the last four from the owner's notes of 2026-10-25 (uncommitted records of that day). Then come the Phase 6 QA pass and the feature gate ([implementation plan](1.0/implementation-plan.md) §4, §2).
- The [paper designs](paper-designs.md) ran on 2026-10-23 (7.6): 37 register rows, folded into §3 by their `P7-` and `P8-` IDs.
- Today's Phase 7 is C1, W4b and the freeze gate, C2, W7b/W9b/W9c, C3 and the 1.0 gate (§5 there). The [Phase 7 rewrite](archive/phase7-rewrite.md) restructures it into performance work before the freeze and a process reset after it. Neither has a game-development pass.
- The feature gate cannot pass as written: it needs the acceptance game to play from a package in its own repository, which was never started. The owner now treats example 01 as the acceptance game.
- 01 is a sandbox on the template (`D-194`); the template's sample is a small top-down game. They are the kit's two consumers.
- Two earlier overhauls already changed the process: the 0.66 project-health pass (2026-10-13) and the Phase 7 rewrite draft (2026-10-16). No Phase 7 stop has started. §2 says what this redo keeps from each.
- §9 lists what adoption supersedes; nothing changes before it.

## 1. Why: what example 01 measured

From the owner's review session's four code sweeps (levels, physics, kit and tagging, assets) at `be16369`, a fresh-context recheck at `6773e75`, and the transition session's recheck at `bb1e4c0` (0.107, 2026-10-25). Figures are `bb1e4c0`'s; a figure that moved carries `@bb1e4c0` and the draft's figure in parentheses. Outside `examples/`, M81–0.107 changed only the kit (M83's animator fix, M84's controller shove and step climb, `D-213`) and one repository check. Everything else grew in 01: its `src/` went from 21,488 to 27,051 lines (`wc -l`; M81 alone added 2,935) and its tests from 14,796 to 18,933.

- **Process.** M32–M85 ran in three weeks (2026-10-04 to 2026-10-25), each with a plan, a decision entry, a changelog section, a register row and a catalog stop; 0.107 shipped a fix with a changelog section only. M78's ten quick fixes touched six record files under `docs/plans/1.0/`, and its release added the changelog, a decision entry, its index row and known issues. M79–M85's releases each touched 4–7 files under `docs/plans/1.0/` and 8–12 record files in all. `D-158` carries seven amendments (`@bb1e4c0`; six). Level C, with its API critique, exists "because the freeze makes API taste expensive to fix later" ([workflow](1.0/workflow.md) §4). A milestone whose code touches benchmark rows shows them not regressed, so 29 of M32–M78 ran capture sittings, and M81 of M79–M85; the rewrite's R10 already proposes counter guards in their place.
- **Kit.** About 17.7k lines of 01's gameplay are its own Rust (`@bb1e4c0`, tokei's lines in `src/features/`; 13.6k at `6773e75` by the same count, the closest to the draft's 13.5k), against about 2.5k of data (`assets/data/`). Element reactions are restated at seven delivery sites (`@bb1e4c0`; six at `be16369`; M81 added `props::heat`). 134 `has::<…>` checks (`@bb1e4c0`; 92) and 18 `(&World, Entity) -> bool` predicates (the draft's seven not reproduced; 13 at `6773e75`) decide interactions, while `TagFilter` appears only in 18 prefabs' data and never in 01's Rust. Statuses exist twice (a timed `Frozen` component and `status.frozen`). 29 backlog rows cite 01, 19 of them waiting for "a second game" or genre to ask (`@bb1e4c0`; 22 and 12), and the kit admits an item on recipes in two genres (`D-172`). `save.rs` is 1,989 lines (`@bb1e4c0`; 1,424) at schema 15; 01 orders 103 systems with 65 edges (`@bb1e4c0`; 84 and 50).
- **Levels.** Map objects take save IDs by order, so two more bats bumped the save schema (M78), and M81's goblins forced schema 12 with every later ID shifted by seven (`save.rs`'s `SHIFT_12`). The tileset is every sprite, sorted (1,949 entries, 513 of them used): 12 crawler frames renumbered 10,056 cells, about 50k lines of diff. Any edit regenerates all 4,815 generated files (`@bb1e4c0`; 4,172). `level.json` is 10,859 lines placing 1,266 props and decorations by number, with 9 crates and 5 kegs (`@bb1e4c0`; 10,220 and 1,194); 74 launch rows in `tests/game/level.rs` hold positions, with copies in the playthrough test; collision comes from three sources.
- **Physics.** Core physics is unchanged since `be16369`: no friction (01 runs its own `iron_friction`, and M81 added wood and rolling friction constants), rotation, joints, one-way platforms or slopes. Contact events carry no speed (01 runs four impact systems), sensors report entry only, and there is no radial impulse (`@bb1e4c0`: three radial copies in 01 and M84's sector push with falloff, `toys/wave.rs`; the draft's five not reproduced). The controller is not pushed by bodies (K7's open half); since M84 it shoves light bodies by handing over its velocity, opt-in (`D-213`). The draft's ten body-centre helpers (`@be16369`) had become `shared::body_centre` by `6773e75`.
- **Assets.** About 13.2k lines of generator code (`@bb1e4c0`, tokei; 10k) own every asset file and refuse any other; M84 emptied the hand-authored sounds list, so there is no exception left. M85 added a placement check that fails the build on floating scenery (`D-214`). One PNG per frame. Tools and generated assets were 2,588 of M76's 2,650 changed files. In-game captures for art review need a scratch crate per session. Large art sessions lose quality: M76's level art took six drafts.
- **Startup.** `resumed` loads every asset synchronously before the first frame (`crates/tungsten/src/app.rs`, `load_all_merged`); 01 loads about 4,105 PNGs and 95 sounds with nothing on screen (`@bb1e4c0`; 3,565 and 81). Not measured.

## 2. Owner answers and earlier work

### Answers, 2026-10-22 and 2026-10-23

Settled. A finding against one is recorded in §10 as a question, not applied.

| Topic | Answer |
| --- | --- |
| Roadmap | Redo it; phases after 6 may be expanded or replaced |
| Workflow | Project health first, before the next large body of work; workflow and high-value work pushed sooner |
| Benchmarks | The suite reworked in two parts, with an expansion pass |
| Phase 6 | Two more passes on 01 (`e01g`, `e01h`), then close. `@bb1e4c0`: both landed (M81, M82), and the owner's notes of 2026-10-24 and 2026-10-25 added (i) to (p) and the second quick fixes, of which six remain (§3) |
| Acceptance game | Example 01, accepted when the owner says it is |
| 1.0 | Promises a fully featured engine, tooling and kit to support game development, for platformer, top-down, story-driven RPG and shooter games with complex enemy AI |
| Breaking changes | Fine now; the API was overvalued and frozen too early |
| Focus | Seams and existing issues in the engine, kit and template before new surface; an analysis pass over 01 |
| Paper designs | Opus designs two games without code, a story-driven RPG and a shooter with movement and complex enemy AI, to find what the engine and kit fail to support |
| Interaction model | Chemistry-style: innate properties extended by data; damage types apply statuses and statuses react with damage types; statuses do much more than damage; statuses on tiles optional; reactions transform things; statuses have intensity; statuses spread to neighbours |
| Levels | Our own level format, easy for agents; slopes in scope |
| Loading | A boot loading screen; loading between areas not now |
| Saves | The work a game does to save is mostly done for it, in a simple format |
| Agents | A pass on each system so Opus and other agents work well with it: levels, assets, prefabs, game code |
| Guides | Unsure: a research and decision step (7.16) |
| Paper designs | Sharpened by §7.2's method, with Lantern Ward and Static Front as the default pitches; `grid` and `shmup` leave the kit's genre claims |
| 01 in Phase 8 | A refresh on Phase 8's systems with a few additions that demo them (8.4) |
| Performance in Phase 7 | Two small passes, one on the CPU and one on the GPU (7.14, 7.15) |
| Assets | Opus generates, shows the owner, takes feedback and repeats until accepted; no owner-side editing tool; small sessions handed to later ones |
| Proof of genres | No vertical-slice games; ultra-basic sketches at most |
| Admission rule | 01 or the template uses an item, and a recipe exists in the other genre (§5.3) |
| Process | Move toward agile by a gradient. From H1, the cheap habits (H1's operating rules). The redo's order is intent, re-cut after each game finding. The 1.0 batch is planned properly; after 1.0, fully iterative |

### Earlier work this builds on

- **The 0.66 project-health pass** (2026-10-13). No owner-approval stops; gates and plans take stated defaults (`D-150`). Opus max only where a session's judgment is final (`D-151`), with a stop's own tier first (`D-195`). Records written at a release's cut hold through its merge (`D-152`). H1 keeps `D-150`'s default rule and `D-151`'s tiers, except their level C clauses, which go with level C; it turns `D-195`'s per-stop tier into a rule, and replaces `D-152`'s owned README fields with `ROADMAP.md`.
- **`D-140`'s planning routes.** Bounded maintenance planned in the session, and a saved handoff plan for multi-session work, both stay ([plans README](README.md#choosing-a-route)). H1 retires only the third route: 1.0 milestone candidates with the milestone skill, the plan critique and level C. §4's plan sizes are the two routes plus a large plan file.
- **The Phase 7 rewrite** (2026-10-16, draft). Its owner answers stand except three: the cutover's timing, a bench suite written only once the kit and every core system are finished, and the roadmap site kept until Phase 7 proper. The review's own answers R-Q5 and R-Q7 (no reset work in Phase 6) yield as well. Its four critique rounds applied 19 findings.
  - **Where its parts go:** its runner work moves to 7.3 and its workloads and budgets to 9.1 (§5.8); its R1, R3, R4 (skills) and R5 move to H1 and H2; R6–R8 stay in Phase 9.
  - **Defaults carried:** R-Q4, `ROADMAP.md` at the root; R-Q9, every operating rule names the failure it prevents; R-Q11, the old records deleted at the end with the tree tagged and `just history`; R-Q13, frame and operation budgets gate; R-Q15, a final tag publishes on push.
  - **Changed:** R-Q14's one candidate becomes the second half only (9.1); O2's formats in the freeze become a version field and a pinned fixture per format that every later release still loads (§5.1, §5.5, 9.4).
  - **Moot:** O5 (the arity-named queries leave in 7.1), O7 and R-Q10 (no freeze), R-Q12 (01 is in the repository).
- **The kit catalogue** (2026-10-16; W13e–W13k landed as M60–M67). The owner's goal that night was to move 01's game-specific components into a generic kit that works across games from the start. The kit now exists; the owner's answer of 2026-10-23 is that parts of it came out generic along the wrong axis. §5.2 keeps the catalogue's structure and replaces its ailment model.
- **W4b's cleanup rows** (`w04-api-freeze.md`; known issues). The deprecated surface leaves in 7.1, since breaking is fine: the eleven arity-named queries, `DeltaTime`, `InspectorState`'s three row functions, and the 32 `#[allow(deprecated)]` sites that keep them alive, with the unused public surface beside them (`LightUbo::byte_size` and `display.scale_mode`, which carry no `#[deprecated]`). `missing_docs` gets a ratchet in H1's release checks so the counts only fall: 1,165 public items without rustdoc at `bb1e4c0` (core 880, render 219, umbrella 66; the kit denies the lint). `@bb1e4c0`: the draft's 3,213 at `12ccd36` summed per-crate clippy runs that each count their workspace dependencies again; distinct items were about 1,207 then. Rustdoc lands per system once it settles, and the rest in 9.4.
- **Workflow proposals carried by the 0.66 pass** (known issues): the archived AI-workflow research plan's proposals 4, 5, 6, 7 and 9, which are still unapproved. They are deny-list bans and a decision-row check; a released-tag consumer check and an early RC-A9 pilot; refreshed client discovery and exact-command permissions; behaviour and visual evidence for feature reviews; `.editorconfig` and `.gitattributes`. H2 decides each. The consumer check is 01 built outside the tree on a tag, and the RC-A9 pilot is §8's first evaluation run.
- **C1's groundwork.** M76 recorded 01's full pit with enemies awake as C1's game row (`w10-closing-phases.md`'s follow-ups, `tests/game/frame_time.rs`), and later stops changed what it measures, so 7.3 ports it and 9.1 retakes it. Known issues propose that new benchmark rows land with the stops that add their features, which §5.8 adopts.

## 3. The roadmap

Ordered by what pays back soonest. The first items in Phase 7 cut the cost of every change after them, so they come before the analysis pass that scopes the rest.

### Phase 6, closing

`@bb1e4c0`: the draft planned `e01g` (M81), then the transition session and H1, then `e01h` (M82) under H1. Instead M81–M85 and 0.107 landed under the old flow (`e01g` props and foes, `e01h` the look, `e01tune`, `e01push`, `e01scene`, and lava biting enemies; `D-210`–`D-214`), and the owner's notes of 2026-10-24 and 2026-10-25 added six stops.

1. The transition session and H1 (§4), 2026-10-25, ending with 0.108, the old pipeline's last release.
2. `e01quick2`, quick fixes from the fourth playtest, under H1. The owner's prompt named `e01tomes` as the next open stop; the notes of 2026-10-25, written to the records the same day, put `e01quick2` before it.
3. `e01tomes` the tomes (M86 in the owner's prompt), then `e01foes`, `e01sfx` (with K29's distance falloff in the kit), `e01hollow` and `e01flask`: `roadmap.json`'s remaining example stops, in its order, which H1 carries into `ROADMAP.md`. Each is re-cut after the owner plays the one before it (H1's habits). The first of them that draws new art is the first trial of §6's asset sessions on today's generator: an art session generates and writes the review sheet, the owner reviews in rounds, then a code session leaves `tools/` art alone.
4. The Phase 6 QA pass's correctness list as small tasks under H1 (Q10).

### Phase 7: Seams

| # | Step | Done when (sketch) |
| --- | --- | --- |
| 7.1 | Quick wins | W4b's deprecated surface removed. Map objects keep authored IDs (P7-9), after one last schema bump and migration, so map edits stop bumping saves (7.9 later changes the format once, with its own migration). The tileset holds only the tiles the map uses, in a stable order, written compactly; this is interim until 7.12's format, which keeps the IDs. Asset families generate on their own; accepted assets are locked; anchors (the lantern's are data since M72), `BODY_OFFSET` and hurtboxes are data beside their sprites. Gamepads read again: M54's step 5 re-landed with its licence files, a radial dead zone and a sensitivity setting (P7-12). One integration-test binary per crate (R7's compile-time item). Small known-issue fixes. Measured: an enemy added to the map needs no schema bump, and a sprite change outside the terrain touches no level file. A terrain change moves only the props `D-214`'s placement sets on it |
| 7.2 | Agent tooling foundations | `tungsten run --script` (inputs at frames, assertions, screenshots, a JSON report); `tungsten render` (a scene, an area or a prefab to PNG with overlays, on the capture path); `tungsten describe`. Asset review sheets and in-game captures come from `render` instead of a scratch crate |
| 7.3 | Benchmarks, part 1: runner and guards (§5.8) | The rewritten runner; CPU counter guards in `just check` and CI, GPU counters local; a minimal row set ported; the old suite and data purged with the carry-overs kept; `tungsten-perf` rewritten with it; per-milestone performance sittings retired |
| 7.4 | H2 (§4) | The release script and its rehearsal, `just status`, the skills rewritten against baselines, the carried workflow proposals decided, agent memory pruned; the overhead remeasured, and lower |
| 7.5 | Analysis pass over 01 and the template (§7.1) | The seam register written and verified; the owner's triage done |
| 7.6 | Paper designs (§7.2) | **Done** 2026-10-23 ([paper designs](paper-designs.md)): two designs with verified build sheets (59 systems: 12 data only, 22 game Rust, 24 gaps, 1 blocked); 37 register rows, folded into this table and Phase 8's by their `P7-` and `P8-` IDs |
| 7.7 | Phase 7 plan from the registers | Every later step's rows assigned, and 7.14's and 7.15's finite lists named (§5.8) |
| 7.8 | Terrain and physics | First the runtime terrain that physics and chemistry stand on (§5.1): per-tile properties (material, health), layers and one-way edges, outline collision from the tile grid, the Rust auto-tiler, and runtime tile changes made in place and re-tiled and re-outlined locally, where today a change clones the map and rescans every collision layer (P7-1). Then §5.6 physics on it: impulses, radial impulses and contact speeds (P7-2), sensor exit (P7-3), with slopes and the controller last |
| 7.9 | Saves and engine seams | §5.5 saves on 7.1's IDs and 7.8's settled physics state, each area's changes under its own level reference (P7-8, G4): `save.rs`'s generic part gone from 01, one-call capture and restore in the template. §5.4 boot loading, with sprite sheets (frames as rectangles in one image) in place of one PNG per frame (P7-15). The feedback plan's small seams (K2 particle depth, K4 animation rate, K5 text outline (P7-14), K9 lit-sprite flash), a tile layer's depth among sprites (P7-10), sound hot reload, and looping cue voices with per-voice pan and bus ducking (P7-13; distance falloff lands earlier as K29, planned for `e01sfx`) |
| 7.10 | Kit pass | §5.2 chemistry, with today's effects merged into statuses (Q17), tile statuses, bands, fuel and `become` (P7-4), and light as a stimulus on a core light-at-a-point query (P7-5). 01's generic code absorbed: blasts (K10's radial payload), cones, conduction, impact damage and bodies as walls by layer (P7-2), the saved chain queue, draining, timers with expiry and saves, named world cues, effect looks, one-shot animator clips (P7-19), hitbox facing (K3, P7-20). Ability fixes: a direction choice, a held trigger, a public refill countdown and K8 (P7-6), after whatever of K25 and K26 `e01tomes` puts in the kit. A hit that carries its angle (P7-7). Motion on 7.8's physics; plugin defaults over hand ordering. The kit's genre tags become `platformer`, `top_down`, `rpg` and `shooter`, with recipe folders from the paper designs, `grid` and `shmup` dropped and one-genre items exempt (P7-18, G1–G3, §5.3) |
| 7.11 | Template updates | Boot splash and loading, one-call saves, the register's template rows |
| 7.12 | Level format, agent CLI, editor (§5.1) | The authoring format on 7.8's runtime terrain, with named places, routes, links and a file per room (P7-17), and the CLI, first; seeded generation with IDs from the seed, `check` over seeds, saves as the seed plus changes and a registry `remove` (P7-16); `check`'s "props supported" generalizing `D-214`'s floating check; the in-game editor on the same commands last |
| 7.13 | 01 on the new systems | 01's reactions, statuses, saves and level on Phase 7's systems, with M81's props and kegs and the later stops' additions as chemistry data; a fresh-agent evaluation run (§8); the owner accepts 01 |
| 7.14 | Performance pass: CPU (§5.8) | With 01 on the new systems as the workload, the measured top CPU costs that Phase 8 will build on: the ECS's `has` and query paths (K12), per-entity kit systems (K13), the spatial index after 7.8, chemistry spread, the save snapshot, the extract. Each is fixed only for a measured cost on a row, with no budgets yet; the whole-frame row and the GPU rows stay not regressed |
| 7.15 | Performance pass: GPU (§5.8) | The same for the GPU, on 7.3's non-stalling timestamps: the lit-sprite path, the light cull past 16 lights by importance, pinning the player's light and skipping dark ones (P7-11), bloom and the post stack, particle quads, sprite batching and upload bytes, tile layers. The CPU frame, the extract included, stays not regressed |
| 7.16 | Agent guidance: research and decision | How engines and repositories guide agents (task guides, `AGENTS.md`, shipped skills) researched against 7.13's evaluation; what the template ships, decided |

Every system step from 7.8 on adds its own benchmark row and counter guard as it lands (§5.8).

### Phase 8: Genres

| # | Step | Done when (sketch) |
| --- | --- | --- |
| 8.1 | Story systems | A condition vocabulary for data; dialogue with reachability, speaker and reference checks, a dialogue box and a `say` cue action, beside P7-3's `Interactable` (P8-1); quests and a journal on its conditions (P8-2); items, inventory with drag and drop between widgets, and shops, with equipment staying statuses (P8-3, Q17); cutscenes and timelines (P8-4); text input from the window's characters, IME later (P8-5); a world clock with phase events and schedules resolved by time, so an NPC is placed while its area is unloaded (P8-6); loading between areas as spawning and persistence on 7.9's saves of many areas, sliced on the main thread (P8-7); a cone light kind, occlusion optional (P8-17). Recipes for each, sketches where feel matters, a benchmark row each |
| 8.2 | Shooter and AI systems | Navigation: grid A\* and flow fields with door links and local repaths on 7.8's terrain (P8-8), and on platforms (jump links and ledges, which 8.4's goblins need; no paper design covers them); AI structure, hierarchical states or utility scoring, with squads and a cover query on the terrain (P8-10); perception with throttled sight, hearing as noise stimuli, memory that decays, sharing within reach, and reading smoke and light (P8-9); hit-stop that leaves game-time windows intact (P8-12); weapons as data: spread, pellets, bursts, recoil, a timed reload, spin-up and charge (P8-11); bullet density with a pattern payload, and waves (P8-13). Rumble stays out (P8-14, G5) and aim assist stays the game's (P8-16). Recipes, sketches, a benchmark row each |
| 8.3 | Tooling for hand-made content | Aseprite import (tags to animations, slices to anchors and hitboxes); `asset add` for a drawn file or sheet, which Lantern Ward's 66 portraits need (P8-15); the Tiled loader's place beside our own format (Q16). A drawn sheet and an Aseprite file each enter a `tungsten new` game with one command |
| 8.4 | 01 refreshed on Phase 8's systems | Not a rewrite: 7.13 already moved 01 onto Phase 7's systems. Its enemies move onto the AI structure, perception and navigation, and it gains hit-stop. A few additions demo Phase 8 in a platformer sandbox: an NPC with a few lines, one choice and a small fetch quest across two areas; a short intro cutscene; a satchel inventory with drag and drop; goblins that path around terrain, take cover and flank as a pack; one or two hand-made sprites through Aseprite import; areas streamed in. Each art need goes through §6. Done when 01's own gameplay Rust shrinks again, each addition is mostly data with any Rust counted, 01's tests pass, and the owner plays it through |
| 8.5 | Template final pass | The template on Phase 8's systems, in a fresh `tungsten new` copy: the agent surface (§8) and the scaffolds (`new feature`, `new enemy`, `new status`) work there; 7.16's shipped guidance is in it; shell polish lands in the template and is carried into 01's byte copy of the shell. One sample, no genre demos (the genres' breadth is in recipes, Q6), within `D-181`'s budget. Done when a fresh agent on a `tungsten new` copy completes §8's five evaluation tasks with no engine patches. A starter per genre (`tungsten new --genre`) is a 1.x backlog note |
| 8.6 | Proof | §8's 8.6 evaluation: tasks on 8.4's additions inside the refreshed 01, and the paper designs' ten weekly tasks on a `tungsten new` copy; the owner accepts each genre's recipes and sketches |

This list was the paper designs' prediction; 7.6's registers (`paper-designs.md` §4) confirmed it: every 8.1 and 8.2 item but navigation on platforms has a row. The rows added the clock and schedules, the cone light, waves, the cover query, the dialogue checks and the `say` action. The `P8-` IDs name them.

### Phase 9: Release

| # | Step | Contents |
| --- | --- | --- |
| 9.1 | Benchmarks, part 2: expansion and C1 (§5.8) | The rows Phases 7 and 8 added, filled out with genre, structural-stress and operation rows; the measured frame timeline; budgets by category (R-Q13, criteria Q12); the dated baseline. One candidate (R-Q14, narrowed) |
| 9.2 | Performance passes | Rows over budget, largest gap first, on internals; the rewrite's §2.1 levers as a list without its breaking-later classes; the render thread (W2's R4) only if a row over budget needs it (`D-102`) |
| 9.3 | Stability policy | Q8 decided and recorded |
| 9.4 | Quality | Rustdoc until `missing_docs` is clean, rustdoc examples, a pinned fixture per format (O2 as changed in §2) |
| 9.5 | Docs and platforms | The rewrite's R6 triage, with 7.16's guidance; W9b's final pass on the getting-started guide; W7b's platform tiers in the README (criteria Q3) |
| 9.6 | Tests | The rewrite's R7 crush on the settled API |
| 9.7 | Records | The rewrite's R8: the old records deleted and the tree tagged (R-Q11) |
| 9.8 | Release candidates, then 1.0 | On H2's release script (R3): C3, the release checklist (revised at adoption) run on each candidate; W9c's crates.io only if criteria Q2 says so |

Throughout Phases 7 and 8, each system ships its agent surface (§8) in the step that builds it, and each art need runs through §6.

## 4. Health: H1 and H2

Pulls the rewrite's R1, R3, R4 and R5 forward, builds on the 0.66 pass (§2), and drops the freeze's process. It comes in two parts. H1 is what stops the overhead: it runs before `e01quick2`, so the remaining example stops already cost less. H2 is what needs real tasks to get right: it runs after 7.1–7.3 have given it some. The old records stay, no longer growing, until 9.7 (the owner's answer of 2026-10-16). The operating rules live in one doc, each naming the failure it prevents (R-Q9).

**H1: the cutover's essentials**

| Area | Change |
| --- | --- |
| Git | Agents commit at each logical step in Conventional Commits, checked by a `commit-msg` hook, and stage only the paths they changed (never `git add -A`). A second session in parallel works in its own worktree or clone. The owner pushes. Until H2 moves publishing to a tag push (R-Q15), the release branch and pull request stay the publish step, merged without squashing so the commits survive; then the per-version branches retire (supersedes `D-105`'s uncommitted tree, and `D-079`'s commit clauses and its squash) |
| Records | History stops growing per change: no register rows, catalog stops, roadmap-site syncs or decision amendments per milestone, and a release's changelog section comes from its commits. Maintained docs (`DESIGN.md`, the LLM index, `README.md`, `docs/assets.md`, `docs/validation.md`, known issues, the template's docs) are still corrected in the same commit as any change that makes them wrong. Each frozen record gets a one-line banner naming what replaced it |
| Checks and scripts | Everything that reads a record H1 freezes is retargeted or removed in the same change, since CI runs `just repo-check` and `just script-test` (`ci.yml`): the repository check's `check_roadmap` and `check_summary` (the README block's version check fails at the next cut once the block is frozen), their tests and the `main()` test that patches them; its `just checks` mapping for `roadmap.json`; `roadmap.py` and its test; `release-cut`'s summary write in the `justfile`; `tungsten-next`'s `state.sh`, `SKILL.md`, `roadmap.md`, `prompts.md` and `page.html`; `tungsten-milestone`'s register and README steps and its `skeleton.md`; the docs that name the owned fields (`validation.md`, `releases.md`, the 1.0 README, the decision index's prose). A frozen record keeps its `draft` or `in progress` status under its banner, since `check_plans` refuses a finished status outside the archive. `decision_index.rs` stays, since decisions continue |
| Read path | `AGENTS.md` and the LLM index point at `ROADMAP.md` and this file in place of the register, the catalog and the implementation plan |
| Status | `ROADMAP.md` at the root (Now, Next, Later) replaces the register, `roadmap.json`, the roadmap site and the 1.0 README's owned fields (supersedes `D-109`, `D-118`, `D-152`); `tungsten-next` reads it. Stops keep their IDs (`e01tomes`); milestone numbers end with the register at M85 |
| Plans | `D-140`'s routes, with the 1.0 milestone route retired. Small: none, the commit is the record. Medium: in the session. Large: one plan file, deleted when done. Done means the tests pass and a demo works. A large plan or a risky change gets one critique round at xhigh, and a second only when the first changed a mechanism: round 2s have mostly repaired round-1 fixes, and max has shown no lift over xhigh. `tungsten-milestone` trimmed to that |
| Decisions | Only for choices that constrain later work (dependencies, formats, policy), short |
| Freeze process | Removed: the break ledger, level C's API critique (critique stays for risky areas: concurrency, formats, the render seam), the kit's rule that prefab and scene files keep their fields (w13 §14, `D-153`, `D-157`), `#[non_exhaustive]` sweeps. The API snapshot stays as a free diff |
| Releases | Cut when something is worth shipping, not per milestone, on a working path (Q13) |
| Tiers | `D-151` stays but for its level C clause. `D-195`'s per-stop tier leaves `roadmap.json` (which `roadmap.py`'s `tier()` reads, for the roadmap site) for a rule, extending `D-195`'s reason: a session that generates art or sound runs on Opus max, a code session at xhigh |
| Assets | §6's procedure, on today's generator |
| Habits | The owner's agile gradient (§2), as operating rules, each naming its failure. A new idea is parked as one line in `ROADMAP.md`'s Later, not planned (ideas planned on arrival grew the queue: the 2026-10-25 notes added five stops in a day). Now holds one to three items (a long Now is a waterfall again). The owner plays before the next thing is planned (each playtest since 0.97 added or re-cut example stops planned before it). An agent names scope creep when it sees it and proposes the smallest version (`AGENTS.md`'s silent "no scope creep" lets a task grow unseen). §3's order is intent, re-cut after each game finding (a fixed order is how Phase 7 went unrevised while 01 measured the seams) |
| Docs | The `missing_docs` ratchet in the release checks, from 1,165 distinct items (§2) |
| Measured | Before H1, a baseline from M76–M85: record files touched per change, plan lines per change, the share of a session spent on bookkeeping. Retaken after the first example stops under H1 and the first quick wins. **Taken 2026-10-25** (`perf-runs/20261025-overhead-baseline/`, with its scripts and method): a median of 12 record files per release (116 over ten); a median plan file of 312 lines, 450 with the record edits around it; 17 sessions, of whose 8,696 tool calls 22.1 % were bookkeeping (15.9 % with each milestone's own plan file counted as work), and 20.0 % of output tokens. Plan-only sessions ran 19–49 % bookkeeping, run sessions 15–24 %. The share is a floor: intake and prompt sessions (404 calls, 41–75 %) are left out |

**H2: the rest, on real tasks (7.4)**

| Area | Change |
| --- | --- |
| Releases | One `just release <kind>` with its rehearsal; a final tag publishes on push (R-Q15). Its first kind is a 0.x release that may break anything and bumps the minor version, as today, because Phases 7 and 8 keep breaking. The rewrite's R3 kinds (patch and minor refusing breaks, the frozen RC series, `promote`) switch on at 9.8's first candidate, and are rehearsed there |
| Status | `just status`, derived (R5) |
| Skills | Each rewritten against a no-skill baseline on the tasks 7.1–7.3 produced (the rewrite's R4): a lean planning skill in place of `tungsten-milestone`, a lean `tungsten-next`, one release wrapper, `tungsten-decision`; `tungsten-perf` already rewritten at 7.3 |
| Proposals | The five carried workflow proposals (§2), each decided |
| Agent memory | The plan, perf and release note hubs pruned to what still applies |
| Measured | The overhead retaken, and lower than H1's |

## 5. Designs the steps start from

The shapes agreed in the session; each step's plan owns the mechanisms.

### 5.1 Levels (7.1 interim, 7.8 runtime terrain, 7.12 authoring)

Runtime terrain (7.8) is everything physics and chemistry need at play time: per-tile properties, layers and one-way edges; outline collision; the Rust auto-tiler; runtime tile changes. It works on today's tile grid. Authoring (7.12) is the format, the commands, the CLI and the editor on top of it. The bullets below cover both.

- **Format.** Our own: one file per room, written by one canonical serializer (sorted keys, stable order, fixed number formatting), with a version field from the start.
- **Identity.** Readable authored IDs (`falls.crawler_2`); save IDs derive from them. 7.1 brings them to today's map first.
- **Terrain as shapes.** Rects, polygons and polylines with a style; one-way, material and layer per edge; grid styles auto-tiled by neighbour mask in Rust at load, headless; per-cell overrides for touch-ups; strip styles for freeform art later (Q3).
- **Collision from outlines.** One-sided segment chains traced from the solid region, in the terrain path where tiles live today, so `Shape` stays `Copy`; no seams between joined boxes.
- **Objects.** A prefab ID with overrides, in tile units, optionally anchored to a named surface so it moves with it.
- **Named things.** Areas, markers, routes and links; tests read names, not coordinates.
- **Commands.** `place`, `move`, `set`, `delete`, `duplicate`, `paint`, `erase`, `link`, `rename`: validated, invertible, journaled. Front-ends: the CLI (`tungsten level apply`, `query`, `check`, `render`, `describe`, on 7.2's foundations), the in-game editor and `tungsten run --script`. Live edits arrive through a watched folder and apply on the main thread (the two-thread rule). A room file's hot reload patches the live room rather than only its tiles, which takes the backlog's live-entity patching.
- **Checks.** Structured errors by room, ID and field path; spawns clear of solids, props supported (01's generator already fails on floating scenery, `D-214`), routes reachable with the controller's own numbers, light and entity budgets.
- **Generation.** A generator is a client of the commands or builds a room in memory. Authoring-time output stays editable: generated objects carry their generator, and a hand edit detaches one. Runtime output spawns through the loader with IDs from the seed and saves as the seed plus changes. Such a save also names its generator's version and each room template's, since the same seed under a changed generator yields other entities and voids the saved changes; a generator change either keeps the old version reproducible or ships a migration, and a test loads a save from before a generator and template change. `check` runs over many seeds headless.
- **Slopes.** Fixed angles (45° and 2:1) in the tile art and the auto-tiler; physics in §5.6.

### 5.2 Chemistry (7.10)

- **Properties:** hierarchical tags on prefabs and tiles (`material.wood`, `flammable`, `conductive`); numeric properties as stats.
- **Stimuli:** a damage type is one; contact, heat, wetness, light, a blast and an impact are others; statuses emit them. Light reads a core light-at-a-point query over `Light` components (P7-5).
- **Statuses:** data with an intensity from 0 to a max, decay and stacking. Bands at thresholds grant tags and behaviour: chill slows in proportion and freezes at its top band. Effects scale with intensity through `curves`. A status can change stats, tags, physics and behaviour, emit stimuli on a period, carry a look (tint, particles, light, a sound loop) and run outputs when it ends. No `Health` is needed.
- **Interactions:** a stimulus matched against the receiver's properties and statuses, the most specific rule winning. Outputs: build, apply or remove a status; spawn; blast; emit a stimulus; damage; transform (`become` a prefab or `set_tile`, optionally reverting); consume intensity.
- **Spread:** a status emits a stimulus in a radius on its period, to entities and tiles.
- **Tiles:** statuses optional per layer or style, stored sparsely by cell.
- **Fuel:** a drained stat whose zero runs the status's end outputs; it covers the backlog's draining effect.
- **Guardrails:** the kit names no element. Rules match tags and thresholds, outputs come from a fixed vocabulary, there are no conditions or variables, and anything beyond calls a game's named Rust handler. One fixed phase after damage resolves, on the fixed step, in a stable order, with a per-step cap through a saved queue; the kit's determinism hash moves once, under 7.3's guards. Physics layers stay collision-only.
- **Bounded work:** the cap bounds processing, not growth, so pending work is bounded separately, as 01's cascade already does it: one pending entry per kind and target (`features/elements/cascade.rs:207-211`), a queue bound with a counted drop (`:212-213`), and a chain-depth limit so cyclic rules end. Tests show a two-rule cycle ending and a spread flood at 12,000 bodies staying under the bound.
- **Replaces:** the ailment ladders on `damage_types`, the buckets and `ailment_threshold`, and today's effects, merged into statuses (Q17); in 01, its element state on bodies, the cascade with M81's detonations, the restated reaction sites, `BallBurn`, `Frozen`, M81's `Fuel`, the hazard douse, the lava-frost prefabs and M85's lava front.
- **Needs:** a cheap "properties within r" query, which K14's per-step index rebuild makes costly today; runtime tile changes (7.8's runtime terrain); identity kept through `become` (§5.5); looping cue voices.
- **Naming:** the kit's `react` domain (cues, counters, levels, loot) and its `kit_react` phase already exist, so the rules here need a name that doesn't collide (Q11).

### 5.3 Admission rule (7.10, Phase 8)

Phase 7 items: 01 or the template uses the item, and a recipe exists in the other genre. An item whose job is one genre's (the controllers) needs only 01's or the template's use (G1). Phase 8 items: a paper design or 01's refresh (8.4) needs it, recipes cover it and a sketch exists where feel matters, and the owner accepts; navigation on platforms enters through 8.4's goblins, since neither design covers it. The kit's genre tags are the four 1.0 genres, `platformer`, `top_down`, `rpg` and `shooter` (G3; the paper designs' §5 shows their uses bring 21 of the 24 components the drop of `grid` and `shmup` would strand back to two genres). This amends `D-172`'s two-genre recipes with a real user, and re-triggers the backlog rows that wait for "a second game": 01's use is enough.

### 5.4 Boot loading (7.9)

A `boot` asset group (a font, a logo, a bar) shown first, then the rest loaded in time-budgeted slices on the main thread with progress; the template's shell shows a splash. A startup capture comes first, and 7.3's operation rows hold it after. A decode worker thread only if the capture shows decoding dominates, as its own decision under the two-thread rule. Loading between areas waits for 8.1.

### 5.5 Saves (7.1 IDs, 7.9)

A component registers as saved in one line, and kit components register themselves. Map objects keep §5.1's IDs and runtime spawns take IDs from their spawner. A save is plain JSON: per area visited, its level reference with its changed components, despawns and runtime spawns in full (G4: Lantern Ward keeps five areas); then registered resources. Each component carries its own version and migration, so adding enemies to a map never touches a save. The game writes only state that is no component, and the template's capture and restore become one call each. The autosave's main-thread cost has a benchmark row and a budget.

Entity references inside saved components (owners, sources, hit targets, contact history, references held in resources) are saved as stable IDs and remapped in a second pass once every entity exists, as 01 does by hand today (`save.rs`'s `name_targets` and `name_sources`, lines 1904 and 1925). A reference into an area that is not loaded stays a stable ID until that area loads, and resolves then; nothing in a loaded area may require it before. 7.9 designs this with saves of many areas, and 8.1's loading between areas tests it: a restore, then a move into the area a saved reference points to. The acceptance test is a restore into a world whose entity allocations differ from the saved one, with 01's existing save tests passing unchanged. One of them checks that an attack saved mid-hit never hits twice after a load (`tests/game/saves.rs:1157`, `@bb1e4c0`; 1144).

### 5.6 Physics (7.8)

Friction first; contact events with closing speed, begin and end; sensor exit; impulses and radial impulses; one-way edges; outline collision (§5.1); the controller pushed and riding, with M84's opt-in shove (`D-213`, a velocity hand-off inside the controller) moved onto impulses and contacts, and a controller with locked input no longer stopping dead at a body it cannot shove (K30); slopes with floor normals, climb and slide limits and snap-down, in place of M84's step climb, which sets the position where no contact sees it; `Shape` made `#[non_exhaustive]`; a cheaper query index (K14). The physics hashes move with these, under 7.3's guards. Rolling and joints: Q1, Q2.

### 5.7 What 1.0 promises

A fully featured engine, tooling and kit to support game development in four genres. No API stability promise at 1.0; the policy after it is 9.3's decision (Q8).

### 5.8 Benchmarks in two parts (7.3, 9.1)

The rewrite's §2.2 is the starting list; the owner's answer of 2026-10-23 splits it. The runner comes first, so the churn of Phases 7 and 8 is guarded. The workloads are expanded and budgeted once the systems exist.

- **Part 1, runner and guards (7.3):**
  - **The runner:** verdicts against absolute budgets as well as A/B compares; a same-sitting A/A and the one-rerun way out for known drift rows, built in; code-placement noise handled by default; a contamination check at sitting start; RSS compared within a sitting only; GPU timestamps read back without stalling the frame (today's timed frame blocks on `device.poll`); a short guard mode beside the full sitting.
  - **Counter guards:** deterministic counters as noise-free guards. In `just check` and CI, the CPU's: allocations, archetype moves, events, and the extracted instances and batches the headless harness's `FrameDraw` holds. The harness has no renderer (`crates/tungsten/src/testing.rs:2`), so the GPU's draw calls and upload bytes are local checks beside `just smoke` and `just visual`. Time rows stay local too (`D-070`).
  - **Rows:** a minimal set ported, the old suite's core rows that still measure something, plus 01's full pit and the template frame (Q12). No budgets yet; provisional baselines only.
  - **Purge:** the old suite, its tests and data purged, after carrying `benchmarks.md`'s engine findings and open proposals into 9.2's lever list and keeping the machine-level capture facts that stay true.
  - **Retired:** per-milestone performance sittings, replaced by the guards and a sitting per minor or major release (the rewrite's R10).
- **Rows as systems land (7.8 to 8.2):** each system step adds its row and counter guard in the same change: save snapshot cost, boot to first frame, friction and slope piles, interaction spread at 12,000 bodies, navigation and AI crowds, dialogue-heavy UI frames, bullet density.
- **Mid-roadmap passes (7.14 CPU, 7.15 GPU):** after 7.13, with 01 on the new systems as the workload and the rows 7.8–7.13 added.
  - **Capped:** measured structural costs that Phase 8 builds on, each fixed only for a measured cost on a row, with no budgets yet (9.1 sets them). 7.7's plan names each pass's finite list (the largest measured costs among the step's candidates) with a target per item, a share of the row's time to remove; a pass ends when its list is done, and everything else waits for 9.2.
  - **Split by resource, but guarded.** The rewrite argued against splitting its final passes by area, since extract cost moves across the CPU–GPU seam (its §2.4). Here each pass keeps the whole-frame row and the other resource's rows not regressed, which answers that.
  - **The payoff:** 9.2's passes start from a lower floor.
- **Part 2, expansion and C1 (9.1):**
  - **Expansion:** genre rows from Phase 8's sketches and 8.4's refreshed 01, and the paper designs' stress numbers; structural stress rows (entity-count scaling, spawn and despawn churn, event volume, load and hot reload, startup parse); operation rows (boot, a level load, a save, a hot reload). Each row names its budget category and the surfaces it exercises.
  - **C1:** the measured frame timeline, budgets by category (frame, operation, scaling; R-Q13 and criteria Q12) and a dated baseline on every row.
  - **Then:** 9.2.

## 6. Asset sessions

| Stage | Who | Output |
| --- | --- | --- |
| Brief | The planning session | One asset family: IDs, sizes, anchors, frame counts, clip names, palette, art-direction notes |
| Generate | An Opus asset session at `D-195`'s tier, on that brief only | The files and a review sheet: a contact sheet, animated previews, in-game captures |
| Review | The owner | Notes or acceptance; rounds repeat, in a new small session when the context grows |
| Lock | Mechanical | Accepted files' hashes, enforced by the generator's check |
| Build | Code sessions at xhigh | Code and data against the brief's contract, on placeholders until the art is accepted |

The brief's contract (sizes, anchors, frame counts, clips, hitboxes) is what code depends on, so art changes within it touch no code. The procedure starts at H1, and the first example stop under H1 that draws new art is its first trial (the draft named `e01h`, which landed as M82 before H1). Its mechanics land in 7.1 (generation per family, locks, anchors and hitboxes as data) and 7.2 (review sheets from `render`). Sounds and music follow the same loop (Q5).

## 7. Briefs

### 7.1 Analysis pass

- **Goal:** every place 01 compensates for the engine, kit or template, with evidence, ranked by the code a generic fix would delete and how often the pattern recurs.
- **Scope:** 01's `src/`, `tests/` and `tools/`, and the template. The engine and kit are read only to verify claims. Bugs are noted, not chased. Seams 7.1–7.4 already fixed are marked as done, not re-reported.
- **Readers:** about eight, with small folders grouped (enemies; elements; toys; level and tools; spells, gun and lightning; combat, player, audio and run; `save.rs` and the root files; the template), each reading its share whole and filling one row shape: pattern, `file:line`, lines, what the engine, kit or template lacks, the generic fix, dependencies.
- **Lenses:** helpers copying engine logic; Rust where data belongs; markers where tags belong; hand-ticked timers; per-kind arms; constants restating data; save and ordering boilerplate; performance workarounds; positions copied into tests; duplication in the tools.
- **Seeds:** the feedback plan's K-rows (K1–K31, `example-01-feedback.md` §6; K6, K15–K18 and K24 are fixed), the backlog rows citing 01, known issues, §1's sweeps, and the paper designs' registers (already folded into §3, so not re-reported).
- **Then:** a merge pass, a fresh-context check of each row against the source, the owner's triage.
- **Stop:** every folder read once and every row verified; no second rounds. The 2026-10-14 whole-repo review is the cost lesson.

### 7.2 Paper designs

- **Scope:** each design is a game one developer with agents could ship in a few months; a probe pitched as a large commercial production finds gaps no 1.0 needs. Original, not modelled on an existing game.
- **Coverage first:** the session writes the coverage matrix before the designs: view and movement, world structure, AI, entity counts, chemistry, story and UI, saves, input, pipelines. The two designs split it between them, complementing 01 and each other.
- **Default pitches,** which the session may improve on:
  - **Lantern Ward, a story-driven RPG (top-down).** The new night warden of a fog-bound harbour town. By day: dialogue, trade, quests and the townsfolk's trust. By night: the fog brings things that hunt by sound and fear light, and the lamps are kept lit with oil and fire. It stresses:
    - dialogue with conditions, quests, a journal, NPC schedules and a day-and-night cycle
    - shops, inventory and equipment
    - cutscenes, five streamed areas and a persistent world in the save
    - lighting as gameplay, and chemistry: oil, fire, wet fog, light as a stimulus
  - **Static Front, a tactical twin-stick shooter (top-down).** Clearing a derelict research facility of rogue security units, eight rooms a run built from room templates. It stresses:
    - squads that take cover, flank, suppress and retreat
    - perception by sight and sound, with memory
    - navigation around cover destroyed at runtime
    - dense projectiles meeting moving bodies, and hit-stop
    - weapons as data (recoil, spread, reload) and gamepad aim
    - runtime generation, and destructible terrain through chemistry: explosive canisters, fire, EMP on `conductive` things
- **Per design:**
  - One page with numbers: map sizes, entity counts, statuses and interactions, screens, what a save holds.
  - A production list: sprites, sounds, rooms, dialogue lines and enemy kinds, and how each is made with our tools, so the pipelines are probed as well as the runtime.
  - Five agent tasks a developer would hand an agent every week; these become 7.13's and 8.6's evaluation tasks (§8).
  - Stress numbers (bullets on screen, agents awake, NPCs loaded), which become 9.1's rows.
  - A use of the engine's lighting, post stack and chemistry.
- **Build sheets:** per system, how it would be built today, naming engine and kit items, data kinds and prefabs, with one verdict: data only; game Rust (size and why); gap (what is missing); blocked (a rule or the architecture).
- **Walkthrough:** one minute of play per design, tracing the systems that run, the events, their order and what is saved.
- **Evidence:** every "data only" or "game Rust" verdict cites an item in `api/*.txt` or the components snapshot; every gap names what was searched for. A fresh-context verifier checks the citations; `/critique` on the build sheets is optional.
- **Output:** rows in the analysis pass's shape, tagged by design. Rows on systems Phase 7 rebuilds fold into Phase 7; the rest scope Phase 8. Neither design covers navigation on platforms (jump links and ledges), so it joins 8.2 for 8.4's refresh of 01.
- **Genres:** the kit's descriptions also claim `grid` and `shmup`. Neither design tests grid placement or building, and Static Front covers what a shmup would, so 7.10 drops both tags and the kit claims §5.7's four genres as `platformer`, `top_down`, `rpg` and `shooter` (§5.3).
- **Ran 2026-10-23** ([paper designs](paper-designs.md), against the API snapshots at `3f82795`; two fresh-context verifier rounds). The transition session folded its registers in: the 20 `P7-` rows into 7.1, 7.8, 7.9, 7.10, 7.12 and 7.15, the 17 `P8-` rows into 8.1–8.3, each by ID. At `bb1e4c0` none is fixed; P7-2, P7-4, P7-8, P7-9, P7-15 and P7-19 grew in 01 since. What the designs changed here:
  - **Two new 7.x items:** gamepads (P7-12), into 7.1, since no window reads a pad and a twin-stick shooter cannot ship without one; the light cull by importance (P7-11), into 7.15.
  - **Saves of many areas** (P7-8, G4): §5.5's save holds a level reference per area.
  - **Genre tags** `rpg` and `shooter` beside `platformer` and `top_down`, and one-genre items exempt from §5.3's other-genre recipe (G1, G3).
  - **Phase 8 additions** no sketch listed: the clock and schedules, the cone light, waves, a cover query, dialogue checks and a `say` cue action.
  - **Confirmed:** joints are 1.x (Q2), the two-thread rule stands for loading between areas (P8-7), rumble stays out (G5), aim assist stays the game's (P8-16).

## 8. Agent surface and evaluations

Tools before docs: a command cannot drift. 7.2 builds the foundations every later surface stands on; each system then ships its own surface in the step that builds it.

| System | Surface |
| --- | --- |
| Foundations (7.2) | `run --script` with a JSON report; `render` to PNG with overlays; `describe` |
| Levels | §5.1's CLI and journal |
| Assets | `asset add` per family, per-family generation, contact-sheet renders, generated IDs |
| Prefabs | Variants by `extends`; `describe` with resolved fields; a preview render; `check` with suggestions |
| Game code | Scaffolds (`new feature`, `new enemy`, `new status`), plugin defaults, a resolved-schedule dump |
| Chemistry | `explain <stimulus> on <entity>`: the matched rule and why, on the kit trace |
| Saves | `save dump`, `save diff`, a round trip per registered component, and a restore into a world with different entity allocations (§5.5) |
| AI and story | `explain` for a decision; dialogue and quest checks |
| Schemas | `describe` and a JSON Schema export |

**Evaluations** extend the release checklist's RC-A9 (a fresh session in a template copy adds a scripted feature). §8's five tasks are: add an enemy, a room, a sound, a status with an interaction; fix a broken prefab. Each run is a fresh agent, and counts turns, errors, files touched and wrong turns; each friction point is a seam, and the run is repeated after the fixes.

| Checkpoint | Workspace | Tasks | Passes when |
| --- | --- | --- | --- |
| 7.13 | 01, and a `tungsten new` copy | §8's five in each; the paper designs' weekly tasks (`paper-designs.md` §2 and §3) whose systems Phase 7 built, in the copy | Every friction point is recorded as a seam, and the rerun after the fixes completes them |
| 8.5 | A fresh `tungsten new` copy on Phase 8's systems | §8's five | All five complete with no engine patches |
| 8.6 | The refreshed 01, and a `tungsten new` copy | Tasks on 8.4's additions in 01; the paper designs' ten weekly tasks in the copy | All complete with no engine patches, and the owner accepts each genre's recipes and sketches |

## 9. Adoption: what it supersedes

One decision entry at the transition session, with its index row in the same change:

- `D-102`'s definition and `D-132`'s feature map as the tier source: 01 is the acceptance game, and 1.0 is §5.7.
- `D-194`'s clause keeping 01 distinct from the acceptance game ("no objective, no win and no boss, so it stays distinct").
- `D-103`'s stability policy, with `D-153`'s amendment of its clause 2, moved to 9.3 (Q8). `D-104`'s API snapshot stays, as a diff.
- `D-105`'s uncommitted tree and its squash clause (release pull requests stay squash-merged), and `D-079`'s commit clauses (`git add -A`, the pre-squash milestone commit) (H1); `D-079`'s merge-publishes clause (H2, R-Q15).
- `D-109`, `D-118` and `D-152` (H1's status row).
- `D-140`'s third route, the 1.0 milestone route (H1's plans row), with `D-150`'s and `D-151`'s level C clauses; `D-150`'s default rule and `D-151`'s tiers otherwise stand.
- `D-195`'s per-stop `tier` field, replaced by H1's tiers rule (`@bb1e4c0`: 14 stops carry it, 19 in the working tree; the draft left it out).
- `D-172`'s admission rule (§5.3, with G1's exemption and G3's tags). w13 §14's field rule (`w13-kit.md:412`, "Prefab and scene files keep their fields", citing `D-153` and `D-157`), as a clause only: `D-153`'s registry and `D-157`'s prefabs stand.
- The owner's answers of 2026-10-16 in the Phase 7 rewrite: the cutover at the freeze, the bench suite written only after the kit and every core system are finished, and the roadmap site kept until Phase 7 proper. Also the review's R-Q5 and R-Q7: no reset work in Phase 6. All are superseded by the answers of 2026-10-23.
- Records: the implementation plan's feature, freeze, RC and 1.0 gate rows (§2) and its §5, §6 and §10 (the register); `roadmap.json`'s gate `gfeat` and Phase 7 stops (`c1`, `w4b`, `gfreeze`, `c2`, `w79`, `grc`, `c3`, `g10`); `w04-api-freeze.md`'s break ledger; workflow §4's level C; the release checklist's RC-A1, RC-B2 and RC-B4, with RC-B1 revised to step 9.3's policy; `acceptance-game.md`, retired or rewritten as 01's acceptance; the Phase 7 rewrite, superseded and archived.

## 10. Open questions, each with a default

The transition session takes the default unless the owner answers first (`D-150`).

- **Q1. Rolling.** Bodies have no angle today, so a ball slides like a puck. Options: none; circles roll (angular velocity on circles only, friction torque at contacts, boxes stay upright); full rotation (oriented boxes and polygons, tumbling crates). *Default: circles roll in 7.8 once friction lands; full rotation 1.x.*
- **Q2. Joints.** *Default: 1.x unless a paper design's register needs them.* Neither register does, so joints are 1.x.
- **Q3. Freeform terrain art (strip styles).** *Default: 1.x; fixed-angle slopes in 1.0.*
- **Q4. Asset review medium.** *Default: image sheets the session writes and shows in the conversation; a review page with comments per asset later, if reviews grow.*
- **Q5. Sounds and music.** *Default: the same loop, with a playlist of files and notes for a sheet.*
- **Q6. The template as the second genre.** Its budget (`D-181`) caps the sample. *Default: recipes carry the top-down genre's breadth; the sample grows only within its budget.*
- **Q7. Where sketches live.** *Default: `examples/03_fixtures`, out of the release archives (`D-193`).*
- **Q8. The stability policy after 1.0.** Semver itself is settled by the owner's answer of 2026-10-16 (1.0.0, 1.0.1, 1.1.0, 2.0.0; the rewrite's R2). Open: how much of the surface it binds, now that 1.0 promises capability rather than a frozen API. *Default: a break makes a major, saves are always migrated, and items with one consumer sit behind an experimental feature outside the promise.*
- **Q9. Where story systems live.** *Default: the kit, unless 7.6's register argues for a crate of their own.*
- **Q10. The Phase 6 QA pass.** *Default: its correctness list only (W8b: the shader ID allocator, queued transitions, the remaining P3s), run as small tasks under H1, since a broad audit of kit code Phase 7 rewrites would be paid twice.*
- **Q11. The chemistry rules' name.** The kit's `react` domain already means cues (§5.2). *Default: the data kind is `interactions` and the domain `chemistry`; cues keep `react` for presentation and sound responses.*
- **Q12. 7.3's row set.** *Default: the old suite's core rows that still measure something (ECS, physics, render, UI), 01's full pit and the template frame; everything else waits for its system step or 9.1.*
- **Q13. H1's release path.** *Default: today's `release-cut` and preflight with the register, catalog and roadmap steps removed, and the release pull request merged without squashing, until H2's script and tag-push trigger replace them.*
- **Q14. When the paper designs run.** **Settled: they ran on 2026-10-23** ([paper designs](paper-designs.md)), and the transition session folded their registers into §3. Their own questions G1–G5 take their defaults (`D-150`): one-genre items exempt from §5.3's other-genre recipe, `stay_in_view` and `wrap` kept until 7.10 decides, `rpg` and `shooter` tags, saves across areas in 7.9, and no rumble in 1.0.
- **Q15. H1 before or after the next example stop.** **Answered 2026-10-25: H1 before `e01tomes`**, so it and every later example stop run under H1. `@bb1e4c0`: `e01quick2` now precedes `e01tomes`, so H1 comes before `e01quick2`. (The draft asked about `e01h`, which landed as M82 under the old flow.)
- **Q16. Tiled after our own format.** `D-157`'s Tiled object classes and the `.tmj` loader exist, and 01 leaves them at 7.12. *Default: the loader stays as an import path for games that author in Tiled, gaining no new features; 8.3 decides whether it needs more.*
- **Q17. Effects and statuses.** Today's effects (casting, regeneration, an orb's heal) are timed buffs with periods, damage, granted tags and stat modifiers. A status with intensity is a superset: an effect is a status at a fixed intensity on a timer. *Default: effects merge into statuses in 7.10, so one model serves buffs, ailments and chemistry, and agents have one thing to reach for.*

## 11. Review and critique

**Review pass, 2026-10-23.** A full review against this session's research, with a fresh-context fact-check of every repository claim at `6773e75`.

- **Changes from the review:**
  - H1 retargets the checks and scripts that read the records it freezes, because CI runs the repository check.
  - Agents stage only their own paths, and a parallel session works in its own worktree.
  - The release pull request stays the publish step until H2, merged without squashing.
  - Maintained docs are kept apart from history.
  - The read path moves to `ROADMAP.md`, and `D-195`'s tier becomes a rule.
  - Phase 9 regains W7b, W9b, W9c and C3.
  - 8.3 adds tooling for hand-made content (Aseprite, `asset add`, Tiled's place: Q16).
  - Sprite sheets join 7.9.
  - Room files hot reload into the live room.
  - The analysis pass has about eight readers, and the paper designs may start now (Q14).
  - The critique policy follows the value audit: one round at xhigh by default.
- **Corrections from the fact-check:**
  - §1's figures moved to HEAD, and they grew in a day.
  - `D-158` has six amendments.
  - `Frozen` is a timed component.
  - The backlog's "second game" rows and `D-172`'s recipe rule are told apart.
  - `D-104` is the snapshot, which stays.
  - K14 is the index rebuild.
  - The two meanings of "R4" are disambiguated.
  - R10's sitting is per minor or major release.
  - `LightUbo::byte_size` joins the deprecated list.
  - A fourth superseded 2026-10-16 answer (the roadmap site's timing) is listed in §2 and §9.

**Critique round 1, 2026-10-23.** `--repo` at xhigh, gpt-6-astra: 935,043 tokens in (844,416 cached), 39 commands, 360 s, 5h 50% → 72%. Five medium findings, each checked against its cited lines and accepted:

1. The chemistry cap bounds processing, not queue growth (`elements/cascade.rs:207`, `:213`). **Accepted:** bounded pending work with deduplication, a counted drop and a chain-depth limit, with tests (§5.2).
2. A per-component round trip doesn't prove a restore: 01 re-links hit targets, contact history and effect sources after every entity exists (`save.rs`'s `name_targets` and `name_sources`; `tests/game/saves.rs:1144`). **Accepted:** stable-ID references remapped in a second pass; the acceptance test is a restore into a world with different entity allocations (§5.5, §8).
3. Physics (7.9) and chemistry (7.10) needed terrain work scheduled at 7.12. **Accepted:** runtime terrain (tile properties, layers, one-way edges, outline collision, the auto-tiler, runtime tile changes) moves to the head of 7.9; 7.12 keeps the authoring format, CLI and editor (§3, §5.1). The later physics-first swap put terrain and physics together at 7.8.
4. H2 imported R3's release kinds, which refuse breaks, while Phases 7 and 8 keep breaking (`phase7-rewrite.md:204-205`). **Accepted:** H2's first kind is a breaking 0.x release; R3's kinds switch on and are rehearsed at 9.8 (§4).
5. Draw-call and upload counters can't run in CPU-only CI: the harness has no renderer (`crates/tungsten/src/testing.rs:2`, `:325`). **Accepted:** CPU counters in CI, GPU counters local beside `just smoke` and `just visual` (§5.8, 7.3).

No second round. The value audit found round 2s mostly repair round-1 fixes, and these five fixes are scoped to their sections.

**Transition recheck, 2026-10-25, at `bb1e4c0`.** Three fresh-context read-only checkers covered §1's figures, the records, decisions and readers behind §2, §4 and §9, and the seams M81–0.107 touched; each reproduced the draft's figure before quoting a new one. What moved is marked `@bb1e4c0`.

- **Order overtaken:** M81–M85 and 0.107 landed under the old flow, and `e01quick2` now precedes `e01tomes` (§3's Phase 6, Q15).
- **Fixed since:** M84's opt-in controller shove and step climb (`D-213`) close half of K7 and all of K18; §5.6 narrows to the controller pushed and riding, with the shove and the climb rebuilt on impulses and slopes. M83 fixed K24. Nothing in core, render or the umbrella changed.
- **Not fixed, and working against 7.1:** `D-214`'s floating check is 01's own. It derives prop positions from terrain art and regenerates everything (4,815 files), so 7.1's measure narrows to sprites outside the terrain, and 7.12's `check` generalizes it.
- **Grown:** every 7.10 item in 01 (M81's detonations, `Fuel` and impact damage; M84's sector push; M82's gesture clips; M81's hand-built swing boxes), and five of the paper designs' rows.
- **Corrected:** `missing_docs` is 1,165 distinct items, not 3,213, which counted dependencies again (§2); `LightUbo::byte_size` and `display.scale_mode` are unused rather than `#[deprecated]`; `D-158` has seven amendments; the ten body-centre helpers were already one by `6773e75`; the draft's seven predicates, five radial copies and 13.5k lines were not reproduced, and §1 gives the method used for each new figure; the K-rows live in the feedback plan's §6, not a register.
- **§9 and §4 widened:** `D-195`'s tier field and `D-150`'s and `D-151`'s level C clauses join §9; the field rule is scoped to `w13-kit.md:412`, since superseding `D-153` and `D-157` whole would remove the registry and prefabs; the squash clause is `D-105`'s. H1's checks row gains `prompts.md`, `skeleton.md`, the `main()` test, the `just checks` mapping, the docs naming the owned fields and `check_plans`' status rule.

**Critique round 2, 2026-10-25.** Artifact-only at xhigh with `--force` (72.7k characters, the owner confirming the size) and a focus note naming the never-critiqued edits of 2026-10-23 and 2026-10-25: gpt-6-astra, 31,690 tokens in, 535 s, 5h 36% → 42%. Five medium findings, each checked against the text and accepted:

1. Seed-plus-changes saves had no generator compatibility contract. **Accepted:** such a save names its generator's and room templates' versions; a generator change keeps the old version reproducible or migrates, with a test (§5.1).
2. Saves of many areas had no rule for references into unloaded areas. **Accepted:** such a reference stays a stable ID until its area loads; 7.9 designs it and 8.1 tests a restore followed by a move into that area (§5.5).
3. The evaluations disagreed on workspace and checkpoint (§7.2 still said 8.5; §8 said a template copy where 8.6 said 01). **Accepted:** one table in §8 gives each checkpoint its workspace, tasks and pass condition; §7.2 and 8.6 follow it.
4. §5.3's Phase 8 rule excluded navigation on platforms, which no design covers. **Accepted:** a Phase 8 item enters on a paper design's need or 01's refresh (8.4).
5. The capped performance passes had no stopping rule. **Accepted:** 7.7 names each pass's finite list with a target per item, and the rest waits for 9.2 (§5.8, 7.7).

No further round: these fixes are local to their sections, and none changes a mechanism another section relies on.

## 12. Risks

- 1.0 moves later: Phases 7 and 8 are larger than Phase 6.
- The redo becomes its own waterfall. Health first, plan sizes, stop conditions and the games' metrics answer it.
- H1 before `e01quick2` delays the remaining example stops. H1 is limited to what stops the overhead; the rest waits for H2.
- 7.1's interim tileset work is partly replaced by 7.12's format. It is kept small, and the authored IDs carry over unchanged.
- Chemistry grows into a scripting language. §5.2's guardrails answer it.
- Genre systems have no game behind them. Recipes, sketches, the owner's acceptance and the experimental feature (Q8) answer it.
- Churn breaks 01 or regresses performance repeatedly. 01's tests and 7.3's counter guards are the net, and each step updates 01 in the same change.
- Old and new records overlap until 9.7; the old ones stop growing at H1.
- Parallel sessions in one tree once agents commit: one session's commit could take another's files. H1's rule is that agents stage only their own paths, and a parallel session works in its own worktree.
- `@be16369` facts drift as M81 and M82 land; ordered step 3 rechecks them. Rechecked at `bb1e4c0`: the seams grew by about 5.6k lines of 01's Rust in five stops, all in 01, so each stop before Phase 7 enlarges 7.10 and 7.13.
- Example stops planned ahead of H1 keep the old shape: `e01quick2`–`e01flask` were written to the register and catalog on 2026-10-25. H1 carries them into `ROADMAP.md` as items, re-cut after play, rather than re-planning them.

## Session log

- **2026-10-22 and 2026-10-23:** the owner's review session: the roadmap, the Phase 7 rewrite, example 01's design and feedback and the backlog read; four code sweeps (levels, physics, kit and tagging, assets) and a web survey; the owner's answers (§2). Drafted on 2026-10-23 at `6773e75`, with M81 in progress in the same tree.
- **2026-10-23, earlier work:** at the owner's prompt, checked whether Phase 7 had already started. No Phase 7 stop had: `roadmap.json`'s stops from `c1` on are untouched, and the first open stop is `e01g`. Folded in: the 0.66 project-health pass (`D-150`–`D-152`), `D-140`'s routes, the Phase 7 rewrite's answers and defaults, W4b's cleanup rows, the AI-workflow proposals in known issues, and C1's recorded game row. The owner's prompts of 2026-10-16 in the Phase 7 rewrite's and the kit catalogue's sessions were read back from the transcripts. They added the catalogue's genericity goal (§2) and semver as already settled (Q8).
- **2026-10-23, reorder:** at the owner's request, workflow and high-value work moved sooner and the benchmark rework split in two. H1 runs before `e01h` and H2 after the quick wins. Phase 7 opens with 7.1 quick wins, 7.2 agent tooling and 7.3 the benchmark runner, ahead of the analysis pass, and the paper designs may start after H1. The benchmark expansion and C1 sit at 9.1 (§5.8).
- **2026-10-23, review and critique:** a full review pass, a fresh-context fact-check and one `--repo` critique round. Every finding was accepted and is recorded in §11.
- **2026-10-23, after the critique** (these changes were not critiqued):
  - Terrain and physics moved before saves (7.8, 7.9), so saves are built against settled physics state and the kit pass follows physics directly.
  - 01's refresh on Phase 8's systems added as 8.4, with proof moved to 8.5.
  - Q17 added: effects merge into statuses.
  - §7.2 gained its method (scope, coverage first, production lists, agent tasks, stress numbers) and the default pitches, Lantern Ward and Static Front.
  - `grid` and `shmup` leave the kit's genre claims.
  - CPU and GPU passes added at 7.14 and 7.15, with guidance moving to 7.16.
- **2026-10-25, the transition session** (Part A, planning edits), at `bb1e4c0` with the owner's uncommitted intake of that day (`e01quick2`, `e01foes`–`e01flask`):
  - Every `@be16369` and `@6773e75` fact, §1's figures and §9's list rechecked by three fresh-context checkers (§11); what moved is marked `@bb1e4c0`.
  - Phase 6's closing replaced with the remaining stops; Q15 answered as H1 before `e01tomes`, and adapted to `e01quick2`, which the intake put first.
  - The paper designs' registers folded into §3 by ID, 7.6 marked done, and G1–G5's defaults taken (§7.2, Q14).
  - 8.5, the template final pass, added; proof moved to 8.6; `tungsten new --genre` noted for the 1.x backlog.
  - The owner's agile gradient of 2026-10-23 added to §2 and as H1's habits.
  - One artifact-only critique round at xhigh on the updated draft; five findings, all accepted (§11).
