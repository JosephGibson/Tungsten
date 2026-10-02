#!/usr/bin/env python3
"""Benchmark log parsing, per-run statistics, guards, verdicts and reports.

`scripts/bench.py` runs `example-02-bench` and hands each run's logs here;
this module groups the telemetry lines into frames, computes per-run
statistics, evaluates the benchmark's guards and the determinism digest, and
renders `capture.json` plus a human-readable `README.md`. It also compares two
captures (comparability, Welch verdicts, `compare.md`, a self-contained
`compare.html`) and renders the sweep and capacity reports. Standard library
only (Python 3.12).
"""

import dataclasses
import hashlib
import html
import json
import math
import re
import statistics
from pathlib import Path

SCHEMA = 1

# Companion lines attach to the preceding `frame:` line; the others describe
# the whole run.
COMPANIONS = ("systems", "gpu_passes", "physics", "bench")
LINE_RE = re.compile(r"^(frame|systems|gpu_passes|physics|bench-config|bench|backend|startup):(?:\s+|$)(.*)$")
LOG_PREFIX_RE = re.compile(r"^\[[^\]]*\]\s*")
BACKEND_RE = re.compile(
    r"^(\S+) adapter: (.+) present_mode: (\S+) max_frame_latency: (\d+) timestamp_query: (true|false)$"
)

# `frame:` stages that partition `total`; the remainder is `unattributed`.
# `render` already contains acquire, encode and submit/present. `interval`
# is not work: it spans the previous frame and the wait after it, so it is
# no part of `total`, of the stacked bars or of the limiting stage.
TOTAL_PARTS = ("update", "flush", "particles", "tweens", "hot_reload", "extract", "render", "audio")
# A spike is a measured frame whose `total` exceeds this multiple of the run's p50.
SPIKE_FACTOR = 1.5

# Metric-name prefix -> per-run stats group.
GROUPS = {
    "stage": "stages",
    "system": "systems",
    "gpu": "gpu_passes",
    "physics": "physics",
    "counter": "counters",
}

PERCENTILES = (50, 95, 99)


@dataclasses.dataclass
class Frame:
    """One `frame:` line plus whichever companion lines followed it."""

    stages: dict
    systems: dict | None = None
    gpu_passes: dict | None = None
    physics: dict | None = None
    bench: dict | None = None
    raw: dict = dataclasses.field(default_factory=dict)


@dataclasses.dataclass
class Log:
    frames: list
    backend: dict | None = None
    bench_config: str | None = None
    startup: dict | None = None
    orphans: int = 0


def parse_value(text):
    """`12.34ms` -> 12.34, `8000` -> 8000.0, `n/a` -> None."""
    if text.endswith("ms"):
        text = text[:-2]
    if text == "n/a":
        return None
    try:
        return float(text)
    except ValueError:
        return None


def parse_pairs(text):
    """`name=value` tokens in line order. Names are literal keys (the engine
    maps whitespace and `=` in system names to `_`), never patterns."""
    pairs = {}
    for token in text.split():
        name, sep, value = token.partition("=")
        if sep and name:
            pairs[name] = parse_value(value)
    return pairs


def parse_log(text):
    """Group telemetry lines into frames. A companion line before the first
    `frame:` line counts as an orphan; a missing companion is tolerated."""
    log = Log(frames=[])
    for line in text.splitlines():
        match = LINE_RE.match(LOG_PREFIX_RE.sub("", line, count=1))
        if not match:
            continue
        tag, rest = match[1], match[2].strip()
        if tag == "frame":
            log.frames.append(Frame(stages=parse_pairs(rest)))
        elif tag in COMPANIONS:
            if not log.frames:
                log.orphans += 1
                continue
            frame = log.frames[-1]
            setattr(frame, tag, parse_pairs(rest))
            frame.raw[tag] = rest
        elif tag == "bench-config":
            log.bench_config = rest
        elif tag == "backend" and log.backend is None:
            backend = BACKEND_RE.match(rest)
            if backend:
                log.backend = {
                    "backend": backend[1],
                    "adapter": backend[2],
                    "present_mode": backend[3],
                    "max_frame_latency": int(backend[4]),
                    "timestamp_query": backend[5] == "true",
                }
        elif tag == "startup":
            log.startup = parse_pairs(rest)
    return log


