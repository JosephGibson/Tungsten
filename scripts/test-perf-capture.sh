#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source-path=SCRIPTDIR source=perf-capture.sh
source "$SCRIPT_DIR/perf-capture.sh"

assert_eq() {
  local expected="$1"
  local actual="$2"
  local label="$3"
  if [[ "$actual" != "$expected" ]]; then
    echo "$label: expected '$expected', got '$actual'"
    exit 1
  fi
}

WARMUP_FRAMES=2

log_file="$(mktemp)"
scratch_dir="$(mktemp -d)"
trap 'rm -f "$log_file"; rm -rf "$scratch_dir"' EXIT

cat >"$log_file" <<'EOF'
[2026-04-15T18:46:51Z DEBUG tungsten::app] backend: Vulkan adapter: AMD Radeon 660M (RADV REMBRANDT) present_mode: Mailbox max_frame_latency: 3 timestamp_query: true
[2026-04-15T18:46:51Z DEBUG tungsten::app] frame: total=9.00ms update=0.00ms flush=0.00ms extract=0.00ms render=0.00ms render_acquire=0.90ms render_encode=0.00ms render_submit_present=0.00ms gpu=n/a audio=0.00ms hot_reload=0.00ms
[2026-04-15T18:46:51Z DEBUG tungsten::app] systems: steer_agents_system=9.00ms physics_step=0.00ms
[2026-04-15T18:46:51Z DEBUG tungsten::app] frame: total=8.00ms update=0.00ms flush=0.00ms extract=0.00ms render=0.00ms render_acquire=0.80ms render_encode=0.00ms render_submit_present=0.00ms gpu=n/a audio=0.00ms hot_reload=0.00ms
[2026-04-15T18:46:51Z DEBUG tungsten::app] systems: steer_agents_system=8.00ms physics_step=0.00ms
[2026-04-15T18:46:51Z DEBUG tungsten::app] frame: total=1.00ms update=0.10ms flush=0.00ms extract=0.00ms render=0.00ms render_acquire=0.10ms render_encode=0.00ms render_submit_present=0.00ms gpu=n/a audio=0.00ms hot_reload=0.00ms
[2026-04-15T18:46:51Z DEBUG tungsten::app] systems: steer_agents_system=1.00ms physics_step=0.05ms
[2026-04-15T18:46:51Z DEBUG tungsten::app] frame: total=2.00ms update=0.20ms flush=0.00ms extract=0.00ms render=0.00ms render_acquire=0.20ms render_encode=0.00ms render_submit_present=0.00ms gpu=n/a audio=0.00ms hot_reload=0.00ms
[2026-04-15T18:46:51Z DEBUG tungsten::app] systems: steer_agents_system=2.00ms physics_step=0.10ms
[2026-04-15T18:46:51Z DEBUG tungsten::app] frame: total=3.00ms update=0.30ms flush=0.00ms extract=0.03ms render=0.00ms render_acquire=0.30ms render_encode=0.00ms render_submit_present=0.00ms gpu=n/a audio=0.00ms hot_reload=0.00ms
[2026-04-15T18:46:51Z DEBUG tungsten::app] systems: steer_agents_system=3.00ms physics_step=0.15ms
[2026-04-15T18:46:51Z DEBUG tungsten::app] frame: total=4.00ms update=0.40ms flush=0.00ms extract=0.00ms render=0.00ms render_acquire=0.40ms render_encode=0.00ms render_submit_present=0.00ms gpu=n/a audio=0.00ms hot_reload=0.00ms
[2026-04-15T18:46:51Z DEBUG tungsten::app] systems: steer_agents_system=4.00ms physics_step=0.20ms
[2026-04-15T18:46:51Z DEBUG tungsten::app] frame: total=5.00ms update=0.50ms flush=0.00ms extract=0.05ms render=0.00ms render_acquire=0.50ms render_encode=0.00ms render_submit_present=0.00ms gpu=n/a audio=0.00ms hot_reload=0.00ms
[2026-04-15T18:46:51Z DEBUG tungsten::app] systems: steer_agents_system=5.00ms physics_step=0.25ms
EOF

