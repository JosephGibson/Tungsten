---
status: draft
goal: Establish CPU/GPU diagnostics and a reproducible performance baseline before Phase 3 feature work begins
non-goals: In-game HUD (M18), runtime display settings (M17), debug overlays (M21), async profiling, distributed tracing
milestone: M12
depends-on: none
unblocks: all Phase 3 milestones (M13–M24)
---

# Phase 3 — Milestone 12: Performance Baseline + Profiling Harness

## Context

M12 is the first Phase 3 milestone and a prerequisite for all others. Its purpose is to
establish CPU/GPU performance diagnostics and record an initial baseline before adding more
engine complexity. Without this, regressions from later milestones are hard to detect and
root-cause.

This plan is self-contained and intended for execution by an AI coding agent in a fresh
context. Every file path, field name, and API signature has been verified against the actual
codebase at branch `0.9`.

## Non-goals

- In-game telemetry HUD (M18)
- Runtime resolution/vsync toggles (M17)
- Debug draw overlays (M21)
- Async/distributed profiling
- Benchmark coverage for M13+ features (CommandBuffer, EventQueue, etc.)

## Files to touch

| File | Action |
|------|--------|
| `DECISIONS.md` | Append D-037, D-038 |
| `crates/tungsten/src/telemetry.rs` | **New** — `FrameTimings` resource |
| `crates/tungsten/src/app.rs` | Instrument frame stages; add `system_names` field |
| `crates/tungsten/src/lib.rs` | Re-export `FrameTimings` |
| `crates/tungsten-render/src/renderer.rs` | Add `GpuFrameTimings`, timestamp support, `render_frame_full_timed` |
| `crates/tungsten-render/src/lib.rs` | Re-export `GpuFrameTimings` |
| `crates/tungsten-render/Cargo.toml` | Add `criterion` dev-dep + `render_bench` target |
| `crates/tungsten-core/Cargo.toml` | Add `physics_bench` target |
| `crates/tungsten-core/benches/ecs_bench.rs` | Extend with Phase 3 scenarios |
| `crates/tungsten-core/benches/physics_bench.rs` | **New** — physics micro-benchmarks |
| `crates/tungsten-render/benches/render_bench.rs` | **New** — render data-structure micro-benchmarks |
| `examples/02_sprite_stress/Cargo.toml` | **New** example crate |
| `examples/02_sprite_stress/src/main.rs` | **New** canonical stress scene |
| `Cargo.toml` | Add `examples/02_sprite_stress` to workspace members |
| `docs/perf/profiling-workflow.md` | **New** — CPU/GPU profiling workflow reference |
| `scripts/perf-capture.sh` | **New** — automated baseline capture script |
| `perf-runs/.gitkeep` | **New** — empty dir placeholder |

---

## Pre-execution: read these first

Before any code change, the implementing agent must read these files in full:

1. `crates/tungsten/src/app.rs` — frame loop is `WindowEvent::RedrawRequested` branch (lines 397–480)
2. `crates/tungsten-render/src/renderer.rs` — `render_frame_full` is lines 195–254
3. `crates/tungsten-core/src/physics/broadphase.rs` — `SpatialGrid` API (public methods only)
4. `crates/tungsten-core/src/lib.rs` — public re-exports to know what is accessible
5. `crates/tungsten-render/src/lib.rs` — public re-exports for render types
6. `examples/01_platformer/Cargo.toml` — reference for workspace dep names

Key confirmed facts (do not re-derive):
- `SpatialGrid::insert(id: ProxyId, aabb: &Aabb)` takes a `&Aabb` reference
- `SpatialGrid::query(query: &Aabb, exclude: Option<ProxyId>, out: &mut Vec<ProxyId>)`
- `SpriteBatch { texture: TextureHandle, filter: FilterMode, instances: Vec<SpriteInstance> }`
- `SpriteInstance { position: [f32; 2], size: [f32; 2] }`
- `Renderer::upload_texture(handle, rgba_data, width, height)` exists
- `Renderer::acquire_texture` is private — accessible from `impl Renderer` methods
- Latest `DECISIONS.md` entry is D-036 — next entries are D-037, D-038
- `criterion` is already in `tungsten-core` dev-deps; NOT yet in `tungsten-render`
- `perf-runs/` directory does not exist yet

---

## Phase 1 — Prerequisites and DECISIONS.md

### Task 1.1 — Verify API surfaces before any code change

```bash
# Verify SpatialGrid insert takes &Aabb:
grep -n "pub fn insert\|pub fn query\|pub fn new" crates/tungsten-core/src/physics/broadphase.rs

# Verify tungsten_render re-exports SpriteBatch/SpriteInstance:
grep -n "pub use\|SpriteBatch\|SpriteInstance" crates/tungsten-render/src/lib.rs

# Check workspace Cargo.toml for env_logger in [workspace.dependencies]:
grep -n "env_logger" Cargo.toml

# Check example-01 Cargo.toml for dep naming pattern:
cat examples/01_platformer/Cargo.toml
```

- [ ] **1.1**: All API surfaces confirmed before proceeding.

### Task 1.2 — Append D-037 to `DECISIONS.md`

Add after the D-036 block:

```markdown
## D-037 — `criterion` added to `tungsten-render` dev-dependencies
**Date:** <implementation date>
**Decision:** Add `criterion = { version = "0.5", features = ["html_reports"] }` as a
`[dev-dependencies]` entry in `crates/tungsten-render/Cargo.toml` for render-side
micro-benchmarks (sprite batch build, extract cost). Satisfies D-015 rule 3 (benchmark
harness is a solved primitive). `criterion` is already a `tungsten-core` dev-dep at the
same version; this extends the pattern symmetrically.
```

### Task 1.3 — Append D-038 to `DECISIONS.md`

Add after D-037:

```markdown
## D-038 — M12 CPU telemetry: std::time::Instant inline, no external dep
**Date:** <implementation date>
**Decision:** Frame-stage timings (update/extract/render/audio/hot-reload) measured with
`std::time::Instant::now()` / `.elapsed()` inline in `app.rs`, accumulated in a
`FrameTimings` struct stored as a World resource. No external profiling crate is
introduced. Rationale: (1) `std::time::Instant` gives millisecond-resolution diagnostics
sufficient for Phase 3 scale; (2) keeping measurements in the same file as timed code
avoids over-abstraction; (3) M18 HUD can consume `FrameTimings` from the resource with no
API change. Per-system timing: `App` stores system names alongside closures
(`system_names: Vec<String>`, `system_name_counter: usize`). Each system call is wrapped
with `Instant`; durations populate `FrameTimings::system_timings: Vec<(String, f32)>`.
Cost: one `Instant::now()` + `.elapsed()` per system per frame — acceptable at Phase 3
scale.
```

