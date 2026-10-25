#!/usr/bin/env python3
"""The missing_docs ratchet (D-215), behind `just docs-ratchet`: public items without rustdoc may only fall.

  missing-docs.py [--check | --write | --list CRATE]

For each crate below, `cargo clippy -p <crate> --lib --locked --message-format=json
-- -W missing_docs` lints it, and its count is the distinct (file, line, column)
primary spans of `missing_docs` warnings inside the crate's own folder, build
scripts aside: each run also lints the workspace crates it depends on, so a plain
sum of the runs counts core three times. The runs share target/missing-docs, so
the main build keeps its artifacts.

--check (the default) fails when a crate's count rises over scripts/missing-docs.json
and prints a note to lower the baseline when one falls; --write rewrites the
baseline; --list prints a crate's items. Standard library only.
"""

import argparse
import json
import os
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
BASELINE = "scripts/missing-docs.json"
TARGET = "target/missing-docs"
# Package name and its folder; the kit denies the lint, so it has no count.
CRATES = (("tungsten-core", "crates/tungsten-core"), ("tungsten-render", "crates/tungsten-render"),
          ("tungsten", "crates/tungsten"))


def items(output, root, folder):
    """The (file, line, column) primary spans of missing_docs warnings in `folder`, from cargo's JSON lines."""
    found = set()
    prefix = folder.rstrip("/") + "/"
    for line in output.splitlines():
        if not line.startswith("{"):
            continue
        message = json.loads(line)
        # A build script is its own crate, outside the library's public surface.
        if message.get("reason") != "compiler-message" or "custom-build" in message.get("target", {}).get("kind", []):
            continue
        diagnostic = message["message"]
        if (diagnostic.get("code") or {}).get("code") != "missing_docs":
            continue
        for span in diagnostic.get("spans", []):
            if not span.get("is_primary"):
                continue
            name = Path(span["file_name"])
            if name.is_absolute():
                name = Path(os.path.relpath(name, root))
            name = name.as_posix()
            if name.startswith(prefix):
                found.add((name, span["line_start"], span["column_start"]))
    return found


def lint(root, crate, folder):
    if not (root / folder / "Cargo.toml").is_file():
        raise SystemExit(f"missing-docs: {folder}/Cargo.toml not found; update CRATES in scripts/missing-docs.py")
    print(f"missing-docs: linting {crate}", file=sys.stderr, flush=True)
    result = subprocess.run(
        ["cargo", "clippy", "-p", crate, "--lib", "--locked", "--message-format=json", "--", "-W", "missing_docs"],
        cwd=root, capture_output=True, text=True, env={**os.environ, "CARGO_TARGET_DIR": str(root / TARGET)},
    )
    if result.returncode:
        sys.stderr.write(result.stderr)
        raise SystemExit(f"missing-docs: cargo clippy -p {crate} failed with exit code {result.returncode}")
    return items(result.stdout, root, folder)


def compare(counts, baseline):
    """Errors for a crate whose count rose or has no baseline, notes for one that fell."""
    errors, notes = [], []
    for crate, count in counts.items():
        base = baseline.get(crate)
        if not isinstance(base, int):
            errors.append(f"{crate}: no baseline in {BASELINE}; run `just docs-ratchet --write`")
        elif count > base:
            errors.append(f"{crate}: {count} public items without rustdoc, over the baseline's {base}; document "
                          f"the new ones (`just docs-ratchet --list {crate}` lists them), since missing_docs only falls")
        elif count < base:
            notes.append(f"{crate}: {count}, under the baseline's {base}; lower it with `just docs-ratchet --write`")
    return errors, notes


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--check", action="store_true", help="fail when a crate's count rises over the baseline (default)")
    mode.add_argument("--write", action="store_true", help=f"rewrite {BASELINE} with the current counts")
    mode.add_argument("--list", metavar="CRATE", choices=[crate for crate, _ in CRATES], help="print a crate's items")
    parser.add_argument("--root", type=Path, default=ROOT)
    args = parser.parse_args(argv)
    root = args.root.resolve()
    if args.list:
        for name, line, column in sorted(lint(root, args.list, dict(CRATES)[args.list])):
            print(f"{name}:{line}:{column}")
        return 0
    counts = {crate: len(lint(root, crate, folder)) for crate, folder in CRATES}
    if args.write:
        (root / BASELINE).write_text(json.dumps(counts, indent=2) + "\n")
        print(f"missing-docs: wrote {BASELINE}: " + ", ".join(f"{c} {n}" for c, n in counts.items()))
        return 0
    baseline = json.loads((root / BASELINE).read_text())
    errors, notes = compare(counts, baseline)
    for note in notes:
        print(f"NOTE: {note}")
    for error in errors:
        print(f"ERROR: {error}")
    print("missing_docs: " + ", ".join(f"{c} {n}" for c, n in counts.items())
          + (f"; {len(errors)} rise(s)" if errors else "; no rise"))
    return int(bool(errors))


if __name__ == "__main__":
    raise SystemExit(main())
