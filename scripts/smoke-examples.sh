#!/usr/bin/env bash
# Runs every example in smoke-test mode (renders a few frames, then exits),
# then the render fixture matrices, the benchmark rows and a frame-cap timing
# row, and reports panics, failures and timeouts.
#
# Requires a real GPU and display — not for CI. Use as a pre-commit check
# when touching engine code, asset manifests, or example wiring. Regression
# tests with a stubbed cargo: scripts/test-smoke-examples.sh.
#
# Env vars:
#   TUNGSTEN_SMOKE_FRAMES   Frames each example renders before exit (default: 3)
#   TUNGSTEN_SMOKE_TIMEOUT  Per-run wall-clock timeout in seconds (default: 90)
#   WGPU_BACKEND            Override wgpu backend if auto-detection picks wrong

set -u -o pipefail

cd "$(dirname "$0")/.." || exit 1

SMOKE_FRAMES="${TUNGSTEN_SMOKE_FRAMES:-3}"
TIMEOUT_SECS="${TUNGSTEN_SMOKE_TIMEOUT:-90}"
# Grace period before SIGKILL when a timed-out run ignores SIGTERM.
KILL_AFTER_SECS=10

if ! metadata="$(cargo metadata --no-deps --format-version 1)"; then
  echo "Example discovery failed: cargo metadata exited non-zero." >&2
  exit 1
fi
if ! names="$(jq -r '.packages[] | select(.name | test("^example-")) | .name' <<<"$metadata")"; then
  echo "Example discovery failed: could not parse cargo metadata output." >&2
  exit 1
fi
mapfile -t EXAMPLES < <(sed '/^$/d' <<<"$names" | sort)
if [ "${#EXAMPLES[@]}" -eq 0 ]; then
  echo "Example discovery failed: no example-* packages in the workspace." >&2
  exit 1
fi

echo "Pre-building all examples..."
if ! cargo build --workspace --quiet 2>&1; then
  echo "Workspace build failed; aborting smoke run."
  exit 1
fi

log_dir="$(mktemp -d)"
echo "Per-example logs: $log_dir"
echo

# Shortest wall time in milliseconds a passing run may take; 0 means no
# minimum. Set around a row whose run must not finish early.
min_run_ms=0

# run_row <label> <log file> <package> [VAR=value ...]
# Runs one example under the timeout, prints OK / TIMEOUT / FAIL and returns
# the run's exit status. Without --preserve-status, `timeout` reports 124 when
# the limit is hit (137 if the SIGKILL grace period also expired). A run that
# exits 0 in less than min_run_ms fails with status 1.
run_row() {
  local label="$1" log_file="$2" pkg="$3"
  shift 3
  printf "  %-28s ... " "$label"
  local code=0 start_ms elapsed_ms
  start_ms="$(date +%s%3N)"
  env TUNGSTEN_SMOKE_FRAMES="$SMOKE_FRAMES" "$@" \
    timeout --kill-after="$KILL_AFTER_SECS" "$TIMEOUT_SECS" \
    cargo run -p "$pkg" --quiet >"$log_file" 2>&1 || code=$?
  elapsed_ms=$(($(date +%s%3N) - start_ms))
  if [ "$code" -eq 0 ] && [ "$elapsed_ms" -lt "$min_run_ms" ]; then
    echo "FAIL (${elapsed_ms} ms, expected at least ${min_run_ms} ms)"
    return 1
  fi
  case "$code" in
    0) echo "OK" ;;
    124) echo "TIMEOUT (${TIMEOUT_SECS}s)" ;;
    137) echo "TIMEOUT (${TIMEOUT_SECS}s, killed after ${KILL_AFTER_SECS}s grace)" ;;
    *) echo "FAIL (exit $code)" ;;
  esac
  return "$code"
}

pass=()
fail=()
for pkg in "${EXAMPLES[@]}"; do
  if run_row "$pkg" "$log_dir/$pkg.log" "$pkg"; then
    pass+=("$pkg")
  else
    fail+=("$pkg")
  fi
done

