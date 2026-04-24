---
status: done
---

# Test Organization Refactor

## Context

Unit tests currently live in three places:

- Inline `#[cfg(test)] mod tests { ... }` blocks at the bottom of implementation files (most files).
- Sibling `*_tests.rs` files wired with `#[cfg(test)] #[path = "<name>_tests.rs"] mod tests;` (`crates/tungsten/src/app_tests.rs`, `crates/tungsten-core/src/config_tests.rs`, `crates/tungsten-core/src/display_tests.rs`).
- One example uses a split `tests.rs` already (`examples/01_platformer/src/tests.rs`).

Empty scaffolding for the target layout already exists under each crate (`crates/*/src/tests/{assets,ecs,input,physics}/` etc.) and under the two examples, with no `.rs` files populated.

## Goal

Move every existing unit test out of implementation files and out of sibling `*_tests.rs` files into a predictable `src/tests/...` layout that mirrors the source tree. Each implementation file keeps a short child `#[cfg(test)] #[path = "..."] mod tests;` declaration so tests still access private internals.

## Non-goals

- Changing what any test asserts.
- Converting unit tests into integration tests under `crates/*/tests/` unless already a public-API black-box test (none qualify — leave `crates/tungsten-core/tests/` and `crates/tungsten/tests/` alone).
- Touching production code beyond what the move requires (fixing imports, removing now-dead `#[cfg(test)] use ...` lines that become dead imports once relocated).
- Reformatting unrelated code.

## Target layout

Source file → test file:

- `crates/tungsten-core/src/camera.rs` → `crates/tungsten-core/src/tests/camera.rs`
- `crates/tungsten-core/src/config.rs` → `crates/tungsten-core/src/tests/config.rs` (replaces `config_tests.rs`)
- `crates/tungsten-core/src/display.rs` → `crates/tungsten-core/src/tests/display.rs` (replaces `display_tests.rs`)
- `crates/tungsten-core/src/input.rs` → `crates/tungsten-core/src/tests/input.rs`
- Every `crates/tungsten-core/src/{ecs,assets,input,physics}/<name>.rs` → `crates/tungsten-core/src/tests/<parent>/<name>.rs`
- Every `crates/tungsten-render/src/<name>.rs` → `crates/tungsten-render/src/tests/<name>.rs`
- Every `crates/tungsten/src/<name>.rs` → `crates/tungsten/src/tests/<name>.rs` (`app_tests.rs` folds into `tests/app.rs`)
- `examples/01_platformer/src/tests.rs` → `examples/01_platformer/src/tests/systems.rs` (the file covers `systems.rs`)
- `examples/02_sprite_stress/src/main.rs` → `examples/02_sprite_stress/src/tests/main.rs`
- `examples/02_sprite_stress/src/ecs_high_load.rs` → `examples/02_sprite_stress/src/tests/ecs_high_load.rs`

Each implementation file keeps the declaration:

```rust
#[cfg(test)]
#[path = "tests/<relative>.rs"]
mod tests;
```

For nested modules, the `#[path]` is relative to the file's directory (e.g. `physics/step.rs` uses `../tests/physics/step.rs`).

## Files to touch

- Every source file listed in the audit (48 `#[cfg(test)]` call sites).
- Delete: `crates/tungsten/src/app_tests.rs`, `crates/tungsten-core/src/config_tests.rs`, `crates/tungsten-core/src/display_tests.rs`, `examples/01_platformer/src/tests.rs`.
- No `Cargo.toml` changes expected (no tests move across crate boundaries).

## Ordered steps

1. Crate-by-crate: `tungsten-core` → `tungsten-render` → `tungsten` → examples.
2. Within each crate: top-level files first, then nested modules.
3. For each file:
   - Cut the `#[cfg(test)] mod tests { ... }` block (or replace the `#[path]` redirect for sibling files).
   - Write the extracted body to `src/tests/<relative>.rs` unchanged (same imports, same tests).
   - Leave a `#[cfg(test)] #[path = "..."] mod tests;` stub in the source file.
   - If the source file had a `#[cfg(test)] use ...` outside the test module to satisfy the inline tests, move that `use` into the new file.
4. Delete obsolete sibling `*_tests.rs` files.
5. `cargo fmt --all`.
6. `cargo test --workspace`.

## Done-when

- `rg -n '#\[cfg\(test\)\]\s*\nmod tests \{' crates examples` returns nothing.
- No `*_tests.rs` siblings remain (`fd '(^|_)tests\.rs$' crates examples`).
- `cargo test --workspace` passes with the same or greater test count.
- `cargo fmt --all` produces no diff.
- Status in this file flipped to `done`.

## Left inline (planned exceptions)

None. Every `#[cfg(test)] mod tests { ... }` block at the bottom of a
source file was extracted; the only remaining `#[cfg(test)]` annotations
outside `tests/` are on test-only helper items inside production impl
blocks (e.g. `SystemTimingOverlay::ewma_for` in
`crates/tungsten/src/systems_overlay.rs`). Those are not tests and are
intentionally left where they are — moving a test-only accessor into
`src/tests/…` would break pub(crate) visibility and serve no readability
gain.

## Outcome

- 48 `mod tests` sites migrated to the `src/tests/…` layout (1:1 with
  their implementation files).
- Three sibling `*_tests.rs` files folded into the new scheme:
  `crates/tungsten/src/app_tests.rs`,
  `crates/tungsten-core/src/{config,display}_tests.rs`.
- One example sibling `examples/01_platformer/src/tests.rs` moved to
  `examples/01_platformer/src/tests/main.rs` and its `main.rs`
  declaration switched to the explicit `#[path]` form.
- Total `#[test]` attributes before and after: 420 (unchanged).
- `cargo test --workspace`: same 420 passed / 1 ignored.
- `cargo fmt --all -- --check`: clean.