def percentile(sorted_values, pct):
    """Nearest rank: the ceil(pct * n / 100)-th smallest value, rank clamped to [1, n]."""
    count = len(sorted_values)
    rank = min(count, max(1, -(-pct * count // 100)))
    return sorted_values[rank - 1]


def summarize(values):
    values = sorted(value for value in values if value is not None)
    if not values:
        return None
    summary = {"n": len(values), "mean": statistics.fmean(values), "min": values[0]}
    for pct in PERCENTILES:
        summary[f"p{pct}"] = percentile(values, pct)
    summary["max"] = values[-1]
    return summary


def unattributed(stages):
    total = stages.get("total")
    if total is None:
        return None
    return total - sum(stages.get(part) or 0.0 for part in TOTAL_PARTS)


def spike_count(frames):
    """Frames whose `total` exceeds SPIKE_FACTOR × the p50 of `frames`, or
    None without a `total`."""
    totals = sorted(total for frame in frames if (total := frame.stages.get("total")) is not None)
    if not totals:
        return None
    limit = SPIKE_FACTOR * percentile(totals, 50)
    return sum(1 for total in totals if total > limit)


def run_stats(frames):
    """Per-metric summaries over `frames`, metrics in first-seen order."""
    samples = {group: {} for group in GROUPS.values()}

    def add(group, pairs):
        for name, value in (pairs or {}).items():
            samples[group].setdefault(name, []).append(value)

    for frame in frames:
        add("stages", frame.stages)
        add("stages", {"unattributed": unattributed(frame.stages)})
        add("systems", frame.systems)
        add("gpu_passes", frame.gpu_passes)
        add("physics", frame.physics)
        add("counters", frame.bench)
    return {
        group: {name: summary for name, values in metrics.items() if (summary := summarize(values))}
        for group, metrics in samples.items()
    }


# Per-run statistics derived from two stored ones, as minuend minus
# subtrahend. `jitter` is the tail's distance from the median.
DERIVED_STATS = {"jitter": ("p99", "p50")}


def stat_value(summary, stat):
    """One statistic of a per-run summary, derived ones included."""
    if stat in DERIVED_STATS:
        high, low = (summary.get(name) for name in DERIVED_STATS[stat])
        return None if high is None or low is None else high - low
    return summary.get(stat)


def metric_value(stats, metric, stat):
    """`system.physics_step` + `p95` -> the run's value, or None."""
    prefix, _, name = metric.partition(".")
    summary = stats.get(GROUPS.get(prefix, ""), {}).get(name)
    return None if summary is None else stat_value(summary, stat)


COUNTER_GUARDS = ("counter_min", "counter_max", "counter_const", "counter_eq", "counter_band")


def guard_name(guard):
    kind = guard["kind"]
    if kind == "physics_max":
        return f"physics.{guard['field']} <= {guard['max']}"
    if kind == "counter_min":
        return f"bench.{guard['counter']} >= {guard['min']}"
    if kind == "counter_max":
        return f"bench.{guard['counter']} <= {guard['max']}"
    if kind == "counter_const":
        return f"bench.{guard['counter']} constant"
    if kind == "counter_eq":
        return f"bench.{guard['counter']} == bench.{guard['other']}"
    if kind == "counter_band":
        return f"bench.{guard['counter']} within ±{guard['tolerance']:.0%} of its median"
    return kind


def guard_fails(guard, value, reference, other):
    """Whether one frame's `value` breaks `guard`; `reference` is the first
    measured value (`counter_const`) or the median (`counter_band`), and
    `other` the compared counter (`counter_eq`)."""
    kind = guard["kind"]
    if kind in ("physics_max", "counter_max"):
        return value > guard["max"]
    if kind == "counter_min":
        return value < guard["min"]
    if kind == "counter_const":
        return value != reference
    if kind == "counter_band":
        return abs(value - reference) > guard["tolerance"] * abs(reference)
    return value != other


def evaluate_guard(guard, frames):
    """Check one guard over the measured frames. `physics:` lines are required
    on every frame; `bench:` lines on every frame but the last, which never
    has one (frame N + 1 logs frame N's counters)."""
    kind = guard["kind"]
    if kind == "physics_max":
        group, key, checked = "physics", guard["field"], frames
    elif kind in COUNTER_GUARDS:
        group, key, checked = "bench", guard["counter"], frames[:-1]
    else:
        return {"guard": guard_name(guard), "ok": False, "detail": f"unknown guard kind '{kind}'"}
    other_key = guard.get("other") if kind == "counter_eq" else None
    # A band centers on the median of every checked frame that has the counter.
    present = [value for frame in checked if (value := (getattr(frame, group) or {}).get(key)) is not None]
    median = statistics.median(present) if kind == "counter_band" and present else None
    bad = []
    values = []
    for index, frame in enumerate(checked, start=1):
        pairs = getattr(frame, group) or {}
        value = pairs.get(key)
        other = pairs.get(other_key) if other_key else None
        if value is None or (other_key and other is None):
            bad.append((index, "missing"))
            continue
        values.append(value)
        if guard_fails(guard, value, values[0] if median is None else median, other):
            shown = f"{key}={value:g}"
            if other_key:
                shown += f" {other_key}={other:g}"
            elif kind == "counter_const":
                shown += f", first {values[0]:g}"
            elif median is not None:
                shown += f", median {median:g}"
            bad.append((index, shown))
    name = guard_name(guard)
    if not checked:
        return {"guard": name, "ok": False, "detail": "no measured frames"}
    if bad:
        first_index, first_value = bad[0]
        detail = f"{len(bad)} of {len(checked)} frames fail; first: measured frame {first_index} ({first_value})"
        return {"guard": name, "ok": False, "detail": detail}
    span = f"{min(values):g}..{max(values):g}"
    if median:
        spread = max(abs(value - median) for value in values) / abs(median)
        span += f" (median {median:g}, largest deviation {spread:.1%})"
    return {"guard": name, "ok": True, "detail": f"{len(checked)} frames, {key} in {span}"}


def digest(frames):
    """Fingerprint of the deterministic per-frame workload: the exact `physics:`
    and `bench:` text of every frame. Equal builds under the pinned smoke dt
    replay identical workloads, so repeats must match."""
    sha = hashlib.sha256()
    for frame in frames:
        for tag in ("physics", "bench"):
            if tag in frame.raw:
                sha.update(f"{tag}:{frame.raw[tag]}\n".encode())
        sha.update(b"--\n")
    return sha.hexdigest()[:16]


# The child frees memory while it shuts down: RSS starts falling 13-16 ms
# after the last frame and the process exits 22-34 ms after it. A 100 ms
# sample can land in that window, so the fit drops samples this close to the
# last one.
RSS_TEARDOWN_S = 0.2


def rss_growth(samples):
    """Least-squares slope in KiB/s of `(seconds, rss_kib)` samples over the
    second half of the run, or None with too few samples. Samples within
    `RSS_TEARDOWN_S` of the last one are dropped first."""
    if samples:
        samples = [sample for sample in samples if sample[0] <= samples[-1][0] - RSS_TEARDOWN_S]
    if len(samples) < 4:
        return None
    start, end = samples[0][0], samples[-1][0]
    half = [(t, rss) for t, rss in samples if t >= (start + end) / 2]
    if len(half) < 2 or half[0][0] == half[-1][0]:
        return None
    return statistics.linear_regression([t for t, _ in half], [rss for _, rss in half]).slope


GUARD_PREFIX = "guard "


def hard_problems(analysis):
    """An analysis's problems other than guard failures: exit, frame-count and
    config problems. Capacity probes fail on these but only record guards."""
    return [problem for problem in analysis["problems"] if not problem.startswith(GUARD_PREFIX)]


# Present modes an `auto*` request may resolve to; the benchmarks turn vsync
# off, so `auto` takes the no-vsync family. A concrete mode is reported as
# requested.
PRESENT_FAMILIES = {
    "auto": ("immediate", "mailbox", "auto_no_vsync"),
    "auto_no_vsync": ("immediate", "mailbox", "auto_no_vsync"),
    "auto_vsync": ("fifo", "auto_vsync"),
}


def present_problems(backend, present_mode, max_frame_latency):
    """Problems when a run's `backend:` line does not confirm the requested
    present mode or frame latency (None: not requested)."""
    if present_mode is None and max_frame_latency is None:
        return []
    if backend is None:
        return ["no backend line to confirm the requested present mode and frame latency"]
    problems = []
    if present_mode is not None and backend["present_mode"] not in PRESENT_FAMILIES.get(present_mode, (present_mode,)):
        problems.append(f"present mode is {backend['present_mode']}, requested {present_mode}")
    if max_frame_latency is not None and backend["max_frame_latency"] != max_frame_latency:
        problems.append(f"max frame latency is {backend['max_frame_latency']}, requested {max_frame_latency}")
    return problems


def analyze_log(text, warmup, frames, guards, expected_config, present_mode=None, max_frame_latency=None):
    """Stats, guards, digest and validity problems for one run's log;
    `expected_config` is the resolved config the binary printed up front, and
    a requested present mode or frame latency must show in the `backend:` line."""
    log = parse_log(text)
    measured = log.frames[warmup : warmup + frames]
    problems = []
    if len(measured) < frames:
        problems.append(f"{len(measured)} measured frames of {frames} ({len(log.frames)} logged, {warmup} warm-up)")
    if log.bench_config is None:
        problems.append("no bench-config line")
    else:
        try:
            logged = json.loads(log.bench_config)
        except ValueError:
            logged = None
        if logged != expected_config:
            problems.append("bench-config does not match the request")
    problems.extend(present_problems(log.backend, present_mode, max_frame_latency))
    guard_results = [evaluate_guard(guard, measured) for guard in guards]
    problems.extend(f"{GUARD_PREFIX}{result['guard']}: {result['detail']}" for result in guard_results if not result["ok"])
    return {
        "frames_logged": len(log.frames),
        "frames_measured": len(measured),
        "backend": log.backend,
        "stats": run_stats(measured),
        "spikes": spike_count(measured),
        "guards": guard_results,
        "digest": digest(measured),
        "problems": problems,
    }


def assemble_capture(*, request, bench, row, config, warmup, frames, build, provenance, runs, profile):
    """`capture.json` (schema 1). A capture is valid when every run exits 0,
    reports every measured frame, matches the requested config and passes the
    guards, and every run (GPU diagnostic runs included) has one digest."""
    reasons = []
    if len(runs) < request["repeat"]:
        reasons.append(f"{len(runs)} of {request['repeat']} runs completed")
    digests = []
    for run in runs:
        for label, part in ((f"run {run['index']}", run), (f"run {run['index']} GPU diagnostic", run.get("gpu_run"))):
            if not part:
                continue
            if part["exit_code"] != 0:
                reasons.append(f"{label}: exited with {part['exit_code']}")
            reasons.extend(f"{label}: {problem}" for problem in part["problems"])
            digests.append(part["digest"])
    match = len(set(digests)) == 1
    if not match:
        reasons.append(f"determinism: digests differ across runs ({', '.join(digests) or 'none'})")
    guards = []
    for position, guard in enumerate(row["guards"]):
        results = [run["guards"][position] for run in runs if len(run["guards"]) > position]
        failing = [result for result in results if not result["ok"]]
        shown = (failing or results or [{"detail": "no runs"}])[0]
        guards.append({"guard": guard_name(guard), "ok": bool(results) and not failing, "detail": shown["detail"]})
    renderer = next((run["backend"] for run in runs if run.get("backend")), None)
    return {
        "schema": SCHEMA,
        "row": row["name"],
        "bench": bench["name"],
        "workload_version": bench["workload_version"],
        "config": config,
        "request": request,
        "frames": frames,
        "warmup": warmup,
        "build": build,
        "provenance": provenance,
        "renderer": renderer,
        "owned": row["owned"],
        "runs": runs,
        "guards": guards,
        "determinism": {"digests": digests, "match": match},
        "profile": profile,
        "valid": not reasons,
        "invalid_reasons": reasons,
    }


def fmt(value, digits=2):
    if value is None:
        return "n/a"
    if isinstance(value, float):
        return f"{value:.0f}" if value.is_integer() else f"{value:.{digits}f}"
    return str(value)


def owned_per_run(capture, metric, stat):
    """`metric` at `stat` per run, None where a run lacks it. GPU metrics come
    from the GPU diagnostic runs, everything else from the timing runs."""
    gpu = is_gpu_metric(metric)
    parts = [run.get("gpu_run") if gpu else run for run in capture["runs"]]
    return [metric_value(part["stats"], metric, stat) if part and part.get("stats") else None for part in parts]


def median_of(values):
    values = [value for value in values if value is not None]
    return statistics.median(values) if values else None


def gpu_metric_note(capture):
    """Why owned GPU metrics are n/a, or None when they all resolve."""
    metrics = [owned["metric"] for owned in capture["owned"] if is_gpu_metric(owned["metric"])]
    if not metrics:
        return None
    gpu_runs = [run["gpu_run"] for run in capture["runs"] if run.get("gpu_run")]
    if not gpu_runs:
        return "GPU-owned metrics are n/a: this capture has no GPU diagnostic run (`--gpu-timing off`)."
    backends = [part["backend"] for part in gpu_runs if part.get("backend")]
    if backends and not any(backend["timestamp_query"] for backend in backends):
        return (
            "Timestamp queries are unavailable on this adapter, so the GPU-owned metrics "
            f"({', '.join(f'`{metric}`' for metric in metrics)}) are n/a. The capture stays valid."
        )
    missing = [
        owned["metric"]
        for owned in capture["owned"]
        if is_gpu_metric(owned["metric"])
        and all(value is None for stat in owned["stats"] for value in owned_per_run(capture, owned["metric"], stat))
    ]
    if missing:
        return f"n/a in every GPU diagnostic run (not in this configuration's pass list): {', '.join(f'`{metric}`' for metric in missing)}."
    return None


def median_stat(runs, group, name, stat):
    return median_of([run["stats"].get(group, {}).get(name, {}).get(stat) for run in runs])


def table(header, rows):
    lines = ["| " + " | ".join(header) + " |", "| " + " | ".join("---" for _ in header) + " |"]
    lines.extend("| " + " | ".join(str(cell) for cell in row) + " |" for row in rows)
    return lines


def group_table(runs, group, stats=("mean", "p50", "p95", "p99", "max")):
    names = []
    for run in runs:
        names.extend(name for name in run["stats"].get(group, {}) if name not in names)
    rows = [[f"`{name}`", *(fmt(median_stat(runs, group, name, stat)) for stat in stats)] for name in names]
    return table(["Metric", *stats], rows) if rows else ["n/a"]


def capture_readme(capture):
    """Human-readable summary; every number is the median of per-run values."""
    config = capture["config"]
    request = capture["request"]
    provenance = capture["provenance"]
    renderer = capture.get("renderer") or {}
    runs = [run for run in capture["runs"] if run.get("stats")]
    overrides = ", ".join(request["set"]) or "none"
    present = f"{renderer.get('present_mode', 'unknown')} / {renderer.get('max_frame_latency', 'unknown')}"
    requested = f"{request['present_mode'] or 'none'} / {request['max_frame_latency'] or 'none'}"
    lines = [f"# Capture: {capture['row']}", ""]
    lines += table(
        ["Field", "Value"],
        [
            ["Valid", "yes" if capture["valid"] else "no"],
            ["Bench / workload version", f"{capture['bench']} / {capture['workload_version']}"],
            ["Preset / scale", f"{config['preset']} / {config['scale']:g}"],
            ["Overrides", overrides],
            ["Frames", f"{capture['frames']} measured after {capture['warmup']} warm-up"],
            ["Runs", f"{len(capture['runs'])} of {request['repeat']}"],
            ["GPU diagnostic run", "on" if request["gpu_timing"] else "off"],
            ["Commit", f"{provenance['commit']} (dirty: {provenance['dirty']})"],
            ["Build RUSTFLAGS", f"`{capture['build']['rustflags']}`"],
            ["Compiler", provenance["rustc"]],
            ["Backend / adapter", f"{renderer.get('backend', 'unknown')} / {renderer.get('adapter', 'unknown')}"],
            ["Present mode / latency (requested)", f"{present} ({requested})"],
            ["Timestamp queries", json.dumps(renderer["timestamp_query"]) if "timestamp_query" in renderer else "unknown"],
            ["CPU / kernel", f"{provenance['cpu']} / {provenance['kernel']}"],
            [
                "Governor / platform profile / AC",
                f"{provenance['governor']} / {provenance['platform_profile']} / {provenance['ac_power']}",
            ],
            ["Machine", provenance["machine"]],
        ],
    )
    lines += ["", "## Validity", ""]
    if capture["invalid_reasons"]:
        lines += [f"- {reason}" for reason in capture["invalid_reasons"]]
    else:
        lines.append("- Every run exited 0, reported every measured frame and matched the requested config.")
    for result in capture["guards"]:
        lines.append(f"- {'ok' if result['ok'] else 'FAIL'}: `{result['guard']}` ({result['detail']})")
    determinism = capture["determinism"]
    state = "match" if determinism["match"] else "DIFFER"
    lines.append(f"- Digests {state}: {', '.join(determinism['digests']) or 'n/a'}")
    note = gpu_metric_note(capture)
    if note:
        lines.append(f"- {note}")

    owned_rows = []
    for owned in capture["owned"]:
        source = "GPU diagnostic runs" if is_gpu_metric(owned["metric"]) else "timing runs"
        for stat in owned["stats"]:
            per_run = owned_per_run(capture, owned["metric"], stat)
            shown = ", ".join(fmt(value) for value in per_run)
            owned_rows.append([f"`{owned['metric']}` {stat}", fmt(median_of(per_run)), shown, source])
    lines += ["", "## Owned metrics (ms)", ""]
    lines += table(["Metric", "Median", "Per run", "Source"], owned_rows) if owned_rows else ["n/a"]
    lines += ["", "## Stages (ms)", ""] + group_table(runs, "stages")
    frame_rows = [
        [run["index"], *(fmt(metric_value(run["stats"], "stage.total", stat)) for stat in ("p50", "max")), fmt(run.get("spikes"))]
        for run in runs
    ]
    lines += ["", "## Frame time per run (timing runs)", ""]
    lines += table(["Run", "`total` p50 (ms)", "`total` max (ms)", f"Spikes (`total` > {SPIKE_FACTOR:g} × p50)"], frame_rows)
    lines += ["", "## Systems (ms)", ""] + group_table(runs, "systems")
    gpu_runs = [run["gpu_run"] for run in capture["runs"] if run.get("gpu_run") and run["gpu_run"].get("stats")]
    if gpu_runs:
        lines += ["", "## GPU passes (ms, diagnostic run)", ""] + group_table(gpu_runs, "gpu_passes")
    lines += ["", "## Physics", ""] + group_table(runs, "physics", ("mean", "min", "max"))
    lines += ["", "## Counters", ""] + group_table(runs, "counters", ("mean", "min", "max"))
    memory_rows = []
    for run in capture["runs"]:
        usage = run.get("rusage") or {}
        peak = usage.get("peak_rss_kib")
        memory_rows.append(
            [
                run["index"],
                fmt(None if peak is None else peak / 1024, 1),
                fmt(run.get("rss_growth_kib_s"), 1),
                fmt(usage.get("user_s")),
                fmt(usage.get("sys_s")),
                usage.get("minflt", "n/a"),
                usage.get("majflt", "n/a"),
                usage.get("nvcsw", "n/a"),
                usage.get("nivcsw", "n/a"),
            ]
        )
    lines += ["", "## Memory and CPU (timing runs)", ""]
    lines += table(
        ["Run", "Peak RSS (MiB)", "RSS growth (KiB/s)", "User s", "Sys s", "Minor faults", "Major faults", "Vol. switches", "Invol. switches"],
        memory_rows,
    )
    profile = capture.get("profile")
    if profile:
        lines += ["", "## Profile (run 1)", ""]
        lines += [f"- `profile/{name}`" for name in profile["files"]] or ["- no profiler output"]
        lines += [f"- {note}" for note in profile["notes"]]
    lines += ["", "Raw telemetry: `run-N/telemetry.log`, `run-N/rss.tsv` and, with GPU timing, `run-N/gpu.log`.", ""]
    return "\n".join(lines)


# --- Compare (A5) ------------------------------------------------------------

MIN_RUNS = 3
# Each side's variance is floored at (0.5% of its mean)^2.
VARIANCE_FLOOR = 0.005
TAU_REL = {"mean": 0.03, "p50": 0.03, "p95": 0.05, "p99": 0.08}
# Statistics judged with another statistic's threshold. Jitter (p99 - p50)
# carries p99's noise, so it takes p99's τ_rel applied to the baseline's
# per-run p99 mean: a relative τ on the small jitter itself would read
# `noisy` in every A/A.
TAU_FROM = {"jitter": "p99"}
TAU_ABS_MS = {"stages": 0.05, "systems": 0.02, "gpu_passes": 0.02}
RSS_TAU_REL = 0.02
RSS_TAU_ABS_KIB = 2 * 1024
# Workload counters whose means move by more than this share raise a drift warning.
DRIFT = 0.05
# Two-sided 95% Student t quantiles by degrees of freedom (the standard library
# has none). An untabulated df uses the next smaller entry, which is wider.
T_975 = {
    1: 12.706, 2: 4.303, 3: 3.182, 4: 2.776, 5: 2.571, 6: 2.447, 7: 2.365, 8: 2.306, 9: 2.262, 10: 2.228,
    11: 2.201, 12: 2.179, 13: 2.160, 14: 2.145, 15: 2.131, 16: 2.120, 17: 2.110, 18: 2.101, 19: 2.093,
    20: 2.086, 21: 2.080, 22: 2.074, 23: 2.069, 24: 2.064, 25: 2.060, 26: 2.056, 27: 2.052, 28: 2.048,
    29: 2.045, 30: 2.042, 40: 2.021, 60: 2.000, 120: 1.980,
}  # fmt: skip
VERDICTS = ("regressed", "improved", "unchanged", "noisy")
STAGE_ORDER = (
    "total", "interval", "update", "flush", "particles", "tweens", "hot_reload", "extract", "render",
    "render_acquire", "render_encode", "render_submit_present", "audio", "unattributed",
)  # fmt: skip
STAGE_STATS = ("p50", "p95", "p99")
SYSTEM_STATS = ("p50", "p95")
SOFT_PROVENANCE = ("rustc", "kernel", "governor", "platform_profile", "ac_power")
HARD_RENDERER = ("backend", "adapter", "present_mode", "max_frame_latency")


def t_quantile(df):
    whole = max(1, math.floor(df))
    return T_975[max(key for key in T_975 if key <= whole)]


def judge(base, cand, tau_rel, tau_abs, reference=None):
    """A5 verdict on per-run values, higher being worse: Δ of the means, its
    95% Welch interval, τ = max(τ_rel × reference, τ_abs) and the verdict,
    which stays None with fewer than MIN_RUNS runs on either side. The
    reference is the baseline mean unless given. A side without values keeps
    None; the other side's mean is still reported."""
    result = {"n": [len(base), len(cand)], "base": None, "cand": None, "delta": None, "delta_pct": None}
    result.update(interval=None, tau=None, verdict=None)
    if base:
        result["base"] = statistics.fmean(base)
    if cand:
        result["cand"] = statistics.fmean(cand)
    if not base or not cand:
        return result
    mean_base, mean_cand = result["base"], result["cand"]
    delta = mean_cand - mean_base
    tau = max(tau_rel * abs(mean_base if reference is None else reference), tau_abs)
    result.update(delta=delta, tau=tau)
    # A zero stage can carry float residue (`unattributed` is a difference).
    result["delta_pct"] = 100 * delta / mean_base if abs(mean_base) > 1e-9 else None
    if min(len(base), len(cand)) < MIN_RUNS:
        return result
    shares = [
        max(statistics.variance(values), (VARIANCE_FLOOR * mean) ** 2) / len(values)
        for values, mean in ((base, mean_base), (cand, mean_cand))
    ]
    error = math.sqrt(sum(shares))
    half = 0.0
    if error > 0:
        df = error**4 / sum(share**2 / (len(values) - 1) for share, values in zip(shares, (base, cand)))
        half = t_quantile(df) * error
    low, high = delta - half, delta + half
    if low > 0 and delta > tau:
        verdict = "regressed"
    elif high < 0 and delta < -tau:
        verdict = "improved"
    elif -tau <= low and high <= tau:
        verdict = "unchanged"
    else:
        verdict = "noisy"
    result.update(interval=[low, high], verdict=verdict)
    return result


def missing_note(result):
    """Which side of a judged result lacks values, or None when both have them."""
    if result["base"] is None and result["cand"] is None:
        return "missing on both sides"
    if result["base"] is None:
        return "missing in the baseline"
    if result["cand"] is None:
        return "missing in the candidate"
    return None


def is_gpu_metric(metric):
    """Metrics only the GPU diagnostic run measures: GPU passes and `stage.gpu`."""
    return metric.startswith("gpu.") or metric == "stage.gpu"


def run_values(capture, metric, stat):
    """Per-run values of `metric` at `stat`, from the GPU diagnostic runs for GPU metrics."""
    values = []
    for run in capture["runs"]:
        part = run.get("gpu_run") if is_gpu_metric(metric) else run
        if part and part.get("stats"):
            value = metric_value(part["stats"], metric, stat)
            if value is not None:
                values.append(value)
    return values


def judge_metric(base, cand, metric, stat, blocked):
    tau_stat = TAU_FROM.get(stat, stat)
    tau_rel = TAU_REL.get(tau_stat)
    group = "gpu_passes" if is_gpu_metric(metric) else GROUPS.get(metric.partition(".")[0], "")
    reference = None
    if tau_stat != stat:
        reference = mean_or_none(run_values(base, metric, tau_stat))
    result = {"metric": metric, "stat": stat}
    result.update(
        judge(run_values(base, metric, stat), run_values(cand, metric, stat), tau_rel or 0.0, TAU_ABS_MS.get(group, 0.0), reference)
    )
    result["note"] = None
    if tau_rel is None or group not in TAU_ABS_MS:
        result.update(verdict=None, note=f"no threshold for {stat}")
    elif blocked:
        result.update(verdict=None, note=blocked)
    elif result["base"] is None or result["cand"] is None:
        result["note"] = missing_note(result)
    elif result["verdict"] is None:
        result["note"] = f"fewer than {MIN_RUNS} runs"
    return result


def show(value):
    return "missing" if value is None else str(value)


def comparability(base, cand):
    """Hard mismatches, which suppress verdicts, and soft notes."""
    hard, soft = [], []

    def differ(label, left, right, into):
        if left != right:
            into.append(f"{label}: {show(left)} vs {show(right)}")

    differ("row", base["row"], cand["row"], hard)
    differ("workload_version", base["workload_version"], cand["workload_version"], hard)
    base_knobs, cand_knobs = base["config"]["knobs"], cand["config"]["knobs"]
    for name in sorted(set(base_knobs) | set(cand_knobs)):
        differ(f"knob {name}", base_knobs.get(name), cand_knobs.get(name), hard)
    differ("frames", base["frames"], cand["frames"], hard)
    differ("warm-up", base["warmup"], cand["warmup"], hard)
    differ("build flags", base["build"].get("rustflags"), cand["build"].get("rustflags"), hard)
    base_renderer, cand_renderer = base.get("renderer") or {}, cand.get("renderer") or {}
    for field in HARD_RENDERER:
        differ(field, base_renderer.get(field), cand_renderer.get(field), hard)
    differ("machine", base["provenance"].get("machine"), cand["provenance"].get("machine"), hard)
    for field in SOFT_PROVENANCE:
        differ(field, base["provenance"].get(field), cand["provenance"].get(field), soft)
    differ("GPU diagnostic run", base["request"].get("gpu_timing"), cand["request"].get("gpu_timing"), soft)
    return hard, soft


def ordered_names(captures, group, gpu=False):
    names = []
    for capture in captures:
        for run in capture["runs"]:
            part = run.get("gpu_run") if gpu else run
            if part and part.get("stats"):
                names.extend(name for name in part["stats"].get(group, {}) if name not in names)
    return names


def workload_counters(capture):
    """Mean per-frame value of each `physics:` field and `bench:` counter,
    averaged over the runs."""
    means = {}
    for prefix, group in (("physics", "physics"), ("counter", "counters")):
        for name in ordered_names([capture], group):
            values = [
                run["stats"][group][name]["mean"]
                for run in capture["runs"]
                if run.get("stats") and name in run["stats"].get(group, {})
            ]
            means[f"{prefix}.{name}"] = statistics.fmean(values)
    return means


def counter_rows(base, cand):
    base_means, cand_means = workload_counters(base), workload_counters(cand)
    rows = []
    for name in [*base_means, *(name for name in cand_means if name not in base_means)]:
        left, right = base_means.get(name), cand_means.get(name)
        pct = None if left is None or right is None or not left else 100 * (right - left) / abs(left)
        if left is None or right is None:
            drift = True
        else:
            drift = abs(right - left) > DRIFT * abs(left) if left else right != 0
        rows.append({"metric": name, "base": left, "cand": right, "delta_pct": pct, "drift": drift})
    return rows


def mean_or_none(values):
    return statistics.fmean(values) if values else None


def side_info(capture, label, path):
    provenance = capture["provenance"]
    renderer = capture.get("renderer") or {}
    return {
        "label": label,
        "path": str(path),
        "row": capture["row"],
        "commit": provenance.get("commit"),
        "dirty": provenance.get("dirty"),
        "valid": capture["valid"],
        "invalid_reasons": capture["invalid_reasons"],
        "runs": len([run for run in capture["runs"] if run.get("stats")]),
        "cpu": provenance.get("cpu"),
        "adapter": renderer.get("adapter"),
        "present": f"{renderer.get('present_mode', 'unknown')} / {renderer.get('max_frame_latency', 'unknown')}",
        "frames": f"{capture['frames']} after {capture['warmup']} warm-up",
        "guards": capture["guards"],
        "determinism": capture["determinism"]["match"],
    }


def compare(base, cand, *, base_side, cand_side, force=False):
    """Compare two captures. `*_side` is `(label, path)`. A hard mismatch
    suppresses every verdict; an invalid capture does too unless `force`."""
    hard, soft = comparability(base, cand)
    invalid = [f"{name} capture is invalid" for name, capture in (("baseline", base), ("candidate", cand)) if not capture["valid"]]
    blocked = None
    if hard:
        blocked = "not comparable"
    elif invalid and not force:
        blocked = "invalid capture"
    owned_specs = list(base["owned"]) + [entry for entry in cand["owned"] if entry not in base["owned"]]
    owned = [judge_metric(base, cand, entry["metric"], stat, blocked) for entry in owned_specs for stat in entry["stats"]]
    stage_names = [name for name in STAGE_ORDER if name in ordered_names([base, cand], "stages")]
    stage_names += [name for name in ordered_names([base, cand], "stages") if name not in stage_names and name != "gpu"]
    stages = [judge_metric(base, cand, f"stage.{name}", stat, blocked) for name in stage_names for stat in STAGE_STATS]
    systems = [
        judge_metric(base, cand, f"system.{name}", stat, blocked)
        for name in ordered_names([base, cand], "systems")
        for stat in SYSTEM_STATS
    ]
    gpu = None
    if all(any(run.get("gpu_run") for run in capture["runs"]) for capture in (base, cand)):
        metrics = ["stage.gpu"] + [f"gpu.{name}" for name in ordered_names([base, cand], "gpu_passes", gpu=True)]
        gpu = [judge_metric(base, cand, metric, stat, blocked) for metric in metrics for stat in SYSTEM_STATS]
    peaks = [[run["rusage"]["peak_rss_kib"] for run in capture["runs"] if run.get("rusage")] for capture in (base, cand)]
    peak = judge(*peaks, RSS_TAU_REL, RSS_TAU_ABS_KIB)
    peak.update(metric="peak_rss_kib", stat="per run", note=blocked)
    if blocked:
        peak["verdict"] = None
    elif peak["verdict"] is None:
        peak["note"] = missing_note(peak) or f"fewer than {MIN_RUNS} runs"
    growth = [mean_or_none([run["rss_growth_kib_s"] for run in c["runs"] if run.get("rss_growth_kib_s") is not None]) for c in (base, cand)]
    cpu = [mean_or_none([run["rusage"]["user_s"] + run["rusage"]["sys_s"] for run in c["runs"] if run.get("rusage")]) for c in (base, cand)]
    counters = counter_rows(base, cand)
    summary = dict.fromkeys((*VERDICTS, "none"), 0)
    for result in owned:
        summary[result["verdict"] or "none"] += 1
    return {
        "schema": SCHEMA,
        "baseline": side_info(base, *base_side),
        "candidate": side_info(cand, *cand_side),
        "comparable": not hard,
        "hard": hard,
        "soft": soft,
        "invalid": invalid,
        "forced": bool(force and invalid and not hard),
        "blocked": blocked,
        "drift": [row["metric"] for row in counters if row["drift"]],
        "owned_summary": summary,
        "owned": owned,
        "stages": stages,
        "systems": systems,
        "gpu": gpu,
        "memory": {"peak_rss": peak, "rss_growth_kib_s": growth, "cpu_s": cpu},
        "counters": counters,
    }


def any_regressed(report):
    """Whether an owned metric regressed, in a capture or a suite report."""
    if report.get("kind") == "suite":
        results = [result for row in report["rows"] for result in row["owned"]]
    else:
        results = report["owned"]
    return any(result["verdict"] == "regressed" for result in results)


# --- Compare Markdown ----------------------------------------------------------


def signed(value, digits=2, suffix=""):
    return "n/a" if value is None else f"{value:+.{digits}f}{suffix}"


def verdict_text(result):
    if result["verdict"]:
        return result["verdict"]
    return f"— ({result['note']})" if result.get("note") else "—"


def interval_text(result, scale=1.0, digits=2):
    if not result["interval"]:
        return "n/a"
    low, high = (value / scale for value in result["interval"])
    return f"[{low:+.{digits}f}, {high:+.{digits}f}]"


def judged_rows(results, label=lambda result: f"`{result['metric']}`"):
    return [
        [
            label(result),
            result["stat"],
            fmt(result["base"]),
            fmt(result["cand"]),
            signed(result["delta"]),
            signed(result["delta_pct"], 1, "%"),
            interval_text(result),
            fmt(result["tau"], 3),
            verdict_text(result),
        ]
        for result in results
    ]


JUDGED_HEADER = ["Metric", "Stat", "Baseline", "Candidate", "Δ", "Δ%", "95% interval", "τ", "Verdict"]


def comparability_lines(report):
    lines = []
    if report["hard"]:
        lines.append("**Not comparable; verdicts suppressed.** Hard mismatches:")
        lines += [f"- {item}" for item in report["hard"]]
    elif report["blocked"]:
        lines.append(f"**Verdicts suppressed:** {'; '.join(report['invalid'])}. Pass `--force` to judge anyway.")
    elif report["forced"]:
        lines.append(f"**Comparable, forced:** {'; '.join(report['invalid'])}; verdicts shown because of `--force`.")
    else:
        lines.append("**Comparable:** row, workload version, knobs, frames, build flags, renderer and machine match.")
    if report["soft"]:
        lines.append("Environment notes: " + "; ".join(report["soft"]) + ".")
    if report["drift"]:
        lines.append(
            f"**Workload drift** (counter means moved by more than {DRIFT:.0%}): "
            + ", ".join(f"`{name}`" for name in report["drift"])
            + ". Verdicts below stand but compare different trajectories."
        )
    return lines


def summary_text(summary):
    parts = [f"{summary[name]} {name}" for name in VERDICTS]
    if summary["none"]:
        parts.append(f"{summary['none']} without a verdict")
    return ", ".join(parts)


def compare_markdown(report):
    base, cand = report["baseline"], report["candidate"]
    lines = [f"# Compare: {cand['row']}", ""]
    lines += table(
        ["", "Baseline", "Candidate"],
        [
            ["Capture", f"`{base['label']}`", f"`{cand['label']}`"],
            ["Commit (dirty)", f"{base['commit']} ({base['dirty']})", f"{cand['commit']} ({cand['dirty']})"],
            ["Valid", "yes" if base["valid"] else "no", "yes" if cand["valid"] else "no"],
            ["Runs", base["runs"], cand["runs"]],
            ["Frames", base["frames"], cand["frames"]],
            ["CPU / adapter", f"{base['cpu']} / {base['adapter']}", f"{cand['cpu']} / {cand['adapter']}"],
            ["Present mode / latency", base["present"], cand["present"]],
        ],
    )
    lines += [""] + comparability_lines(report)
    lines += ["", "## Owned metrics (ms)", "", f"Verdicts: {summary_text(report['owned_summary'])}.", ""]
    lines += table(JUDGED_HEADER, judged_rows(report["owned"]))
    lines += ["", "## Stages (ms)", ""] + table(JUDGED_HEADER, judged_rows(report["stages"]))
    moved = [result for result in report["systems"] if result["delta"] is not None and abs(result["delta"]) > result["tau"]]
    lines += ["", "## Systems with |Δ| > τ (ms)", ""]
    if moved:
        lines += table(JUDGED_HEADER, judged_rows(moved))
    else:
        checked = len({result["metric"] for result in report["systems"]})
        lines.append(f"None of the {checked} systems moved by more than τ at {' or '.join(SYSTEM_STATS)}.")
    lines += ["", "## GPU passes (ms, diagnostic run)", ""]
    if report["gpu"]:
        lines += table(JUDGED_HEADER, judged_rows(report["gpu"]))
    else:
        lines.append("n/a: both captures need a GPU diagnostic run.")
    memory = report["memory"]
    peak = memory["peak_rss"]
    mib = 1024
    lines += ["", "## Memory and CPU", ""]
    lines += table(
        ["Metric", "Baseline", "Candidate", "Δ", "Δ%", "95% interval", "τ", "Verdict"],
        [
            [
                "Peak RSS (MiB, mean of runs)",
                fmt(None if peak["base"] is None else peak["base"] / mib, 1),
                fmt(None if peak["cand"] is None else peak["cand"] / mib, 1),
                signed(None if peak["delta"] is None else peak["delta"] / mib, 1),
                signed(peak["delta_pct"], 1, "%"),
                interval_text(peak, mib, 1),
                fmt(None if peak["tau"] is None else peak["tau"] / mib, 1),
                verdict_text(peak),
            ],
            ["RSS growth (KiB/s, second half)", *(fmt(value, 1) for value in memory["rss_growth_kib_s"]), "", "", "", "", "reported only"],
            ["CPU time, user + sys (s per run)", *(fmt(value) for value in memory["cpu_s"]), "", "", "", "", "reported only"],
            ["Startup", "n/a", "n/a", "", "", "", "", "gap T2"],
        ],
    )
    lines += ["", "## Workload counters (mean per frame)", ""]
    counter_table = [
        [f"`{row['metric']}`", fmt(row["base"]), fmt(row["cand"]), signed(row["delta_pct"], 1, "%"), "DRIFT" if row["drift"] else ""]
        for row in report["counters"]
    ]
    lines += table(["Counter", "Baseline", "Candidate", "Δ%", "Drift"], counter_table) if counter_table else ["n/a"]
    lines += ["", "## Guards and validity", ""]
    for name, side in (("Baseline", base), ("Candidate", cand)):
        state = "valid" if side["valid"] else "INVALID"
        lines.append(f"- {name} ({state}, digests {'match' if side['determinism'] else 'DIFFER'}):")
        lines += [f"  - {'ok' if guard['ok'] else 'FAIL'}: `{guard['guard']}` ({guard['detail']})" for guard in side["guards"]]
        lines += [f"  - {reason}" for reason in side["invalid_reasons"]]
    lines += [
        "",
        f"Rule (A5): runs are the unit; Δ is the difference of per-run means with a 95% Welch interval, each side's "
        f"variance floored at ({VARIANCE_FLOOR:.1%} of its mean)². τ = max(τ_rel × baseline, τ_abs) with τ_rel "
        f"3% (mean, p50), 5% (p95), 8% (p99) and τ_abs 0.05 ms (stages) or 0.02 ms (systems, GPU); peak RSS uses "
        f"2% and 2 MiB. `regressed`/`improved` need the interval clear of 0 and |Δ| > τ; `unchanged` needs the "
        f"interval within ±τ; anything else is `noisy`. Fewer than {MIN_RUNS} runs a side shows deltas only. "
        f"Jitter (p99 − p50) takes p99's τ: 8% of the baseline's p99 mean, with the same τ_abs.",
        "",
    ]
    return "\n".join(lines)


# --- HTML: shared page and SVG helpers ---------------------------------------
# Self-contained pages: inline CSS and SVG only, with no scripts, fonts, links
# or network requests. Marks carry native <title> tooltips; every chart has a
# table or legend with its numbers, so no value depends on color alone.

BUDGETS_MS = (("60 Hz", 16.7), ("144 Hz", 6.9))
SIDES = (("baseline", "base"), ("candidate", "cand"))
# Stacked stage segments: label and the `frame:` keys they sum; `other` (gray)
# is the rest of `total`.
STACK = (
    ("update", ("update",)),
    ("flush", ("flush",)),
    ("extract", ("extract",)),
    ("render_encode", ("render_encode",)),
    ("present wait", ("render_acquire", "render_submit_present")),
    ("unattributed", ("unattributed",)),
)
VERDICT_MARKS = {"regressed": "▲", "improved": "▼", "unchanged": "=", "noisy": "~"}

CSS = """
:root {
  color-scheme: light;
  --page: #f9f9f7; --surface: #fcfcfb; --ink: #0b0b0b; --ink-2: #52514e; --muted: #898781;
  --grid: #e1e0d9; --axis: #c3c2b7; --border: rgba(11, 11, 11, 0.10);
  --base: #2a78d6; --cand: #eb6834; --other: #898781;
  --st1: #1baf7a; --st2: #eda100; --st3: #e87ba4; --st4: #008300; --st5: #4a3aa7; --st6: #e34948;
  --good: #0ca30c; --warn: #fab219; --bad: #d03b3b;
}
@media (prefers-color-scheme: dark) {
  :root {
    color-scheme: dark;
    --page: #0d0d0d; --surface: #1a1a19; --ink: #ffffff; --ink-2: #c3c2b7; --muted: #898781;
    --grid: #2c2c2a; --axis: #383835; --border: rgba(255, 255, 255, 0.10);
    --base: #3987e5; --cand: #d95926;
    --st1: #199e70; --st2: #c98500; --st3: #d55181; --st4: #008300; --st5: #9085e9; --st6: #e66767;
  }
}
* { box-sizing: border-box; }
body { margin: 0; background: var(--page); color: var(--ink);
  font: 14px/1.45 system-ui, -apple-system, "Segoe UI", sans-serif; }
main { max-width: 980px; margin: 0 auto; padding: 24px 16px 48px; }
h1 { font-size: 22px; margin: 0 0 4px; }
h2 { font-size: 16px; margin: 0 0 8px; }
h3 { font-size: 14px; margin: 16px 0 6px; color: var(--ink-2); }
p { margin: 6px 0; }
.muted { color: var(--ink-2); }
section { background: var(--surface); border: 1px solid var(--border); border-radius: 8px;
  padding: 16px; margin: 16px 0; }
.scroll { overflow-x: auto; }
table { border-collapse: collapse; width: 100%; font-variant-numeric: tabular-nums; }
th, td { text-align: left; padding: 4px 8px; border-bottom: 1px solid var(--grid); vertical-align: top; }
th { color: var(--ink-2); font-weight: 600; }
td.num, th.num { text-align: right; white-space: nowrap; }
code { font-size: 12px; }
ul { margin: 6px 0; padding-left: 20px; }
.badge { display: inline-block; padding: 0 6px; border: 1.5px solid var(--muted); border-radius: 4px;
  font-size: 12px; white-space: nowrap; }
.v-regressed { border-color: var(--bad); }
.v-improved { border-color: var(--good); }
.v-noisy { border-color: var(--warn); }
.legend { display: flex; flex-wrap: wrap; gap: 4px 16px; margin: 6px 0; color: var(--ink-2); font-size: 12px; }
.key { display: inline-block; width: 12px; height: 12px; border-radius: 3px; vertical-align: -2px; margin-right: 4px; }
svg.chart { display: block; width: 100%; height: auto; margin: 4px 0; }
svg.chart text { fill: var(--muted); font-size: 11px; font-variant-numeric: tabular-nums; }
svg.chart text.ink { fill: var(--ink-2); }
.grid { stroke: var(--grid); stroke-width: 1; }
.axis { stroke: var(--axis); stroke-width: 1; }
.budget { stroke: var(--ink-2); stroke-width: 1; stroke-dasharray: 4 3; }
.line { fill: none; stroke-width: 2; stroke-linejoin: round; }
.whisker { stroke-width: 1.5; }
.s-base { stroke: var(--base); } .f-base { fill: var(--base); } .k-base { background: var(--base); }
.s-cand { stroke: var(--cand); } .f-cand { fill: var(--cand); } .k-cand { background: var(--cand); }
.dot { stroke: var(--surface); stroke-width: 2; }
.hollow { fill: var(--surface); stroke-width: 2; }
.seg { stroke: var(--surface); stroke-width: 2; }
.f-st1 { fill: var(--st1); } .k-st1 { background: var(--st1); }
.f-st2 { fill: var(--st2); } .k-st2 { background: var(--st2); }
.f-st3 { fill: var(--st3); } .k-st3 { background: var(--st3); }
.f-st4 { fill: var(--st4); } .k-st4 { background: var(--st4); }
.f-st5 { fill: var(--st5); } .k-st5 { background: var(--st5); }
.f-st6 { fill: var(--st6); } .k-st6 { background: var(--st6); }
.f-other { fill: var(--other); } .k-other { background: var(--other); }
"""


def esc(value):
    return html.escape(str(value), quote=True)


def page(title, body):
    return (
        '<!doctype html>\n<html lang="en">\n<head>\n<meta charset="utf-8">\n'
        '<meta name="viewport" content="width=device-width, initial-scale=1">\n'
        f"<title>{esc(title)}</title>\n<style>{CSS}</style>\n</head>\n<body>\n<main>\n{body}\n</main>\n</body>\n</html>\n"
    )


def html_table(header, rows, numeric=()):
    """`numeric` holds the column indexes to right-align. Cells are escaped
    unless wrapped in `Markup`."""
    head = "".join(f'<th{" class=num" if index in numeric else ""}>{esc(cell)}</th>' for index, cell in enumerate(header))
    body = []
    for row in rows:
        cells = []
        for index, cell in enumerate(row):
            text = cell.html if isinstance(cell, Markup) else esc(cell)
            cells.append(f'<td{" class=num" if index in numeric else ""}>{text}</td>')
        body.append("<tr>" + "".join(cells) + "</tr>")
    return f'<div class="scroll"><table><thead><tr>{head}</tr></thead><tbody>{"".join(body)}</tbody></table></div>'


@dataclasses.dataclass
class Markup:
    html: str


def badge(result):
    verdict = result.get("verdict")
    if not verdict:
        return Markup(f'<span class="badge">— {esc(result.get("note") or "no verdict")}</span>')
    return Markup(f'<span class="badge v-{verdict}">{VERDICT_MARKS[verdict]} {verdict}</span>')


def nice_ticks(low, high, count=5):
    if not high > low:
        high = low + 1.0
    raw = (high - low) / count
    magnitude = 10 ** math.floor(math.log10(raw))
    step = next(factor * magnitude for factor in (1, 2, 2.5, 5, 10) if factor * magnitude >= raw)
    ticks, value = [], math.ceil(low / step) * step
    while value <= high + step * 1e-9:
        ticks.append(round(value, 10))
        value += step
    return ticks


def tick_ceiling(high, count=4):
    """`high` rounded up to the next `nice_ticks` step, so the top gridline is labeled."""
    ticks = nice_ticks(0.0, high, count)
    step = ticks[1] - ticks[0] if len(ticks) > 1 else high
    return math.ceil(high / step - 1e-9) * step


def tick_text(value):
    return f"{value:g}"


class Frame2D:
    """Linear (or log2 x) mapping from data to a fixed SVG plot area."""

    def __init__(self, width, height, x_range, y_range, margin=(24, 20, 40, 56), log_x=False):
        self.width, self.height = width, height
        self.top, self.right, self.bottom, self.left = margin
        self.log_x = log_x
        self.x0, self.x1 = (math.log2(v) for v in x_range) if log_x else x_range
        self.y0, self.y1 = y_range

    def x(self, value):
        value = math.log2(value) if self.log_x else value
        span = (self.x1 - self.x0) or 1.0
        return self.left + (value - self.x0) / span * (self.width - self.left - self.right)

    def y(self, value):
        span = (self.y1 - self.y0) or 1.0
        return self.top + (1 - (value - self.y0) / span) * (self.height - self.top - self.bottom)

    def open(self, label):
        return f'<svg class="chart" viewBox="0 0 {self.width} {self.height}" role="img" aria-label="{esc(label)}">'

    def y_grid(self, ticks, fmt_tick=tick_text):
        parts = []
        for tick in ticks:
            y = self.y(tick)
            parts.append(f'<line class="grid" x1="{self.left}" x2="{self.width - self.right}" y1="{y:.1f}" y2="{y:.1f}"/>')
            parts.append(f'<text x="{self.left - 6}" y="{y + 4:.1f}" text-anchor="end">{esc(fmt_tick(tick))}</text>')
        return "".join(parts)

    def x_axis(self, ticks, fmt_tick=tick_text, title=""):
        base = self.height - self.bottom
        parts = [f'<line class="axis" x1="{self.left}" x2="{self.width - self.right}" y1="{base}" y2="{base}"/>']
        for tick in ticks:
            x = self.x(tick)
            parts.append(f'<line class="axis" x1="{x:.1f}" x2="{x:.1f}" y1="{base}" y2="{base + 4}"/>')
            parts.append(f'<text x="{x:.1f}" y="{base + 16}" text-anchor="middle">{esc(fmt_tick(tick))}</text>')
        if title:
            middle = (self.left + self.width - self.right) / 2
            parts.append(f'<text class="ink" x="{middle:.1f}" y="{self.height - 6}" text-anchor="middle">{esc(title)}</text>')
        return "".join(parts)

    def y_title(self, title):
        middle = (self.top + self.height - self.bottom) / 2
        return f'<text class="ink" x="12" y="{middle:.1f}" transform="rotate(-90 12 {middle:.1f})" text-anchor="middle">{esc(title)}</text>'


def padded_range(values, floor=None):
    low, high = min(values), max(values)
    pad = (high - low) * 0.06 or max(abs(high) * 0.05, 0.05)
    low = low - pad if floor is None else max(floor, low - pad)
    return low, high + pad


def legend(items):
    """`items`: (css key class, text)."""
    return '<div class="legend">' + "".join(f'<span><span class="key {cls}"></span>{esc(text)}</span>' for cls, text in items) + "</div>"


def polyline(points, cls):
    path = " ".join(f"{x:.1f},{y:.1f}" for x, y in points)
    return f'<polyline class="line {cls}" points="{path}"/>'


def budget_lines_x(frame, low, high):
    parts = []
    for label, ms in BUDGETS_MS:
        if low <= ms <= high:
            x = frame.x(ms)
            parts.append(f'<line class="budget" x1="{x:.1f}" x2="{x:.1f}" y1="{frame.top}" y2="{frame.height - frame.bottom}"/>')
            parts.append(f'<text class="ink" x="{x + 4:.1f}" y="{frame.top + 10}">{esc(label)} {ms} ms</text>')
    return "".join(parts)


def budget_lines_y(frame, low, high):
    parts = []
    for label, ms in BUDGETS_MS:
        if low <= ms <= high:
            y = frame.y(ms)
            parts.append(f'<line class="budget" x1="{frame.left}" x2="{frame.width - frame.right}" y1="{y:.1f}" y2="{y:.1f}"/>')
            parts.append(f'<text class="ink" x="{frame.width - frame.right - 4}" y="{y - 4:.1f}" text-anchor="end">{esc(label)} {ms} ms</text>')
    return "".join(parts)


def off_scale_note(low, high):
    missing = [f"{label} ({ms} ms)" for label, ms in BUDGETS_MS if not low <= ms <= high]
    return f'<p class="muted">Budget lines outside this range: {esc(", ".join(missing))}.</p>' if missing else ""


def sample_indexes(count, limit=600):
    """At most `limit` evenly spaced indexes into `count` points, ends included."""
    if count <= limit:
        return range(count)
    return sorted({round(i * (count - 1) / (limit - 1)) for i in range(limit)})


def ecdf_svg(pooled):
    """ECDF of each side's pooled measured `total` frames, with p50/p95/p99 markers."""
    values = [value for side in pooled.values() for value in side]
    if not values:
        return '<p class="muted">No frame data: the runs\' telemetry logs are missing.</p>'
    low, high = padded_range(values, floor=0.0)
    frame = Frame2D(720, 300, (low, high), (0.0, 1.0))
    parts = [frame.open("ECDF of total frame time per side")]
    parts.append(frame.y_grid([0, 0.25, 0.5, 0.75, 1.0], lambda tick: f"{tick:.0%}"))
    parts.append(frame.x_axis(nice_ticks(low, high), title="total frame time (ms)"))
    parts.append(frame.y_title("frames at or below"))
    parts.append(budget_lines_x(frame, low, high))
    for name, cls in SIDES:
        ordered = sorted(pooled.get(name) or [])
        if not ordered:
            continue
        count = len(ordered)
        points = [(frame.x(ordered[0]), frame.y(0.0))]
        points += [(frame.x(ordered[i]), frame.y((i + 1) / count)) for i in sample_indexes(count)]
        parts.append(polyline(points, f"s-{cls}"))
        for pct in PERCENTILES:
            value = percentile(ordered, pct)
            x, y = frame.x(value), frame.y(pct / 100)
            parts.append(f'<circle class="dot f-{cls}" cx="{x:.1f}" cy="{y:.1f}" r="4"><title>{name} p{pct}: {value:.2f} ms</title></circle>')
    parts.append("</svg>")
    return "".join(parts) + off_scale_note(low, high)


def series_svg(run1):
    """Run-1 `total` per measured frame, per side."""
    values = [value for side in run1.values() for value in side]
    if not values:
        return '<p class="muted">No frame data for run 1.</p>'
    low, high = padded_range(values)
    frames = max(len(side) for side in run1.values())
    frame = Frame2D(720, 260, (1, max(frames, 2)), (low, high))
    parts = [frame.open("Run-1 total frame time per frame")]
    parts.append(frame.y_grid(nice_ticks(low, high, 4)))
    parts.append(frame.x_axis(nice_ticks(1, frames), title="measured frame"))
    parts.append(frame.y_title("total (ms)"))
    parts.append(budget_lines_y(frame, low, high))
    for name, cls in SIDES:
        side = run1.get(name) or []
        if side:
            parts.append(polyline([(frame.x(index + 1), frame.y(value)) for index, value in enumerate(side)], f"s-{cls}"))
    parts.append("</svg>")
    return "".join(parts) + off_scale_note(low, high)


def stage_means(capture):
    """Mean ms per stage, averaged over the runs' per-run means."""
    means = {}
    for name in ordered_names([capture], "stages"):
        means[name] = mean_or_none(
            [run["stats"]["stages"][name]["mean"] for run in capture["runs"] if run.get("stats") and name in run["stats"]["stages"]]
        )
    return means


def stack_segments(means):
    segments = [(label, sum(means.get(part) or 0.0 for part in parts)) for label, parts in STACK]
    total = means.get("total") or 0.0
    segments.append(("other", max(0.0, total - sum(value for _, value in segments))))
    return segments, total


def stack_svg(captures):
    """Stacked mean stage bars, one per side; `other` is hot reload, audio,
    the render remainder and any T1 stages."""
    stacks = {name: stack_segments(stage_means(capture)) for name, capture in captures.items()}
    widest = max(total for _, total in stacks.values()) or 1.0
    left, right, bar = 88, 72, 22
    width = 720
    height = 16 + len(stacks) * (bar + 14) + 8
    scale = (width - left - right) / widest
    parts = [f'<svg class="chart" viewBox="0 0 {width} {height}" role="img" aria-label="Mean stage time per side">']
    y = 16
    for name, (segments, total) in stacks.items():
        x = float(left)
        parts.append(f'<text class="ink" x="{left - 8}" y="{y + bar / 2 + 4:.1f}" text-anchor="end">{esc(name)}</text>')
        for index, (label, value) in enumerate(segments):
            span = value * scale
            if span <= 0:
                continue
            cls = "f-other" if label == "other" else f"f-st{index + 1}"
            parts.append(
                f'<rect class="seg {cls}" x="{x:.1f}" y="{y}" width="{span:.1f}" height="{bar}" rx="2">'
                f"<title>{esc(name)} {esc(label)}: {value:.3f} ms</title></rect>"
            )
            x += span
        parts.append(f'<text x="{x + 6:.1f}" y="{y + bar / 2 + 4:.1f}">{total:.2f} ms</text>')
        y += bar + 14
    parts.append("</svg>")
    names = list(stacks)
    keys = []
    for index, (label, _) in enumerate(STACK + (("other", ()),)):
        cls = "k-other" if label == "other" else f"k-st{index + 1}"
        values = " / ".join(f"{dict(stacks[name][0])[label]:.2f}" for name in names)
        keys.append((cls, f"{label} {values} ms"))
    return "".join(parts) + legend(keys) + f'<p class="muted">Legend values: {esc(" / ".join(names))}.</p>'


def hbar_svg(items, unit, label):
    """Paired horizontal bars. `items`: (name, {side: (value, whisker)}) in
    display order; the whisker (for example p95) may be None."""
    if not items:
        return '<p class="muted">No data.</p>'
    top_value = max((whisker or value) for _, sides in items for value, whisker in sides.values() if value is not None) or 1.0
    left, right, bar, gap = 200, 96, 9, 14
    width = 720
    height = 8 + len(items) * (2 * bar + 2 + gap) + 22
    frame = Frame2D(width, height, (0.0, top_value * 1.02), (0.0, 1.0), margin=(8, right, 22, left))
    parts = [frame.open(label)]
    for tick in nice_ticks(0.0, top_value * 1.02, 4):
        x = frame.x(tick)
        parts.append(f'<line class="grid" x1="{x:.1f}" x2="{x:.1f}" y1="8" y2="{height - 22}"/>')
        parts.append(f'<text x="{x:.1f}" y="{height - 8}" text-anchor="middle">{esc(tick_text(tick))}</text>')
    y = 8.0
    for name, sides in items:
        text = name if len(name) <= 30 else name[:29] + "…"
        parts.append(f'<text class="ink" x="{left - 8}" y="{y + bar + 5:.1f}" text-anchor="end">{esc(text)}<title>{esc(name)}</title></text>')
        for side, cls in SIDES:
            value, whisker = sides.get(side, (None, None))
            if value is not None:
                end = frame.x(value)
                parts.append(
                    f'<rect class="f-{cls}" x="{left}" y="{y:.1f}" width="{max(end - left, 1):.1f}" height="{bar}" rx="2">'
                    f"<title>{esc(name)} {side}: {value:.3f} {unit}</title></rect>"
                )
                reach = end
                if whisker is not None and whisker > value:
                    reach = frame.x(whisker)
                    middle = y + bar / 2
                    parts.append(f'<line class="whisker s-{cls}" x1="{end:.1f}" x2="{reach:.1f}" y1="{middle:.1f}" y2="{middle:.1f}"/>')
                    parts.append(f'<line class="whisker s-{cls}" x1="{reach:.1f}" x2="{reach:.1f}" y1="{y:.1f}" y2="{y + bar:.1f}"/>')
                shown = f"{value:.3g}" if whisker is None else f"{value:.3g} / {whisker:.3g}"
                parts.append(f'<text x="{reach + 5:.1f}" y="{y + bar - 1:.1f}">{esc(shown)}</text>')
            y += bar + 2
        y += gap - 2
    parts.append("</svg>")
    return "".join(parts)


def side_legend(extra=""):
    return legend([("k-base", f"baseline{extra}"), ("k-cand", f"candidate{extra}")])


def median_run_stat(capture, metric, stat):
    return median_of(run_values(capture, metric, stat))


def system_items(captures, group="systems", prefix="system", limit=16):
    """Per-name (p50, p95) medians per side, sorted by baseline p50, dropping
    names under 0.005 ms on both sides."""
    names = ordered_names(list(captures.values()), group, gpu=prefix == "gpu")
    items = []
    for name in names:
        sides = {side: (median_run_stat(capture, f"{prefix}.{name}", "p50"), median_run_stat(capture, f"{prefix}.{name}", "p95")) for side, capture in captures.items()}
        if max((whisker or value or 0.0) for value, whisker in sides.values()) < 0.005:
            continue
        items.append((name, sides))
    items.sort(key=lambda item: -(item[1].get("baseline", (0.0, 0.0))[0] or 0.0))
    return items[:limit], len(items)


def judged_html_rows(results):
    return [
        [
            Markup(f"<code>{esc(result['metric'])}</code>"),
            result["stat"],
            fmt(result["base"]),
            fmt(result["cand"]),
            signed(result["delta"]),
            signed(result["delta_pct"], 1, "%"),
            interval_text(result),
            fmt(result["tau"], 3),
            badge(result),
        ]
        for result in results
    ]


def compare_html(report, captures, frames):
    """`captures`: {side: capture}; `frames`: {side: [[run-1 totals], ...]}."""
    base, cand = report["baseline"], report["candidate"]
    body = [f"<h1>Compare: {esc(cand['row'])}</h1>"]
    body.append(f'<p class="muted">Baseline <code>{esc(base["label"])}</code> ({esc(base["commit"])}) against candidate <code>{esc(cand["label"])}</code> ({esc(cand["commit"])}).</p>')
    body.append("<section><h2>Comparability</h2>")
    for line in comparability_lines(report):
        text = esc(line.lstrip("- ").replace("**", "").replace("`", ""))
        body.append(f"<p>{text}</p>")
    body.append(
        html_table(
            ["", "Baseline", "Candidate"],
            [
                ["Valid", "yes" if base["valid"] else "no", "yes" if cand["valid"] else "no"],
                ["Runs", base["runs"], cand["runs"]],
                ["Frames", base["frames"], cand["frames"]],
                ["CPU / adapter", f"{base['cpu']} / {base['adapter']}", f"{cand['cpu']} / {cand['adapter']}"],
                ["Present mode / latency", base["present"], cand["present"]],
            ],
        )
    )
    body.append("</section>")
    body.append(f"<section><h2>Owned metrics (ms)</h2><p>Verdicts: {esc(summary_text(report['owned_summary']))}.</p>")
    body.append(html_table(JUDGED_HEADER, judged_html_rows(report["owned"]), numeric=(2, 3, 4, 5, 7)))
    body.append("</section>")
    pooled = {side: [value for run in runs for value in run] for side, runs in frames.items()}
    body.append("<section><h2>Total frame time</h2>" + side_legend())
    body.append("<h3>ECDF of pooled measured frames, all runs</h3>" + ecdf_svg(pooled))
    stats_rows = [
        [side, len(values), *(fmt(percentile(sorted(values), pct)) if values else "n/a" for pct in PERCENTILES)]
        for side, values in pooled.items()
    ]
    body.append(html_table(["Side", "Frames", "p50", "p95", "p99"], stats_rows, numeric=(1, 2, 3, 4)))
    body.append("<h3>Run 1, frame by frame</h3>" + series_svg({side: runs[0] if runs else [] for side, runs in frames.items()}))
    body.append("</section>")
    body.append("<section><h2>Stages (mean ms)</h2>" + stack_svg(captures))
    body.append("<h3>Verdicts</h3>" + html_table(JUDGED_HEADER, judged_html_rows(report["stages"]), numeric=(2, 3, 4, 5, 7)))
    body.append("</section>")
    items, shown = system_items(captures)
    body.append("<section><h2>Systems (ms)</h2>" + side_legend(": p50 bar, p95 whisker"))
    body.append(f'<p class="muted">Median of per-run values, sorted by baseline p50; {len(items)} of {shown} systems above 0.005 ms shown.</p>')
    body.append(hbar_svg(items, "ms", "Per-system p50 and p95"))
    moved = [result for result in report["systems"] if result["delta"] is not None and abs(result["delta"]) > result["tau"]]
    if moved:
        body.append("<h3>Systems with |Δ| &gt; τ</h3>" + html_table(JUDGED_HEADER, judged_html_rows(moved), numeric=(2, 3, 4, 5, 7)))
    body.append("</section>")
    body.append("<section><h2>GPU passes (ms, diagnostic run)</h2>")
    if report["gpu"]:
        gpu_items, _ = system_items(captures, "gpu_passes", "gpu")
        body.append(side_legend(": p50 bar, p95 whisker") + hbar_svg(gpu_items, "ms", "GPU pass p50 and p95"))
        body.append(html_table(JUDGED_HEADER, judged_html_rows(report["gpu"]), numeric=(2, 3, 4, 5, 7)))
    else:
        body.append('<p class="muted">n/a: both captures need a GPU diagnostic run.</p>')
    body.append("</section>")
    memory = report["memory"]
    peak = memory["peak_rss"]
    rss_sides = {}
    for side, capture in captures.items():
        peaks = [run["rusage"]["peak_rss_kib"] / 1024 for run in capture["runs"] if run.get("rusage")]
        if peaks:
            rss_sides[side] = (statistics.fmean(peaks), max(peaks))
    rss_items = [("peak RSS", rss_sides)] if rss_sides else []
    body.append("<section><h2>Memory</h2>" + side_legend(": mean peak RSS, whisker to the largest run"))
    body.append(hbar_svg(rss_items, "MiB", "Peak RSS per side"))
    body.append(
        html_table(
            ["Metric", "Baseline", "Candidate", "Δ", "95% interval", "τ", "Verdict"],
            [
                [
                    "Peak RSS (MiB)",
                    fmt(None if peak["base"] is None else peak["base"] / 1024, 1),
                    fmt(None if peak["cand"] is None else peak["cand"] / 1024, 1),
                    signed(None if peak["delta"] is None else peak["delta"] / 1024, 1),
                    interval_text(peak, 1024, 1),
                    fmt(None if peak["tau"] is None else peak["tau"] / 1024, 1),
                    badge(peak),
                ],
                ["RSS growth (KiB/s)", *(fmt(value, 1) for value in memory["rss_growth_kib_s"]), "", "", "", "reported only"],
                ["CPU time (s per run)", *(fmt(value) for value in memory["cpu_s"]), "", "", "", "reported only"],
            ],
            numeric=(1, 2, 3, 5),
        )
    )
    body.append("</section>")
    counter_rows_html = [
        [Markup(f"<code>{esc(row['metric'])}</code>"), fmt(row["base"]), fmt(row["cand"]), signed(row["delta_pct"], 1, "%"), "drift" if row["drift"] else ""]
        for row in report["counters"]
    ]
    body.append("<section><h2>Workload counters (mean per frame)</h2>")
    body.append(html_table(["Counter", "Baseline", "Candidate", "Δ%", "Drift"], counter_rows_html, numeric=(1, 2, 3)))
    body.append("</section>")
    body.append("<section><h2>Guards and validity</h2>")
    for name, side in (("Baseline", base), ("Candidate", cand)):
        state = "valid" if side["valid"] else "INVALID"
        body.append(f"<h3>{name} ({state})</h3><ul>")
        body += [f"<li>{'ok' if guard['ok'] else 'FAIL'}: <code>{esc(guard['guard'])}</code> ({esc(guard['detail'])})</li>" for guard in side["guards"]]
        body += [f"<li>{esc(reason)}</li>" for reason in side["invalid_reasons"]]
        body.append("</ul>")
    body.append("</section>")
    return page(f"Compare {cand['row']}", "\n".join(body))


def frame_totals(capture_dir, capture):
    """Measured `total` per frame for each timing run, re-parsed from
    `run-N/telemetry.log`; capture.json holds only per-run statistics."""
    series = []
    for run in capture["runs"]:
        try:
            text = (Path(capture_dir) / f"run-{run['index']}" / "telemetry.log").read_text(errors="replace")
        except OSError:
            continue
        measured = parse_log(text).frames[capture["warmup"] : capture["warmup"] + capture["frames"]]
        series.append([frame.stages["total"] for frame in measured if frame.stages.get("total") is not None])
    return series


# --- Sweep report --------------------------------------------------------------


def spread(values):
    return {"median": statistics.median(values), "min": min(values), "max": max(values)} if values else None


def sweep_report(knob, entries):
    """`entries`: (value, capture directory name, capture) in sweep order."""
    owned = entries[0][2]["owned"]
    captures = []
    for value, name, capture in entries:
        runs = [run for run in capture["runs"] if run.get("stats")]
        metrics = {
            f"{entry['metric']} {stat}": spread(run_values(capture, entry["metric"], stat)) for entry in owned for stat in entry["stats"]
        }
        peaks = [run["rusage"]["peak_rss_kib"] / 1024 for run in capture["runs"] if run.get("rusage")]
        captures.append(
            {
                "value": value,
                "capture": name,
                "valid": capture["valid"],
                "invalid_reasons": capture["invalid_reasons"],
                "runs": len(runs),
                "metrics": metrics,
                "total_p95": median_stat(runs, "stages", "total", "p95"),
                "peak_rss_mib": statistics.median(peaks) if peaks else None,
            }
        )
    return {"schema": SCHEMA, "row": entries[0][2]["row"], "knob": knob, "metrics": list(captures[0]["metrics"]), "captures": captures}


def sweep_markdown(report):
    lines = [f"# Sweep: {report['row']} over `{report['knob']}`", ""]
    lines.append("Medians of per-run values in ms; peak RSS in MiB.")
    lines.append("")
    header = [report["knob"], *(f"`{name}`" for name in report["metrics"]), "total p95", "Peak RSS", "Valid", "Capture"]
    rows = []
    for entry in report["captures"]:
        metrics = [fmt(entry["metrics"][name]["median"]) if entry["metrics"][name] else "n/a" for name in report["metrics"]]
        valid = "yes" if entry["valid"] else "no: " + "; ".join(entry["invalid_reasons"])
        rows.append([entry["value"], *metrics, fmt(entry["total_p95"]), fmt(entry["peak_rss_mib"], 1), valid, f"`{entry['capture']}/`"])
    lines += table(header, rows)
    lines.append("")
    return "\n".join(lines)


def sweep_svg(report, name):
    """One owned metric against the swept values (evenly spaced, in sweep
    order): the median per value, whiskers from the lowest to the highest run."""
    points = [(index, entry["metrics"].get(name)) for index, entry in enumerate(report["captures"])]
    values = [value for _, point in points if point for value in (point["min"], point["max"])]
    if not values:
        return '<p class="muted">No data.</p>'
    count = len(points)
    high = tick_ceiling(max(values) * 1.02 or 1.0)
    frame = Frame2D(720, 260, (-0.5, count - 0.5), (0.0, high))
    parts = [frame.open(f"{name} against {report['knob']}")]
    parts.append(frame.y_grid(nice_ticks(0.0, high, 4)))
    base = frame.height - frame.bottom
    parts.append(f'<line class="axis" x1="{frame.left}" x2="{frame.width - frame.right}" y1="{base}" y2="{base}"/>')
    for index, entry in enumerate(report["captures"]):
        parts.append(f'<text x="{frame.x(index):.1f}" y="{base + 16}" text-anchor="middle">{esc(entry["value"])}</text>')
    parts.append(f'<text class="ink" x="{(frame.left + frame.width - frame.right) / 2:.1f}" y="{frame.height - 6}" text-anchor="middle">{esc(report["knob"])}</text>')
    parts.append(frame.y_title("ms"))
    line = [(frame.x(index), frame.y(point["median"])) for index, point in points if point]
    parts.append(polyline(line, "s-base"))
    for index, point in points:
        if not point:
            continue
        x = frame.x(index)
        if point["max"] > point["min"]:
            parts.append(f'<line class="whisker s-base" x1="{x:.1f}" x2="{x:.1f}" y1="{frame.y(point["min"]):.1f}" y2="{frame.y(point["max"]):.1f}"/>')
        label = f"{report['knob']}={report['captures'][index]['value']}: median {point['median']:.3f} ms (runs {point['min']:.3f}..{point['max']:.3f})"
        parts.append(f'<circle class="dot f-base" cx="{x:.1f}" cy="{frame.y(point["median"]):.1f}" r="4"><title>{esc(label)}</title></circle>')
        parts.append(f'<text x="{x + 7:.1f}" y="{frame.y(point["median"]) - 6:.1f}">{point["median"]:.2f}</text>')
    parts.append("</svg>")
    return "".join(parts)


def sweep_html(report):
    body = [f"<h1>Sweep: {esc(report['row'])} over {esc(report['knob'])}</h1>"]
    body.append('<p class="muted">Each point is the median of the per-run values; whiskers span the lowest and highest run.</p>')
    for name in report["metrics"]:
        body.append(f"<section><h2><code>{esc(name)}</code></h2>{sweep_svg(report, name)}</section>")
    rows = []
    for entry in report["captures"]:
        metrics = [fmt(entry["metrics"][name]["median"]) if entry["metrics"][name] else "n/a" for name in report["metrics"]]
        rows.append([entry["value"], *metrics, fmt(entry["total_p95"]), fmt(entry["peak_rss_mib"], 1), "yes" if entry["valid"] else "no", entry["capture"]])
    header = [report["knob"], *report["metrics"], "total p95", "Peak RSS (MiB)", "Valid", "Capture"]
    body.append("<section><h2>Table</h2>" + html_table(header, rows, numeric=tuple(range(1, len(report["metrics"]) + 3))) + "</section>")
    return page(f"Sweep {report['row']}", "\n".join(body))


# --- Capacity report -----------------------------------------------------------

# Limiting-stage candidates besides the top `update` system: label and the
# `frame:` keys whose means they sum.
LIMITING = (
    ("stage.flush", ("flush",)),
    ("stage.particles", ("particles",)),
    ("stage.extract", ("extract",)),
    ("stage.render_encode", ("render_encode",)),
    ("present", ("render_acquire", "render_submit_present")),
    ("stage.unattributed", ("unattributed",)),
)


def limiting_candidates(stats):
    """Mean ms of each limiting-stage candidate in one run: the top `update`
    system, flush, particles, extract, render_encode, the present wait and
    unattributed."""
    candidates = {}
    systems = stats.get("systems", {})
    if systems:
        top = max(systems, key=lambda name: systems[name]["mean"])
        candidates[f"system.{top}"] = systems[top]["mean"]
    stages = stats.get("stages", {})
    for label, parts in LIMITING:
        means = [stages[part]["mean"] for part in parts if part in stages]
        if means:
            candidates[label] = sum(means)
    return candidates


def limiting_stage(candidate_sets, bottleneck):
    """The candidate with the largest mean over the given runs, and whether it
    is the row's declared bottleneck (`update` matches any system)."""
    pooled = {}
    for candidates in candidate_sets:
        for label, value in candidates.items():
            pooled.setdefault(label, []).append(value)
    if not pooled:
        return None
    means = {label: statistics.fmean(values) for label, values in pooled.items()}
    stage = max(means, key=means.get)
    matches = stage == bottleneck or (bottleneck == "update" and stage.startswith("system."))
    return {"stage": stage, "ms": means[stage], "ok": matches, "bottleneck": bottleneck}


def capacity_result_text(row):
    value = row["value_text"]
    status = row["status"]
    if status == "bound":
        return f"≥ {value} (bound reached)"
    if status == "unreachable":
        return f"< {value} (budget not reachable)"
    if status == "unconfirmed":
        return f"unconfirmed ({value} failed its last confirmation)"
    return value


def limiting_text(limiting):
    if not limiting:
        return "n/a"
    mark = "✓" if limiting["ok"] else f"✗ (declared {limiting['bottleneck']})"
    return f"{limiting['stage']} {mark}, {limiting['ms']:.2f} ms"


def guard_note(row):
    """Guard failures over the row's probes; they never fail a probe."""
    notes = []
    for guard in {result["guard"] for probe in row["probes"] for result in probe["guards"]}:
        failing = [probe for probe in row["probes"] if any(result["guard"] == guard and not result["ok"] for result in probe["guards"])]
        if failing:
            values = ", ".join(sorted({probe["value_text"] for probe in failing}, key=float))
            notes.append(f"guard `{guard}` failed in {len(failing)} of {len(row['probes'])} probes ({row['axis']} {values})")
    return sorted(notes)


def capacity_markdown(report):
    budget = report["budget"]
    lines = [f"# Capacity: {budget['label']} ({budget['ms']:g} ms)", ""]
    lines.append(
        f"Statistic: `stage.total` {report['stat']} of {report['frames']} frames after each benchmark's warm-up; axis `{report['axis']}`; "
        f"tolerance {report['tolerance']:.0%}. Commit {report['provenance']['commit']} ({report['provenance']['dirty']}), "
        f"{report['provenance']['cpu']}. Machine-specific and informational (`D-070`)."
    )
    lines.append("")
    header = ["Row", f"Max {report['axis']}", "Key counts", "p95 (ms)", "p99 (ms)", "Limiting stage", "Peak RSS (MiB)", "Probes", "Wall time (s)"]
    rows = []
    for row in report["rows"]:
        counts = ", ".join(f"{name}={value}" for name, value in row["key_counts"].items())
        rows.append(
            [
                f"`{row['row']}`",
                capacity_result_text(row),
                counts,
                fmt(row["p95"]),
                fmt(row["p99"]),
                limiting_text(row["limiting"]),
                fmt(row["peak_rss_mib"], 1),
                len(row["probes"]),
                fmt(row["wall_s"], 0),
            ]
        )
    lines += table(header, rows)
    lines += ["", "p95, p99, the limiting stage and peak RSS are medians over the confirmation probes of the reported value."]
    for row in report["rows"]:
        lines += ["", f"## `{row['row']}`", ""]
        lines.append(f"- {row['axis']} range {row['lower_text']}..{row['upper_text']}, start {row['start_text']}; result: {capacity_result_text(row)}.")
        lines += [f"- {note}." for note in guard_note(row)]
        lines += [f"- probe failure at {probe['value_text']}: {'; '.join(probe['problems'])}" for probe in row["probes"] if probe["problems"]]
        lines.append("")
        lines += table(
            ["#", "Phase", row["axis"], f"{report['stat']} (ms)", "Pass", "Guards", "Probe"],
            [
                [
                    index,
                    probe["phase"],
                    probe["value_text"],
                    fmt(probe["stat"]),
                    "pass" if probe["passed"] else "fail",
                    "ok" if all(result["ok"] for result in probe["guards"]) else "FAIL",
                    f"`{probe['dir']}`",
                ]
                for index, probe in enumerate(row["probes"], start=1)
            ],
        )
    lines.append("")
    return "\n".join(lines)


def capacity_svg(row, report):
    """Probe statistic against the axis value on a log2 axis, with the budget line."""
    budget = report["budget"]["ms"]
    probes = row["probes"]
    if not probes:
        return '<p class="muted">No probes.</p>'
    xs = [probe["value"] for probe in probes]
    measured = [probe["stat"] for probe in probes if probe["stat"] is not None]
    high = tick_ceiling(max(measured + [budget]) * 1.05)
    x_low, x_high = min(xs) / 1.25, max(xs) * 1.25
    frame = Frame2D(720, 280, (x_low, x_high), (0.0, high), log_x=True)
    parts = [frame.open(f"{row['row']} capacity probes")]
    parts.append(frame.y_grid(nice_ticks(0.0, high, 4)))
    ticks = [2.0**power for power in range(math.floor(math.log2(x_low)), math.ceil(math.log2(x_high)) + 1) if x_low <= 2.0**power <= x_high]
    parts.append(frame.x_axis(ticks, tick_text, title=f"{row['axis']} (log scale)"))
    parts.append(frame.y_title(f"total {report['stat']} (ms)"))
    y = frame.y(budget)
    parts.append(f'<line class="budget" x1="{frame.left}" x2="{frame.width - frame.right}" y1="{y:.1f}" y2="{y:.1f}"/>')
    parts.append(f'<text class="ink" x="{frame.width - frame.right - 4}" y="{y - 4:.1f}" text-anchor="end">budget {budget:g} ms</text>')
    if row["status"] in ("max", "bound"):
        x = frame.x(row["value"])
        parts.append(f'<line class="budget" x1="{x:.1f}" x2="{x:.1f}" y1="{frame.top}" y2="{frame.height - frame.bottom}"/>')
        parts.append(f'<text class="ink" x="{x + 4:.1f}" y="{frame.top + 10}">max {esc(row["value_text"])}</text>')
    for index, probe in enumerate(probes, start=1):
        x = frame.x(probe["value"])
        label = f"probe {index} ({probe['phase']}): {row['axis']} {probe['value_text']}, "
        if probe["stat"] is None:
            label += "child failed: " + "; ".join(probe["problems"])
            top = frame.top + 4
            parts.append(
                f'<path class="line s-cand" d="M{x - 4:.1f},{top - 4:.1f} L{x + 4:.1f},{top + 4:.1f} M{x - 4:.1f},{top + 4:.1f} L{x + 4:.1f},{top - 4:.1f}">'
                f"<title>{esc(label)}</title></path>"
            )
            continue
        label += f"{probe['stat']:.2f} ms, {'pass' if probe['passed'] else 'fail'}"
        cls = "dot f-base" if probe["passed"] else "hollow s-cand"
        parts.append(f'<circle class="{cls}" cx="{x:.1f}" cy="{frame.y(probe["stat"]):.1f}" r="4"><title>{esc(label)}</title></circle>')
    parts.append("</svg>")
    return "".join(parts)


def capacity_html(report):
    budget = report["budget"]
    body = [f"<h1>Capacity: {esc(budget['label'])} ({budget['ms']:g} ms)</h1>"]
    body.append(
        f'<p class="muted">Statistic: stage.total {esc(report["stat"])} over {report["frames"]} measured frames per probe; '
        f"axis {esc(report['axis'])}; tolerance {report['tolerance']:.0%}. Machine-specific and informational.</p>"
    )
    rows = []
    for row in report["rows"]:
        rows.append(
            [
                row["row"],
                capacity_result_text(row),
                ", ".join(f"{name}={value}" for name, value in row["key_counts"].items()),
                fmt(row["p95"]),
                fmt(row["p99"]),
                limiting_text(row["limiting"]),
                fmt(row["peak_rss_mib"], 1),
                len(row["probes"]),
                fmt(row["wall_s"], 0),
            ]
        )
    header = ["Row", f"Max {report['axis']}", "Key counts", "p95 (ms)", "p99 (ms)", "Limiting stage", "Peak RSS (MiB)", "Probes", "Wall (s)"]
    body.append("<section><h2>Results</h2>" + html_table(header, rows, numeric=(3, 4, 6, 7, 8)) + "</section>")
    for row in report["rows"]:
        body.append(f"<section><h2>{esc(row['row'])}</h2>")
        body.append(legend([("k-base", "pass (filled)"), ("k-cand", "fail (hollow; × is a failed child)")]))
        body.append(capacity_svg(row, report))
        notes = [note.replace("`", "") for note in guard_note(row)]
        if notes:
            body.append("<ul>" + "".join(f"<li>{esc(note)}</li>" for note in notes) + "</ul>")
        body.append("</section>")
    return page(f"Capacity {budget['label']}", "\n".join(body))


# --- Suites --------------------------------------------------------------------

SUITE_TOTAL_STATS = ("p50", "p95", "p99", "jitter")


def suite_row(capture, directory):
    """One `suite.json` row: validity, owned medians, `total` medians and the
    median peak RSS of the row's capture in `directory`."""
    runs = [run for run in capture["runs"] if run.get("stats")]
    peaks = [run["rusage"]["peak_rss_kib"] / 1024 for run in capture["runs"] if run.get("rusage")]
    return {
        "row": capture["row"],
        "bench": capture["bench"],
        "preset": capture["config"]["preset"],
        "dir": directory,
        "valid": capture["valid"],
        "invalid_reasons": capture["invalid_reasons"],
        "runs": len(runs),
        "owned": [
            {"metric": entry["metric"], "stat": stat, "median": median_of(owned_per_run(capture, entry["metric"], stat))}
            for entry in capture["owned"]
            for stat in entry["stats"]
        ],
        "total": {stat: median_of([metric_value(run["stats"], "stage.total", stat) for run in runs]) for stat in SUITE_TOTAL_STATS},
        "peak_rss_mib": median_of(peaks),
    }


def assemble_suite(*, request, build, provenance, entries):
    """`suite.json` (schema 1): the request, build and provenance every row
    shares, plus one row per `(directory, capture)` in `entries`. A suite is
    valid when every row's capture is."""
    rows = [suite_row(capture, directory) for directory, capture in entries]
    return {
        "schema": SCHEMA,
        "kind": "suite",
        "request": request,
        "build": build,
        "provenance": provenance,
        "rows": rows,
        "valid": bool(rows) and all(row["valid"] for row in rows),
    }


def suite_preset_note(request):
    if not request.get("preset"):
        return None
    return (
        f"`--preset {request['preset']}` replaced the preset of the rows whose own preset is `default`; "
        "rows defined by another preset kept theirs."
    )


def suite_readme(suite):
    """The suite's `README.md`: medians per row; each row's README has the rest."""
    request, provenance = suite["request"], suite["provenance"]
    lines = ["# Suite", ""]
    lines += table(
        ["Field", "Value"],
        [
            ["Valid", "yes" if suite["valid"] else "no"],
            ["Rows", ", ".join(f"`{row['row']}`" for row in suite["rows"]) or "none"],
            ["Preset override", request.get("preset") or "none: each row's preset"],
            ["Scale", request.get("scale") or "1"],
            ["Runs per row", request["repeat"]],
            ["Frames", f"{request['frames']} measured after each benchmark's warm-up"],
            ["Commit", f"{provenance['commit']} (dirty: {provenance['dirty']})"],
            ["Build RUSTFLAGS", f"`{suite['build']['rustflags']}`"],
            ["CPU / kernel", f"{provenance['cpu']} / {provenance['kernel']}"],
            ["Machine", provenance["machine"]],
        ],
    )
    note = suite_preset_note(request)
    if note:
        lines += ["", note]
    lines += ["", "## Rows", "", "Medians of the per-run values in ms; peak RSS in MiB. Jitter is p99 − p50 of `total`.", ""]
    rows = []
    for row in suite["rows"]:
        owned = "; ".join(f"`{entry['metric']}` {entry['stat']} {fmt(entry['median'])}" for entry in row["owned"])
        total = " / ".join(fmt(row["total"][stat]) for stat in ("p50", "p95", "p99"))
        valid = "yes" if row["valid"] else "**no**"
        rows.append([f"`{row['row']}`", row["preset"], valid, owned, total, fmt(row["total"]["jitter"]), fmt(row["peak_rss_mib"], 1), f"`{row['dir']}/`"])
    lines += table(["Row", "Preset", "Valid", "Owned metrics", "total p50 / p95 / p99", "Jitter", "Peak RSS", "Capture"], rows)
    invalid = [row for row in suite["rows"] if not row["valid"]]
    if invalid:
        lines += ["", "## Invalid rows", ""]
        for row in invalid:
            lines.append(f"- `{row['row']}`:")
            lines += [f"  - {reason}" for reason in row["invalid_reasons"]]
    lines.append("")
    return "\n".join(lines)


def suite_compare(base_suite, cand_suite, row_reports, *, base_side, cand_side):
    """Suite-level summary over per-row `compare()` reports. `row_reports`
    maps each row both suites hold to its report; `*_side` is `(label, path)`."""
    base_rows = {row["row"]: row for row in base_suite["rows"]}
    cand_rows = {row["row"]: row for row in cand_suite["rows"]}
    summary = dict.fromkeys((*VERDICTS, "none"), 0)
    rows = []
    for name, report in row_reports.items():
        for verdict, count in report["owned_summary"].items():
            summary[verdict] += count
        rows.append(
            {
                "row": name,
                "dir": name,
                "comparable": report["comparable"],
                "blocked": report["blocked"],
                "hard": report["hard"],
                "invalid": report["invalid"],
                "drift": report["drift"],
                "owned_summary": report["owned_summary"],
                "owned": report["owned"],
                "total": {"baseline": base_rows[name]["total"], "candidate": cand_rows[name]["total"]},
                "peak_rss": report["memory"]["peak_rss"],
            }
        )

    def side(suite, label, path):
        provenance = suite["provenance"]
        return {
            "label": label,
            "path": str(path),
            "commit": provenance.get("commit"),
            "dirty": provenance.get("dirty"),
            "valid": suite["valid"],
            "rows": [row["row"] for row in suite["rows"]],
            "request": suite["request"],
        }

    return {
        "schema": SCHEMA,
        "kind": "suite",
        "baseline": side(base_suite, *base_side),
        "candidate": side(cand_suite, *cand_side),
        "rows": rows,
        "only_baseline": [name for name in base_rows if name not in cand_rows],
        "only_candidate": [name for name in cand_rows if name not in base_rows],
        "owned_summary": summary,
    }


def suite_row_notes(row):
    notes = []
    if row["hard"]:
        notes.append("not comparable: " + "; ".join(row["hard"]))
    elif row["blocked"]:
        notes.append("verdicts suppressed: " + "; ".join(row["invalid"]))
    if row["drift"]:
        notes.append("workload drift: " + ", ".join(row["drift"]))
    return notes


def peak_rss_text(peak):
    base, cand = (None if value is None else value / 1024 for value in (peak["base"], peak["cand"]))
    return f"{fmt(base, 1)} → {fmt(cand, 1)}"


def suite_compare_markdown(report):
    base, cand = report["baseline"], report["candidate"]
    lines = ["# Compare suites", ""]
    lines += table(
        ["", "Baseline", "Candidate"],
        [
            ["Suite", f"`{base['label']}`", f"`{cand['label']}`"],
            ["Commit (dirty)", f"{base['commit']} ({base['dirty']})", f"{cand['commit']} ({cand['dirty']})"],
            ["Valid", "yes" if base["valid"] else "no", "yes" if cand["valid"] else "no"],
            ["Rows", len(base["rows"]), len(cand["rows"])],
        ],
    )
    lines += ["", f"Owned verdicts over {len(report['rows'])} rows: {summary_text(report['owned_summary'])}."]
    for label, names in (("Only in the baseline", report["only_baseline"]), ("Only in the candidate", report["only_candidate"])):
        if names:
            lines.append(f"{label}, not compared: {', '.join(f'`{name}`' for name in names)}.")
    lines += ["", "## Rows", "", "total p95 in ms and peak RSS in MiB, baseline → candidate; medians of per-run values.", ""]
    summary_rows = []
    for row in report["rows"]:
        total = f"{fmt(row['total']['baseline']['p95'])} → {fmt(row['total']['candidate']['p95'])}"
        comparable = "yes" if row["comparable"] else "**no**"
        summary_rows.append(
            [f"`{row['row']}`", comparable, summary_text(row["owned_summary"]), total, peak_rss_text(row["peak_rss"]), verdict_text(row["peak_rss"]), f"`{row['dir']}/compare.md`"]
        )
    lines += table(["Row", "Comparable", "Owned verdicts", "total p95", "Peak RSS", "Peak RSS verdict", "Report"], summary_rows)
    for row in report["rows"]:
        lines += ["", f"## `{row['row']}`", ""]
        lines += [f"- {note}" for note in suite_row_notes(row)]
        lines += table(JUDGED_HEADER, judged_rows(row["owned"]))
        lines += ["", f"Full report: `{row['dir']}/compare.md` and `{row['dir']}/compare.html`."]
    lines += ["", "Verdicts follow A5 as in each row's report; jitter (p99 − p50) takes p99's τ.", ""]
    return "\n".join(lines)


def suite_compare_html(report):
    """Self-contained suite page: verdict counts, per-row `total` and peak-RSS
    bars, and each row's owned-metric table. Row reports sit in `<row>/`."""
    base, cand = report["baseline"], report["candidate"]
    body = ["<h1>Compare suites</h1>"]
    body.append(f'<p class="muted">Baseline <code>{esc(base["label"])}</code> ({esc(base["commit"])}) against candidate <code>{esc(cand["label"])}</code> ({esc(cand["commit"])}).</p>')
    body.append(f"<section><h2>Summary</h2><p>Owned verdicts over {len(report['rows'])} rows: {esc(summary_text(report['owned_summary']))}.</p>")
    for label, names in (("Only in the baseline", report["only_baseline"]), ("Only in the candidate", report["only_candidate"])):
        if names:
            body.append(f"<p>{label}, not compared: {esc(', '.join(names))}.</p>")
    rows = []
    for row in report["rows"]:
        rows.append(
            [
                Markup(f"<code>{esc(row['row'])}</code>"),
                "yes" if row["comparable"] else "no",
                summary_text(row["owned_summary"]),
                f"{fmt(row['total']['baseline']['p95'])} → {fmt(row['total']['candidate']['p95'])}",
                peak_rss_text(row["peak_rss"]),
                badge(row["peak_rss"]),
                f"{row['dir']}/compare.html",
            ]
        )
    body.append(html_table(["Row", "Comparable", "Owned verdicts", "total p95 (ms)", "Peak RSS (MiB)", "Peak RSS verdict", "Row report"], rows, numeric=(3, 4)))
    body.append("</section>")
    totals = [
        (row["row"], {side: (row["total"][side]["p50"], row["total"][side]["p95"]) for side, _ in SIDES if row["total"][side]["p50"] is not None})
        for row in report["rows"]
    ]
    body.append("<section><h2>Total frame time (ms)</h2>" + side_legend(": median p50 bar, p95 whisker"))
    body.append(hbar_svg(totals, "ms", "total p50 and p95 per row"))
    body.append("</section>")
    peaks = []
    for row in report["rows"]:
        peak = row["peak_rss"]
        sides = {side: (peak[key] / 1024, None) for side, key in SIDES if peak[key] is not None}
        peaks.append((row["row"], sides))
    body.append("<section><h2>Peak RSS (MiB)</h2>" + side_legend(": mean of the runs' peaks") + hbar_svg(peaks, "MiB", "Peak RSS per row") + "</section>")
    for row in report["rows"]:
        body.append(f"<section><h2>{esc(row['row'])}</h2>")
        body += [f"<p>{esc(note)}</p>" for note in suite_row_notes(row)]
        body.append(html_table(JUDGED_HEADER, judged_html_rows(row["owned"]), numeric=(2, 3, 4, 5, 7)))
        body.append(f'<p class="muted">Full report: {esc(row["dir"])}/compare.html</p></section>')
    return page("Compare suites", "\n".join(body))
