# Roadmap

What is being worked on, what comes next and what waits, one line per item ([operating rules](docs/operating-rules.md#status)). Now holds one to three items; a new idea goes to Later as one line. The plan behind the order is the [roadmap redo](docs/plans/roadmap-redo.md) (`D-215`), and its order is intent, re-cut after each game finding.

## Now

- `phase-7`: the Phase 7 plan (redo 7.7) assigns every register row by ID, `PA-` rows included, and names 7.14's and 7.15's finite lists ([plan](docs/plans/phase-7.md)); 7.2 (`tungsten run --script`, `render` and `describe`), 7.3 (`just perf`'s sittings, counter guards and GPU counters, `D-226`), 7.4 (H2: `just status`, releases on a tag push with `just release`, `D-227`, and the lean skills) and 7.5 (the analysis pass: [the seam register](docs/plans/seam-register.md)'s 193 verified S-rows, triaged into the plan's tables) have landed, and 7.8, terrain and physics, is next; Phase 7 runs from it a step at a time.
- `physics-on-terrain`: redo 7.8's second plan, physics on the runtime terrain: body seams, physics layer names and materials, friction and circles that roll, contact events with speeds, begins and ends, sensor exits and swept sensors, impulses and a kinematic body's push, casts that pass refused hits, a resting query index, slope tiles, the platformer controller on slopes and pushed by bodies, and `friction` and `slopes` rows ([plan](docs/plans/physics-on-terrain.md)); it closes phase 7's Step 5.

## Next

## Later

- The platformer controller's free air-jump count set at run time, and an unlimited free count (K25's rest): a game that wants them.
- Example 01's route test: eight `LAUNCHES` rows start with the body over the next step's face; restart them clear of the rock, as the tomes' rows are.
- Example 01's snowballs dousing burning props and chilling bodies, if play asks for it.
- Example 01's crates breaking into planks under a blast or as they burn out, if play asks for it.
- Example 01's geysers lighting, throwing or chilling props and balls, and throwing a goblin as high as the wizard (it hops about 13 px), if play asks for it.
- Example 01's props' and toys' boxes and anchors as data beside their sprites, as the actors' bodies are (`assets/data/bodies/`): `STONE_*`, `CRATE_*`, `KEG_*`, `MUZZLE_OFFSET`, `SPRAY_MUZZLE`, `BOMB_TIP`, `SCORCH_ANCHOR`, and the wisp prefab's glow offset, its body's centre; in a later 01 stop.
- Example 01's tile variants, re-rolled once at 0.112 (stills in `perf-runs/20261028-quick-wins-tileset/`): a platform the owner names at a playtest takes its old picks back as per-cell overrides from `5e516e60`'s `level.tmj`.
- Example 01's ribcage prop (`bones_1`), read as barbed wire in the hollow, off the troll bridge too (385.6, 401), if the owner reads it so there.
- Example 01's hurt flash drawn as a generated full-view sprite it stretches and fades (`examples/01_platformer/tools/effects.py:3179`): an edge falloff, deeper corners and a dither on the stock vignette, so a hurt flash drives a second vignette. S-103.
- Example 01's tests hold a press over frames until a fixed step reads it (`examples/01_platformer/tests/game/main.rs:375`): a harness `step_fixed(n)` that steps frames until `n` fixed steps have run. S-182.
- The kit's cue listener for `SoundOnEvent`, whose events carry no position, with redo 7.9's pan and looping cue voices (P7-13).
- `just perf`'s GPU run merged into its timing run, now that the timestamp ring costs `gpu` 1.01× (redo 7.3's Q5): at 9.1, once the merged run's cost is measured.
- Agent memory outside H2's three hubs: `git_fetch_denied` still quotes `scripts/roadmap.py`'s last fetch, which `just status` now prints, and `agent_tooling_run_traps`' perf-gate bullet and `milestone_step_refactor_outside_list` describe retired flows; prune them at the next memory pass.
- The per-version branches retired (H1's Git row): agents would then push `main`, which needs `main`'s pull-request rule lifted and the push guard (`D-216`) widened, both the owner's settings.
- The step's tile proxies without the interior cells, which make no contact: 01's level keeps 1,551 of its 10,093, and one-cell boxes without them matched every pit case (`perf-runs/20261030-runtime-terrain/`), so a frame copies and an edit restages a sixth of them; only which sleepers an edit wakes moves (`D-156`). A measured perf item for 7.14, read on `terrain` (`D-228`).
- Faces between two collision layers' cells exposed only to bodies that do not meet the other layer, seamless joins across layers (`D-228`, runtime-terrain's Q2).
- The auto-tiler's 8-bit blob masks, 47 variants, beside its 4-bit open sides (`D-228`, runtime-terrain's Q4).
- Dropping through a one-way tile edge, with the physics plan's controller (`D-228`, runtime-terrain's Q8).
- Phase 7, Seams (redo §3, in [phase-7.md](docs/plans/phase-7.md)'s order): the analysis pass, terrain and physics, saves, the kit pass, the UI pass (7.17), screens as data (7.20), camera rigs (7.18), the template, the level format, 01 on the new systems, a CPU and a GPU pass, agent guidance.
- Phase 8, Genres: story systems, shooter and AI systems, tooling for hand-made content, ARPG systems (8.7), 01 refreshed, the template's final pass, proof.
- Phase 9, Release: benchmarks with budgets, performance, the stability policy, quality, docs and platforms, tests, records, release candidates, 1.0.