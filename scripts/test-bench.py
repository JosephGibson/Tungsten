#!/usr/bin/env python3
"""Synthetic tests for scripts/bench.py and scripts/bench_report.py; no GPU or cargo."""

import argparse
import contextlib
import copy
import hashlib
import html.parser
import io
import json
import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent))
import bench  # noqa: E402
import bench_report  # noqa: E402

APP = "[2026-09-30T00:00:00Z DEBUG tungsten::app] "
BENCH = "[2026-09-30T00:00:00Z DEBUG bench] "
CONFIG = {"bench": "physics", "row": "physics", "knobs": {"balls": 8000}}
CONFIG_LINE = '[2026-09-30T00:00:00Z INFO  bench] bench-config: {"bench":"physics","knobs":{"balls":8000},"row":"physics"}'
GUARDS = [
    {"kind": "physics_max", "field": "sleeping", "max": 0},
    {"kind": "counter_min", "counter": "teleports", "min": 1},
]


def frame_line(total, update=6.0, interval=None):
    """`interval` is the field's text (`16.70ms`, or `n/a` on a first frame);
    None leaves the field out, as a log from before it does."""
    field = "" if interval is None else f"interval={interval} "
    return (
        APP + f"frame: total={total:.2f}ms {field}update={update:.2f}ms flush=0.10ms extract=1.00ms render=2.00ms "
        "render_acquire=0.10ms render_encode=1.00ms render_submit_present=0.90ms gpu=n/a audio=0.01ms hot_reload=0.00ms"
    )


def physics_frame(total, sleeping=0, teleports=3, bench_line=True):
    lines = [
        frame_line(total),
        APP + f"systems: bench_counters=0.01ms physics_step={total / 2:.2f}ms",
        APP + "gpu_passes:",
        APP + f"physics: proxies=9 dynamic=8 sleeping={sleeping} pairs=6 contacts=4",
    ]
    if bench_line:
        lines.append(BENCH + f"bench: balls=8 teleports={teleports} events=12")
    return lines


def physics_log(frames):
    lines = [CONFIG_LINE]
    for frame in frames:
        lines.extend(frame)
    return "\n".join(lines) + "\n"


ROW = {"name": "physics", "preset": "default", "owned": [{"metric": "system.physics_step", "stats": ["p50", "p95"]}], "guards": GUARDS}
INTEGRATED_ROW = {
    "name": "integrated",
    "preset": "default",
    "owned": [{"metric": "stage.total", "stats": ["p50", "p95", "p99", "jitter"]}],
    "guards": [],
}
PROVENANCE_KEYS = ("commit", "dirty", "rustc", "cpu", "kernel", "governor", "platform_profile", "ac_power", "machine")
SUITE_REQUEST = {"preset": None, "scale": None, "only": None, "repeat": 3, "frames": 3}


def write_capture(directory, run_totals, peaks_kib, row=ROW, teleports=3):
    """A capture directory with one telemetry log per run, built from each
    run's per-frame totals, plus its capture.json. `teleports` changes the
    `bench:` text, so it moves the digest."""
    runs = []
    for index, (totals, peak) in enumerate(zip(run_totals, peaks_kib), start=1):
        text = physics_log([physics_frame(total, teleports=teleports) for total in totals])
        (directory / f"run-{index}").mkdir(parents=True)
        (directory / f"run-{index}" / "telemetry.log").write_text(text)
        usage = {"peak_rss_kib": peak, "user_s": 1.0, "sys_s": 0.1, "minflt": 1, "majflt": 0, "nvcsw": 2, "nivcsw": 3}
        run = {"index": index, "exit_code": 0, "rusage": usage, "rss_growth_kib_s": 0.5}
        run.update(bench_report.analyze_log(text, 0, len(totals), GUARDS, CONFIG))
        runs.append(run)
    capture = bench_report.assemble_capture(
        request={"set": [], "repeat": len(runs), "gpu_timing": False, "present_mode": None, "max_frame_latency": None},
        bench={"name": "physics", "workload_version": 1},
        row=row,
        config={**CONFIG, "preset": "default", "scale": 1.0},
        warmup=0,
        frames=len(run_totals[0]),
        build={"rustflags": bench.DEFAULT_RUSTFLAGS},
        provenance=dict.fromkeys(PROVENANCE_KEYS, "x"),
        runs=runs,
        profile=None,
    )
    (directory / "capture.json").write_text(json.dumps(capture))
    return capture


def shifted(run_totals, factor):
    return [[total * factor for total in totals] for totals in run_totals]


def write_suite(directory, run_totals, peaks_kib, rows=("physics", "integrated")):
    """A suite directory: one synthetic capture per row plus suite.json."""
    entries = []
    for name in rows:
        row = INTEGRATED_ROW if name == "integrated" else {**ROW, "name": name}
        entries.append((name, write_capture(directory / name, run_totals, peaks_kib, row=row)))
    suite = bench_report.assemble_suite(
        request=SUITE_REQUEST,
        build={"rustflags": bench.DEFAULT_RUSTFLAGS},
        provenance=dict.fromkeys(PROVENANCE_KEYS, "x"),
        entries=entries,
    )
    (directory / "suite.json").write_text(json.dumps(suite))
    return suite


BASE_TOTALS = [[10.0, 11.0, 12.0], [10.2, 11.1, 12.1], [9.9, 10.9, 11.8]]
BASE_PEAKS = [100_000, 100_100, 99_900]
BUILT_TREE = {"commit": "abc1234", "dirty": "no"}
FAKE_CONFIG = {**CONFIG, "preset": "default", "scale": 1.0}


def fake_bench(directory, sleep):
    """An executable standing in for example-02-bench: it counts its runs in
    `runs`, holds `alive` (its run number) for `sleep` seconds, then prints a
    valid three-frame physics log."""
    config_line = "[2026-09-30T00:00:00Z INFO  bench] bench-config: " + json.dumps(FAKE_CONFIG)
    log = "\n".join([config_line, *(line for total in (10.0, 11.0, 12.0) for line in physics_frame(total))]) + "\n"
    script = directory / "fake-bench"
    script.write_text(
        f"#!{sys.executable}\n"
        "import pathlib, sys, time\n"
        f"state = pathlib.Path({str(directory)!r})\n"
        "count = state / 'runs'\n"
        "run = int(count.read_text()) + 1 if count.exists() else 1\n"
        "count.write_text(str(run))\n"
        "(state / 'alive').write_text(str(run))\n"
        f"time.sleep({sleep})\n"
        f"sys.stdout.write({log!r})\n"
        "(state / 'alive').unlink()\n"
    )
    script.chmod(0o755)
    return script


