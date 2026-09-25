# Visual regression fixtures

`baseline-sprite-stress.png` is the reference capture for the opt-in
`sprite_stress_matches_baseline` integration test.

## Regenerating the baseline

Run on the reference machine only (the baseline is driver-sensitive). Use a
**debug build** — the test harness compiles the binary under the `dev`
profile (via `CARGO_BIN_EXE_...`), and matching profiles removes one drift
source:

```bash
TUNGSTEN_SMOKE_FRAMES=8 \
TUNGSTEN_CAPTURE_FRAME=5 \
TUNGSTEN_CAPTURE_RESOLUTION=1280x720 \
TUNGSTEN_CAPTURE_PATH=examples/02_sprite_stress/tests/fixtures/baseline-sprite-stress.png \
cargo run -p example-02-sprite-stress
```

Determinism: under `TUNGSTEN_SMOKE_FRAMES`, `App::stage_delta_time` pins the
per-frame `DeltaTime.dt` to `1/60 s` (see `SMOKE_MODE_FIXED_DT_SECS` in
`crates/tungsten/src/app.rs`). That pin is what makes this capture
reproducible; regenerating without `TUNGSTEN_SMOKE_FRAMES` set would drift.

Commit the resulting PNG together with an update to the **Reference machine**
block below so future drift can be diagnosed against a known driver.

## Running the regression test

```bash
TUNGSTEN_VISUAL_REGRESSION=1 cargo test -p example-02-sprite-stress --test visual_regression -- --nocapture
```

Without `TUNGSTEN_VISUAL_REGRESSION` the test short-circuits and reports as
passing so `cargo test --workspace` remains green on machines without a GPU.

The comparison uses `tungsten_render::compare_png` with `tolerance = 2`
(per-channel delta) and asserts `pixels_above_tolerance == 0`. If the Linux
Vulkan path jitters at that floor in a future driver update, see `D-047` for
the agreed fallback (`pixels_above_tolerance < 16`).

## Reference machine

- OS: Arch Linux, kernel 7.2.6-arch2-1, X11 session
- GPU: AMD Radeon 660M (integrated, Ryzen 5 6600H)
- Driver: Mesa 26.2.3 RADV (`RADV REMBRANDT`), Vulkan API 1.4.354
- wgpu backend: Vulkan (`WGPU_BACKEND=vulkan`), wgpu 29.0.1
- Toolchain: rustc 1.95.0, `dev` profile
- Date: 2026-09-25
- Commit: `6f80bb0` (captured before the Rust 1.98.1 / dependency migrations)
- SHA-256: `acd3c434ef7854bcc99f59d4cb4053f550e7679b5fd4ad515fda0ea961355f00`
