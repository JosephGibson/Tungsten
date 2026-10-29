#!/usr/bin/env python3
"""Benchmark runner (redo 7.3, `D-078`; docs/perf/profiling-workflow.md).

  list [--json]                   the rows `scripts/perf-rows.json` holds
  run <row|group> [flags]         one capture per row
  compare <baseline> <candidate>  verdicts on the candidate's owned metrics
  sitting [flags]                 the working tree against a ref, A1 B1 A2 B2 per row
  counters [--write]              the GPU counter pins, checked or written
  baseline save|list|rm           named baselines under perf-runs/baselines/

A capture is `perf-runs/<UTC>-<row>/capture.json` at schema 2. Builds are
release builds into `target/perf/`, every function and non-fallthrough block
aligned to 64 bytes unless `--plain`. Exit status 2 is a refusal or an
error, 3 a capture written but invalid. Standard library only, Python 3.12.
"""

import argparse
import datetime
import hashlib
import json
import os
import re
import shutil
import socket
import subprocess
import sys
import time
from pathlib import Path

sys.dont_write_bytecode = True  # importing perf_stats must not leave scripts/__pycache__
import perf_stats  # noqa: E402

REPO_ROOT = Path(__file__).resolve().parent.parent
REGISTRY = Path(__file__).resolve().parent / "perf-rows.json"
# RUSTFLAGS replaces .cargo/config.toml's target-cpu=native: captures are
# generic x86-64 with frame pointers, aligned against code-placement noise.
PLAIN_FLAGS = "-C force-frame-pointers=yes"
ALIGNED_FLAGS = PLAIN_FLAGS + " -C llvm-args=-align-all-functions=6 -C llvm-args=-align-all-nofallthru-blocks=6"
RUST_LOG = "tungsten::app=debug,bench=debug"
# Inherited variables that would change what a child measures.
CLEARED_PREFIXES = ("TUNGSTEN_BENCH", "TUNGSTEN_RENDER_", "TUNGSTEN_DISPLAY_", "TUNGSTEN_CAPTURE_")
CLEARED_NAMES = (
    "TUNGSTEN_GPU_TIMING", "TUNGSTEN_OVERLAYS_ON", "TUNGSTEN_SMOKE_FRAMES", "TUNGSTEN_PERF_LOG", "RUST_LOG",
    "TUNGSTEN_SCRIPT", "TUNGSTEN_REPORT", "TUNGSTEN_USER_DIR", "TUNGSTEN_CHECK", "TUNGSTEN_DESCRIBE",
)  # fmt: skip
RSS_POLL_SECONDS = 0.1
# Background load (`D-095`, `D-141`): the remote-desktop encoder anywhere, and
# builds outside the runner's own process tree.
ENCODER = "nxcodec.bin"
BUILD_NAMES = ("cargo", "rustc")
BACKGROUND_SCAN_SECONDS = 1.0
# Frames a GPU run takes past its window, so the window's last timestamps
# land before the process exits: the renderer's ring depth (4) plus one.
GPU_TAIL = 5
EXIT_REGRESSED = 1
EXIT_ERROR = 2
EXIT_INVALID = 3
# A sitting's start check (`D-141` as 7.3 extends it): a process other than
# the runner and its ancestors above this share of a core over the sample is
# busy. `--wait-quiet` polls every QUIET_POLL_S until QUIET_SPAN_S run quiet.
QUIET_SAMPLE_S = 5
BUSY_SHARE = 0.20
QUIET_POLL_S = 10
QUIET_SPAN_S = 60
# Quick mode: CPU rows, A1 B1 A2, no rerun; informational.
QUICK_REPEAT = 3
QUICK_FRAMES = 150
# The GPU counter check: these rows, each at its warm-up plus 30 frames.
COUNTER_ROWS = ("gpu", "gpu-throughput", "template", "e01-area-clearing")
COUNTER_FRAMES = 30
COUNTER_FIELDS = ("draw_calls", "upload_bytes", "text_renders", "sprite_batches")


class PerfError(Exception):
    """A request or environment problem reported without a traceback."""


# --- Rows --------------------------------------------------------------------


def load_registry(path=REGISTRY):
    return json.loads(Path(path).read_text())


def select_rows(registry, name):
    """The row named `name`, or every row of the group `name`."""
    rows = [row for row in registry["rows"] if row["name"] == name or row.get("group") == name]
    if not rows:
        names = sorted({row["name"] for row in registry["rows"]} | {row["group"] for row in registry["rows"] if row.get("group")})
        raise PerfError(f"unknown row or group '{name}'; rows: {', '.join(names)}")
    return rows


def script_sha256(row):
    """A scripted row's script, pinned by the hash of its canonical JSON."""
    script = row["launch"].get("script")
    if script is None:
        return None
    return hashlib.sha256(json.dumps(script, sort_keys=True, separators=(",", ":")).encode()).hexdigest()


def compose_script(row, warmup, frames, tail):
    """The script a scripted row runs: its setup steps, `until: "end"` held
    through the last frame, and its asserts on the window's last frame."""
    script = row["launch"]["script"]
    total = warmup + frames + tail
    steps = []
    for step in script["steps"]:
        step = dict(step)
        if step.get("until") == "end":
            step["until"] = total
        steps.append(step)
    steps += [{"frame": warmup + frames, "assert": dict(entry)} for entry in script.get("asserts", [])]
    return {"version": 1, "frames": total, "resolution": row["resolution"], "steps": steps}


# --- Provenance ----------------------------------------------------------------


def git_output(root, *args):
    return subprocess.run(["git", "-C", str(root), *args], capture_output=True, check=True).stdout


def git_commit(root, ref="HEAD"):
    try:
        return git_output(root, "rev-parse", "--short", f"{ref}^{{commit}}").decode().strip() or "unknown"
    except (OSError, subprocess.CalledProcessError):
        return "unknown"


def git_dirty(root):
    """`no` for a clean tree, else `yes (diff <12 hex>)` over the tracked diff
    and the untracked files' hashes, so captures of one dirty tree match."""
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


def tree_state(root, ref=None, repo=REPO_ROOT):
    """The measured tree: the working tree's own commit and dirty state, or
    for an export of `repo` the ref's commit, which an export cannot change."""
    if ref is not None:
        return {"commit": git_commit(repo, ref), "dirty": "no", "ref": ref}
    return {"commit": git_commit(root), "dirty": git_dirty(root), "ref": None}