parse_backend_metadata "$log_file"
assert_eq "Vulkan" "$METADATA_BACKEND" "backend"
assert_eq "AMD Radeon 660M (RADV REMBRANDT)" "$METADATA_ADAPTER" "adapter"
assert_eq "Mailbox" "$METADATA_PRESENT_MODE" "present mode"
assert_eq "3" "$METADATA_MAX_FRAME_LATENCY" "max frame latency"
assert_eq "true" "$METADATA_TIMESTAMP_QUERY" "timestamp query"
assert_eq "mailbox-lat3" "$(capture_config_suffix "mailbox" "3")" "config suffix"
assert_eq "count20000" "$(capture_config_suffix "" "" "20000")" "stress count suffix"
assert_eq "mailbox-lat3-count20000" "$(capture_config_suffix "mailbox" "3" "20000")" "combined suffix"
assert_eq "full" "$(capture_mode_label 0)" "full capture mode label"
assert_eq "telemetry-only" "$(capture_mode_label 1)" "telemetry-only capture mode label"
assert_eq "mailbox" "$(requested_value_or_none "mailbox")" "requested value passthrough"
assert_eq "none" "$(requested_value_or_none "")" "requested value empty label"
assert_eq "example-02-sprite-stress" "$(resolve_scene_package "ecs-high-load")" "ecs-high-load package"
assert_eq "-C force-frame-pointers=yes" "$(unset TUNGSTEN_PERF_RUSTFLAGS; perf_rustflags)" "default perf RUSTFLAGS"
assert_eq "-C target-cpu=native" "$(TUNGSTEN_PERF_RUSTFLAGS="-C target-cpu=native" perf_rustflags)" "perf RUSTFLAGS override"

mapfile -t ecs_scene_env < <(scene_env_overrides "ecs-high-load")
assert_eq "1" "${#ecs_scene_env[@]}" "ecs-high-load env count"
assert_eq "STRESS_SCENE=ecs-high-load" "${ecs_scene_env[0]}" "ecs-high-load env override"

mapfile -t baseline_scene_env < <(scene_env_overrides "sprite-stress")
assert_eq "1" "${#baseline_scene_env[@]}" "baseline env count"
assert_eq "STRESS_SCENE=baseline" "${baseline_scene_env[0]}" "baseline env override"

assert_eq "3.00" "$(avg_metric "$log_file" "total")" "average total"
assert_eq "3.00" "$(percentile_metric "$log_file" "total" 50)" "p50 total"
assert_eq "5.00" "$(percentile_metric "$log_file" "total" 95)" "p95 total"
assert_eq "5.00" "$(percentile_metric "$log_file" "total" 99)" "p99 total"
assert_eq "0.30" "$(avg_metric "$log_file" "update")" "average update"
assert_eq "0.30" "$(percentile_metric "$log_file" "update" 50)" "p50 update"
assert_eq "0.50" "$(percentile_metric "$log_file" "update" 95)" "p95 update"
assert_eq "0.30" "$(avg_metric "$log_file" "render_acquire")" "average acquire"
assert_eq "0.30" "$(percentile_metric "$log_file" "render_acquire" 50)" "p50 acquire"
assert_eq "0.50" "$(percentile_metric "$log_file" "render_acquire" 95)" "p95 acquire"
assert_eq "0.50" "$(percentile_metric "$log_file" "render_acquire" 99)" "p99 acquire"

assert_eq "0.02" "$(avg_metric "$log_file" "extract")" "average extract"
assert_eq "0.05" "$(percentile_metric "$log_file" "extract" 95)" "p95 extract"

mapfile -t systems < <(system_names "$log_file")
assert_eq "2" "${#systems[@]}" "system count"
assert_eq "steer_agents_system" "${systems[0]}" "first system in registration order"
assert_eq "physics_step" "${systems[1]}" "second system in registration order"
assert_eq "3.00" "$(avg_metric "$log_file" "steer_agents_system" "systems:")" "average system"
assert_eq "5.00" "$(percentile_metric "$log_file" "steer_agents_system" 95 "systems:")" "p95 system"
assert_eq "0.25" "$(percentile_metric "$log_file" "physics_step" 99 "systems:")" "p99 system"
assert_eq "| physics_step | 0.15 | 0.15 | 0.25 | 0.25 |" "$(system_timing_rows "$log_file" | tail -n 1)" "system table row"

