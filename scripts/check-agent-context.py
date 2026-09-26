#!/usr/bin/env python3
"""Check agent instruction budgets, navigation links and shared-skill symlinks.

Prints repo-byte totals for the root and render instruction sets. They are file
sizes, not token counts and not total session context (client system prompts,
user settings and memory are excluded). Never reads or walks
docs/plans/archive/. Standard library only.

Usage: check-agent-context.py [--root DIR] [--self-test]
"""

import argparse
import os
import re
import sys
import tempfile
from pathlib import Path

ARCHIVE = "docs/plans/archive"
IMPORT_ONLY = "@AGENTS.md\n"
# path -> (max bytes, max lines or None); lines must stay below the limit.
BUDGETS = {
    "AGENTS.md": (6144, 200),
    "crates/tungsten-render/AGENTS.md": (4096, 200),
    "docs/LLM_INDEX.md": (8192, None),
}
IMPORT_FILES = ["CLAUDE.md", "crates/tungsten-render/CLAUDE.md"]
LINK_FILES = [
    "AGENTS.md",
    "crates/tungsten-render/AGENTS.md",
    "docs/LLM_INDEX.md",
    "docs/DECISION_INDEX.md",
    "docs/agent-setup.md",
    "docs/plans/README.md",
]
INDEX = "docs/LLM_INDEX.md"
INDEX_PREFIXES = {
    "core/": "crates/tungsten-core/src/",
    "render/": "crates/tungsten-render/src/",
    "tungsten/": "crates/tungsten/src/",
}
SKILLS = ".claude/skills"
SHARED_SKILLS = ".agents/skills"
SKILL_DESCRIPTION_MAX = 300
SKILL_BODY_MAX = 8192

LINK_RE = re.compile(r"\]\(([^)\s#]+)(?:#[^)]*)?\)")
CODE_RE = re.compile(r"`([^`\s]+)`")
PATHLIKE_RE = re.compile(r"/|\.(md|json|rs|wgsl|toml|sh|py)$")


def in_archive(rel: str) -> bool:
    rel = rel.rstrip("/")
    return rel == ARCHIVE or rel.startswith(ARCHIVE + "/")


def check_links(root: Path, doc: str, base_dir: str, errors: list) -> None:
    """Markdown links in `doc`, resolved lexically against `base_dir`."""
    text = (root / doc).read_text(encoding="utf-8")
    for target in LINK_RE.findall(text):
        if re.match(r"^[a-z]+:", target):
            continue
        rel = os.path.normpath(os.path.join(base_dir, target))
        if in_archive(rel):
            continue
        if not (root / rel).exists():
            errors.append(f"{doc}: broken link {target} (from {base_dir or '.'}/)")


def check_index_paths(root: Path, errors: list) -> None:
    # Markdown links are checked relative to the index by check_links; only
    # bare backticked paths are repo-root-relative (with crate prefixes).
    text = re.sub(r"\[[^\]]*\]\([^)]*\)", "", (root / INDEX).read_text(encoding="utf-8"))
    for token in CODE_RE.findall(text):
        if token in INDEX_PREFIXES or not PATHLIKE_RE.search(token):
            continue
        if any(c in token for c in "*<>{}") or "NN" in token or token.startswith("D-"):
            continue
        rel = token
        for prefix, expansion in INDEX_PREFIXES.items():
            if token.startswith(prefix):
                rel = expansion + token[len(prefix):]
                break
        if in_archive(rel):
            continue
        if not (root / rel).exists():
            errors.append(f"{INDEX}: path `{token}` does not resolve ({rel})")


def frontmatter(text: str):
    if not text.startswith("---\n"):
        return None, text
    end = text.find("\n---\n", 4)
    if end < 0:
        return None, text
    fields = {}
    for line in text[4:end].splitlines():
        key, _, value = line.partition(":")
        fields[key.strip()] = value.strip()
    return fields, text[end + 5:]


def check_skills(root: Path, errors: list) -> None:
    skills_dir = root / SKILLS
    names = sorted(p.name for p in skills_dir.iterdir() if p.is_dir()) if skills_dir.is_dir() else []
    if not names:
        errors.append(f"{SKILLS}: no skills found")
    for name in names:
        skill = f"{SKILLS}/{name}/SKILL.md"
        if not (root / skill).is_file():
            errors.append(f"{skill}: missing")
            continue
        fields, body = frontmatter((root / skill).read_text(encoding="utf-8"))
        if fields is None:
            errors.append(f"{skill}: missing frontmatter")
            continue
        if fields.get("name") != name:
            errors.append(f"{skill}: frontmatter name {fields.get('name')!r} != directory {name!r}")
        description = fields.get("description", "")
        if not description or len(description) > SKILL_DESCRIPTION_MAX:
            errors.append(f"{skill}: description is {len(description)} chars (1..{SKILL_DESCRIPTION_MAX})")
        if len(body.encode()) > SKILL_BODY_MAX:
            errors.append(f"{skill}: body is {len(body.encode())} B (max {SKILL_BODY_MAX})")
        link = root / SHARED_SKILLS / name
        expected = f"../../{SKILLS}/{name}"
        if not link.is_symlink():
            errors.append(f"{SHARED_SKILLS}/{name}: missing symlink to {expected}")
        elif os.readlink(link) != expected or not (link / "SKILL.md").is_file():
            errors.append(f"{SHARED_SKILLS}/{name}: points at {os.readlink(link)!r}, expected {expected!r}")
        for base in (f"{SKILLS}/{name}", f"{SHARED_SKILLS}/{name}"):
            check_links(root, skill, base, errors)
    shared = root / SHARED_SKILLS
    if shared.is_dir():
        for entry in sorted(shared.iterdir()):
            if entry.name not in names:
                errors.append(f"{SHARED_SKILLS}/{entry.name}: no matching {SKILLS}/{entry.name}")


