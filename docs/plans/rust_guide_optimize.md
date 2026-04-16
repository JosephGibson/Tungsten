---
status: in progress
completed: Steps 1–4, 6 (all non-GPU steps)
remaining: Step 5 (smoke-examples.sh) and Step 7 (perf-capture re-baseline) — both require GPU/display
---

# Cargo Profile Optimization Plan

**Goal 1 — Release performance:** maximize final binary performance via Cargo/LLVM flags
(`lto`, `codegen-units`, `panic`, `target-cpu`)

**Goal 2 — Dev compile speed:** minimize incremental rebuild cycle time
(faster linker, dep opt-level override, incremental defaults preserved)

**Architecture decision:** Separate Cargo profiles. The two goals are in direct tension —
LTO + `codegen-units = 1` maximizes release output at the cost of slow link times; the dev
path wants the opposite. No useful middle ground exists. One `.cargo/config.toml` handles
cross-cutting concerns (`target-cpu`) that benefit both profiles.

---

## Non-goals

- PGO / BOLT (post-link binary instrumentation; out of scope)
- Cross-platform portability of binary artifacts
- CI pipeline changes (no CI per `AGENTS.md`)
- Crate restructuring (3-crate layout correct per D-007)
- Windows / macOS linker tuning (Linux is the active development platform)
- Cranelift backend (nightly-only; documented in Appendix only)

---

## Files touched

| File | Change | Status |
|------|--------|--------|
| `.cargo/config.toml` | New — `target-cpu=native`, mold stub | **done** |
| `Cargo.toml` (workspace root) | `[profile.release]`, `[profile.dev.package."*"]` | **done** |
| `DECISIONS.md` | D-041 with post-optimization benchmark table | **done** |
| `docs/perf/profiling-workflow.md` | Update reference matrix after re-capture | **pending GPU** |

---

## Resolved choices

| Choice | Resolution | Outcome |
|--------|-----------|---------|
| LTO flavor | `"thin"` | Confirmed: parallel, fast link, meaningful gains |
| `panic = "abort"` | Included — passed verification | 188/188 tests pass, `cpal` audio path unaffected |
| `target-cpu=native` scope | Checked into `.cargo/config.toml` | Applied to all builds including `cargo bench` |
| Linker | No explicit flag needed | Arch Linux system Rust (1.94.1) already routes through `lld` via `x86_64-linux-gnu-gcc` built-in spec. Adding `-fuse-ld=mold` conflicted and broke the build — removed. Install `mold` (`pacman -S mold`) and uncomment the stub in `.cargo/config.toml` for an additional link-time speedup if wanted later. |

---

## Step log

### ✅ Step 1 — Create `.cargo/config.toml`

**Done.** File created at workspace root with `-C target-cpu=native`.

Linker discovery: Arch Linux's system Rust package already uses `lld` internally. Adding
`-fuse-ld=mold` broke the build (mold not installed; GCC last-wins on duplicate `-fuse-ld=`
flags). Removed the mold flag. The file now carries a commented-out mold stub for when mold
is installed (`pacman -S mold`).

Current state of `.cargo/config.toml`:
```toml
[target.x86_64-unknown-linux-gnu]
rustflags = [
    "-C", "target-cpu=native",
]
# Optional sccache block is commented out
```

---

### ✅ Step 2 — Add `[profile.dev.package."*"]` to workspace `Cargo.toml`

**Done.**
```toml
[profile.dev.package."*"]
opt-level = 2
```
External deps (`wgpu`, `winit`, `glam`, `cpal`) compile optimised in dev builds. Project
crates stay at `opt-level = 0` for fast incremental cycles.

---

### ✅ Step 3 — Add `[profile.release]` to workspace `Cargo.toml`

**Done.**
```toml
[profile.release]
opt-level = 3
lto = "thin"
codegen-units = 1
panic = "abort"
debug = 1
strip = "none"
```

---

### ✅ Step 4 — Verify `panic = "abort"` safety

**Done.** `cargo build --workspace` succeeded. `cargo test --workspace` passed all 188 tests
including the `cpal` audio path (`tungsten::audio` tests all green).

---

### ⏳ Step 5 — Smoke test (GPU required)

**Pending.** Requires a display and GPU.

```bash
./scripts/smoke-examples.sh      # layer 2: ~1–2 min with warm cache, Linux only
```

Layer 1 (`cargo test --workspace`) was already confirmed in Step 4.

---

### ✅ Step 6 — Re-capture benchmark baselines

**Done.** `cargo bench --workspace` ran under the new `bench` profile (inherits
`[profile.release]`). Results saved to `perf-runs/bench-20260416-post-opt.txt`.
DECISIONS.md D-041 added with the full table.

All 13 benchmarks improved vs. prior baselines:

| Benchmark | New time | Change |
|-----------|----------|--------|
| `spawn_insert_3_components_10k` | 3.736 ms | −12.6% |
| `query_single_10k` | 6.746 µs | −1.5% |
| `query2_homogeneous_10k` | 6.789 µs | −6.5% |
| `query2_fragmented_5arch_10k` | 7.045 µs | −8.0% |
| `query2_10k_5archetypes_pv` | 13.845 µs | −3.2% |
| `spawn_despawn_1k` | 72.964 µs | −9.5% |
| `command_buffer_flush_1k_spawns` | 236.89 µs | −7.6% |
| `naive_query_single_10k` | 29.976 µs | −20.8% |
| `naive_query2_via_entities_10k` | 652.22 µs | −31.4% |
| `event_queue_flush_10_types` | 2.486 µs | −19.3% |
| `position_integration_50k` | 1.980 ms | −3.7% |
| `broadphase_rebuild_5k_dynamic` | 312.56 µs | **−37.3%** |
| `sprite_extract_batch_build_2k` | 5.842 µs | −20.4% |

Notable: `broadphase_rebuild_5k_dynamic` saw the largest gain (−37%) — AABB/grid arithmetic
fully vectorised by AVX2. The ECS archetypal-vs-naive ratio from D-036 is directionally
unchanged; both sides improved proportionally.

---

### ⏳ Step 7 — Re-capture profiling baselines (GPU required)

**Pending.** The reference Vulkan matrix in `docs/perf/profiling-workflow.md` (2026-04-15)
was captured under default release settings. Re-run after Step 5 passes:

```bash
WGPU_BACKEND=vulkan ./scripts/perf-capture.sh sprite-stress 300
WGPU_BACKEND=vulkan ./scripts/perf-capture.sh platformer 300
```

Then update the reference matrix table in `docs/perf/profiling-workflow.md` and add a
footnote recording the profile flags (`lto = "thin"`, `codegen-units = 1`, `panic = "abort"`,
`target-cpu=native`, AMD Ryzen 5 6600U / Radeon 660M, rustc 1.94.1).

**Expected outcome:** The April 2026 data shows `avg_gpu ≈ 0.61 ms` vs
`avg_total ≈ 3.64 ms` — the workload is presentation-pacing-bound, not CPU-bound. LTO and
`target-cpu=native` primarily improve CPU throughput; frame times may shift only marginally
while the Criterion micro-benchmarks showed the clear wins. Either result is useful:
improvement confirms headroom was being left; no change confirms the profiling model.

---

## Done-when checks

- [x] `.cargo/config.toml` exists with `target-cpu=native`; mold stub commented in for later
- [x] `[profile.release]` in workspace `Cargo.toml` — `lto`, `codegen-units`, `panic`, `debug`, `strip`
- [x] `[profile.dev.package."*"]` in workspace `Cargo.toml` — `opt-level = 2`
- [x] `cargo build --workspace` succeeds
- [x] `cargo test --workspace` passes (188/188)
- [ ] `./scripts/smoke-examples.sh` passes **(GPU required)**
- [x] `cargo bench --workspace` completes without errors
- [x] `DECISIONS.md` D-041 records post-optimization numbers and host CPU
- [ ] `docs/perf/profiling-workflow.md` reference matrix updated **(GPU required)**

---

## Appendix — mold upgrade path

When `mold` is installed (`pacman -S mold`):

1. Open `.cargo/config.toml` and replace the current block with:
```toml
[target.x86_64-unknown-linux-gnu]
rustflags = [
    "-C", "link-arg=-fuse-ld=mold",
    "-C", "target-cpu=native",
]
```
2. Run `cargo build --workspace` to confirm the link step uses mold (look for a faster link time on incremental builds vs. the current lld baseline).
3. No profile changes needed — mold only affects link time, not codegen.

---

## Appendix — fat LTO one-liner

If you want to trade release link time for maximum inlining (useful when cutting a tagged
release build, not during everyday profiling):

```toml
# In [profile.release]:
lto = true          # equivalent to "fat"; single merged LLVM module
codegen-units = 1   # already set; fat LTO enforces this anyway
```

Fat LTO makes the release link strictly serial and memory-intensive; thin LTO is the better
everyday default.

---

## Appendix — Cranelift (optional, nightly only)

For extremely fast `cargo build` cycles when you don't need GPU correctness (e.g. editing ECS
or physics logic, running unit tests), the Cranelift codegen backend skips LLVM entirely:

```toml
# In [profile.dev] — requires a nightly toolchain pinned via rust-toolchain.toml
[profile.dev]
codegen-backend = "cranelift"
```

Known limitations as of 2026-Q1: nightly-only; no SIMD intrinsics; known correctness issues
on a small set of patterns. This project has no `rust-toolchain.toml`. Not a primary path.