# Registered system names are literal keys, including regex punctuation.
printf 'systems: steer[0]=9.00ms\nsystems: steer[0]=8.00ms\nsystems: steer[0]=1.25ms\n' >"$scratch_dir/names.txt"
assert_eq "1.25" "$(avg_metric "$scratch_dir/names.txt" 'steer[0]' 'systems:')" "literal system name"

# Provenance: sysfs-style values, the n/a path, and git label shapes.
printf 'performance\n' >"$scratch_dir/governor"
assert_eq "performance" "$(file_value_or_na "$scratch_dir/governor")" "sysfs value"
assert_eq "n/a" "$(file_value_or_na "$scratch_dir/missing")" "absent sysfs file"
: >"$scratch_dir/empty"
assert_eq "n/a" "$(file_value_or_na "$scratch_dir/empty")" "empty sysfs file"
commit_label="$(git_commit_label)"
[[ "$commit_label" =~ ^([0-9a-f]{7,}|unknown)$ ]] || { echo "git commit label: got '$commit_label'"; exit 1; }
dirty_label="$(git_dirty_label)"
[[ "$dirty_label" =~ ^(no|unknown|yes\ \(diff\ [0-9a-f]{12}\))$ ]] || { echo "git dirty label: got '$dirty_label'"; exit 1; }

# Repeat runs: medians, per-run metrics rows and the summary table.
assert_eq "2.00" "$(printf '3\n1\n2\n' | median_of_values)" "odd median"
assert_eq "2.50" "$(printf '4\nn/a\n1\n2\n3\n' | median_of_values)" "even median skips n/a"
assert_eq "n/a" "$(printf 'n/a\n' | median_of_values)" "median of nothing"
write_run_metrics "$log_file" "$scratch_dir/run-1.tsv"
assert_eq "$(printf 'total\t3.00\t5.00')" "$(head -n 1 "$scratch_dir/run-1.tsv")" "run metrics total row"
assert_eq "$(printf 'system:physics_step\t0.15\t0.25')" "$(tail -n 1 "$scratch_dir/run-1.tsv")" "run metrics system row"
printf 'total\t4.00\t6.00\nsystem:physics_step\t0.35\t0.45\n' >"$scratch_dir/run-2.tsv"
printf 'total\t2.00\t9.00\n' >"$scratch_dir/run-3.tsv"
mapfile -t summary < <(summary_rows "$scratch_dir/run-1.tsv" "$scratch_dir/run-2.tsv" "$scratch_dir/run-3.tsv")
assert_eq "| total | 3.00 | 6.00 | 3.00 / 4.00 / 2.00 | 5.00 / 6.00 / 9.00 |" "${summary[0]}" "summary total row"
assert_eq "| \`physics_step\` (system) | 0.25 | 0.35 | 0.15 / 0.35 / n/a | 0.25 / 0.45 / n/a |" "${summary[-1]}" "summary row with a missing run"

