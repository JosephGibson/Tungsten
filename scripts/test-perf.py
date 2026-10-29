#!/usr/bin/env python3
"""Synthetic tests for scripts/perf.py and scripts/perf_stats.py; no GPU or cargo."""

import contextlib
import io
import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent))
import perf  # noqa: E402
import perf_stats  # noqa: E402

APP = "[2026-10-29T00:00:00Z DEBUG tungsten::app] "
BENCH = "[2026-10-29T00:00:00Z DEBUG bench] "
CONFIG = {"bench": "physics", "preset": "default", "row": "physics", "knobs": {"balls": 8000}}
BACKEND = APP + "backend: Vulkan adapter: AMD Radeon 660M (RADV REMBRANDT) present_mode: immediate max_frame_latency: 1 timestamp_query: true"
GUARDS = [
    {"kind": "physics_max", "field": "sleeping", "max": 0},
    {"kind": "counter_min", "counter": "teleports", "min": 1},
]


def frame_line(total, update=6.0):
    return APP + f"frame: total={total:.2f}ms update={update:.2f}ms flush=0.10ms extract=1.00ms render=2.00ms"


def physics_log(frames, *, tail=0, config=CONFIG):
    """A bench log of `frames` frames plus `tail`: each frame's `frame:`,
    `systems:` and `physics:` lines, and the frame before's `bench:` line
    printed in the next frame, as the bench prints it."""
    lines = [BENCH.replace("DEBUG", "INFO ") + "bench-config: " + json.dumps(config), BACKEND]
    for index in range(frames + tail):
        if index > 0:
            lines.append(BENCH + f"bench: balls=8 teleports={index}")
        lines.append(frame_line(10.0 + index % 3))
        lines.append(APP + f"systems: physics_step={4.0 + index % 2:.2f}ms")
        lines.append(APP + f"physics: proxies=10 dynamic=8 sleeping=0 pairs={index} contacts=3")
    return "\n".join(lines) + "\n"


def row(**changes):
    base = {
        "name": "physics",
        "package": "example-02-bench",
        "folder": ".",
        "launch": {"bench": "physics", "preset": "default"},
        "resolution": [1920, 1080],
        "warmup": 2,
        "frames": 5,
        "repeat": 3,
        "gpu": False,
        "owned": [{"metric": "stage.update", "stats": ["p50"]}, {"metric": "system.physics_step", "stats": ["p50"]}],
        "guards": GUARDS,
        "known_drift": [],
        "budgets": [],
        "workload_version": 1,
    }
    base.update(changes)
    return base


def capture(updates=(10.0, 10.1, 9.9), **changes):
    """A schema-2 capture whose runs' `stage.update` p50s are `updates`."""
    runs = []
    for index, value in enumerate(updates, start=1):
        stats = {"stages": {"update": {"p50": value, "p95": value, "p99": value, "mean": value}}, "systems": {}, "counters": {}}
        runs.append({"index": index, "valid": True, "reasons": [], "digest": "d1", "rss_peak_kib": 100_000 + index, "cpu": stats, "gpu": None})
    data = {
        "schema": 2,
        "kind": "capture",
        "row": "physics",
        "sitting": None,
        "tree": {"commit": "abc1234", "dirty": "no", "ref": None},
        "build": {"flags": perf.ALIGNED_FLAGS, "package": "example-02-bench"},
        "workload": {"version": 1, "config": {"knobs": {"balls": 8000}}, "script_sha256": None, "resolution": [1920, 1080], "warmup": 120, "frames": 300, "tail": 0},
        "renderer": {"backend": "Vulkan", "adapter": "AMD", "present_mode": "immediate", "max_frame_latency": 1},
        "machine": {"fingerprint": "236ad1576f1c"},
        "valid": True,
        "runs": runs,
    }
    for key, value in changes.items():
        data[key] = value
    return data


OWNED = [{"metric": "stage.update", "stats": ["p50"]}]


