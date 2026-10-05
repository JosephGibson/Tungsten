#!/usr/bin/env bash
# The outside-copy check (D-123), behind `just template-check`, one of the
# release checks: copies templates/basic to a fresh folder outside the
# repository, as a game starts out, then builds, tests and smoke-runs it there
# against this checkout's engine.
#
# The copy builds into this checkout's target folder with the workspace's
# profiles and .cargo/config.toml, offline, so most artifacts are shared; a
# free-space floor on that folder's filesystem runs first (workflow §6.5).
# Needs a GPU and display for the smoke run.
#
# Env vars:
#   TUNGSTEN_TEMPLATE_MIN_FREE_GB  Free-space floor in GiB (default: 3)
#   TUNGSTEN_SMOKE_TIMEOUT         Smoke-run timeout in seconds (default: 90)
#   TMPDIR                         Where mktemp puts the copy

set -euo pipefail

repo="$(cd "$(dirname "$0")/.." && pwd)"
min_free_gb="${TUNGSTEN_TEMPLATE_MIN_FREE_GB:-3}"
timeout_secs="${TUNGSTEN_SMOKE_TIMEOUT:-90}"

mkdir -p "$repo/target"
free_kb="$(df -Pk "$repo/target" | awk 'NR == 2 { print $4 }')"
free_gib="$(awk -v kb="$free_kb" 'BEGIN { printf "%.1f", kb / 1048576 }')"
echo "Free space for $repo/target: $free_gib GiB (floor $min_free_gb GiB)"
if [ "$free_kb" -lt $((min_free_gb * 1048576)) ]; then
  echo "error: $free_gib GiB free is under the $min_free_gb GiB floor; delete target/debug/incremental or free space first" >&2
  exit 1
fi

copy_root="$(mktemp -d)"
trap 'rm -rf "$copy_root"' EXIT
copy="$copy_root/basic"
cp -R "$repo/templates/basic" "$copy"
echo "Copy: $copy"

# A game reads its files from its own folder: nothing it reads may sit in a
# folder above the copy, where a path that escapes the copy would find it.
dir="$copy_root"
while :; do
  for name in tungsten.json input.json assets; do
    if [ -e "$dir/$name" ]; then
      echo "error: $dir/$name sits above the copy; point TMPDIR at a folder outside any game" >&2
      exit 1
    fi
  done
  [ "$dir" = / ] && break
  dir="$(dirname "$dir")"
done
echo "Above the copy: no tungsten.json, input.json or assets/"

# The engine as this checkout's absolute path, the workspace's profiles, and
# its lock, so the copy resolves offline to the versions the engine tests.
manifest="$copy/Cargo.toml"
sed "s|\"\.\./\.\./crates/|\"$repo/crates/|g" "$manifest" >"$manifest.new"
mv "$manifest.new" "$manifest"
if relative="$(grep -nE '^[^#]*path *= *"[^/"]' "$manifest")"; then
  echo "error: the copy's Cargo.toml keeps a relative path:" >&2
  echo "$relative" >&2
  exit 1
fi
{
  echo
  echo "# The engine workspace's profiles."
  awk '/^\[/ { profile = ($0 ~ /^\[profile\./) } profile && NF && !/^#/' "$repo/Cargo.toml"
} >>"$manifest"
cp "$repo/Cargo.lock" "$copy/Cargo.lock"

in_copy() {
  (cd "$copy" && env CARGO_TARGET_DIR="$repo/target" "$@")
}
cargo_flags=(--offline --config "$repo/.cargo/config.toml")

if ! in_copy cargo build --quiet "${cargo_flags[@]}"; then
  echo "error: the copy does not build" >&2
  exit 1
fi
echo "Build: OK"
if ! in_copy cargo test --quiet "${cargo_flags[@]}"; then
  echo "error: the copy's tests fail" >&2
  exit 1
fi
echo "Test: OK"
log="$copy_root/smoke.log"
if ! in_copy TUNGSTEN_SMOKE_FRAMES=3 timeout --kill-after=10 "$timeout_secs" \
  cargo run --quiet "${cargo_flags[@]}" >"$log" 2>&1; then
  echo "error: the copy's smoke run failed (timeout ${timeout_secs}s); the end of its log:" >&2
  tail -20 "$log" >&2
  exit 1
fi
if grep -qF 'Unknown font ID' "$log"; then
  echo "error: the copy's smoke run logged an unknown font ID:" >&2
  grep -F 'Unknown font ID' "$log" >&2
  exit 1
fi
echo "Smoke (TUNGSTEN_SMOKE_FRAMES=3): OK"
echo "Template check passed: the copy builds, tests and smoke-runs outside the repository."
