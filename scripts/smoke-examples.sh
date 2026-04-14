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

EXAMPLES=(
  example-01-window
  example-02-ecs
  example-03-dots
  example-04-sprites
  example-05-animation
  example-06-text
  example-07-audio
  example-08-hot-reload
  example-09-tilemap
  example-10-platformer
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