class Parsing(unittest.TestCase):
    def test_companion_lines_attach_to_the_frame_before_them(self):
        log = perf_stats.parse_log(physics_log(3))
        self.assertEqual(len(log.frames), 3)
        self.assertEqual(log.frames[0].bench, {"balls": 8.0, "teleports": 1.0}, "frame 1's bench prints in frame 2")
        self.assertIsNone(log.frames[2].bench, "the last frame's bench line never printed")
        self.assertEqual(log.frames[1].physics["pairs"], 1.0)
        self.assertEqual(log.backend["present_mode"], "immediate")
        self.assertEqual(json.loads(log.bench_config)["row"], "physics")

    def test_orphans_and_values(self):
        log = perf_stats.parse_log(APP + "systems: a=1.00ms\n" + frame_line(9.0) + "\n" + APP + "counters: draw_calls=12 upload_bytes=n/a")
        self.assertEqual(log.orphans, 1)
        self.assertEqual(log.frames[0].counters, {"draw_calls": 12.0, "upload_bytes": None})

    def test_cpu_stats_group_stages_systems_and_counters(self):
        frames = perf_stats.parse_log(physics_log(4)).frames
        stats = perf_stats.cpu_stats(frames)
        self.assertEqual(set(stats["stages"]["update"]), {"p50", "p95", "p99", "mean"})
        self.assertIn("unattributed", stats["stages"])
        self.assertIn("physics_step", stats["systems"])
        self.assertIn("bench.teleports", stats["counters"])
        self.assertIn("physics.pairs", stats["counters"])

    def test_percentiles_are_nearest_rank(self):
        summary = perf_stats.summarize([float(value) for value in range(1, 101)])
        self.assertEqual((summary["p50"], summary["p95"], summary["p99"]), (50.0, 95.0, 99.0))
        self.assertIsNone(perf_stats.summarize([None]))


def gpu_log(frames, delays, *, tail=0):
    """A GPU run's log: each frame's `frame:` line, then the `gpu_passes:`
    lines of the frames `delays` says it reads back, `{frame: delay}`."""
    lines = [BACKEND]
    due = {}
    for frame, delay in delays.items():
        due.setdefault(frame + delay, []).append(frame)
    for number in range(1, frames + tail + 1):
        lines.append(frame_line(10.0))
        for measured in sorted(due.get(number, [])):
            lines.append(APP + f"gpu_passes: from={measured} scene={measured / 10:.2f}ms render_span=1.00ms")
    return "\n".join(lines) + "\n"


class FromAttribution(unittest.TestCase):
    """`gpu_passes: from=N` lines attach to frame N, whenever they print."""

    def test_a_warm_up_frames_sample_inside_the_window_is_dropped(self):
        # Warm-up 2: frame 2's sample prints in frame 3, the window's first.
        analysis = perf_stats.analyze(gpu_log(6, {2: 1}), 2, 3, [])
        self.assertTrue(all(frame.gpu_passes is None for frame in analysis["measured"]))
        self.assertEqual(analysis["log"].frames[1].gpu_passes["scene"], 0.2, "it stays with frame 2")

    def test_two_samples_read_in_one_frame_attach_to_their_own_frames(self):
        log = perf_stats.parse_log(gpu_log(6, {2: 2, 3: 1}))
        self.assertEqual(log.frames[1].gpu_passes["scene"], 0.2)
        self.assertEqual(log.frames[2].gpu_passes["scene"], 0.3)
        self.assertNotIn("from", log.frames[1].gpu_passes)

    def test_the_windows_last_frames_are_sampled_from_the_tail(self):
        delays = {frame: 3 for frame in range(1, 8)}
        analysis = perf_stats.analyze(gpu_log(7, delays, tail=4), 2, 5, [], gpu=True)
        self.assertEqual(analysis["problems"], [])
        self.assertTrue(all(frame.gpu_passes for frame in analysis["measured"]))
        short = perf_stats.analyze(gpu_log(7, delays), 2, 5, [], gpu=True)
        self.assertIn("GPU timings on 2 of 5 window frames, under 95%", short["problems"])

    def test_a_window_under_95_percent_sampled_is_invalid(self):
        delays = {frame: 1 for frame in range(1, 101) if frame != 50}
        ok = perf_stats.analyze(gpu_log(100, delays, tail=4), 0, 100, [], gpu=True)
        self.assertEqual(ok["problems"], [], "99 of 100 sampled")
        sparse = {frame: 1 for frame in range(1, 101) if frame % 10}
        bad = perf_stats.analyze(gpu_log(100, sparse, tail=4), 0, 100, [], gpu=True)
        self.assertIn("GPU timings on 90 of 100 window frames, under 95%", bad["problems"])
        no_timestamps = gpu_log(100, {}).replace("timestamp_query: true", "timestamp_query: false")
        self.assertEqual(perf_stats.analyze(no_timestamps, 0, 100, [], gpu=True)["problems"], [])