def file_value_or_na(path):
    try:
        with open(path, encoding="utf-8", errors="replace") as handle:
            value = handle.readline().strip()
    except OSError:
        return "n/a"
    return value or "n/a"


def read_text(path):
    try:
        return Path(path).read_text(encoding="utf-8", errors="replace")
    except OSError:
        return ""


def ac_power(power_supply=Path("/sys/class/power_supply")):
    try:
        supplies = sorted(Path(power_supply).iterdir())
    except OSError:
        return "n/a"
    for supply in supplies:
        if file_value_or_na(supply / "type") == "Mains":
            return {"1": "online", "0": "offline"}.get(file_value_or_na(supply / "online"), "n/a")
    return "n/a"


def machine():
    """The host: a fingerprint of its name, CPU and memory, which compare
    holds equal, and the state a reader weighs (governor, power, kernel)."""
    cpu = next(
        (value.strip() for key, _, value in (line.partition(":") for line in read_text("/proc/cpuinfo").splitlines()) if key.strip() == "model name"),
        "unknown",
    )
    match = re.search(r"^MemTotal:\s+(\d+) kB", read_text("/proc/meminfo"), re.MULTILINE)
    mem_kib = int(match[1]) if match else None
    hostname = socket.gethostname()
    uname = os.uname()
    try:
        rustc = subprocess.run(["rustc", "--version"], capture_output=True, text=True, check=True).stdout.strip()
    except (OSError, subprocess.CalledProcessError):
        rustc = "unknown"
    return {
        "fingerprint": hashlib.sha256(f"{hostname}\n{cpu}\n{mem_kib}".encode()).hexdigest()[:12],
        "hostname": hostname,
        "cpu": cpu,
        "mem_total_kib": mem_kib,
        "kernel": f"{uname.sysname} {uname.release}",
        "governor": file_value_or_na("/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor"),
        "platform_profile": file_value_or_na("/sys/firmware/acpi/platform_profile"),
        "ac_power": ac_power(),
        "rustc": rustc,
        "wgpu_backend_env": os.environ.get("WGPU_BACKEND", "auto"),
    }


# --- Background load -------------------------------------------------------------


def list_processes(proc="/proc"):
    """`(pid, ppid, name)` of every process, from `/proc/<pid>/stat`."""
    processes = []
    try:
        entries = list(os.scandir(proc))
    except OSError:
        return processes
    for entry in entries:
        if not entry.name.isdigit():
            continue
        try:
            with open(os.path.join(entry.path, "stat"), encoding="utf-8", errors="replace") as handle:
                text = handle.read()
            close = text.rindex(")")
            processes.append((int(entry.name), int(text[close + 2 :].split()[1]), text[text.index("(") + 1 : close]))
        except (OSError, ValueError, IndexError):
            continue  # exited during the scan
    return processes


def own_tree(processes, own_pid):
    """The pids in the process tree rooted at `own_pid`."""
    parents = {pid: ppid for pid, ppid, _ in processes}

    def own(pid):
        seen = set()
        while pid is not None and pid not in seen:
            if pid == own_pid:
                return True
            seen.add(pid)
            pid = parents.get(pid)
        return False

    return {pid for pid, _, _ in processes if own(pid)}


def background_offenders(processes, own_pid):
    """Sorted names loading the machine beside a measured run: the encoder
    anywhere, `cargo` or `rustc` outside the runner's own tree."""
    mine = own_tree(processes, own_pid)
    return sorted({name for pid, _, name in processes if name == ENCODER or (name in BUILD_NAMES and pid not in mine)})


def preflight_background(allow_background, lister=list_processes):
    offenders = background_offenders(lister(), os.getpid())
    if offenders and not allow_background:
        raise PerfError(
            f"background load: {', '.join(offenders)}; stop it before capturing or use --allow-background for a diagnostic capture"
        )


# --- Build and child runs ------------------------------------------------------------


def build(tree, package, flags, target_dir):
    """A release build of `package` in `tree` into `target_dir`; its binary."""
    print(f'Building {package} with RUSTFLAGS="{flags}" into {target_dir}...', flush=True)
    env = dict(os.environ, RUSTFLAGS=flags, CARGO_TARGET_DIR=str(target_dir))
    if subprocess.run(["cargo", "build", "--release", "--locked", "-p", package], cwd=tree, env=env).returncode != 0:
        raise PerfError(f"build of {package} failed")
    binary = Path(target_dir) / "release" / package
    if not binary.is_file():
        raise PerfError(f"built binary not found at {binary}")
    return binary


def child_env(parent, extra=None):
    """`parent` without the inherited variables that change a measurement,
    plus `extra`."""
    env = {name: value for name, value in parent.items() if not name.startswith(CLEARED_PREFIXES) and name not in CLEARED_NAMES}
    env.update(extra or {})
    return env


def read_rss_kib(pid, page_kib):
    try:
        with open(f"/proc/{pid}/statm", encoding="ascii") as handle:
            return int(handle.read().split()[1]) * page_kib
    except (OSError, IndexError, ValueError):
        return None


def run_child(argv, env, cwd, log_path, watch=None):
    """Run `argv` with its output in `log_path`, reaped by `os.wait4` for its
    rusage; `watch`, the background scan, runs before it, once a second
    while it runs and after it exits."""
    log_path = Path(log_path)
    log_path.parent.mkdir(parents=True, exist_ok=True)
    if watch is not None:
        watch()
    start = time.monotonic()
    next_scan = start + BACKGROUND_SCAN_SECONDS
    with open(log_path, "wb") as log:
        proc = subprocess.Popen(argv, env=env, cwd=cwd, stdout=log, stderr=subprocess.STDOUT)
        try:
            while True:
                pid, status, usage = os.wait4(proc.pid, os.WNOHANG)
                if pid == proc.pid:
                    break
                if watch is not None and time.monotonic() >= next_scan:
                    watch()
                    next_scan += BACKGROUND_SCAN_SECONDS
                time.sleep(RSS_POLL_SECONDS)
        except BaseException:
            proc.kill()
            proc.wait()
            raise
    proc.returncode = os.waitstatus_to_exitcode(status)
    if watch is not None:
        watch()
    return {"exit_code": proc.returncode, "wall_s": round(time.monotonic() - start, 3), "rss_peak_kib": usage.ru_maxrss}