class Parsing(unittest.TestCase):
    def test_companion_lines_attach_to_the_preceding_frame(self):
        text = "\n".join(
            [
                CONFIG_LINE,
                APP + "physics: proxies=1 dynamic=1 sleeping=0 pairs=0 contacts=0",
                APP + "backend: Vulkan adapter: AMD Radeon 660M (RADV REMBRANDT) present_mode: immediate "
                "max_frame_latency: 1 timestamp_query: true",
                frame_line(10.0),
                APP + "systems: physics_step=4.00ms",
                APP + "gpu_passes:",
                APP + "physics: proxies=10 dynamic=8 sleeping=0 pairs=5 contacts=3",
                BENCH + "bench: balls=8 teleports=2",
                frame_line(12.0),
                APP + "systems: physics_step=5.00ms",
                "unrelated output",
                frame_line(11.0),
                BENCH + "bench: balls=8 teleports=1",
            ]
        )
        log = bench_report.parse_log(text)
        self.assertEqual(len(log.frames), 3)
        self.assertEqual(log.orphans, 1)
        first, second, third = log.frames
        self.assertEqual(first.stages["total"], 10.0)
        self.assertIsNone(first.stages["gpu"])
        self.assertEqual(first.systems, {"physics_step": 4.0})
        self.assertEqual(first.gpu_passes, {})
        self.assertEqual(first.physics["pairs"], 5.0)
        self.assertEqual(first.bench["teleports"], 2.0)
        self.assertEqual(second.systems, {"physics_step": 5.0})
        self.assertIsNone(second.physics)
        self.assertIsNone(second.bench)
        self.assertIsNone(third.systems)
        self.assertEqual(third.bench["teleports"], 1.0)
        self.assertEqual(log.backend["adapter"], "AMD Radeon 660M (RADV REMBRANDT)")
        self.assertEqual(log.backend["max_frame_latency"], 1)
        self.assertTrue(log.backend["timestamp_query"])
        self.assertEqual(log.bench_config, CONFIG_LINE.split("bench-config: ", 1)[1])
        stats = bench_report.run_stats(log.frames)
        self.assertEqual(stats["systems"]["physics_step"]["n"], 2)
        self.assertEqual(stats["counters"]["teleports"]["n"], 2)
        self.assertAlmostEqual(stats["stages"]["unattributed"]["max"], 12.0 - 9.11)

    def test_system_names_are_literal_keys(self):
        text = "\n".join(
            [
                frame_line(10.0),
                APP + "systems: physics_step=1.00ms physics_step_extra=2.00ms a.b(c)+*=3.00ms [x]|y=4.00ms",
            ]
        )
        stats = bench_report.run_stats(bench_report.parse_log(text).frames)
        systems = stats["systems"]
        self.assertEqual(list(systems), ["physics_step", "physics_step_extra", "a.b(c)+*", "[x]|y"])
        self.assertEqual(systems["physics_step"]["mean"], 1.0)
        self.assertEqual(bench_report.metric_value(stats, "system.a.b(c)+*", "p50"), 3.0)
        self.assertIsNone(bench_report.metric_value(stats, "system.physics", "p50"))

    def test_interval_is_reported_and_is_no_part_of_the_frames_work(self):
        totals = (10.0, 12.0, 11.0)
        text = "\n".join(frame_line(total, interval=interval) for total, interval in zip(totals, ("n/a", "10.40ms", "12.30ms")))
        frames = bench_report.parse_log(text).frames
        self.assertIsNone(frames[0].stages["interval"])
        stats = bench_report.run_stats(frames)
        self.assertEqual(list(stats["stages"])[:2], ["total", "interval"])
        self.assertEqual(bench_report.STAGE_ORDER[:2], ("total", "interval"))
        # The first frame has no previous frame start.
        self.assertEqual((stats["stages"]["interval"]["n"], stats["stages"]["interval"]["max"]), (2, 12.3))
        # `unattributed`, the stacked bars and the limiting stage read the
        # frames as they read a log without the field.
        plain = bench_report.run_stats(bench_report.parse_log("\n".join(frame_line(total) for total in totals)).frames)
        self.assertNotIn("interval", plain["stages"])
        self.assertEqual(stats["stages"]["unattributed"], plain["stages"]["unattributed"])
        self.assertEqual(bench_report.limiting_candidates(stats), bench_report.limiting_candidates(plain))

        def stack(run):
            return bench_report.stack_segments({name: summary["mean"] for name, summary in run["stages"].items()})

        self.assertEqual(stack(stats), stack(plain))

    def test_spikes_are_frames_above_one_and_a_half_times_the_runs_p50(self):
        totals = [10.0, 10.0, 15.0, 15.01, 10.0, 22.0]
        frames = bench_report.parse_log("\n".join(frame_line(total) for total in totals)).frames
        # p50 is 10 ms (nearest rank), so the limit is 15 ms; 15.00 ms is not above it.
        self.assertEqual(bench_report.spike_count(frames), 2)
        self.assertIsNone(bench_report.spike_count([]))
        with tempfile.TemporaryDirectory() as temp:
            capture = write_capture(Path(temp) / "capture", [[10.0, 10.0, 10.0, 16.0], [10.0, 10.0, 10.0, 12.0]], BASE_PEAKS[:2])
        self.assertEqual([run["spikes"] for run in capture["runs"]], [1, 0])
        readme = bench_report.capture_readme(capture)
        self.assertIn("## Frame time per run (timing runs)", readme)
        self.assertIn("| Run | `total` p50 (ms) | `total` max (ms) | Spikes (`total` > 1.5 × p50) |", readme)
        self.assertIn("| 1 | 10 | 16 | 1 |", readme)
        self.assertIn("| 2 | 10 | 12 | 0 |", readme)

    def test_nearest_rank_percentiles(self):
        values = [float(value) for value in range(1, 301)]
        self.assertEqual(bench_report.percentile(values, 50), 150.0)
        self.assertEqual(bench_report.percentile(values, 95), 285.0)
        self.assertEqual(bench_report.percentile(values, 99), 297.0)
        self.assertEqual(bench_report.percentile(values, 100), 300.0)
        self.assertEqual(bench_report.percentile(values, 0), 1.0)
        self.assertEqual(bench_report.percentile([7.0], 99), 7.0)
        self.assertEqual(bench_report.percentile([1.0, 2.0], 50), 1.0)
        self.assertEqual(bench_report.percentile([1.0, 2.0, 3.0, 4.0], 95), 4.0)
        summary = bench_report.summarize([None, 3.0, 1.0, 2.0])
        self.assertEqual(summary, {"n": 3, "mean": 2.0, "min": 1.0, "p50": 2.0, "p95": 3.0, "p99": 3.0, "max": 3.0})
        self.assertIsNone(bench_report.summarize([None]))


