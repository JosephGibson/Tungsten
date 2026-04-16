#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
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
trap 'rm -f "$log_file"' EXIT

cat >"$log_file" <<'EOF'
[2026-04-15T18:46:51Z DEBUG tungsten::app] backend: Vulkan adapter: AMD Radeon 660M (RADV REMBRANDT) present_mode: Mailbox max_frame_latency: 3 timestamp_query: true
[2026-04-15T18:46:51Z DEBUG tungsten::app] frame: total=9.00ms update=0.00ms flush=0.00ms extract=0.00ms render=0.00ms render_acquire=0.90ms render_encode=0.00ms render_submit_present=0.00ms gpu=n/a audio=0.00ms hot_reload=0.00ms
[2026-04-15T18:46:51Z DEBUG tungsten::app] frame: total=8.00ms update=0.00ms flush=0.00ms extract=0.00ms render=0.00ms render_acquire=0.80ms render_encode=0.00ms render_submit_present=0.00ms gpu=n/a audio=0.00ms hot_reload=0.00ms
[2026-04-15T18:46:51Z DEBUG tungsten::app] frame: total=1.00ms update=0.00ms flush=0.00ms extract=0.00ms render=0.00ms render_acquire=0.10ms render_encode=0.00ms render_submit_present=0.00ms gpu=n/a audio=0.00ms hot_reload=0.00ms
[2026-04-15T18:46:51Z DEBUG tungsten::app] frame: total=2.00ms update=0.00ms flush=0.00ms extract=0.00ms render=0.00ms render_acquire=0.20ms render_encode=0.00ms render_submit_present=0.00ms gpu=n/a audio=0.00ms hot_reload=0.00ms
[2026-04-15T18:46:51Z DEBUG tungsten::app] frame: total=3.00ms update=0.00ms flush=0.00ms extract=0.00ms render=0.00ms render_acquire=0.30ms render_encode=0.00ms render_submit_present=0.00ms gpu=n/a audio=0.00ms hot_reload=0.00ms
[2026-04-15T18:46:51Z DEBUG tungsten::app] frame: total=4.00ms update=0.00ms flush=0.00ms extract=0.00ms render=0.00ms render_acquire=0.40ms render_encode=0.00ms render_submit_present=0.00ms gpu=n/a audio=0.00ms hot_reload=0.00ms
[2026-04-15T18:46:51Z DEBUG tungsten::app] frame: total=5.00ms update=0.00ms flush=0.00ms extract=0.00ms render=0.00ms render_acquire=0.50ms render_encode=0.00ms render_submit_present=0.00ms gpu=n/a audio=0.00ms hot_reload=0.00ms
EOF

parse_backend_metadata "$log_file"
assert_eq "Vulkan" "$METADATA_BACKEND" "backend"
assert_eq "AMD Radeon 660M (RADV REMBRANDT)" "$METADATA_ADAPTER" "adapter"
assert_eq "Mailbox" "$METADATA_PRESENT_MODE" "present mode"
assert_eq "3" "$METADATA_MAX_FRAME_LATENCY" "max frame latency"
assert_eq "true" "$METADATA_TIMESTAMP_QUERY" "timestamp query"
assert_eq "mailbox-lat3" "$(capture_config_suffix "mailbox" "3")" "config suffix"
assert_eq "full" "$(capture_mode_label 0)" "full capture mode label"
assert_eq "telemetry-only" "$(capture_mode_label 1)" "telemetry-only capture mode label"
assert_eq "mailbox" "$(requested_value_or_none "mailbox")" "requested value passthrough"
assert_eq "none" "$(requested_value_or_none "")" "requested value empty label"

assert_eq "3.00" "$(avg_metric "$log_file" "total")" "average total"
assert_eq "3.00" "$(percentile_metric "$log_file" "total" 50)" "p50 total"
assert_eq "5.00" "$(percentile_metric "$log_file" "total" 95)" "p95 total"
assert_eq "5.00" "$(percentile_metric "$log_file" "total" 99)" "p99 total"
assert_eq "0.30" "$(avg_metric "$log_file" "render_acquire")" "average acquire"
assert_eq "0.30" "$(percentile_metric "$log_file" "render_acquire" 50)" "p50 acquire"
assert_eq "0.50" "$(percentile_metric "$log_file" "render_acquire" 95)" "p95 acquire"
assert_eq "0.50" "$(percentile_metric "$log_file" "render_acquire" 99)" "p99 acquire"

echo "perf-capture helpers: OK"