# --- Captures ------------------------------------------------------------------------


def utc_stamp():
    return datetime.datetime.now(datetime.UTC).strftime("%Y%m%dT%H%M%SZ")


def new_dir(parent, name):
    """`parent/name`, or `name-2`, `-3`... when taken; created."""
    parent = Path(parent)
    candidate, suffix = parent / name, 2
    while candidate.exists():
        candidate, suffix = parent / f"{name}-{suffix}", suffix + 1
    candidate.mkdir(parents=True)
    return candidate


def side_env(row, *, warmup, frames, gpu, run_dir):
    """A run's variables: perf lines on, and the bench's selection and frame
    count, or the scripted row's script, report and fresh user folder."""
    extra = {"TUNGSTEN_PERF_LOG": "1", "RUST_LOG": RUST_LOG}
    tail = GPU_TAIL if gpu else 0
    if gpu:
        extra["TUNGSTEN_GPU_TIMING"] = "1"
    launch = row["launch"]
    if "bench" in launch:
        extra["TUNGSTEN_BENCH"] = launch["bench"]
        extra["TUNGSTEN_BENCH_PRESET"] = launch["preset"]
        if launch.get("scale") is not None:
            extra["TUNGSTEN_BENCH_SCALE"] = str(launch["scale"])
        extra["TUNGSTEN_SMOKE_FRAMES"] = str(warmup + frames + tail)
    else:
        label = "gpu" if gpu else "timing"
        script = run_dir / f"{label}-script.json"
        script.write_text(json.dumps(compose_script(row, warmup, frames, tail), indent=2) + "\n")
        user = run_dir / f"{label}-user"
        user.mkdir(parents=True, exist_ok=True)
        extra.update(
            TUNGSTEN_SCRIPT=str(script), TUNGSTEN_REPORT=str(run_dir / f"{label}-report.json"), TUNGSTEN_USER_DIR=str(user)
        )
    return extra


def report_problems(run_dir, label):
    """A scripted run's report: missing, or failed with its first reasons."""
    path = run_dir / f"{label}-report.json"
    try:
        report = json.loads(path.read_text())
    except (OSError, ValueError):
        return ["no script report"]
    if report.get("passed"):
        return []
    failed = [entry for entry in report.get("asserts", []) if not entry.get("passed")]
    shown = "; ".join(f"{entry.get('probe')}{entry.get('path') or ''} {entry.get('op')} {entry.get('value')!r}: actual {entry.get('actual')!r}" for entry in failed[:3])
    return [f"script report failed{': ' + shown if shown else ''}"]


def bench_config_problems(log, row):
    if "bench" not in row["launch"]:
        return [], None
    if log.bench_config is None:
        return ["no bench-config line"], None
    try:
        config = json.loads(log.bench_config)
    except ValueError:
        return ["bench-config is not JSON"], None
    problems = []
    if config.get("preset") != row["launch"]["preset"]:
        problems.append(f"bench-config names preset {config.get('preset')}, not {row['launch']['preset']}")
    if config.get("row") != row["name"]:
        problems.append(f"bench-config names row {config.get('row')}, not {row['name']}")
    return problems, config


def one_run(row, binary, tree, run_dir, *, warmup, frames, gpu, lister, tree_reader, built, allow_background):
    """One measured child run and its analysis: its stats, digest, renderer,
    config and problems, background load included."""
    label = "gpu" if gpu else "timing"
    env = child_env(os.environ, side_env(row, warmup=warmup, frames=frames, gpu=gpu, run_dir=run_dir))
    seen = set()
    result = run_child(
        [str(binary)], env, Path(tree) / row["folder"], run_dir / f"{label}.log",
        watch=lambda: seen.update(background_offenders(lister(), os.getpid())),
    )  # fmt: skip
    shutil.rmtree(run_dir / f"{label}-user", ignore_errors=True)
    log_text = (run_dir / f"{label}.log").read_text(errors="replace")
    analysis = perf_stats.analyze(log_text, warmup, frames, row.get("guards", []), gpu=gpu)
    problems = []
    if result["exit_code"] != 0:
        problems.append(f"exited with {result['exit_code']}")
    problems += analysis["problems"]
    config_problems, config = bench_config_problems(analysis["log"], row)
    problems += config_problems
    if "script" in row["launch"]:
        problems += report_problems(run_dir, label)
    load = [f"background load: {name}" for name in sorted(seen)]
    after = tree_reader()
    if after != built:
        load.append("background load: tree changed")
    notes = []
    (notes if allow_background else problems).extend(load)
    measured = analysis["measured"]
    return {
        "label": label,
        "exit_code": result["exit_code"],
        "wall_s": result["wall_s"],
        "rss_peak_kib": result["rss_peak_kib"],
        "digest": analysis["digest"],
        "guards": analysis["guards"],
        "stats": perf_stats.gpu_stats(measured) if gpu else perf_stats.cpu_stats(measured),
        "backend": analysis["log"].backend,
        "config": config,
        "problems": problems,
        "notes": notes,
    }