echo
echo "Passed: ${#pass[@]}/${#EXAMPLES[@]}"
if [ ${#fail[@]} -gt 0 ]; then
  echo "Failed:"
  for p in "${fail[@]}"; do
    echo "  - $p   (tail of $log_dir/$p.log):"
    tail -15 "$log_dir/$p.log" | sed 's/^/      /'
  done
  exit 1
fi

# Fixture matrices: each row honors TUNGSTEN_SMOKE_FRAMES and the timeout, and
# uses env overrides so no tracked config edits are needed.
section_ok=0
section_fail=()

begin_section() {
  echo
  echo "$1"
  section_ok=0
  section_fail=()
}

# row <label> <log file> <package> [VAR=value ...]
row() {
  if run_row "$@"; then
    section_ok=$((section_ok + 1))
  else
    section_fail+=("$1 ($2)")
  fi
}

# end_section <summary prefix> <failure heading> <expected rows>
end_section() {
  echo "$1: ${section_ok}/$3"
  if [ "${#section_fail[@]}" -gt 0 ]; then
    echo "$2:"
    printf '  - %s\n' "${section_fail[@]}"
    exit 1
  fi
}

# M25: msaa × depth_sort matrix over the gpu benchmark at `min` (lit, material,
# glow, tile and text batches). TUNGSTEN_RENDER_MSAA wins over the preset's
# `aa=off`, and the benchmark never sets depth_sort.
matrix_pkg="example-02-bench"
begin_section "M25 MSAA × depth_sort matrix (pkg: $matrix_pkg, gpu preset=min)"
for msaa in 1 4; do
  for sort in cpu_stable gpu_depth; do
    row "msaa=${msaa} depth_sort=${sort}" "$log_dir/${matrix_pkg}-msaa${msaa}-${sort}.log" "$matrix_pkg" \
      TUNGSTEN_BENCH=gpu TUNGSTEN_BENCH_PRESET=min TUNGSTEN_RENDER_MSAA="$msaa" TUNGSTEN_RENDER_DEPTH_SORT="$sort"
  done
done
end_section "Matrix passed" "Matrix failures" 4

# M26: post-stack fixture matrix over example-04-shader-playground. Keeps the
# byte-identity gate separate from the 17-effect walk so a per-effect failure
# surfaces without pulling the empty-stack row down.
post_pkg="example-04-shader-playground"
begin_section "M26 post-stack fixture matrix (pkg: $post_pkg)"
for fixture in empty all; do
  row "fixture=${fixture}" "$log_dir/${post_pkg}-${fixture}.log" "$post_pkg" \
    TUNGSTEN_POST_STACK_FIXTURE="$fixture"
done
end_section "Post-stack passed" "Post-stack failures" 2

# M27: post-AA fixture row. Pins TUNGSTEN_POST_STACK_FIXTURE=empty so the SMAA
# tail is the only added work, then runs once with the High preset.
begin_section "M27 post-AA fixture matrix (pkg: $post_pkg)"
row "post_stack=empty post_aa=smaa_high" "$log_dir/${post_pkg}-post-aa-smaa_high.log" "$post_pkg" \
  TUNGSTEN_POST_STACK_FIXTURE=empty TUNGSTEN_POST_AA_FIXTURE=smaa_high
end_section "Post-AA passed" "Post-AA failures" 1

# M28: bloom fixture row. Locks the post stack to bloom-only and turns the
# bloom env hint on so the playground spawns the emissive quad and pushes the
# demo-tuned BloomParams. Verifies the multi-subpass slot path runs cleanly.
begin_section "M28 bloom fixture matrix (pkg: $post_pkg)"
row "post_stack=bloom_only bloom_fixture=on" "$log_dir/${post_pkg}-bloom.log" "$post_pkg" \
  TUNGSTEN_POST_STACK_FIXTURE=bloom_only TUNGSTEN_BLOOM_FIXTURE=on
end_section "Bloom passed" "Bloom failures" 1

# M29: lighting fixture row over example-01-platformer. Pins lighting_fixture=on
# so the platformer spawns warm + cool point lights and a directional, routes the
# walk_* sprites through the lit pipeline, and exercises the LightUbo upload
# path on the lit batch keying.
lighting_pkg="example-01-platformer"
begin_section "M29 lighting fixture matrix (pkg: $lighting_pkg)"
row "lighting_fixture=on" "$log_dir/${lighting_pkg}-lighting.log" "$lighting_pkg" \
  TUNGSTEN_LIGHTING_FIXTURE=on
end_section "Lighting passed" "Lighting failures" 1

# M30: game-feel rows. The playground row pins the fixture env so trauma and
# every squash envelope are armed at startup — a three-frame run would not
# otherwise reach a collision. The platformer row runs plain: the parallax
# backdrop and the landing squash are wired unconditionally there.
feel_pkg="example-04-shader-playground"
begin_section "M30 game-feel fixture matrix (pkgs: $feel_pkg, $lighting_pkg)"
row "game_feel_fixture=on" "$log_dir/${feel_pkg}-game-feel.log" "$feel_pkg" \
  TUNGSTEN_GAME_FEEL_FIXTURE=on
row "parallax backdrop" "$log_dir/${lighting_pkg}-game-feel.log" "$lighting_pkg"
end_section "Game-feel passed" "Game-feel failures" 2

# Benchmarks: each example-02-bench row at smoke length. Default-scale rows
# run at dev opt-level 0; the slowest, ecs at default, takes about 12 s. The
# gpu default row also runs the per-pass timestamp queries (materials, lit
# atlases, tiles, text, bloom, vignette, SMAA).
bench_pkg="example-02-bench"
begin_section "Benchmarks (pkg: $bench_pkg)"
for bench_row in physics:min physics:default physics:sparse-min physics:sparse \
  ecs:min ecs:default churn:min churn:default gpu:min gpu:default gpu:throughput \
  particles:min particles:default integrated:min integrated:default; do
  bench="${bench_row%%:*}"
  preset="${bench_row#*:}"
  timing=()
  if [ "$bench_row" = gpu:default ]; then
    timing=(TUNGSTEN_GPU_TIMING=1)
  fi
  row "${bench} preset=${preset}${timing:+ timing=on}" "$log_dir/${bench_pkg}-${bench}-${preset}.log" "$bench_pkg" \
    TUNGSTEN_BENCH="$bench" TUNGSTEN_BENCH_PRESET="$preset" "${timing[@]}"
done
end_section "Benchmarks passed" "Benchmark failures" 15

# Frame cap: 20 frames at display.frame_rate_cap = 20 must take at least 90%
# of frames / cap (0.9 s). Uncapped, the same run takes about 0.5 s, so a cap
# that stops limiting the frame rate fails the row. Last, so a slow machine
# cannot hide the rows above.
cap_pkg="example-03-scene-state"
cap_frames=20
cap_fps=20
begin_section "Frame-cap row (pkg: $cap_pkg)"
min_run_ms=$((cap_frames * 900 / cap_fps))
row "frames=${cap_frames} frame_rate_cap=${cap_fps}" "$log_dir/${cap_pkg}-frame-cap.log" "$cap_pkg" \
  TUNGSTEN_SMOKE_FRAMES="$cap_frames" TUNGSTEN_DISPLAY_FRAME_RATE_CAP="$cap_fps"
min_run_ms=0
end_section "Frame cap passed" "Frame-cap failures" 1
