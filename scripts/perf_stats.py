#!/usr/bin/env python3
"""Telemetry parsing, per-run statistics, guards, the workload digest and
verdicts for `scripts/perf.py` (redo 7.3, `D-078`). Pure: no process, file
or clock access, so `scripts/test-perf.py` imports it as it is. Standard
library only (Python 3.12).
"""

import dataclasses
import hashlib
import math
import re
import statistics

SCHEMA = 2

# Companion lines attach to the latest `frame:` line before them; the others
# describe the whole run.
COMPANIONS = ("systems", "gpu_passes", "physics", "bench", "counters")
LINE_RE = re.compile(r"^(frame|systems|gpu_passes|physics|bench-config|bench|counters|backend):(?:\s+|$)(.*)$")
LOG_PREFIX_RE = re.compile(r"^\[[^\]]*\]\s*")
BACKEND_RE = re.compile(
    r"^(\S+) adapter: (.+) present_mode: (\S+) max_frame_latency: (\d+) timestamp_query: (true|false)$"
)

# `frame:` stages that partition `total`; the remainder is `unattributed`.
TOTAL_PARTS = ("update", "flush", "particles", "tweens", "hot_reload", "extract", "render", "audio")
PERCENTILES = (50, 95, 99)


@dataclasses.dataclass
class Frame:
    """One `frame:` line and the companion lines that followed it."""

    stages: dict
    systems: dict | None = None
    gpu_passes: dict | None = None
    physics: dict | None = None
    bench: dict | None = None
    counters: dict | None = None
    raw: dict = dataclasses.field(default_factory=dict)


@dataclasses.dataclass
class Log:
    frames: list
    backend: dict | None = None
    bench_config: str | None = None
    orphans: int = 0


def parse_value(text):
    """`12.34ms` -> 12.34, `8000` -> 8000.0, `n/a` -> None."""
    text = text.removesuffix("ms")
    if text == "n/a":
        return None
    try:
        return float(text)
    except ValueError:
        return None


def parse_pairs(text):
    """`name=value` tokens in line order; names are literal keys."""
    pairs = {}
    for token in text.split():
        name, sep, value = token.partition("=")
        if sep and name:
            pairs[name] = parse_value(value)
    return pairs


def parse_log(text):
    """Group telemetry lines into frames. A companion before the first
    `frame:` line is an orphan; a missing companion is tolerated."""
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
    return log