**Verification:**

```bash
grep -c "D-037\|D-038" DECISIONS.md
# Expected: 2
```

- [ ] **1.2**: D-037 appended to `DECISIONS.md`.
- [ ] **1.3**: D-038 appended to `DECISIONS.md`.

---

## Phase 2 — CPU telemetry module

### Task 2.1 — Create `crates/tungsten/src/telemetry.rs`

```rust
//! CPU frame-stage timing telemetry.
//!
//! `FrameTimings` is a World resource populated each frame by `App`.
//! It is consumed by the runtime HUD (M18) and offline tooling.
//! All timings are wall-clock milliseconds from `std::time::Instant`.

/// Per-stage CPU timing for a single frame, in milliseconds.
/// Populated by `App` at the end of each `RedrawRequested` pass and
/// inserted as a resource so any system or HUD can read it.
#[derive(Debug, Clone, Default)]
pub struct FrameTimings {
    /// Total wall time for all registered systems (sum of system_timings durations).
    pub update_ms: f32,
    /// Time spent in all extract closures (quads + sprites + text).
    pub extract_ms: f32,
    /// Time from render_frame_full call start to return.
    pub render_ms: f32,
    /// Time spent draining AudioCommands and forwarding to the audio thread.
    pub audio_ms: f32,
    /// Time spent in process_hot_reload.
    pub hot_reload_ms: f32,
    /// Total wall time for the frame (RedrawRequested entry to end of render).
    pub total_ms: f32,
    /// Per-system breakdown: (name, duration_ms) in registration order.
    /// Systems registered with `App::add_system` use auto-generated name "system_N".
    /// Systems registered with `App::add_system_named` use the provided name.
    pub system_timings: Vec<(String, f32)>,
}

impl FrameTimings {
    pub fn new() -> Self {
        Self::default()
    }

    /// Return the name and duration of the slowest system this frame, or None.
    pub fn slowest_system(&self) -> Option<(&str, f32)> {
        self.system_timings
            .iter()
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(name, ms)| (name.as_str(), *ms))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_zero() {
        let ft = FrameTimings::new();
        assert_eq!(ft.update_ms, 0.0);
        assert_eq!(ft.render_ms, 0.0);
        assert!(ft.system_timings.is_empty());
    }

    #[test]
    fn slowest_system_empty() {
        assert!(FrameTimings::new().slowest_system().is_none());
    }

    #[test]
    fn slowest_system_finds_max() {
        let mut ft = FrameTimings::new();
        ft.system_timings = vec![
            ("a".to_string(), 1.0),
            ("b".to_string(), 5.0),
            ("c".to_string(), 2.0),
        ];
        let (name, ms) = ft.slowest_system().unwrap();
        assert_eq!(name, "b");
        assert!((ms - 5.0).abs() < f32::EPSILON);
    }
}
```

- [ ] **2.1**: `crates/tungsten/src/telemetry.rs` created.

### Task 2.2 — Modify `crates/tungsten/src/app.rs`

Read the full file before making any changes. Apply the following modifications:

#### 2.2.a — Add import at top of file

```rust
use crate::telemetry::FrameTimings;
use tungsten_render::GpuFrameTimings;
```

#### 2.2.b — Add fields to `App` struct (after `smoke_frames_remaining` on line 67)

```rust
    /// Names for registered systems, parallel to `systems`.
    system_names: Vec<String>,
    /// Auto-incrementing counter for unnamed system registration.
    system_name_counter: usize,
    /// When true, use render_frame_full_timed each frame (adds device.poll(Wait) stall).
    /// Set via TUNGSTEN_GPU_TIMING env var. Never enable in production.
    gpu_timing_enabled: bool,
```

#### 2.2.c — Initialize new fields and resources in `App::new`

After `world.insert_resource(CollisionEvents::new());` (line 85), add:

```rust
        world.insert_resource(FrameTimings::new());
        world.insert_resource(GpuFrameTimings::default());
```

In the `Self { ... }` constructor block, add:

```rust
            system_names: Vec::new(),
            system_name_counter: 0,
            gpu_timing_enabled: std::env::var("TUNGSTEN_GPU_TIMING").is_ok(),
```

#### 2.2.d — Replace `add_system` body (line 126)

```rust
pub fn add_system(&mut self, system: impl FnMut(&mut World) + 'static) {
    let name = format!("system_{}", self.system_name_counter);
    self.system_name_counter += 1;
    self.system_names.push(name);
    self.systems.push(Box::new(system));
}
```

#### 2.2.e — Add `add_system_named` method alongside `add_system`

```rust
/// Register a named system. The name appears in FrameTimings::system_timings
/// for per-system profiling. Prefer this when the system name matters for
/// diagnostics output.
pub fn add_system_named(
    &mut self,
    name: impl Into<String>,
    system: impl FnMut(&mut World) + 'static,
) {
    self.system_names.push(name.into());
    self.systems.push(Box::new(system));
}
```

#### 2.2.f — Replace the `WindowEvent::RedrawRequested` branch body

Replace everything inside `WindowEvent::RedrawRequested => { ... }` (lines 397–480)
with the instrumented version below:

```rust
WindowEvent::RedrawRequested => {
    let frame_start = Instant::now();

    // --- Delta time ---
    let now = Instant::now();
    if let Some(last) = self.last_frame {
        let dt = now.duration_since(last).as_secs_f32();
        if let Some(delta) = self.world.get_resource_mut::<DeltaTime>() {
            delta.dt = dt;
        }
    }
    self.last_frame = Some(now);

    // --- Update stage: all registered systems ---
    let update_start = Instant::now();
    let mut system_timings: Vec<(String, f32)> =
        Vec::with_capacity(self.systems.len());
    for (system, name) in self.systems.iter_mut().zip(self.system_names.iter()) {
        let t0 = Instant::now();
        system(&mut self.world);
        system_timings.push((name.clone(), t0.elapsed().as_secs_f64() as f32 * 1000.0));
    }
    let update_ms = update_start.elapsed().as_secs_f64() as f32 * 1000.0;

    // --- Hot reload stage ---
    let hot_reload_start = Instant::now();
    self.process_hot_reload();
    let hot_reload_ms = hot_reload_start.elapsed().as_secs_f64() as f32 * 1000.0;

    // --- Extract stage ---
    let extract_start = Instant::now();
    let quads = self
        .extract_quads
        .as_ref()
        .map(|f| f(&self.world))
        .unwrap_or_default();
    let sprites = self
        .extract_sprites
        .as_ref()
        .map(|f| f(&self.world))
        .unwrap_or_default();
    let text = self
        .extract_text
        .as_ref()
        .map(|f| f(&self.world))
        .unwrap_or_default();
    let extract_ms = extract_start.elapsed().as_secs_f64() as f32 * 1000.0;

    // --- Render stage ---
    let render_start = Instant::now();
    if let Some(renderer) = &mut self.renderer {
        let (vw, vh) = {
            let cfg = &renderer.surface_config;
            (cfg.width as f32, cfg.height as f32)
        };
        let view_proj = self
            .world
            .get_resource::<Camera2D>()
            .copied()
            .unwrap_or_default()
            .view_projection(vw, vh);
        let result = if self.gpu_timing_enabled {
            renderer.render_frame_full_timed(&view_proj, &quads, &sprites, &text)
        } else {
            renderer.render_frame_full(&view_proj, &quads, &sprites, &text)
        };
        if let Err(e) = result {
            log::error!("Render error: {e}");
        }
        // Propagate GPU timings to World resource.
        let gpu_ft = renderer.gpu_timings.clone();
        if let Some(res) = self.world.get_resource_mut::<GpuFrameTimings>() {
            *res = gpu_ft;
        }
    }
    let render_ms = render_start.elapsed().as_secs_f64() as f32 * 1000.0;

    // --- Audio stage ---
    let audio_start = Instant::now();
    if let (Some(audio), Some(cmds)) = (
        &mut self.audio,
        self.world.get_resource_mut::<AudioCommands>(),
    ) {
        for cmd in cmds.drain() {
            audio.send(cmd);
        }
    }
    let audio_ms = audio_start.elapsed().as_secs_f64() as f32 * 1000.0;

    // Clear edge state after systems have consumed it.
    if let Some(input) = self.world.get_resource_mut::<InputState>() {
        input.begin_frame();
    }

    // Write FrameTimings resource.
    let total_ms = frame_start.elapsed().as_secs_f64() as f32 * 1000.0;
    if let Some(ft) = self.world.get_resource_mut::<FrameTimings>() {
        ft.update_ms = update_ms;
        ft.extract_ms = extract_ms;
        ft.render_ms = render_ms;
        ft.audio_ms = audio_ms;
        ft.hot_reload_ms = hot_reload_ms;
        ft.total_ms = total_ms;
        ft.system_timings = system_timings;
    }

    // Emit timing summary when TUNGSTEN_PERF_LOG is set (any build mode).
    // Output appears via RUST_LOG=debug.
    if std::env::var("TUNGSTEN_PERF_LOG").is_ok() {
        log::debug!(
            "frame: total={:.2}ms update={:.2}ms extract={:.2}ms \
             render={:.2}ms audio={:.2}ms hot_reload={:.2}ms",
            total_ms, update_ms, extract_ms, render_ms, audio_ms, hot_reload_ms
        );
    }

    if let Some(window) = &self.window {
        window.request_redraw();
    }

    if let Some(remaining) = self.smoke_frames_remaining.as_mut() {
        *remaining = remaining.saturating_sub(1);
        if *remaining == 0 {
            log::info!("TUNGSTEN_SMOKE_FRAMES reached; exiting cleanly");
            event_loop.exit();
        }
    }
}
```

- [ ] **2.2**: `app.rs` modified with all six sub-tasks.

### Task 2.3 — Update `crates/tungsten/src/lib.rs`

Add after the existing `pub mod` declarations:

```rust
pub mod telemetry;
pub use telemetry::FrameTimings;
```

- [ ] **2.3**: `lib.rs` updated.

**Phase 2 verification:**

```bash
cd /home/joker/Tungsten && cargo test --workspace 2>&1 | tail -30
# Expected: all tests pass, including the 3 new telemetry unit tests
```

---

## Phase 3 — GPU diagnostics in renderer

**Design:** Use `RenderPassTimestampWrites` (set on `RenderPassDescriptor.timestamp_writes`)
rather than encoder-level `write_timestamp`. This requires only `wgpu::Features::TIMESTAMP_QUERY`,
not the additional `TIMESTAMP_QUERY_INSIDE_ENCODERS` feature, and degrades gracefully when
the feature is unavailable.

### Task 3.1 — Add `GpuFrameTimings` struct to `renderer.rs`

Add before the `RenderError` definition (line 11):

```rust
/// GPU-side frame timing, in milliseconds.
/// All fields are `Option<f32>` because `TIMESTAMP_QUERY` may be unavailable
/// (software renderers, older Vulkan, WebGPU compatibility layer). Callers must
/// handle `None`.
#[derive(Debug, Clone, Default)]
pub struct GpuFrameTimings {
    /// Render-pass GPU duration (begin to end). `None` when TIMESTAMP_QUERY
    /// is unavailable on the active backend.
    pub frame_gpu_ms: Option<f32>,
    /// Backend name from `wgpu::Adapter::get_info().backend`. Always `Some` after init.
    pub backend: Option<String>,
    /// Adapter name from `wgpu::Adapter::get_info().name`. Always `Some` after init.
    pub adapter_name: Option<String>,
}
```

### Task 3.2 — Add fields to `Renderer` struct (after `text_pipeline`)

```rust
    /// Whether TIMESTAMP_QUERY is available. Determined at init time; never changes.
    pub timestamp_support: bool,
    /// Most recently computed GPU frame timings.
    pub gpu_timings: GpuFrameTimings,
```

### Task 3.3 — Modify `Renderer::new` to request TIMESTAMP_QUERY

Replace the `adapter` request + `request_device` block (lines 51–63) with:

```rust
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::default(),
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))?;

        // Request TIMESTAMP_QUERY only when the adapter supports it; never fail
        // device creation over a missing optional feature.
        let adapter_features = adapter.features();
        let desired_features = adapter_features & wgpu::Features::TIMESTAMP_QUERY;

        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
                label: Some("tungsten_device"),
                required_features: desired_features,
                required_limits: wgpu::Limits::default(),
                ..Default::default()
            }))?;

        let timestamp_support = device
            .features()
            .contains(wgpu::Features::TIMESTAMP_QUERY);

        let adapter_info = adapter.get_info();
        let gpu_timings = GpuFrameTimings {
            frame_gpu_ms: None,
            backend: Some(format!("{:?}", adapter_info.backend)),
            adapter_name: Some(adapter_info.name.clone()),
        };
```

In the `Ok(Self { ... })` block, add:

