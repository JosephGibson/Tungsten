# Roadmap redo — draft

A working draft that replaces the 1.0 roadmap after Phase 6. It brings the workflow cutover forward into Phase 6's tail and the cheap, high-leverage fixes and agent tooling to the head of Phase 7. It reworks the benchmark suite in two parts, and replaces today's Phase 7 with three phases (Seams, Genres, Release), defining 1.0 as a fully featured engine, tooling and kit rather than a frozen API. Written from the owner's review session of 2026-10-22 and 2026-10-23. Nothing here changes the register, `roadmap.json`, decisions or the running example stops; the transition session after M81 adopts it (ordered step 3). Facts marked `@be16369` were measured at that commit with M78's uncommitted tree, before M79–M81 landed; the transition session rechecks them.

- **status:** draft
- **goal:** Re-aim 1.0 at the bottleneck example 01 measured, the cost of making a game, not engine speed or API stability, and do the work that pays back soonest first:
  1. Health in two parts: the cutover's essentials (H1) before 01's last stop, the rest (H2) once the quick wins and tooling give it real tasks.
  2. Phase 7, Seams: quick wins, agent tooling and the benchmark runner first; then the analysis pass and the paper designs; then the engine, kit, template and level work they scope, with physics and slopes before saves, a chemistry-style interaction model, and saves the engine does for the game; then a CPU and a GPU performance pass.
  3. Phase 8, Genres: the systems a story-driven RPG and a shooter with complex enemy AI need, then 01 refreshed on them.
  4. Phase 9, Release: the benchmark expansion pass with budgets, performance, quality, docs and tests, release candidates, 1.0.
- **non-goals:**
  - Edits outside this file before adoption: the register, `roadmap.json`, decisions, workstream files, the Phase 7 rewrite, skills.
  - Changes to `e01g` (M81, in progress).
  - Dates and time estimates.
  - Each step's mechanisms: its own plan specifies them from the tree of its day.
  - Implementation.
- **files to touch:** This file while it is a draft. At adoption (ordered step 3): `docs/plans/1.0/implementation-plan.md` (§2, §5, §6, §10), `docs/plans/1.0/roadmap.json`, `docs/plans/1.0/README.md`, `docs/plans/1.0/acceptance-game.md`, `docs/plans/1.0/w04-api-freeze.md`, `docs/plans/1.0/workflow.md`, `docs/plans/1.0/release-checklist.md`, `docs/plans/1.0/w13-kit.md` (§14), `docs/plans/phase7-rewrite.md` (superseded and archived), `DECISIONS.md` and `docs/DECISION_INDEX.md` (one entry, §9). H1 then creates `ROADMAP.md`.
- **ordered steps:**
  1. The owner reads this draft while M81 runs and answers or accepts the open questions (§10).
  2. M81 (`e01g`) releases.
  3. The transition session: recheck every `@be16369` fact and §9's list against the tree; write the decision entry; adopt or change §3's order; archive the Phase 7 rewrite. H1 follows in the same session if it fits, otherwise in the next.
  4. `e01h` (M82) runs under H1, as the first trial of §6's asset sessions.
  5. Phases 7, 8 and 9 in §3's order.
- **done-when:**
  - Every open question in §10 has an owner answer or a stated default.
  - The transition session has rechecked the facts, written the decision entry and adopted, changed or rejected §3's order.
  - H1 is in place and `e01h` has run under it.

## Context digest

- On 2026-10-23, `e01g` props and foes (M81) is in progress on `0.102`, and `e01h` the look is the last example stop. Then come the Phase 6 QA pass and the feature gate ([implementation plan](1.0/implementation-plan.md) §4, §2).
- Today's Phase 7 is C1, W4b and the freeze gate, C2, W7b/W9b/W9c, C3 and the 1.0 gate (§5 there). The [Phase 7 rewrite](phase7-rewrite.md) restructures it into performance work before the freeze and a process reset after it. Neither has a game-development pass.
- The feature gate cannot pass as written: it needs the acceptance game to play from a package in its own repository, which was never started. The owner now treats example 01 as the acceptance game.
- 01 is a sandbox on the template (`D-194`); the template's sample is a small top-down game. They are the kit's two consumers.
- Two earlier overhauls already changed the process: the 0.66 project-health pass (2026-10-13) and the Phase 7 rewrite draft (2026-10-16). No Phase 7 stop has started. §2 says what this redo keeps from each.
- §9 lists what adoption supersedes; nothing changes before it.

