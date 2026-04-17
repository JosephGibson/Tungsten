#!/usr/bin/env bash

set -u

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

: "${WARMUP_FRAMES:=60}"

METADATA_BACKEND="unknown"
METADATA_ADAPTER="unknown"
METADATA_PRESENT_MODE="unknown"
METADATA_MAX_FRAME_LATENCY="unknown"
METADATA_TIMESTAMP_QUERY="unknown"

usage() {
  cat <<'EOF'
Usage: perf-capture.sh [scene] [frames] [--present-mode <mode>] [--max-frame-latency <n>] [--telemetry-only]

Scenes:
  sprite-stress
  platformer

Flags:
  --present-mode <mode>       Override render.present_mode for child capture runs
  --max-frame-latency <n>     Override render.max_frame_latency for child capture runs
  --telemetry-only            Skip flamegraph/perf artifact capture; still writes telemetry logs and README
EOF
}

metric_samples() {
  local file="$1"
  local key="$2"
  awk -v warmup="$WARMUP_FRAMES" -v key="$key" '
    /frame:/ {
      seen += 1
      if (seen <= warmup) {
        next
      }
      for (i = 1; i <= NF; i++) {
        prefix = key "="
        if (index($i, prefix) == 1) {
          value = $i
          sub("^" prefix, "", value)
          sub(/ms$/, "", value)
          if (value != "n/a") {
            print value
          }
        }
      }
    }
  ' "$file"
}