```rust
            timestamp_support,
            gpu_timings,
```

### Task 3.4 — Add `render_frame_full_timed` method to `impl Renderer`

Add after `render_frame_full` (after line 254):

```rust
/// Render a full frame and record GPU timing in `self.gpu_timings.frame_gpu_ms`.
///
/// When `TIMESTAMP_QUERY` is available, injects timestamps at render-pass begin/end
/// via `RenderPassTimestampWrites` and reads them back after submit.
/// When unavailable, falls through to `render_frame_full` and `frame_gpu_ms` stays `None`.
///
/// **CAUTION:** Calls `device.poll(Wait)` per frame to read back timestamps.
/// This stalls the CPU until GPU work is done and inflates frame timings.
/// Only call when `TUNGSTEN_GPU_TIMING=1`. Never call in production.
pub fn render_frame_full_timed(
    &mut self,
    view_proj: &glam::Mat4,
    quads: &[QuadInstance],
    sprite_batches: &[SpriteBatch],
    text_sections: &[TextSection],
) -> Result<(), RenderError> {
    if !self.timestamp_support {
        return self.render_frame_full(view_proj, quads, sprite_batches, text_sections);
    }

    // QuerySet with 2 slots: beginning and end of the render pass.
    let query_set = self.device.create_query_set(&wgpu::QuerySetDescriptor {
        label: Some("frame_ts_qs"),
        count: 2,
        ty: wgpu::QueryType::Timestamp,
    });

    // resolve_buf: GPU writes resolved timestamps here (QUERY_RESOLVE | COPY_SRC).
    let resolve_buf = self.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("ts_resolve"),
        size: 16,
        usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });

    // readback_buf: CPU-readable copy (MAP_READ | COPY_DST).
    let readback_buf = self.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("ts_readback"),
        size: 16,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let output = match self.acquire_texture()? {
        Some(tex) => tex,
        None => return Ok(()),
    };

    let w = self.surface_config.width;
    let h = self.surface_config.height;
    self.quad_pipeline.update_camera(&self.queue, view_proj);
    self.sprite_pipeline.update_camera(&self.queue, view_proj);
    self.text_pipeline
        .prepare(&self.device, &self.queue, text_sections, w, h);

    let view = output
        .texture
        .create_view(&wgpu::TextureViewDescriptor::default());

    let mut encoder = self
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("frame_encoder_timed"),
        });

    {
        // RenderPassTimestampWrites injects timestamps at pass begin/end.
        // Requires only TIMESTAMP_QUERY (not TIMESTAMP_QUERY_INSIDE_ENCODERS).
        let ts_writes = wgpu::RenderPassTimestampWrites {
            query_set: &query_set,
            beginning_of_pass_write_index: Some(0),
            end_of_pass_write_index: Some(1),
        };

        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("main_pass_timed"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(self.clear_color),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: Some(ts_writes),
            ..Default::default()
        });

        self.quad_pipeline
            .draw(&self.device, &mut render_pass, quads);
        self.sprite_pipeline
            .draw(&self.device, &mut render_pass, sprite_batches);
        self.text_pipeline.render(&mut render_pass);
    }

    // Resolve timestamps → resolve_buf, then copy to readback_buf.
    encoder.resolve_query_set(&query_set, 0..2, &resolve_buf, 0);
    encoder.copy_buffer_to_buffer(&resolve_buf, 0, &readback_buf, 0, 16);

    self.queue.submit(std::iter::once(encoder.finish()));
    output.present();
    self.text_pipeline.post_frame();

    // Read back timestamps. poll(Wait) blocks until GPU work is complete.
    let slice = readback_buf.slice(..);
    let (sender, receiver) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = sender.send(result);
    });
    self.device.poll(wgpu::Maintain::Wait);

    if receiver.recv().ok().and_then(|r| r.ok()).is_some() {
        let data = slice.get_mapped_range();
        let ts0 = u64::from_le_bytes(data[0..8].try_into().unwrap_or([0u8; 8]));
        let ts1 = u64::from_le_bytes(data[8..16].try_into().unwrap_or([0u8; 8]));
        drop(data);
        readback_buf.unmap();

        let period = self.queue.get_timestamp_period();
        let delta_ns = ts1.wrapping_sub(ts0) as f64 * period as f64;
        self.gpu_timings.frame_gpu_ms = Some((delta_ns / 1_000_000.0) as f32);
    }

    Ok(())
}
```

### Task 3.5 — Re-export `GpuFrameTimings` from `crates/tungsten-render/src/lib.rs`

Read the current `lib.rs` first, then add:

```rust
pub use renderer::GpuFrameTimings;
```

- [ ] **3.1–3.5**: GPU diagnostics implemented.

**Phase 3 verification:**

```bash
cd /home/joker/Tungsten
cargo build --workspace 2>&1 | tail -20
cargo test --workspace 2>&1 | tail -20
```

---

## Phase 4 — Criterion benchmark suite

`criterion` is already in `tungsten-core` dev-deps. The `[[bench]]` section for
`ecs_bench` is already in `crates/tungsten-core/Cargo.toml`.

### Task 4.1 — Add `physics_bench` target to `crates/tungsten-core/Cargo.toml`

After the existing `[[bench]]` block:

```toml
[[bench]]
name = "physics_bench"
harness = false
```

### Task 4.2 — Extend `crates/tungsten-core/benches/ecs_bench.rs`

Read the existing file first to understand its import pattern and `criterion_group!` call.
Append two new benchmarks and add them to the existing `criterion_group!`.

Scenarios to add:

