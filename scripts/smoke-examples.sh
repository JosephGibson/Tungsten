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

# M26: post-stack fixture matrix over example-04-shader-playground. Keeps the
# byte-identity gate separate from the 17-effect walk so a per-effect failure
# surfaces without pulling the empty-stack row down.
post_pkg="example-04-shader-playground"
post_pass=()
post_fail=()
echo
echo "M26 post-stack fixture matrix (pkg: $post_pkg)"
for fixture in empty all; do
  label="fixture=${fixture}"
  printf "  %-28s ... " "$label"
  log_file="$log_dir/${post_pkg}-${fixture}.log"
  if TUNGSTEN_SMOKE_FRAMES="$SMOKE_FRAMES" \
     TUNGSTEN_POST_STACK_FIXTURE="$fixture" \
     timeout --preserve-status "$TIMEOUT_SECS" \
     cargo run -p "$post_pkg" --quiet >"$log_file" 2>&1; then
    echo "OK"
    post_pass+=("$label")
  else
    code=$?
    if [ "$code" -eq 124 ]; then
      echo "TIMEOUT (${TIMEOUT_SECS}s)"
    else
      echo "FAIL (exit $code)"
    fi
    post_fail+=("$label ($log_file)")
  fi
done

echo "Post-stack passed: ${#post_pass[@]}/2"
if [ ${#post_fail[@]} -gt 0 ]; then
  echo "Post-stack failures:"
  for row in "${post_fail[@]}"; do
    echo "  - $row"
  done
  exit 1
fi

# M27: post-AA fixture row. Pins TUNGSTEN_POST_STACK_FIXTURE=empty so the SMAA
# tail is the only added work, then runs once with the High preset.
echo
echo "M27 post-AA fixture matrix (pkg: $post_pkg)"
post_aa_pass=()
post_aa_fail=()
for aa in smaa_high; do
  label="post_stack=empty post_aa=${aa}"
  printf "  %-28s ... " "$label"
  log_file="$log_dir/${post_pkg}-post-aa-${aa}.log"
  if TUNGSTEN_SMOKE_FRAMES="$SMOKE_FRAMES" \
     TUNGSTEN_POST_STACK_FIXTURE="empty" \
     TUNGSTEN_POST_AA_FIXTURE="$aa" \
     timeout --preserve-status "$TIMEOUT_SECS" \
     cargo run -p "$post_pkg" --quiet >"$log_file" 2>&1; then
    echo "OK"
    post_aa_pass+=("$label")
  else
    code=$?
    if [ "$code" -eq 124 ]; then
      echo "TIMEOUT (${TIMEOUT_SECS}s)"
    else
      echo "FAIL (exit $code)"
    fi
    post_aa_fail+=("$label ($log_file)")
  fi
done

echo "Post-AA passed: ${#post_aa_pass[@]}/1"
if [ ${#post_aa_fail[@]} -gt 0 ]; then
  echo "Post-AA failures:"
  for row in "${post_aa_fail[@]}"; do
    echo "  - $row"
  done
  exit 1
fi

# M28: bloom fixture row. Locks the post stack to bloom-only and turns the
# bloom env hint on so the playground spawns the emissive quad and pushes the
# demo-tuned BloomParams. Verifies the multi-subpass slot path runs cleanly.
echo
echo "M28 bloom fixture matrix (pkg: $post_pkg)"
bloom_pass=()
bloom_fail=()
for label_bloom in "post_stack=bloom_only bloom_fixture=on"; do
  label="$label_bloom"
  printf "  %-28s ... " "$label"
  log_file="$log_dir/${post_pkg}-bloom.log"
  if TUNGSTEN_SMOKE_FRAMES="$SMOKE_FRAMES" \
     TUNGSTEN_POST_STACK_FIXTURE="bloom_only" \
     TUNGSTEN_BLOOM_FIXTURE="on" \
     timeout --preserve-status "$TIMEOUT_SECS" \
     cargo run -p "$post_pkg" --quiet >"$log_file" 2>&1; then
    echo "OK"
    bloom_pass+=("$label")
  else
    code=$?
    if [ "$code" -eq 124 ]; then
      echo "TIMEOUT (${TIMEOUT_SECS}s)"
    else
      echo "FAIL (exit $code)"
    fi
    bloom_fail+=("$label ($log_file)")
  fi
done

echo "Bloom passed: ${#bloom_pass[@]}/1"
if [ ${#bloom_fail[@]} -gt 0 ]; then
  echo "Bloom failures:"
  for row in "${bloom_fail[@]}"; do
    echo "  - $row"
  done
  exit 1
fi

# M29: lighting fixture row over example-01-platformer. Pins lighting_fixture=on
# so the platformer spawns warm + cool point lights and a directional, routes the
# walk_* sprites through the lit pipeline, and exercises the LightUbo upload
# path on the lit batch keying.
lighting_pkg="example-01-platformer"
echo
echo "M29 lighting fixture matrix (pkg: $lighting_pkg)"
lighting_pass=()
lighting_fail=()
for label_light in "lighting_fixture=on"; do
  label="$label_light"
  printf "  %-28s ... " "$label"
  log_file="$log_dir/${lighting_pkg}-lighting.log"
  if TUNGSTEN_SMOKE_FRAMES="$SMOKE_FRAMES" \
     TUNGSTEN_LIGHTING_FIXTURE="on" \
     timeout --preserve-status "$TIMEOUT_SECS" \
     cargo run -p "$lighting_pkg" --quiet >"$log_file" 2>&1; then
    echo "OK"
    lighting_pass+=("$label")
  else
    code=$?
    if [ "$code" -eq 124 ]; then
      echo "TIMEOUT (${TIMEOUT_SECS}s)"
    else
      echo "FAIL (exit $code)"
    fi
    lighting_fail+=("$label ($log_file)")
  fi
done

echo "Lighting passed: ${#lighting_pass[@]}/1"
if [ ${#lighting_fail[@]} -gt 0 ]; then
  echo "Lighting failures:"
  for row in "${lighting_fail[@]}"; do
    echo "  - $row"
  done
  exit 1
fi