## 1. Why: what example 01 measured

From this session's four code sweeps (levels, physics, kit and tagging, assets) at `be16369`, and a fresh-context recheck at `6773e75` (HEAD, before M81); figures are HEAD's where the two differ. The seams grew between the two: M79 and M80 added about 2k lines of 01's own Rust in a day.

- **Process.** M32–M78 ran in under three weeks, each with a plan, a decision entry, a changelog section, a register row and a catalog stop. M78's ten quick fixes touched six record files under `docs/plans/1.0/`, and its release added the changelog, a decision entry, its index row and known issues. `D-158` carries six amendments. Level C, with its API critique, exists "because the freeze makes API taste expensive to fix later" ([workflow](1.0/workflow.md) §4). A milestone whose code touches benchmark rows shows them not regressed, so 29 of M32–M78 ran capture sittings; the rewrite's R10 already proposes counter guards in their place.
- **Kit.** About 13.5k lines of 01's gameplay are its own Rust, against about 2.5k of data. Element reactions were restated at six delivery sites (`@be16369`). 92 `has::<…>` checks and seven predicates decide interactions while `TagFilter` goes unused, and statuses exist twice (a timed `Frozen` component and `status.frozen`). 22 backlog rows cite 01, 12 of them waiting for "a second game" or genre to ask, and the kit admits an item on recipes in two genres (`D-172`). `save.rs` is 1,424 lines; 01 orders 84 systems with 50 edges.
- **Levels.** Map objects take save IDs by order, so two more bats bumped the save schema (M78). The tileset is every sprite, sorted: 12 crawler frames renumbered 10,056 cells, about 50k lines of diff. Any edit regenerates all 4,172 generated files. `level.json` is 10,220 lines placing 1,194 props by number; 74 launch rows in `tests/game/level.rs` hold positions, with copies in the playthrough test; collision comes from three sources.
- **Physics.** No friction, rotation, joints, one-way platforms or slopes. Contact events carry no speed (01 runs four impact systems), sensors report entry only, there is no radial impulse (five copies in 01), the controller neither pushes nor is pushed (K7), and 01 has ten body-centre helpers (`@be16369`).
- **Assets.** About 10k lines of generator code own every asset file and refuse any other. One PNG per frame. Tools and generated assets were 2,588 of M76's 2,650 changed files. In-game captures for art review need a scratch crate per session. Large art sessions lose quality: M76's level art took six drafts.
- **Startup.** `resumed` loads every asset synchronously before the first frame (`crates/tungsten/src/app.rs`, `load_all_merged`); 01 loads about 3,565 PNGs and 81 sounds with nothing on screen. Not measured.

## 2. Owner answers and earlier work

### Answers, 2026-10-22 and 2026-10-23

Settled. A finding against one is recorded in §10 as a question, not applied.

| Topic | Answer |
| --- | --- |
| Roadmap | Redo it; phases after 6 may be expanded or replaced |
| Workflow | Project health first, before the next large body of work; workflow and high-value work pushed sooner |
| Benchmarks | The suite reworked in two parts, with an expansion pass |
| Phase 6 | Two more passes on 01 (`e01g`, `e01h`), then close |
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

### Earlier work this builds on

