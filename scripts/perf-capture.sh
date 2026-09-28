#!/usr/bin/env bash

set -u

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

: "${WARMUP_FRAMES:=60}"

# Build flags for every capture binary. Setting RUSTFLAGS replaces the
# target flags in .cargo/config.toml (target-cpu=native), so perf builds are
# generic x86-64 with frame pointers unless TUNGSTEN_PERF_RUSTFLAGS says
# otherwise. Historical captures used exactly this default; only compare
# captures whose README records the same "Build RUSTFLAGS".
DEFAULT_PERF_RUSTFLAGS="-C force-frame-pointers=yes"

METADATA_BACKEND="unknown"
METADATA_ADAPTER="unknown"
METADATA_PRESENT_MODE="unknown"
METADATA_MAX_FRAME_LATENCY="unknown"
METADATA_TIMESTAMP_QUERY="unknown"

usage() {
  cat <<'EOF'
Usage: perf-capture.sh [scene] [frames] [--present-mode <mode>] [--max-frame-latency <n>] [--stress-count <n>] [--physics-sleep on|off] [--ecs-density fixed|preserve] [--repeat <n>] [--call-graph dwarf|fp] [--sample-frequency <hz>] [--telemetry-only]

Scenes:
  ecs-high-load (default)
  sprite-stress
  physics-stress
  render-features

Flags:
  --present-mode <mode>       Override the resolved present mode for child capture runs
  --max-frame-latency <n>     Override the requested max-frame-latency hint for child capture runs
  --stress-count <n>          Override the scene entity/body count for child capture runs
  --physics-sleep on|off      physics-stress only: `off` disables island sleeping (child-only
                              STRESS_PHYSICS_SLEEP=0) so every frame runs the awake solve
  --ecs-density fixed|preserve  ecs-high-load only: scale world area with count (default: fixed)
  --repeat <n>                Run n sequential captures into run-1..run-n/ and write summary.md with
                              per-metric medians across runs; profilers run on run-1 only
  --call-graph dwarf|fp       Stack recording mode (default: dwarf, keeps inline attribution);
                              fp uses smaller records and requires force-frame-pointers=yes
  --sample-frequency <hz>     perf record sampling frequency (default: perf's default)
  --telemetry-only            Skip flamegraph/perf artifact capture; still writes telemetry logs and README

Env:
  TUNGSTEN_PERF_RUSTFLAGS     Build flags for the capture binary (default: -C force-frame-pointers=yes)
EOF
}

# Samples of `key` from post-warm-up log lines tagged `tag` (default
# `frame:`; per-system values use `systems:`).
metric_samples() {
  local file="$1"
  local key="$2"
  local tag="${3:-frame:}"
  awk -v warmup="$WARMUP_FRAMES" -v key="$key" -v tag="$tag" '
    index($0, tag) {
      seen += 1
      if (seen <= warmup) {
        next
      }
      for (i = 1; i <= NF; i++) {
        prefix = key "="
        if (index($i, prefix) == 1) {
          value = substr($i, length(prefix) + 1)
          sub(/ms$/, "", value)
          if (value != "n/a") {
            print value
          }
        }
      }
    }
  ' "$file"
}

# Average of the samples on stdin (blank lines skipped), or `n/a`.
samples_avg() {
  awk '
    NF {
      sum += $1
      n += 1
    }
    END {
      if (n > 0) {
        printf "%.2f", sum / n
      } else {
        printf "n/a"
      }
    }
  '
}

# Nearest-rank percentile of the samples on stdin (blank lines skipped), or
# `n/a`.
samples_percentile() {
  local percentile="$1"
  sort -g | awk -v percentile="$percentile" '
    NF {
      values[++n] = $1
    }
    END {
      if (n == 0) {
        printf "n/a"
        exit
      }
      rank = int((percentile * n + 99) / 100)
      if (rank < 1) {
        rank = 1
      }
      if (rank > n) {
        rank = n
      }
      printf "%.2f", values[rank]
    }
  '
}

avg_metric() {
  metric_samples "$1" "$2" "${3:-frame:}" | samples_avg
}

percentile_metric() {
  metric_samples "$1" "$2" "${4:-frame:}" | samples_percentile "$3"
}

# Post-warm-up `key` samples from `tag` lines, kept only for frames whose
# `physics:` line reports awake dynamic bodies (`sleeping < dynamic`). A
# `frame:` line opens each frame; its `systems:` and `physics:` lines follow.
awake_metric_samples() {
  local file="$1"
  local key="$2"
  local tag="${3:-frame:}"
  awk -v warmup="$WARMUP_FRAMES" -v key="$key" -v tag="$tag" '
    function field(name,    i, prefix) {
      prefix = name "="
      for (i = 1; i <= NF; i++) {
        if (index($i, prefix) == 1) {
          return substr($i, length(prefix) + 1)
        }
      }
      return ""
    }
    function flush() {
      if (frames > warmup && has_physics && sleeping < dynamic && has_value) {
        print value
      }
      has_value = 0
      has_physics = 0
    }
    index($0, "frame:") {
      flush()
      frames += 1
    }
    index($0, tag) {
      v = field(key)
      sub(/ms$/, "", v)
      if (v != "" && v != "n/a") {
        value = v
        has_value = 1
      }
    }
    index($0, "physics:") {
      dynamic = field("dynamic") + 0
      sleeping = field("sleeping") + 0
      has_physics = 1
    }
    END {
      flush()
    }
  ' "$file"
}

# True when any `physics:` line reports dynamic collider bodies.
has_dynamic_physics() {
  awk '
    index($0, "physics:") {
      for (i = 1; i <= NF; i++) {
        if (index($i, "dynamic=") == 1 && substr($i, 9) + 0 > 0) {
          found = 1
          exit
        }
      }
    }
    END {
      exit !found
    }
  ' "$1"
}

# `| label | avg | p50 | p95 | p99 |` over awake frames only.
awake_row() {
  local file="$1"
  local label="$2"
  local key="$3"
  local tag="${4:-frame:}"
  local samples
  samples="$(awake_metric_samples "$file" "$key" "$tag")"
  printf '| %s | %s | %s | %s | %s |\n' "$label" \
    "$(samples_avg <<<"$samples")" \
    "$(samples_percentile 50 <<<"$samples")" \
    "$(samples_percentile 95 <<<"$samples")" \
    "$(samples_percentile 99 <<<"$samples")"
}

# System names from `systems:` lines, in first-seen (registration) order.
system_names() {
  local file="$1"
  local tag="${2:-systems:}"
  awk -v tag="$tag" '
    index($0, tag) {
      for (i = 1; i <= NF; i++) {
        eq = index($i, "=")
        if (eq > 1 && $i ~ /ms$/) {
          name = substr($i, 1, eq - 1)
          if (!(name in seen)) {
            seen[name] = 1
            print name
          }
        }
      }
    }
  ' "$file"
}

# Markdown rows `| name | avg | p50 | p95 | p99 |` for every system.
system_timing_rows() {
  local file="$1"
  local tag="${2:-systems:}"
  local name
  while IFS= read -r name; do
    printf '| %s | %s | %s | %s | %s |\n' "$name" \
      "$(avg_metric "$file" "$name" "$tag")" \
      "$(percentile_metric "$file" "$name" 50 "$tag")" \
      "$(percentile_metric "$file" "$name" 95 "$tag")" \
      "$(percentile_metric "$file" "$name" 99 "$tag")"
  done < <(system_names "$file" "$tag")
}

parse_backend_metadata() {
  local file="$1"
  local line
  line="$(grep -m1 'backend:' "$file" 2>/dev/null || true)"

  METADATA_BACKEND="unknown"
  METADATA_ADAPTER="unknown"
  METADATA_PRESENT_MODE="unknown"
  METADATA_MAX_FRAME_LATENCY="unknown"
  METADATA_TIMESTAMP_QUERY="unknown"

  if [[ "$line" =~ backend:\ ([^[:space:]]+)\ adapter:\ (.+)\ present_mode:\ ([^[:space:]]+)\ max_frame_latency:\ ([0-9]+)\ timestamp_query:\ (true|false) ]]; then
    METADATA_BACKEND="${BASH_REMATCH[1]}"
    METADATA_ADAPTER="${BASH_REMATCH[2]}"
    METADATA_PRESENT_MODE="${BASH_REMATCH[3]}"
    METADATA_MAX_FRAME_LATENCY="${BASH_REMATCH[4]}"
    METADATA_TIMESTAMP_QUERY="${BASH_REMATCH[5]}"
  fi
}

capture_config_suffix() {
  local present_mode_override="${1:-}"
  local max_frame_latency_override="${2:-}"
  local stress_count_override="${3:-}"
  local physics_sleep_override="${4:-}"
  local ecs_density_override="${5:-}"
  local -a parts=()

  if [ -n "$present_mode_override" ]; then
    parts+=("$present_mode_override")
  fi
  if [ -n "$max_frame_latency_override" ]; then
    parts+=("lat${max_frame_latency_override}")
  fi
  if [ -n "$stress_count_override" ]; then
    parts+=("count${stress_count_override}")
  fi
  if [ "$physics_sleep_override" = "off" ]; then
    parts+=("sleepoff")
  fi

  if [ -n "$ecs_density_override" ]; then
    parts+=("density${ecs_density_override}")
  fi

  if [ "${#parts[@]}" -eq 0 ]; then
    return 0
  fi

  local IFS='-'
  printf '%s' "${parts[*]}"
}

requested_value_or_none() {
  local value="${1:-}"
  if [ -n "$value" ]; then
    printf '%s' "$value"
  else
    printf 'none'
  fi
}

capture_mode_label() {
  local telemetry_only="${1:-0}"
  if [ "$telemetry_only" -eq 1 ]; then
    printf 'telemetry-only'
  else
    printf 'full'
  fi
}

resolve_scene_package() {
  local scene="$1"
  case "$scene" in
    sprite-stress|ecs-high-load|physics-stress|render-features)
      printf '%s' "example-02-sprite-stress"
      ;;
    *)
      return 1
      ;;
  esac
}