def capture_row(row, binary, tree, out_dir, *, flags, provenance, machine_info, repeat=None, frames=None,
                sitting=None, allow_background=False, lister=list_processes, tree_reader=None, watched=None):  # fmt: skip
    """`repeat` runs of `row` (a timing run, a GPU run beside it or alone, as
    the row's `gpu` says), then `capture.json` and `README.md` in `out_dir`.
    `tree_reader` must keep returning `watched` (the working tree's commit and
    dirty state, by default the provenance's) or a run saw the tree change."""
    repeat = repeat or row["repeat"]
    frames = frames or row["frames"]
    warmup = row["warmup"]
    gpu = row.get("gpu", False)
    labels = {"only": [True], True: [False, True]}.get(gpu, [False])
    built = watched or {key: provenance[key] for key in ("commit", "dirty")}
    reader = tree_reader or (lambda: {key: value for key, value in tree_state(tree).items() if key in ("commit", "dirty")})
    runs, digests, notes = [], [], []
    backend = config = None
    for index in range(1, repeat + 1):
        run_dir = out_dir / f"run-{index}"
        run_dir.mkdir(parents=True, exist_ok=True)
        parts = {}
        for is_gpu in labels:
            print(f"{row['name']}: {'GPU' if is_gpu else 'timing'} run {index} of {repeat}...", flush=True)
            parts["gpu" if is_gpu else "cpu"] = one_run(
                row, binary, tree, run_dir, warmup=warmup, frames=frames, gpu=is_gpu, lister=lister,
                tree_reader=reader, built=built, allow_background=allow_background,
            )  # fmt: skip
        main = parts.get("cpu") or parts["gpu"]
        for part in parts.values():
            backend = backend or part["backend"]
            config = config or part["config"]
        reasons = [f"{part['label']}: {problem}" for part in parts.values() for problem in part["problems"]]
        notes += [f"run {index} {part['label']}: {note}" for part in parts.values() for note in part["notes"]]
        digests += [part["digest"] for part in parts.values()]
        runs.append(
            {
                "index": index,
                "valid": not reasons,
                "reasons": reasons,
                "digest": main["digest"],
                "rss_peak_kib": main["rss_peak_kib"],
                "cpu": parts["cpu"]["stats"] if "cpu" in parts else None,
                "gpu": parts["gpu"]["stats"] if "gpu" in parts else None,
                "guards": main["guards"],
                "exit_codes": {part["label"]: part["exit_code"] for part in parts.values()},
                "wall_s": {part["label"]: part["wall_s"] for part in parts.values()},
            }
        )
        if any(part["exit_code"] != 0 for part in parts.values()):
            print(f"Run {index} failed; skipping the rest.", flush=True)
            break
    reasons = []
    if len(runs) < repeat:
        reasons.append(f"{len(runs)} of {repeat} runs completed")
    reasons += [f"run {run['index']} {reason}" for run in runs for reason in run["reasons"]]
    if len(set(digests)) > 1:
        reasons.append(f"determinism: digests differ across runs ({', '.join(digests)})")
    capture = {
        "schema": perf_stats.SCHEMA,
        "kind": "capture",
        "row": row["name"],
        "sitting": sitting,
        "tree": {key: provenance[key] for key in ("commit", "dirty", "ref")},
        "build": {"flags": flags, "package": row["package"]},
        "workload": {
            "version": row["workload_version"],
            "config": config if "bench" in row["launch"] else None,
            "script_sha256": script_sha256(row),
            "resolution": row["resolution"],
            "warmup": warmup,
            "frames": frames,
            "tail": GPU_TAIL if gpu else 0,
        },
        "renderer": None if backend is None else {key: backend[key] for key in ("backend", "adapter", "present_mode", "max_frame_latency")},
        "machine": machine_info,
        "owned": row["owned"],
        "valid": not reasons,
        "reasons": reasons,
        "notes": notes,
        "runs": runs,
    }
    (out_dir / "capture.json").write_text(json.dumps(capture, indent=2) + "\n")
    (out_dir / "README.md").write_text(capture_readme(capture))
    return capture


def fmt(value, digits=3):
    return "n/a" if value is None else f"{value:.{digits}f}"


def capture_readme(capture):
    """A capture's summary: validity, provenance and each owned metric per run."""
    tree = capture["tree"]
    lines = [
        f"# {capture['row']}",
        "",
        f"- Valid: {'yes' if capture['valid'] else 'no'}",
        f"- Tree: {tree['commit']}, dirty {tree['dirty']}" + (f", ref {tree['ref']}" if tree.get("ref") else ""),
        f"- Build flags: `{capture['build']['flags']}`",
        f"- Workload: version {capture['workload']['version']}, {capture['workload']['warmup']} warm-up, {capture['workload']['frames']} frames",
        f"- Sitting: {capture.get('sitting') or 'none'}",
    ]
    lines += [f"- Invalid: {reason}" for reason in capture["reasons"]]
    lines += [f"- Note: {note}" for note in capture["notes"]]
    lines += ["", "| Metric | Stat | " + " | ".join(f"Run {run['index']}" for run in capture["runs"]) + " |"]
    lines.append("| --- | --- | " + " | ".join("---" for _ in capture["runs"]) + " |")
    for entry in capture["owned"]:
        for stat in entry["stats"]:
            prefix, _, name = entry["metric"].partition(".")
            block, group = perf_stats.METRIC_GROUPS.get(prefix, (None, None))
            cells = [fmt((((run.get(block) or {}).get(group) or {}).get(name) or {}).get(stat)) for run in capture["runs"]]
            lines.append(f"| `{entry['metric']}` | {stat} | " + " | ".join(cells) + " |")
    lines += ["", "Peak RSS (KiB): " + ", ".join(str(run["rss_peak_kib"]) for run in capture["runs"]), ""]
    return "\n".join(lines)


# --- Sittings ------------------------------------------------------------------------


def cpu_ticks(proc="/proc"):
    """`{pid: (ppid, name, utime + stime ticks)}` of every process."""
    table = {}
    try:
        entries = list(os.scandir(proc))
    except OSError:
        return table
    for entry in entries:
        if not entry.name.isdigit():
            continue
        try:
            with open(os.path.join(entry.path, "stat"), encoding="utf-8", errors="replace") as handle:
                text = handle.read()
            close = text.rindex(")")
            fields = text[close + 2 :].split()
            table[int(entry.name)] = (int(fields[1]), text[text.index("(") + 1 : close], int(fields[11]) + int(fields[12]))
        except (OSError, ValueError, IndexError):
            continue
    return table


def ancestors(table, pid):
    """`pid` and every process above it in `table`."""
    seen = set()
    while pid in table and pid not in seen:
        seen.add(pid)
        pid = table[pid][0]
    return seen


def load_1m():
    return read_text("/proc/loadavg").split(" ", 1)[0] or "n/a"