def check(root: Path) -> list:
    errors = []
    for rel, (max_bytes, max_lines) in BUDGETS.items():
        path = root / rel
        if not path.is_file():
            errors.append(f"{rel}: missing")
            continue
        data = path.read_bytes()
        lines = data.count(b"\n")
        if len(data) > max_bytes:
            errors.append(f"{rel}: {len(data)} B exceeds {max_bytes} B")
        if max_lines is not None and lines >= max_lines:
            errors.append(f"{rel}: {lines} lines, must stay below {max_lines}")
    for rel in IMPORT_FILES:
        path = root / rel
        if not path.is_file() or path.read_text(encoding="utf-8") != IMPORT_ONLY:
            errors.append(f"{rel}: must contain only '@AGENTS.md'")
    for rel in LINK_FILES:
        if (root / rel).is_file():
            check_links(root, rel, os.path.dirname(rel), errors)
        else:
            errors.append(f"{rel}: missing")
    if (root / INDEX).is_file():
        check_index_paths(root, errors)
    check_skills(root, errors)
    return errors


def size(root: Path, *rels: str) -> int:
    return sum((root / rel).stat().st_size for rel in rels if (root / rel).is_file())


def report(root: Path) -> None:
    render = "crates/tungsten-render"
    rows = [
        ("Claude, repo root", size(root, "CLAUDE.md", "AGENTS.md")),
        ("Codex, repo root", size(root, "AGENTS.md")),
        ("Claude, render crate", size(root, "CLAUDE.md", "AGENTS.md", f"{render}/CLAUDE.md", f"{render}/AGENTS.md")),
        ("Codex, render crate", size(root, "AGENTS.md", f"{render}/AGENTS.md")),
        ("On demand: LLM_INDEX.md", size(root, INDEX)),
    ]
    print("Instruction repo bytes (file sizes, not tokens or total session context):")
    for label, total in rows:
        print(f"  {label:26s} {total:>6} B")


def self_test() -> int:
    """Synthetic fixtures: a valid tree passes, each corruption fails."""
    ok = "# Rules\n\nSee [index](docs/LLM_INDEX.md).\n"
    files = {
        "AGENTS.md": ok,
        "CLAUDE.md": IMPORT_ONLY,
        "crates/tungsten-render/AGENTS.md": "# Render\n",
        "crates/tungsten-render/CLAUDE.md": IMPORT_ONLY,
        "crates/tungsten-core/src/lib.rs": "",
        "docs/LLM_INDEX.md": "| ECS | `core/lib.rs` |\n",
        "docs/DECISION_INDEX.md": "# Decisions\n",
        "docs/agent-setup.md": "[rules](../AGENTS.md)\n",
        "docs/plans/README.md": "# Plans\n",
        f"{SKILLS}/demo/SKILL.md": "---\nname: demo\ndescription: Demo skill.\n---\n[rules](../../../AGENTS.md)\n",
    }
    cases = {
        "valid": lambda r: None,
        "oversize AGENTS.md": lambda r: (r / "AGENTS.md").write_text(ok + "x" * 7000),
        "too many lines": lambda r: (r / "AGENTS.md").write_text(ok + "\n" * 200),
        "broken link": lambda r: (r / "docs/agent-setup.md").write_text("[gone](missing.md)\n"),
        "broken index path": lambda r: (r / INDEX).write_text("`core/nope.rs`\n"),
        "CLAUDE.md with body": lambda r: (r / "CLAUDE.md").write_text(IMPORT_ONLY + "extra\n"),
        "long skill description": lambda r: (r / f"{SKILLS}/demo/SKILL.md").write_text(
            "---\nname: demo\ndescription: " + "d" * 301 + "\n---\n"),
        "broken skill link": lambda r: (r / f"{SKILLS}/demo/SKILL.md").write_text(
            "---\nname: demo\ndescription: Demo.\n---\n[x](../../AGENTS.md)\n"),
        "missing symlink": lambda r: (r / SHARED_SKILLS / "demo").unlink(),
    }
    failures = 0
    for name, corrupt in cases.items():
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            for rel, text in files.items():
                (root / rel).parent.mkdir(parents=True, exist_ok=True)
                (root / rel).write_text(text)
            (root / SHARED_SKILLS).mkdir(parents=True)
            os.symlink(f"../../{SKILLS}/demo", root / SHARED_SKILLS / "demo")
            corrupt(root)
            errors = check(root)
            passed = not errors if name == "valid" else bool(errors)
            print(f"  {'ok  ' if passed else 'FAIL'} {name}: {len(errors)} error(s)")
            if not passed:
                failures += 1
                for error in errors:
                    print(f"       {error}")
    print("self-test:", "OK" if failures == 0 else f"{failures} failure(s)")
    return 1 if failures else 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parent.parent)
    parser.add_argument("--self-test", action="store_true", help="run synthetic fixture checks")
    args = parser.parse_args()
    if args.self_test:
        return self_test()
    errors = check(args.root)
    report(args.root)
    if errors:
        print(f"\n{len(errors)} problem(s):")
        for error in errors:
            print(f"  - {error}")
        return 1
    print("\nAgent context checks: OK")
    return 0


if __name__ == "__main__":
    sys.exit(main())
