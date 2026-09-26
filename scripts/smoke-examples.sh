#!/usr/bin/env bash
# Runs every example in smoke-test mode (renders a few frames, then exits),
# then the render fixture matrices, and reports panics, failures and timeouts.
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

# run_row <label> <log file> <package> [VAR=value ...]
# Runs one example under the timeout, prints OK / TIMEOUT / FAIL and returns
# the run's exit status. Without --preserve-status, `timeout` reports 124 when
# the limit is hit (137 if the SIGKILL grace period also expired).
run_row() {
  local label="$1" log_file="$2" pkg="$3"
  shift 3
  printf "  %-28s ... " "$label"
  local code=0
  env TUNGSTEN_SMOKE_FRAMES="$SMOKE_FRAMES" "$@" \
    timeout --kill-after="$KILL_AFTER_SECS" "$TIMEOUT_SECS" \
    cargo run -p "$pkg" --quiet >"$log_file" 2>&1 || code=$?
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

# M25: msaa × depth_sort matrix over example-02-sprite-stress.
matrix_pkg="example-02-sprite-stress"
begin_section "M25 MSAA × depth_sort matrix (pkg: $matrix_pkg)"
for msaa in 1 4; do
  for sort in cpu_stable gpu_depth; do
    row "msaa=${msaa} depth_sort=${sort}" "$log_dir/${matrix_pkg}-msaa${msaa}-${sort}.log" "$matrix_pkg" \
      TUNGSTEN_RENDER_MSAA="$msaa" TUNGSTEN_RENDER_DEPTH_SORT="$sort"
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
