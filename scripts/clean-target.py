#!/usr/bin/env python3
"""Prune stale build artifacts from target/<profile>/ (stdlib only), behind `just clean-target`.

  clean-target.py [--apply] [--keep N] [--profile NAME ...] [--target-dir DIR]

Cargo never removes the artifacts of an old hash: every flag, feature or profile
combination leaves its own test binaries (about 400 MB each, mostly debug info), their
.dwo files, incremental sessions and fingerprints behind. A unit is every entry of
deps/, build/, .fingerprint/ and incremental/ that shares one `name-hash` prefix. For
each name the newest N units by modification time stay (default 2) and the rest go, so
the next build of a unit pruned while still current recompiles that crate only.

Without --apply it only reports what would go. --apply refuses while cargo or rustc
runs. Files whose names do not end in a hash are never touched. Default profile:
debug; release holds the perf-capture binary, so it is opt-in (--profile release).
"""

import argparse
import os
import re
import shutil
import subprocess
import sys
from collections import defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
HEX16 = re.compile(r"^(?P<name>.+)-(?P<hash>[0-9a-f]{16})$")
BASE36 = re.compile(r"^(?P<name>.+)-(?P<hash>[0-9a-z]{13})$")
SUBDIRS = {"deps": HEX16, "build": HEX16, ".fingerprint": HEX16, "incremental": BASE36}
GIB = 1024 ** 3


def stale_units(directory, pattern, keep):
    """Units of `directory` older than the newest `keep` of their name, as lists of paths."""
    units = defaultdict(dict)  # name -> unit -> [newest mtime, paths]
    with os.scandir(directory) as entries:
        for entry in entries:
            match = pattern.match(entry.name.split(".", 1)[0])
            if not match:
                continue
            unit = f"{match['name']}-{match['hash']}"
            slot = units[match["name"]].setdefault(unit, [0.0, []])
            slot[0] = max(slot[0], entry.stat(follow_symlinks=False).st_mtime)
            slot[1].append(Path(entry.path))
    stale = []
    for by_unit in units.values():
        newest_first = sorted(by_unit.values(), key=lambda slot: slot[0], reverse=True)
        stale.extend(slot[1] for slot in newest_first[keep:])
    return stale


def disk_bytes(paths, seen):
    """Allocated bytes under `paths`, counting each inode once across calls through `seen`."""
    total = 0
    for path in paths:
        stack = [path]
        while stack:
            current = stack.pop()
            info = current.lstat()
            if (info.st_dev, info.st_ino) not in seen:
                seen.add((info.st_dev, info.st_ino))
                total += info.st_blocks * 512
            if current.is_dir() and not current.is_symlink():
                stack.extend(current.iterdir())
    return total


def remove(path):
    if path.is_dir() and not path.is_symlink():
        shutil.rmtree(path)
    else:
        path.unlink()


def build_running():
    return subprocess.run(["pgrep", "-x", "cargo|rustc"], capture_output=True).returncode == 0


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.split("\n", 1)[0])
    parser.add_argument("--apply", action="store_true", help="delete; the default only reports")
    parser.add_argument("--keep", type=int, default=2, help="newest units kept per name (default 2)")
    parser.add_argument("--profile", action="append", help="target/ profile folder; repeatable (default debug)")
    parser.add_argument("--target-dir", type=Path, default=ROOT / "target")
    args = parser.parse_args(argv)
    if args.keep < 1:
        parser.error("--keep must be at least 1")
    if args.apply and build_running():
        print("error: cargo or rustc is running; wait for it or stop it first", file=sys.stderr)
        return 1

    free_before = shutil.disk_usage(args.target_dir).free
    seen = set()
    total = 0
    for profile in args.profile or ["debug"]:
        for name, pattern in SUBDIRS.items():
            directory = args.target_dir / profile / name
            if not directory.is_dir():
                continue
            stale = stale_units(directory, pattern, args.keep)
            paths = [path for unit in stale for path in unit]
            size = disk_bytes(paths, seen)
            total += size
            print(f"{profile}/{name}: {len(stale)} stale units, {len(paths)} entries, {size / GIB:.1f} GiB")
            if args.apply:
                for path in paths:
                    remove(path)
    if args.apply:
        freed = shutil.disk_usage(args.target_dir).free - free_before
        print(f"removed; free space up {freed / GIB:.1f} GiB")
    else:
        print(f"would free up to {total / GIB:.1f} GiB (hard links shared with kept files stay); rerun with --apply")
    return 0


if __name__ == "__main__":
    sys.exit(main())
