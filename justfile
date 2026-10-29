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
    RUSTFLAGS="${TUNGSTEN_PERF_RUSTFLAGS--C force-frame-pointers=yes}" cargo test --release -p tungsten-core --locked --test integration physics_ "$@"

# Compile every benchmark without running it.
bench-build:
    cargo bench --workspace --no-run --locked

# GPU smoke run of every example plus the render fixture matrices.
smoke:
    ./scripts/smoke-examples.sh

# Pixel tests: the reference PNGs (reference machine only), then the post-stack, transition and UI overlay regressions.
visual:
    TUNGSTEN_VISUAL_REGRESSION=1 cargo test -p example-02-bench --test visual_regression --locked -- --nocapture
    TUNGSTEN_VISUAL_REGRESSION=1 cargo test -p example-03-fixtures --test integration --locked post_regression:: -- --nocapture
    TUNGSTEN_VISUAL_REGRESSION=1 cargo test -p example-03-fixtures --test integration --locked transition_regression:: -- --nocapture
    TUNGSTEN_VISUAL_REGRESSION=1 cargo test -p example-03-fixtures --test integration --locked ui_regression:: -- --nocapture
    TUNGSTEN_VISUAL_REGRESSION=1 cargo test -p tungsten-template-basic --test game --locked -- visual:: --nocapture

# Benchmark runner; arguments go to scripts/bench.py, e.g. `just perf run physics --repeat 5`.
perf *args:
    python3 -B scripts/bench.py "$@"

# Benchmark-runner regression tests: the old runner's and the new one's (redo 7.3).
perf-test:
    python3 -B scripts/test-bench.py
    python3 -B scripts/test-perf.py

# Report stale build artifacts in target/debug (hashes cargo left behind; newest 2 per crate stay); `just clean-target --apply` deletes, --keep N and --profile release adjust.
clean-target *args:
    python3 -B scripts/clean-target.py "$@"

# Shell, hook and workflow lint plus smoke-script, perf-helper, repo-checker, commit-hook and release-script tests (no GPU).
script-test: perf-test
    shellcheck scripts/*.sh .githooks/commit-msg
    actionlint
    bash scripts/test-smoke-examples.sh
    python3 -B scripts/test-clean-target.py
    python3 -B scripts/test-check-repo.py
    python3 -B scripts/test-commit-msg.py
    python3 -B scripts/test-release.py
    python3 -B scripts/test-crash-report.py
    python3 -B scripts/test-release-preflight.py
    python3 -B templates/basic/tools/sounds.py --check
    python3 -B templates/basic/tools/sprites.py --check
    python3 -B templates/basic/tools/icon.py --check

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

# Platformer generator: `--check` and its unittests (examples/01_platformer/tools/README.md). They need Pillow, so not in CI or the standard-library-only recipes.
level-check:
    python3 -B examples/01_platformer/tools/generate.py --check
    python3 -B -m unittest discover -s examples/01_platformer/tools -p 'test_*.py'

# Agent instruction budgets, links, skill symlinks and repo-byte totals.
ctx:
    python3 -B scripts/check-agent-context.py
    python3 -B scripts/check-agent-context.py --self-test

# File coverage, docs, active plans, version/changelog agreement, manifest/index tests.
repo-check:
    python3 -B scripts/check-repo.py
    python3 -B scripts/release.py check
    cargo test -p tungsten-core --test integration --locked -q -- manifests:: decision_index::

# Recommend checks without running them; explicit paths or Git changes, including untracked (--base REF for committed work).
checks *args:
    python3 -B scripts/check-repo.py --select-checks "$@"

# `tungsten new`'s game outside the repository: checked, built, tested, smoke-run and packaged there, then seeded errors (D-123, D-183); a release check. GPU; free-space floor first.
template-check:
    bash scripts/template-check.sh

# Third-party license notices for both release targets into target/notices/<target>/ (D-122); a release check. Its cargo fetch may need the network.
notices:
    for target in x86_64-unknown-linux-gnu x86_64-pc-windows-msvc; do \
        python3 -B scripts/release.py licenses "$target" --out "target/notices/$target/licenses.json" && \
        python3 -B scripts/release.py notices "target/notices/$target/licenses.json" --out "target/notices/$target" || exit 1; \
    done

# Version/changelog agreement (D-071); a tag argument (v0.27.0) is checked too.
release-check *args:
    python3 -B scripts/release.py check "$@"

# Read-only Git/GitHub inspection that prints the remaining release commands; VERSION, --repo, optional --message/--branch/--ref/--no-pr.
release-preflight version *args:
    python3 -B scripts/release-preflight.py "$@"

# Cut VERSION after the missing_docs ratchet: [Unreleased] becomes [VERSION] - today (--date to override); version, status lines and Cargo.lock follow.
release-cut version *args: docs-ratchet
    python3 -B scripts/release.py cut "$@"
    cargo update --workspace --offline || { echo "Cargo.lock not refreshed; run: cargo update --workspace" >&2; exit 1; }

# missing_docs only falls (D-215): public items without rustdoc per crate against scripts/missing-docs.json; `--write` lowers the baseline, `--list CRATE` names the items.
docs-ratchet *args:
    python3 -B scripts/missing-docs.py "$@"

# Use the repository's Git hooks (.githooks/): commit-msg checks Conventional Commits, one line (docs/operating-rules.md#git).
hooks:
    git config core.hooksPath .githooks

# Fast iteration: format check, agent/repo QA, type-check; `just check` still runs clippy and tests.
quick: fmt-check ctx repo-check
    cargo check --workspace --all-targets --locked

# What CI runs, in its order, in one local command (`D-070`): `check`, the dev-profile bench build (`D-112`), then the recipes after `&&`; GPU smoke, `just visual` and perf stay separate.
ci: check && deps ctx repo-check script-test
    cargo bench --workspace --no-run --profile dev --locked
