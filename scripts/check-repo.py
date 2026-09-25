#!/usr/bin/env python3
"""CPU-only asset coverage, documentation and active-plan checks (stdlib only).

Never traverses docs/plans/archive, follows directory symlinks, edits manifests,
or deletes files. Rust's manifests/decision_index tests remain authoritative for
manifest semantics and index membership; `just repo-check` runs them too.
"""

import argparse
import importlib.util
import json
import os
import re
from pathlib import Path
from urllib.parse import unquote, urlsplit

DOCS = (
    "README.md", "CHANGELOG.md", "docs/LLM_INDEX.md", "docs/DECISION_INDEX.md",
    "docs/agent-setup.md", "docs/perf/profiling-workflow.md",
    "docs/showcase/README.md", "docs/plans/README.md",
)
ARCHIVE = "docs/plans/archive"
# Existing content outside the manifest schema, not a blanket file exemption.
ASSET_EXCEPTIONS = {
    "assets/fonts/README.md": "font inventory and licensing documentation",
    "examples/03_scene_state/assets/scene.json": "D-046 scene loaded by the example startup",
    "assets/shaders/stock/lygia/LICENSE": "vendored shader license",
    **{f"assets/shaders/stock/lygia/{name}.wgsl": "compiled shader helper fragment"
       for name in ("hash", "luma", "noise", "srgb")},
}
DELETION_CANDIDATES = {
    "examples/01_platformer/assets/sprites/player.png": "unregistered legacy player image; tracked, retain for owner",
}
FIELDS = ("status", "goal", "non-goals", "files to touch", "ordered steps", "done-when")


def archive_path(path):
    return path == ARCHIVE or path.startswith(ARCHIVE + "/")


def walk_files(directory):
    for base, dirs, files in os.walk(directory, followlinks=False):
        dirs[:] = sorted(d for d in dirs if not (Path(base) / d).is_symlink())
        for name in sorted(files):
            yield Path(base) / name


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError(f"duplicate JSON key: {key}")
        result[key] = value
    return result


def check_assets(root, errors, notes):
    roots = [root / "assets"]
    roots.extend(sorted(p / "assets" for p in (root / "examples").iterdir()
                        if p.is_dir() and not p.is_symlink() and (p / "assets").is_dir()))
    for directory in roots:
        manifest = directory / "manifest.json"
        try:
            data = json.loads(manifest.read_text(), object_pairs_hook=unique_object)
            refs = set()
            for section, entries in data.items():
                for name, entry in entries.items():
                    for field in ("path", "normal_map", "emissive_mask"):
                        if entry.get(field) is None:
                            continue
                        path = Path(os.path.normpath(directory / entry[field]))
                        refs.add(path)
                        if not path.is_file():
                            errors.append(f"{manifest.relative_to(root)}: {section}.{name}.{field}: missing file {entry[field]}")
        except (OSError, ValueError, TypeError, AttributeError) as exc:
            errors.append(f"{manifest.relative_to(root)}: {exc}")
            continue
        for path in walk_files(directory):
            if path == manifest or path in refs:
                continue
            rel = path.relative_to(root).as_posix()
            local = path.relative_to(directory).parts
            # AGENTS.md permits complete font-family directories, including licenses.
            if len(local) >= 3 and local[0] == "fonts":
                continue
            if rel in ASSET_EXCEPTIONS:
                notes.append(f"asset exception: {rel}: {ASSET_EXCEPTIONS[rel]}")
            elif rel in DELETION_CANDIDATES:
                notes.append(f"deletion candidate: {rel}: {DELETION_CANDIDATES[rel]}")
            else:
                errors.append(f"unlisted asset: {rel}")


def plan_fields(text):
    # YAML frontmatter and the repository's older bold bullet headers.
    header = text.split("\n## ", 1)[0]
    result = {}
    for line in header.splitlines():
        match = re.match(r"^(?:-\s+\*\*)?([a-z -]+):(?:\*\*)?\s*(.*)$", line)
        if match:
            result[match[1]] = match[2].strip().strip('"')
    return result


def check_plans(root, errors, notes):
    # Deliberately non-recursive: archive is neither listed nor opened.
    for path in sorted((root / "docs/plans").glob("*.md")):
        if path.name == "README.md" or path.is_symlink():
            continue
        text = path.read_text()
        fields = plan_fields(text)
        rel = path.relative_to(root)
        for field in FIELDS:
            if field not in fields:
                errors.append(f"{rel}: missing plan header '{field}'")
        status = re.split(r"\s*[(—;]", fields.get("status", ""), maxsplit=1)[0].strip()
        if status in ("done", "abandoned", "superseded"):
            errors.append(f"{rel}: {status} plan belongs in {ARCHIVE}/")
        elif status not in ("draft", "in progress"):
            errors.append(f"{rel}: unknown status {status!r}")
        elif status == "in progress":
            notes.append(f"active plan: {rel}; review remaining checks before archiving (age alone is not completion)")


def check_docs(root, errors, notes):
    decisions = set()
    with (root / "DECISIONS.md").open() as source:
        for line in source:
            match = re.match(r"^## (D-\d{3})\b", line)
            if match:
                decisions.add(match[1])
    for rel in DOCS:
        path = root / rel
        text = path.read_text()
        for decision in set(re.findall(r"\bD-\d{3}\b", text)) - decisions:
            errors.append(f"{rel}: unknown decision {decision}")
        # Inline Markdown destinations, including image links. External URLs
        # are a manual/network QA task; local archive targets are never probed.
        for target in re.findall(r"\]\(([^\s)]+)\)", text):
            url = urlsplit(target)
            if url.scheme or url.netloc:
                continue
            dest = Path(os.path.normpath(path.parent / unquote(url.path))) if url.path else path
            if archive_path(os.path.relpath(dest, root)):
                notes.append(f"archive link left unread: {rel}: {target}")
                continue
            if not dest.exists():
                errors.append(f"{rel}: broken link {target}")
    # Reuse prefix expansion and skill/import checks rather than duplicate them.
    spec = importlib.util.spec_from_file_location("agent_context", root / "scripts/check-agent-context.py")
    context = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(context)
    errors.extend(context.check(root))


def check_agent_config(root, errors):
    settings = json.loads((root / ".claude/settings.json").read_text())
    if settings.get("env", {}).get("CLAUDE_CODE_GLOB_NO_IGNORE") != "false":
        errors.append(".claude/settings.json: Glob ignore setting must be false")
    if "Read(./docs/plans/archive/**)" not in settings.get("permissions", {}).get("deny", []):
        errors.append(".claude/settings.json: missing archive Read deny")
    for rule in settings.get("permissions", {}).get("allow", []):
        if rule == "Bash" or (rule.startswith("Bash(") and "*" in rule):
            errors.append(".claude/settings.json: execution allows must use exact commands")
    if "docs/plans/archive/" not in (root / ".ignore").read_text().splitlines():
        errors.append(".ignore: missing archive exclusion")


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parent.parent)
    args = parser.parse_args()
    errors, notes = [], []
    root = args.root.absolute()
    for check in (check_assets, check_plans, check_docs):
        try:
            check(root, errors, notes)
        except (OSError, ValueError) as exc:
            errors.append(f"{check.__name__}: {exc}")
    try:
        check_agent_config(root, errors)
    except (OSError, ValueError) as exc:
        errors.append(f"agent config: {exc}")
    for note in sorted(set(notes)):
        print(f"NOTE: {note}")
    for error in errors:
        print(f"ERROR: {error}")
    print(f"Repository QA: {len(errors)} error(s)")
    return int(bool(errors))


if __name__ == "__main__":
    raise SystemExit(main())
