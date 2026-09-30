#!/usr/bin/env python3
"""Benchmark runner for example-02-bench (docs/perf/profiling-workflow.md).

  describe [bench] [--json]      knobs, presets and tracked rows, from the binary
  run <bench> [flags]            one capture (or a --sweep), optionally --compare
  suite [flags]                  one capture per tracked row, suite.json and README.md
  compare <baseline> <candidate> Welch verdicts, compare.md and compare.html (captures or suites)
  capacity (<bench>... | --all)  largest scale within a frame-time budget
  baseline save|list|rm          named baselines under perf-runs/baselines/

`just perf <subcommand>` wraps it. Captures land in perf-runs/<UTC>-<row>...;
exit status 3 means a capture was written but is invalid, and `compare
--fail-on regressed` exits 1 on a regressed owned metric. Standard library
only, Python 3.12.
"""

import argparse
import datetime
import hashlib
import json
import math
import os
import re
import shutil
import socket
import statistics
import subprocess
import sys
import time
from pathlib import Path

sys.dont_write_bytecode = True  # importing bench_report must not leave scripts/__pycache__
import bench_report  # noqa: E402

REPO_ROOT = Path(__file__).resolve().parent.parent
PACKAGE = "example-02-bench"
# RUSTFLAGS replaces .cargo/config.toml's target-cpu=native, so captures are
# generic x86-64 with frame pointers unless TUNGSTEN_PERF_RUSTFLAGS says
# otherwise. Only compare captures built with the same flags.
DEFAULT_RUSTFLAGS = "-C force-frame-pointers=yes"
RUST_LOG = "tungsten::app=debug,bench=debug"
# Inherited variables that would change what a child measures.
CLEARED_PREFIXES = ("TUNGSTEN_BENCH", "TUNGSTEN_RENDER_", "TUNGSTEN_DISPLAY_", "TUNGSTEN_CAPTURE_")
CLEARED_NAMES = ("TUNGSTEN_GPU_TIMING", "TUNGSTEN_OVERLAYS_ON", "TUNGSTEN_SMOKE_FRAMES", "TUNGSTEN_PERF_LOG", "RUST_LOG")
RSS_POLL_SECONDS = 0.1
EXIT_INVALID = 3
EXIT_REGRESSED = 1
BUDGETS_MS = {"60hz": 16.7, "144hz": 6.9}
CONFIRM_RUNS = 3
MAX_STEP_DOWNS = 3


class BenchError(Exception):
    """A request or environment problem reported without a traceback."""


# --- Provenance ------------------------------------------------------------


def file_value_or_na(path):
    """First line of a sysfs-style file, or `n/a` when absent or empty."""
    try:
        with open(path, encoding="utf-8", errors="replace") as handle:
            value = handle.readline().strip()
    except OSError:
        return "n/a"
    return value or "n/a"


def git_output(root, *args):
    return subprocess.run(["git", "-C", str(root), *args], capture_output=True, check=True).stdout


def git_commit(root):
    try:
        return git_output(root, "rev-parse", "--short", "HEAD").decode().strip() or "unknown"
    except (OSError, subprocess.CalledProcessError):
        return "unknown"


def git_dirty(root):
    """`no` for a clean tree, else `yes (diff <12 hex>)`: SHA-256 over the
    tracked diff plus `sha256sum` lines of the untracked files, so dirty
    captures of one tree still match."""
    root = Path(root)
    try:
        status = git_output(root, "status", "--porcelain")
    except (OSError, subprocess.CalledProcessError):
        return "unknown"
    if not status.strip():
        return "no"
    sha = hashlib.sha256()
    try:
        sha.update(git_output(root, "diff", "HEAD", "--binary"))
        untracked = git_output(root, "ls-files", "-z", "--others", "--exclude-standard")
    except (OSError, subprocess.CalledProcessError):
        untracked = b""
    for rel in untracked.split(b"\0"):
        if not rel:
            continue
        try:
            file_hash = hashlib.sha256((root / os.fsdecode(rel)).read_bytes()).hexdigest()
        except OSError:
            continue
        sha.update(file_hash.encode() + b"  " + rel + b"\n")
    return f"yes (diff {sha.hexdigest()[:12]})"


def cpu_model(cpuinfo_text):
    for line in cpuinfo_text.splitlines():
        key, sep, value = line.partition(":")
        if sep and key.strip() == "model name":
            return value.strip() or "unknown"
    return "unknown"


def mem_total_kib(meminfo_text):
    match = re.search(r"^MemTotal:\s+(\d+) kB", meminfo_text, re.MULTILINE)
    return int(match[1]) if match else None


def ac_power(power_supply=Path("/sys/class/power_supply")):
    """`online` or `offline` from the first mains supply, `n/a` without one."""
    try:
        supplies = sorted(Path(power_supply).iterdir())
    except OSError:
        return "n/a"
    for supply in supplies:
        if file_value_or_na(supply / "type") == "Mains":
            return {"1": "online", "0": "offline"}.get(file_value_or_na(supply / "online"), "n/a")
    return "n/a"


def read_text(path):
    try:
        return Path(path).read_text(encoding="utf-8", errors="replace")
    except OSError:
        return ""


def machine_fingerprint(hostname, cpu, mem_kib):
    """Identity of the capture host; compare treats a mismatch as hard."""
    return hashlib.sha256(f"{hostname}\n{cpu}\n{mem_kib}".encode()).hexdigest()[:12]


def frame_pointers_enabled(rustflags):
    """RUSTFLAGS is whitespace-separated; the last frame-pointer flag wins."""
    enabled = False
    for token in rustflags.split():
        token = token.removeprefix("-C")
        if token.startswith("force-frame-pointers="):
            enabled = token == "force-frame-pointers=yes"
    return enabled


def rustc_version():
    try:
        return subprocess.run(["rustc", "--version"], capture_output=True, text=True, check=True).stdout.strip()
    except (OSError, subprocess.CalledProcessError):
        return "unknown"


def collect_provenance(root):
    """Host and tree state, read before the build so it describes the built sources."""
    cpu = cpu_model(read_text("/proc/cpuinfo"))
    hostname = socket.gethostname()
    mem_kib = mem_total_kib(read_text("/proc/meminfo"))
    uname = os.uname()
    return {
        "commit": git_commit(root),
        "dirty": git_dirty(root),
        "governor": file_value_or_na("/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor"),
        "platform_profile": file_value_or_na("/sys/firmware/acpi/platform_profile"),
        "ac_power": ac_power(),
        "kernel": f"{uname.sysname} {uname.release}",
        "cpu": cpu,
        "mem_total_kib": mem_kib,
        "hostname": hostname,
        "machine": machine_fingerprint(hostname, cpu, mem_kib),
        "rustc": rustc_version(),
        "wgpu_backend_env": os.environ.get("WGPU_BACKEND", "auto"),
    }


# --- Child processes -------------------------------------------------------


def child_env(parent, extra=None):
    """Copy of `parent` without inherited benchmark, render, display, capture,
    overlay, smoke and log settings, plus `extra`."""
    env = {
        name: value
        for name, value in parent.items()
        if not name.startswith(CLEARED_PREFIXES) and name not in CLEARED_NAMES
    }
    env.update(extra or {})
    return env