- **The 0.66 project-health pass** (2026-10-13). No owner-approval stops; gates and plans take stated defaults (`D-150`). Opus max only where a session's judgment is final (`D-151`), with a stop's own tier first (`D-195`). Records written at a release's cut hold through its merge (`D-152`). H1 keeps `D-150`'s default rule and `D-151`'s tiers, and replaces `D-152`'s owned README fields with `ROADMAP.md`.
- **`D-140`'s planning routes.** Bounded maintenance planned in the session, and a saved handoff plan for multi-session work, both stay ([plans README](README.md#choosing-a-route)). H1 retires only the third route: 1.0 milestone candidates with the milestone skill, the plan critique and level C. §4's plan sizes are the two routes plus a large plan file.
- **The Phase 7 rewrite** (2026-10-16, draft). Its owner answers stand except three: the cutover's timing, a bench suite written only once the kit and every core system are finished, and the roadmap site kept until Phase 7 proper. The review's own answers R-Q5 and R-Q7 (no reset work in Phase 6) yield as well. Its four critique rounds applied 19 findings.
  - **Where its parts go:** its runner work moves to 7.3 and its workloads and budgets to 9.1 (§5.8); its R1, R3, R4 (skills) and R5 move to H1 and H2; R6–R8 stay in Phase 9.
  - **Defaults carried:** R-Q4, `ROADMAP.md` at the root; R-Q9, every operating rule names the failure it prevents; R-Q11, the old records deleted at the end with the tree tagged and `just history`; R-Q13, frame and operation budgets gate; R-Q15, a final tag publishes on push.
  - **Changed:** R-Q14's one candidate becomes the second half only (9.1); O2's formats in the freeze become a version field and a pinned fixture per format that every later release still loads (§5.1, §5.5, 9.4).
  - **Moot:** O5 (the arity-named queries leave in 7.1), O7 and R-Q10 (no freeze), R-Q12 (01 is in the repository).
- **The kit catalogue** (2026-10-16; W13e–W13k landed as M60–M67). The owner's goal that night was to move 01's game-specific components into a generic kit that works across games from the start. The kit now exists; the owner's answer of 2026-10-23 is that parts of it came out generic along the wrong axis. §5.2 keeps the catalogue's structure and replaces its ailment model.
- **W4b's cleanup rows** (`w04-api-freeze.md`; known issues). The deprecated surface leaves in 7.1, since breaking is fine: the eleven arity-named queries, `DeltaTime`, `InspectorState`'s three row functions, `LightUbo::byte_size`, `display.scale_mode`, and the 32 `#[allow(deprecated)]` sites that keep them alive. `missing_docs` (3,213 public items without rustdoc at `12ccd36`) gets a ratchet in H1's release checks so the counts only fall; rustdoc lands per system once it settles, and the rest in 9.4.
- **Workflow proposals carried by the 0.66 pass** (known issues): the archived AI-workflow research plan's proposals 4, 5, 6, 7 and 9, which are still unapproved. They are deny-list bans and a decision-row check; a released-tag consumer check and an early RC-A9 pilot; refreshed client discovery and exact-command permissions; behaviour and visual evidence for feature reviews; `.editorconfig` and `.gitattributes`. H2 decides each. The consumer check is 01 built outside the tree on a tag, and the RC-A9 pilot is §8's first evaluation run.
- **C1's groundwork.** M76 recorded 01's full pit with enemies awake as C1's game row (`w10-closing-phases.md`'s follow-ups, `tests/game/frame_time.rs`), and later stops changed what it measures, so 7.3 ports it and 9.1 retakes it. Known issues propose that new benchmark rows land with the stops that add their features, which §5.8 adopts.

## 3. The roadmap

Ordered by what pays back soonest. The first items in Phase 7 cut the cost of every change after them, so they come before the analysis pass that scopes the rest.

### Phase 6, closing

1. `e01g` props and foes (M81, in progress).
2. The transition session and H1 (§4), after M81's release.
3. `e01h` the look (M82), under H1: the first trial of §6's asset sessions on today's generator. An art session generates and writes the review sheet, the owner reviews in rounds, then a code session leaves `tools/` art alone.
4. The Phase 6 QA pass's correctness list as small tasks under H1 (Q10).

### Phase 7: Seams