class Guards(unittest.TestCase):
    def test_guards_pass_and_fail(self):
        frames = perf_stats.parse_log(physics_log(5)).frames
        results = [perf_stats.evaluate_guard(guard, frames) for guard in GUARDS]
        self.assertTrue(all(result["ok"] for result in results), results)
        failing = perf_stats.evaluate_guard({"kind": "counter_min", "counter": "teleports", "min": 3}, frames)
        self.assertFalse(failing["ok"])
        self.assertIn("first: measured frame 1", failing["detail"])
        const = perf_stats.evaluate_guard({"kind": "counter_const", "counter": "teleports"}, frames)
        self.assertFalse(const["ok"])

    def test_analysis_reports_missing_frames_and_guards(self):
        analysis = perf_stats.analyze(physics_log(4), 2, 5, GUARDS)
        self.assertIn("2 measured frames of 5 (4 logged, 2 warm-up)", analysis["problems"])


class Digest(unittest.TestCase):
    def test_the_last_window_frames_bench_line_and_the_tail_stay_out(self):
        without = perf_stats.analyze(physics_log(7), 2, 5, [])["digest"]
        with_tail = perf_stats.analyze(physics_log(7, tail=4), 2, 5, [])["digest"]
        self.assertEqual(without, with_tail)
        frames = perf_stats.parse_log(physics_log(7, tail=4)).frames[2:7]
        self.assertIsNotNone(frames[-1].bench, "the tail printed the last frame's bench line")

    def test_a_workload_change_moves_the_digest(self):
        base = perf_stats.analyze(physics_log(7), 2, 5, [])["digest"]
        moved = perf_stats.analyze(physics_log(7).replace("pairs=3", "pairs=99"), 2, 5, [])["digest"]
        self.assertNotEqual(base, moved)


class Provenance(unittest.TestCase):
    def git(self, repo, *args):
        return subprocess.run(["git", "-C", str(repo), *args], capture_output=True, text=True, check=True).stdout.strip()

    def test_an_export_records_its_refs_commit_and_the_tree_its_own(self):
        with tempfile.TemporaryDirectory() as directory:
            repo = Path(directory) / "repo"
            repo.mkdir()
            self.git(repo, "init", "-q")
            self.git(repo, "config", "user.email", "t@example.com")
            self.git(repo, "config", "user.name", "t")
            (repo / "a.txt").write_text("one\n")
            self.git(repo, "add", "a.txt")
            self.git(repo, "commit", "-q", "-m", "one")
            self.git(repo, "tag", "v1")
            first = self.git(repo, "rev-parse", "--short", "HEAD")
            (repo / "a.txt").write_text("two\n")
            self.git(repo, "commit", "-q", "-am", "two")
            second = self.git(repo, "rev-parse", "--short", "HEAD")
            export = Path(directory) / "export"
            self.assertEqual(perf.tree_state(export, ref="v1", repo=repo), {"commit": first, "dirty": "no", "ref": "v1"})
            self.assertEqual(perf.tree_state(repo), {"commit": second, "dirty": "no", "ref": None})
            (repo / "a.txt").write_text("three\n")
            dirty = perf.tree_state(repo)["dirty"]
            self.assertTrue(dirty.startswith("yes (diff "), dirty)