def bench_vars(bench, preset=None, scale=None, sets=()):
    env = {"TUNGSTEN_BENCH": bench}
    if preset is not None:
        env["TUNGSTEN_BENCH_PRESET"] = preset
    if scale is not None:
        env["TUNGSTEN_BENCH_SCALE"] = scale
    if sets:
        env["TUNGSTEN_BENCH_SET"] = ",".join(sets)
    return env


def capture_env(parent, bench_env, total_frames, *, gpu=False, profile=False, present_mode=None, max_frame_latency=None):
    """Environment of a timing run (perf lines on), a GPU diagnostic run
    (`gpu`) or a profiler run (`profile`: errors only). Present overrides
    reach only the child."""
    env = child_env(parent, bench_env)
    env["TUNGSTEN_SMOKE_FRAMES"] = str(total_frames)
    if profile:
        env["RUST_LOG"] = "error"
    else:
        env["TUNGSTEN_PERF_LOG"] = "1"
        env["RUST_LOG"] = RUST_LOG
    if gpu:
        env["TUNGSTEN_GPU_TIMING"] = "1"
    if present_mode is not None:
        env["TUNGSTEN_RENDER_PRESENT_MODE"] = present_mode
    if max_frame_latency is not None:
        env["TUNGSTEN_RENDER_MAX_FRAME_LATENCY"] = str(max_frame_latency)
    return env


def read_rss_kib(pid, page_kib):
    try:
        with open(f"/proc/{pid}/statm", encoding="ascii") as handle:
            return int(handle.read().split()[1]) * page_kib
    except (OSError, IndexError, ValueError):
        return None


def run_child(argv, env, cwd, log_path, rss_path=None):
    """Run `argv` with stdout and stderr in `log_path`, reaping it with
    `os.wait4` for its rusage while sampling `/proc/<pid>/statm` every 100 ms.
    Popen's returncode is set from the wait status, so it never waits again."""
    log_path = Path(log_path)
    log_path.parent.mkdir(parents=True, exist_ok=True)
    page_kib = os.sysconf("SC_PAGE_SIZE") // 1024
    samples = []
    start = time.monotonic()
    with open(log_path, "wb") as log:
        proc = subprocess.Popen(argv, env=env, cwd=cwd, stdout=log, stderr=subprocess.STDOUT)
        try:
            while True:
                pid, status, usage = os.wait4(proc.pid, os.WNOHANG)
                if pid == proc.pid:
                    break
                rss = read_rss_kib(proc.pid, page_kib)
                if rss:
                    samples.append((round(time.monotonic() - start, 3), rss))
                time.sleep(RSS_POLL_SECONDS)
        except BaseException:
            proc.kill()
            proc.wait()
            raise
    proc.returncode = os.waitstatus_to_exitcode(status)
    if rss_path is not None:
        lines = ["seconds\trss_kib"] + [f"{seconds}\t{rss}" for seconds, rss in samples]
        Path(rss_path).write_text("\n".join(lines) + "\n")
    return {
        "exit_code": proc.returncode,
        "wall_s": round(time.monotonic() - start, 3),
        "rusage": {
            "peak_rss_kib": usage.ru_maxrss,
            "user_s": round(usage.ru_utime, 3),
            "sys_s": round(usage.ru_stime, 3),
            "minflt": usage.ru_minflt,
            "majflt": usage.ru_majflt,
            "nvcsw": usage.ru_nvcsw,
            "nivcsw": usage.ru_nivcsw,
        },
        "rss_samples": samples,
    }


# --- Build and describe ----------------------------------------------------


def perf_rustflags():
    """Capture build flags: `TUNGSTEN_PERF_RUSTFLAGS`, else `DEFAULT_RUSTFLAGS`."""
    return os.environ.get("TUNGSTEN_PERF_RUSTFLAGS", DEFAULT_RUSTFLAGS)


def build(root, rustflags):
    print(f'Building {PACKAGE} with RUSTFLAGS="{rustflags}"...', flush=True)
    env = dict(os.environ, RUSTFLAGS=rustflags)
    if subprocess.run(["cargo", "build", "--release", "-p", PACKAGE], cwd=root, env=env).returncode != 0:
        raise BenchError("build failed")
    metadata = subprocess.run(
        ["cargo", "metadata", "--no-deps", "--format-version", "1"],
        cwd=root,
        capture_output=True,
        text=True,
        check=True,
    ).stdout
    binary = Path(json.loads(metadata)["target_directory"]) / "release" / PACKAGE
    if not binary.is_file():
        raise BenchError(f"built binary not found at {binary}")
    return binary


def binary_json(binary, root, env):
    """Run the binary in one of its describe modes and parse its JSON."""
    result = subprocess.run([str(binary)], cwd=root, env=env, capture_output=True, text=True)
    if result.returncode != 0:
        raise BenchError(result.stderr.strip() or f"{binary} exited with {result.returncode}")
    return json.loads(result.stdout)


def find_bench(schema, name):
    for bench in schema["benches"]:
        if bench["name"] == name:
            return bench
    names = ", ".join(bench["name"] for bench in schema["benches"])
    raise BenchError(f"unknown benchmark '{name}'; benchmarks: {names}")


def find_row(bench, name):
    for row in bench["rows"]:
        if row["name"] == name:
            return row
    raise BenchError(f"{bench['name']} does not list row '{name}'")


# --- Captures --------------------------------------------------------------


def capture_dir_name(stamp, row, preset, row_preset, scale, sets, present_mode, max_frame_latency):
    """`<UTC>-<row>[-<preset>][-s<scale>][-set<hash6>][-<present>][-lat<N>]`."""
    parts = [stamp, row]
    if preset is not None and preset != row_preset:
        parts.append(preset)
    if scale is not None and float(scale) != 1.0:
        parts.append(f"s{float(scale):g}")
    if sets:
        parts.append("set" + hashlib.sha256(",".join(sets).encode()).hexdigest()[:6])
    if present_mode is not None:
        parts.append(present_mode)
    if max_frame_latency is not None:
        parts.append(f"lat{max_frame_latency}")
    return "-".join(parts)


def unique_dir(parent, name):
    path = parent / name
    suffix = 2
    while path.exists():
        path = parent / f"{name}-{suffix}"
        suffix += 1
    return path


def utc_stamp():
    return datetime.datetime.now(datetime.UTC).strftime("%Y%m%dT%H%M%SZ")


def new_run_dir(root, name):
    """A fresh `perf-runs/<name>` directory; `unique_dir` suffixes a taken name."""
    runs_root = root / "perf-runs"
    runs_root.mkdir(exist_ok=True)
    path = unique_dir(runs_root, name)
    path.mkdir()
    return path


def perf_record_args(call_graph, sample_frequency):
    args = ["--call-graph", call_graph]
    if sample_frequency:
        args += ["-F", str(sample_frequency)]
    return args