scene_env_overrides() {
  local scene="$1"
  case "$scene" in
    sprite-stress)
      printf '%s\n' "STRESS_SCENE=baseline"
      ;;
    ecs-high-load)
      printf '%s\n' "STRESS_SCENE=ecs-high-load"
      ;;
    render-features)
      printf '%s\n' "STRESS_SCENE=render-features"
      ;;
    physics-stress)
      printf '%s\n' "STRESS_SCENE=physics-stress"
      ;;
    *)
      return 1
      ;;
  esac
}

perf_rustflags() {
  printf '%s' "${TUNGSTEN_PERF_RUSTFLAGS:-$DEFAULT_PERF_RUSTFLAGS}"
}

# First line of a sysfs-style file, or `n/a` when it is absent or empty
# (no cpufreq driver, no ACPI platform profile, non-Linux hosts).
file_value_or_na() {
  local path="$1"
  local value=""
  if [ -r "$path" ]; then
    IFS= read -r value <"$path" || true
  fi
  printf '%s' "${value:-n/a}"
}

git_commit_label() {
  git -C "$REPO_ROOT" rev-parse --short HEAD 2>/dev/null || printf 'unknown'
}

# `no` for a clean tree; otherwise `yes` plus a fingerprint of the tracked
# diff and untracked files, so dirty captures of the same tree still match.
git_dirty_label() {
  local status
  if ! status="$(git -C "$REPO_ROOT" status --porcelain 2>/dev/null)"; then
    printf 'unknown'
    return 0
  fi
  if [ -z "$status" ]; then
    printf 'no'
    return 0
  fi
  local fingerprint
  fingerprint="$(
    {
      git -C "$REPO_ROOT" diff HEAD --binary
      git -C "$REPO_ROOT" ls-files -z --others --exclude-standard \
        | (cd "$REPO_ROOT" && xargs -0 -r sha256sum)
    } 2>/dev/null | sha256sum | cut -c1-12
  )"
  printf 'yes (diff %s)' "$fingerprint"
}