# Physics awake phase: frames with `sleeping < dynamic` only (the pile falls
# asleep after the fourth frame); warm-up frames never count.
physics_log="$scratch_dir/physics.txt"
{
  for sample in "9 9.00 0" "9 9.00 0" "1 0.50 0" "2 0.70 1" "3 0.10 2" "4 0.10 2"; do
    read -r total step sleeping <<<"$sample"
    echo "frame: total=${total}.00ms update=${step}ms extract=0.00ms render=0.00ms"
    echo "systems: physics_step=${step}ms sync_position_to_transform=0.01ms"
    echo "physics: proxies=3 dynamic=2 sleeping=${sleeping} pairs=$((4 - 2 * sleeping)) contacts=1"
  done
} >"$physics_log"
assert_eq "0.60" "$(awake_metric_samples "$physics_log" physics_step "systems:" | samples_avg)" "awake physics_step avg"
assert_eq "0.70" "$(awake_metric_samples "$physics_log" physics_step "systems:" | samples_percentile 95)" "awake physics_step p95"
assert_eq "1.50" "$(awake_metric_samples "$physics_log" total | samples_avg)" "awake total avg"
assert_eq "3.00" "$(awake_metric_samples "$physics_log" pairs "physics:" | samples_avg)" "awake pairs avg"
assert_eq "| update ms | 0.60 | 0.50 | 0.70 | 0.70 |" "$(awake_row "$physics_log" "update ms" update)" "awake README row"
assert_eq "n/a" "$(awake_metric_samples "$log_file" total | samples_avg)" "no physics lines, no awake frames"
has_dynamic_physics "$physics_log" || { echo "has_dynamic_physics: expected true"; exit 1; }
if has_dynamic_physics "$log_file"; then echo "has_dynamic_physics: expected false"; exit 1; fi
write_run_metrics "$physics_log" "$scratch_dir/physics.tsv"
assert_eq "$(printf 'awake_frames\t2\t-')" "$(grep '^awake_frames' "$scratch_dir/physics.tsv")" "awake frame count row"
assert_eq "$(printf 'awake:physics_step\t0.60\t0.70')" "$(grep '^awake:physics_step' "$scratch_dir/physics.tsv")" "awake physics_step row"
# Render-throughput rows: submit/present and GPU percentiles, GPU from the
# GPU-timing log of the same run.
gpu_log="$scratch_dir/gpu.txt"
for ms in 9.00 9.00 1.00 2.00 3.00 4.00; do
  echo "frame: total=${ms}ms render_encode=0.10ms render_submit_present=0.20ms gpu=${ms}ms"
done >"$gpu_log"
write_run_metrics "$log_file" "$scratch_dir/run-gpu.tsv" "$gpu_log"
assert_eq "$(printf 'gpu\t2.50\t4.00')" "$(grep '^gpu' "$scratch_dir/run-gpu.tsv")" "gpu metrics row"
assert_eq "$(printf 'render_submit_present\t0.00\t0.00')" "$(grep '^render_submit_present' "$scratch_dir/run-gpu.tsv")" "submit/present metrics row"
assert_eq "" "$(grep '^gpu' "$scratch_dir/run-1.tsv" || true)" "no gpu row without a GPU log"
assert_eq "4.00" "$(percentile_metric "$gpu_log" gpu 95)" "p95 gpu"

assert_eq "sleepoff" "$(capture_config_suffix "" "" "" "off")" "physics sleep off suffix"
assert_eq "count10000-sleepoff" "$(capture_config_suffix "" "" "10000" "off")" "count plus sleep off suffix"
assert_eq "" "$(capture_config_suffix "" "" "" "on")" "physics sleep on adds no suffix"

frame_pointers_enabled '-Cforce-frame-pointers=yes' || { echo "compact -C flag rejected"; exit 1; }
if frame_pointers_enabled '-C force-frame-pointers=yes -C force-frame-pointers=no'; then
  echo "last frame-pointer override ignored"
  exit 1
fi

# GPU names are unique literal keys, with one companion line per frame,
# including empty unsupported/skipped frames. They come only from the GPU log.
cat >"$scratch_dir/passes.txt" <<'EOF'
frame: gpu=9.00ms
gpu_passes: scene=9.00ms
frame: gpu=9.00ms
gpu_passes: scene=9.00ms
frame: gpu=n/a
gpu_passes:
frame: gpu=1.00ms
gpu_passes: scene=1.00ms post0_bloom_down1=0.25ms post1_bloom_down1=0.50ms render_span=3.00ms
frame: gpu=2.00ms
gpu_passes: scene=2.00ms post0_bloom_down1=0.75ms render_span=4.00ms
EOF
assert_eq "0.50" "$(avg_metric "$scratch_dir/passes.txt" post0_bloom_down1 gpu_passes:)" "GPU warmup includes empty frames"
assert_eq "0.50" "$(avg_metric "$scratch_dir/passes.txt" post1_bloom_down1 gpu_passes:)" "repeated bloom slots stay distinct"
write_run_metrics "$log_file" "$scratch_dir/passes.tsv" "$scratch_dir/passes.txt"
assert_eq "$(printf 'gpu_pass:scene\t1.50\t2.00')" "$(grep '^gpu_pass:scene' "$scratch_dir/passes.tsv")" "GPU metrics use diagnostic source"
assert_eq "example-02-sprite-stress" "$(resolve_scene_package render-features)" "render scene package"
assert_eq "STRESS_SCENE=render-features" "$(scene_env_overrides render-features)" "render scene env"

