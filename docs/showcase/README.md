# Showcase Captures

Manual visual acceptance artifacts, separate from `cargo test` and the benchmark's golden fixture. Regenerate on a GPU/display machine when the relevant visuals change; a recipe alone is not evidence that an artifact was captured or reviewed.

## Artifact status

| Milestone / output | Availability | Comparison |
| --- | --- | --- |
| M27 — `smaa_off_vs_high.png` | Not checked in; regeneration recipe only | Shader playground: AA off vs SMAA High, with an empty post stack |
| M28 — `bloom_off_vs_on.png` | Not checked in; regeneration recipe only | Shader playground: `ex04_emissive_quad` without bloom vs demo-tuned bloom alone |
| M29 — `lighting_off_vs_on.png` | Checked in at `215303d` (0.26); historical art before the platformer revamp | Platformer: material/unlit player vs normal-mapped player, two point lights and one directional |

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
```

Use the same build and inherited render/display settings for both sides. `TUNGSTEN_CAPTURE_FRAME` keeps the renderer's screenshot path; `TUNGSTEN_CAPTURE_DIRECT=1` is a separate test hook for comparing presentation paths.

Compose a pair using ImageMagick, for example:

```bash
convert docs/showcase/_smaa_off.png docs/showcase/_smaa_high.png +append docs/showcase/smaa_off_vs_high.png
convert docs/showcase/_bloom_off.png docs/showcase/_bloom_on.png +append docs/showcase/bloom_off_vs_on.png
convert docs/showcase/_lighting_off.png docs/showcase/_lighting_on.png +append docs/showcase/lighting_off_vs_on.png
```

Inspect the comparison, update the artifact status/provenance, and remove the two known intermediate files for the pair. Commit the reviewed composite and its documentation.