def start_check(*, own_pid, tick_reader=cpu_ticks, tree_reader, sleep=time.sleep, seconds=QUIET_SAMPLE_S,
                ticks_per_s=None):  # fmt: skip
    """A sitting's contamination check: the encoder or a build outside the
    runner, a process other than the runner and its ancestors above
    BUSY_SHARE of a core over `seconds`, or the tree's dirty state changing
    meanwhile, each a refusal. Returns `(refusals, record)`; the record holds
    the 1-minute load, the governor, AC power and the five busiest processes."""
    ticks_per_s = ticks_per_s or os.sysconf("SC_CLK_TCK")
    before, tree_before = tick_reader(), tree_reader()
    sleep(seconds)
    after, tree_after = tick_reader(), tree_reader()
    excluded = ancestors(after, own_pid)
    processes = [(pid, ppid, name) for pid, (ppid, name, _) in after.items()]
    refusals = [f"background load: {name}" for name in background_offenders(processes, own_pid)]
    shares = []
    for pid, (_, name, ticks) in after.items():
        if pid in excluded or pid not in before or before[pid][1] != name:
            continue
        share = (ticks - before[pid][2]) / (ticks_per_s * seconds)
        shares.append((share, name, pid))
    shares.sort(reverse=True)
    refusals += [f"busy process: {name} ({pid}) at {share:.0%} of a core" for share, name, pid in shares if share > BUSY_SHARE]
    if tree_after != tree_before:
        refusals.append("the tree changed during the check")
    record = {
        "load_1m": load_1m(),
        "governor": file_value_or_na("/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor"),
        "ac_power": ac_power(),
        "top": [{"name": name, "pid": pid, "share": round(share, 3)} for share, name, pid in shares[:5]],
    }
    return refusals, record


def wait_quiet(limit_s, check, *, sleep=time.sleep, clock=time.monotonic):
    """Runs `check` every QUIET_POLL_S until its runs have been quiet for
    QUIET_SPAN_S, or until `limit_s`; returns the last `(refusals, record)`,
    the refusals empty once quiet."""
    start = clock()
    quiet_since = None
    while True:
        refusals, record = check()
        now = clock()
        if refusals:
            quiet_since = None
        elif quiet_since is None:
            quiet_since = now
        if quiet_since is not None and now - quiet_since >= QUIET_SPAN_S:
            return [], record
        if now - start >= limit_s:
            return refusals or [f"no {QUIET_SPAN_S} s of quiet within {limit_s} s"], record
        print(f"Waiting for quiet: {'; '.join(refusals) or 'quiet so far'}", flush=True)
        sleep(QUIET_POLL_S)


def last_tag(repo=REPO_ROOT):
    try:
        return git_output(repo, "describe", "--tags", "--abbrev=0").decode().strip()
    except (OSError, subprocess.CalledProcessError) as error:
        raise PerfError("no tag reachable from HEAD; pass --base") from error


def export_tree(ref, dest, repo=REPO_ROOT):
    """`git archive ref` into `dest`, emptied first; the export's tree state
    records the ref's commit."""
    dest = Path(dest)
    shutil.rmtree(dest, ignore_errors=True)
    dest.mkdir(parents=True)
    archive = subprocess.Popen(["git", "-C", str(repo), "archive", ref], stdout=subprocess.PIPE)
    untar = subprocess.run(["tar", "-x", "-C", str(dest)], stdin=archive.stdout)
    archive.stdout.close()
    if archive.wait() != 0 or untar.returncode != 0:
        raise PerfError(f"cannot export {ref}")
    return tree_state(dest, ref=ref, repo=repo)


def verdict_map(report):
    return {f"{result['metric']} {result['stat']}": result["verdict"] for result in report["owned"]}


def unstable(aa):
    """The owned metrics an A/A reads moved."""
    return sorted(key for key, verdict in aa.items() if verdict in ("regressed", "improved"))


def judge_row(row, captures, quick=False):
    """A row's verdict from its captures, `{"A1": capture, "B1": ...}`: the
    A/A A1↔A2 and the direct pairs A1→B1, A2→B2 (quick: A1→B1 alone). A
    metric reads `regressed` when every pair reads it so and the A/A reads it
    `unchanged` or `noisy`. Returns the verdict, the regressed metrics, the
    A/A, the pairs and whether a rerun is due: an A/A reading any owned
    metric moved, or a known-drift metric regressed in a pair."""
    owned = row["owned"]
    aa = verdict_map(perf_stats.compare(captures["A1"], captures["A2"], owned))
    pairs = [verdict_map(perf_stats.compare(captures["A1"], captures["B1"], owned))]
    if not quick:
        pairs.append(verdict_map(perf_stats.compare(captures["A2"], captures["B2"], owned)))
    drift = {f"{entry['metric']} {stat}" for entry in row.get("known_drift", []) for stat in entry["stats"]}
    rerun = bool(unstable(aa)) or any(pair.get(key) == "regressed" for pair in pairs for key in drift)
    regressed = sorted(key for key in pairs[0] if all(pair.get(key) == "regressed" for pair in pairs) and aa.get(key) in ("unchanged", "noisy"))
    if unstable(aa):
        verdict = "unresolved"
    else:
        verdict = "regressed" if regressed else "ok"
    return {"verdict": verdict, "regressed": regressed, "aa": aa, "pairs": pairs, "rerun_due": rerun}


def sit_row(row, capture, quick=False, base_ready=True):
    """One row of a sitting. `capture(side, label)` takes a capture of side
    `A` (the base) or `B` (the candidate). Full: A1 B1 A2 B2, then, when the
    first set calls for it, A3 B3 A4 B4, whose verdicts stand alone; quick:
    A1 B1 A2 and no rerun. A row the base cannot build, or cannot run while
    the candidate can, reads `new`, measured on the candidate."""
    if not base_ready:
        cand = capture("B", "B1")
        return {"verdict": "new" if cand["valid"] else "invalid", "regressed": [], "reran": False, "aa": {}, "pairs": [], "candidate": cand}
    order = ("A1", "B1", "A2") if quick else ("A1", "B1", "A2", "B2")
    captures = {}
    for label in order:
        captures[label] = capture(label[0], label)
        if label == "B1" and not captures["A1"]["valid"] and captures["B1"]["valid"]:
            return {"verdict": "new", "regressed": [], "reran": False, "aa": {}, "pairs": [], "candidate": captures["B1"]}
    if not all(item["valid"] for item in captures.values()):
        return {"verdict": "invalid", "regressed": [], "reran": False, "aa": {}, "pairs": [], "candidate": captures["B1"]}
    judged = judge_row(row, captures, quick)
    reran = False
    if judged["rerun_due"] and not quick:
        reran = True
        second = {}
        for label, name in zip(("A1", "B1", "A2", "B2"), ("A3", "B3", "A4", "B4")):
            second[label] = capture(name[0], name)
        if not all(item["valid"] for item in second.values()):
            return {"verdict": "invalid", "regressed": [], "reran": True, "aa": {}, "pairs": [], "candidate": second["B1"]}
        captures = second
        judged = judge_row({**row, "known_drift": []}, captures)
    candidate = captures["B2"] if "B2" in captures else captures["B1"]
    return {"verdict": judged["verdict"], "regressed": judged["regressed"], "reran": reran, "aa": judged["aa"],
            "pairs": judged["pairs"], "candidate": candidate}  # fmt: skip