class Comparability(unittest.TestCase):
    def test_each_hard_gate_suppresses_every_verdict(self):
        base = capture()
        cases = {
            "row": lambda c: c.update(row="ecs"),
            "workload version": lambda c: c["workload"].update(version=2),
            "knob balls": lambda c: c["workload"]["config"]["knobs"].update(balls=4000),
            "script": lambda c: c["workload"].update(script_sha256="beef"),
            "resolution": lambda c: c["workload"].update(resolution=[3840, 2160]),
            "warm-up": lambda c: c["workload"].update(warmup=60),
            "frames": lambda c: c["workload"].update(frames=150),
            "build flags": lambda c: c["build"].update(flags=perf.PLAIN_FLAGS),
            "backend": lambda c: c["renderer"].update(backend="Metal"),
            "adapter": lambda c: c["renderer"].update(adapter="Other"),
            "present mode": lambda c: c["renderer"].update(present_mode="fifo"),
            "max frame latency": lambda c: c["renderer"].update(max_frame_latency=2),
            "machine": lambda c: c["machine"].update(fingerprint="000000000000"),
        }
        for label, change in cases.items():
            with self.subTest(label=label):
                cand = capture(updates=(12.0, 12.1, 11.9))
                change(cand)
                report = perf_stats.compare(base, cand, OWNED)
                self.assertTrue(any(item.startswith(label) for item in report["hard"]), report["hard"])
                self.assertTrue(all(result["verdict"] is None for result in report["owned"]))
        report = perf_stats.compare(base, capture(updates=(12.0, 12.1, 11.9)), OWNED)
        self.assertEqual(report["hard"], [])
        self.assertEqual(report["owned"][0]["verdict"], "regressed")

    def test_an_invalid_side_suppresses_verdicts(self):
        report = perf_stats.compare(capture(), capture(updates=(12.0, 12.1, 11.9), valid=False), OWNED)
        self.assertEqual(report["invalid"], ["candidate invalid"])
        self.assertIsNone(report["owned"][0]["verdict"])


class Verdicts(unittest.TestCase):
    """The `D-078` rule on the old runner's fixtures."""

    def test_labels_and_too_few_runs(self):
        judge = perf_stats.judge
        base = [10.0, 10.1, 9.9]
        self.assertEqual(judge(base, [11.0, 11.1, 10.9], 0.05, 0.05)["verdict"], "regressed")
        self.assertEqual(judge(base, [9.0, 9.1, 8.9], 0.05, 0.05)["verdict"], "improved")
        steady = judge([10.0, 10.02, 9.98], [10.01, 10.03, 9.99], 0.03, 0.05)
        self.assertEqual(steady["verdict"], "unchanged")
        self.assertLess(steady["interval"][0], 0.0)
        self.assertEqual(judge([10.0, 12.0, 8.0], [10.5, 12.5, 8.5], 0.03, 0.05)["verdict"], "noisy")
        few = judge([10.0, 10.0], [11.0, 11.0, 11.0], 0.05, 0.05)
        self.assertIsNone(few["verdict"])
        self.assertAlmostEqual(few["delta"], 1.0)
        self.assertAlmostEqual(few["delta_pct"], 10.0)

    def test_absolute_floors_and_rss_thresholds(self):
        judge = perf_stats.judge
        system, stage = perf_stats.TAU_ABS_MS["systems"], perf_stats.TAU_ABS_MS["stages"]
        small = judge([0.1] * 3, [0.115] * 3, perf_stats.TAU_REL["p95"], system)
        self.assertAlmostEqual(small["tau"], 0.02)
        self.assertEqual(small["verdict"], "unchanged")
        self.assertEqual(judge([0.1] * 3, [0.13] * 3, perf_stats.TAU_REL["p95"], system)["verdict"], "regressed")
        self.assertEqual(judge([0.5] * 3, [0.54] * 3, perf_stats.TAU_REL["p50"], stage)["verdict"], "unchanged")
        self.assertEqual(judge([0.5] * 3, [0.56] * 3, perf_stats.TAU_REL["p50"], stage)["verdict"], "regressed")
        rss = (perf_stats.RSS_TAU_REL, perf_stats.RSS_TAU_ABS_KIB)
        peaks = [100_000, 100_100, 99_900]
        self.assertEqual(judge(peaks, [peak + 500 for peak in peaks], *rss)["verdict"], "unchanged")
        self.assertEqual(judge(peaks, [peak + 3_000 for peak in peaks], *rss)["verdict"], "regressed")
        large = [400_000, 400_200, 399_800]
        self.assertEqual(judge(large, [peak + 2_000 for peak in large], *rss)["tau"], 8_000)
        self.assertEqual(judge(large, [peak + 14_000 for peak in large], *rss)["verdict"], "regressed")