def capture_profile(binary, env, root, out_dir, call_graph, sample_frequency):
    """perf stat, perf record and a flamegraph folded from that recording, all
    under `profile/`. Profilers run the capture binary from the repo root and
    write only into the capture."""
    profile_dir = out_dir / "profile"
    profile_dir.mkdir()
    notes = []
    if shutil.which("perf"):
        print("Capturing perf stat...", flush=True)
        subprocess.run(
            ["perf", "stat", "-d", "-o", str(profile_dir / "perf-stat.txt"), str(binary)],
            cwd=root,
            env=env,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        print("Capturing perf record...", flush=True)
        with open(profile_dir / "perf-record.log", "wb") as log:
            subprocess.run(
                ["perf", "record", *perf_record_args(call_graph, sample_frequency), "-o", str(profile_dir / "perf-record.data"), str(binary)],
                cwd=root,
                env=env,
                stdout=log,
                stderr=subprocess.STDOUT,
            )
    else:
        notes.append("perf not installed; perf stat and perf record skipped")
    if not shutil.which("flamegraph"):
        notes.append("flamegraph not installed (cargo install flamegraph); flamegraph skipped")
    elif (profile_dir / "perf-record.data").is_file():
        print("Rendering flamegraph from perf record data...", flush=True)
        subprocess.run(
            ["flamegraph", "--perfdata", str(profile_dir / "perf-record.data"), "--output", str(profile_dir / "flamegraph.svg")],
            cwd=profile_dir,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
    files = sorted(path.name for path in profile_dir.iterdir())
    return {"call_graph": call_graph, "sample_frequency": sample_frequency, "files": files, "notes": notes}


def validate_sets(sets):
    for entry in sets:
        name, sep, value = entry.partition("=")
        if not sep or not name or not value or "," in entry:
            raise BenchError(f"--set expects knob=value without commas, got '{entry}'")


def load_schema(binary, root):
    return binary_json(binary, root, child_env(os.environ, {"TUNGSTEN_BENCH_DESCRIBE": "1"}))


def resolve_config(binary, root, request_env):
    """The binary's resolved `bench-config` object for a request; a bad knob,
    preset or value is its fatal error, raised here before anything runs."""
    return binary_json(binary, root, child_env(os.environ, {**request_env, "TUNGSTEN_BENCH_DESCRIBE": "config"}))


def run_request(args, sets, bench):
    return {
        "bench": args.bench,
        "preset": args.preset,
        "scale": args.scale,
        "set": list(sets),
        "frames": args.frames,
        "warmup": args.warmup,
        "repeat": args.repeat,
        "gpu_timing": bench["gpu_timing"] if args.gpu_timing is None else args.gpu_timing == "on",
        "present_mode": args.present_mode,
        "max_frame_latency": args.max_frame_latency,
        "profile": args.profile,
    }


def capture_to(out_dir, *, binary, root, bench, config, request_env, request, provenance, rustflags, profile_args=None):
    """Timing runs (plus GPU diagnostic runs), optional profile, then
    `capture.json` and `README.md` in `out_dir`. Returns the capture."""
    row = find_row(bench, config["row"])
    warmup = bench["warmup"] if request["warmup"] is None else request["warmup"]
    gpu_timing = request["gpu_timing"]
    frames = request["frames"]
    total_frames = warmup + frames
    repeat = request["repeat"]
    present = {"present_mode": request["present_mode"], "max_frame_latency": request["max_frame_latency"]}
    runs = []
    for index in range(1, repeat + 1):
        run_dir = out_dir / f"run-{index}"
        print(f"Timing run {index} of {repeat} ({total_frames} frames)...", flush=True)
        env = capture_env(os.environ, request_env, total_frames, **present)
        timing = run_child([str(binary)], env, root, run_dir / "telemetry.log", run_dir / "rss.tsv")
        analysis = bench_report.analyze_log(
            (run_dir / "telemetry.log").read_text(errors="replace"), warmup, frames, row["guards"], config
        )
        samples = timing.pop("rss_samples")
        run = {"index": index, **timing, "rss_samples": len(samples), "rss_growth_kib_s": bench_report.rss_growth(samples)}
        run.update(analysis)
        if gpu_timing:
            print(f"GPU diagnostic run {index} of {repeat}...", flush=True)
            env = capture_env(os.environ, request_env, total_frames, gpu=True, **present)
            gpu = run_child([str(binary)], env, root, run_dir / "gpu.log")
            gpu.pop("rss_samples")
            gpu.update(
                bench_report.analyze_log(
                    (run_dir / "gpu.log").read_text(errors="replace"), warmup, frames, row["guards"], config
                )
            )
            run["gpu_run"] = gpu
        runs.append(run)
        if timing["exit_code"] != 0 or (gpu_timing and run["gpu_run"]["exit_code"] != 0):
            print(f"Run {index} failed; skipping the remaining runs.", flush=True)
            break

    profile = None
    if profile_args:
        env = capture_env(os.environ, request_env, total_frames, profile=True, **present)
        profile = capture_profile(binary, env, root, out_dir, *profile_args)

    capture = bench_report.assemble_capture(
        request=request,
        bench=bench,
        row=row,
        config=config,
        warmup=warmup,
        frames=frames,
        build={"package": PACKAGE, "profile": "release", "rustflags": rustflags},
        provenance=provenance,
        runs=runs,
        profile=profile,
    )
    (out_dir / "capture.json").write_text(json.dumps(capture, indent=2) + "\n")
    (out_dir / "README.md").write_text(bench_report.capture_readme(capture))
    return capture


def parse_sweep(text):
    """`knob=v1,v2,...` -> (knob, [values]); values keep their spelling."""
    name, sep, raw = text.partition("=")
    values = [value.strip() for value in raw.split(",") if value.strip()]
    if not sep or not name.strip() or not values:
        raise BenchError(f"--sweep expects knob=v1,v2,..., got '{text}'")
    if len(set(values)) != len(values):
        raise BenchError(f"--sweep lists a value twice: '{text}'")
    return name.strip(), values


def path_label(value):
    return re.sub(r"[^A-Za-z0-9._-]", "_", value)


def cmd_run(args):
    root = REPO_ROOT
    validate_sets(args.set)
    sweep = parse_sweep(args.sweep) if args.sweep else None
    if sweep:
        if args.compare or args.profile:
            raise BenchError("--sweep can't be combined with --compare or --profile")
        if sweep[0] in {entry.partition("=")[0] for entry in args.set}:
            raise BenchError(f"--sweep {sweep[0]} conflicts with --set {sweep[0]}=...")
    baseline = resolve_capture(args.compare, root) if args.compare else None
    rustflags = perf_rustflags()
    if args.profile and args.call_graph == "fp" and not frame_pointers_enabled(rustflags):
        raise BenchError("--call-graph fp needs TUNGSTEN_PERF_RUSTFLAGS with -C force-frame-pointers=yes")
    provenance = collect_provenance(root)
    binary = build(root, rustflags)
    bench = find_bench(load_schema(binary, root), args.bench)
    if sweep:
        return run_sweep(args, sweep, root=root, binary=binary, bench=bench, provenance=provenance, rustflags=rustflags)

    request_env = bench_vars(args.bench, args.preset, args.scale, args.set)
    config = resolve_config(binary, root, request_env)
    row = find_row(bench, config["row"])
    request = run_request(args, args.set, bench)

    name = capture_dir_name(
        utc_stamp(), config["row"], args.preset, row["preset"], args.scale, args.set, args.present_mode, args.max_frame_latency
    )
    out_dir = new_run_dir(root, name)
    print(f"Output directory: {out_dir.relative_to(root)}", flush=True)
    profile_args = (args.call_graph, args.sample_frequency) if args.profile else None
    capture = capture_to(
        out_dir,
        binary=binary,
        root=root,
        bench=bench,
        config=config,
        request_env=request_env,
        request=request,
        provenance=provenance,
        rustflags=rustflags,
        profile_args=profile_args,
    )
    print_summary(capture, out_dir.relative_to(root))
    code = 0 if capture["valid"] else EXIT_INVALID
    if baseline is not None:
        write_compare(baseline, out_dir, root=root)
    return code


def run_sweep(args, sweep, *, root, binary, bench, provenance, rustflags):
    """One capture per swept value under one sweep directory, plus
    `sweep.json`, `sweep.md` and `sweep.html`. Every value resolves through
    the binary before anything runs."""
    knob, values = sweep
    plans = []
    for value in values:
        sets = [*args.set, f"{knob}={value}"]
        request_env = bench_vars(args.bench, args.preset, args.scale, sets)
        plans.append((value, sets, request_env, resolve_config(binary, root, request_env)))
    rows = {config["row"] for _, _, _, config in plans}
    if len(rows) != 1:
        raise BenchError(f"--sweep {knob} selects different rows ({', '.join(sorted(rows))}); sweep within one row")
    row = find_row(bench, plans[0][3]["row"])
    name = capture_dir_name(
        utc_stamp(), row["name"], args.preset, row["preset"], args.scale, args.set, args.present_mode, args.max_frame_latency
    )
    sweep_dir = new_run_dir(root, f"{name}-sweep-{path_label(knob)}")
    print(f"Sweep directory: {sweep_dir.relative_to(root)}", flush=True)
    entries = []
    for value, sets, request_env, config in plans:
        out_dir = sweep_dir / f"{path_label(knob)}-{path_label(value)}"
        out_dir.mkdir()
        print(f"Sweep {knob}={value}: {out_dir.relative_to(root)}", flush=True)
        request = run_request(args, sets, bench)
        capture = capture_to(
            out_dir,
            binary=binary,
            root=root,
            bench=bench,
            config=config,
            request_env=request_env,
            request=request,
            provenance=provenance,
            rustflags=rustflags,
        )
        print_summary(capture, out_dir.relative_to(root))
        entries.append((value, out_dir.name, capture))
    report = bench_report.sweep_report(knob, entries)
    (sweep_dir / "sweep.json").write_text(json.dumps(report, indent=2) + "\n")
    (sweep_dir / "sweep.md").write_text(bench_report.sweep_markdown(report))
    (sweep_dir / "sweep.html").write_text(bench_report.sweep_html(report))
    print(f"Sweep report: {sweep_dir.relative_to(root)}/sweep.md and sweep.html")
    return 0 if all(capture["valid"] for _, _, capture in entries) else EXIT_INVALID


def print_summary(capture, out_dir):
    runs = [run for run in capture["runs"] if run.get("stats")]
    for owned in capture["owned"]:
        for stat in owned["stats"]:
            per_run = [value for value in bench_report.owned_per_run(capture, owned["metric"], stat) if value is not None]
            value = bench_report.median_of(per_run)
            print(f"  {owned['metric']} {stat}: {bench_report.fmt(value)} ms (median of {len(per_run)} runs)")
    note = bench_report.gpu_metric_note(capture)
    if note:
        print(f"  {note}")
    total = bench_report.median_stat(runs, "stages", "total", "p95")
    print(f"  stage.total p95: {bench_report.fmt(total)} ms")
    peaks = [run["rusage"]["peak_rss_kib"] for run in capture["runs"]]
    print(f"  peak RSS: {', '.join(f'{peak / 1024:.1f}' for peak in peaks)} MiB")
    if capture["valid"]:
        print(f"Capture valid: {out_dir}/capture.json")
    else:
        print(f"Capture INVALID: {out_dir}/capture.json")
        for reason in capture["invalid_reasons"]:
            print(f"  - {reason}")


# --- Compare ---------------------------------------------------------------


REF_FILES = (("capture", "capture.json"), ("suite", "suite.json"))


def ref_kind(path):
    """`capture` or `suite` for a directory holding capture.json or suite.json, else None."""
    for kind, name in REF_FILES:
        if (Path(path) / name).is_file():
            return kind
    return None


def resolve_ref(ref, root=REPO_ROOT):
    """A capture or suite directory, or the name of a baseline under
    perf-runs/baselines/, as `(resolved path, "capture" | "suite")`."""
    path = Path(ref)
    if path.is_dir():
        kind = ref_kind(path)
        if kind:
            return path.resolve(), kind
        raise BenchError(f"{ref} has no capture.json or suite.json; only bench.py captures and suites can be compared")
    if re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9._-]*", ref):
        kind = ref_kind(baselines_dir(root) / ref)
        if kind:
            return (baselines_dir(root) / ref).resolve(), kind
    raise BenchError(f"'{ref}' is neither a capture or suite directory nor a baseline name (see `baseline list`)")


def resolve_capture(ref, root=REPO_ROOT):
    """`resolve_ref` for a single capture."""
    path, kind = resolve_ref(ref, root)
    if kind != "capture":
        raise BenchError(f"'{ref}' is a suite; compare suites with `compare` or `suite --compare`")
    return path


def capture_label(path, root=REPO_ROOT):
    path = Path(path)
    if path.parent == baselines_dir(root).resolve():
        return f"baseline {path.name}"
    try:
        return str(path.relative_to(root))
    except ValueError:
        return str(path)


def write_compare(base_dir, cand_dir, *, root=REPO_ROOT, out=None, force=False, echo=True):
    """Compare two capture directories and write compare.json, compare.md and
    compare.html; returns the report. `echo` prints the verdicts."""
    base = json.loads((Path(base_dir) / "capture.json").read_text())
    cand = json.loads((Path(cand_dir) / "capture.json").read_text())
    report = bench_report.compare(
        base,
        cand,
        base_side=(capture_label(base_dir, root), base_dir),
        cand_side=(capture_label(cand_dir, root), cand_dir),
        force=force,
    )
    if out is None:
        out_dir = new_run_dir(root, f"{utc_stamp()}-compare-{cand['row']}")
    else:
        out_dir = Path(out)
        out_dir.mkdir(parents=True, exist_ok=True)
    captures = {"baseline": base, "candidate": cand}
    frames = {"baseline": bench_report.frame_totals(base_dir, base), "candidate": bench_report.frame_totals(cand_dir, cand)}
    (out_dir / "compare.json").write_text(json.dumps(report, indent=2) + "\n")
    (out_dir / "compare.md").write_text(bench_report.compare_markdown(report))
    (out_dir / "compare.html").write_text(bench_report.compare_html(report, captures, frames))
    if echo:
        print_compare(report)
        print(f"Compare report: {capture_label(out_dir, root)}/compare.md and compare.html")
    return report


def write_suite_compare(base_dir, cand_dir, *, root=REPO_ROOT, out=None, force=False):
    """Compare two suite directories: each row both hold gets its own
    compare report in `<out>/<row>/`, and the suite-level compare.json,
    compare.md and compare.html summarize them. Returns the suite report."""
    base = json.loads((Path(base_dir) / "suite.json").read_text())
    cand = json.loads((Path(cand_dir) / "suite.json").read_text())
    if out is None:
        out_dir = new_run_dir(root, f"{utc_stamp()}-compare-suite")
    else:
        out_dir = Path(out)
        out_dir.mkdir(parents=True, exist_ok=True)
    cand_dirs ={row["row"]: row["dir"] for row in cand["rows"]}
    reports = {}
    for row in base["rows"]:
        if row["row"] in cand_dirs:
            reports[row["row"]] = write_compare(
                Path(base_dir) / row["dir"],
                Path(cand_dir) / cand_dirs[row["row"]],
                root=root,
                out=out_dir / row["row"],
                force=force,
                echo=False,
            )
    report = bench_report.suite_compare(
        base,
        cand,
        reports,
        base_side=(capture_label(base_dir, root), base_dir),
        cand_side=(capture_label(cand_dir, root), cand_dir),
    )
    (out_dir / "compare.json").write_text(json.dumps(report, indent=2) + "\n")
    (out_dir / "compare.md").write_text(bench_report.suite_compare_markdown(report))
    (out_dir / "compare.html").write_text(bench_report.suite_compare_html(report))
    for row in report["rows"]:
        notes = bench_report.suite_row_notes(row)
        shown = f" ({'; '.join(notes)})" if notes else ""
        print(f"  {row['row']}: {bench_report.summary_text(row['owned_summary'])}{shown}")
    for label, names in (("only in the baseline", report["only_baseline"]), ("only in the candidate", report["only_candidate"])):
        if names:
            print(f"  Rows {label}, not compared: {', '.join(names)}")
    print(f"Owned verdicts, all rows: {bench_report.summary_text(report['owned_summary'])}")
    print(f"Suite compare report: {capture_label(out_dir, root)}/compare.md and compare.html")
    return report


def print_compare(report):
    if report["hard"]:
        print("Not comparable; verdicts suppressed:")
        for item in report["hard"]:
            print(f"  - {item}")
    elif report["blocked"]:
        print(f"Verdicts suppressed: {'; '.join(report['invalid'])} (pass --force to judge anyway)")
    if report["drift"]:
        print(f"Workload drift: {', '.join(report['drift'])}")
    for result in report["owned"]:
        verdict = result["verdict"] or f"no verdict ({result['note']})"
        delta = bench_report.signed(result["delta"])
        pct = bench_report.signed(result["delta_pct"], 1, "%")
        print(f"  {result['metric']} {result['stat']}: {bench_report.fmt(result['base'])} -> {bench_report.fmt(result['cand'])} ms ({delta}, {pct}): {verdict}")
    print(f"Owned verdicts: {bench_report.summary_text(report['owned_summary'])}")


def cmd_compare(args, root=REPO_ROOT):
    base_dir, base_kind = resolve_ref(args.baseline, root)
    cand_dir, cand_kind = resolve_ref(args.candidate, root)
    if base_kind != cand_kind:
        raise BenchError(f"can't compare a {base_kind} ({args.baseline}) with a {cand_kind} ({args.candidate})")
    write = write_suite_compare if base_kind == "suite" else write_compare
    report = write(base_dir, cand_dir, root=root, out=args.out, force=args.force)
    if args.fail_on == "regressed" and bench_report.any_regressed(report):
        return EXIT_REGRESSED
    return 0


# --- Suites ----------------------------------------------------------------

SUITE_FRAMES = 300


def parse_only(text):
    """`--only a,b` -> ["a", "b"], in order and without repeats."""
    names = []
    for name in (part.strip() for part in text.split(",")):
        if name and name not in names:
            names.append(name)
    if not names:
        raise BenchError("--only expects comma-separated row names")
    return names


def suite_plan(schema, preset=None, only=None):
    """`(bench, row, preset)` for every tracked row `describe` lists, in its
    order, narrowed to `only`. `preset` replaces the preset of rows whose own
    preset is `default`; a row another preset defines (`physics-sparse`,
    `gpu-throughput`) keeps its own, because replacing it would measure a
    different row."""
    listed = [row["name"] for bench in schema["benches"] for row in bench["rows"]]
    unknown = [name for name in only or () if name not in listed]
    if unknown:
        raise BenchError(f"--only: unknown rows {', '.join(unknown)}; rows: {', '.join(listed)}")
    plan = []
    for bench in schema["benches"]:
        presets = [entry["name"] for entry in bench["presets"]]
        for row in bench["rows"]:
            if only is not None and row["name"] not in only:
                continue
            chosen = preset if preset is not None and row["preset"] == "default" else row["preset"]
            if chosen not in presets:
                raise BenchError(f"--preset {chosen}: row {row['name']} ({bench['name']}) has presets {', '.join(presets)}")
            plan.append((bench, row, chosen))
    if not plan:
        raise BenchError("the suite selects no rows")
    return plan


def suite_dir_name(stamp, preset, scale):
    """`<UTC>-suite[-<preset>][-s<scale>]`."""
    parts = [stamp, "suite"]
    if preset is not None:
        parts.append(preset)
    if scale is not None and float(scale) != 1.0:
        parts.append(f"s{float(scale):g}")
    return "-".join(parts)


def cmd_suite(args, root=REPO_ROOT):
    """One capture per tracked row under `perf-runs/<UTC>-suite/<row>/`, then
    `suite.json` and a suite README, and optionally a suite compare."""
    only = parse_only(args.only) if args.only else None
    baseline = None
    if args.compare:
        baseline, kind = resolve_ref(args.compare, root)
        if kind != "suite":
            raise BenchError(f"'{args.compare}' is a single capture; `suite --compare` needs a suite")
    rustflags = perf_rustflags()
    provenance = collect_provenance(root)
    binary = build(root, rustflags)
    plan = []
    for bench, row, preset in suite_plan(load_schema(binary, root), args.preset, only):
        request_env = bench_vars(bench["name"], preset, args.scale)
        config = resolve_config(binary, root, request_env)
        if config["row"] != row["name"]:
            raise BenchError(f"preset {preset} makes row {row['name']} measure {config['row']}")
        plan.append((bench, row, preset, request_env, config))

    out_dir = new_run_dir(root, suite_dir_name(utc_stamp(), args.preset, args.scale))
    print(f"Suite directory: {out_dir.relative_to(root)} ({len(plan)} rows)", flush=True)
    entries = []
    for bench, row, preset, request_env, config in plan:
        row_dir = out_dir / row["name"]
        row_dir.mkdir()
        print(f"Suite row {row['name']} (preset {preset}): {row_dir.relative_to(root)}", flush=True)
        request = {
            "bench": bench["name"],
            "preset": preset,
            "scale": args.scale,
            "set": [],
            "frames": SUITE_FRAMES,
            "warmup": None,
            "repeat": args.repeat,
            "gpu_timing": bench["gpu_timing"],
            "present_mode": None,
            "max_frame_latency": None,
            "profile": False,
        }
        capture = capture_to(
            row_dir,
            binary=binary,
            root=root,
            bench=bench,
            config=config,
            request_env=request_env,
            request=request,
            provenance=provenance,
            rustflags=rustflags,
        )
        print_summary(capture, row_dir.relative_to(root))
        entries.append((row["name"], capture))
    suite = bench_report.assemble_suite(
        request={"preset": args.preset, "scale": args.scale, "only": only, "repeat": args.repeat, "frames": SUITE_FRAMES},
        build={"package": PACKAGE, "profile": "release", "rustflags": rustflags},
        provenance=provenance,
        entries=entries,
    )
    (out_dir / "suite.json").write_text(json.dumps(suite, indent=2) + "\n")
    (out_dir / "README.md").write_text(bench_report.suite_readme(suite))
    print("Suite:")
    for row in suite["rows"]:
        total = "/".join(bench_report.fmt(row["total"][stat]) for stat in ("p50", "p95", "p99"))
        state = "valid" if row["valid"] else "INVALID"
        print(
            f"  {row['row']}: {state}; total p50/p95/p99 {total} ms, jitter {bench_report.fmt(row['total']['jitter'])} ms; "
            f"peak RSS {bench_report.fmt(row['peak_rss_mib'], 1)} MiB"
        )
    print(f"Suite {'valid' if suite['valid'] else 'INVALID'}: {out_dir.relative_to(root)}/suite.json and README.md")
    if baseline is not None:
        write_suite_compare(baseline, out_dir, root=root)
    return 0 if suite["valid"] else EXIT_INVALID


# --- Capacity search -------------------------------------------------------


def capacity_search(probe, *, start, lower, upper, budget, tolerance, key=lambda value: value,
                    confirm_runs=CONFIRM_RUNS, max_step_downs=MAX_STEP_DOWNS):  # fmt: skip
    """Largest axis value whose probe statistic stays within `budget`.

    `probe(value, phase)` runs one probe and returns a record whose `stat` is
    the measured statistic in ms, or None when the child failed (a fail).
    `key(value)` names the resolved workload, so values that round to the same
    integer knobs count as one point. Bracket from `start` by doubling on a
    pass or halving on a fail within [lower, upper], bisect geometrically
    until the pass/fail ratio is at most 1 + tolerance, then rerun the best
    pass `confirm_runs` times: it holds when their median is within budget,
    otherwise step down one tolerance step, at most `max_step_downs` times.
    """
    records = []

    def run(value, phase):
        record = dict(probe(value, phase))
        record.update(value=value, phase=phase, passed=record["stat"] is not None and record["stat"] <= budget)
        records.append(record)
        return record["passed"]

    def outcome(status, value, evidence, bracket):
        return {"status": status, "value": value, "evidence": evidence, "bracket": bracket, "probes": records}

    good = bad = None
    value = min(max(start, lower), upper)
    if run(value, "bracket"):
        good = value
        while good < upper:
            step = min(good * 2, upper)
            if not run(step, "bracket"):
                bad = step
                break
            good = step
    else:
        bad = value
        while bad > lower:
            step = max(bad / 2, lower)
            if run(step, "bracket"):
                good = step
                break
            bad = step
        if good is None:
            return outcome("unreachable", lower, [records[-1]], [None, bad])

    while bad is not None and bad / good > 1 + tolerance:
        middle = math.sqrt(good * bad)
        if key(middle) in (key(good), key(bad)):
            break
        if run(middle, "bisect"):
            good = middle
        else:
            bad = middle

    candidate = good
    for step_down in range(max_step_downs + 1):
        first = len(records)
        for _ in range(confirm_runs):
            run(candidate, "confirm")
        batch = records[first:]
        median = statistics.median(math.inf if record["stat"] is None else record["stat"] for record in batch)
        if median <= budget:
            status = "bound" if bad is None and candidate == upper else "max"
            return outcome(status, candidate, batch, [good, bad])
        if step_down == max_step_downs or candidate <= lower:
            break
        lowered = candidate
        while lowered > lower:
            lowered = max(lowered / (1 + tolerance), lower)
            if key(lowered) != key(candidate):
                break
        candidate = lowered
    return outcome("unreachable" if candidate <= lower else "unconfirmed", candidate, batch, [good, bad])


def parse_budget(text):
    """`60hz`, `144hz` or milliseconds (`12`, `12.5ms`) -> (label, ms)."""
    key = text.strip().lower()
    if key in BUDGETS_MS:
        return key, BUDGETS_MS[key]
    try:
        ms = float(key.removesuffix("ms"))
    except ValueError:
        ms = math.nan
    if not (math.isfinite(ms) and ms > 0):
        raise argparse.ArgumentTypeError(f"expected 60hz, 144hz or a positive millisecond value, got '{text}'")
    return f"{ms:g}ms", ms


def positive_fraction(text):
    value = float(text)
    if not 0 < value < 1:
        raise argparse.ArgumentTypeError(f"expected a value in (0, 1), got {text}")
    return value


def axis_knob(bench, axis):
    for knob in bench["knobs"]:
        if knob["name"] == axis:
            if knob["type"] not in ("int", "float"):
                raise BenchError(f"--axis {axis}: {bench['name']} knob '{axis}' is a {knob['type']}, not a number")
            return knob
    raise BenchError(f"--axis {axis}: {bench['name']} has no knob '{axis}'")


def capacity_row(args, *, root, binary, bench, row, budget_ms, out_dir):
    """Search one tracked row at its preset; returns its capacity.json entry."""
    axis = args.axis
    warmup = bench["warmup"]
    knob = None if axis == "scale" else axis_knob(bench, axis)
    resolved = {}

    def text(value):
        if knob is not None and knob["type"] == "int":
            return str(round(value))
        return f"{value:.6g}"

    def request_env(value):
        if knob is None:
            return bench_vars(bench["name"], row["preset"], text(value))
        return bench_vars(bench["name"], row["preset"], None, [f"{axis}={text(value)}"])

    def config(value):
        if text(value) not in resolved:
            resolved[text(value)] = resolve_config(binary, root, request_env(value))
        return resolved[text(value)]

    base = resolve_config(binary, root, bench_vars(bench["name"], row["preset"]))["knobs"]
    missing = [name for name in row["key_knobs"] if name not in base]
    if missing:
        raise BenchError(f"{row['name']}: key knobs {', '.join(missing)} are not knobs of {bench['name']}")
    if knob is None:
        start = 1.0
        scaled = [entry for entry in bench["knobs"] if entry["scales"] and entry["name"] in row["key_knobs"]]
        if not scaled:
            raise BenchError(f"{row['name']}: no scaled key knob bounds the scale axis")
        # A key knob whose range starts at 0 bounds the axis at its first
        # nonzero value, so halving toward an unreachable budget stops.
        lower = max(max(entry["min"], 1 if entry["type"] == "int" else base[entry["name"]] / 1024) / base[entry["name"]] for entry in scaled)
        upper = min(entry["max"] / base[entry["name"]] for entry in scaled)

        def key(value):
            return tuple(config(value)["knobs"][name] for name in row["key_knobs"])
    else:
        start = float(base[axis])
        if start <= 0:
            raise BenchError(f"--axis {axis}: {row['name']} resolves it to {start:g}; a geometric search needs a positive start")
        lower = max(float(knob["min"]), 1.0 if knob["type"] == "int" else start / 1024)
        upper = float(knob["max"])

        def key(value):
            return config(value)["knobs"][axis]

    row_dir = out_dir / "probes" / row["name"]
    probes = []

    def probe(value, phase):
        probe_dir = row_dir / f"{len(probes) + 1:02d}-{phase}-{path_label(text(value))}"
        env = capture_env(os.environ, request_env(value), warmup + args.frames)
        child = run_child([str(binary)], env, root, probe_dir / "telemetry.log")
        child.pop("rss_samples")
        analysis = bench_report.analyze_log(
            (probe_dir / "telemetry.log").read_text(errors="replace"), warmup, args.frames, row["guards"], config(value)
        )
        problems = bench_report.hard_problems(analysis)
        if child["exit_code"] != 0:
            problems.insert(0, f"exited with {child['exit_code']}")
        stats = analysis["stats"]
        record = {
            "value_text": text(value),
            "stat": None if problems else bench_report.metric_value(stats, "stage.total", args.stat),
            "p95": bench_report.metric_value(stats, "stage.total", "p95"),
            "p99": bench_report.metric_value(stats, "stage.total", "p99"),
            "candidates": bench_report.limiting_candidates(stats),
            "peak_rss_kib": child["rusage"]["peak_rss_kib"],
            "wall_s": child["wall_s"],
            "exit_code": child["exit_code"],
            "problems": problems,
            "guards": analysis["guards"],
            "dir": str(probe_dir.relative_to(out_dir)),
        }
        probes.append(record)
        verdict = "pass" if record["stat"] is not None and record["stat"] <= budget_ms else "fail"
        shown = "failed: " + "; ".join(problems) if problems else f"{args.stat} {record['stat']:.2f} ms"
        guards = "" if all(result["ok"] for result in analysis["guards"]) else " (guard failure recorded)"
        print(f"  {row['name']} {phase} {axis}={text(value)}: {shown}, {verdict}{guards}", flush=True)
        return record

    print(f"Capacity {row['name']}: {axis} in {text(lower)}..{text(upper)}, budget {budget_ms:g} ms", flush=True)
    started = time.monotonic()
    result = capacity_search(probe, start=start, lower=lower, upper=upper, budget=budget_ms, tolerance=args.tolerance, key=key)
    wall = time.monotonic() - started
    measured = [record for record in result["evidence"] if record["stat"] is not None] or result["evidence"]

    def median_of(field):
        return bench_report.median_of([record[field] for record in measured])

    peak = median_of("peak_rss_kib")
    return {
        "row": row["name"],
        "bench": bench["name"],
        "preset": row["preset"],
        "axis": axis,
        "bottleneck": row["bottleneck"],
        "status": result["status"],
        "value": result["value"],
        "value_text": text(result["value"]),
        "start_text": text(start),
        "lower_text": text(lower),
        "upper_text": text(upper),
        "bracket": result["bracket"],
        "key_counts": {name: config(result["value"])["knobs"][name] for name in row["key_knobs"]},
        "p95": median_of("p95"),
        "p99": median_of("p99"),
        "limiting": bench_report.limiting_stage([record["candidates"] for record in measured], row["bottleneck"]),
        "peak_rss_mib": None if peak is None else peak / 1024,
        "wall_s": wall,
        "probes": result["probes"],
    }


def cmd_capacity(args, root=REPO_ROOT):
    if bool(args.all) == bool(args.benches):
        raise BenchError("name one or more benchmarks, or pass --all")
    budget_label, budget_ms = args.budget
    rustflags = perf_rustflags()
    provenance = collect_provenance(root)
    binary = build(root, rustflags)
    schema = load_schema(binary, root)
    benches = schema["benches"] if args.all else [find_bench(schema, name) for name in args.benches]
    if args.axis != "scale":
        for bench in benches:
            axis_knob(bench, args.axis)
    out_dir = new_run_dir(root, f"{utc_stamp()}-capacity-{budget_label}")
    print(f"Output directory: {out_dir.relative_to(root)}", flush=True)
    rows = [
        capacity_row(args, root=root, binary=binary, bench=bench, row=row, budget_ms=budget_ms, out_dir=out_dir)
        for bench in benches
        for row in bench["rows"]
    ]
    report = {
        "schema": bench_report.SCHEMA,
        "budget": {"label": budget_label, "ms": budget_ms},
        "stat": args.stat,
        "axis": args.axis,
        "tolerance": args.tolerance,
        "frames": args.frames,
        "build": {"package": PACKAGE, "profile": "release", "rustflags": rustflags},
        "provenance": provenance,
        "rows": rows,
    }
    (out_dir / "capacity.json").write_text(json.dumps(report, indent=2) + "\n")
    (out_dir / "capacity.md").write_text(bench_report.capacity_markdown(report))
    (out_dir / "capacity.html").write_text(bench_report.capacity_html(report))
    for row in rows:
        counts = ", ".join(f"{name}={value}" for name, value in row["key_counts"].items())
        print(
            f"  {row['row']}: max {args.axis} {bench_report.capacity_result_text(row)} ({counts}); "
            f"p95 {bench_report.fmt(row['p95'])} ms, p99 {bench_report.fmt(row['p99'])} ms; "
            f"limiting {bench_report.limiting_text(row['limiting'])}; peak RSS {bench_report.fmt(row['peak_rss_mib'], 1)} MiB; "
            f"{len(row['probes'])} probes in {row['wall_s']:.0f} s"
        )
    print(f"Capacity report: {out_dir.relative_to(root)}/capacity.md and capacity.html")
    return 0


# --- Describe --------------------------------------------------------------


def knob_range(knob):
    if knob["type"] == "int":
        return f"{knob['min']}..{knob['max']}"
    if knob["type"] == "float":
        return f"{knob['min']:g}..{knob['max']:g}"
    if knob["type"] == "choice":
        return "|".join(knob["choices"])
    return "u64"


def describe_text(bench):
    gpu = "on" if bench["gpu_timing"] else "off"
    lines = [f"{bench['name']} (workload v{bench['workload_version']}, warm-up {bench['warmup']} frames, GPU diagnostic run {gpu})"]
    lines.append("  rows:")
    for row in bench["rows"]:
        owned = ", ".join(f"{owned['metric']} {'/'.join(owned['stats'])}" for owned in row["owned"])
        guards = ", ".join(bench_report.guard_name(guard) for guard in row["guards"]) or "none"
        lines.append(f"    {row['name']} (preset {row['preset']}): judged on {owned}; guards: {guards}")
        if row.get("note"):
            lines.append(f"      note: {row['note']}")
    presets = []
    for preset in bench["presets"]:
        values = ", ".join(f"{name}={value}" for name, value in preset["set"].items())
        presets.append(f"{preset['name']} ({values})" if values else preset["name"])
    lines.append(f"  presets: {', '.join(presets)}")
    lines.append("  knobs:")
    width = max(len(knob["name"]) for knob in bench["knobs"])
    for knob in bench["knobs"]:
        scales = "scales" if knob["scales"] else ""
        note = f"  {knob['note']}" if knob["note"] else ""
        lines.append(
            f"    {knob['name']:<{width}}  {knob['type']:<6}  {str(knob['default']):<9}  {knob_range(knob):<22}  {scales:<6}{note}".rstrip()
        )
    return "\n".join(lines)


def cmd_describe(args):
    binary = build(REPO_ROOT, perf_rustflags())
    schema = load_schema(binary, REPO_ROOT)
    benches = [find_bench(schema, args.bench)] if args.bench else schema["benches"]
    if args.json:
        print(json.dumps({"schema": schema["schema"], "benches": benches}, indent=2))
    else:
        print("\n\n".join(describe_text(bench) for bench in benches))
    return 0


# --- Baselines -------------------------------------------------------------


def baselines_dir(root=REPO_ROOT):
    return root / "perf-runs" / "baselines"


def baseline_path(name, root=REPO_ROOT):
    if not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9._-]*", name):
        raise BenchError(f"baseline names use letters, digits, '.', '_' and '-'; got '{name}'")
    return baselines_dir(root) / name


