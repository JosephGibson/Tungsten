#!/usr/bin/env bash
# Writes each library crate's public API to api/<crate>.txt (D-104, D-107):
# rustdoc JSON from the pinned toolchain in rust-toolchain.toml, with
# RUSTC_BOOTSTRAP=1 for that one rustdoc call, listed by cargo-public-api.
# The umbrella's call enables its `testing` feature, so the headless harness
# is listed too (D-110).
# --check writes to target/public-api/check/ instead and fails when a tracked
# file differs, so the release checks catch a stale snapshot.
#
# Requires: cargo install --locked cargo-public-api@0.52.0
# A pin bump that changes the rustdoc JSON format stops here: list the crates
# with the new format, confirm the output still parses as before, then raise
# FORMAT_VERSION with a decision (D-107).

set -euo pipefail

cd "$(dirname "$0")/.." || exit 1

TOOL_VERSION="0.52.0"
FORMAT_VERSION=60
CRATES=(tungsten-core tungsten-render tungsten)
# Its own target dir, so the bootstrap flag never touches the debug or perf builds.
TARGET="target/public-api"

case "${1:-}" in
  "") out="api" ;;
  --check) out="$TARGET/check" ;;
  *) echo "usage: $0 [--check]" >&2; exit 2 ;;
esac

if ! version="$(cargo public-api --version 2>/dev/null)"; then
  echo "cargo-public-api is missing: cargo install --locked cargo-public-api@$TOOL_VERSION" >&2
  exit 1
fi
if [[ "$version" != "cargo-public-api $TOOL_VERSION" ]]; then
  echo "need cargo-public-api $TOOL_VERSION, found '$version'" >&2
  exit 1
fi
toolchain="$(sed -n 's/^channel = "\(.*\)"$/\1/p' rust-toolchain.toml)"
if [[ -z "$toolchain" ]]; then
  echo "no channel in rust-toolchain.toml" >&2
  exit 1
fi

mkdir -p "$out"
stale=0
for crate in "${CRATES[@]}"; do
  json="$TARGET/doc/${crate//-/_}.json"
  features=()
  if [[ "$crate" == tungsten ]]; then
    features=(--features testing)
  fi
  RUSTC_BOOTSTRAP=1 CARGO_TARGET_DIR="$TARGET" cargo +"$toolchain" rustdoc -q --locked \
    -p "$crate" "${features[@]}" --lib -- -Z unstable-options --output-format json
  format="$(python3 -c 'import json, sys; print(json.load(open(sys.argv[1]))["format_version"])' "$json")"
  if [[ "$format" != "$FORMAT_VERSION" ]]; then
    echo "$crate: rustdoc JSON format $format, expected $FORMAT_VERSION (see this script's header)" >&2
    exit 1
  fi
  # -s omits blanket impls only: auto-trait and derived impls are semver surface.
  cargo public-api -s --color never --rustdoc-json "$json" > "$out/$crate.txt"
  if [[ "$out" != "api" ]] && ! cmp -s "$out/$crate.txt" "api/$crate.txt"; then
    echo "api/$crate.txt is stale; run 'just api' and review its diff:" >&2
    diff -u "api/$crate.txt" "$out/$crate.txt" | head -40 >&2 || true
    stale=1
  fi
done
if [[ "$out" == "api" ]]; then
  printf 'Wrote api/%s.txt\n' "${CRATES[@]}"
elif [[ "$stale" == 0 ]]; then
  echo "Public API snapshots current: ${#CRATES[@]} crates"
fi
exit "$stale"
