#!/usr/bin/env bash
# The outside-copy check (D-123, D-183), behind `just template-check`, one of
# the release checks: `tungsten new` makes a game in a fresh folder outside
# the repository, as a game starts out, and the command line checks it, it
# builds, tests, smoke-runs, describes itself and runs a script there against
# this checkout's engine, and `tungsten package` archives it; the archive's
# binary smoke-runs too. Then two entries that leave their manifest's folder
# and seven seeded errors must fail as they should (RC-A7).
#
# The copy builds into this checkout's target folder with the workspace's
# profiles (which `new` writes) and .cargo/config.toml, offline, so most
# artifacts are shared; the package's portable release build keeps a folder
# of its own, target/package-check, so target/release (`just perf`) is never
# rebuilt with other flags. A free-space floor on that folder's filesystem
# runs first (workflow §6.5). Needs a GPU and display for the smoke runs.
#
# Env vars:
#   TUNGSTEN_TEMPLATE_MIN_FREE_GB  Free-space floor in GiB (default: 6)
#   TUNGSTEN_SMOKE_TIMEOUT         Smoke-run timeout in seconds (default: 90)
#   TMPDIR                         Where mktemp puts the copy

set -euo pipefail

repo="$(cd "$(dirname "$0")/.." && pwd)"
min_free_gb="${TUNGSTEN_TEMPLATE_MIN_FREE_GB:-6}"
timeout_secs="${TUNGSTEN_SMOKE_TIMEOUT:-90}"
version="$(python3 -B "$repo/scripts/release.py" version)"

mkdir -p "$repo/target"
free_kb="$(df -Pk "$repo/target" | awk 'NR == 2 { print $4 }')"
free_gib="$(awk -v kb="$free_kb" 'BEGIN { printf "%.1f", kb / 1048576 }')"
echo "Free space for $repo/target: $free_gib GiB (floor $min_free_gb GiB)"
if [ "$free_kb" -lt $((min_free_gb * 1048576)) ]; then
  echo "error: $free_gib GiB free is under the $min_free_gb GiB floor; delete target/debug/incremental or free space first" >&2
  exit 1
fi

# The command line, built from this checkout, and the folders its cargo
# runs share: this checkout's target folder, offline.
export CARGO_TARGET_DIR="$repo/target"
export CARGO_NET_OFFLINE=true
(cd "$repo" && cargo build --quiet --locked -p tungsten-cli)
tungsten="$repo/target/debug/tungsten"

copy_root="$(mktemp -d)"
trap 'rm -rf "$copy_root"' EXIT
name="outside-game"
copy="$copy_root/$name"
"$tungsten" new "$copy" >/dev/null

# A game reads its files from its own folder: nothing it reads may sit in a
# folder above the copy, where a path that escapes the copy would find it.
dir="$copy_root"
while :; do
  for file in tungsten.json input.json assets; do
    if [ -e "$dir/$file" ]; then
      echo "error: $dir/$file sits above the copy; point TMPDIR at a folder outside any game" >&2
      exit 1
    fi
  done
  [ "$dir" = / ] && break
  dir="$(dirname "$dir")"
done
echo "Copy (tungsten new, nothing above it): OK $copy"

# `new` pins the engine to this version's tag; the check points both pins
# at this checkout, which the tag does not exist for until it is pushed.
manifest="$copy/Cargo.toml"
pins="$(grep -cF "tag = \"v$version\"" "$manifest" || true)"
if [ "$pins" != 2 ]; then
  echo "error: the copy's Cargo.toml pins tag v$version $pins times, not twice:" >&2
  grep -n 'tungsten = ' "$manifest" >&2 || true
  exit 1
fi
sed "s|git = \"https://github.com/JosephGibson/Tungsten\", tag = \"v$version\"|path = \"$repo/crates/tungsten\"|" \
  "$manifest" >"$manifest.new"
mv "$manifest.new" "$manifest"
if left="$(grep -n 'git = ' "$manifest")"; then
  echo "error: the copy's Cargo.toml keeps a git dependency:" >&2
  echo "$left" >&2
  exit 1
fi
mkdir -p "$copy/.cargo"
cp "$repo/.cargo/config.toml" "$copy/.cargo/config.toml"
echo "Pin (tag v$version, twice): OK"

in_copy() {
  (cd "$copy" && "$@")
}
# Every package run, failing ones too: no shell flags, its own folder.
package() {
  in_copy env -u RUSTFLAGS -u CARGO_ENCODED_RUSTFLAGS \
    CARGO_TARGET_DIR="$repo/target/package-check" "$tungsten" package
}
# Runs a JSON edit (python on stdin) with the copy, then this call's
# arguments, as its arguments.
edit() {
  python3 -I - "$copy" "$@"
}

