---
status: complete
completed: Steps 1–7
remaining: none
---

# Cargo Profile Optimization Plan

## Summary

- Goal 1: maximize release-binary performance through Cargo/LLVM flags
  (`lto`, `codegen-units`, `panic`, `target-cpu`)
- Goal 2: minimize incremental rebuild time during development
  (faster linker path when available, dep opt-level override, incremental defaults preserved)

Architecture decision:

- use separate Cargo profiles
- release performance and dev rebuild speed are in direct tension
- `lto + codegen-units = 1` improves release output and slows link time
- dev wants the opposite
- no useful middle ground exists
- `.cargo/config.toml` handles cross-cutting settings such as `target-cpu` that help both profiles

## Non-Goals

- `PGO` / `BOLT` (post-link binary instrumentation; out of scope)
- cross-platform portability of binary artifacts
- `CI` pipeline changes (no `CI` per `AGENTS.md`)
- crate restructuring (3-crate layout is correct per `D-007`)
- Windows / macOS linker tuning (Linux is the active development platform)
- Cranelift as the primary backend (nightly-only; appendix only)

## Files Touched

| File | Change | Status |
| --- | --- | --- |
| `.cargo/config.toml` | New — `target-cpu=native`, mold stub | **done** |
| `Cargo.toml` (workspace root) | `[profile.release]`, `[profile.dev.package."*"]` | **done** |
| `crates/tungsten-core/src/config.rs` | Child-process render override parsing for perf capture | **done** |
| `scripts/perf-capture.sh` | `--present-mode`, `--max-frame-latency`, `--telemetry-only` | **done** |
| `scripts/test-perf-capture.sh` | Override-label regression coverage | **done** |
| `DECISIONS.md` | `D-041` with post-optimization benchmark table | **done** |
| `docs/perf/profiling-workflow.md` | Reference matrix updated after re-capture | **done** |

## Resolved Choices

| Choice | Resolution | Outcome |
| --- | --- | --- |
| LTO flavor | `"thin"` | Confirmed: parallel, faster link, meaningful gains |
| `panic = "abort"` | Included and verified | Final follow-up validation passed `cargo test --workspace`; `cpal` audio path unaffected |
| `target-cpu=native` scope | Checked into `.cargo/config.toml` | Applied to all builds, including `cargo bench` |
| Linker | No explicit flag needed | Arch Linux system Rust (`1.94.1`) already routes through `lld` via `x86_64-linux-gnu-gcc` built-in spec. Adding `-fuse-ld=mold` conflicted and broke the build, so it was removed. Install `mold` (`pacman -S mold`) and uncomment the stub in `.cargo/config.toml` if an extra link-time speedup is wanted later. |

## Step Log

### ✅ Step 1 — Create `.cargo/config.toml`

Done. Created at workspace root with `-C target-cpu=native`.

Linker discovery:

- Arch Linux’s system Rust package already uses `lld` internally
- adding `-fuse-ld=mold` broke the build:
  `mold` was not installed,
  and GCC uses the last duplicate `-fuse-ld=` flag
- the mold flag was removed
- the file now keeps a commented-out mold stub for later use after `pacman -S mold`

Current `.cargo/config.toml`:

```toml
[target.x86_64-unknown-linux-gnu]
rustflags = [
    "-C", "target-cpu=native",
]
# Optional sccache block is commented out
```

### ✅ Step 2 — Add `[profile.dev.package."*"]` to workspace `Cargo.toml`

Done.

```toml
[profile.dev.package."*"]
opt-level = 2
```

Effect:

- external deps such as `wgpu`, `winit`, `glam`, and `cpal` compile optimized in dev
- project crates stay at `opt-level = 0` for fast incremental cycles

### ✅ Step 3 — Add `[profile.release]` to workspace `Cargo.toml`

Done.

```toml
[profile.release]
opt-level = 3
lto = "thin"
codegen-units = 1
panic = "abort"
debug = 1
strip = "none"
```

### ✅ Step 4 — Verify `panic = "abort"` Safety

Done.

- `cargo build --workspace` succeeded
- final follow-up validation passed `cargo test --workspace`
- the run reported `193` passing tests
- `cpal` audio-path tests under `tungsten::audio` stayed green

### ✅ Step 5 — Smoke Test (GPU Required)

Done.

- `./scripts/smoke-examples.sh` passed `2/2` examples on the `2026-04-16` follow-up validation run
- layer 1 (`cargo test --workspace`) was already confirmed in Step 4

```bash
./scripts/smoke-examples.sh      # layer 2: ~1–2 min with warm cache, Linux only
```

### ✅ Step 6 — Re-Capture Benchmark Baselines

Done.

- `cargo bench --workspace` ran under the new `bench` profile
- `bench` inherits `[profile.release]`
- results were saved to `perf-runs/bench-20260416-post-opt.txt`
- `DECISIONS.md` `D-041` now carries the full table

All `13` benchmarks improved vs. prior baselines:

| Benchmark | New time | Change |
| --- | --- | --- |
| `spawn_insert_3_components_10k` | `3.736 ms` | `−12.6%` |
| `query_single_10k` | `6.746 µs` | `−1.5%` |
| `query2_homogeneous_10k` | `6.789 µs` | `−6.5%` |
| `query2_fragmented_5arch_10k` | `7.045 µs` | `−8.0%` |
| `query2_10k_5archetypes_pv` | `13.845 µs` | `−3.2%` |
| `spawn_despawn_1k` | `72.964 µs` | `−9.5%` |
| `command_buffer_flush_1k_spawns` | `236.89 µs` | `−7.6%` |
| `naive_query_single_10k` | `29.976 µs` | `−20.8%` |
| `naive_query2_via_entities_10k` | `652.22 µs` | `−31.4%` |
| `event_queue_flush_10_types` | `2.486 µs` | `−19.3%` |
| `position_integration_50k` | `1.980 ms` | `−3.7%` |
| `broadphase_rebuild_5k_dynamic` | `312.56 µs` | `−37.3%` |
| `sprite_extract_batch_build_2k` | `5.842 µs` | `−20.4%` |