# Median of the numeric values on stdin (`n/a` and blanks skipped); the mean
# of the two middle values for an even count.
median_of_values() {
  awk '$1 ~ /^-?[0-9]+(\.[0-9]+)?$/ { print $1 }' | sort -g | awk '
    {
      values[NR] = $1
    }
    END {
      if (NR == 0) {
        printf "n/a"
      } else if (NR % 2 == 1) {
        printf "%.2f", values[(NR + 1) / 2]
      } else {
        printf "%.2f", (values[NR / 2] + values[NR / 2 + 1]) / 2
      }
    }
  '
}

# Per-run `metric<TAB>avg<TAB>p95` rows; summary.md takes their medians. The
# `gpu` row comes from the optional GPU-timing log of the same run.
write_run_metrics() {
  local engine_log="$1"
  local out="$2"
  local gpu_log="${3:-}"
  local key
  local name
  {
    for key in total update extract render render_acquire render_encode render_submit_present; do
      printf '%s\t%s\t%s\n' "$key" \
        "$(avg_metric "$engine_log" "$key")" \
        "$(percentile_metric "$engine_log" "$key" 95)"
    done
    if [ -n "$gpu_log" ]; then
      printf 'gpu\t%s\t%s\n' \
        "$(avg_metric "$gpu_log" gpu)" \
        "$(percentile_metric "$gpu_log" gpu 95)"
      while IFS= read -r name; do
        printf 'gpu_pass:%s\t%s\t%s\n' "$name" \
          "$(avg_metric "$gpu_log" "$name" "gpu_passes:")" \
          "$(percentile_metric "$gpu_log" "$name" 95 "gpu_passes:")"
      done < <(system_names "$gpu_log" "gpu_passes:")
    fi
    while IFS= read -r name; do
      printf 'system:%s\t%s\t%s\n' "$name" \
        "$(avg_metric "$engine_log" "$name" "systems:")" \
        "$(percentile_metric "$engine_log" "$name" 95 "systems:")"
    done < <(system_names "$engine_log")
    if has_dynamic_physics "$engine_log"; then
      printf 'awake_frames\t%s\t-\n' "$(awake_metric_samples "$engine_log" total | wc -l)"
      local samples
      for key in physics_step update total; do
        if [ "$key" = physics_step ]; then
          samples="$(awake_metric_samples "$engine_log" physics_step "systems:")"
        else
          samples="$(awake_metric_samples "$engine_log" "$key")"
        fi
        printf 'awake:%s\t%s\t%s\n' "$key" \
          "$(samples_avg <<<"$samples")" \
          "$(samples_percentile 95 <<<"$samples")"
      done
    fi
  } >"$out"
}

# Markdown rows `| metric | median avg | median p95 | avg per run | p95 per
# run |` over run metrics files, metrics in first-seen order. A metric absent
# from a run shows `n/a` in that run's slot and is left out of the median.
summary_rows() {
  local -a files=("$@")
  local metric
  local file
  local value
  local label
  while IFS= read -r metric; do
    local -a avgs=()
    local -a p95s=()
    for file in "${files[@]}"; do
      value="$(awk -F'\t' -v m="$metric" '$1 == m { print $2; exit }' "$file")"
      avgs+=("${value:-n/a}")
      value="$(awk -F'\t' -v m="$metric" '$1 == m { print $3; exit }' "$file")"
      p95s+=("${value:-n/a}")
    done
    label="$metric"
    if [[ "$metric" == system:* ]]; then
      label="\`${metric#system:}\` (system)"
    elif [[ "$metric" == gpu_pass:* ]]; then
      label="\`${metric#gpu_pass:}\` (GPU)"
    elif [[ "$metric" == awake:* ]]; then
      label="${metric#awake:} (awake frames)"
    elif [ "$metric" = awake_frames ]; then
      label="awake frame count"
    fi
    local joined_avgs joined_p95s
    joined_avgs="$(printf '%s / ' "${avgs[@]}")"
    joined_p95s="$(printf '%s / ' "${p95s[@]}")"
    printf '| %s | %s | %s | %s | %s |\n' "$label" \
      "$(printf '%s\n' "${avgs[@]}" | median_of_values)" \
      "$(printf '%s\n' "${p95s[@]}" | median_of_values)" \
      "${joined_avgs% / }" \
      "${joined_p95s% / }"
  done < <(awk -F'\t' '!seen[$1]++ { print $1 }' "${files[@]}")
}