def percentile(sorted_values, pct):
    """Nearest rank: the ceil(pct * n / 100)-th smallest value, clamped to [1, n]."""
    count = len(sorted_values)
    rank = min(count, max(1, -(-pct * count // 100)))
    return sorted_values[rank - 1]


def summarize(values):
    """`{p50, p95, p99, mean}` of the values present, or None."""
    values = sorted(value for value in values if value is not None)
    if not values:
        return None
    summary = {f"p{pct}": percentile(values, pct) for pct in PERCENTILES}
    summary["mean"] = statistics.fmean(values)
    return summary


def unattributed(stages):
    total = stages.get("total")
    if total is None:
        return None
    return total - sum(stages.get(part) or 0.0 for part in TOTAL_PARTS)


def group_stats(frames, pick):
    """Summaries of each metric `pick(frame)` yields, in first-seen order."""
    samples = {}
    for frame in frames:
        for name, value in (pick(frame) or {}).items():
            samples.setdefault(name, []).append(value)
    return {name: summary for name, values in samples.items() if (summary := summarize(values))}


def cpu_stats(frames):
    """A timing run's `cpu` block: stages, systems and counters. Counters
    hold the `counters:` line's fields as they are, and the `bench:` and
    `physics:` fields under `bench.` and `physics.`."""

    def counters(frame):
        values = dict(frame.counters or {})
        values.update({f"bench.{name}": value for name, value in (frame.bench or {}).items()})
        values.update({f"physics.{name}": value for name, value in (frame.physics or {}).items()})
        return values

    return {
        "stages": group_stats(frames, lambda frame: {**frame.stages, "unattributed": unattributed(frame.stages)}),
        "systems": group_stats(frames, lambda frame: frame.systems),
        "counters": group_stats(frames, counters),
    }


def gpu_stats(frames):
    """A GPU run's `gpu` block: its frame stages and its GPU passes."""
    return {
        "stages": group_stats(frames, lambda frame: frame.stages),
        "passes": group_stats(frames, lambda frame: frame.gpu_passes),
    }


def digest(frames):
    """The workload's fingerprint over a measured window: each frame's
    `physics:` line, and the `bench:` line of every frame but the last. A
    frame's `bench:` line prints in the next frame, so the last one exists
    only when tail frames follow; leaving it out makes a run with tails hash
    as one without. Equal builds at the pinned dt replay equal workloads."""
    sha = hashlib.sha256()
    last = len(frames) - 1
    for index, frame in enumerate(frames):
        if "physics" in frame.raw:
            sha.update(f"physics:{frame.raw['physics']}\n".encode())
        if index < last and "bench" in frame.raw:
            sha.update(f"bench:{frame.raw['bench']}\n".encode())
        sha.update(b"--\n")
    return sha.hexdigest()[:16]


# --- Guards ------------------------------------------------------------------

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
    """One guard over the measured frames: `physics:` on every frame,
    `bench:` on every frame but the last (its line prints in the next)."""
    kind = guard["kind"]
    if kind == "physics_max":
        group, key, checked = "physics", guard["field"], frames
    elif kind in COUNTER_GUARDS:
        group, key, checked = "bench", guard["counter"], frames[:-1]
    else:
        return {"guard": guard_name(guard), "ok": False, "detail": f"unknown guard kind '{kind}'"}
    name = guard_name(guard)
    if not checked:
        return {"guard": name, "ok": False, "detail": "no measured frames"}
    other_key = guard.get("other") if kind == "counter_eq" else None
    present = [value for frame in checked if (value := (getattr(frame, group) or {}).get(key)) is not None]
    median = statistics.median(present) if kind == "counter_band" and present else None
    bad, values = [], []
    for index, frame in enumerate(checked, start=1):
        pairs = getattr(frame, group) or {}
        value = pairs.get(key)
        other = pairs.get(other_key) if other_key else None
        if value is None or (other_key and other is None):
            bad.append((index, "missing"))
            continue
        values.append(value)
        if guard_fails(guard, value, values[0] if median is None else median, other):
            bad.append((index, f"{key}={value:g}"))
    if bad:
        first_index, first_value = bad[0]
        detail = f"{len(bad)} of {len(checked)} frames fail; first: measured frame {first_index} ({first_value})"
        return {"guard": name, "ok": False, "detail": detail}
    return {"guard": name, "ok": True, "detail": f"{len(checked)} frames, {key} in {min(values):g}..{max(values):g}"}


def analyze(text, warmup, frames, guards):
    """One run's measured window, its stats inputs, guard results, digest and
    problems: frames missing from the window and failing guards."""
    log = parse_log(text)
    measured = log.frames[warmup : warmup + frames]
    problems = []
    if len(measured) < frames:
        problems.append(f"{len(measured)} measured frames of {frames} ({len(log.frames)} logged, {warmup} warm-up)")
    results = [evaluate_guard(guard, measured) for guard in guards]
    problems.extend(f"guard {result['guard']}: {result['detail']}" for result in results if not result["ok"])
    return {"log": log, "measured": measured, "guards": results, "digest": digest(measured), "problems": problems}


# --- Verdicts (`D-078`) ------------------------------------------------------

MIN_RUNS = 3
# Each side's variance is floored at (0.5% of its mean)^2.
VARIANCE_FLOOR = 0.005
TAU_REL = {"mean": 0.03, "p50": 0.03, "p95": 0.05, "p99": 0.08}
TAU_ABS_MS = {"stages": 0.05, "systems": 0.02, "passes": 0.02}
RSS_TAU_REL = 0.02
RSS_TAU_ABS_KIB = 2 * 1024
# Two-sided 95% Student t quantiles by degrees of freedom; an untabulated df
# uses the next smaller entry, which is wider.
T_975 = {
    1: 12.706, 2: 4.303, 3: 3.182, 4: 2.776, 5: 2.571, 6: 2.447, 7: 2.365, 8: 2.306, 9: 2.262, 10: 2.228,
    11: 2.201, 12: 2.179, 13: 2.160, 14: 2.145, 15: 2.131, 16: 2.120, 17: 2.110, 18: 2.101, 19: 2.093,
    20: 2.086, 21: 2.080, 22: 2.074, 23: 2.069, 24: 2.064, 25: 2.060, 26: 2.056, 27: 2.052, 28: 2.048,
    29: 2.045, 30: 2.042, 40: 2.021, 60: 2.000, 120: 1.980,
}  # fmt: skip
VERDICTS = ("regressed", "improved", "unchanged", "noisy")


def t_quantile(df):
    whole = max(1, math.floor(df))
    return T_975[max(key for key in T_975 if key <= whole)]


def floored_share(values, mean):
    return max(statistics.variance(values), (VARIANCE_FLOOR * mean) ** 2) / len(values)


def judge(base, cand, tau_rel, tau_abs):
    """The `D-078` verdict on per-run values, higher being worse: Δ of the
    means, its 95% Welch interval with each variance floored, τ = max(τ_rel ×
    the base mean, τ_abs), and the verdict, None with fewer than MIN_RUNS runs
    on a side."""
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
    tau = max(tau_rel * abs(mean_base), tau_abs)
    result.update(delta=delta, tau=tau)
    result["delta_pct"] = 100 * delta / mean_base if abs(mean_base) > 1e-9 else None
    if min(len(base), len(cand)) < MIN_RUNS:
        return result
    shares = [floored_share(values, mean) for values, mean in ((base, mean_base), (cand, mean_cand))]
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


def interval(values):
    """The 95% interval of the mean of per-run values, variance floored, or
    None with fewer than two."""
    if len(values) < 2:
        return None
    mean = statistics.fmean(values)
    half = t_quantile(len(values) - 1) * math.sqrt(floored_share(values, mean))
    return [mean - half, mean + half]


def budget_verdict(values, budget):
    """`within` when the candidate's interval lies under `budget`, `over`
    when it lies above, `near` when it straddles; None without an interval."""
    bounds = interval(values)
    if bounds is None:
        return None
    low, high = bounds
    if high < budget:
        return "within"
    if low > budget:
        return "over"
    return "near"


# --- Captures ----------------------------------------------------------------

# Metric prefix -> (run block, group in it).
METRIC_GROUPS = {
    "stage": ("cpu", "stages"),
    "system": ("cpu", "systems"),
    "counter": ("cpu", "counters"),
    "gpu": ("gpu", "passes"),
    "gpustage": ("gpu", "stages"),
}


def metric_values(capture, metric, stat):
    """Per-run values of `metric` (`stage.update`, `gpu.scene`) at `stat`."""
    prefix, _, name = metric.partition(".")
    block, group = METRIC_GROUPS.get(prefix, (None, None))
    values = []
    for run in capture.get("runs", []):
        summary = ((run.get(block) or {}).get(group) or {}).get(name)
        if summary and summary.get(stat) is not None:
            values.append(summary[stat])
    return values


def metric_group(metric):
    return METRIC_GROUPS.get(metric.partition(".")[0], (None, None))[1]


def hard_mismatches(base, cand):
    """Every field two captures must share for a verdict: the row, the
    workload (version, knobs, script, resolution, warm-up, frames), the build
    flags, the renderer and the machine, each read from the captures."""
    hard = []

    def differ(label, left, right):
        if left != right:
            hard.append(f"{label}: {'missing' if left is None else left} vs {'missing' if right is None else right}")

    differ("row", base.get("row"), cand.get("row"))
    base_load, cand_load = base.get("workload") or {}, cand.get("workload") or {}
    differ("workload version", base_load.get("version"), cand_load.get("version"))
    base_knobs = ((base_load.get("config") or {}).get("knobs")) or {}
    cand_knobs = ((cand_load.get("config") or {}).get("knobs")) or {}
    for name in sorted(set(base_knobs) | set(cand_knobs)):
        differ(f"knob {name}", base_knobs.get(name), cand_knobs.get(name))
    for field, label in (("script_sha256", "script"), ("resolution", "resolution"), ("warmup", "warm-up"), ("frames", "frames")):
        differ(label, base_load.get(field), cand_load.get(field))
    differ("build flags", (base.get("build") or {}).get("flags"), (cand.get("build") or {}).get("flags"))
    base_renderer, cand_renderer = base.get("renderer") or {}, cand.get("renderer") or {}
    for field in ("backend", "adapter", "present_mode", "max_frame_latency"):
        differ(field.replace("_", " "), base_renderer.get(field), cand_renderer.get(field))
    differ("machine", (base.get("machine") or {}).get("fingerprint"), (cand.get("machine") or {}).get("fingerprint"))
    return hard


def rss_values(capture):
    return [run["rss_peak_kib"] for run in capture.get("runs", []) if run.get("rss_peak_kib") is not None]


def compare(base, cand, owned, budgets=()):
    """Verdicts of `cand` against `base` on each owned `{metric, stats}`,
    suppressed on any hard mismatch or an invalid side. Peak RSS is judged
    only between captures of one sitting. Budgets judge the candidate alone."""
    hard = hard_mismatches(base, cand)
    invalid = [f"{side} invalid" for side, capture in (("baseline", base), ("candidate", cand)) if not capture.get("valid")]
    blocked = "; ".join(hard + invalid) or None
    results = []
    for entry in owned:
        metric = entry["metric"]
        group = metric_group(metric)
        for stat in entry["stats"]:
            result = {"metric": metric, "stat": stat}
            result.update(judge(metric_values(base, metric, stat), metric_values(cand, metric, stat), TAU_REL.get(stat, 0.0), TAU_ABS_MS.get(group, 0.0)))
            if blocked:
                result["verdict"] = None
            results.append(result)
    sitting = cand.get("sitting")
    rss = judge(rss_values(base), rss_values(cand), RSS_TAU_REL, RSS_TAU_ABS_KIB)
    if blocked or sitting is None or base.get("sitting") != sitting:
        rss["verdict"] = None
        rss["note"] = blocked or "different sittings: printed, not judged"
    budget_results = []
    for budget in budgets:
        values = metric_values(cand, budget["metric"], budget["stat"])
        budget_results.append({**budget, "verdict": budget_verdict(values, budget["ms"]), "interval": interval(values)})
    return {
        "hard": hard,
        "invalid": invalid,
        "owned": results,
        "rss": rss,
        "budgets": budget_results,
        "digests": {"base": first_digest(base), "cand": first_digest(cand)},
    }


def first_digest(capture):
    runs = capture.get("runs") or []
    return runs[0].get("digest") if runs else None


def regressed(report):
    return sorted({f"{result['metric']} {result['stat']}" for result in report["owned"] if result["verdict"] == "regressed"})