class Rss(unittest.TestCase):
    def test_rss_is_judged_within_a_sitting_only(self):
        base, cand = capture(), capture()
        for run in cand["runs"]:
            run["rss_peak_kib"] += 60 * 1024
        across = perf_stats.compare(base, cand, OWNED)["rss"]
        self.assertIsNone(across["verdict"])
        self.assertIn("different sittings", across["note"])
        self.assertIsNotNone(across["cand"], "both values are still printed")
        base["sitting"] = cand["sitting"] = "20261029T000000Z"
        self.assertEqual(perf_stats.compare(base, cand, OWNED)["rss"]["verdict"], "regressed")
        cand["sitting"] = "20261030T000000Z"
        self.assertIsNone(perf_stats.compare(base, cand, OWNED)["rss"]["verdict"])


class Budgets(unittest.TestCase):
    def test_within_near_and_over(self):
        self.assertEqual(perf_stats.budget_verdict([10.0, 10.1, 9.9], 11.0), "within")
        self.assertEqual(perf_stats.budget_verdict([10.0, 10.1, 9.9], 10.0), "near")
        self.assertEqual(perf_stats.budget_verdict([10.0, 10.1, 9.9], 9.0), "over")
        self.assertIsNone(perf_stats.budget_verdict([10.0], 9.0))
        budget = {"metric": "stage.update", "stat": "p50", "ms": 9.0}
        report = perf_stats.compare(capture(), capture(), OWNED, [budget])
        self.assertEqual(report["budgets"][0]["verdict"], "over")


def fake_game(directory, log_text, *, report=None, exit_code=0):
    """An executable standing in for a game: it records its environment in
    `env-<n>.json`, writes a script report when asked, prints `log_text`
    and exits with `exit_code`."""
    script = Path(directory) / "fake-game"
    script.write_text(
        f"#!{sys.executable}\n"
        "import json, os, pathlib, sys\n"
        f"state = pathlib.Path({str(directory)!r})\n"
        "count = len(list(state.glob('env-*.json'))) + 1\n"
        "(state / f'env-{count}.json').write_text(json.dumps(dict(os.environ)))\n"
        f"report = {json.dumps(report)!r}\n"
        "if os.environ.get('TUNGSTEN_REPORT') and report != 'null':\n"
        "    pathlib.Path(os.environ['TUNGSTEN_REPORT']).write_text(report)\n"
        f"sys.stdout.write({log_text!r})\n"
        f"sys.exit({exit_code})\n"
    )
    script.chmod(0o755)
    return script


PROVENANCE = {"commit": "abc1234", "dirty": "no", "ref": None}


