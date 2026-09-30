# Platformer ball pit and effects

status: done

goal: six example-local improvements: rapid MMB small balls, dedicated lit art,
large collection pit, inward black-hole particles, bounded rainbow impacts,
and richer rising fire.

non-goals: engine changes, other examples, general physics changes.

Context: generated assets and layout are owned by the example's tools. Keep
generator sources and outputs together. Preserve LMB behavior. Override the
two example-specific bindings after input hot reload without changing shared input.

files to touch: examples/01_platformer/src/{state,setup,systems,gameplay,extract}.rs,
src/tests/{main,ball_pit}.rs, tools/{art,generate,test_generate}.py, tools/level.json,
tools/README.md, generated assets/layout/inventory, and this plan.

ordered steps:
1. Add independent small-ball spawn state, sprite/animation selection and size;
   configure MMB spawn_small_ball and S-only audio_stop_all locally.
2. Capture pre-physics velocities and trigger impacts above 420 px/s closing
   speed; cap at four/frame, 0.12 s/ball, and 16 live transient emitters.
3. Generate six distinct lit small-ball frames and rainbow burst preset; improve
   black-hole annular birth/inward steering and rising fire presets.
4. Extend the end with a 48x28-tile pit interior and four-tile-thick shell;
   regenerate the map and bounds together.
5. Test input, rates/sizes/art, impacts/caps, generated sources, and containment;
   run just check and just smoke. Record visual limits.

done-when: required spawn/impact/audio tests pass; generator --check and
authoring tests pass; just check and just smoke pass. Human visual review uses
cargo run -p example-01-platformer.

## Results

Implemented all six changes within the platformer; shared engine/input files
are unchanged. Generated source and assets match (339 outputs). Six new runtime
regressions cover rate/size/sprites, S-only stop including reload, speed threshold,
relative body contacts, emitter limits/cleanup, rainbow/vortex behavior and pit
containment. The pit test contains 2,048 mixed balls for 180 physics frames.

Validation passed: just check (55 platformer tests and the full workspace),
just smoke (all four examples and fixture matrices), just repo-check,
generator --check, 15 Python authoring tests, and git diff --check.
The final vortex correction also verifies inward integration at 30/60/144 FPS.

Human visual review remains: small-sprite readability, pit fit during camera
travel, and black-hole/rainbow/fire appearance and density in motion. Startup
smoke does not verify these. Run cargo run -p example-01-platformer.