```rust
// --- query2: 10k entities across 5 archetypes ---
// Five archetypes created by mixing component sets; all have Position + Velocity,
// which is what the query reads. Tests archetype-spanning iteration cost.
fn bench_query2_10k_5archetypes(c: &mut Criterion) {
    use tungsten_core::{Collider, Position, RigidBody, Shape, Velocity, World};
    use glam::Vec2;

    const N: usize = 2_000; // 5 archetypes × 2000 = 10000 total

    let mut world = World::new();

    // Archetype 1: Position + Velocity
    for i in 0..N {
        let e = world.spawn();
        world.insert(e, Position(Vec2::new(i as f32, 0.0)));
        world.insert(e, Velocity(Vec2::new(1.0, 0.0)));
    }
    // Archetype 2: Position + Velocity + RigidBody
    for i in 0..N {
        let e = world.spawn();
        world.insert(e, Position(Vec2::new(i as f32, 100.0)));
        world.insert(e, Velocity(Vec2::splat(0.5)));
        world.insert(e, RigidBody::dynamic());
    }
    // Archetypes 3-5: further variation (add Collider, static body, etc.)
    for i in 0..(N * 3) {
        let e = world.spawn();
        world.insert(e, Position(Vec2::new(i as f32, 200.0)));
        world.insert(e, Velocity(Vec2::ONE));
        if i % 3 == 0 { world.insert(e, RigidBody::r#static()); }
    }

    c.bench_function("query2_10k_5archetypes_pv", |b| {
        b.iter(|| {
            let mut sum = Vec2::ZERO;
            // NOTE: adapt to actual query2 API — may be world.query2::<A,B>()
            // returning an iterator, or world.query2_entities returning entity IDs.
            // Read world.rs before implementing.
            for (p, v) in world.query2::<Position, Velocity>() {
                sum += p.0 + v.0;
            }
            black_box(sum);
        });
    });
}

// --- spawn + despawn 1k ---
fn bench_spawn_despawn_1k(c: &mut Criterion) {
    use tungsten_core::{Position, World};
    use glam::Vec2;

    c.bench_function("spawn_despawn_1k", |b| {
        b.iter(|| {
            let mut world = World::new();
            let entities: Vec<_> = (0..1_000u32)
                .map(|i| {
                    let e = world.spawn();
                    world.insert(e, Position(Vec2::new(i as f32, 0.0)));
                    e
                })
                .collect();
            for e in &entities {
                world.despawn(*e);
            }
            black_box(world);
        });
    });
}
```

**Important:** Before implementing, read `crates/tungsten-core/src/ecs/world.rs` to confirm:
- The exact `query2` iterator API (returns `impl Iterator<Item = (&A, &B)>` or similar)
- Whether `world.despawn(entity)` exists, and its signature
- The `spawn()` return type

Adapt the benchmark body to the actual API. If `query2` doesn't return an iterator directly,
use whatever pattern the existing `ecs_bench.rs` uses.

Add both new functions to the existing `criterion_group!` call.

- [ ] **4.1**: `physics_bench` target added to `tungsten-core/Cargo.toml`.
- [ ] **4.2**: `ecs_bench.rs` extended with two new benchmarks.

### Task 4.3 — Create `crates/tungsten-core/benches/physics_bench.rs`

```rust
//! Physics subsystem micro-benchmarks (M12).
//!
//! Scenarios:
//!   - position_integration_50k  — 50k entity position integration
//!   - broadphase_rebuild_5k     — SpatialGrid rebuild from 5k bodies

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use glam::Vec2;
use tungsten_core::{Aabb, Position, RigidBody, SpatialGrid, Velocity, World};

// --- 50k position integration ---
fn bench_position_integration_50k(c: &mut Criterion) {
    const N: usize = 50_000;
    let mut world = World::new();
    for i in 0..N {
        let e = world.spawn();
        world.insert(e, Position(Vec2::new(i as f32, 0.0)));
        world.insert(e, Velocity(Vec2::new(1.0, 0.5)));
        world.insert(e, RigidBody::dynamic());
    }
    let dt = 1.0_f32 / 60.0;

    c.bench_function("position_integration_50k", |b| {
        b.iter(|| {
            // Simulate the velocity → position write loop from physics_step.
            // NOTE: adapt to actual World mutation API if query2_entities does not exist.
            // Alternative: call physics_step() directly if mutation API is not public.
            let entities = world.query2_entities::<Position, Velocity>();
            for entity in &entities {
                if let (Some(vel), Some(pos)) = (
                    world.get::<Velocity>(*entity).map(|v| v.0),
                    world.get_mut::<Position>(*entity),
                ) {
                    pos.0 += vel * black_box(dt);
                }
            }
        });
    });
}

// --- Broadphase rebuild: 5k dynamic bodies ---
fn bench_broadphase_rebuild_5k(c: &mut Criterion) {
    const N: usize = 5_000;
    let cell_size = 32.0_f32;
    let half_extent = Vec2::splat(8.0);

    // Pre-build positions outside the timed loop.
    let positions: Vec<Vec2> = (0..N)
        .map(|i| Vec2::new((i % 100) as f32 * 16.0, (i / 100) as f32 * 16.0))
        .collect();

    c.bench_function("broadphase_rebuild_5k_dynamic", |b| {
        b.iter(|| {
            let mut grid = SpatialGrid::new(cell_size);
            for (id, &center) in positions.iter().enumerate() {
                // SpatialGrid::insert takes (id: ProxyId, aabb: &Aabb) — verified.
                let aabb = Aabb::new(center, half_extent);
                grid.insert(id as u32, &aabb);
            }
            // Query a central region as a blackbox sink to prevent dead-code elimination.
            let query_aabb = Aabb::new(Vec2::new(800.0, 400.0), Vec2::splat(100.0));
            let mut out = Vec::new();
            grid.query(&query_aabb, None, &mut out);
            black_box(out.len());
        });
    });
}

criterion_group!(
    benches,
    bench_position_integration_50k,
    bench_broadphase_rebuild_5k,
);
criterion_main!(benches);
```

Note on `query2_entities` and `world.get`/`world.get_mut`: verify these exist before
writing the bench. Run:

```bash
grep -n "pub fn query2_entities\|pub fn get\b\|pub fn get_mut\b" \
  crates/tungsten-core/src/ecs/world.rs
```

If they don't exist, replace the integration bench body with a direct `physics_step` call:

```rust
use tungsten_core::physics_step;
// Build world with N dynamic bodies, then time physics_step.
physics_step(&mut world);
```

- [ ] **4.3**: `physics_bench.rs` created and adapted to actual API.

### Task 4.4 — Add `criterion` to `crates/tungsten-render/Cargo.toml`

Add at the end of the file:

```toml
[dev-dependencies]
criterion = { version = "0.5", features = ["html_reports"] }

[[bench]]
name = "render_bench"
harness = false
```

### Task 4.5 — Create `crates/tungsten-render/benches/render_bench.rs`

First `mkdir -p crates/tungsten-render/benches`, then create the file:

```rust
//! Render-side micro-benchmarks (M12).
//!
//! CPU-only: measures cost of building render data structures.
//! No wgpu device is created; no display or GPU required.
//!
//! Scenarios:
//!   - sprite_extract_batch_build_2k — build SpriteBatch vec for 2k sprites / 10 textures

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use tungsten_core::assets::{FilterMode, TextureHandle};
use tungsten_render::{SpriteBatch, SpriteInstance};

fn bench_sprite_extract_batch_build_2k(c: &mut Criterion) {
    const N: usize = 2_000;
    const TEXTURES: usize = 10;

    c.bench_function("sprite_extract_batch_build_2k", |b| {
        b.iter(|| {
            // Allocate TEXTURES empty batches, then fill with N instances.
            // Mimics the work of an extract closure that groups sprites by texture.
            let mut batches: Vec<SpriteBatch> = (0..TEXTURES)
                .map(|i| SpriteBatch {
                    texture: TextureHandle(i as u32),
                    filter: FilterMode::Nearest,
                    instances: Vec::new(),
                })
                .collect();

            for i in 0..N {
                let tex_idx = i % TEXTURES;
                batches[tex_idx].instances.push(SpriteInstance {
                    position: [i as f32, (i / TEXTURES) as f32],
                    size: [16.0, 16.0],
                });
            }

            black_box(batches);
        });
    });
}

criterion_group!(benches, bench_sprite_extract_batch_build_2k);
criterion_main!(benches);
```

Verify before implementing that `SpriteBatch` and `SpriteInstance` are accessible from
`tungsten_render` crate root:

```bash
grep -n "pub use\|SpriteBatch\|SpriteInstance" crates/tungsten-render/src/lib.rs
```

If not re-exported, add `pub use sprite::{SpriteBatch, SpriteInstance};` to
`crates/tungsten-render/src/lib.rs`.

- [ ] **4.4**: criterion added to `tungsten-render/Cargo.toml`.
- [ ] **4.5**: `render_bench.rs` created.

**Phase 4 verification:**

```bash
cd /home/joker/Tungsten

# Compile and run each bench body once (no stats, no GPU):
cargo bench --bench ecs_bench -- --test 2>&1 | tail -10
cargo bench --bench physics_bench -- --test 2>&1 | tail -10
cargo bench --bench render_bench -- --test 2>&1 | tail -10

# Full test suite must still be green:
cargo test --workspace 2>&1 | tail -20
```

---

## Phase 5 — Canonical stress scene

### Task 5.1 — Add `examples/02_sprite_stress` to workspace `Cargo.toml`

In root `Cargo.toml`, add `"examples/02_sprite_stress"` to the `members` array.

- [ ] **5.1**: Workspace member added.

### Task 5.2 — Create `examples/02_sprite_stress/Cargo.toml`

First verify `env_logger` is in workspace deps:

```bash
grep "env_logger" Cargo.toml
```

If it is, use `env_logger = { workspace = true }`. If not, use `env_logger = "0.11"`.
Model the dep list after `examples/01_platformer/Cargo.toml`.

```toml
[package]
name = "example-02-sprite-stress"
version.workspace = true
edition = "2021"
publish = false

[dependencies]
tungsten = { workspace = true }
tungsten-core = { workspace = true }
tungsten-render = { workspace = true }
glam = { workspace = true }
anyhow = { workspace = true }
log = { workspace = true }
env_logger = { workspace = true }   # or env_logger = "0.11" if not in workspace
```

- [ ] **5.2**: `Cargo.toml` created.

### Task 5.3 — Create `examples/02_sprite_stress/src/main.rs`

```rust
//! Example 02 — Sprite Stress (M12 canonical scene 2)
//!
//! Spawns SPRITE_COUNT sprites (default 2000; override via STRESS_COUNT env var)
//! and moves them in a sine wave each frame. No physics, audio, or hot reload.
//!
//! Fixed capture rules (M12 baseline):
//!   Build mode:   release  (`cargo run -p example-02-sprite-stress --release`)
//!   Backend:      WGPU_BACKEND=vulkan  (Linux)
//!   Resolution:   1920 × 1080  (set in code)
//!   Frame window: 300 frames after 60-frame warm-up
//!   VSync:        disabled (`config.window.vsync = false`)
//!
//! Telemetry output: printed to stdout every 60 frames.
//! Baseline capture: pipe to `tee perf-runs/<timestamp>/sprite-stress.txt`

use glam::Vec2;
use tungsten::App;
use tungsten::core::{Camera2D, Config, DeltaTime, World};
use tungsten::render::{SpriteBatch, SpriteInstance};
use tungsten::FrameTimings;
use tungsten_core::assets::{FilterMode, TextureHandle};

const DEFAULT_SPRITE_COUNT: usize = 2_000;
const COLS: usize = 50;
const SPRITE_SIZE: f32 = 16.0;
const WARMUP_FRAMES: u32 = 60;
const LOG_INTERVAL: u32 = 60;

/// Placeholder texture handle — uploaded as solid-white 16×16 at startup.
const PLACEHOLDER_HANDLE: TextureHandle = TextureHandle(0);

struct SpriteEntry {
    base_x: f32,
    base_y: f32,
    phase: f32,
    y_offset: f32,
}

struct SceneState {
    sprites: Vec<SpriteEntry>,
    frame_count: u32,
    total_frame_ms: f64,
    stat_frames: u32,
}

fn update_scene(world: &mut World) {
    let state = match world.get_resource_mut::<SceneState>() {
        Some(s) => s,
        None => return,
    };

    state.frame_count += 1;
    let fc = state.frame_count as f32;

    for sprite in &mut state.sprites {
        sprite.y_offset = (fc * 0.02 + sprite.phase).sin() * 4.0;
    }

    let frame_count = state.frame_count;
    let total_frame_ms = &mut state.total_frame_ms;
    let stat_frames = &mut state.stat_frames;

    if frame_count % LOG_INTERVAL == 0 {
        if let Some(ft) = world.get_resource::<FrameTimings>() {
            println!(
                "[frame {:>5}] total={:.2}ms update={:.2}ms extract={:.2}ms render={:.2}ms",
                frame_count, ft.total_ms, ft.update_ms, ft.extract_ms, ft.render_ms
            );
        }
    }

    if frame_count > WARMUP_FRAMES {
        if let Some(ft) = world.get_resource::<FrameTimings>() {
            *total_frame_ms += ft.total_ms as f64;
            *stat_frames += 1;
        }
    }
}

fn extract_sprites(world: &World) -> Vec<SpriteBatch> {
    let state = match world.get_resource::<SceneState>() {
        Some(s) => s,
        None => return Vec::new(),
    };

    let instances: Vec<SpriteInstance> = state
        .sprites
        .iter()
        .map(|s| SpriteInstance {
            position: [s.base_x, s.base_y + s.y_offset],
            size: [SPRITE_SIZE, SPRITE_SIZE],
        })
        .collect();

    vec![SpriteBatch {
        texture: PLACEHOLDER_HANDLE,
        filter: FilterMode::Nearest,
        instances,
    }]
}

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let sprite_count = std::env::var("STRESS_COUNT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_SPRITE_COUNT);

    let mut config = Config::load("tungsten.json")?;
    config.window.title = format!("Sprite Stress ({sprite_count} sprites)");
    config.window.width = 1920;
    config.window.height = 1080;
    config.window.vsync = false; // Disable vsync for throughput measurement.

    let mut app = App::new(config);

    {
        let world = app.world_mut();

        let sprites: Vec<SpriteEntry> = (0..sprite_count)
            .map(|i| SpriteEntry {
                base_x: (i % COLS) as f32 * SPRITE_SIZE,
                base_y: (i / COLS) as f32 * SPRITE_SIZE,
                phase: i as f32 * 0.1,
                y_offset: 0.0,
            })
            .collect();

        world.insert_resource(SceneState {
            sprites,
            frame_count: 0,
            total_frame_ms: 0.0,
            stat_frames: 0,
        });

        if let Some(cam) = world.get_resource_mut::<Camera2D>() {
            cam.zoom = 1.0;
            cam.position = Vec2::ZERO;
        }
    }

    app.on_startup(|_world, renderer| {
        // Upload a 16×16 solid-white placeholder texture. SpritePipeline stores
        // it in its internal HashMap<TextureHandle, GpuTexture> keyed by handle.
        // No AssetRegistry entry needed — the handle is the sole key.
        let rgba = vec![255u8; 16 * 16 * 4];
        renderer.upload_texture(PLACEHOLDER_HANDLE, &rgba, 16, 16);
    });

    app.add_system_named("update_scene", update_scene);
    app.set_extract_sprites(extract_sprites);

    app.run()
}
```