avg_metric() {
  local file="$1"
  local key="$2"
  metric_samples "$file" "$key" | awk '
    {
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

percentile_metric() {
  local file="$1"
  local key="$2"
  local percentile="$3"
  metric_samples "$file" "$key" | sort -g | awk -v percentile="$percentile" '
    {
      values[NR] = $1
    }
    END {
      if (NR == 0) {
        printf "n/a"
        exit
      }
      rank = int((percentile * NR + 99) / 100)
      if (rank < 1) {
        rank = 1
      }
      if (rank > NR) {
        rank = NR
      }
      printf "%.2f", values[rank]
    }
  '
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
  local -a parts=()

  if [ -n "$present_mode_override" ]; then
    parts+=("$present_mode_override")
  fi
  if [ -n "$max_frame_latency_override" ]; then
    parts+=("lat${max_frame_latency_override}")
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

main() {
  cd "$REPO_ROOT"

  local scene=""
  local frames=""
  local requested_present_mode=""
  local requested_max_frame_latency=""
  local telemetry_only=0

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

  scene="${scene:-sprite-stress}"
  frames="${frames:-300}"
  local total_frames=$((frames + WARMUP_FRAMES))
  local timestamp
  timestamp="$(date -u +%Y%m%dT%H%M%SZ)"
  local config_suffix
  config_suffix="$(capture_config_suffix "$requested_present_mode" "$requested_max_frame_latency")"
  local out_dir="perf-runs/${timestamp}-${scene}"
  if [ -n "$config_suffix" ]; then
    out_dir="${out_dir}-${config_suffix}"
  fi
  mkdir -p "$out_dir"

  local pkg
  case "$scene" in
    sprite-stress)
      pkg="example-02-sprite-stress"
      ;;
    platformer)
      pkg="example-01-platformer"
      ;;
    *)
      echo "Unknown scene '$scene'. Expected: sprite-stress or platformer."
      exit 1
      ;;
  esac

  echo "Building $pkg with frame pointers..."
  if ! RUSTFLAGS="-C force-frame-pointers=yes" cargo build --release -p "$pkg"; then
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

  local engine_log="$out_dir/engine-telemetry.txt"
  local gpu_log="$out_dir/gpu-timing.txt"
  local flamegraph_out="$out_dir/flamegraph.svg"
  local perf_stat_out="$out_dir/perf-stat.txt"
  local perf_record_out="$out_dir/perf-record.data"
  local readme_out="$out_dir/README.md"

  local -a env_base=(
    env
    -u TUNGSTEN_RENDER_PRESENT_MODE
    -u TUNGSTEN_RENDER_MAX_FRAME_LATENCY
  )
  if [ -n "$requested_present_mode" ]; then
    env_base+=("TUNGSTEN_RENDER_PRESENT_MODE=$requested_present_mode")
  fi
  if [ -n "$requested_max_frame_latency" ]; then
    env_base+=("TUNGSTEN_RENDER_MAX_FRAME_LATENCY=$requested_max_frame_latency")
  fi

  local -a telemetry_env=(
    "${env_base[@]}"
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
    "TUNGSTEN_SMOKE_FRAMES=$total_frames"
    "RUST_LOG=error"
  )

  echo "Capturing engine telemetry..."
  "${telemetry_env[@]}" "$binary" >"$engine_log" 2>&1
  local engine_status=$?
  if [ "$engine_status" -ne 0 ]; then
    echo "Engine telemetry capture failed."
    exit "$engine_status"
  fi

  echo "Capturing GPU timing telemetry..."
  "${gpu_telemetry_env[@]}" "$binary" >"$gpu_log" 2>&1
  local gpu_status=$?
  if [ "$gpu_status" -ne 0 ]; then
    echo "GPU timing capture failed."
    exit "$gpu_status"
  fi

  if [ "$telemetry_only" -eq 0 ]; then
    if cargo flamegraph --help >/dev/null 2>&1; then
      echo "Capturing flamegraph..."
      "${profile_env[@]}" \
      cargo flamegraph \
        --package "$pkg" \
        --bin "$pkg" \
        --release \
        --output "$flamegraph_out" \
        -- \
        >/dev/null 2>&1 || true
    else
      echo "cargo-flamegraph not installed; skipping flamegraph capture."
    fi

    if command -v perf >/dev/null 2>&1; then
      echo "Capturing perf stat..."
      "${profile_env[@]}" perf stat -d -o "$perf_stat_out" "$binary" >/dev/null 2>&1 || true

      echo "Capturing perf record..."
      "${profile_env[@]}" perf record --call-graph dwarf -o "$perf_record_out" "$binary" >/dev/null 2>&1 || true
    else
      echo "perf not installed; skipping perf captures."
    fi
  else
    echo "Telemetry-only mode: skipping flamegraph and perf captures."
  fi

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

  local avg_render_encode_ms
  avg_render_encode_ms="$(avg_metric "$engine_log" "render_encode")"
  local avg_render_submit_ms
  avg_render_submit_ms="$(avg_metric "$engine_log" "render_submit_present")"
  local avg_gpu_ms
  avg_gpu_ms="$(avg_metric "$gpu_log" "gpu")"

  local cpu_model
  cpu_model="$(grep -m1 'model name' /proc/cpuinfo 2>/dev/null | cut -d: -f2- | sed 's/^ //')"
  local host_kernel
  host_kernel="$(uname -sr)"
  local capture_mode
  capture_mode="$(capture_mode_label "$telemetry_only")"
  local requested_present_mode_label
  requested_present_mode_label="$(requested_value_or_none "$requested_present_mode")"
  local requested_max_frame_latency_label
  requested_max_frame_latency_label="$(requested_value_or_none "$requested_max_frame_latency")"
  local flamegraph_note
  local perf_stat_note
  local perf_record_note

  if [ "$telemetry_only" -eq 1 ]; then
    flamegraph_note="Skipped (--telemetry-only)"
    perf_stat_note="Skipped (--telemetry-only)"
    perf_record_note="Skipped (--telemetry-only)"
  else
    if [ -f "$flamegraph_out" ]; then
      flamegraph_note="Captured"
    elif cargo flamegraph --help >/dev/null 2>&1; then
      flamegraph_note="Skipped (capture failed)"
    else
      flamegraph_note="Skipped (cargo-flamegraph not installed)"
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

  cat >"$readme_out" <<EOF
# Perf Capture: $scene

## Machine

| Field | Value |
| --- | --- |
| Kernel | ${host_kernel:-unknown} |
| CPU | ${cpu_model:-unknown} |
| Binary | $binary |
| Backend env | ${WGPU_BACKEND:-auto} |
| Capture mode | ${capture_mode} |
| Requested present mode override | ${requested_present_mode_label} |
| Requested max frame latency override | ${requested_max_frame_latency_label} |
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
| Average render encode ms | $avg_render_encode_ms |
| Average render submit/present ms | $avg_render_submit_ms |
| Average GPU frame ms | $avg_gpu_ms |
| Warm-up frames skipped | $WARMUP_FRAMES |
| Measured frames requested | $frames |

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
- Flamegraph and perf captures intentionally run without \`TUNGSTEN_GPU_TIMING\` to avoid the blocking timestamp readback stall.
- Compare like-for-like runs only: same scene, resolution, backend, release build, present mode, and max frame latency.
EOF

  echo "Capture complete."
}

if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then
  main "$@"
fi
