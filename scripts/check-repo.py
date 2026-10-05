#!/usr/bin/env python3
"""CPU-only asset coverage, documentation, active-plan, plan-citation and roadmap checks (stdlib only).

Never traverses docs/plans/archive, follows directory symlinks, edits manifests,
or deletes files. Rust's manifests/decision_index tests remain authoritative for
manifest semantics and index membership; `just repo-check` runs them too.
"""

import argparse
import importlib.util
import json
import os
import re
import subprocess
from pathlib import Path
from urllib.parse import unquote, urlsplit

DOCS = (
    "README.md", "CHANGELOG.md", "docs/LLM_INDEX.md", "docs/DECISION_INDEX.md",
    "docs/agent-setup.md", "docs/perf/profiling-workflow.md", "docs/perf/benchmarks.md",
    "docs/showcase/README.md", "docs/plans/README.md", "docs/releases.md", "docs/assets.md",
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
FIELDS = ("status", "goal", "non-goals", "files to touch", "ordered steps", "done-when")
CITING_SUFFIXES = (".rs", ".py", ".sh", ".md", ".toml", ".yml")
# History keeps the plan names of its time; the scripts' tests cite fixture plans.
CITATION_EXEMPT = ("CHANGELOG.md", "DECISIONS.md")
PLAN_CITATION = re.compile(r"docs/plans/[\w./-]+?\.md")
ROADMAP_ID = re.compile(r"[a-z0-9]+")

# The register parser the roadmap sync uses, so both read the register one way.
_spec = importlib.util.spec_from_file_location("roadmap", Path(__file__).with_name("roadmap.py"))
roadmap = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(roadmap)


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
                # A list section such as `font_fallback` names IDs, not files.
                if not isinstance(entries, dict):
                    continue
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
            # docs/assets.md permits complete font-family directories, including licenses.
            if len(local) >= 3 and local[0] == "fonts":
                continue
            if rel in ASSET_EXCEPTIONS:
                notes.append(f"asset exception: {rel}: {ASSET_EXCEPTIONS[rel]}")
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


def plan_files(root):
    """Plans at the top level and one program folder deep (`docs/plans/<program>/`)."""
    plans = root / "docs/plans"
    # Deliberately one level deep: archive is neither listed nor opened.
    folders = [plans] + sorted(d for d in plans.iterdir()
                               if d.is_dir() and not d.is_symlink() and d.name != "archive")
    for folder in folders:
        for path in sorted(folder.glob("*.md")):
            if path.name != "README.md" and not path.is_symlink():
                yield path


def check_plans(root, errors, notes):
    for path in plan_files(root):
        text = path.read_text()
        fields = plan_fields(text)
        rel = path.relative_to(root)
        for field in FIELDS:
            if field not in fields:
                errors.append(f"{rel}: missing plan header '{field}'")
        status = re.split(r"\s*[(—;]", fields.get("status", ""), maxsplit=1)[0].strip()
        if status in ("done", "abandoned", "superseded"):
            program = path.parent.relative_to(root / "docs/plans").as_posix()
            target = ARCHIVE if program == "." else f"{ARCHIVE}/{program}"
            errors.append(f"{rel}: {status} plan belongs in {target}/")
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


def tracked_files(root):
    """Paths Git tracks under a work-tree root; elsewhere every file outside
    `.git`, `target` and `docs/plans`."""
    if (root / ".git").exists():
        try:
            listed = subprocess.run(
                ["git", "-C", str(root), "ls-files", "-z"], capture_output=True, check=True,
                env={**os.environ, "GIT_OPTIONAL_LOCKS": "0"},
            ).stdout.decode()
            return [rel for rel in listed.split("\0") if rel]
        except (OSError, subprocess.CalledProcessError):
            pass
    files = []
    for base, dirs, names in os.walk(root, followlinks=False):
        rel_base = Path(base).relative_to(root)
        dirs[:] = sorted(d for d in dirs if d not in (".git", "target")
                         and (rel_base / d).as_posix() != "docs/plans"
                         and not (Path(base) / d).is_symlink())
        files.extend((rel_base / name).as_posix() for name in names)
    return sorted(files)


def check_plan_citations(root, errors, notes):
    # A cited plan that moved to the archive or was deleted leaves agents a dead
    # route. Archive citations stay notes: archived plans are never opened.
    for rel in tracked_files(root):
        if (not rel.endswith(CITING_SUFFIXES) or rel.startswith("docs/plans/")
                or rel in CITATION_EXEMPT or re.fullmatch(r"scripts/test-[^/]+\.py", rel)):
            continue
        path = root / rel
        if not path.is_file():  # deleted in the work tree, still in the index
            continue
        for cited in sorted(set(PLAN_CITATION.findall(path.read_text(errors="replace")))):
            if archive_path(cited):
                notes.append(f"archive citation left unread: {rel}: {cited}")
            elif not (root / cited).is_file():
                errors.append(f"{rel}: cites missing plan {cited}")


def check_roadmap(root, errors, notes):
    """The roadmap catalog against the register, the §3 cards and the question lists."""
    rel = roadmap.CATALOG
    if not (root / rel).exists() and not (root / roadmap.PLAN).exists():
        return
    try:
        catalog = roadmap.load_catalog(root)
        stops, questions = catalog["stops"], catalog["questions"]
        groups = {group["id"] for group in catalog["groups"]}
        qids = [question["id"] for question in questions]
        ids = [stop["id"] for stop in stops] + qids
        for stop in stops:
            if stop["group"] not in groups:
                errors.append(f"{rel}: {stop['id']}: unknown group {stop['group']}")
            for qid in stop.get("questions", []):
                if qid not in qids:
                    errors.append(f"{rel}: {stop['id']}: unknown question {qid}")
            for entry in stop.get("src", []) + stop.get("reads", []):
                path = entry.split(" ", 1)[0]
                target = root / path
                if not (target.is_dir() if path.endswith("/") else target.is_file()):
                    errors.append(f"{rel}: {stop['id']}: missing path {path}")
        if (root / roadmap.PROMPTS).exists():  # each stop's stages from prompts.md's Flow table (D-118)
            roadmap.catalog_payload(root)
    except (OSError, ValueError, KeyError, TypeError, AttributeError) as exc:
        errors.append(f"{rel}: {exc!r}")
        return
    for duplicate in sorted({i for i in ids if ids.count(i) > 1}):
        errors.append(f"{rel}: duplicate id {duplicate}")
    for bad in sorted({i for i in ids if not ROADMAP_ID.fullmatch(str(i))}):
        errors.append(f"{rel}: id {bad!r} is not lowercase letters and digits")

    # Pairs and placeholders are one register row and one stop, or several stops in a run.
    rows = []
    for stop in stops:
        if stop.get("row") is not None and (not rows or rows[-1] != stop["row"]):
            rows.append(stop["row"])
    register = [row["candidate"] for row in roadmap.register(root)]
    if rows != register:
        at = next((i for i, (a, b) in enumerate(zip(rows, register)) if a != b), min(len(rows), len(register)))
        ours = rows[at] if at < len(rows) else "(end)"
        theirs = register[at] if at < len(register) else "(end)"
        errors.append(f"{rel}: row {at + 1} is {ours!r}, the register's (implementation plan §10) is {theirs!r}")

    cards = roadmap.card_levels(root)
    for stop in stops:
        card = cards.get(stop.get("row"))
        if card is not None and not (stop.get("level") and card.startswith(stop["level"])):
            errors.append(f"{rel}: {stop['id']}: level {stop.get('level')!r}, its §3 card says {card!r}")

    numbers = roadmap.question_numbers(root)
    for qid in qids:
        match = re.fullmatch(r"q(\d+)", str(qid))
        if not match or int(match[1]) not in numbers:
            errors.append(f"{rel}: question {qid} is not in implementation plan §7 or criteria §10")


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
    for check in (check_assets, check_plans, check_docs, check_plan_citations, check_roadmap):
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
