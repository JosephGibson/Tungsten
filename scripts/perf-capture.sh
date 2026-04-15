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

main() {
  cd "$REPO_ROOT"

  local scene="${1:-sprite-stress}"
  local frames="${2:-300}"
  local total_frames=$((frames + WARMUP_FRAMES))
  local timestamp
  timestamp="$(date -u +%Y%m%dT%H%M%SZ)"
  local out_dir="perf-runs/${timestamp}-${scene}"
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

  echo "Capturing engine telemetry..."
  TUNGSTEN_SMOKE_FRAMES="$total_frames" \
  TUNGSTEN_PERF_LOG=1 \
  RUST_LOG=tungsten::app=debug \
  "$binary" >"$engine_log" 2>&1
  local engine_status=$?
  if [ "$engine_status" -ne 0 ]; then
    echo "Engine telemetry capture failed."
    exit "$engine_status"
  fi

  echo "Capturing GPU timing telemetry..."
  TUNGSTEN_SMOKE_FRAMES="$total_frames" \
  TUNGSTEN_PERF_LOG=1 \
  TUNGSTEN_GPU_TIMING=1 \
  RUST_LOG=tungsten::app=debug \
  "$binary" >"$gpu_log" 2>&1
  local gpu_status=$?
  if [ "$gpu_status" -ne 0 ]; then
    echo "GPU timing capture failed."
    exit "$gpu_status"
  fi

  if cargo flamegraph --help >/dev/null 2>&1; then
    echo "Capturing flamegraph..."
    env TUNGSTEN_SMOKE_FRAMES="$total_frames" RUST_LOG=error \
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
    perf stat -d -o "$perf_stat_out" \
      env TUNGSTEN_SMOKE_FRAMES="$total_frames" RUST_LOG=error "$binary" >/dev/null 2>&1 || true

    echo "Capturing perf record..."
    perf record --call-graph dwarf -o "$perf_record_out" \
      env TUNGSTEN_SMOKE_FRAMES="$total_frames" RUST_LOG=error "$binary" >/dev/null 2>&1 || true
  else
    echo "perf not installed; skipping perf captures."
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

  cat >"$readme_out" <<EOF
# Perf Capture: $scene

## Machine

| Field | Value |
| --- | --- |
| Kernel | ${host_kernel:-unknown} |
| CPU | ${cpu_model:-unknown} |
| Binary | $binary |
| Backend env | ${WGPU_BACKEND:-auto} |
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
| \`flamegraph.svg\` | Present when cargo-flamegraph is installed |
| \`perf-stat.txt\` | Present when \`perf stat\` succeeded |
| \`perf-record.data\` | Present when \`perf record\` succeeded |

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

- Flamegraph and perf captures intentionally run without \`TUNGSTEN_GPU_TIMING\` to avoid the blocking timestamp readback stall.
- Compare like-for-like runs only: same scene, resolution, backend, release build, present mode, and max frame latency.
EOF

  echo "Capture complete."
}

if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then
  main "$@"
fi
