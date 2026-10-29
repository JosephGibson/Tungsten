#!/usr/bin/env python3
"""Benchmark runner (redo 7.3, `D-078`; docs/perf/profiling-workflow.md).

  list [--json]                   the rows `scripts/perf-rows.json` holds
  run <row|group> [flags]         one capture per row
  compare <baseline> <candidate>  verdicts on the candidate's owned metrics
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
EXIT_ERROR = 2
EXIT_INVALID = 3


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
                sitting=None, allow_background=False, lister=list_processes, tree_reader=None):  # fmt: skip
    """`repeat` runs of `row` (a timing run, a GPU run beside it or alone, as
    the row's `gpu` says), then `capture.json` and `README.md` in `out_dir`."""
    repeat = repeat or row["repeat"]
    frames = frames or row["frames"]
    warmup = row["warmup"]
    gpu = row.get("gpu", False)
    labels = {"only": [True], True: [False, True]}.get(gpu, [False])
    built = {key: provenance[key] for key in ("commit", "dirty")}
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
    handlers = {"list": cmd_list, "run": cmd_run, "compare": cmd_compare, "baseline": cmd_baseline}
    try:
        return handlers[args.command](args)
    except PerfError as error:
        print(f"error: {error}", file=sys.stderr)
        return EXIT_ERROR


if __name__ == "__main__":
    sys.exit(main())
