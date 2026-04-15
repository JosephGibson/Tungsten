#!/usr/bin/env bash

set -u

cd "$(dirname "$0")/.."

SCENE="${1:-sprite-stress}"
FRAMES="${2:-300}"
WARMUP_FRAMES=60
TOTAL_FRAMES=$((FRAMES + WARMUP_FRAMES))
TIMESTAMP="$(date -u +%Y%m%dT%H%M%SZ)"
OUT_DIR="perf-runs/${TIMESTAMP}-${SCENE}"
mkdir -p "$OUT_DIR"

case "$SCENE" in
  sprite-stress)
    PKG="example-02-sprite-stress"
    ;;
  platformer)
    PKG="example-01-platformer"
    ;;
  *)
    echo "Unknown scene '$SCENE'. Expected: sprite-stress or platformer."
    exit 1
    ;;
esac

echo "Building $PKG with frame pointers..."
if ! RUSTFLAGS="-C force-frame-pointers=yes" cargo build --release -p "$PKG"; then
  echo "Build failed."
  exit 1
fi

BINARY="target/release/$PKG"
if [ ! -x "$BINARY" ]; then
  ALT_BINARY="target/release/${PKG//-/_}"
  if [ -x "$ALT_BINARY" ]; then
    BINARY="$ALT_BINARY"
  else
    echo "Could not find built binary for $PKG."
    exit 1
  fi
fi

echo "Output directory: $OUT_DIR"

ENGINE_LOG="$OUT_DIR/engine-telemetry.txt"
GPU_LOG="$OUT_DIR/gpu-timing.txt"
FLAMEGRAPH_OUT="$OUT_DIR/flamegraph.svg"
PERF_STAT_OUT="$OUT_DIR/perf-stat.txt"
PERF_RECORD_OUT="$OUT_DIR/perf-record.data"
README_OUT="$OUT_DIR/README.md"

echo "Capturing engine telemetry..."
TUNGSTEN_SMOKE_FRAMES="$TOTAL_FRAMES" \
TUNGSTEN_PERF_LOG=1 \
RUST_LOG=debug \
"$BINARY" >"$ENGINE_LOG" 2>&1
engine_status=$?
if [ "$engine_status" -ne 0 ]; then
  echo "Engine telemetry capture failed."
  exit "$engine_status"
fi

echo "Capturing GPU timing telemetry..."
TUNGSTEN_SMOKE_FRAMES="$TOTAL_FRAMES" \
TUNGSTEN_PERF_LOG=1 \
TUNGSTEN_GPU_TIMING=1 \
RUST_LOG=debug \
"$BINARY" >"$GPU_LOG" 2>&1
gpu_status=$?
if [ "$gpu_status" -ne 0 ]; then
  echo "GPU timing capture failed."
  exit "$gpu_status"
fi

if cargo flamegraph --help >/dev/null 2>&1; then
  echo "Capturing flamegraph..."
  env TUNGSTEN_SMOKE_FRAMES="$TOTAL_FRAMES" RUST_LOG=error \
  cargo flamegraph \
    --package "$PKG" \
    --bin "$PKG" \
    --release \
    --output "$FLAMEGRAPH_OUT" \
    -- \
    >/dev/null 2>&1 || true
else
  echo "cargo-flamegraph not installed; skipping flamegraph capture."
fi

if command -v perf >/dev/null 2>&1; then
  echo "Capturing perf stat..."
  perf stat -d -o "$PERF_STAT_OUT" \
    env TUNGSTEN_SMOKE_FRAMES="$TOTAL_FRAMES" RUST_LOG=error "$BINARY" >/dev/null 2>&1 || true

  echo "Capturing perf record..."
  perf record --call-graph dwarf -o "$PERF_RECORD_OUT" \
    env TUNGSTEN_SMOKE_FRAMES="$TOTAL_FRAMES" RUST_LOG=error "$BINARY" >/dev/null 2>&1 || true
else
  echo "perf not installed; skipping perf captures."
fi

AVG_TOTAL_MS="$(
  awk '
    /frame:/ {
      seen += 1
      if (seen <= warmup) {
        next
      }
      for (i = 1; i <= NF; i++) {
        if ($i ~ /^total=/) {
          value = $i
          sub(/^total=/, "", value)
          sub(/ms$/, "", value)
          sum += value
          n += 1
        }
      }
    }
    END {
      if (n > 0) {
        printf "%.2f", sum / n
      } else {
        printf "n/a"
      }
    }
  ' warmup="$WARMUP_FRAMES" "$ENGINE_LOG"
)"

CPU_MODEL="$(grep -m1 'model name' /proc/cpuinfo 2>/dev/null | cut -d: -f2- | sed 's/^ //')"
GPU_INFO="$(grep -m1 'backend:' "$GPU_LOG" 2>/dev/null || true)"
HOST_KERNEL="$(uname -sr)"

cat >"$README_OUT" <<EOF
# Perf Capture: $SCENE

## Machine

| Field | Value |
| --- | --- |
| Kernel | ${HOST_KERNEL:-unknown} |
| CPU | ${CPU_MODEL:-unknown} |
| Binary | $BINARY |
| Backend env | ${WGPU_BACKEND:-auto} |

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
| Average total frame ms | $AVG_TOTAL_MS |
| GPU timing line | ${GPU_INFO:-not found} |
| Warm-up frames skipped | $WARMUP_FRAMES |
| Measured frames requested | $FRAMES |

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
- Compare like-for-like runs only: same scene, resolution, backend, and release build.
EOF

echo "Capture complete."
