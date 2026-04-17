---
status: draft
goal: Split `crates/tungsten/src/app.rs` into smaller internal modules to reduce repeated agent context and improve maintainability without changing public behavior.
non-goals: changing the public `App` API, changing frame order, redesigning display or hot-reload behavior, touching example code, or making unrelated engine feature changes
files-to-touch: crates/tungsten/src/app.rs, crates/tungsten/src/app/, crates/tungsten/src/lib.rs
---

# `app.rs` Split Plan

## Why

`crates/tungsten/src/app.rs` currently mixes several responsibilities in one file:

- `App` state and public registration APIs
- startup / resume flow
- `winit` window event handling
- frame execution and telemetry timing
- display request application
- hot-reload processing
- module-local tests

That makes common engine tasks more expensive for coding agents because even small changes often require reopening a nearly `1000`-line file. The goal of this refactor is to keep the behavior and public surface stable while reducing the amount of code any one task needs to load.

## Constraints

- Preserve `pub use app::{App, WindowSize};` from `crates/tungsten/src/lib.rs`.
- Preserve frame order invariant:
  `run systems -> flush commands -> flush events -> hot-reload -> extract -> render`.
- Keep `tungsten` as the crate that bridges core and render.
- Keep tests adjacent to the `app` module, but out of the main implementation file.
- Do not fold example-specific logic into the engine while doing this split.

## Proposed Target Layout

| Path | Responsibility |
| --- | --- |
| `crates/tungsten/src/app/mod.rs` | `App` struct, shared type aliases, public registration methods, top-level helpers re-exported inside the module |
| `crates/tungsten/src/app/startup.rs` | startup-time helpers and `resumed` support code |
| `crates/tungsten/src/app/window_events.rs` | `WindowEvent` dispatch helpers used by the `ApplicationHandler` impl |
| `crates/tungsten/src/app/frame.rs` | redraw/frame execution path, timing capture, input reset, smoke-frame exit logic |
| `crates/tungsten/src/app/hot_reload.rs` | `process_hot_reload` and any small helpers local to reload dispatch |
| `crates/tungsten/src/app/display.rs` | pending display request apply path plus runtime fullscreen helpers |
| `crates/tungsten/src/app/tests.rs` | current module tests, updated to import from the new module layout |

Notes:

- `ApplicationHandler for App` can stay in `mod.rs` if that keeps visibility simple, but the large `RedrawRequested` and `WindowEvent` bodies should delegate immediately into helpers in `frame.rs` and `window_events.rs`.
- If a direct `window_events.rs` split turns out awkward, a smaller first pass with `startup.rs`, `frame.rs`, `display.rs`, `hot_reload.rs`, and `tests.rs` is still acceptable.

## Ordered Steps

1. Convert `crates/tungsten/src/app.rs` into `crates/tungsten/src/app/mod.rs` with no behavior changes.
2. Move the existing `#[cfg(test)] mod tests` block into `crates/tungsten/src/app/tests.rs` and wire it in from `mod.rs`.
3. Extract display-specific helpers and methods:
   `apply_pending_display_request`, `runtime_display_mode`, `apply_window_fullscreen`, and `resolve_startup_display` into `app/display.rs`.
4. Extract hot-reload-specific logic:
   `process_hot_reload` into `app/hot_reload.rs`.
5. Extract the redraw/frame body from `WindowEvent::RedrawRequested` into `app/frame.rs` as one helper first, then optionally split further only if it stays clear.
6. Extract startup / resume initialization from `resumed` into `app/startup.rs`.
7. Extract non-redraw `WindowEvent` arms into `app/window_events.rs` if the remaining `ApplicationHandler` impl in `mod.rs` is still too large.
8. Run `cargo fmt` and `cargo test --workspace`.
9. If engine wiring changed in a meaningful way, run `./scripts/smoke-examples.sh`.

## Implementation Notes

- Prefer moving code in behavior-preserving chunks rather than rewriting while splitting.
- Keep helpers `pub(super)` or private unless cross-module visibility requires otherwise.
- Keep shared imports centralized only if that does not make module files noisy; otherwise let each module import what it uses.
- Avoid introducing a large internal “utils” module. The split should follow runtime responsibilities, not generic helper buckets.
- If borrow-checker pressure appears after extraction, prefer moving small state calculations into local helper structs or returned values rather than collapsing modules back together.

## Validation

- `cargo fmt --all`
- `cargo test --workspace`
- `./scripts/smoke-examples.sh` if the refactor changes app-loop wiring beyond pure file moves

## Done When

- The public `App` API remains unchanged from a consumer perspective.
- `crates/tungsten/src/app/mod.rs` is materially smaller and primarily acts as the module entrypoint, not the whole implementation.
- Display, hot reload, frame execution, and tests each live in their own files.
- Existing `app` tests still pass.
- Workspace tests pass without behavior regressions.