Notable outcome:

- `broadphase_rebuild_5k_dynamic` improved the most (`−37.3%`)
- reason: AABB/grid arithmetic vectorized cleanly under `AVX2`
- the D-036 archetypal-vs-naive ratio still holds directionally
- both sides improved proportionally

### ✅ Step 7 — Re-Capture Profiling Baselines (GPU Required)

Done.

`scripts/perf-capture.sh` now supports child-process render overrides via:

- `--present-mode`
- `--max-frame-latency`
- `--telemetry-only`

This lets the Vulkan matrix be regenerated without editing `tungsten.json`.

```bash
WGPU_BACKEND=vulkan ./scripts/perf-capture.sh sprite-stress 300
WGPU_BACKEND=vulkan ./scripts/perf-capture.sh platformer 300
WGPU_BACKEND=vulkan ./scripts/perf-capture.sh sprite-stress 300 --present-mode immediate --max-frame-latency 2 --telemetry-only
WGPU_BACKEND=vulkan ./scripts/perf-capture.sh sprite-stress 300 --present-mode immediate --max-frame-latency 3 --telemetry-only
WGPU_BACKEND=vulkan ./scripts/perf-capture.sh sprite-stress 300 --present-mode mailbox --max-frame-latency 2 --telemetry-only
WGPU_BACKEND=vulkan ./scripts/perf-capture.sh sprite-stress 300 --present-mode mailbox --max-frame-latency 3 --telemetry-only
WGPU_BACKEND=vulkan ./scripts/perf-capture.sh platformer 300 --present-mode immediate --max-frame-latency 2 --telemetry-only
WGPU_BACKEND=vulkan ./scripts/perf-capture.sh platformer 300 --present-mode mailbox --max-frame-latency 2 --telemetry-only
```

`docs/perf/profiling-workflow.md` now reflects the `2026-04-16` re-capture sweep with these recorded profile flags:

- `lto = "thin"`
- `codegen-units = 1`
- `panic = "abort"`
- `target-cpu=native`
- AMD Ryzen `5 6600H`
- Radeon `660M`
- `rustc 1.94.1`

Observed outcome:

- the post-optimization sweep still reads as presentation-pacing-bound
- on the default `Immediate / 1` path, `example-02-sprite-stress` kept `avg_gpu ≈ 0.61 ms`
- total/acquire timing moved primarily with present mode and max-frame-latency choices
- `Mailbox / 3` produced the lowest sprite-stress averages:
  `avg_total = 2.46 ms`,
  `avg_acquire = 2.07 ms`
- the checked-in default remains `Immediate / 1`
- reason:
  the engine’s `auto` path intentionally preserves the existing no-vsync selection,
  and cross-scene gains were not strong enough to justify a blanket override

## Done-When Checks

- [x] `.cargo/config.toml` exists with `target-cpu=native`; mold stub is commented in for later
- [x] `[profile.release]` exists in workspace `Cargo.toml` with `lto`, `codegen-units`, `panic`, `debug`, `strip`
- [x] `[profile.dev.package."*"]` exists in workspace `Cargo.toml` with `opt-level = 2`
- [x] `cargo build --workspace` succeeds
- [x] `cargo test --workspace` passes (`193/193` on the final follow-up validation run)
- [x] `./scripts/smoke-examples.sh` passes
- [x] `cargo bench --workspace` completes without errors
- [x] `DECISIONS.md` `D-041` records post-optimization numbers and host CPU
- [x] `docs/perf/profiling-workflow.md` reference matrix is updated

## Appendix — Mold Upgrade Path

When `mold` is installed (`pacman -S mold`):

1. Open `.cargo/config.toml` and replace the current block with:

```toml
[target.x86_64-unknown-linux-gnu]
rustflags = [
    "-C", "link-arg=-fuse-ld=mold",
    "-C", "target-cpu=native",
]
```

2. Run `cargo build --workspace` to confirm the link step uses mold. Compare incremental link time against the current `lld` baseline.
3. Make no profile changes. Mold affects link time, not codegen.

## Appendix — Fat LTO One-Liner

Use this only when you intentionally trade release link time for maximum inlining, for example while cutting a tagged release build rather than doing everyday profiling.

```toml
# In [profile.release]:
lto = true          # equivalent to "fat"; single merged LLVM module
codegen-units = 1   # already set; fat LTO enforces this anyway
```

Fat `LTO` makes release linking strictly serial and memory-intensive. Thin `LTO` is the better everyday default.

## Appendix — Cranelift (Optional, Nightly Only)

Cranelift is useful for very fast `cargo build` cycles when GPU correctness is not the focus, for example while editing ECS or physics logic or running unit tests.

```toml
# In [profile.dev] — requires a nightly toolchain pinned via rust-toolchain.toml
[profile.dev]
codegen-backend = "cranelift"
```

Known limitations as of `2026-Q1`:

- nightly-only
- no SIMD intrinsics
- known correctness issues on a small set of patterns

This project has no `rust-toolchain.toml`. Cranelift is not a primary path.
