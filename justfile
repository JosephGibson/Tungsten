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

# Release physics tests: determinism and containment (ignored in debug, so not in `check`) plus tunneling, with the perf runner's build flags.
physics-release *args:
    RUSTFLAGS="${TUNGSTEN_PERF_RUSTFLAGS--C force-frame-pointers=yes}" cargo test --release -p tungsten-core --locked --test physics_determinism --test physics_tunneling --test physics_containment "$@"

# Compile every benchmark without running it.
bench-build:
    cargo bench --workspace --no-run --locked

# GPU smoke run of every example plus the render fixture matrices.
smoke:
    ./scripts/smoke-examples.sh

# Pixel tests: the reference PNG (reference machine only), then the post-stack and transition regressions.
visual:
    TUNGSTEN_VISUAL_REGRESSION=1 cargo test -p example-02-bench --test visual_regression --locked -- --nocapture
    TUNGSTEN_VISUAL_REGRESSION=1 cargo test -p example-04-shader-playground --test post_regression --locked -- --nocapture
    TUNGSTEN_VISUAL_REGRESSION=1 cargo test -p example-03-scene-state --test transition_regression --locked -- --nocapture

# Benchmark runner; arguments go to scripts/bench.py, e.g. `just perf run physics --repeat 5`.
perf *args:
    python3 -B scripts/bench.py "$@"

# Benchmark-runner regression tests.
perf-test:
    python3 -B scripts/test-bench.py

# Shell and workflow lint plus smoke-script, perf-helper, repo-checker, roadmap-helper and release-script tests (no GPU).
script-test: perf-test
    shellcheck scripts/*.sh
    actionlint
    bash scripts/test-smoke-examples.sh
    python3 -B scripts/test-check-repo.py
    python3 -B scripts/test-roadmap.py
    python3 -B scripts/test-release.py
    python3 -B scripts/test-release-preflight.py

# Dependency policy: advisories, licenses, bans, sources.
deps:
    cargo deny --locked check

# Public API snapshots in api/ (D-104, D-107; `cargo install --locked cargo-public-api@0.52.0`). Not in CI.
api:
    bash scripts/public-api.sh

# Fails when an api/ snapshot is stale; a release check.
api-check:
    bash scripts/public-api.sh --check

# Unused dependencies (cargo-shear; `cargo install --locked cargo-shear`). Not in CI.
udeps:
    cargo shear --locked

# Agent instruction budgets, links, skill symlinks and repo-byte totals.
ctx:
    python3 -B scripts/check-agent-context.py
    python3 -B scripts/check-agent-context.py --self-test

# File coverage, docs, active plans, version/changelog agreement, manifest/index tests.
repo-check:
    python3 -B scripts/check-repo.py
    python3 -B scripts/release.py check
    cargo test -p tungsten-core --test manifests --test decision_index --locked -q

# Version/changelog agreement (D-071); a tag argument (v0.27.0) is checked too.
release-check *args:
    python3 -B scripts/release.py check "$@"

# Read-only Git/GitHub inspection that prints the remaining release commands; VERSION, --repo, optional --message/--branch/--ref/--no-pr.
release-preflight version *args:
    python3 -B scripts/release-preflight.py "$@"

# Cut VERSION: [Unreleased] becomes [VERSION] - today (--date to override); version, status lines, Cargo.lock follow.
release-cut version *args:
    python3 -B scripts/release.py cut "$@"
    cargo update --workspace --offline || { echo "Cargo.lock not refreshed; run: cargo update --workspace" >&2; exit 1; }

# Fast iteration: format check, agent/repo QA, type-check; `just check` still runs clippy and tests.
quick: fmt-check ctx repo-check
    cargo check --workspace --all-targets --locked

# The six recipes CI runs, in one local command (`D-070`); GPU smoke, `just visual` and perf stay separate.
ci: check bench-build deps ctx repo-check script-test