def sitting_rows(registry, names, quick=False, cpu_only=False):
    """The rows a sitting measures: `names` (rows or groups) or every row;
    quick and `--cpu-only` drop GPU-only rows and the GPU runs of the rest."""
    rows = registry["rows"] if not names else [row for name in names for row in select_rows(registry, name)]
    rows = list({row["name"]: row for row in rows}.values())
    if quick or cpu_only:
        rows = [{**row, "gpu": False} for row in rows if row.get("gpu") != "only"]
    if quick:
        rows = [{**row, "repeat": QUICK_REPEAT, "frames": QUICK_FRAMES} for row in rows]
    return rows


def sitting_readme(sitting):
    lines = [
        f"# Sitting {sitting['id']}{' (quick, informational)' if sitting['quick'] else ''}",
        "",
        f"- Base: {sitting['base']['ref']} ({sitting['base']['commit']})",
        f"- Candidate: {sitting['cand']['commit']}, dirty {sitting['cand']['dirty']}",
        f"- Wall: {sitting['wall_s']:.0f} s",
        f"- Start: load {sitting['start']['load_1m']}, governor {sitting['start']['governor']}, AC {sitting['start']['ac_power']}",
        "",
        "| Row | Verdict | Regressed | Reran |",
        "| --- | --- | --- | --- |",
    ]
    for row in sitting["rows"]:
        lines.append(f"| {row['name']} | {row['verdict']} | {', '.join(row['regressed']) or '—'} | {'yes' if row['reran'] else 'no'} |")
    lines += ["", "Captures: `captures/<row>-<A|B><n>/`.", ""]
    return "\n".join(lines)


def cmd_sitting(args, root=REPO_ROOT):
    started = time.monotonic()
    registry = load_registry()
    names = [name.strip() for name in (args.rows or "").split(",") if name.strip()]
    rows = sitting_rows(registry, names, args.quick, args.cpu_only)
    cand_state = {key: value for key, value in tree_state(root).items() if key in ("commit", "dirty")}

    def check():
        return start_check(own_pid=os.getpid(), tree_reader=lambda: git_dirty(root))

    refusals, record = check()
    if refusals and args.wait_quiet:
        refusals, record = wait_quiet(args.wait_quiet, check)
    if refusals:
        message = "; ".join(refusals)
        if not args.allow_background:
            raise PerfError(f"the sitting refuses to start: {message}")
        print(f"Starting despite: {message}", flush=True)
    base_ref = args.base or last_tag()
    base_dir = Path(root) / "target" / "perf" / "base-tree"
    base_tree = export_tree(base_ref, base_dir)
    cand_tree = tree_state(root)
    flags = PLAIN_FLAGS if args.plain else ALIGNED_FLAGS
    machine_info = machine()
    binaries = {"A": {}, "B": {}}
    for package in dict.fromkeys(row["package"] for row in rows):
        binaries["B"][package] = build(root, package, flags, target_dir(root, "tree", args.plain))
        try:
            binaries["A"][package] = build(base_dir, package, flags, target_dir(root, "base", args.plain))
        except PerfError as error:
            print(f"Base: {error}; its rows read new", flush=True)
    sitting_id = utc_stamp()
    out = new_dir(Path(root) / "perf-runs", f"{sitting_id}-sitting{'-quick' if args.quick else ''}")
    print(f"Output directory: {out.relative_to(root)}", flush=True)
    results = []
    for row in rows:
        def capture(side, label, row=row):
            tree, provenance = (base_dir, base_tree) if side == "A" else (root, cand_tree)
            for attempt in (1, 2):
                directory = new_dir(out / "captures", f"{row['name']}-{label}")
                taken = capture_row(
                    row, binaries[side][row["package"]], tree, directory, flags=flags, provenance=provenance,
                    machine_info=machine_info, sitting=sitting_id, allow_background=args.allow_background,
                    tree_reader=lambda: {key: value for key, value in tree_state(root).items() if key in ("commit", "dirty")},
                    watched=cand_state,
                )  # fmt: skip
                background = taken["reasons"] and all("background load" in reason for reason in taken["reasons"])
                if taken["valid"] or not background or attempt == 2:
                    return taken
                print(f"{row['name']} {label}: background load; waiting for quiet, then once more", flush=True)
                wait_quiet(args.wait_quiet or QUIET_SPAN_S * 5, check)
            return taken

        print(f"== {row['name']}", flush=True)
        outcome = sit_row(row, capture, args.quick, base_ready=row["package"] in binaries["A"])
        budgets = perf_stats.compare(outcome["candidate"], outcome["candidate"], [], row.get("budgets", []))["budgets"]
        results.append(
            {"name": row["name"], "verdict": outcome["verdict"], "regressed": outcome["regressed"], "reran": outcome["reran"],
             "aa": outcome["aa"], "pairs": outcome["pairs"], "budgets": budgets}
        )  # fmt: skip
        print(f"{row['name']}: {outcome['verdict']}{' ' + ', '.join(outcome['regressed']) if outcome['regressed'] else ''}", flush=True)
    if not args.keep:
        shutil.rmtree(base_dir, ignore_errors=True)
    sitting = {
        "schema": perf_stats.SCHEMA,
        "kind": "sitting",
        "id": sitting_id,
        "base": {"ref": base_ref, "commit": base_tree["commit"]},
        "cand": {"commit": cand_tree["commit"], "dirty": cand_tree["dirty"]},
        "quick": bool(args.quick),
        "cpu_only": bool(args.cpu_only or args.quick),
        "flags": flags,
        "start": record,
        "wall_s": round(time.monotonic() - started, 1),
        "rows": results,
    }
    (out / "sitting.json").write_text(json.dumps(sitting, indent=2) + "\n")
    (out / "README.md").write_text(sitting_readme(sitting))
    print(f"Sitting: {', '.join(f'{row['name']} {row['verdict']}' for row in results)}; {sitting['wall_s']:.0f} s")
    if args.quick:
        return 0
    verdicts = {row["verdict"] for row in results}
    if "invalid" in verdicts:
        return EXIT_INVALID
    return EXIT_REGRESSED if verdicts & {"regressed", "unresolved"} else 0


