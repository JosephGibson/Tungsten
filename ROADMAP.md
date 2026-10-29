# Roadmap

What is being worked on, what comes next and what waits, one line per item ([operating rules](docs/operating-rules.md#status)). Now holds one to three items; a new idea goes to Later as one line. The plan behind the order is the [roadmap redo](docs/plans/roadmap-redo.md) (`D-215`), and its order is intent, re-cut after each game finding.

## Now

- `phase-7`: the Phase 7 plan (redo 7.7) assigns every register row by ID, `PA-` rows included, and names 7.14's and 7.15's finite lists ([plan](docs/plans/phase-7.md)); 7.2 (`tungsten run --script`, `render` and `describe`) and 7.3 (`just perf`'s sittings, counter guards and GPU counters, `D-226`) have landed, and 7.4 (H2) runs from its plan; 7.4–7.5 have not landed, so 7.5's rows join at its triage; Phase 7 runs from it a step at a time.
- `health-h2`: redo 7.4, H2: H1's overhead retaken, the five carried workflow proposals decided, `just status`, `just release <kind>` with a final tag publishing on its push, the planning, next, release and decision skills rewritten lean against no-skill baselines with a phase step's prompt, the memory hubs pruned ([plan](docs/plans/health-h2.md)).

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
- The kit's cue listener for `SoundOnEvent`, whose events carry no position, with redo 7.9's pan and looping cue voices (P7-13).
- `just perf`'s GPU run merged into its timing run, now that the timestamp ring costs `gpu` 1.01× (redo 7.3's Q5): at 9.1, once the merged run's cost is measured.
- Phase 7, Seams (redo §3, in [phase-7.md](docs/plans/phase-7.md)'s order): agent tooling, the benchmark runner, H2, the analysis pass, terrain and physics, saves, the kit pass, the UI pass (7.17), screens as data (7.20), camera rigs (7.18), the template, the level format, 01 on the new systems, a CPU and a GPU pass, agent guidance.
- Phase 8, Genres: story systems, shooter and AI systems, tooling for hand-made content, ARPG systems (8.7), 01 refreshed, the template's final pass, proof.
- Phase 9, Release: benchmarks with budgets, performance, the stability policy, quality, docs and platforms, tests, records, release candidates, 1.0.