#!/usr/bin/env bash
# Runs every example in smoke-test mode (renders a few frames, then exits)
# and reports which ones panic or time out.
#
# Requires a real GPU and display — not for CI. Use as a pre-commit check
# when touching engine code, asset manifests, or example wiring.
#
# Env vars:
#   TUNGSTEN_SMOKE_FRAMES   Frames each example renders before exit (default: 3)
#   TUNGSTEN_SMOKE_TIMEOUT  Per-example wall-clock timeout in seconds (default: 90)
#   WGPU_BACKEND            Override wgpu backend if auto-detection picks wrong

set -u

cd "$(dirname "$0")/.."

SMOKE_FRAMES="${TUNGSTEN_SMOKE_FRAMES:-3}"
TIMEOUT_SECS="${TUNGSTEN_SMOKE_TIMEOUT:-90}"

mapfile -t EXAMPLES < <(
  cargo metadata --no-deps --format-version 1 \
    | jq -r '.packages[] | select(.name | test("^example-")) | .name' \
    | sort
)

echo "Pre-building all examples..."
if ! cargo build --workspace --quiet 2>&1; then
  echo "Workspace build failed; aborting smoke run."
  exit 1
fi

pass=()
fail=()
log_dir="$(mktemp -d)"
echo "Per-example logs: $log_dir"
echo

for pkg in "${EXAMPLES[@]}"; do
  printf "  %-28s ... " "$pkg"
  log_file="$log_dir/$pkg.log"
  if TUNGSTEN_SMOKE_FRAMES="$SMOKE_FRAMES" \
     timeout --preserve-status "$TIMEOUT_SECS" \
     cargo run -p "$pkg" --quiet >"$log_file" 2>&1; then
    echo "OK"
    pass+=("$pkg")
  else
    code=$?
    if [ "$code" -eq 124 ]; then
      echo "TIMEOUT (${TIMEOUT_SECS}s)"
    else
      echo "FAIL (exit $code)"
    fi
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

# M25: msaa × depth_sort matrix over example-02-sprite-stress. Uses env
# overrides from `tungsten-core::config` so no tracked config edits are
# needed. Each row still honors TUNGSTEN_SMOKE_FRAMES + the per-example
# timeout from above.
matrix_pkg="example-02-sprite-stress"
matrix_pass=()
matrix_fail=()
echo
echo "M25 MSAA × depth_sort matrix (pkg: $matrix_pkg)"
for msaa in 1 4; do
  for sort in cpu_stable gpu_depth; do
    label="msaa=${msaa} depth_sort=${sort}"
    printf "  %-28s ... " "$label"
    log_file="$log_dir/${matrix_pkg}-msaa${msaa}-${sort}.log"
    if TUNGSTEN_SMOKE_FRAMES="$SMOKE_FRAMES" \
       TUNGSTEN_RENDER_MSAA="$msaa" \
       TUNGSTEN_RENDER_DEPTH_SORT="$sort" \
       timeout --preserve-status "$TIMEOUT_SECS" \
       cargo run -p "$matrix_pkg" --quiet >"$log_file" 2>&1; then
      echo "OK"
      matrix_pass+=("$label")
    else
      code=$?
      if [ "$code" -eq 124 ]; then
        echo "TIMEOUT (${TIMEOUT_SECS}s)"
      else
        echo "FAIL (exit $code)"
      fi
      matrix_fail+=("$label ($log_file)")
    fi
  done
done

echo "Matrix passed: ${#matrix_pass[@]}/4"
if [ ${#matrix_fail[@]} -gt 0 ]; then
  echo "Matrix failures:"
  for row in "${matrix_fail[@]}"; do
    echo "  - $row"
  done
  exit 1
fi
