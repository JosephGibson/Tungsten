---
status: done
goal: Explain Tungsten's current swapchain pacing bottleneck and define a practical follow-up plan to reduce acquire/present stalls without regressing correctness.
non-goals: solving VRR/exclusive-fullscreen across all platforms, replacing wgpu, introducing new runtime dependencies, compositor-specific or OS-specific tuning hacks
files-to-touch:
  - docs/plans/swapchain-frame-pacing-followup.md
  - docs/perf/profiling-workflow.md (after experiments, to document chosen policy)
  - crates/tungsten-core/src/config.rs (add render.max_frame_latency and render.present_mode fields)
  - crates/tungsten-render/src/renderer.rs (consume new config fields; adjust latency/mode policy)
  - crates/tungsten/src/app.rs (pass updated RenderConfig through; later if pacing hooks added)
  - tungsten.json (update defaults after policy is decided)
  - scripts/perf-capture.sh (parse present_mode/max_frame_latency into separate README rows; add p95 stat)
---

# Swapchain Frame Pacing Follow-Up

## Implementation Result

Implemented on 2026-04-15 for the `0.10.0` release line:

- `render.present_mode` and `render.max_frame_latency` shipped as typed config knobs.
- Renderer init now rejects `max_frame_latency = 0` and fails fast on unsupported explicit
  concrete present modes.
- `scripts/perf-capture.sh` now reports parsed renderer metadata and post-warm-up
  `p50`/`p95`/`p99` for `total` and `render_acquire`, with
  `scripts/test-perf-capture.sh` covering the parser/percentile helpers.
- Full Vulkan matrix results were documented in
  [docs/perf/profiling-workflow.md](../perf/profiling-workflow.md).
- Checked-in default remains `render.present_mode = "auto"` with
  `render.max_frame_latency = 1`; the later-acquire render-loop refactor was deferred.

## Goal

Turn the current perf observation into an actionable engine plan:

- explain what the `render_acquire` bottleneck likely means in Tungsten today,
- separate "GPU is slow" from "presentation path is pacing us",
- prioritize the highest-value experiments and engine changes,
- avoid over-correcting toward throughput in ways that would quietly regress latency or smoothness.

## Non-Goals

- No VRR / G-Sync / FreeSync work in this pass.
- No compositor-specific or OS-specific hacks — the goal is behavior that is correct and
  reasonable on any compositor and any target OS. Platform-specific tuning is explicitly
  out of scope.