class Guards(unittest.TestCase):
    def analyze(self, frames, warmup=2, measured=3, config=CONFIG):
        return bench_report.analyze_log(physics_log(frames), warmup, measured, GUARDS, config)

    def test_guards_check_measured_frames_only(self):
        warmup = [physics_frame(9.0, sleeping=4, teleports=0), physics_frame(9.0, teleports=0)]
        good = warmup + [physics_frame(10.0), physics_frame(11.0), physics_frame(12.0, bench_line=False)]
        result = self.analyze(good)
        self.assertEqual(result["problems"], [])
        self.assertTrue(all(guard["ok"] for guard in result["guards"]))
        self.assertEqual(result["frames_measured"], 3)

        sleeper = warmup + [physics_frame(10.0), physics_frame(11.0, sleeping=1), physics_frame(12.0)]
        problems = self.analyze(sleeper)["problems"]
        self.assertEqual(len(problems), 1)
        self.assertIn("physics.sleeping <= 0", problems[0])
        self.assertIn("measured frame 2", problems[0])

        stalled = warmup + [physics_frame(10.0, teleports=0), physics_frame(11.0), physics_frame(12.0)]
        self.assertIn("bench.teleports >= 1", self.analyze(stalled)["problems"][0])

        gap = warmup + [physics_frame(10.0, bench_line=False), physics_frame(11.0), physics_frame(12.0)]
        self.assertIn("missing", self.analyze(gap)["problems"][0])

        short = self.analyze(good[:4])["problems"]
        self.assertIn("2 measured frames of 3", short[0])
        mismatch = self.analyze(good, config={**CONFIG, "row": "physics-sparse"})["problems"]
        self.assertEqual(mismatch, ["bench-config does not match the request"])

        def counter_frames(*bench_texts):
            lines = []
            for text in bench_texts:
                lines.append(frame_line(10.0))
                if text:
                    lines.append(BENCH + f"bench: {text}")
            return bench_report.parse_log("\n".join(lines)).frames

        counter_guards = [
            {"kind": "counter_max", "counter": "structural", "max": 0},
            {"kind": "counter_const", "counter": "population"},
            {"kind": "counter_eq", "counter": "spawned", "other": "despawned"},
        ]
        steady = counter_frames(
            "population=100 spawned=5 despawned=5 structural=0 digest=0x00ff",
            "population=100 spawned=5 despawned=5 structural=0 digest=0x0f0f",
            None,
        )
        self.assertTrue(all(bench_report.evaluate_guard(guard, steady)["ok"] for guard in counter_guards))
        broken = counter_frames(
            "population=100 spawned=5 despawned=5 structural=0",
            "population=101 spawned=6 despawned=5 structural=2",
            None,
        )
        results = [bench_report.evaluate_guard(guard, broken) for guard in counter_guards]
        self.assertEqual(
            [result["guard"] for result in results],
            ["bench.structural <= 0", "bench.population constant", "bench.spawned == bench.despawned"],
        )
        self.assertFalse(any(result["ok"] for result in results))
        self.assertIn("measured frame 2 (structural=2)", results[0]["detail"])
        self.assertIn("measured frame 2 (population=101, first 100)", results[1]["detail"])
        self.assertIn("measured frame 2 (spawned=6 despawned=5)", results[2]["detail"])
        no_other = counter_frames("population=100 spawned=5", "population=100 spawned=5 despawned=5", None)
        self.assertIn("measured frame 1 (missing)", bench_report.evaluate_guard(counter_guards[2], no_other)["detail"])

        # The band centers on the median of the checked frames, so it follows the knobs.
        band = {"kind": "counter_band", "counter": "live", "tolerance": 0.1}
        steady_live = bench_report.evaluate_guard(band, counter_frames("live=1000", "live=1090", "live=950", "live=1000", None))
        self.assertTrue(steady_live["ok"])
        self.assertEqual(steady_live["guard"], "bench.live within ±10% of its median")
        self.assertIn("median 1000, largest deviation 9.0%", steady_live["detail"])
        drifting = bench_report.evaluate_guard(band, counter_frames("live=1000", "live=1000", "live=1120", "live=990", None))
        self.assertFalse(drifting["ok"])
        self.assertIn("1 of 4 frames fail; first: measured frame 3 (live=1120, median 1000)", drifting["detail"])
        gap = bench_report.evaluate_guard(band, counter_frames("live=1000", None, "live=1000", None))
        self.assertIn("measured frame 2 (missing)", gap["detail"])

    def test_failing_guard_invalidates_the_capture(self):
        good = [physics_frame(10.0), physics_frame(11.0), physics_frame(12.0)]
        bad = [physics_frame(10.0), physics_frame(11.0, sleeping=2), physics_frame(12.0)]
        runs = []
        for index, frames in enumerate((good, bad), start=1):
            usage = {"peak_rss_kib": 65536, "user_s": 1.0, "sys_s": 0.1, "minflt": 1, "majflt": 0, "nvcsw": 2, "nivcsw": 3}
            run = {"index": index, "exit_code": 0, "rusage": usage, "rss_growth_kib_s": 0.0}
            run.update(bench_report.analyze_log(physics_log(frames), 0, 3, GUARDS, CONFIG))
            runs.append(run)
        row = {"name": "physics", "preset": "default", "owned": [{"metric": "system.physics_step", "stats": ["p50", "p95"]}], "guards": GUARDS}
        request = {"set": [], "repeat": 2, "gpu_timing": False, "present_mode": None, "max_frame_latency": None}
        provenance = dict.fromkeys(("commit", "dirty", "rustc", "cpu", "kernel", "governor", "platform_profile", "ac_power", "machine"), "x")
        capture = bench_report.assemble_capture(
            request=request,
            bench={"name": "physics", "workload_version": 1},
            row=row,
            config={**CONFIG, "preset": "default", "scale": 1.0},
            warmup=0,
            frames=3,
            build={"rustflags": bench.DEFAULT_RUSTFLAGS},
            provenance=provenance,
            runs=runs,
            profile=None,
        )
        self.assertFalse(capture["valid"])
        self.assertEqual([guard["ok"] for guard in capture["guards"]], [False, True])
        self.assertTrue(any(reason.startswith("run 2: guard physics.sleeping") for reason in capture["invalid_reasons"]))
        self.assertIn("determinism: digests differ", capture["invalid_reasons"][-1])
        readme = bench_report.capture_readme(capture)
        self.assertIn("FAIL: `physics.sleeping <= 0`", readme)
        self.assertIn("| `system.physics_step` p95 |", readme)

    def test_unconfirmed_present_override_invalidates_the_capture(self):
        frames = [physics_frame(10.0), physics_frame(11.0), physics_frame(12.0)]

        def problems(backend_line, **present):
            log = physics_log(frames)
            if backend_line:
                log = APP + f"backend: Vulkan adapter: Test GPU {backend_line} timestamp_query: true\n" + log
            return bench_report.analyze_log(log, 0, 3, GUARDS, CONFIG, **present)["problems"]

        immediate = "present_mode: immediate max_frame_latency: 1"
        self.assertEqual(problems(immediate), [])
        self.assertEqual(problems(None), [])
        self.assertEqual(problems("present_mode: fifo max_frame_latency: 2", present_mode="fifo", max_frame_latency=2), [])
        self.assertEqual(problems(immediate, present_mode="auto"), [])
        self.assertEqual(problems("present_mode: fifo max_frame_latency: 2", present_mode="auto_vsync"), [])
        self.assertEqual(
            problems(immediate, present_mode="fifo", max_frame_latency=2),
            ["present mode is immediate, requested fifo", "max frame latency is 1, requested 2"],
        )
        self.assertEqual(problems(immediate, present_mode="auto_vsync"), ["present mode is immediate, requested auto_vsync"])
        self.assertEqual(
            problems(None, max_frame_latency=2), ["no backend line to confirm the requested present mode and frame latency"]
        )

        usage = {"peak_rss_kib": 65536, "user_s": 1.0, "sys_s": 0.1, "minflt": 1, "majflt": 0, "nvcsw": 2, "nivcsw": 3}
        log = APP + f"backend: Vulkan adapter: Test GPU {immediate} timestamp_query: true\n" + physics_log(frames)
        run = {"index": 1, "exit_code": 0, "rusage": usage, "rss_growth_kib_s": 0.0}
        run.update(bench_report.analyze_log(log, 0, 3, GUARDS, CONFIG, present_mode="fifo", max_frame_latency=1))
        self.assertEqual(bench_report.hard_problems(run), ["present mode is immediate, requested fifo"])
        capture = bench_report.assemble_capture(
            request={"set": [], "repeat": 1, "gpu_timing": False, "present_mode": "fifo", "max_frame_latency": 1},
            bench={"name": "physics", "workload_version": 1},
            row=ROW,
            config={**CONFIG, "preset": "default", "scale": 1.0},
            warmup=0,
            frames=3,
            build={"rustflags": bench.DEFAULT_RUSTFLAGS},
            provenance=dict.fromkeys(PROVENANCE_KEYS, "x"),
            runs=[run],
            profile=None,
        )
        self.assertFalse(capture["valid"])
        self.assertEqual(capture["invalid_reasons"], ["run 1: present mode is immediate, requested fifo"])
        readme = bench_report.capture_readme(capture)
        self.assertIn("| Valid | no |", readme)
        self.assertIn("| Present mode / latency (requested) | immediate / 1 (fifo / 1) |", readme)
        self.assertIn("- run 1: present mode is immediate, requested fifo", readme)