out="$copy_root/out.txt"
if ! in_copy "$tungsten" check >"$out" 2>&1 || [ "$(tail -1 "$out")" != "tungsten check: no errors" ]; then
  echo "error: tungsten check on the clean copy failed:" >&2
  tail -20 "$out" >&2
  exit 1
fi
echo "Clean check: OK"

if ! in_copy cargo build --quiet; then
  echo "error: the copy does not build" >&2
  exit 1
fi
echo "Build: OK"
if ! in_copy cargo test --quiet; then
  echo "error: the copy's tests fail" >&2
  exit 1
fi
echo "Test: OK"
log="$copy_root/smoke.log"
if ! in_copy env TUNGSTEN_SMOKE_FRAMES=3 timeout --kill-after=10 "$timeout_secs" \
  cargo run --quiet >"$log" 2>&1; then
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

# `describe` and `run --script` (redo 7.2) on the copy: the description
# names the engine's probes, and a 30-frame script captures one frame and
# passes one assert, its report written beside the copy.
describe="$copy_root/describe.json"
if ! in_copy "$tungsten" describe --json >"$describe" 2>"$out" \
  || ! python3 -I -c 'import json, sys; d = json.load(open(sys.argv[1])); sys.exit(0 if d["components"] and any(p["name"] == "entities" for p in d["script"]["probes"]) else 1)' "$describe"; then
  echo "error: tungsten describe on the copy failed or named no components or probes:" >&2
  tail -20 "$out" >&2
  exit 1
fi
echo "Describe (components, the entities probe): OK"
script="$copy_root/script.json"
shot="$copy_root/script-shot.png"
report="$copy_root/script-report.json"
cat >"$script" <<JSON
{ "version": 1, "frames": 30, "steps": [
  { "frame": 20, "capture": "$shot" },
  { "frame": 30, "assert": { "probe": "frame", "op": "==", "value": 30 } }
] }
JSON
if ! in_copy timeout --kill-after=10 "$timeout_secs" "$tungsten" run --script "$script" --report "$report" >"$out" 2>&1 \
  || [ "$(tail -1 "$out")" != "tungsten run: 30 frames, 1 capture, 1 assert: passed" ] || [ ! -s "$shot" ]; then
  echo "error: tungsten run --script on the copy failed:" >&2
  tail -20 "$out" >&2
  exit 1
fi
echo "Run --script (30 frames, one capture, one assert): OK"

if ! package >"$out" 2>&1; then
  echo "error: tungsten package on the copy failed:" >&2
  tail -20 "$out" >&2
  exit 1
fi
top="$name-0.1.0-x86_64-unknown-linux-gnu"
archive="$copy/dist/$top.tar.gz"
debug_archive="$copy/dist/$top-debug.tar.gz"
listing="$(tar -tzf "$archive")"
for file in "$name" tungsten.json input.json assets/manifest.json THIRD-PARTY-NOTICES.txt \
  THIRD-PARTY-NOTICES-rust-std.html; do
  if ! grep -qxF "$top/$file" <<<"$listing"; then
    echo "error: the player archive lacks $top/$file" >&2
    exit 1
  fi
done
if grep -q '\.debug$' <<<"$listing"; then
  echo "error: the player archive holds a .debug file" >&2
  exit 1
fi
if [ "$(tar -tzf "$debug_archive" | grep -v '/$')" != "$top/$name.debug" ]; then
  echo "error: the debug archive does not hold $top/$name.debug alone:" >&2
  tar -tzf "$debug_archive" >&2
  exit 1
fi
extract="$copy_root/extract"
mkdir -p "$extract"
tar -xzf "$archive" -C "$extract"
tar -xzf "$debug_archive" -C "$extract"
notices="$extract/$top/THIRD-PARTY-NOTICES.txt"
if ! grep -qx "tungsten $version" "$notices" || ! grep -qF 'JetBrains Mono' "$notices"; then
  echo "error: the notices list neither tungsten $version nor JetBrains Mono" >&2
  exit 1
fi
echo "Package (both archives, the notices): OK"
log="$copy_root/archive-smoke.log"
if ! (cd "$extract/$top" && TUNGSTEN_SMOKE_FRAMES=3 timeout --kill-after=10 "$timeout_secs" \
  "./$name" >"$log" 2>&1); then
  echo "error: the archive's binary failed its smoke run; the end of its log:" >&2
  tail -20 "$log" >&2
  exit 1