def cmd_baseline(args, root=REPO_ROOT):
    if args.action == "list":
        found = sorted(baselines_dir(root).glob("*/baseline.json")) if baselines_dir(root).is_dir() else []
        if not found:
            print("No baselines.")
        for path in found:
            meta = json.loads(path.read_text())
            valid = "valid" if meta.get("valid") else "INVALID"
            what = f"suite ({len(meta['rows'])} rows)" if meta.get("kind") == "suite" else meta.get("row", "?")
            print(f"{meta['name']}\t{what}\t{meta['date']}\t{valid}\t{meta['source']}")
        return 0
    dest = baseline_path(args.name, root)
    if args.action == "rm":
        if not (dest / "baseline.json").is_file():
            raise BenchError(f"no baseline named '{args.name}'")
        shutil.rmtree(dest)
        print(f"Removed baseline {args.name}")
        return 0
    source = Path(args.capture).resolve()
    kind = ref_kind(source)
    if kind is None:
        raise BenchError(f"{args.capture} has no capture.json or suite.json; only bench.py captures and suites can be baselines")
    if dest.exists():
        raise BenchError(f"baseline '{args.name}' exists; remove it first")
    saved = json.loads((source / f"{kind}.json").read_text())
    shutil.copytree(source, dest, ignore=shutil.ignore_patterns("*.data"))
    meta = {
        "name": args.name,
        "kind": kind,
        "source": os.path.relpath(source, root),
        "date": datetime.datetime.now(datetime.UTC).isoformat(timespec="seconds"),
        "machine": saved["provenance"]["machine"],
        "valid": saved["valid"],
    }
    if kind == "suite":
        meta["rows"] = [row["row"] for row in saved["rows"]]
    else:
        meta["row"] = saved["row"]
    (dest / "baseline.json").write_text(json.dumps(meta, indent=2) + "\n")
    note = "" if saved["valid"] else f" (the {kind} is invalid; compare will refuse verdicts)"
    print(f"Saved baseline {args.name} from {meta['source']}{note}")
    return 0


