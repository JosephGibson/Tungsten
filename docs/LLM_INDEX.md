# LLM Navigation Index

Use this as the first repo map. Keep scope tight.

- Rules and commands: [`AGENTS.md`](../AGENTS.md)
- Architecture: [`DESIGN.md`](../DESIGN.md)
- Rationale: [`DECISIONS.md`](../DECISIONS.md)

| Area | Start Here |
| --- | --- |
| ECS (`World`, entities, components, resources) | [`crates/tungsten-core/src/ecs/`](../crates/tungsten-core/src/ecs/), [`lib.rs`](../crates/tungsten-core/src/lib.rs) |
| Event queue (`EventQueue<T>`, frame flush) | [`crates/tungsten-core/src/ecs/event_queue.rs`](../crates/tungsten-core/src/ecs/event_queue.rs) |
| Asset manifest, registry, IDs | [`crates/tungsten-core/src/assets/manifest.rs`](../crates/tungsten-core/src/assets/manifest.rs), [`registry.rs`](../crates/tungsten-core/src/assets/registry.rs), [`assets/mod.rs`](../crates/tungsten-core/src/assets/mod.rs) |
| App / `winit` loop, smoke frames | [`crates/tungsten/src/app.rs`](../crates/tungsten/src/app.rs), [`lib.rs`](../crates/tungsten/src/lib.rs) |
| Runtime telemetry | [`crates/tungsten/src/telemetry.rs`](../crates/tungsten/src/telemetry.rs), [`crates/tungsten/src/app.rs`](../crates/tungsten/src/app.rs) |
| Load path, GPU upload bridge | [`crates/tungsten/src/asset_loader.rs`](../crates/tungsten/src/asset_loader.rs) |
| Hot reload | [`crates/tungsten/src/hot_reload.rs`](../crates/tungsten/src/hot_reload.rs) |
| `wgpu` renderer, pools, draw, GPU timings | [`crates/tungsten-render/src/lib.rs`](../crates/tungsten-render/src/lib.rs), [`renderer.rs`](../crates/tungsten-render/src/renderer.rs) |
| Tilemaps (core data + umbrella extract) | [`crates/tungsten-core/src/assets/tilemap.rs`](../crates/tungsten-core/src/assets/tilemap.rs), [`crates/tungsten/src/tilemap_extract.rs`](../crates/tungsten/src/tilemap_extract.rs) |
| 2D physics (M11) | [`crates/tungsten-core/src/physics/`](../crates/tungsten-core/src/physics/) |
| Config (`tungsten.json`) | [`crates/tungsten-core/src/config.rs`](../crates/tungsten-core/src/config.rs), [`tungsten.json`](../tungsten.json) at workspace root |
| Examples (by feature) | [`examples/`](../examples/) — `cargo run -p example-NN-name` |
| Perf workflow | [`docs/perf/profiling-workflow.md`](../docs/perf/profiling-workflow.md), [`scripts/perf-capture.sh`](../scripts/perf-capture.sh), [`scripts/test-perf-capture.sh`](../scripts/test-perf-capture.sh) |

Core/render seam and invariants: `AGENTS.md` and `DECISIONS.md` (`D-007`, `D-018`).
