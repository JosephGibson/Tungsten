# Showcase Captures

Manual visual acceptance artifacts, separate from `cargo test` and the benchmark's golden fixture. Regenerate on a GPU/display machine when the relevant visuals change; a recipe alone is not evidence that an artifact was captured or reviewed.

## Artifact status

| Milestone / output | Availability | Comparison |
| --- | --- | --- |
| M27 — `smaa_off_vs_high.png` | Not checked in; regeneration recipe only | Shader playground: AA off vs SMAA High, with an empty post stack |
| M28 — `bloom_off_vs_on.png` | Not checked in; regeneration recipe only | Shader playground: `ex04_emissive_quad` without bloom vs demo-tuned bloom alone |
| M29 — `lighting_off_vs_on.png` | Checked in at `215303d` (0.26); historical art before the platformer revamp | Platformer: material/unlit player vs normal-mapped player, two point lights and one directional |
| M31 — `mesh_trail_off_vs_on.png` | Not checked in; regeneration recipe only. Captured and inspected on 2026-10-02 (`10e3cd9` plus the M31 working tree, Vulkan on Radeon 660M): 943 pixels differ, yellow triangles over the first bouncer and none without the trail | Shader playground: `TUNGSTEN_MESH_TRAIL_FIXTURE=off` vs the default bullet trail, with an empty post stack and AA off |
| M31 — `transitions.png` | Not checked in; regeneration recipe only. Captured and inspected on 2026-10-02 (same tree and backend), frame 4 at cover 0.67: against `none`, fade changes 921,600 of 921,600 pixels, the radial wipe 758,996, dissolve 889,772 and pixelate 42,759; the example's text dims with the cover and stays visible over the wipe | Scene/state example: the menu under `none`, fade, radial wipe, dissolve and pixelate, mid-`Out` |

The M29 image documents its original milestone, rather than the current platformer art. New captures should identify the tested commit, backend, fixture settings and result here. Do not replace the benchmark's golden image through this workflow; see [its fixture guide](../../examples/02_bench/tests/fixtures/README.md).

## Regenerate from the repository root

Define the shared capture helper in Bash:

```bash
capture_fixture() {
  local package="$1" capture_path="$2"
  shift 2
  env WGPU_BACKEND=vulkan \
    TUNGSTEN_SMOKE_FRAMES=8 TUNGSTEN_CAPTURE_FRAME=6 \
    TUNGSTEN_CAPTURE_RESOLUTION=1280x720 \
    TUNGSTEN_CAPTURE_PATH="$capture_path" \
    "$@" cargo run -p "$package" --quiet --locked
}
```

Capture both sides with the chosen fixture settings:

```bash
# SMAA; keep bloom/post effects off in both images.
capture_fixture example-04-shader-playground docs/showcase/_smaa_off.png \
  TUNGSTEN_POST_AA_FIXTURE=off TUNGSTEN_POST_STACK_FIXTURE=empty TUNGSTEN_BLOOM_FIXTURE=off
capture_fixture example-04-shader-playground docs/showcase/_smaa_high.png \
  TUNGSTEN_POST_AA_FIXTURE=smaa_high TUNGSTEN_POST_STACK_FIXTURE=empty TUNGSTEN_BLOOM_FIXTURE=off

# Bloom; keep AA off in both images.
capture_fixture example-04-shader-playground docs/showcase/_bloom_off.png \
  TUNGSTEN_POST_AA_FIXTURE=off TUNGSTEN_POST_STACK_FIXTURE=empty TUNGSTEN_BLOOM_FIXTURE=off
capture_fixture example-04-shader-playground docs/showcase/_bloom_on.png \
  TUNGSTEN_POST_AA_FIXTURE=off TUNGSTEN_POST_STACK_FIXTURE=bloom_only TUNGSTEN_BLOOM_FIXTURE=on

# Lighting; the same smoke frame keeps player/ball state aligned.
capture_fixture example-01-platformer docs/showcase/_lighting_off.png TUNGSTEN_LIGHTING_FIXTURE=off
capture_fixture example-01-platformer docs/showcase/_lighting_on.png TUNGSTEN_LIGHTING_FIXTURE=on

# Mesh particle trail (M31); keep the post stack empty and AA off in both images.
capture_fixture example-04-shader-playground docs/showcase/_mesh_trail_off.png \
  TUNGSTEN_POST_AA_FIXTURE=off TUNGSTEN_POST_STACK_FIXTURE=empty TUNGSTEN_MESH_TRAIL_FIXTURE=off
capture_fixture example-04-shader-playground docs/showcase/_mesh_trail_on.png \
  TUNGSTEN_POST_AA_FIXTURE=off TUNGSTEN_POST_STACK_FIXTURE=empty

# Screen transitions (M31). A fixture effect runs 0.1 s per phase and smoke frames step
# 1/60 s: frame 4 is mid-`Out` at cover 0.67, and frame 6 is the fully covered boundary frame.
for effect in none fade wipe_radial dissolve pixelate; do
  capture_fixture example-03-scene-state "docs/showcase/_transition_${effect}.png" \
    TUNGSTEN_TRANSITION_FIXTURE="$effect" TUNGSTEN_CAPTURE_FRAME=4
done
```

Use the same build and inherited render/display settings for both sides. `TUNGSTEN_CAPTURE_FRAME` keeps the renderer's screenshot path; `TUNGSTEN_CAPTURE_DIRECT=1` is a separate test hook for comparing presentation paths.

Compose a pair using ImageMagick, for example:

```bash
convert docs/showcase/_smaa_off.png docs/showcase/_smaa_high.png +append docs/showcase/smaa_off_vs_high.png
convert docs/showcase/_bloom_off.png docs/showcase/_bloom_on.png +append docs/showcase/bloom_off_vs_on.png
convert docs/showcase/_lighting_off.png docs/showcase/_lighting_on.png +append docs/showcase/lighting_off_vs_on.png
convert docs/showcase/_mesh_trail_off.png docs/showcase/_mesh_trail_on.png +append docs/showcase/mesh_trail_off_vs_on.png
convert docs/showcase/_transition_{none,fade,wipe_radial,dissolve,pixelate}.png +append docs/showcase/transitions.png
```

Inspect the comparison, update the artifact status/provenance, and remove the known intermediate files of the comparison. Commit the reviewed composite and its documentation.