# --- CLI -------------------------------------------------------------------


def positive_int(text):
    value = int(text)
    if value < 1:
        raise argparse.ArgumentTypeError(f"expected a positive integer, got {text}")
    return value


def non_negative_int(text):
    value = int(text)
    if value < 0:
        raise argparse.ArgumentTypeError(f"expected a non-negative integer, got {text}")
    return value


def parser():
    top = argparse.ArgumentParser(prog="bench.py", description=__doc__.splitlines()[0])
    commands = top.add_subparsers(dest="command", required=True)

    describe = commands.add_parser("describe", help="print knobs, presets and tracked rows")
    describe.add_argument("bench", nargs="?")
    describe.add_argument("--json", action="store_true", help="raw schema JSON")

    run = commands.add_parser("run", help="capture one benchmark")
    run.add_argument("bench")
    run.add_argument("--preset")
    run.add_argument("--scale")
    run.add_argument("--set", action="append", default=[], metavar="KNOB=VALUE")
    run.add_argument("--frames", type=positive_int, default=300)
    run.add_argument("--warmup", type=non_negative_int, help="default: the benchmark's warm-up")
    run.add_argument("--repeat", type=positive_int, default=1)
    run.add_argument("--gpu-timing", choices=("on", "off"), help="default: the benchmark's setting")
    run.add_argument("--present-mode")
    run.add_argument("--max-frame-latency", type=positive_int)
    run.add_argument("--profile", action="store_true", help="perf stat, perf record and a flamegraph of run 1's config")
    run.add_argument("--call-graph", choices=("dwarf", "fp"), default="dwarf")
    run.add_argument("--sample-frequency", type=positive_int, metavar="HZ")
    run.add_argument("--sweep", metavar="KNOB=V1,V2,...", help="one capture per value plus sweep.md and sweep.html")
    run.add_argument("--compare", metavar="BASELINE", help="compare the new capture against a baseline name or capture directory")

    suite = commands.add_parser("suite", help="capture every tracked row")
    suite.add_argument("--preset", help="replaces the preset of rows whose own preset is `default`; other rows keep theirs")
    suite.add_argument("--scale")
    suite.add_argument("--only", metavar="ROWS", help="comma-separated row names")
    suite.add_argument("--repeat", type=positive_int, default=1)
    suite.add_argument("--compare", metavar="BASELINE", help="compare the suite against a baseline name or suite directory")

    compare = commands.add_parser("compare", help="compare two captures or two suites")
    compare.add_argument("baseline", help="baseline name, capture directory or suite directory")
    compare.add_argument("candidate", help="baseline name, capture directory or suite directory")
    compare.add_argument("--out", metavar="DIR", help="default: perf-runs/<UTC>-compare-<row> or -compare-suite")
    compare.add_argument("--fail-on", choices=("regressed",), help="exit 1 when an owned metric regressed")
    compare.add_argument("--force", action="store_true", help="judge even when a capture is invalid")

    capacity = commands.add_parser("capacity", help="largest scale within a frame-time budget")
    capacity.add_argument("benches", nargs="*", metavar="bench")
    capacity.add_argument("--all", action="store_true", help="every benchmark the binary describes")
    capacity.add_argument("--budget", type=parse_budget, default=parse_budget("60hz"), help="60hz, 144hz or milliseconds")
    capacity.add_argument("--stat", choices=("mean", "p50", "p95", "p99", "max"), default="p95", help="statistic of stage.total")
    capacity.add_argument("--axis", default="scale", help="scale, or one numeric knob")
    capacity.add_argument("--tolerance", type=positive_fraction, default=0.05)
    capacity.add_argument("--frames", type=positive_int, default=180, help="measured frames per probe")

    baseline = commands.add_parser("baseline", help="manage named baselines")
    actions = baseline.add_subparsers(dest="action", required=True)
    save = actions.add_parser("save")
    save.add_argument("capture", help="capture or suite directory")
    save.add_argument("name")
    actions.add_parser("list")
    remove = actions.add_parser("rm")
    remove.add_argument("name")
    return top


def main(argv=None):
    argv = sys.argv[1:] if argv is None else argv
    args = parser().parse_args(argv)
    try:
        if args.command == "run":
            return cmd_run(args)
        if args.command == "describe":
            return cmd_describe(args)
        if args.command == "suite":
            return cmd_suite(args)
        if args.command == "compare":
            return cmd_compare(args)
        if args.command == "capacity":
            return cmd_capacity(args)
        return cmd_baseline(args)
    except BenchError as error:
        print(f"error: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main())
