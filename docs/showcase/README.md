# Showcase Captures

Manual acceptance artifacts for milestone visual checks. These are not part of
`cargo test` — regenerate with the noted commands when the relevant subsystem
changes meaningfully.

## M27 — SMAA presentation AA (`smaa_off_vs_high.png`)

A 2-up PNG composed offline from two screenshots taken under
`example-04-shader-playground` with a deliberately aliased scene:
- Left half: `post_aa = off`, hard pixel staircases on rotated sprites.
- Right half: `post_aa = smaa_high`, edges resolved.

Regenerate (Linux):

```bash
WGPU_BACKEND=vulkan \
  TUNGSTEN_SMOKE_FRAMES=8 \
  TUNGSTEN_CAPTURE_FRAME=6 \
  TUNGSTEN_CAPTURE_PATH=docs/showcase/_smaa_off.png \
  TUNGSTEN_CAPTURE_RESOLUTION=1280x720 \
  TUNGSTEN_POST_AA_FIXTURE=off \
  cargo run -p example-04-shader-playground --quiet

WGPU_BACKEND=vulkan \
  TUNGSTEN_SMOKE_FRAMES=8 \
  TUNGSTEN_CAPTURE_FRAME=6 \
  TUNGSTEN_CAPTURE_PATH=docs/showcase/_smaa_high.png \
  TUNGSTEN_CAPTURE_RESOLUTION=1280x720 \
  TUNGSTEN_POST_AA_FIXTURE=smaa_high \
  cargo run -p example-04-shader-playground --quiet
```

Then compose the side-by-side image (e.g. `convert _smaa_off.png _smaa_high.png +append smaa_off_vs_high.png` from ImageMagick) and remove the two intermediate `_smaa_*.png` files. Commit `smaa_off_vs_high.png` only.