class Runner(unittest.TestCase):
    def test_child_environment_hygiene(self):
        parent = {
            "HOME": "/home/user",
            "WGPU_BACKEND": "vulkan",
            "TUNGSTEN_PERF_RUSTFLAGS": "-C force-frame-pointers=yes",
            "TUNGSTEN_BENCH": "gpu",
            "TUNGSTEN_BENCH_SET": "balls=1",
            "TUNGSTEN_BENCH_DESCRIBE": "1",
            "TUNGSTEN_GPU_TIMING": "1",
            "TUNGSTEN_RENDER_MSAA": "4",
            "TUNGSTEN_RENDER_PRESENT_MODE": "fifo",
            "TUNGSTEN_DISPLAY_RESOLUTION": "640x480",
            "TUNGSTEN_CAPTURE_FRAME": "5",
            "TUNGSTEN_OVERLAYS_ON": "physics",
            "TUNGSTEN_SMOKE_FRAMES": "3",
            "TUNGSTEN_PERF_LOG": "1",
            "RUST_LOG": "trace",
        }
        snapshot = dict(parent)
        request = bench.bench_vars("physics", "sparse", "2", ["cell=64", "speed_max=2000"])
        kept = {name: parent[name] for name in ("HOME", "WGPU_BACKEND", "TUNGSTEN_PERF_RUSTFLAGS")}
        self.assertEqual(
            bench.capture_env(parent, request, 420),
            {
                **kept,
                "TUNGSTEN_BENCH": "physics",
                "TUNGSTEN_BENCH_PRESET": "sparse",
                "TUNGSTEN_BENCH_SCALE": "2",
                "TUNGSTEN_BENCH_SET": "cell=64,speed_max=2000",
                "TUNGSTEN_SMOKE_FRAMES": "420",
                "TUNGSTEN_PERF_LOG": "1",
                "RUST_LOG": "tungsten::app=debug,bench=debug",
            },
        )
        gpu = bench.capture_env(parent, bench.bench_vars("physics"), 420, gpu=True, present_mode="mailbox", max_frame_latency=2)
        self.assertEqual(gpu["TUNGSTEN_GPU_TIMING"], "1")
        self.assertEqual(gpu["TUNGSTEN_DISPLAY_PRESENT_MODE"], "mailbox")
        self.assertEqual(gpu["TUNGSTEN_DISPLAY_MAX_FRAME_LATENCY"], "2")
        self.assertNotIn("TUNGSTEN_RENDER_PRESENT_MODE", gpu)
        self.assertNotIn("TUNGSTEN_RENDER_MSAA", gpu)
        self.assertNotIn("TUNGSTEN_BENCH_SET", gpu)
        profile = bench.capture_env(parent, bench.bench_vars("physics"), 420, profile=True)
        self.assertEqual(profile["RUST_LOG"], "error")
        self.assertNotIn("TUNGSTEN_PERF_LOG", profile)
        self.assertEqual(bench.child_env(parent, {"TUNGSTEN_BENCH_DESCRIBE": "1"})["TUNGSTEN_BENCH_DESCRIBE"], "1")
        self.assertEqual(parent, snapshot)

    def test_provenance_helpers(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            (root / "empty").write_text("")
            (root / "governor").write_text("performance\n")
            self.assertEqual(bench.file_value_or_na(root / "missing"), "n/a")
            self.assertEqual(bench.file_value_or_na(root / "empty"), "n/a")
            self.assertEqual(bench.file_value_or_na(root / "governor"), "performance")
            supplies = root / "power_supply"
            (supplies / "BAT0").mkdir(parents=True)
            (supplies / "BAT0" / "type").write_text("Battery\n")
            self.assertEqual(bench.ac_power(supplies), "n/a")
            (supplies / "ACAD").mkdir()
            (supplies / "ACAD" / "type").write_text("Mains\n")
            (supplies / "ACAD" / "online").write_text("0\n")
            self.assertEqual(bench.ac_power(supplies), "offline")
            (supplies / "ACAD" / "online").write_text("1\n")
            self.assertEqual(bench.ac_power(supplies), "online")
            self.assertEqual(bench.ac_power(root / "missing"), "n/a")
        cpuinfo = "processor\t: 0\nvendor_id\t: AuthenticAMD\nmodel name\t: AMD Ryzen 7 6800H with Radeon Graphics\n"
        self.assertEqual(bench.cpu_model(cpuinfo), "AMD Ryzen 7 6800H with Radeon Graphics")
        self.assertEqual(bench.cpu_model(""), "unknown")
        self.assertEqual(bench.mem_total_kib("MemTotal:       31998768 kB\nMemFree:  1 kB\n"), 31998768)
        self.assertIsNone(bench.mem_total_kib(""))
        self.assertTrue(bench.frame_pointers_enabled("-C force-frame-pointers=yes"))
        self.assertTrue(bench.frame_pointers_enabled("-Copt-level=3 -Cforce-frame-pointers=yes"))
        self.assertFalse(bench.frame_pointers_enabled("-C force-frame-pointers=yes -C force-frame-pointers=no"))
        self.assertFalse(bench.frame_pointers_enabled("-C target-cpu=native"))
        fingerprint = bench.machine_fingerprint("host", "cpu", 1024)
        self.assertRegex(fingerprint, r"^[0-9a-f]{12}$")
        self.assertNotEqual(fingerprint, bench.machine_fingerprint("other", "cpu", 1024))
        stamp = "20261002T101500Z"
        self.assertEqual(bench.capture_dir_name(stamp, "physics", None, "default", None, [], None, None), f"{stamp}-physics")
        self.assertEqual(
            bench.capture_dir_name(stamp, "physics-sparse", "sparse", "sparse", "1.0", [], None, None),
            f"{stamp}-physics-sparse",
        )
        set_hash = hashlib.sha256(b"cell=64").hexdigest()[:6]
        self.assertEqual(
            bench.capture_dir_name(stamp, "physics", "min", "default", "2", ["cell=64"], "immediate", 1),
            f"{stamp}-physics-min-s2-set{set_hash}-immediate-lat1",
        )

    @unittest.skipUnless(shutil.which("git") and shutil.which("sha256sum"), "needs git and sha256sum")
    def test_dirty_fingerprint_matches_the_shell_algorithm(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)

            def git(*args):
                subprocess.run(["git", "-C", temp, "-c", "user.name=t", "-c", "user.email=t@t", *args], check=True, capture_output=True)

            git("init", "-q")
            (root / "tracked.txt").write_text("one\n")
            git("add", "tracked.txt")
            git("commit", "-q", "-m", "init")
            self.assertEqual(bench.git_dirty(root), "no")
            self.assertRegex(bench.git_commit(root), r"^[0-9a-f]{4,}$")
            (root / "tracked.txt").write_text("two\n")
            (root / "new file.bin").write_bytes(b"\x00\x01")
            shell = (
                '{ git -C "$ROOT" diff HEAD --binary; git -C "$ROOT" ls-files -z --others --exclude-standard '
                '| (cd "$ROOT" && xargs -0 -r sha256sum); } 2>/dev/null | sha256sum | cut -c1-12'
            )
            expected = subprocess.run(
                ["bash", "-c", shell], env={**os.environ, "ROOT": temp}, capture_output=True, text=True, check=True
            ).stdout.strip()
            self.assertEqual(bench.git_dirty(root), f"yes (diff {expected})")
        self.assertEqual(bench.git_dirty(Path(temp) / "gone"), "unknown")

    def test_rss_growth_ignores_teardown_samples(self):
        # 2 KiB/s at 10 Hz, then one sample taken while the child shuts down.
        samples = [(round(0.1 * index, 3), 100_000 + 0.2 * index) for index in range(60)]
        samples.append((6.0, 87_000))
        self.assertAlmostEqual(bench_report.rss_growth(samples), 2.0, places=6)

    def test_wait4_reaps_peak_rss_and_exit_code(self):
        code = "import sys, time\nblock = b'x' * (64 << 20)\ntime.sleep(0.5)\nsys.exit(7)\n"
        with tempfile.TemporaryDirectory() as temp:
            rss_path = Path(temp) / "rss.tsv"
            result = bench.run_child([sys.executable, "-c", code], dict(os.environ), temp, Path(temp) / "child.log", rss_path)
            rows = rss_path.read_text().splitlines()
        self.assertEqual(result["exit_code"], 7)
        self.assertGreaterEqual(result["rusage"]["peak_rss_kib"], 64 * 1024)
        self.assertEqual(rows[0], "seconds\trss_kib")
        self.assertEqual(len(rows) - 1, len(result["rss_samples"]))
        self.assertGreaterEqual(max(rss for _, rss in result["rss_samples"]), 64 * 1024)


class BackgroundLoad(unittest.TestCase):
    """`D-095`: background load during a measured run invalidates it unless allowed."""

    ENCODER = [(4242, 1, "nxcodec.bin")]

    def capture(self, directory, *, lister, trees=(BUILT_TREE, BUILT_TREE), allow=False, sleep=0.0):
        """Two timing runs of the fake binary through `capture_to`, with an
        injected process lister and tree-state reader (one state per run)."""
        out = directory / "capture"
        out.mkdir()
        states = iter(trees)
        request = {"bench": "physics", "preset": None, "scale": None, "set": [], "frames": 3, "warmup": 0, "repeat": 2, "gpu_timing": False}
        request.update(present_mode=None, max_frame_latency=None, profile=False, allow_background=allow)
        with mock.patch.object(bench, "BACKGROUND_SCAN_SECONDS", 0.05), contextlib.redirect_stdout(io.StringIO()):
            capture = bench.capture_to(
                out,
                binary=fake_bench(directory, sleep),
                root=directory,
                bench={"name": "physics", "workload_version": 1, "warmup": 0, "rows": [ROW]},
                config=FAKE_CONFIG,
                request_env={},
                request=request,
                provenance={**dict.fromkeys(PROVENANCE_KEYS, "x"), **BUILT_TREE},
                rustflags=bench.DEFAULT_RUSTFLAGS,
                lister=lister,
                tree_reader=lambda root: next(states),
            )
        return capture, (out / "README.md").read_text()

    def encoder_in_run_2(self, directory):
        """A process lister that shows the encoder only while run 2's child runs."""

        def lister():
            try:
                running = (directory / "alive").read_text()
            except OSError:
                return []
            return self.ENCODER if running == "2" else []

        return lister

    def test_offenders_skip_the_runners_own_tree(self):
        processes = [
            (1, 0, "systemd"),
            (100, 1, "python3"),  # the runner
            (101, 100, "cargo"),  # its build
            (102, 101, "rustc"),
            (200, 1, "claude"),  # an idle agent session
            (300, 1, "rust-analyzer"),
        ]
        self.assertEqual(bench.background_offenders(processes, 100), [])
        others = [(201, 200, "cargo"), (202, 201, "rustc"), (400, 1, "nxcodec.bin")]
        self.assertEqual(bench.background_offenders(processes + others, 100), ["cargo", "nxcodec.bin", "rustc"])
        # The encoder counts inside the tree too, and a parent loop ends the walk.
        self.assertEqual(bench.background_offenders([(5, 100, "nxcodec.bin"), (7, 8, "cargo"), (8, 7, "sh")], 100), ["cargo", "nxcodec.bin"])

    def test_process_list_parses_names_and_parents(self):
        with tempfile.TemporaryDirectory() as temp:
            proc = Path(temp)
            for pid, stat in (("42", "42 (nxcodec.bin) S 1 42 42 0 -1"), ("43", "43 (a (b) c) R 42 43 43 0 -1")):
                (proc / pid).mkdir()
                (proc / pid / "stat").write_text(stat + "\n")
            (proc / "44").mkdir()  # exited before its stat was read
            (proc / "self").mkdir()
            self.assertEqual(sorted(bench.list_processes(proc)), [(42, 1, "nxcodec.bin"), (43, 42, "a (b) c")])
        self.assertEqual(bench.list_processes(Path(temp) / "gone"), [])
        if Path("/proc/self/stat").is_file():
            parents = {pid: ppid for pid, ppid, _ in bench.list_processes()}
            self.assertEqual(parents[os.getpid()], os.getppid())

    def test_run_child_scans_before_during_and_after_the_child(self):
        with tempfile.TemporaryDirectory() as temp:
            started = Path(temp) / "started"
            code = f"import pathlib, time\npathlib.Path({str(started)!r}).write_text('1')\ntime.sleep(0.4)\n"
            seen = []
            with mock.patch.object(bench, "BACKGROUND_SCAN_SECONDS", 0.1):
                bench.run_child(
                    [sys.executable, "-c", code], dict(os.environ), temp, Path(temp) / "child.log", watch=lambda: seen.append(started.exists())
                )
        self.assertFalse(seen[0], "the first scan runs before the child starts")
        self.assertTrue(seen[-1], "the last scan runs after it exits")
        self.assertGreaterEqual(seen.count(True), 3, seen)

    def test_clean_runs_stay_valid(self):
        # The runner's own build and an idle agent session are not load.
        own = [(os.getpid(), 1, "python3"), (900_001, os.getpid(), "cargo"), (900_002, 900_001, "rustc"), (900_003, 1, "claude")]
        with tempfile.TemporaryDirectory() as temp:
            capture, readme = self.capture(Path(temp), lister=lambda: own, sleep=0.1)
        self.assertTrue(capture["valid"], capture["invalid_reasons"])
        self.assertEqual(capture["notes"], [])
        self.assertEqual(capture["provenance"]["background"], {"allowed": False, "offenders": [], "tree_changes": []})
        self.assertEqual([(run["background"], run["tree"]) for run in capture["runs"]], [([], BUILT_TREE)] * 2)
        self.assertIn("| Background load | none |", readme)

    def test_encoder_during_a_run_invalidates_it(self):
        with tempfile.TemporaryDirectory() as temp:
            capture, readme = self.capture(Path(temp), lister=self.encoder_in_run_2(Path(temp)), sleep=0.3)
        self.assertFalse(capture["valid"])
        self.assertEqual(capture["invalid_reasons"], ["run 2: background load: nxcodec.bin"])
        self.assertEqual([run["background"] for run in capture["runs"]], [[], ["nxcodec.bin"]])
        self.assertEqual(capture["provenance"]["background"]["offenders"], ["nxcodec.bin"])
        self.assertIn("| Background load | nxcodec.bin |", readme)
        self.assertIn("- run 2: background load: nxcodec.bin", readme)

    def test_tree_changed_after_a_run_invalidates_it(self):
        edited = {**BUILT_TREE, "dirty": "yes (diff 0123456789ab)"}
        with tempfile.TemporaryDirectory() as temp:
            capture, readme = self.capture(Path(temp), lister=list, trees=(BUILT_TREE, edited))
        self.assertFalse(capture["valid"])
        self.assertEqual(capture["invalid_reasons"], ["run 2: background load: tree changed"])
        self.assertEqual(capture["provenance"]["background"]["tree_changes"], [{"run": "run 2", **edited}])
        self.assertIn("| Background load | tree changed |", readme)

    def test_allow_background_keeps_the_capture_valid_with_notes(self):
        edited = {**BUILT_TREE, "commit": "def5678"}
        with tempfile.TemporaryDirectory() as temp:
            directory = Path(temp)
            capture, readme = self.capture(directory, lister=self.encoder_in_run_2(directory), trees=(BUILT_TREE, edited), allow=True, sleep=0.3)
        self.assertTrue(capture["valid"], capture["invalid_reasons"])
        self.assertEqual(capture["notes"], ["run 2: background load: nxcodec.bin", "run 2: background load: tree changed"])
        self.assertTrue(capture["provenance"]["background"]["allowed"])
        self.assertIn("| Background load | nxcodec.bin, tree changed (allowed) |", readme)
        self.assertIn("- Allowed by `--allow-background`: run 2: background load: tree changed", readme)
        # Compare keeps its verdicts and notes the allowed load as a soft difference.
        clean = copy.deepcopy(capture)
        clean["provenance"]["background"] = {"allowed": False, "offenders": [], "tree_changes": []}
        self.assertEqual(bench_report.comparability(clean, capture), ([], ["candidate background load: nxcodec.bin, tree changed (allowed)"]))
        self.assertTrue(bench.parser().parse_args(["run", "physics", "--allow-background"]).allow_background)
        self.assertTrue(bench.parser().parse_args(["suite", "--allow-background"]).allow_background)
        self.assertFalse(bench.parser().parse_args(["suite"]).allow_background)


class Verdicts(unittest.TestCase):
    def test_labels_and_too_few_runs(self):
        judge = bench_report.judge
        base = [10.0, 10.1, 9.9]
        self.assertEqual(judge(base, [11.0, 11.1, 10.9], 0.05, 0.05)["verdict"], "regressed")
        self.assertEqual(judge(base, [9.0, 9.1, 8.9], 0.05, 0.05)["verdict"], "improved")
        # Spread below 0.5% of the mean is floored, so the interval stays finite and inside ±τ.
        steady = judge([10.0, 10.02, 9.98], [10.01, 10.03, 9.99], 0.03, 0.05)
        self.assertEqual(steady["verdict"], "unchanged")
        self.assertLess(steady["interval"][0], 0.0)
        self.assertEqual(judge([10.0, 12.0, 8.0], [10.5, 12.5, 8.5], 0.03, 0.05)["verdict"], "noisy")
        few = judge([10.0, 10.0], [11.0, 11.0, 11.0], 0.05, 0.05)
        self.assertIsNone(few["verdict"])
        self.assertAlmostEqual(few["delta"], 1.0)
        self.assertAlmostEqual(few["delta_pct"], 10.0)
        self.assertIsNone(few["interval"])

    def test_absolute_floors_and_rss_thresholds(self):
        judge = bench_report.judge
        system = bench_report.TAU_ABS_MS["systems"]
        stage = bench_report.TAU_ABS_MS["stages"]
        # 0.1 ms system: τ_rel gives 0.005 ms, so the 0.02 ms floor decides.
        small = judge([0.1] * 3, [0.115] * 3, bench_report.TAU_REL["p95"], system)
        self.assertAlmostEqual(small["tau"], 0.02)
        self.assertEqual(small["verdict"], "unchanged")
        self.assertEqual(judge([0.1] * 3, [0.13] * 3, bench_report.TAU_REL["p95"], system)["verdict"], "regressed")
        self.assertEqual(judge([0.5] * 3, [0.54] * 3, bench_report.TAU_REL["p50"], stage)["verdict"], "unchanged")
        self.assertEqual(judge([0.5] * 3, [0.56] * 3, bench_report.TAU_REL["p50"], stage)["verdict"], "regressed")
        rss = (bench_report.RSS_TAU_REL, bench_report.RSS_TAU_ABS_KIB)
        # 2% of 100,000 KiB is 2,000 KiB, under the 2 MiB floor.
        small_rss = judge(BASE_PEAKS, [peak + 500 for peak in BASE_PEAKS], *rss)
        self.assertEqual(small_rss["tau"], 2_048)
        self.assertEqual(small_rss["verdict"], "unchanged")
        self.assertEqual(judge(BASE_PEAKS, [peak + 3_000 for peak in BASE_PEAKS], *rss)["verdict"], "regressed")
        # At 400,000 KiB the 2% share (8,000 KiB) is the threshold.
        large = [400_000, 400_200, 399_800]
        self.assertEqual(judge(large, [peak + 2_000 for peak in large], *rss)["tau"], 8_000)
        self.assertEqual(judge(large, [peak + 2_000 for peak in large], *rss)["verdict"], "unchanged")
        self.assertEqual(judge(large, [peak + 14_000 for peak in large], *rss)["verdict"], "regressed")


class Compare(unittest.TestCase):
    def test_comparability_failures_suppress_verdicts(self):
        with tempfile.TemporaryDirectory() as temp:
            base = write_capture(Path(temp) / "base", BASE_TOTALS, BASE_PEAKS)
        self.assertEqual(bench_report.comparability(base, copy.deepcopy(base)), ([], []))
        changes = {
            "workload_version": lambda capture: capture.update(workload_version=2),
            "knob balls": lambda capture: capture["config"]["knobs"].update(balls=9000),
            "build flags": lambda capture: capture["build"].update(rustflags="-C target-cpu=native"),
            "machine": lambda capture: capture["provenance"].update(machine="other"),
        }
        for field, change in changes.items():
            cand = copy.deepcopy(base)
            change(cand)
            report = bench_report.compare(base, cand, base_side=("a", "a"), cand_side=("b", "b"))
            self.assertFalse(report["comparable"], field)
            self.assertEqual(len(report["hard"]), 1, field)
            self.assertTrue(report["hard"][0].startswith(field), report["hard"])
            self.assertEqual(report["blocked"], "not comparable")
            self.assertTrue(all(result["verdict"] is None for result in report["owned"] + [report["memory"]["peak_rss"]]))
            self.assertEqual(report["owned_summary"]["none"], len(report["owned"]))
        # A soft difference is a note, not a mismatch.
        cand = copy.deepcopy(base)
        cand["provenance"]["kernel"] = "Linux 8"
        self.assertEqual(bench_report.comparability(base, cand), ([], ["kernel: x vs Linux 8"]))

    def test_one_sided_metrics_keep_the_present_mean_and_name_the_missing_side(self):
        with tempfile.TemporaryDirectory() as temp:
            base = write_capture(Path(temp) / "base", BASE_TOTALS, BASE_PEAKS)
        cand = copy.deepcopy(base)
        for run in cand["runs"]:
            del run["stats"]["systems"]["physics_step"]
            del run["rusage"]
        # Per-run physics_step p50s are 5.5, 5.55 and 5.45 ms.
        base_only = bench_report.judge_metric(base, cand, "system.physics_step", "p50", None)
        self.assertAlmostEqual(base_only["base"], 5.5)
        self.assertEqual((base_only["cand"], base_only["delta"], base_only["verdict"]), (None, None, None))
        self.assertEqual(base_only["note"], "missing in the candidate")
        self.assertEqual(bench_report.judge_metric(cand, base, "system.physics_step", "p50", None)["note"], "missing in the baseline")
        self.assertEqual(bench_report.judge_metric(base, cand, "system.absent", "p50", None)["note"], "missing on both sides")
        report = bench_report.compare(base, cand, base_side=("a", "a"), cand_side=("b", "b"))
        self.assertEqual(report["memory"]["peak_rss"]["note"], "missing in the candidate")
        markdown = bench_report.compare_markdown(report)
        self.assertIn("| `system.physics_step` | p50 | 5.50 | n/a | n/a | n/a | n/a | n/a | — (missing in the candidate) |", markdown)
        self.assertIn("| Peak RSS (MiB, mean of runs) | 97.7 | n/a |", markdown)

    def test_two_captures_write_both_reports_with_peak_rss(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            write_capture(root / "base", BASE_TOTALS, BASE_PEAKS)
            write_capture(root / "cand", shifted(BASE_TOTALS, 1.1), [peak + 4_000 for peak in BASE_PEAKS])
            with contextlib.redirect_stdout(io.StringIO()):
                report = bench.write_compare(root / "base", root / "cand", root=root, out=root / "out")
            markdown = (root / "out" / "compare.md").read_text()
            page = (root / "out" / "compare.html").read_text()
            saved = json.loads((root / "out" / "compare.json").read_text())
        self.assertTrue(report["comparable"])
        self.assertEqual([result["verdict"] for result in report["owned"]], ["regressed", "regressed"])
        self.assertEqual(report["memory"]["peak_rss"]["verdict"], "regressed")
        self.assertTrue(bench_report.any_regressed(saved))
        for text in (markdown, page):
            self.assertIn("Peak RSS", text)
            self.assertIn("system.physics_step", text)
            self.assertIn("regressed", text)
        self.assertIn("| Peak RSS (MiB, mean of runs) | 97.7 | 101.6 |", markdown)
        # The ECDF pools the re-parsed telemetry frames: 9 per side.
        self.assertIn("baseline p95: 12.10 ms", page)

    def test_compare_reports_whether_first_run_digests_match(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            base = write_capture(root / "base", BASE_TOTALS, BASE_PEAKS)
            write_capture(root / "same", BASE_TOTALS, BASE_PEAKS)
            moved = write_capture(root / "moved", BASE_TOTALS, BASE_PEAKS, teleports=4)
            printed = {}
            for name in ("same", "moved"):
                with contextlib.redirect_stdout(io.StringIO()) as out:
                    bench.write_compare(root / "base", root / name, root=root, out=root / f"out-{name}")
                printed[name] = out.getvalue()
            reports = {name: json.loads((root / f"out-{name}" / "compare.json").read_text()) for name in printed}
            markdown = (root / "out-moved" / "compare.md").read_text()
            page = (root / "out-moved" / "compare.html").read_text()
        digest, other = bench_report.first_digest(base), bench_report.first_digest(moved)
        self.assertNotEqual(digest, other)
        self.assertEqual(reports["same"]["digests"], {"baseline": digest, "candidate": digest, "match": True})
        self.assertEqual(reports["moved"]["digests"], {"baseline": digest, "candidate": other, "match": False})
        self.assertIn(f"First-run digests match: {digest}.", printed["same"])
        self.assertIn(f"First-run digests differ: {digest} → {other}.", printed["moved"])
        self.assertIn(f"| First-run digest | `{digest}` | `{other}` |", markdown)
        self.assertIn(f"First-run digests differ: `{digest}` → `{other}`.", markdown)
        self.assertIn(f"First-run digests differ: {digest} → {other}.", page)
        # Information only: equal frame times read unchanged whether or not the digests match.
        for report in reports.values():
            self.assertEqual(report["owned_summary"], {"regressed": 0, "improved": 0, "unchanged": 2, "noisy": 0, "none": 0})

    def test_jitter_is_derived_per_run_and_judged_on_p99s_threshold(self):
        self.assertEqual(bench_report.metric_value({"stages": {"total": {"p50": 10.0, "p99": 13.5}}}, "stage.total", "jitter"), 3.5)
        self.assertIsNone(bench_report.metric_value({"stages": {"total": {"p50": 10.0}}}, "stage.total", "jitter"))
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            base = write_capture(root / "base", BASE_TOTALS, BASE_PEAKS, row=INTEGRATED_ROW)
            cand = write_capture(root / "cand", shifted(BASE_TOTALS, 1.1), BASE_PEAKS, row=INTEGRATED_ROW)
        # Nearest rank over 3 frames: p99 is the largest total and p50 the middle one.
        self.assertEqual([round(value, 6) for value in bench_report.owned_per_run(base, "stage.total", "jitter")], [1.0, 1.0, 0.9])
        self.assertIn("| `stage.total` jitter | 1 | 1, 1, 0.90 | timing runs |", bench_report.capture_readme(base))
        report = bench_report.compare(base, cand, base_side=("a", "a"), cand_side=("b", "b"))
        verdicts = {result["stat"]: result for result in report["owned"]}
        self.assertEqual([verdicts[stat]["verdict"] for stat in ("p50", "p95", "p99")], ["regressed"] * 3)
        # Jitter takes p99's τ: 8% of the baseline's per-run p99 mean (11.97 ms).
        jitter = verdicts["jitter"]
        self.assertAlmostEqual(jitter["delta"], 0.29 / 3)
        self.assertAlmostEqual(jitter["tau"], 0.08 * (12.0 + 12.1 + 11.8) / 3)
        self.assertEqual(jitter["verdict"], "unchanged")
        self.assertEqual(report["owned_summary"], {"regressed": 3, "improved": 0, "unchanged": 1, "noisy": 0, "none": 0})
        self.assertIn("| `stage.total` | jitter | 0.97 | 1.06 | +0.10 | +10.0% |", bench_report.compare_markdown(report))

        # The case jitter exists for: p50 improves and p99 stays within its τ
        # while the tail pulls away from the median.
        tail_base = [[12.0, 12.0, 13.0], [12.1, 12.1, 13.1], [11.9, 11.9, 12.9]]
        tail_cand = [[11.0, 11.0, 13.5], [11.1, 11.1, 13.6], [10.9, 10.9, 13.4]]
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            base = write_capture(root / "base", tail_base, BASE_PEAKS, row=INTEGRATED_ROW)
            cand = write_capture(root / "cand", tail_cand, BASE_PEAKS, row=INTEGRATED_ROW)
        report = bench_report.compare(base, cand, base_side=("a", "a"), cand_side=("b", "b"))
        verdicts = {result["stat"]: result["verdict"] for result in report["owned"]}
        self.assertEqual([verdicts[stat] for stat in ("p50", "p99", "jitter")], ["improved", "unchanged", "regressed"])

    def test_html_has_no_external_references_and_parses(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            base = write_capture(root / "base", BASE_TOTALS, BASE_PEAKS)
            cand = write_capture(root / "cand", shifted(BASE_TOTALS, 0.97), BASE_PEAKS)
            with contextlib.redirect_stdout(io.StringIO()):
                bench.write_compare(root / "base", root / "cand", root=root, out=root / "out")
            pages = {"compare.html": (root / "out" / "compare.html").read_text()}
        sweep = bench_report.sweep_report("fill", [("0.3", "fill-0.3", base), ("0.36", "fill-0.36", cand)])
        pages["sweep.html"] = bench_report.sweep_html(sweep)
        for name, page in pages.items():
            lowered = page.lower()
            for needle in ("<script", "<link", "<img", "<iframe", "<object", "<embed", "@import", "url(", "http:", "https:", "src=", "href=", "//"):
                self.assertNotIn(needle, lowered, f"{name} contains {needle}")
            checker = TagBalance()
            checker.feed(page)
            checker.close()
            self.assertEqual(checker.errors, [], name)
            self.assertEqual(checker.stack, [], name)
            self.assertIn("svg", checker.seen, name)


class Suites(unittest.TestCase):
    SCHEMA = {
        "schema": 1,
        "benches": [
            {
                "name": "physics",
                "presets": [{"name": "min"}, {"name": "default"}, {"name": "sparse-min"}, {"name": "sparse"}],
                "rows": [{"name": "physics", "preset": "default"}, {"name": "physics-sparse", "preset": "sparse"}],
            },
            {
                "name": "gpu",
                "presets": [{"name": "min"}, {"name": "default"}, {"name": "throughput"}, {"name": "visual"}],
                "rows": [{"name": "gpu", "preset": "default"}, {"name": "gpu-throughput", "preset": "throughput"}],
            },
            {"name": "ecs", "presets": [{"name": "min"}, {"name": "default"}], "rows": [{"name": "ecs", "preset": "default"}]},
        ],
    }

    def plan(self, preset=None, only=None):
        return [(bench["name"], row["name"], chosen) for bench, row, chosen in bench.suite_plan(self.SCHEMA, preset, only)]

    def test_row_list_comes_from_describe(self):
        self.assertEqual(
            self.plan(),
            [
                ("physics", "physics", "default"),
                ("physics", "physics-sparse", "sparse"),
                ("gpu", "gpu", "default"),
                ("gpu", "gpu-throughput", "throughput"),
                ("ecs", "ecs", "default"),
            ],
        )
        # --preset replaces only the rows whose own preset is `default`.
        self.assertEqual(
            self.plan("min"),
            [
                ("physics", "physics", "min"),
                ("physics", "physics-sparse", "sparse"),
                ("gpu", "gpu", "min"),
                ("gpu", "gpu-throughput", "throughput"),
                ("ecs", "ecs", "min"),
            ],
        )
        # --only keeps describe's order.
        self.assertEqual(self.plan(only=["ecs", "gpu-throughput"]), [("gpu", "gpu-throughput", "throughput"), ("ecs", "ecs", "default")])
        with self.assertRaisesRegex(bench.BenchError, "unknown rows bogus"):
            self.plan(only=["ecs", "bogus"])
        with self.assertRaisesRegex(bench.BenchError, r"--preset visual: row physics \(physics\) has presets min, default, sparse-min, sparse"):
            self.plan("visual")
        with self.assertRaisesRegex(bench.BenchError, r"--preset visual: row ecs \(ecs\) has presets min, default$"):
            self.plan("visual", ["ecs"])
        self.assertEqual(bench.parse_only("gpu, ecs,,gpu"), ["gpu", "ecs"])
        with self.assertRaises(bench.BenchError):
            bench.parse_only(" , ")
        stamp = "20261002T101500Z"
        self.assertEqual(bench.suite_dir_name(stamp, None, None), f"{stamp}-suite")
        self.assertEqual(bench.suite_dir_name(stamp, "min", "2"), f"{stamp}-suite-min-s2")

    def test_suite_json_and_readme(self):
        with tempfile.TemporaryDirectory() as temp:
            suite = write_suite(Path(temp), BASE_TOTALS, BASE_PEAKS)
        self.assertEqual(suite["kind"], "suite")
        self.assertTrue(suite["valid"])
        physics, integrated = suite["rows"]
        self.assertEqual((physics["row"], physics["dir"], physics["preset"], physics["runs"]), ("physics", "physics", "default", 3))
        self.assertEqual([entry["stat"] for entry in integrated["owned"]], ["p50", "p95", "p99", "jitter"])
        self.assertAlmostEqual(integrated["total"]["p50"], 11.0)
        self.assertAlmostEqual(integrated["total"]["jitter"], 1.0)
        self.assertAlmostEqual(physics["peak_rss_mib"], 100_000 / 1024)
        readme = bench_report.suite_readme(suite)
        self.assertIn("| `integrated` | default | yes | `stage.total` p50 11; `stage.total` p95 12; `stage.total` p99 12; `stage.total` jitter 1 | 11 / 12 / 12 | 1 | 97.7 | `integrated/` |", readme)
        self.assertNotIn("Invalid rows", readme)
        broken = copy.deepcopy(suite)
        broken["rows"][0].update(valid=False, invalid_reasons=["run 2: exited with 101"])
        broken["rows"][1]["notes"] = ["run 1: background load: cargo"]
        broken["valid"] = False
        broken["request"]["preset"] = "min"
        readme = bench_report.suite_readme(broken)
        self.assertIn("| Valid | no |", readme)
        self.assertIn("- `physics`:\n  - run 2: exited with 101", readme)
        self.assertIn("## Background load allowed by `--allow-background`\n\n- `integrated`:\n  - run 1: background load: cargo", readme)
        self.assertIn("`--preset min` replaced the preset of the rows whose own preset is `default`", readme)
        self.assertFalse(bench_report.assemble_suite(request=SUITE_REQUEST, build={}, provenance={}, entries=[])["valid"])

    def test_suite_compare_writes_both_reports_with_peak_rss(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            write_suite(root / "base", BASE_TOTALS, BASE_PEAKS)
            write_suite(root / "cand", shifted(BASE_TOTALS, 1.1), [peak + 4_000 for peak in BASE_PEAKS], rows=("physics", "integrated", "extra"))
            with contextlib.redirect_stdout(io.StringIO()):
                report = bench.write_suite_compare(root / "base", root / "cand", root=root, out=root / "out")
            markdown = (root / "out" / "compare.md").read_text()
            page = (root / "out" / "compare.html").read_text()
            saved = json.loads((root / "out" / "compare.json").read_text())
            row_reports = sorted(str(path.relative_to(root / "out")) for path in (root / "out").glob("*/compare.*"))
            # A saved suite baseline resolves by name on either side.
            with contextlib.redirect_stdout(io.StringIO()):
                bench.cmd_baseline(argparse.Namespace(action="save", capture=str(root / "base"), name="suite-a"), root=root)
            self.assertEqual(bench.resolve_ref("suite-a", root), ((root / "perf-runs" / "baselines" / "suite-a").resolve(), "suite"))
            # `run --compare` takes the suite's capture of its row.
            self.assertEqual(bench.resolve_capture("suite-a", root, "physics"), (root / "perf-runs" / "baselines" / "suite-a").resolve() / "physics")
            with self.assertRaisesRegex(bench.BenchError, "suite 'suite-a' has no row 'gpu'; rows: physics, integrated"):
                bench.resolve_capture("suite-a", root, "gpu")
            meta = json.loads((root / "perf-runs" / "baselines" / "suite-a" / "baseline.json").read_text())
            args = argparse.Namespace(baseline="suite-a", candidate=str(root / "cand"), out=str(root / "out2"), force=False, fail_on="regressed")
            with contextlib.redirect_stdout(io.StringIO()):
                code = bench.cmd_compare(args, root=root)
            with self.assertRaisesRegex(bench.BenchError, "is a suite"):
                bench.resolve_capture(str(root / "base"), root)
            with self.assertRaisesRegex(bench.BenchError, "can't compare a suite"):
                bench.cmd_compare(argparse.Namespace(**{**vars(args), "candidate": str(root / "cand" / "physics")}), root=root)
        self.assertEqual((meta["kind"], meta["rows"], meta["valid"]), ("suite", ["physics", "integrated"], True))
        self.assertEqual(code, bench.EXIT_REGRESSED)
        self.assertEqual(row_reports, sorted(f"{row}/compare.{ext}" for row in ("physics", "integrated") for ext in ("json", "md", "html")))
        self.assertEqual([row["row"] for row in report["rows"]], ["physics", "integrated"])
        self.assertEqual((report["only_baseline"], report["only_candidate"]), ([], ["extra"]))
        self.assertEqual(report["owned_summary"], {"regressed": 5, "improved": 0, "unchanged": 1, "noisy": 0, "none": 0})
        self.assertTrue(bench_report.any_regressed(saved))
        self.assertTrue(all(row["peak_rss"]["verdict"] == "regressed" for row in report["rows"]))
        for text in (markdown, page):
            self.assertIn("Peak RSS", text)
            self.assertIn("integrated", text)
            self.assertIn("5 regressed", text)
            self.assertIn("jitter", text)
        self.assertIn("| `physics` | yes | 2 regressed, 0 improved, 0 unchanged, 0 noisy | 12 → 13.20 | 97.7 → 101.6 | regressed | `physics/compare.md` |", markdown)
        self.assertIn("Only in the candidate, not compared: `extra`.", markdown)
        self.assertIn("First-run digests match in 2 of 2 rows (information only).", markdown)
        lowered = page.lower()
        for needle in ("<script", "<link", "<img", "<iframe", "@import", "url(", "http:", "https:", "src=", "href=", "//"):
            self.assertNotIn(needle, lowered)
        checker = TagBalance()
        checker.feed(page)
        checker.close()
        self.assertEqual((checker.errors, checker.stack), ([], []))
        self.assertIn("svg", checker.seen)

    def test_suite_compare_lists_rows_whose_digests_differ(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            write_suite(root / "base", BASE_TOTALS, BASE_PEAKS)
            write_suite(root / "cand", BASE_TOTALS, BASE_PEAKS)
            shutil.rmtree(root / "cand" / "physics")
            moved = write_capture(root / "cand" / "physics", BASE_TOTALS, BASE_PEAKS, teleports=4)
            with contextlib.redirect_stdout(io.StringIO()) as out:
                report = bench.write_suite_compare(root / "base", root / "cand", root=root, out=root / "out")
            markdown = (root / "out" / "compare.md").read_text()
            page = (root / "out" / "compare.html").read_text()
        self.assertEqual(report["digests_differ"], ["physics"])
        self.assertEqual([row["digests"]["match"] for row in report["rows"]], [False, True])
        self.assertIn("First-run digests match in 1 of 2 rows; they differ in `physics` (information only).", markdown)
        self.assertIn("First-run digests match in 1 of 2 rows; they differ in physics (information only).", out.getvalue())
        self.assertIn(f"first-run digests differ: {report['rows'][0]['digests']['baseline']} → {bench_report.first_digest(moved)}", markdown)
        self.assertIn("they differ in physics", page)
        self.assertEqual(report["owned_summary"]["regressed"] + report["owned_summary"]["improved"], 0)


class TagBalance(html.parser.HTMLParser):
    """Every non-void element closes in order."""

    VOID = {"meta", "br", "hr", "img", "input", "link", "source", "wbr", "col", "area", "base", "embed", "param", "track"}
    SVG_EMPTY = {"line", "circle", "rect", "polyline", "path"}

    def __init__(self):
        super().__init__()
        self.stack, self.errors, self.seen = [], [], set()

    def handle_starttag(self, tag, attrs):
        self.seen.add(tag)
        if tag not in self.VOID:
            self.stack.append(tag)

    def handle_startendtag(self, tag, attrs):
        self.seen.add(tag)
        if tag not in self.VOID | self.SVG_EMPTY:
            self.errors.append(f"self-closed <{tag}/>")

    def handle_endtag(self, tag):
        if not self.stack or self.stack[-1] != tag:
            self.errors.append(f"</{tag}> closes {self.stack[-1] if self.stack else 'nothing'}")
            return
        self.stack.pop()


class Capacity(unittest.TestCase):
    BUDGET = 16.7

    def search(self, model, **bounds):
        calls = []

        def probe(value, phase):
            calls.append((value, phase))
            return {"stat": model(value, len(calls))}

        options = {"start": 1.0, "lower": 0.025, "upper": 50.0, "budget": self.BUDGET, "tolerance": 0.05}
        options.update(bounds)
        return bench.capacity_search(probe, **options), calls

    def test_converges_within_tolerance_on_a_monotone_model(self):
        capacity = self.BUDGET / 10.0
        result, calls = self.search(lambda value, _: 10.0 * value)
        self.assertEqual(result["status"], "max")
        self.assertLessEqual(result["value"], capacity)
        self.assertGreaterEqual(result["value"], capacity / 1.05)
        self.assertEqual([phase for _, phase in calls[-3:]], ["confirm"] * 3)
        self.assertEqual(len(result["evidence"]), 3)
        # Starting above the budget halves down first.
        result, _ = self.search(lambda value, _: 10.0 * value, start=8.0)
        self.assertEqual(result["status"], "max")
        self.assertGreaterEqual(result["value"], capacity / 1.05)

    def test_bound_reached_and_unreachable_budget(self):
        bound, calls = self.search(lambda value, _: 10.0 * value, upper=1.5)
        self.assertEqual(bound["status"], "bound")
        self.assertEqual(bound["value"], 1.5)
        self.assertEqual([value for value, _ in calls[:2]], [1.0, 1.5])
        unreachable, calls = self.search(lambda value, _: 10.0 * value, lower=0.5, budget=3.0)
        self.assertEqual(unreachable["status"], "unreachable")
        self.assertEqual(unreachable["value"], 0.5)
        self.assertEqual([value for value, _ in calls], [1.0, 0.5])
        # A failing child (no statistic) counts as a fail.
        failing, _ = self.search(lambda value, _: None if value > 1.0 else 10.0 * value)
        self.assertEqual(failing["status"], "max")
        self.assertLessEqual(failing["value"], 1.0)

    def test_survives_one_noisy_probe_at_the_boundary(self):
        capacity = self.BUDGET / 10.0
        noisy = []

        def model(value, _):
            # The first probe just above capacity reads 30% fast: a false pass.
            if capacity < value < capacity * 1.1 and not noisy:
                noisy.append(value)
                return 7.0 * value
            return 10.0 * value

        result, calls = self.search(model)
        self.assertEqual(len(noisy), 1)
        self.assertEqual(result["status"], "max")
        self.assertLessEqual(result["value"], capacity)
        self.assertGreaterEqual(result["value"], capacity / 1.05**2)
        confirmed = [record for record in result["probes"] if record["phase"] == "confirm"]
        self.assertGreater(len(confirmed), 3, "the false pass must fail its confirmation and step down")


if __name__ == "__main__":
    unittest.main()