# --- Counters ------------------------------------------------------------------------


def counter_series(log_text, warmup, frames):
    """Each window frame's `counters:` fields, `{field: [values]}`."""
    measured = perf_stats.parse_log(log_text).frames[warmup : warmup + frames]
    return {field: [(frame.counters or {}).get(field) for frame in measured] for field in COUNTER_FIELDS}


def counter_pins(series):
    """A pin per field: `{"each": v}` when every frame reads `v`, else the
    window's `{"sum": total}`."""
    pins = {}
    for field, values in series.items():
        if values and len(set(values)) == 1 and values[0] is not None:
            pins[field] = {"each": int(values[0])}
        else:
            pins[field] = {"sum": int(sum(value or 0 for value in values))}
    return pins


def counter_mismatches(pins, series):
    """How `series` breaks `pins`, one line per field; empty when it holds."""
    problems = []
    for field, values in series.items():
        pin = pins.get(field)
        if pin is None:
            problems.append(f"{field}: no pin")
        elif "each" in pin:
            bad = [(index, value) for index, value in enumerate(values, start=1) if value != pin["each"]]
            if bad:
                index, value = bad[0]
                problems.append(f"{field}: {len(bad)} of {len(values)} frames differ from {pin['each']}; first: window frame {index} reads {value}")
        elif int(sum(value or 0 for value in values)) != pin["sum"]:
            problems.append(f"{field}: the window sums {int(sum(value or 0 for value in values))}, pinned {pin['sum']}")
    return problems


def cmd_counters(args, root=REPO_ROOT, registry_path=REGISTRY):
    registry = load_registry(registry_path)
    rows = [{**row, "gpu": False, "repeat": 1, "frames": COUNTER_FRAMES} for row in registry["rows"] if row["name"] in COUNTER_ROWS]
    binaries = {package: build(root, package, ALIGNED_FLAGS, target_dir(root, "tree", False)) for package in dict.fromkeys(row["package"] for row in rows)}
    out = new_dir(Path(root) / "perf-runs", f"{utc_stamp()}-counters")
    failed = []
    for row in rows:
        run_dir = out / row["name"]
        run_dir.mkdir()
        env = child_env(os.environ, side_env(row, warmup=row["warmup"], frames=COUNTER_FRAMES, gpu=False, run_dir=run_dir))
        result = run_child([str(binaries[row["package"]])], env, Path(root) / row["folder"], run_dir / "timing.log")
        shutil.rmtree(run_dir / "timing-user", ignore_errors=True)
        series = counter_series((run_dir / "timing.log").read_text(errors="replace"), row["warmup"], COUNTER_FRAMES)
        if result["exit_code"] != 0 or any(len(values) < COUNTER_FRAMES for values in series.values()):
            failed.append(f"{row['name']}: the run exited {result['exit_code']} or logged fewer than {COUNTER_FRAMES} window frames")
            continue
        entry = next(entry for entry in registry["rows"] if entry["name"] == row["name"])
        if args.write:
            entry["counters"] = counter_pins(series)
            print(f"{row['name']}: pinned {json.dumps(entry['counters'], sort_keys=True)}")
            continue
        problems = counter_mismatches(entry.get("counters") or {}, series)
        failed += [f"{row['name']}: {problem}" for problem in problems]
        print(f"{row['name']}: {'ok' if not problems else 'differs'}")
    if args.write and not failed:
        Path(registry_path).write_text(json.dumps(registry, indent=2) + "\n")
        print(f"Wrote the pins to {Path(registry_path).name}")
    for line in failed:
        print(f"  - {line}")
    print(f"Logs: {out.relative_to(root)}")
    return 1 if failed else 0


# --- Commands ------------------------------------------------------------------------


def cmd_list(args, registry=None):
    registry = registry or load_registry()
    rows = registry["rows"]
    if args.json:
        print(json.dumps(rows, indent=2))
        return 0
    for row in rows:
        kind = {"only": "GPU only", True: "CPU + GPU"}.get(row.get("gpu"), "CPU")
        group = f" [{row['group']}]" if row.get("group") else ""
        print(f"{row['name']}{group}: {row['package']}, {kind}, {row['warmup']} warm-up + {row['frames']} frames x {row['repeat']}")
    return 0


def target_dir(root, side, plain):
    return Path(root) / "target" / "perf" / f"{side}-{'plain' if plain else 'aligned'}"


def cmd_run(args, root=REPO_ROOT):
    registry = load_registry()
    rows = select_rows(registry, args.row)
    preflight_background(args.allow_background)
    flags = PLAIN_FLAGS if args.plain else ALIGNED_FLAGS
    provenance = tree_state(root)
    machine_info = machine()
    binaries = {}
    for package in dict.fromkeys(row["package"] for row in rows):
        binaries[package] = build(root, package, flags, target_dir(root, "tree", args.plain))
    status = 0
    for row in rows:
        out_dir = new_dir(Path(root) / "perf-runs", f"{utc_stamp()}-{row['name']}")
        print(f"Output directory: {out_dir.relative_to(root)}", flush=True)
        capture = capture_row(
            row, binaries[row["package"]], root, out_dir, flags=flags, provenance=provenance, machine_info=machine_info,
            repeat=args.repeat, frames=args.frames, allow_background=args.allow_background,
        )  # fmt: skip
        print(f"{row['name']}: {'valid' if capture['valid'] else 'INVALID'}; digest {capture['runs'][0]['digest'] if capture['runs'] else 'none'}")
        for reason in capture["reasons"]:
            print(f"  - {reason}")
        if not capture["valid"]:
            status = EXIT_INVALID
    return status