| # | Step | Done when (sketch) |
| --- | --- | --- |
| 7.1 | Quick wins | W4b's deprecated surface removed. Map objects keep authored IDs, after one last schema bump and migration, so map edits stop bumping saves (7.9 later changes the format once, with its own migration). The tileset holds only the tiles the map uses, in a stable order, written compactly; this is interim until 7.12's format, which keeps the IDs. Asset families generate on their own; accepted assets are locked; anchors and hitboxes are data beside their sprites. One integration-test binary per crate (R7's compile-time item). Small known-issue fixes. Measured: an enemy added to the map needs no schema bump, and one sprite change touches no level file |
| 7.2 | Agent tooling foundations | `tungsten run --script` (inputs at frames, assertions, screenshots, a JSON report); `tungsten render` (a scene, an area or a prefab to PNG with overlays, on the capture path); `tungsten describe`. Asset review sheets and in-game captures come from `render` instead of a scratch crate |
| 7.3 | Benchmarks, part 1: runner and guards (§5.8) | The rewritten runner; CPU counter guards in `just check` and CI, GPU counters local; a minimal row set ported; the old suite and data purged with the carry-overs kept; `tungsten-perf` rewritten with it; per-milestone performance sittings retired |
| 7.4 | H2 (§4) | The release script and its rehearsal, `just status`, the skills rewritten against baselines, the carried workflow proposals decided, agent memory pruned; the overhead remeasured, and lower |
| 7.5 | Analysis pass over 01 and the template (§7.1) | The seam register written and verified; the owner's triage done |
| 7.6 | Paper designs (§7.2), startable any time, even before H1 (Q14) | Two designs with verified build sheets; the genre registers written |
| 7.7 | Phase 7 plan from the registers | Every later step's rows assigned |
| 7.8 | Terrain and physics | First the runtime terrain that physics and chemistry stand on (§5.1): per-tile properties, layers and one-way edges, outline collision from the tile grid, the Rust auto-tiler, and runtime tile changes re-tiled and re-outlined locally. Then §5.6 physics on it, with slopes and the controller last |
| 7.9 | Saves and engine seams | §5.5 saves on 7.1's IDs and 7.8's settled physics state: `save.rs`'s generic part gone from 01, one-call capture and restore in the template. §5.4 boot loading, with sprite sheets (frames as rectangles in one image) in place of one PNG per frame. The register's small seams (K2 particle depth, K4 animation rate, K5 text outline, K9 lit-sprite flash, sound hot reload, looping cue voices) |
| 7.10 | Kit pass | §5.2 chemistry, with today's effects merged into statuses (Q17); 01's generic code absorbed (blasts, cones, conduction, impact damage, the saved chain queue, draining, timers with expiry and saves, named world cues, effect looks, one-shot animator clips, hitbox facing); motion on 7.8's physics; plugin defaults over hand ordering; the kit descriptions' genre tags matched to §5.7's four genres, with `grid` and `shmup` dropped (§7.2) |
| 7.11 | Template updates | Boot splash and loading, one-call saves, the register's template rows |
| 7.12 | Level format, agent CLI, editor (§5.1) | The authoring format on 7.8's runtime terrain, and the CLI, first; the in-game editor on the same commands last |
| 7.13 | 01 on the new systems | 01's reactions, statuses, saves and level on Phase 7's systems, with `e01g`'s props and kegs as chemistry data; a fresh-agent evaluation run (§8); the owner accepts 01 |
| 7.14 | Performance pass: CPU (§5.8) | With 01 on the new systems as the workload, the measured top CPU costs that Phase 8 will build on: the ECS's `has` and query paths (K12), per-entity kit systems (K13), the spatial index after 7.8, chemistry spread, the save snapshot, the extract. Each is fixed only for a measured cost on a row, with no budgets yet; the whole-frame row and the GPU rows stay not regressed |
| 7.15 | Performance pass: GPU (§5.8) | The same for the GPU, on 7.3's non-stalling timestamps: the lit-sprite path, the light cull past 16 lights, bloom and the post stack, particle quads, sprite batching and upload bytes, tile layers. The CPU frame, the extract included, stays not regressed |
| 7.16 | Agent guidance: research and decision | How engines and repositories guide agents (task guides, `AGENTS.md`, shipped skills) researched against 7.13's evaluation; what the template ships, decided |

Every system step from 7.8 on adds its own benchmark row and counter guard as it lands (§5.8).

### Phase 8: Genres

| # | Step | Done when (sketch) |
| --- | --- | --- |
| 8.1 | Story systems | A condition vocabulary for data; dialogue, quests and flags; cutscenes and timelines; inventory with drag and drop; text input; loading between areas. Recipes for each, sketches where feel matters, a benchmark row each |
| 8.2 | Shooter and AI systems | Navigation, top-down and on platforms (jump links and ledges, which 8.4's goblins need); AI structure (hierarchical states or utility scoring, perception with memory, squads); hit-stop that leaves game-time windows intact; weapons as data; bullet density. Recipes, sketches, a benchmark row each |
| 8.3 | Tooling for hand-made content | Aseprite import (tags to animations, slices to anchors and hitboxes); `asset add` for a drawn file or sheet; the Tiled loader's place beside our own format (Q16). A drawn sheet and an Aseprite file each enter a `tungsten new` game with one command |
| 8.4 | 01 refreshed on Phase 8's systems | Not a rewrite: 7.13 already moved 01 onto Phase 7's systems. Its enemies move onto the AI structure, perception and navigation, and it gains hit-stop. A few additions demo Phase 8 in a platformer sandbox: an NPC with a few lines, one choice and a small fetch quest across two areas; a short intro cutscene; a satchel inventory with drag and drop; goblins that path around terrain, take cover and flank as a pack; one or two hand-made sprites through Aseprite import; areas streamed in. Each art need goes through §6. Done when 01's own gameplay Rust shrinks again, each addition is mostly data with any Rust counted, 01's tests pass, and the owner plays it through |
| 8.5 | Proof | A fresh-agent evaluation run on the finished systems inside the refreshed 01; the owner accepts each genre's recipes and sketches |

This list is the paper designs' prediction; 7.6's registers replace it.

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

Pulls the rewrite's R1, R3, R4 and R5 forward, builds on the 0.66 pass (§2), and drops the freeze's process. It comes in two parts. H1 is what stops the overhead: it runs before `e01h`, so the last example stop already costs less. H2 is what needs real tasks to get right: it runs after 7.1–7.3 have given it some. The old records stay, no longer growing, until 9.7 (the owner's answer of 2026-10-16). The operating rules live in one doc, each naming the failure it prevents (R-Q9).

**H1: the cutover's essentials**

| Area | Change |
| --- | --- |
| Git | Agents commit at each logical step in Conventional Commits, checked by a `commit-msg` hook, and stage only the paths they changed (never `git add -A`). A second session in parallel works in its own worktree or clone. The owner pushes. Until H2 moves publishing to a tag push (R-Q15), the release branch and pull request stay the publish step, merged without squashing so the commits survive; then the per-version branches retire (supersedes `D-105`'s uncommitted tree, and `D-079`'s commit clauses and its squash) |
| Records | History stops growing per change: no register rows, catalog stops, roadmap-site syncs or decision amendments per milestone, and a release's changelog section comes from its commits. Maintained docs (`DESIGN.md`, the LLM index, `README.md`, `docs/assets.md`, `docs/validation.md`, known issues, the template's docs) are still corrected in the same commit as any change that makes them wrong. Each frozen record gets a one-line banner naming what replaced it |
| Checks and scripts | Everything that reads a record H1 freezes is retargeted or removed in the same change, since CI runs the repository check: its `check_roadmap` and `check_summary` (the README block's version check fails at the next cut once the block is frozen) and their tests; `roadmap.py` and its test; `release-cut`'s summary write in the `justfile`; `tungsten-next`'s `state.sh`, `SKILL.md`, `roadmap.md` and `page.html`; `tungsten-milestone`'s register and README steps. `decision_index.rs` stays, since decisions continue |
| Read path | `AGENTS.md` and the LLM index point at `ROADMAP.md` and this file in place of the register, the catalog and the implementation plan |
| Status | `ROADMAP.md` at the root (Now, Next, Later) replaces the register, `roadmap.json`, the roadmap site and the 1.0 README's owned fields (supersedes `D-109`, `D-118`, `D-152`); `tungsten-next` reads it |
| Plans | `D-140`'s routes, with the 1.0 milestone route retired. Small: none, the commit is the record. Medium: in the session. Large: one plan file, deleted when done. Done means the tests pass and a demo works. A large plan or a risky change gets one critique round at xhigh, and a second only when the first changed a mechanism: round 2s have mostly repaired round-1 fixes, and max has shown no lift over xhigh. `tungsten-milestone` trimmed to that |
| Decisions | Only for choices that constrain later work (dependencies, formats, policy), short |
| Freeze process | Removed: the break ledger, level C's API critique (critique stays for risky areas: concurrency, formats, the render seam), the kit's rule that prefab and scene files keep their fields (w13 §14, `D-153`, `D-157`), `#[non_exhaustive]` sweeps. The API snapshot stays as a free diff |
| Releases | Cut when something is worth shipping, not per milestone, on a working path (Q13) |
| Tiers | `D-151` stays. `D-195`'s per-stop tier leaves `roadmap.json` (which `roadmap.py` reads it from) for a rule: asset sessions run on Opus max, code sessions at xhigh |
| Assets | §6's procedure, on today's generator |
| Docs | The `missing_docs` ratchet in the release checks |
| Measured | Before H1, a baseline from M76–M81: record files touched per change, plan lines per change, the share of a session spent on bookkeeping. Retaken after `e01h` and the first quick wins |

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
- **Checks.** Structured errors by room, ID and field path; spawns clear of solids, props supported, routes reachable with the controller's own numbers, light and entity budgets.
- **Generation.** A generator is a client of the commands or builds a room in memory. Authoring-time output stays editable: generated objects carry their generator, and a hand edit detaches one. Runtime output spawns through the loader with IDs from the seed and saves as the seed plus changes. `check` runs over many seeds headless.
- **Slopes.** Fixed angles (45° and 2:1) in the tile art and the auto-tiler; physics in §5.6.

### 5.2 Chemistry (7.10)

- **Properties:** hierarchical tags on prefabs and tiles (`material.wood`, `flammable`, `conductive`); numeric properties as stats.
- **Stimuli:** a damage type is one; contact, heat, wetness, a blast and an impact are others; statuses emit them.
- **Statuses:** data with an intensity from 0 to a max, decay and stacking. Bands at thresholds grant tags and behaviour: chill slows in proportion and freezes at its top band. Effects scale with intensity through `curves`. A status can change stats, tags, physics and behaviour, emit stimuli on a period, carry a look (tint, particles, light, a sound loop) and run outputs when it ends. No `Health` is needed.
- **Interactions:** a stimulus matched against the receiver's properties and statuses, the most specific rule winning. Outputs: build, apply or remove a status; spawn; blast; emit a stimulus; damage; transform (`become` a prefab or `set_tile`, optionally reverting); consume intensity.
- **Spread:** a status emits a stimulus in a radius on its period, to entities and tiles.
- **Tiles:** statuses optional per layer or style, stored sparsely by cell.
- **Fuel:** a drained stat whose zero runs the status's end outputs; it covers the backlog's draining effect.
- **Guardrails:** the kit names no element. Rules match tags and thresholds, outputs come from a fixed vocabulary, there are no conditions or variables, and anything beyond calls a game's named Rust handler. One fixed phase after damage resolves, on the fixed step, in a stable order, with a per-step cap through a saved queue; the kit's determinism hash moves once, under 7.3's guards. Physics layers stay collision-only.
- **Bounded work:** the cap bounds processing, not growth, so pending work is bounded separately, as 01's cascade already does it: one pending entry per kind and target (`elements/cascade.rs:208-212`), a queue bound with a counted drop (`:213-214`), and a chain-depth limit so cyclic rules end. Tests show a two-rule cycle ending and a spread flood at 12,000 bodies staying under the bound.
- **Replaces:** the ailment ladders on `damage_types`, the buckets and `ailment_threshold`, and today's effects, merged into statuses (Q17); in 01, its element state on bodies, the cascade, the restated reaction sites, `BallBurn`, `Frozen`, the hazard douse and the lava-frost prefabs.
- **Needs:** a cheap "properties within r" query, which K14's per-step index rebuild makes costly today; runtime tile changes (7.8's runtime terrain); identity kept through `become` (§5.5); looping cue voices.
- **Naming:** the kit's `react` domain (cues, counters, levels, loot) and its `kit_react` phase already exist, so the rules here need a name that doesn't collide (Q11).

### 5.3 Admission rule (7.10, Phase 8)

Phase 7 items: 01 or the template uses the item, and a recipe exists in the other genre. Phase 8 items: a paper design needs it, recipes cover it and a sketch exists where feel matters, and the owner accepts. This amends `D-172`'s two-genre recipes with a real user, and re-triggers the backlog rows that wait for "a second game": 01's use is enough.

### 5.4 Boot loading (7.9)

A `boot` asset group (a font, a logo, a bar) shown first, then the rest loaded in time-budgeted slices on the main thread with progress; the template's shell shows a splash. A startup capture comes first, and 7.3's operation rows hold it after. A decode worker thread only if the capture shows decoding dominates, as its own decision under the two-thread rule. Loading between areas waits for 8.1.

### 5.5 Saves (7.1 IDs, 7.9)

A component registers as saved in one line, and kit components register themselves. Map objects keep §5.1's IDs and runtime spawns take IDs from their spawner. A save is plain JSON: the level reference, changed components, despawns, runtime spawns in full and registered resources. Each component carries its own version and migration, so adding enemies to a map never touches a save. The game writes only state that is no component, and the template's capture and restore become one call each. The autosave's main-thread cost has a benchmark row and a budget.

Entity references inside saved components (owners, sources, hit targets, contact history, references held in resources) are saved as stable IDs and remapped in a second pass once every entity exists, as 01 does by hand today (`save.rs`'s `name_targets` and `name_sources`). The acceptance test is a restore into a world whose entity allocations differ from the saved one, with 01's existing save tests passing unchanged. One of them checks that an attack saved mid-hit never hits twice after a load (`tests/game/saves.rs:1144`).

### 5.6 Physics (7.8)

Friction first; contact events with closing speed, begin and end; sensor exit; impulses and radial impulses; one-way edges; outline collision (§5.1); the controller pushing, pushed and riding; slopes with floor normals, climb and slide limits and snap-down; `Shape` made `#[non_exhaustive]`; a cheaper query index (K14). The physics hashes move with these, under 7.3's guards. Rolling and joints: Q1, Q2.

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
  - **Capped:** measured structural costs that Phase 8 builds on, each fixed only for a measured cost on a row, with no budgets yet (9.1 sets them).
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

The brief's contract (sizes, anchors, frame counts, clips, hitboxes) is what code depends on, so art changes within it touch no code. The procedure starts at H1 and `e01h` is its first trial. Its mechanics land in 7.1 (generation per family, locks, anchors and hitboxes as data) and 7.2 (review sheets from `render`). Sounds and music follow the same loop (Q5).

## 7. Briefs

### 7.1 Analysis pass

- **Goal:** every place 01 compensates for the engine, kit or template, with evidence, ranked by the code a generic fix would delete and how often the pattern recurs.
- **Scope:** 01's `src/`, `tests/` and `tools/`, and the template. The engine and kit are read only to verify claims. Bugs are noted, not chased. Seams 7.1–7.4 already fixed are marked as done, not re-reported.
- **Readers:** about eight, with small folders grouped (enemies; elements; toys; level and tools; spells, gun and lightning; combat, player, audio and run; `save.rs` and the root files; the template), each reading its share whole and filling one row shape: pattern, `file:line`, lines, what the engine, kit or template lacks, the generic fix, dependencies.
- **Lenses:** helpers copying engine logic; Rust where data belongs; markers where tags belong; hand-ticked timers; per-kind arms; constants restating data; save and ordering boilerplate; performance workarounds; positions copied into tests; duplication in the tools.
- **Seeds:** K1–K15 and the K-rows after them, the backlog rows citing 01, known issues, §1's sweeps.
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
  - Five agent tasks a developer would hand an agent every week; these become 7.13's and 8.5's evaluation tasks.
  - Stress numbers (bullets on screen, agents awake, NPCs loaded), which become 9.1's rows.
  - A use of the engine's lighting, post stack and chemistry.
- **Build sheets:** per system, how it would be built today, naming engine and kit items, data kinds and prefabs, with one verdict: data only; game Rust (size and why); gap (what is missing); blocked (a rule or the architecture).
- **Walkthrough:** one minute of play per design, tracing the systems that run, the events, their order and what is saved.
- **Evidence:** every "data only" or "game Rust" verdict cites an item in `api/*.txt` or the components snapshot; every gap names what was searched for. A fresh-context verifier checks the citations; `/critique` on the build sheets is optional.
- **Output:** rows in the analysis pass's shape, tagged by design. Rows on systems Phase 7 rebuilds fold into Phase 7; the rest scope Phase 8. Neither design covers navigation on platforms (jump links and ledges), so it joins 8.2 for 8.4's refresh of 01.
- **Genres:** the kit's descriptions also claim `grid` and `shmup`. Neither design tests grid placement or building, and Static Front covers what a shmup would, so 7.10 drops both tags and the kit claims §5.7's four genres.

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

**Evaluations** at 7.13 and 8.5, extending the release checklist's RC-A9 (a fresh session in a template copy adds a scripted feature): three to five real tasks per system (add an enemy, a room, a sound, a status with an interaction; fix a broken prefab), run by a fresh agent on a `tungsten new` copy. Count turns, errors, files touched and wrong turns; each friction point is a seam; rerun after the fixes.

## 9. Adoption: what it supersedes

One decision entry at the transition session, with its index row in the same change:

- `D-102`'s definition and `D-132`'s feature map as the tier source: 01 is the acceptance game, and 1.0 is §5.7.
- `D-194`'s clause keeping 01 distinct from the acceptance game.
- `D-103`'s stability policy, moved to 9.3 (Q8). `D-104`'s API snapshot stays, as a diff.
- `D-105`'s uncommitted tree and `D-079`'s commit clauses and squash (H1); `D-079`'s merge-publishes clause (H2, R-Q15).
- `D-109`, `D-118` and `D-152` (H1's status row).
- `D-140`'s third route, the 1.0 milestone route (H1's plans row); `D-150`'s default rule and `D-151`'s tiers stand.
- `D-172`'s admission rule (§5.3); w13 §14's field rule, with `D-153` and `D-157`.
- The owner's answers of 2026-10-16 in the Phase 7 rewrite: the cutover at the freeze, the bench suite written only after the kit and every core system are finished, and the roadmap site kept until Phase 7 proper. Also the review's R-Q5 and R-Q7: no reset work in Phase 6. All are superseded by the answers of 2026-10-23.
- Records: the implementation plan's feature, freeze and RC gate rows (§2) and its §5, §6 and §10; `roadmap.json`'s Phase 7 stops; `w04-api-freeze.md`'s ledger; workflow §4's level C; the release checklist's RC-A1 and API rows; `acceptance-game.md`, retired or rewritten as 01's acceptance; the Phase 7 rewrite, superseded and archived.

## 10. Open questions, each with a default

The transition session takes the default unless the owner answers first (`D-150`).

- **Q1. Rolling.** Bodies have no angle today, so a ball slides like a puck. Options: none; circles roll (angular velocity on circles only, friction torque at contacts, boxes stay upright); full rotation (oriented boxes and polygons, tumbling crates). *Default: circles roll in 7.8 once friction lands; full rotation 1.x.*
- **Q2. Joints.** *Default: 1.x unless a paper design's register needs them.*
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
- **Q14. When the paper designs run.** They read the API snapshots and write only their own doc, so they need neither H1 nor a stable 01. *Default: any time from now, in a session of their own; their registers then inform the transition session as well as 7.7.*
- **Q15. H1 before or after `e01h`.** *Default: before, so `e01h` is the asset-session trial and the last example stop already runs on the lighter flow.*
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

## 12. Risks

- 1.0 moves later: Phases 7 and 8 are larger than Phase 6.
- The redo becomes its own waterfall. Health first, plan sizes, stop conditions and the games' metrics answer it.
- H1 before `e01h` delays the last example stop. H1 is limited to what stops the overhead; the rest waits for H2.
- 7.1's interim tileset work is partly replaced by 7.12's format. It is kept small, and the authored IDs carry over unchanged.
- Chemistry grows into a scripting language. §5.2's guardrails answer it.
- Genre systems have no game behind them. Recipes, sketches, the owner's acceptance and the experimental feature (Q8) answer it.
- Churn breaks 01 or regresses performance repeatedly. 01's tests and 7.3's counter guards are the net, and each step updates 01 in the same change.
- Old and new records overlap until 9.7; the old ones stop growing at H1.
- Parallel sessions in one tree once agents commit: one session's commit could take another's files. H1's rule is that agents stage only their own paths, and a parallel session works in its own worktree.
- `@be16369` facts drift as M81 and M82 land; ordered step 3 rechecks them.

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