# summary.md for a --repeat capture. Reads the capture context from main's
# locals (bash dynamic scope), like write_run_readme.
write_summary() {
  local out_dir="$1"
  shift
  local -a run_dirs=("$@")
  local -a metric_files=()
  local run_dir
  local run_links=""
  for run_dir in "${run_dirs[@]}"; do
    metric_files+=("$run_dir/metrics.tsv")
    run_links+="[\`${run_dir##*/}\`](${run_dir##*/}/README.md) "
  done
  parse_backend_metadata "${run_dirs[0]}/engine-telemetry.txt"

  cat >"$out_dir/summary.md" <<EOF
# Perf Summary: $scene (${#run_dirs[@]} runs)

| Field | Value |
| --- | --- |
| Git commit | ${git_commit} |
| Dirty tree | ${git_dirty} |
| CPU governor (cpu0) | ${cpu_governor} |
| Platform profile | ${platform_profile} |
| Compiler | ${rustc_version} |
| Build RUSTFLAGS | \`${build_rustflags}\` |
| perf call graph | ${call_graph} |
| perf sampling Hz | ${sample_frequency:-perf default} |
| Backend env | ${WGPU_BACKEND:-auto} |
| Capture mode | ${capture_mode} |
| Requested present mode override | $(requested_value_or_none "$requested_present_mode") |
| Requested max frame latency override | $(requested_value_or_none "$requested_max_frame_latency") |
| Requested stress count override | $(requested_value_or_none "$requested_stress_count") |
| Requested physics sleep override | $(requested_value_or_none "$requested_physics_sleep") |
| ECS density mode | ${requested_ecs_density:-fixed} |
| Present mode | ${METADATA_PRESENT_MODE} |
| Renderer adapter | ${METADATA_ADAPTER} |
| Warm-up / measured frames | ${WARMUP_FRAMES} / ${frames} |
| Runs | ${run_links% } |

## Medians Across Runs

Each run's post-warm-up average and p95, then the median across runs (the comparison rule in docs/perf/profiling-workflow.md). Values are ms.

| Metric | Median avg | Median p95 | Avg per run | p95 per run |
| --- | --- | --- | --- | --- |
$(summary_rows "${metric_files[@]}")
EOF
}

# RUSTFLAGS is whitespace-separated; honor the last frame-pointer override.
frame_pointers_enabled() {
  awk '{
    for (i = 1; i <= NF; i++) {
      sub(/^-C/, "", $i)
      if ($i ~ /^force-frame-pointers=/) enabled = ($i == "force-frame-pointers=yes")
    }
  } END { exit !enabled }' <<<"$1"
}

# One argument per line, shared by capture and the command regression tests.
perf_record_args() {
  printf '%s\n' --call-graph "$1"
  if [ -n "${2:-}" ]; then
    printf '%s\n' -F "$2"
  fi
}

# perf stat, perf record and the flamegraph folded from that recording, all
# written into `dir`. Reads the binary and profile env from main's locals.
capture_profiles() {
  local dir="$1"
  # Profilers run the binary built above from the repo root (config and
  # manifests resolve from the CWD) and write only into $dir. The
  # flamegraph is folded from this run's perf record data rather than via
  # `cargo flamegraph`, which would rebuild with different flags and leave
  # perf.data in the repo root. Optional failures are recorded in README.
  if command -v perf >/dev/null 2>&1; then
    echo "Capturing perf stat..."
    "${profile_env[@]}" perf stat -d -o "$dir/perf-stat.txt" "$binary" >/dev/null 2>&1 || true

    echo "Capturing perf record..."
    local -a record_args=()
    mapfile -t record_args < <(perf_record_args "$call_graph" "$sample_frequency")
    "${profile_env[@]}" perf record "${record_args[@]}" -o "$dir/perf-record.data" "$binary" >"$dir/perf-record.log" 2>&1 || true
  else
    echo "perf not installed; skipping perf captures."
  fi

  if [ -f "$dir/perf-record.data" ] && command -v flamegraph >/dev/null 2>&1; then
    echo "Rendering flamegraph from perf record data..."
    flamegraph --perfdata "$dir/perf-record.data" --output "$dir/flamegraph.svg" >/dev/null 2>&1 || true
  elif ! command -v flamegraph >/dev/null 2>&1; then
    echo "flamegraph not installed (cargo install flamegraph); skipping flamegraph."
  fi
}

# Per-run README.md. Reads the capture context (scene, frames, binary, flags,
# provenance, overrides) from main's locals via bash dynamic scope.
write_run_readme() {
  local run_dir="$1"
  local profiled="$2"
  local run_mode="$3"
  local skip_reason="$4"
  local run_label="$5"
  local engine_log="$run_dir/engine-telemetry.txt"
  local gpu_log="$run_dir/gpu-timing.txt"
  local flamegraph_out="$run_dir/flamegraph.svg"
  local perf_stat_out="$run_dir/perf-stat.txt"
  local perf_record_out="$run_dir/perf-record.data"

  parse_backend_metadata "$engine_log"

  local avg_total_ms
  avg_total_ms="$(avg_metric "$engine_log" "total")"
  local p50_total_ms
  p50_total_ms="$(percentile_metric "$engine_log" "total" 50)"
  local p95_total_ms
  p95_total_ms="$(percentile_metric "$engine_log" "total" 95)"
  local p99_total_ms
  p99_total_ms="$(percentile_metric "$engine_log" "total" 99)"

  local avg_render_acquire_ms
  avg_render_acquire_ms="$(avg_metric "$engine_log" "render_acquire")"
  local p50_render_acquire_ms
  p50_render_acquire_ms="$(percentile_metric "$engine_log" "render_acquire" 50)"
  local p95_render_acquire_ms
  p95_render_acquire_ms="$(percentile_metric "$engine_log" "render_acquire" 95)"
  local p99_render_acquire_ms
  p99_render_acquire_ms="$(percentile_metric "$engine_log" "render_acquire" 99)"

  local avg_update_ms
  avg_update_ms="$(avg_metric "$engine_log" "update")"
  local p50_update_ms
  p50_update_ms="$(percentile_metric "$engine_log" "update" 50)"
  local p95_update_ms
  p95_update_ms="$(percentile_metric "$engine_log" "update" 95)"
  local p99_update_ms
  p99_update_ms="$(percentile_metric "$engine_log" "update" 99)"

  local avg_extract_ms
  avg_extract_ms="$(avg_metric "$engine_log" "extract")"
  local p50_extract_ms
  p50_extract_ms="$(percentile_metric "$engine_log" "extract" 50)"
  local p95_extract_ms
  p95_extract_ms="$(percentile_metric "$engine_log" "extract" 95)"
  local p99_extract_ms
  p99_extract_ms="$(percentile_metric "$engine_log" "extract" 99)"

  local avg_render_ms
  avg_render_ms="$(avg_metric "$engine_log" "render")"
  local p50_render_ms
  p50_render_ms="$(percentile_metric "$engine_log" "render" 50)"
  local p95_render_ms
  p95_render_ms="$(percentile_metric "$engine_log" "render" 95)"
  local p99_render_ms
  p99_render_ms="$(percentile_metric "$engine_log" "render" 99)"

  local measured_frames
  measured_frames="$(metric_samples "$engine_log" total | wc -l)"
  local physics_awake_section
  if has_dynamic_physics "$engine_log"; then
    local awake_frames
    awake_frames="$(awake_metric_samples "$engine_log" total | wc -l)"
    physics_awake_section="Measured frames whose \`physics:\` line shows \`sleeping < dynamic\`: ${awake_frames} of ${measured_frames}. These rows cover only those frames, so they track the awake solve independent of when the scene falls asleep. Pairs and contacts are counts from the final substep.

| Metric | Avg | p50 | p95 | p99 |
| --- | --- | --- | --- | --- |
$(awake_row "$engine_log" "physics_step ms" physics_step "systems:")
$(awake_row "$engine_log" "update ms" update)
$(awake_row "$engine_log" "total ms" total)
$(awake_row "$engine_log" "pairs" pairs "physics:")
$(awake_row "$engine_log" "contacts" contacts "physics:")"
  else
    physics_awake_section="No dynamic collider bodies: no \`physics:\` line reports \`dynamic > 0\` (or the binary predates the physics perf line)."
  fi

  local system_rows
  system_rows="$(system_timing_rows "$engine_log")"
  if [ -z "$system_rows" ]; then
    system_rows="| (no \`systems:\` lines; binary predates per-system perf logging) | n/a | n/a | n/a | n/a |"
  fi

  local avg_render_encode_ms
  avg_render_encode_ms="$(avg_metric "$engine_log" "render_encode")"
  local avg_render_submit_ms
  avg_render_submit_ms="$(avg_metric "$engine_log" "render_submit_present")"
  local avg_gpu_ms
  avg_gpu_ms="$(avg_metric "$gpu_log" "gpu")"
  # Render-throughput rows (R1/R7) are judged on these percentiles, not on
  # total p95, which present pacing dominates in the canonical scenes.
  local p95_render_encode_ms
  p95_render_encode_ms="$(percentile_metric "$engine_log" "render_encode" 95)"
  local p95_render_submit_ms
  p95_render_submit_ms="$(percentile_metric "$engine_log" "render_submit_present" 95)"
  local p95_gpu_ms
  p95_gpu_ms="$(percentile_metric "$gpu_log" "gpu" 95)"

  local cpu_model
  cpu_model="$(grep -m1 'model name' /proc/cpuinfo 2>/dev/null | cut -d: -f2- | sed 's/^ //')"
  local host_kernel
  host_kernel="$(uname -sr)"
  local requested_present_mode_label
  requested_present_mode_label="$(requested_value_or_none "$requested_present_mode")"
  local requested_max_frame_latency_label
  requested_max_frame_latency_label="$(requested_value_or_none "$requested_max_frame_latency")"
  local requested_stress_count_label
  requested_stress_count_label="$(requested_value_or_none "$requested_stress_count")"
  local flamegraph_note
  local perf_stat_note
  local perf_record_note

  if [ "$profiled" -eq 0 ]; then
    flamegraph_note="Skipped ($skip_reason)"
    perf_stat_note="Skipped ($skip_reason)"
    perf_record_note="Skipped ($skip_reason)"
  else
    if [ -f "$flamegraph_out" ]; then
      flamegraph_note="Captured (folded from perf-record.data)"
    elif ! command -v flamegraph >/dev/null 2>&1; then
      flamegraph_note="Skipped (flamegraph not installed)"
    elif [ ! -f "$perf_record_out" ]; then
      flamegraph_note="Skipped (no perf record data)"
    else
      flamegraph_note="Skipped (capture failed)"
    fi

    if [ -f "$perf_stat_out" ]; then
      perf_stat_note="Captured"
    elif command -v perf >/dev/null 2>&1; then
      perf_stat_note="Skipped (capture failed)"
    else
      perf_stat_note="Skipped (perf not installed)"
    fi

    if [ -f "$perf_record_out" ]; then
      perf_record_note="Captured"
    elif command -v perf >/dev/null 2>&1; then
      perf_record_note="Skipped (capture failed)"
    else
      perf_record_note="Skipped (perf not installed)"
    fi
  fi

  cat >"$run_dir/README.md" <<EOF
# Perf Capture: $scene

## Machine

| Field | Value |
| --- | --- |
| Kernel | ${host_kernel:-unknown} |
| CPU | ${cpu_model:-unknown} |
| CPU governor (cpu0) | ${cpu_governor} |
| Platform profile | ${platform_profile} |
| Git commit | ${git_commit} |
| Dirty tree | ${git_dirty} |
| Binary | $binary |
| Compiler | ${rustc_version} |
| Build RUSTFLAGS | \`${build_rustflags}\` |
| perf call graph | ${call_graph} |
| perf sampling Hz | ${sample_frequency:-perf default} |
| Backend env | ${WGPU_BACKEND:-auto} |
| Capture mode | ${run_mode} |
| Run | ${run_label} |
| Requested present mode override | ${requested_present_mode_label} |
| Requested max frame latency override | ${requested_max_frame_latency_label} |
| Requested stress count override | ${requested_stress_count_label} |
| Requested physics sleep override | $(requested_value_or_none "$requested_physics_sleep") |
| ECS density mode | ${requested_ecs_density:-fixed} |
| Renderer backend | ${METADATA_BACKEND} |
| Renderer adapter | ${METADATA_ADAPTER} |
| Present mode | ${METADATA_PRESENT_MODE} |
| Requested max frame latency hint | ${METADATA_MAX_FRAME_LATENCY} |
| Timestamp query support | ${METADATA_TIMESTAMP_QUERY} |

## Captured Outputs

| File | Notes |
| --- | --- |
| \`engine-telemetry.txt\` | Stage-level frame timing log |
| \`gpu-timing.txt\` | Same run with \`TUNGSTEN_GPU_TIMING=1\` |
| \`flamegraph.svg\` | ${flamegraph_note} |
| \`perf-stat.txt\` | ${perf_stat_note} |
| \`perf-record.data\` | ${perf_record_note} |

## Measured Values

| Metric | Value |
| --- | --- |
| Average total frame ms | $avg_total_ms |
| p50 total frame ms | $p50_total_ms |
| p95 total frame ms | $p95_total_ms |
| p99 total frame ms | $p99_total_ms |
| Average render acquire ms | $avg_render_acquire_ms |
| p50 render acquire ms | $p50_render_acquire_ms |
| p95 render acquire ms | $p95_render_acquire_ms |
| p99 render acquire ms | $p99_render_acquire_ms |
| Average update ms | $avg_update_ms |
| p50 update ms | $p50_update_ms |
| p95 update ms | $p95_update_ms |
| p99 update ms | $p99_update_ms |
| Average extract ms | $avg_extract_ms |
| p50 extract ms | $p50_extract_ms |
| p95 extract ms | $p95_extract_ms |
| p99 extract ms | $p99_extract_ms |
| Average render ms | $avg_render_ms |
| p50 render ms | $p50_render_ms |
| p95 render ms | $p95_render_ms |
| p99 render ms | $p99_render_ms |
| Average render encode ms | $avg_render_encode_ms |
| p95 render encode ms | $p95_render_encode_ms |
| Average render submit/present ms | $avg_render_submit_ms |
| p95 render submit/present ms | $p95_render_submit_ms |
| Average GPU frame ms | $avg_gpu_ms |
| p95 GPU frame ms | $p95_gpu_ms |
| Warm-up frames skipped | $WARMUP_FRAMES |
| Measured frames requested | $frames |

## Per-System Update Timings

Post-warm-up values from the \`systems:\` telemetry lines, in registration order.

| System | Avg ms | p50 ms | p95 ms | p99 ms |
| --- | --- | --- | --- | --- |
${system_rows}

## GPU Pass Timings

Separate blocking-readback diagnostic; \`gpu=\` remains scene-only.
\`render_span\` spans scene start through present-blit end, includes gaps,
and excludes uploads, resolve/readback and presentation waits. It is not a pass sum.
Empty on unsupported/disabled/skipped frames; all GPU timing is excluded from CPU profiles.

| Pass / span | Avg ms | p50 ms | p95 ms | p99 ms |
| --- | --- | --- | --- | --- |
$(system_timing_rows "$gpu_log" "gpu_passes:")

## Physics Awake Phase

${physics_awake_section}

## Budget Targets

| Metric | Target |
| --- | --- |
| Sustained FPS | >= 60 |
| p95 frame time | <= 16.7ms |
| Update stage | well below 4ms |
| Extract stage | well below 3ms |
| Render stage | well below 8ms |

## Notes

- Render overrides are injected only into child capture processes; the parent shell environment is left unchanged.
- Scene selection is also injected only into child capture processes so shell-local \`STRESS_SCENE\` / \`STRESS_COUNT\` values cannot skew canonical runs.
- Flamegraph and perf captures intentionally run without \`TUNGSTEN_GPU_TIMING\` to avoid the blocking timestamp readback stall.
- Compare like-for-like runs only: same scene, resolution, backend, release build, build RUSTFLAGS, present mode, and max frame latency.
EOF
}

main() {
  cd "$REPO_ROOT" || exit 1

  local scene=""
  local frames=""
  local requested_present_mode=""
  local requested_max_frame_latency=""
  local requested_stress_count=""
  local requested_physics_sleep=""
  local requested_ecs_density=""
  local telemetry_only=0
  local repeat=1
  local call_graph=dwarf
  local sample_frequency=""

  while [ "$#" -gt 0 ]; do
    case "$1" in
      --present-mode)
        if [ "$#" -lt 2 ]; then
          echo "Missing value for --present-mode"
          usage
          exit 1
        fi
        requested_present_mode="$2"
        shift 2
        ;;
      --max-frame-latency)
        if [ "$#" -lt 2 ]; then
          echo "Missing value for --max-frame-latency"
          usage
          exit 1
        fi
        requested_max_frame_latency="$2"
        shift 2
        ;;
      --stress-count)
        if [ "$#" -lt 2 ]; then
          echo "Missing value for --stress-count"
          usage
          exit 1
        fi
        if ! [[ "$2" =~ ^[0-9]+$ ]] || [ "$2" -eq 0 ]; then
          echo "--stress-count expects a positive integer, got '$2'"
          usage
          exit 1
        fi
        requested_stress_count="$2"
        shift 2
        ;;
      --physics-sleep)
        if [ "$#" -lt 2 ]; then
          echo "Missing value for --physics-sleep"
          usage
          exit 1
        fi
        case "$2" in
          on|off)
            requested_physics_sleep="$2"
            ;;
          *)
            echo "--physics-sleep expects 'on' or 'off', got '$2'"
            usage
            exit 1
            ;;
        esac
        shift 2
        ;;
      --ecs-density)
        if [ "$#" -lt 2 ] || [[ "$2" != fixed && "$2" != preserve ]]; then
          echo "--ecs-density expects 'fixed' or 'preserve'"
          exit 1
        fi
        requested_ecs_density="$2"
        shift 2
        ;;
      --repeat)
        if [ "$#" -lt 2 ]; then
          echo "Missing value for --repeat"
          usage
          exit 1
        fi
        if ! [[ "$2" =~ ^[0-9]+$ ]] || [ "$2" -eq 0 ]; then
          echo "--repeat expects a positive integer, got '$2'"
          usage
          exit 1
        fi
        repeat="$2"
        shift 2
        ;;
      --call-graph)
        if [ "$#" -lt 2 ] || [[ "$2" != dwarf && "$2" != fp ]]; then
          echo "--call-graph expects 'dwarf' or 'fp'"
          exit 1
        fi
        call_graph="$2"
        shift 2
        ;;
      --sample-frequency)
        if [ "$#" -lt 2 ] || ! [[ "$2" =~ ^[1-9][0-9]*$ ]]; then
          echo "--sample-frequency expects a positive integer in Hz"
          exit 1
        fi
        sample_frequency="$2"
        shift 2
        ;;
      --telemetry-only)
        telemetry_only=1
        shift
        ;;
      -h|--help)
        usage
        exit 0
        ;;
      -*)
        echo "Unknown flag '$1'"
        usage
        exit 1
        ;;
      *)
        if [ -z "$scene" ]; then
          scene="$1"
        elif [ -z "$frames" ]; then
          frames="$1"
        else
          echo "Unexpected argument '$1'"
          usage
          exit 1
        fi
        shift
        ;;
    esac
  done

  scene="${scene:-ecs-high-load}"
  frames="${frames:-300}"
  if [ -n "$requested_physics_sleep" ] && [ "$scene" != "physics-stress" ]; then
    echo "--physics-sleep applies to physics-stress only, not '$scene'."
    exit 1
  fi
  if [ -n "$requested_ecs_density" ] && [ "$scene" != "ecs-high-load" ]; then
    echo "--ecs-density applies to ecs-high-load only."
    exit 1
  fi
  local total_frames=$((frames + WARMUP_FRAMES))
  local timestamp
  timestamp="$(date -u +%Y%m%dT%H%M%SZ)"
  local config_suffix
  config_suffix="$(capture_config_suffix "$requested_present_mode" "$requested_max_frame_latency" "$requested_stress_count" "$requested_physics_sleep" "$requested_ecs_density")"
  local out_dir="perf-runs/${timestamp}-${scene}"
  if [ -n "$config_suffix" ]; then
    out_dir="${out_dir}-${config_suffix}"
  fi
  mkdir -p "$out_dir"

  local pkg
  if ! pkg="$(resolve_scene_package "$scene")"; then
    echo "Unknown scene '$scene'. Expected: ecs-high-load, sprite-stress, physics-stress, or render-features."
    exit 1
  fi
  local -a scene_env=()
  mapfile -t scene_env < <(scene_env_overrides "$scene")

  # Provenance is read before the build so it describes the built sources.
  local git_commit
  git_commit="$(git_commit_label)"
  local git_dirty
  git_dirty="$(git_dirty_label)"
  local cpu_governor
  cpu_governor="$(file_value_or_na /sys/devices/system/cpu/cpu0/cpufreq/scaling_governor)"
  local platform_profile
  platform_profile="$(file_value_or_na /sys/firmware/acpi/platform_profile)"

  local build_rustflags
  build_rustflags="$(perf_rustflags)"
  if [ "$call_graph" = fp ] && ! frame_pointers_enabled "$build_rustflags"; then
    echo "--call-graph fp requires TUNGSTEN_PERF_RUSTFLAGS to include -C force-frame-pointers=yes"
    exit 1
  fi
  local rustc_version
  rustc_version="$(rustc --version 2>/dev/null || echo unknown)"
  echo "Building $pkg with RUSTFLAGS=\"$build_rustflags\"..."
  if ! RUSTFLAGS="$build_rustflags" cargo build --release -p "$pkg"; then
    echo "Build failed."
    exit 1
  fi

  local binary="target/release/$pkg"
  if [ ! -x "$binary" ]; then
    local alt_binary="target/release/${pkg//-/_}"
    if [ -x "$alt_binary" ]; then
      binary="$alt_binary"
    else
      echo "Could not find built binary for $pkg."
      exit 1
    fi
  fi

  echo "Output directory: $out_dir"

  local -a env_base=(
    env
    -u TUNGSTEN_GPU_TIMING
    -u TUNGSTEN_RENDER_PRESENT_MODE
    -u TUNGSTEN_RENDER_MAX_FRAME_LATENCY
    -u STRESS_SCENE
    -u STRESS_COUNT
    -u STRESS_PHYSICS_SLEEP
    -u STRESS_ECS_DENSITY
  )
  if [ -n "$requested_present_mode" ]; then
    env_base+=("TUNGSTEN_RENDER_PRESENT_MODE=$requested_present_mode")
  fi
  if [ -n "$requested_max_frame_latency" ]; then
    env_base+=("TUNGSTEN_RENDER_MAX_FRAME_LATENCY=$requested_max_frame_latency")
  fi
  if [ -n "$requested_stress_count" ]; then
    env_base+=("STRESS_COUNT=$requested_stress_count")
  fi
  if [ "$requested_physics_sleep" = "off" ]; then
    env_base+=("STRESS_PHYSICS_SLEEP=0")
  elif [ "$requested_physics_sleep" = "on" ]; then
    env_base+=("STRESS_PHYSICS_SLEEP=1")
  fi

  env_base+=("STRESS_ECS_DENSITY=${requested_ecs_density:-fixed}")

  local -a telemetry_env=(
    "${env_base[@]}"
    "${scene_env[@]}"
    "TUNGSTEN_SMOKE_FRAMES=$total_frames"
    "TUNGSTEN_PERF_LOG=1"
    "RUST_LOG=tungsten::app=debug"
  )
  local -a gpu_telemetry_env=(
    "${telemetry_env[@]}"
    "TUNGSTEN_GPU_TIMING=1"
  )
  local -a profile_env=(
    "${env_base[@]}"
    "${scene_env[@]}"
    "TUNGSTEN_SMOKE_FRAMES=$total_frames"
    "RUST_LOG=error"
  )

  local -a run_dirs=()
  local run
  if [ "$repeat" -eq 1 ]; then
    run_dirs=("$out_dir")
  else
    for ((run = 1; run <= repeat; run++)); do
      run_dirs+=("$out_dir/run-$run")
    done
  fi

  # Measured runs go back to back; profilers follow, so no measured run
  # starts right after a perf record or a flamegraph fold.
  local run_dir
  local status
  run=0
  for run_dir in "${run_dirs[@]}"; do
    run=$((run + 1))
    mkdir -p "$run_dir"
    echo "Capturing engine telemetry (run $run of $repeat)..."
    "${telemetry_env[@]}" "$binary" >"$run_dir/engine-telemetry.txt" 2>&1
    status=$?
    if [ "$status" -ne 0 ]; then
      echo "Engine telemetry capture failed."
      exit "$status"
    fi

    echo "Capturing GPU timing telemetry (run $run of $repeat)..."
    "${gpu_telemetry_env[@]}" "$binary" >"$run_dir/gpu-timing.txt" 2>&1
    status=$?
    if [ "$status" -ne 0 ]; then
      echo "GPU timing capture failed."
      exit "$status"
    fi
  done

  if [ "$telemetry_only" -eq 0 ]; then
    capture_profiles "${run_dirs[0]}"
  else
    echo "Telemetry-only mode: skipping flamegraph and perf captures."
  fi

  local capture_mode
  capture_mode="$(capture_mode_label "$telemetry_only")"
  if [ "$repeat" -gt 1 ] && [ "$telemetry_only" -eq 0 ]; then
    capture_mode="full (profilers on run-1 only)"
  fi
  run=0
  for run_dir in "${run_dirs[@]}"; do
    run=$((run + 1))
    local run_label="$run of $repeat"
    if [ "$telemetry_only" -eq 1 ]; then
      write_run_readme "$run_dir" 0 "telemetry-only" "--telemetry-only" "$run_label"
    elif [ "$run" -eq 1 ]; then
      write_run_readme "$run_dir" 1 "full" "" "$run_label"
    else
      write_run_readme "$run_dir" 0 "telemetry-only (repeat run)" "profilers run on run-1 only" "$run_label"
    fi
    write_run_metrics "$run_dir/engine-telemetry.txt" "$run_dir/metrics.tsv" "$run_dir/gpu-timing.txt"
  done

  if [ "$repeat" -gt 1 ]; then
    write_summary "$out_dir" "${run_dirs[@]}"
    echo "Summary: $out_dir/summary.md"
  fi

  echo "Capture complete."
}

if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then
  main "$@"
fi