def baselines_dir(root):
    return Path(root) / "perf-runs" / "baselines"


def load_capture(ref, root=REPO_ROOT):
    """A capture or sitting by directory, `capture.json`/`sitting.json` path
    or baseline name; it must be schema 2."""
    candidates = [Path(ref), Path(root) / ref, baselines_dir(root) / ref]
    for candidate in candidates:
        for path in (candidate, candidate / "capture.json", candidate / "sitting.json"):
            if path.is_file():
                data = json.loads(path.read_text())
                if data.get("schema") != perf_stats.SCHEMA:
                    raise PerfError(f"{path} is schema {data.get('schema')}; the runner reads schema {perf_stats.SCHEMA} only")
                return data, path.parent
    raise PerfError(f"no capture, sitting or baseline '{ref}'")


def print_compare(report):
    if report["hard"] or report["invalid"]:
        print("Verdicts suppressed:")
        for item in report["hard"] + report["invalid"]:
            print(f"  - {item}")
    for result in report["owned"]:
        verdict = result["verdict"] or "no verdict"
        print(f"  {result['metric']} {result['stat']}: {fmt(result['base'])} -> {fmt(result['cand'])} ms: {verdict}")
    rss = report["rss"]
    print(f"  peak RSS: {fmt(rss['base'], 0)} -> {fmt(rss['cand'], 0)} KiB: {rss['verdict'] or rss.get('note', 'no verdict')}")
    for budget in report["budgets"]:
        print(f"  budget {budget['metric']} {budget['stat']} <= {budget['ms']} ms: {budget['verdict'] or 'no verdict'}")
    digests = report["digests"]
    same = "match" if digests["base"] == digests["cand"] else "differ"
    print(f"First-run digests {same}: {digests['base']} / {digests['cand']}")


def owned_for(row_name, capture, registry):
    row = next((row for row in registry["rows"] if row["name"] == row_name), None)
    if row is not None:
        return row["owned"], row.get("budgets", [])
    return capture.get("owned", []), []


def cmd_compare(args, root=REPO_ROOT):
    base, _ = load_capture(args.baseline, root)
    cand, _ = load_capture(args.candidate, root)
    if base.get("kind") != "capture" or cand.get("kind") != "capture":
        raise PerfError("compare takes two captures")
    owned, budgets = owned_for(cand["row"], cand, load_registry())
    report = perf_stats.compare(base, cand, owned, budgets)
    print_compare(report)
    return 0


def cmd_baseline(args, root=REPO_ROOT):
    directory = baselines_dir(root)
    if args.action == "list":
        for path in sorted(directory.glob("*/")) if directory.is_dir() else []:
            for name in ("capture.json", "sitting.json"):
                if (path / name).is_file():
                    data = json.loads((path / name).read_text())
                    if data.get("schema") == perf_stats.SCHEMA:
                        print(f"{path.name}: {data.get('kind')} {data.get('row') or data.get('id', '')}")
        return 0
    if args.action == "rm":
        target = directory / args.name
        if not target.is_dir():
            raise PerfError(f"no baseline '{args.name}'")
        shutil.rmtree(target)
        print(f"Removed baseline {args.name}")
        return 0
    data, source = load_capture(args.source, root)
    if not re.fullmatch(r"[A-Za-z0-9._-]+", args.name):
        raise PerfError("a baseline name is letters, digits, '.', '_' and '-'")
    target = directory / args.name
    if target.exists():
        raise PerfError(f"baseline '{args.name}' exists; remove it first")
    shutil.copytree(source, target, ignore=shutil.ignore_patterns("*.log", "*-user"))
    print(f"Saved baseline {args.name} ({data.get('kind')}) from {source}")
    return 0


def parser():
    top = argparse.ArgumentParser(prog="perf.py", description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    commands = top.add_subparsers(dest="command", required=True)
    listing = commands.add_parser("list", help="the registry's rows")
    listing.add_argument("--json", action="store_true")
    run = commands.add_parser("run", help="one capture per row of a row or group")
    run.add_argument("row")
    run.add_argument("--repeat", type=int)
    run.add_argument("--frames", type=int)
    run.add_argument("--plain", action="store_true", help="shipping flags: no 64-byte alignment")
    run.add_argument("--allow-background", action="store_true")
    compare = commands.add_parser("compare", help="verdicts of a candidate capture against a baseline")
    compare.add_argument("baseline")
    compare.add_argument("candidate")
    sitting = commands.add_parser("sitting", help="the working tree against a ref, with an A/A and one rerun")
    sitting.add_argument("--base", help="the ref to export and measure as the base (default: the last reachable tag)")
    sitting.add_argument("--rows", help="comma-separated rows or groups (default: every row)")
    sitting.add_argument("--quick", action="store_true", help="CPU rows, A1 B1 A2 at 3 repeats and 150 frames; informational")
    sitting.add_argument("--cpu-only", action="store_true", help="skip the GPU runs and GPU-only rows")
    sitting.add_argument("--wait-quiet", type=int, metavar="S", help="wait up to S seconds for 60 s of quiet")
    sitting.add_argument("--allow-background", action="store_true")
    sitting.add_argument("--plain", action="store_true", help="shipping flags on both sides")
    sitting.add_argument("--keep", action="store_true", help="keep the base export")
    counters = commands.add_parser("counters", help="check the GPU counter pins, or --write them")
    counters.add_argument("--write", action="store_true")
    baseline = commands.add_parser("baseline", help="save, list or remove named baselines")
    actions = baseline.add_subparsers(dest="action", required=True)
    save = actions.add_parser("save")
    save.add_argument("source")
    save.add_argument("name")
    actions.add_parser("list")
    remove = actions.add_parser("rm")
    remove.add_argument("name")
    return top


def main(argv=None):
    args = parser().parse_args(argv)
    handlers = {"list": cmd_list, "run": cmd_run, "compare": cmd_compare, "sitting": cmd_sitting, "counters": cmd_counters, "baseline": cmd_baseline}
    try:
        return handlers[args.command](args)
    except PerfError as error:
        print(f"error: {error}", file=sys.stderr)
        return EXIT_ERROR


if __name__ == "__main__":
    sys.exit(main())