class Runner(unittest.TestCase):
    def run_capture(self, test_row, log_text, **kwargs):
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        root = Path(directory.name)
        report = kwargs.pop("report", None)
        exit_code = kwargs.pop("exit_code", 0)
        binary = fake_game(root, log_text, report=report, exit_code=exit_code)
        out = root / "out"
        out.mkdir()
        with contextlib.redirect_stdout(io.StringIO()):
            result = self.capture_row(test_row, binary, root, out, kwargs)
        envs = [json.loads(path.read_text()) for path in sorted(root.glob("env-*.json"))]
        return result, envs, out

    @staticmethod
    def capture_row(test_row, binary, root, out, kwargs):
        return perf.capture_row(
            test_row, binary, root, out, flags=perf.ALIGNED_FLAGS, provenance=PROVENANCE, machine_info={"fingerprint": "f"},
            lister=kwargs.pop("lister", lambda: []), tree_reader=kwargs.pop("tree_reader", lambda: {"commit": "abc1234", "dirty": "no"}),
            **kwargs,
        )  # fmt: skip

    def test_a_bench_row_is_valid_with_its_config_guards_and_digest(self):
        capture, envs, out = self.run_capture(row(), physics_log(7))
        self.assertTrue(capture["valid"], capture["reasons"])
        self.assertEqual(len(capture["runs"]), 3)
        self.assertEqual(capture["workload"]["config"]["knobs"], {"balls": 8000})
        self.assertEqual(capture["renderer"]["adapter"], "AMD Radeon 660M (RADV REMBRANDT)")
        self.assertEqual(envs[0]["TUNGSTEN_SMOKE_FRAMES"], "7")
        self.assertEqual(envs[0]["TUNGSTEN_BENCH_PRESET"], "default")
        self.assertTrue((out / "capture.json").is_file() and (out / "README.md").is_file())

    def test_a_gpu_run_takes_tail_frames_and_digests_alike(self):
        lines, frame = [], 0
        for line in physics_log(11).splitlines():
            if "frame:" in line:
                frame += 1
                if frame > 1:
                    lines.append(APP + f"gpu_passes: from={frame - 1} scene=1.00ms render_span=2.00ms")
            lines.append(line)
        capture, envs, _ = self.run_capture(row(gpu=True, repeat=1), "\n".join(lines) + "\n")
        self.assertTrue(capture["valid"], capture["reasons"])
        timing, gpu = envs
        self.assertNotIn("TUNGSTEN_GPU_TIMING", timing)
        self.assertEqual(gpu["TUNGSTEN_GPU_TIMING"], "1")
        self.assertEqual(int(gpu["TUNGSTEN_SMOKE_FRAMES"]), 7 + perf.GPU_TAIL)
        self.assertEqual(capture["workload"]["tail"], perf.GPU_TAIL)

    def test_invalid_on_a_wrong_preset_a_short_window_an_exit_or_a_guard(self):
        wrong = {**CONFIG, "preset": "min"}
        cases = {
            "names preset min": (physics_log(7, config=wrong), 0),
            "measured frames of 5": (physics_log(4), 0),
            "exited with 1": (physics_log(7), 1),
        }
        for label, (text, code) in cases.items():
            with self.subTest(label=label):
                capture, _, _ = self.run_capture(row(repeat=1), text, exit_code=code)
                self.assertFalse(capture["valid"])
                self.assertTrue(any(label in reason for reason in capture["reasons"]), capture["reasons"])
        guarded = row(repeat=1, guards=[{"kind": "counter_min", "counter": "teleports", "min": 100}])
        capture, _, _ = self.run_capture(guarded, physics_log(7))
        self.assertFalse(capture["valid"])

    def script_row(self, **changes):
        script = {"steps": [{"frame": 2, "action": "start_run"}, {"frame": 3, "until": "end", "action": "hold"}],
                  "asserts": [{"probe": "pit", "path": "/balls", "op": "==", "value": 12000}]}  # fmt: skip
        return row(name="e01-pit", package="example-01-platformer", folder=".", launch={"script": script}, guards=[], repeat=1, **changes)

    def test_a_scripted_row_needs_its_report_to_pass(self):
        text = "\n".join(frame_line(10.0) for _ in range(7)) + "\n"
        capture, envs, out = self.run_capture(self.script_row(), text, report={"passed": True, "asserts": []})
        self.assertTrue(capture["valid"], capture["reasons"])
        self.assertIsNone(capture["workload"]["config"])
        self.assertEqual(capture["workload"]["script_sha256"], perf.script_sha256(self.script_row()))
        written = json.loads((out / "run-1" / "timing-script.json").read_text())
        self.assertEqual(written["frames"], 7)
        self.assertEqual(written["steps"][1]["until"], 7)
        self.assertEqual(written["steps"][-1], {"frame": 7, "assert": {"probe": "pit", "path": "/balls", "op": "==", "value": 12000}})
        self.assertIn("TUNGSTEN_USER_DIR", envs[0])
        self.assertNotIn("TUNGSTEN_SMOKE_FRAMES", envs[0])
        failed = {"passed": False, "asserts": [{"probe": "pit", "path": "/balls", "op": "==", "value": 12000, "actual": 3, "passed": False}]}
        capture, _, _ = self.run_capture(self.script_row(), text, report=failed)
        self.assertFalse(capture["valid"])
        self.assertIn("actual 3", " ".join(capture["reasons"]))
        capture, _, _ = self.run_capture(self.script_row(), text)
        self.assertIn("timing: no script report", " ".join(capture["reasons"]))

    def test_background_load_invalidates_unless_allowed(self):
        lister = lambda: [(4242, 1, "nxcodec.bin")]  # noqa: E731
        capture, _, _ = self.run_capture(row(repeat=1), physics_log(7), lister=lister)
        self.assertFalse(capture["valid"])
        self.assertIn("background load: nxcodec.bin", " ".join(capture["reasons"]))
        capture, _, _ = self.run_capture(row(repeat=1), physics_log(7), lister=lister, allow_background=True)
        self.assertTrue(capture["valid"], capture["reasons"])
        self.assertTrue(capture["notes"])
        moved = lambda: {"commit": "abc1234", "dirty": "yes (diff 1)"}  # noqa: E731
        capture, _, _ = self.run_capture(row(repeat=1), physics_log(7), tree_reader=moved)
        self.assertIn("tree changed", " ".join(capture["reasons"]))

    def test_own_builds_are_not_background_load(self):
        own = [(os.getpid(), 1, "python3"), (900_001, os.getpid(), "cargo"), (900_002, 900_001, "rustc")]
        self.assertEqual(perf.background_offenders(own, os.getpid()), [])
        self.assertEqual(perf.background_offenders([(5, 1, "cargo")], os.getpid()), ["cargo"])

    def test_child_env_clears_measuring_variables(self):
        env = perf.child_env({"TUNGSTEN_BENCH_SCALE": "2", "RUST_LOG": "trace", "TUNGSTEN_SCRIPT": "x", "HOME": "/h"}, {"A": "1"})
        self.assertEqual(env, {"HOME": "/h", "A": "1"})


class Registry(unittest.TestCase):
    def test_every_row_is_well_formed_and_groups_select(self):
        registry = perf.load_registry()
        names = [entry["name"] for entry in registry["rows"]]
        self.assertEqual(len(names), len(set(names)))
        for entry in registry["rows"]:
            with self.subTest(row=entry["name"]):
                self.assertEqual(set(entry["launch"]) & {"bench", "script"}, set(entry["launch"]) - {"preset", "scale"})
                for owned in entry["owned"]:
                    self.assertIn(owned["metric"].partition(".")[0], perf_stats.METRIC_GROUPS)
        self.assertEqual(len(perf.select_rows(registry, "e01-areas")), 6)
        with self.assertRaises(perf.PerfError):
            perf.select_rows(registry, "nope")

    def test_baselines_refuse_schema_one(self):
        with tempfile.TemporaryDirectory() as directory:
            old = Path(directory) / "old"
            old.mkdir()
            (old / "capture.json").write_text(json.dumps({"schema": 1}))
            with self.assertRaisesRegex(perf.PerfError, "schema 1"):
                perf.load_capture(str(old), root=Path(directory))


if __name__ == "__main__":
    unittest.main()