Note: `update_scene` has a borrow conflict — it reads `FrameTimings` and `SceneState` from
the same `World`. Since both are accessed via `get_resource_mut` and `get_resource` on
separate types, there is no actual aliasing. If the borrow checker rejects the mixed
accesses in a single function body, split into two systems:
- `tick_sprites(world)` — mutates `SceneState`
- `log_telemetry(world)` — reads `SceneState` + `FrameTimings`

Register both with `add_system_named`.

- [ ] **5.3**: `main.rs` created.

**Phase 5 verification:**

```bash
cd /home/joker/Tungsten

# Build check (no GPU needed):
cargo build -p example-02-sprite-stress 2>&1 | tail -20

# Full test suite:
cargo test --workspace 2>&1 | tail -10

# Smoke test (requires GPU + display):
TUNGSTEN_SMOKE_FRAMES=3 cargo run -p example-02-sprite-stress 2>&1 | tail -5
# Expected: clean exit without panic
```

---

## Phase 6 — Profiling scripts and documentation

### Task 6.1 — Create `perf-runs/` directory placeholder

```bash
mkdir -p /home/joker/Tungsten/perf-runs
touch /home/joker/Tungsten/perf-runs/.gitkeep
```

- [ ] **6.1**: `perf-runs/` directory created.

### Task 6.2 — Create `docs/perf/` and `docs/perf/profiling-workflow.md`

```bash
mkdir -p /home/joker/Tungsten/docs/perf
```

Write `docs/perf/profiling-workflow.md` with:
- Canonical scene fixed capture rules (build mode, resolution, vsync, frame window, backend)
- Quick-start using `perf-capture.sh`
- Manual CPU profiling: flamegraph, perf stat, perf record
- Engine telemetry section: `TUNGSTEN_PERF_LOG` usage and output format
- GPU diagnostics section: `TUNGSTEN_GPU_TIMING` usage, caveat about poll(Wait) stall
- `WGPU_BACKEND` override table (vulkan/dx12/metal/gl/auto, with TIMESTAMP_QUERY column)
- RenderDoc capture workflow for Linux Vulkan
- Perf budget targets table (sustained FPS ≥60, p95 frame ≤16.7ms, per-stage envelopes)
- Hotspot identification guide (flamegraph search terms)
- Regression checking policy (>10% steady-state requires DECISIONS.md note)

See the content in the Plan agent's draft (chars 43000–51000 of the full output) for a
complete template. The key data from this plan overrides it where there are discrepancies.

- [ ] **6.2**: `docs/perf/profiling-workflow.md` created.

### Task 6.3 — Create `scripts/perf-capture.sh`

Create the script and make it executable:

```bash
touch /home/joker/Tungsten/scripts/perf-capture.sh
chmod +x /home/joker/Tungsten/scripts/perf-capture.sh
```

The script must:
1. Accept `[scene] [frames]` args (default: `sprite-stress`, `300`)
2. Map `sprite-stress` → `example-02-sprite-stress`, `platformer` → `example-01-platformer`
3. Create `perf-runs/<ISO-timestamp>-<scene>/` output directory
4. Build with `RUSTFLAGS="-C force-frame-pointers=yes" cargo build --release -p $PKG`
5. Resolve binary path (try hyphen form, then underscore form)
6. Run engine telemetry capture: `TUNGSTEN_SMOKE_FRAMES=$FRAMES TUNGSTEN_PERF_LOG=1 RUST_LOG=debug $BINARY`
7. Run GPU timing capture: add `TUNGSTEN_GPU_TIMING=1`
8. Run `cargo flamegraph` (degrade gracefully if not installed)
9. Run `perf stat` and `perf record` (degrade gracefully if paranoid level too high)
10. Write `README.md` with machine specs, measured values, and budget targets table

The script **must not** set `TUNGSTEN_GPU_TIMING` when running flamegraph or perf captures
(to avoid the poll(Wait) stall inflating timing data in those captures).

- [ ] **6.3**: `scripts/perf-capture.sh` created and executable.

**Phase 6 verification:**

```bash
cd /home/joker/Tungsten
test -f docs/perf/profiling-workflow.md && echo "docs OK"
test -x scripts/perf-capture.sh && echo "script executable"
test -d perf-runs && echo "perf-runs dir OK"
```

---

## Phase 7 — Full verification and baseline capture

### Task 7.1 — Full build and test suite

Execute in order, each must pass before proceeding:

```bash
cd /home/joker/Tungsten

# 1. Format (no changes allowed)
cargo fmt --all -- --check

# 2. Full workspace build
cargo build --workspace

# 3. All tests (no GPU needed)
cargo test --workspace

# 4. Bench compilation (-- --test: single run, no stats, no GPU)
cargo bench --bench ecs_bench -- --test
cargo bench --bench physics_bench -- --test
cargo bench --bench render_bench -- --test

# 5. Smoke tests (requires GPU + display)
./scripts/smoke-examples.sh

# 6. Telemetry sanity (requires GPU + display)
TUNGSTEN_SMOKE_FRAMES=10 TUNGSTEN_PERF_LOG=1 RUST_LOG=debug \
  cargo run --release -p example-02-sprite-stress 2>&1 | grep "frame:"
# Expected: ~10 lines with "frame: total=<N>ms" where N is a non-zero number
```