- No replacement of wgpu or new runtime dependencies.
- No architectural changes without data supporting them (see suggestion #6).

## Decisions Made

The following were resolved before implementation begins:

| Question | Decision |
| --- | --- |
| Latency + present mode knobs | Config fields in `tungsten.json` / `RenderConfig`, not env vars. Goal is shipping, not just experiments. |
| Override granularity | Raw knobs in config: `render.max_frame_latency` (integer) and `render.present_mode` (serde enum serialized as a JSON string), parsed into typed Rust config rather than matched as ad hoc `String`s. |
| Architectural changes | In scope if supported by benchmark data — not ruled out by this plan. |
| Compositor / platform testing | Out of scope. Target: correct behavior across any compositor and OS generically. |

## Critical Risks

| Risk | Why it matters | Mitigation |
| --- | --- | --- |
| `render.present_mode` conflicts with `window.vsync` | Without a precedence rule, implementation will guess wrong and docs will drift. | Explicit `render.present_mode` wins. `window.vsync` only chooses between auto-vsync and auto-no-vsync when `render.present_mode` is absent or `auto`. |
| Unsupported explicit present mode silently falls back | That would poison perf captures and make the knob misleading. | Explicit non-`auto` mode must fail renderer init with a clear error listing supported modes. Only auto modes are allowed to fall back. |
| `max_frame_latency = 0` slips through | `wgpu` clamps it anyway, but silent clamping hides bad config and makes captures less trustworthy. | Reject `0` as invalid config or renderer init error. Do not silently coerce it. |
| Config-only knobs make the benchmark matrix awkward | The plan says "no env vars", but the matrix still needs a repeatable execution path. | Matrix runs are manual for the first pass: edit `tungsten.json`, run capture, restore. Script automation is optional follow-up and not required to land the feature. |
| Present-mode policy lands before script/docs can report it | Then captures become harder to compare and regression reports lose context. | Parsing `present_mode` / `max_frame_latency` into separate script README rows is part of the initial implementation, not deferred cleanup. |

## Current Tungsten Reading

Short local validation on 2026-04-15 showed:

- `present_mode = Immediate`
- `max_frame_latency = 1`
- short no-GPU-timing sprite-stress sample:
  - `avg_total ~= 3.913 ms`
  - `avg_render_acquire ~= 3.488 ms`
  - `avg_render_encode ~= 0.165 ms`
  - `avg_render_submit_present ~= 0.228 ms`
- short GPU-timed sprite-stress sample:
  - `avg_gpu ~= 0.57 ms`

### What that means

The GPU is not the main bottleneck here.

If the actual render pass itself is only about `0.57 ms`, but `render_acquire` is several
milliseconds, then most of the steady-state wait is happening before we can even start
encoding commands against the next surface image. In other words:

- we are not primarily limited by sprite draw cost,
- we are not primarily limited by CPU command generation,
- we are mostly limited by when the swapchain/window system hands us the next
  presentable image.

That is why the earlier conclusion said "VSync is off, Immediate is chosen, but
steady-state bottleneck is still mostly swapchain pacing rather than GPU render cost."

## Why "Immediate + VSync Off" Still Waits

This is the key mental model.

`Immediate` means the presentation engine does not intentionally wait for vblank before
swapping, so tearing is allowed. It does **not** mean "the app will never block on
presentation again."

There are still several places where pacing can happen:

1. The surface may have no reusable image available yet.
2. The backend may serialize CPU/GPU work when maximum frame latency is very low.
3. The platform may expose "Immediate" semantics at the API level while still inserting
   practical throttling in the display path.

In Tungsten today, `render_acquire_ms` is measured around `Surface::get_current_texture()`
([renderer.rs:237-252](../crates/tungsten-render/src/renderer.rs)), so that field is
exactly where these waits show up.

## Code Reality Check

Before running the benchmark matrix, several hardcoded constraints must be addressed:

### A. `desired_maximum_frame_latency` is not configurable

[renderer.rs:136-145](../crates/tungsten-render/src/renderer.rs) hardcodes:

```rust
let desired_maximum_frame_latency = if matches!(
    present_mode,
    wgpu::PresentMode::Immediate | wgpu::PresentMode::Mailbox | wgpu::PresentMode::AutoNoVsync
) {
    1
} else {
    2
};
```

There is no config field to change this. The benchmark matrix cannot be run without first
adding `render.max_frame_latency` to `RenderConfig` and wiring it through to the renderer.

### B. `Mailbox` cannot be selected independently

[renderer.rs:73-87](../crates/tungsten-render/src/renderer.rs): `choose_present_mode`
prefers `Immediate` over `Mailbox` when both are supported. Testing `Mailbox` requires
`render.present_mode` to be exposed as a config field that overrides auto-detection.

### C. `present_mode` and `max_frame_latency` are already logged — but not parsed by the script

[app.rs:339-360](../crates/tungsten/src/app.rs) emits at init time:
```
backend: Vulkan adapter: … present_mode: Immediate max_frame_latency: 1 timestamp_query: true
```
`perf-capture.sh` captures this as a single raw cell. The script needs to parse
`present_mode` and `max_frame_latency` into separate README table rows.

### D. `perf-capture.sh` only computes averages — no percentiles

The budget in [profiling-workflow.md](../docs/perf/profiling-workflow.md) references
`p95 frame time <= 16.7ms`, but the script only computes means. Adding p50/p95/p99 for
`render_acquire` and `total` is part of the script extension work.

### E. Precedence and failure behavior are not yet defined

Even after the two fields exist, implementation still needs explicit policy for:

- what happens if `window.vsync = true` but `render.present_mode = "immediate"`,
- what happens if the user asks for `mailbox` on a backend that only supports `fifo`,
- what happens if `max_frame_latency = 0`.

These are resolved in this plan's API section below. They should not be re-litigated during
implementation.

## Research Summary

### 1. `wgpu` present modes do not eliminate pacing by themselves

`wgpu` documents:

- `AutoNoVsync` falls back in this order: `Immediate` → `Mailbox` → `Fifo`
- `Fifo` explicitly blocks `Surface::get_current_texture()` until a queue slot is available.
- `Immediate` removes the presentation queue, but does not guarantee zero wait in the
  broader window-system path.

### 2. `desired_maximum_frame_latency = 1` is intentionally latency-biased, not throughput-biased

`wgpu`'s `SurfaceConfiguration` docs:

- `1` prioritizes minimum latency.
- `1` also means CPU and GPU do **not** get to run in parallel as effectively.
- `2` is the documented balance point.
- `3+` favors throughput over latency.

Most importantly, `wgpu` explicitly warns that when the backend cannot wait on present
directly, `desired_maximum_frame_latency = 1` can cause `Surface::get_current_texture()`
to block and serialize CPU/GPU work. That warning maps directly to Tungsten's current
measurements.

### 3. Raph Levien's frame-pacing article matches the shape of our data

Key takeaways for Tungsten:

- blocking in acquire/present is often the wrong place to pace the frame,
- low-latency rendering is about starting work at the right time,
- a tiny GPU render time does not matter much if image availability and present deadlines
  dominate.

### 4. Platform APIs expose better pacing controls than `wgpu` currently gives us

- DXGI offers `SetMaximumFrameLatency` plus a frame-latency waitable object so apps can
  wait **before starting the next frame**, instead of accidentally blocking deep inside
  present/acquire.
- Vulkan has `VK_KHR_present_wait` for monitoring outstanding presents.

These are noted for context. Tungsten uses `wgpu`, so portable equivalents are limited to
what `desired_maximum_frame_latency` and `PresentMode` expose today.

## Likely Causes In Tungsten Right Now

### A. We are over-biasing toward low latency with `max_frame_latency = 1`

This is the most likely self-inflicted contributor.

Current renderer policy ([renderer.rs:136-145](../crates/tungsten-render/src/renderer.rs)):
any no-vsync present mode gets `desired_maximum_frame_latency = 1`.

That is a valid low-latency policy, but a questionable **throughput profiling** policy. If
the goal is "how fast can the engine sustainably run this scene," forcing latency=1 may be
making `get_current_texture()` block more often than necessary.

### B. The render path acquires first, then does the rest of the CPU work

That is the simplest render loop, but it means all acquire wait time lands in
`render_acquire_ms` and CPU-side work that could theoretically happen earlier is delayed.
This is not necessarily wrong — but it means acquire stalls are very visible in telemetry,
and any improvement to acquire latency shows up immediately in the dominant field.

## Best First Suggestions

These are ordered by likely value per unit of effort.

### 1. Add `render.max_frame_latency` and `render.present_mode` config fields

Add to `RenderConfig` in [config.rs](../crates/tungsten-core/src/config.rs):

```rust
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PresentModeConfig {
    Auto,
    Immediate,
    Mailbox,
    Fifo,
    AutoVsync,
    AutoNoVsync,
}

pub struct RenderConfig {
    pub clear_color: [f64; 4],
    /// Frames-in-flight hint passed to wgpu's desired_maximum_frame_latency.
    /// 1 = lowest latency (current default for no-vsync). 2 = wgpu balance point.
    /// 3 = throughput-biased. Defaults to 2 when vsync is off.
    pub max_frame_latency: Option<u32>,
    /// Override for present mode selection. `Auto` preserves the current
    /// `window.vsync`-driven auto-select behavior.
    pub present_mode: Option<PresentModeConfig>,
}
```

Use `Option` so that `null` / absent in `tungsten.json` falls back to the current auto
logic. This is non-breaking: existing configs with no new fields behave identically.

### API rules for those fields

These rules are part of the implementation contract:

1. `render.present_mode = null` or `"auto"`:
   - preserve current behavior,
   - `window.vsync = true` chooses the auto-vsync family,
   - `window.vsync = false` chooses the auto-no-vsync family.
2. `render.present_mode = "immediate" | "mailbox" | "fifo" | "auto_vsync" | "auto_no_vsync"`:
   - explicit override wins over `window.vsync`,
   - `window.vsync` is ignored for actual present-mode selection,
   - docs should describe `window.vsync` as the legacy/default-path selector, not the final authority.
3. Explicit non-`auto` present mode unsupported by the active surface:
   - fail renderer init with a clear `RenderError`,
   - do not silently fall back.
4. `render.max_frame_latency = 0`:
   - reject as invalid,
   - do not silently clamp.
5. `render.max_frame_latency >= 1`:
   - pass through to `wgpu`,
   - backend clamping is still allowed after that, and the actual chosen value should continue to be logged.

Then in [renderer.rs](../crates/tungsten-render/src/renderer.rs):
- `choose_present_mode` should respect `config.present_mode` when `Some` and not `Auto`.
- `desired_maximum_frame_latency` should use `config.max_frame_latency.unwrap_or(default)`.
- explicit unsupported modes should return a dedicated renderer error such as:

```rust
UnsupportedPresentMode {
    requested: String,
    available: Vec<String>,
}
```

Suggested `tungsten.json` defaults after experiments confirm the better value:
```json
"render": {
  "clear_color": [0.05, 0.05, 0.08, 1.0],
  "max_frame_latency": 2,
  "present_mode": "auto"
}
```

### 2. Run a real capture matrix before changing defaults

Run full 300-measured-frame captures for at least this matrix:

| Scene | Present mode | Max frame latency | GPU timing | Goal |
| --- | --- | --- | --- | --- |
| sprite-stress | Immediate | 1 | off | current low-latency reference |
| sprite-stress | Immediate | 2 | off | first throughput candidate |
| sprite-stress | Immediate | 3 | off | upper-throughput candidate |
| sprite-stress | Mailbox | 2 | off | tear-free high-throughput candidate |
| sprite-stress | Mailbox | 3 | off | queue-heavy candidate |
| sprite-stress | current best | on | spot-check | compare CPU pacing vs true GPU time |
| platformer | same best 2-3 configs | off | sanity | ensure scene-general behavior |

Success signal:

- `render_acquire_ms` falls materially,
- total frame time falls,
- GPU time stays roughly flat,
- no obvious smoothness regression in manual observation.

### Capture protocol for that matrix

Use the existing M12 profiling workflow consistently while gathering comparison data:

- warm up for 60 frames, then measure 300 frames,
- pin the backend for the whole matrix on a given machine (for example
  `WGPU_BACKEND=vulkan` on Linux),
- keep resolution and scene content fixed while varying only present mode / frame latency,
- treat each run directory as immutable evidence; do not overwrite a previous capture with a
  new config,
- record the chosen config in the run directory name and in the generated README.

This keeps the experiment focused on swapchain pacing instead of quietly mixing in backend,
scene, or warm-up differences.

### Matrix execution note

Because this plan chooses config-backed knobs instead of env vars, the first implementation
does **not** need to automate the matrix inside `perf-capture.sh`.

For the initial pass it is acceptable to:

1. edit `tungsten.json`,
2. run `./scripts/perf-capture.sh ...`,
3. archive the result under a mode/latency-specific directory name,
4. restore `tungsten.json`.

If matrix automation is added later, it must use backup/restore plus `trap` so the script
never leaves the repo's checked-in config modified after failure.

### Decision rubric for choosing the default

Do not pick the winning config on average frame time alone.

Prioritize in this order:

1. Correctness and explicitness:
   unsupported explicit modes fail fast; the config does exactly what it says.
2. Frame pacing quality:
   lower `p95`/`p99` for `total` and `render_acquire` beats a tiny average-only win.
3. Throughput:
   lower mean `total` and `render_acquire`.
4. Latency conservatism:
   if two configs are close, prefer the one with lower queue depth / frame latency.

That rule avoids shipping a default that benchmarks well on average but feels worse in
practice.

### 3. Prefer `desired_maximum_frame_latency = 2` as the first serious candidate

Why:

- `wgpu` explicitly documents `2` as the balance point,
- DXGI guidance also treats `2` as the point where CPU and GPU can overlap better,
- it is less likely than `3` to create visible latency creep or queue-induced jitter.

If one change had to be tried first before the full matrix, this would be it.

### 4. Evaluate `Mailbox` separately from `Immediate`

`Mailbox` is worth testing even if `Immediate` is available because:

- it avoids tearing,
- on some backends the real-world pacing may be more stable than `Immediate`,
- `wgpu` docs note backend-specific interactions between `Mailbox` and
  `desired_maximum_frame_latency`.

Requires the `render.present_mode` config field (suggestion #1) since the current
auto-select logic always prefers `Immediate` over `Mailbox`.

### 5. Extend `perf-capture.sh` with percentiles and parsed metadata

Concrete additions:

- Parse p50, p95, p99 from the post-warmup `render_acquire` and `total` columns. This
  closes the gap between the existing `p95 frame time` budget target and what the script
  actually measures.
- Extract `present_mode` and `max_frame_latency` from the `backend:` init line into
  separate README table rows (they are already logged by [app.rs:339-360](../crates/tungsten/src/app.rs)).
- Keep the existing average rows; percentiles are additive, not a replacement.
- Do not attempt to automate the whole matrix in the first pass. Script work here is
  reporting/attribution, not orchestration.

### 6. Consider moving more work before surface acquisition if tuning is insufficient

If latency/throughput tuning does not move the needle enough, the next architectural step
is to restructure the render loop in [renderer.rs](../crates/tungsten-render/src/renderer.rs):

- do camera update, command encoding prep, and CPU-side work before calling
  `get_current_texture()`,
- acquire the swapchain image as late as possible — ideally just before `queue.submit()`.

This reduces the window during which `get_current_texture()` can block, because the GPU
has had more time to finish its previous frame by the time we ask for the next image.

Trade-offs to consider:
- The render pass descriptor references the surface texture view, so acquisition still
  must happen before `begin_render_pass`. The prep work that can move earlier is primarily
  camera buffer writes, text layout (`text_pipeline.prepare`), and sprite buffer uploads.
- Adds minor structural complexity to `render_frame_full` but does not require an offscreen
  target or extra blit.

Only pursue this if the latency=2 tuning leaves `render_acquire_ms` still dominant. The
current data does not justify the added complexity yet.

## Suggested Execution Order

- [ ] **Step 0:** Add `PresentModeConfig`, `render.max_frame_latency`, and
      `render.present_mode` to `RenderConfig` in
      [config.rs](../crates/tungsten-core/src/config.rs). Wire through `App` → `Renderer::new`.
- [ ] **Step 1:** Implement precedence/validation rules in
      [renderer.rs](../crates/tungsten-render/src/renderer.rs):
  - explicit `render.present_mode` wins over `window.vsync`,
  - explicit unsupported mode returns renderer init error,
  - `max_frame_latency = 0` is rejected,
  - auto modes preserve current fallback behavior.
- [ ] **Step 2:** Extend `perf-capture.sh` to parse present_mode/max_frame_latency into
      separate README rows; add p50/p95/p99 for `render_acquire` and `total`.
- [ ] **Step 3:** Set `max_frame_latency = 2` in `tungsten.json` temporarily and capture
      300-frame `sprite-stress` baseline. Compare against `latency = 1` reference.
- [ ] **Step 4:** Run the full 7-config matrix from suggestion #2.
- [ ] **Step 5:** Repeat the best two configs on `example-01-platformer`.
- [ ] **Step 6:** Based on data, decide:
  - update `tungsten.json` defaults to the winning config,
  - document the chosen policy in [profiling-workflow.md](../docs/perf/profiling-workflow.md).
- [ ] **Step 7 (if needed):** If `render_acquire_ms` remains dominant after steps 0–6,
      restructure `render_frame_full` to move CPU-side prep before acquisition.

## Done When

- `render.max_frame_latency` and `render.present_mode` are config fields in `tungsten.json`.
- Explicit precedence/error behavior is implemented and documented:
  - explicit present mode beats `window.vsync`,
  - unsupported explicit mode fails fast,
  - `max_frame_latency = 0` is rejected.
- `perf-capture.sh` emits p95 for acquire and total, and logs present mode/latency as
  separate README fields.
- Full 300-frame captures exist for the matrix in suggestion #2.
- We can explain, with data, whether `render_acquire_ms` is reduced by larger frame latency.
- `tungsten.json` defaults reflect the winning config.
- `docs/perf/profiling-workflow.md` documents the chosen policy (low-latency, balanced,
  throughput) and what each `max_frame_latency` value means in practice.

## Sources

- User-provided background:
  - https://raphlinus.github.io/ui/graphics/gpu/2021/10/22/swapchain-frame-pacing.html
- `wgpu` present-mode docs:
  - https://docs.rs/wgpu-types/latest/wgpu_types/enum.PresentMode.html
- `wgpu` surface-configuration docs:
  - https://doc.servo.org/wgpu_types/struct.SurfaceConfiguration.html
- Vulkan present modes:
  - https://docs.vulkan.org/refpages/latest/refpages/source/VkPresentModeKHR.html
- Vulkan present-wait extension:
  - https://docs.vulkan.org/refpages/latest/refpages/source/VK_KHR_present_wait.html
- DXGI maximum frame latency:
  - https://learn.microsoft.com/en-us/windows/win32/api/dxgi1_3/nf-dxgi1_3-idxgiswapchain2-setmaximumframelatency
- Microsoft guidance on reducing latency with DXGI 1.3 swap chains:
  - https://learn.microsoft.com/fr-fr/windows/uwp/gaming/reduce-latency-with-dxgi-1-3-swap-chains
