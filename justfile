# Tungsten task runner (https://just.systems). Recipes wrap the raw commands
# listed in AGENTS.md; those commands stay valid without just. Arguments are
# passed as positional shell arguments ("$@"), never spliced into the script.

set shell := ["bash", "-uc"]
set positional-arguments := true

# List recipes.
default:
    @just --list

# Format every crate.
fmt:
    cargo fmt --all

# Check formatting without writing.
fmt-check:
    cargo fmt --all -- --check

# Strict clippy over every target.
lint:
    cargo clippy --workspace --all-targets --locked -- -D warnings

# Workspace tests, no GPU needed. Extra arguments go to cargo test.
test *args:
    cargo test --workspace --locked "$@"

# CPU gate: format check, strict clippy, then every test including doctests.
check: fmt-check lint
    cargo test --workspace --locked -q

# Compile every benchmark without running it.
bench-build:
    cargo bench --workspace --no-run --locked

# GPU smoke run of every example plus the render fixture matrices.
smoke:
    ./scripts/smoke-examples.sh

# Pixel comparison against the reference PNG (reference machine only).
visual:
    TUNGSTEN_VISUAL_REGRESSION=1 cargo test -p example-02-sprite-stress --test visual_regression --locked -- --nocapture

# Perf capture; arguments go to scripts/perf-capture.sh, e.g. `just perf ecs-high-load 300 --telemetry-only`.
perf *args:
    ./scripts/perf-capture.sh "$@"

# Perf-capture parser and helper regression test.
perf-test:
    bash scripts/test-perf-capture.sh

# Shell lint plus the stubbed smoke-script and perf-helper tests (no GPU).
script-test: perf-test
    shellcheck scripts/*.sh
    bash scripts/test-smoke-examples.sh
    python3 -B scripts/test-check-repo.py

# Dependency policy: advisories, licenses, bans, sources.
deps:
    cargo deny --locked check

# Agent instruction budgets, links, skill symlinks and repo-byte totals.
ctx:
    python3 scripts/check-agent-context.py
    python3 scripts/check-agent-context.py --self-test

# File coverage, docs, active plans, and existing Rust manifest/index validation.
repo-check:
    python3 -B scripts/check-repo.py
    cargo test -p tungsten-core --test manifests --test decision_index --locked -q

# Fast iteration: formatting, agent/repo QA, then type-check every target.
# Full clippy and workspace tests still run in `just check` before finishing.
quick: fmt-check ctx repo-check
    cargo check --workspace --all-targets --locked
