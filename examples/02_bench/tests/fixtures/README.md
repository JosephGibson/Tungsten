# Visual regression fixtures

`gpu-visual.png` is the reference capture for the opt-in
`gpu_visual_matches_fixture` integration test: the `gpu` benchmark's
`visual` preset (`docs/perf/benchmarks.md`, `D-078`). The preset
draws 2,000 sprites over 3 z layers, half of them lit with normal and
emissive maps, 10% with the `bench_heavy` material at 8 iterations, 4 moving
point lights plus the directional light, one tile layer, 64 static glyphs,
and bloom plus vignette with AA off and a fixed camera.

## Regenerating the fixture

Run on the reference machine only (the fixture is driver-sensitive), from the
repository root, because the benchmark loads `assets/manifest.json` and
`examples/02_bench/assets/manifest.json` by root-relative path. Use a
**debug build**: the test harness compiles the binary under the `dev` profile
(via `CARGO_BIN_EXE_...`), and matching profiles removes one drift source.

```bash
WGPU_BACKEND=vulkan \
TUNGSTEN_BENCH=gpu \
TUNGSTEN_BENCH_PRESET=visual \
TUNGSTEN_SMOKE_FRAMES=8 \
TUNGSTEN_CAPTURE_FRAME=5 \
TUNGSTEN_CAPTURE_RESOLUTION=1280x720 \
TUNGSTEN_CAPTURE_PATH=examples/02_bench/tests/fixtures/gpu-visual.png \
cargo run -p example-02-bench --locked
```

Determinism: under `TUNGSTEN_SMOKE_FRAMES`, `App::stage_time` pins the
real clock's per-frame dt to `1/60 s` (see `SMOKE_MODE_FIXED_DT_SECS` in
`crates/tungsten/src/app.rs`), and `Time::delta()` reads it at scale 1. The
lights move with that pinned time, the text is static (`text_change=0`), and
every other choice comes from the preset's seed, so the capture is
reproducible. Regenerating without `TUNGSTEN_SMOKE_FRAMES` set would drift.

Accept a new fixture only after `just visual` passes twice in a row, then
commit the PNG together with an update to the **Reference machine** block
below so future drift can be diagnosed against a known driver.

## Running the regression test

```bash
TUNGSTEN_VISUAL_REGRESSION=1 cargo test -p example-02-bench --test visual_regression --locked -- --nocapture
```

`just visual` runs the same command. Without `TUNGSTEN_VISUAL_REGRESSION` the
test short-circuits and reports as passing so `cargo test --workspace`
remains green on machines without a GPU.

The command also runs `direct_and_capture_paths_draw_the_same_image`
(`D-087`), behind the same gate. It needs no fixture: it captures frame 5 of
five knob variants of the preset twice, once through the capture path and
once with `TUNGSTEN_CAPTURE_DIRECT=1`, and asserts that the two images are
equal with `tolerance = 0`.

The comparison uses `tungsten::render::compare_png` with `tolerance = 2`
(per-channel delta) and asserts `pixels_above_tolerance == 0`. If the Linux
Vulkan path jitters at that floor in a future driver update, see `D-047` for
the agreed fallback (`pixels_above_tolerance < 16`).

## Reference machine

- OS: Arch Linux, kernel 7.2.8-arch1-2, X11 session
- GPU: AMD Radeon 660M (integrated, Ryzen 5 6600H)
- Driver: Mesa 26.2.4 RADV (`RADV REMBRANDT`), Vulkan API 1.4.354
- wgpu backend: Vulkan (`WGPU_BACKEND=vulkan`), wgpu 30.0.1
- Toolchain: rustc 1.98.1, `dev` profile
- Date: 2026-10-04
- Commit: `6a61cc2` plus M34 (`D-116`): text draws with the packaged JetBrains Mono 2.211. The previous fixture (`0521a2a0…`) drew the HUD with JetBrains Mono 2.304 installed on this machine, which shadowed the packaged face while system fonts loaded; only the two HUD lines differ (2155 pixels).
- SHA-256: `969bdf53653edb27761701085236b0b36399ceb782c0ee4427e812d11c453f06`
- Accepted after two consecutive `just visual` passes (2026-10-04, 22:58:08Z and 22:58:15Z)