# Recording policy: DWARF stays the default; frequency is an optional override.
assert_eq $'--call-graph\ndwarf' "$(perf_record_args dwarf '')" "default record args"
assert_eq $'--call-graph\nfp\n-F\n499' "$(perf_record_args fp 499)" "fp frequency args"
for args in '--call-graph bogus' '--call-graph' '--sample-frequency 0' '--sample-frequency -1' '--sample-frequency abc' '--sample-frequency' '--ecs-density' '--ecs-density wrong' 'sprite-stress --ecs-density preserve'; do
  read -r -a invalid <<<"$args"
  if (main "${invalid[@]}") >"$scratch_dir/invalid.txt" 2>&1; then
    echo "Invalid flags accepted: $args"
    exit 1
  fi
done

# Exercise main's child environments without building or opening a window.
# An inherited GPU timer must not add a blocking readback to CPU captures.
(
  mkdir -p "$scratch_dir/capture/target/release" "$scratch_dir/bin"
  cp "$log_file" "$scratch_dir/capture/fixture.txt"
  cat >"$scratch_dir/bin/cargo" <<'EOF'
#!/usr/bin/env bash
exit 0
EOF
  cat >"$scratch_dir/capture/target/release/example-02-sprite-stress" <<'EOF'
#!/usr/bin/env bash
printf '%s\n' "${TUNGSTEN_GPU_TIMING-unset}" >>child-env.txt
printf '%s\n' "${STRESS_ECS_DENSITY-unset}" >>density-env.txt
cat fixture.txt
EOF
  cat >"$scratch_dir/bin/perf" <<'EOF'
#!/usr/bin/env bash
printf '%s\n' "$*" >>perf-args.txt
printf '%s\n' "${TUNGSTEN_GPU_TIMING-unset}" >>profile-env.txt
EOF
  chmod +x "$scratch_dir/bin/perf"
  chmod +x "$scratch_dir/bin/cargo" "$scratch_dir/capture/target/release/example-02-sprite-stress"
  export PATH="$scratch_dir/bin:$PATH"
  export TUNGSTEN_GPU_TIMING=1
  export STRESS_ECS_DENSITY=preserve
  REPO_ROOT="$scratch_dir/capture"
  main sprite-stress 5 --telemetry-only --call-graph fp --sample-frequency 499 >"$scratch_dir/capture-output.txt"
  capture_readme="$(find perf-runs -name README.md -print -quit)"
  grep -q '| perf call graph | fp |' "$capture_readme"
  grep -q '| perf sampling Hz | 499 |' "$capture_readme"
  if (TUNGSTEN_PERF_RUSTFLAGS='-C target-cpu=native' main sprite-stress 5 --call-graph fp) >"$scratch_dir/no-fp.txt" 2>&1; then
    echo "FP capture accepted flags without frame pointers"
    exit 1
  fi
  assert_eq $'fixed\nfixed' "$(cat density-env.txt)" "inherited density mode cleared"
  assert_eq $'unset\n1' "$(cat child-env.txt)" "CPU/GPU capture environment isolation"
  main sprite-stress 5 --call-graph fp --sample-frequency 499 >"$scratch_dir/full-output.txt"
  : >density-env.txt
  main ecs-high-load 5 --ecs-density preserve --stress-count 12500 --telemetry-only >"$scratch_dir/density-output.txt"
  assert_eq $'preserve\npreserve' "$(cat density-env.txt)" "density mode reaches both captures"
  grep -q '| ECS density mode | preserve |' perf-runs/*-densitypreserve/README.md
  grep -q 'record --call-graph fp -F 499 -o ' perf-args.txt
  assert_eq $'unset\nunset' "$(cat profile-env.txt)" "profilers exclude GPU readback"
)

assert_eq "count12500-densitypreserve" "$(capture_config_suffix "" "" "12500" "" "preserve")" "density suffix"

echo "perf-capture helpers: OK"