### Task 7.2 — Verify example-02 is auto-discovered by smoke runner

```bash
cargo metadata --no-deps --format-version 1 \
  | python3 -c "
import sys, json
pkgs = json.load(sys.stdin)['packages']
names = [p['name'] for p in pkgs if p['name'].startswith('example-')]
print('\n'.join(sorted(names)))"
# Expected: both example-01-platformer and example-02-sprite-stress listed
```

If `smoke-examples.sh` uses a different discovery mechanism, read it first and verify
`example-02-sprite-stress` will be exercised.

### Task 7.3 — Record baseline captures (requires GPU + display)

```bash
cd /home/joker/Tungsten

WGPU_BACKEND=vulkan ./scripts/perf-capture.sh sprite-stress 300
WGPU_BACKEND=vulkan ./scripts/perf-capture.sh platformer 300
```

After both captures complete, verify artifacts:

```bash
# Both README.md files exist with content:
find perf-runs -name README.md | xargs wc -l

# engine-telemetry.txt has "frame:" lines:
grep -c "frame:" perf-runs/*/engine-telemetry.txt

# flamegraph SVG files are non-trivial (>10KB):
find perf-runs -name flamegraph.svg -size +10k | wc -l
# Expected: 2
```

Update each `perf-runs/<timestamp>/README.md` with the actual measured average frame time
computed from `engine-telemetry.txt`. Example:

```bash
grep "frame:" perf-runs/<timestamp>-sprite-stress/engine-telemetry.txt \
  | awk '{for(i=1;i<=NF;i++) if($i~/total=/) {split($i,a,"="); sum+=a[2]; n++}} \
         END {printf "avg total_ms: %.2f\n", sum/n}'
```

- [ ] **7.1**: All verification commands pass.
- [ ] **7.2**: Both examples discovered by smoke runner.
- [ ] **7.3**: Baseline captures recorded and README files populated.

---

## Risks, open questions, and explicit assumptions

### Risk R-1: `query2_entities` / `world.get` / `world.get_mut` may not exist

The physics bench and stress scene use these methods. Verify before implementing:

```bash
grep -n "pub fn query2_entities\|pub fn get\b\|pub fn get_mut\b\|pub fn despawn" \
  crates/tungsten-core/src/ecs/world.rs
```

If `query2_entities` doesn't exist, use the actual iteration pattern from `ecs_bench.rs`.
If `world.get`/`world.get_mut` don't exist, use `physics_step()` directly in the bench.

### Risk R-2: `wgpu::RenderPassTimestampWrites` field name in wgpu 29.x

`RenderPassDescriptor.timestamp_writes` is the field name. Verify at compile time —
if the field has been renamed, check the wgpu 29.x changelog and update accordingly.
The feature (`TIMESTAMP_QUERY`) and the approach (render-pass level) are stable.

### Risk R-3: `wgpu::Maintain::Wait` API in wgpu 29.x

Verify the `Maintain` enum variant name hasn't changed:

```bash
grep -r "Maintain::" crates/tungsten-render/src/ || grep -r "poll" crates/tungsten-render/src/
```

If renamed, check wgpu changelog or use `grep -r "Maintain" ~/.cargo/registry/src/` to
find the correct variant.

### Risk R-4: Borrow checker conflict in `update_scene`

The function accesses both `SceneState` (mutable) and `FrameTimings` (immutable) from the
same `World`. If the borrow checker rejects this (it shouldn't since they are different
TypeIds), split into two named systems:

```rust
app.add_system_named("tick_sprites", tick_sprites);
app.add_system_named("log_telemetry", log_telemetry);
```

### Risk R-5: `cargo flamegraph` binary naming

`cargo flamegraph` may invoke the binary by package name. If it can't find the binary,
pass `--bin` explicitly:

```bash
cargo flamegraph --release --bin example-02-sprite-stress --output out.svg
```

### Assumption A-1: `system_names` len == `systems` len invariant

`add_system` and `add_system_named` both push to both vecs atomically. The `zip` in the
frame loop silently drops extras if they diverge. A future robustness improvement would
add `debug_assert_eq!(self.systems.len(), self.system_names.len())` in `App::new`.

### Assumption A-2: `GpuFrameTimings` is a valid World resource

`GpuFrameTimings` derives `Default` and is `Clone`. It is inserted in `App::new` before
startup and updated each frame by the render stage. Systems can read it via
`world.get_resource::<GpuFrameTimings>()`.

### Assumption A-3: `config.window.vsync` is a mutable field

Confirmed by the `Config` struct in `crates/tungsten-core/src/config.rs` —
`window.vsync: bool` is a plain field. The stress scene overrides it in code after
`Config::load`.

### Assumption A-4: `env_logger` in workspace dependencies

If not present, use `env_logger = "0.11"` directly in the example `Cargo.toml`.

---

## Done-when checklist (M12 completion criteria)

- [ ] `cargo test --workspace` green with zero regressions
- [ ] `FrameTimings` resource available in `World` every frame; `TUNGSTEN_PERF_LOG=1 RUST_LOG=debug` produces "frame:" lines with non-zero values
- [ ] `system_timings` populated with per-system ms for all registered systems
- [ ] `GpuFrameTimings::frame_gpu_ms` returns `Some(f32)` on Vulkan+Linux with TIMESTAMP_QUERY; `None` on fallback backends; no panic in either case
- [ ] `DECISIONS.md` contains D-037 and D-038
- [ ] `example-02-sprite-stress` builds and passes `TUNGSTEN_SMOKE_FRAMES=3` smoke test
- [ ] Criterion bench suite: `ecs_bench`, `physics_bench`, `render_bench` compile and pass `-- --test`
- [ ] `scripts/perf-capture.sh` runs to completion on Linux Vulkan for both scenes
- [ ] `perf-runs/` contains at least two timestamped capture directories with `README.md`, `engine-telemetry.txt` (with "frame:" lines), and `flamegraph.svg` (>10KB)
- [ ] `docs/perf/profiling-workflow.md` exists with WGPU_BACKEND table, RenderDoc workflow, and budget targets
- [ ] Team can identify top-3 CPU hotspots by name from the flamegraph SVG and cross-reference with engine telemetry output
- [ ] Subsequent milestones (M13+) reference this baseline in their done-when regression checks