fi
echo "Archive smoke (TUNGSTEN_SMOKE_FRAMES=3): OK"

# Two entries that leave their manifest's folder, one climbing out with `..`
# and one absolute, to files inside the game's folder: check and package
# each refuse them, naming the manifest (D-224).
# Runs a step (its name, the entry, then the command), which must fail
# naming the entry; the report writes paths from the game's folder, so an
# absolute entry reads relative.
refused() {
  local step=$1 entry=$2
  shift 2
  if "$@" >"$out" 2>&1; then
    echo "error: tungsten $step passed a sprite at $entry, outside its manifest's folder" >&2
    exit 1
  fi
  if ! grep -q "^assets/manifest\.json: sprites\.stray names '[^']*', which is absolute or leaves its manifest's folder" "$out"; then
    echo "error: tungsten $step did not name assets/manifest.json for $entry:" >&2
    tail -20 "$out" >&2
    exit 1
  fi
}
cp "$copy/assets/sprites/player.png" "$copy/outside.png"
for entry in "../outside.png" "$copy/assets/sprites/player.png"; do
  edit "$entry" <<'EOF'
import json, sys, pathlib
manifest = pathlib.Path(sys.argv[1]) / "assets/manifest.json"
data = json.loads(manifest.read_text())
data["sprites"]["stray"] = {"path": sys.argv[2]}
manifest.write_text(json.dumps(data, indent=2))
EOF
  refused check "$entry" in_copy "$tungsten" check
  refused package "$entry" package
  edit <<'EOF'
import json, sys, pathlib
manifest = pathlib.Path(sys.argv[1]) / "assets/manifest.json"
data = json.loads(manifest.read_text())
del data["sprites"]["stray"]
manifest.write_text(json.dumps(data, indent=2))
EOF
done
rm "$copy/outside.png"
echo "Paths leaving the manifest's folder (../outside.png, an absolute path): OK"

# Five content errors, one file each.
edit <<'EOF'
import json, sys, pathlib
root = pathlib.Path(sys.argv[1])
def change(rel, update):
    path = root / rel
    data = json.loads(path.read_text())
    update(data)
    path.write_text(json.dumps(data, indent=2))
change("assets/manifest.json", lambda m: m["sprites"].update(ghost={"path": "sprites/ghost.png"}))
change("assets/prefabs/gem.json", lambda p: p["components"].update(no_such_component={}))
change("assets/prefabs/hazard.json", lambda p: p["sprite"].update(asset_id="nobody"))
(root / "assets/data/cue_rules/labels.json").write_text("not json\n")
(root / "assets/broken_scene.json").write_text(json.dumps(
    {"entities": [{"transform": {"position": [0, 0]}, "components": {"no_such_component": {}}}]}))
EOF
if in_copy "$tungsten" check >"$out" 2>&1; then
  echo "error: tungsten check passed five seeded content errors" >&2
  exit 1
fi
for file in assets/manifest.json assets/prefabs/gem.json assets/prefabs/hazard.json \
  assets/data/cue_rules/labels.json assets/broken_scene.json; do
  if ! grep -q "^$file: " "$out"; then
    echo "error: tungsten check named no error in $file:" >&2
    cat "$out" >&2
    exit 1
  fi
done
if [ "$(tail -1 "$out")" != "tungsten check: 5 errors in 5 files" ]; then
  echo "error: tungsten check's summary is not '5 errors in 5 files':" >&2
  cat "$out" >&2
  exit 1
fi
echo "Five seeded content errors: OK"

# Two config errors: the content stage is skipped.
edit <<'EOF'
import json, sys, pathlib
root = pathlib.Path(sys.argv[1])
config = json.loads((root / "tungsten.json").read_text())
config.setdefault("render", {})["msaa"] = 3
(root / "tungsten.json").write_text(json.dumps(config, indent=2))
(root / "input.json").write_text("not json\n")
EOF
if in_copy "$tungsten" check >"$out" 2>&1; then
  echo "error: tungsten check passed two seeded config errors" >&2
  exit 1
fi
if [ "$(grep -c '^tungsten.json: \|^input.json: ' "$out")" != 2 ] ||
  [ "$(tail -1 "$out")" != "tungsten check: 2 errors; the game's assets are checked once tungsten.json and input.json load" ]; then
  echo "error: tungsten check did not name both config files and skip the assets:" >&2
  cat "$out" >&2
  exit 1
fi
echo "Two seeded config errors: OK"
echo "Template check passed: tungsten new's game checks, builds, tests, smoke-runs and packages outside the repository, and check and package fail where they should."
