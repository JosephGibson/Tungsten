#!/usr/bin/env bash
# Regression tests for scripts/smoke-examples.sh with a stubbed `cargo` on
# PATH: example discovery failures, child failure, timeout, and the exact
# fixture matrix. Needs no GPU; real jq/timeout are used.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SMOKE="$SCRIPT_DIR/smoke-examples.sh"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
mkdir -p "$work/bin" "$work/tmp"

cat >"$work/bin/cargo" <<'EOF'
#!/usr/bin/env bash
# Stub cargo. STUB_METADATA: ok | fail | badjson | none.
# STUB_FAIL_PKG / STUB_HANG_PKG pick an example that panics / hangs;
# STUB_FAIL_MSAA makes matrix rows with that TUNGSTEN_RENDER_MSAA fail.
case "$1" in
  metadata)
    case "${STUB_METADATA:-ok}" in
      fail) echo "error: stub metadata failure" >&2; exit 101 ;;
      badjson) echo '{"packages": [' ;;
      none) echo '{"packages":[{"name":"tungsten-core"},{"name":"tungsten"}]}' ;;
      *) echo '{"packages":[{"name":"tungsten"},{"name":"example-02-sprite-stress"},{"name":"example-01-platformer"},{"name":"example-04-shader-playground"},{"name":"example-03-scene-state"}]}' ;;
    esac
    ;;
  build) exit 0 ;;
  run)
    pkg="$3"
    echo "$pkg msaa=${TUNGSTEN_RENDER_MSAA:-} sort=${TUNGSTEN_RENDER_DEPTH_SORT:-} post=${TUNGSTEN_POST_STACK_FIXTURE:-} aa=${TUNGSTEN_POST_AA_FIXTURE:-} bloom=${TUNGSTEN_BLOOM_FIXTURE:-} light=${TUNGSTEN_LIGHTING_FIXTURE:-} frames=${TUNGSTEN_SMOKE_FRAMES:-}" >>"$STUB_RUNS"
    if [ "$pkg" = "${STUB_HANG_PKG:-}" ]; then exec sleep 30; fi
    if [ "$pkg" = "${STUB_FAIL_PKG:-}" ]; then echo "thread 'main' panicked at stub"; exit 101; fi
    if [ -n "${STUB_FAIL_MSAA:-}" ] && [ "${TUNGSTEN_RENDER_MSAA:-}" = "$STUB_FAIL_MSAA" ]; then exit 3; fi
    exit 0
    ;;
  *) echo "stub cargo: unexpected command $*" >&2; exit 99 ;;
esac
EOF
chmod +x "$work/bin/cargo"

failures=0
out="$work/out.txt"

# run_case <name> <expected exit: 0|nonzero> [VAR=value ...]
run_case() {
  local name="$1" expect="$2"
  shift 2
  : >"$work/runs.txt"
  local code=0
  env PATH="$work/bin:$PATH" TMPDIR="$work/tmp" STUB_RUNS="$work/runs.txt" \
    TUNGSTEN_SMOKE_TIMEOUT=2 "$@" bash "$SMOKE" >"$out" 2>&1 || code=$?
  if { [ "$expect" = 0 ] && [ "$code" -ne 0 ]; } || { [ "$expect" = nonzero ] && [ "$code" -eq 0 ]; }; then
    echo "FAIL $name: exit $code, expected $expect"
    sed 's/^/    /' "$out"
    failures=$((failures + 1))
    return 1
  fi
}

# expect_output <name> <fixed string>
expect_output() {
  if ! grep -qF -- "$2" "$out"; then
    echo "FAIL $1: output lacks '$2'"
    sed 's/^/    /' "$out"
    failures=$((failures + 1))
  fi
}

if run_case "all pass" 0; then
  for line in "Passed: 4/4" "Matrix passed: 4/4" "Post-stack passed: 2/2" \
    "Post-AA passed: 1/1" "Bloom passed: 1/1" "Lighting passed: 1/1"; do
    expect_output "all pass" "$line"
  done
  expected_runs="$work/expected-runs.txt"
  cat >"$expected_runs" <<'EOF'
example-01-platformer msaa= sort= post= aa= bloom= light= frames=3
example-02-sprite-stress msaa= sort= post= aa= bloom= light= frames=3
example-03-scene-state msaa= sort= post= aa= bloom= light= frames=3
example-04-shader-playground msaa= sort= post= aa= bloom= light= frames=3
example-02-sprite-stress msaa=1 sort=cpu_stable post= aa= bloom= light= frames=3
example-02-sprite-stress msaa=1 sort=gpu_depth post= aa= bloom= light= frames=3
example-02-sprite-stress msaa=4 sort=cpu_stable post= aa= bloom= light= frames=3
example-02-sprite-stress msaa=4 sort=gpu_depth post= aa= bloom= light= frames=3
example-04-shader-playground msaa= sort= post=empty aa= bloom= light= frames=3
example-04-shader-playground msaa= sort= post=all aa= bloom= light= frames=3
example-04-shader-playground msaa= sort= post=empty aa=smaa_high bloom= light= frames=3
example-04-shader-playground msaa= sort= post=bloom_only aa= bloom=on light= frames=3
example-01-platformer msaa= sort= post= aa= bloom= light=on frames=3
EOF
  if ! diff -u "$expected_runs" "$work/runs.txt"; then
    echo "FAIL all pass: run matrix changed"
    failures=$((failures + 1))
  fi
fi

run_case "metadata failure" nonzero STUB_METADATA=fail && expect_output "metadata failure" "cargo metadata exited non-zero"
run_case "unparseable metadata" nonzero STUB_METADATA=badjson && expect_output "unparseable metadata" "could not parse cargo metadata"
run_case "zero examples" nonzero STUB_METADATA=none && expect_output "zero examples" "no example-* packages"

if run_case "child failure" nonzero STUB_FAIL_PKG=example-03-scene-state; then
  expect_output "child failure" "FAIL (exit 101)"
  expect_output "child failure" "Passed: 3/4"
  expect_output "child failure" "panicked at stub"
fi

if run_case "timeout" nonzero STUB_HANG_PKG=example-01-platformer; then
  expect_output "timeout" "TIMEOUT (2s)"
  expect_output "timeout" "Passed: 3/4"
fi

if run_case "matrix failure" nonzero STUB_FAIL_MSAA=4; then
  expect_output "matrix failure" "Matrix passed: 2/4"
  expect_output "matrix failure" "Matrix failures:"
fi

if [ "$failures" -gt 0 ]; then
  echo "smoke-examples regression tests: $failures failure(s)"
  exit 1
fi
echo "smoke-examples regression tests: OK"
