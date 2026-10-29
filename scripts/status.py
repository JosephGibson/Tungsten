#!/usr/bin/env python3
"""Where Tungsten stands, derived from the tree (`just status`; the tungsten-next skill reads it).

Read-only and offline: origin/* are local refs as of the last fetch. Standard library only.
Six sections: git, tree, sessions, frozen records, ROADMAP.md's Now and Next, active plans.
Sessions read Linux's /proc and print `unavailable` without it.
"""

import os
import re
import subprocess
import sys
import time
from pathlib import Path

PROC = Path("/proc")
# A file written under 3 minutes ago means a session is probably mid-step.
LIVE = 180
SHOWN = 30
FINAL_TAG = re.compile(r"v(\d+)\.(\d+)\.(\d+)")
FROZEN_BANNER = r"Frozen [0-9-]+ \(`?D-215"
CELL = re.compile(r"(?<!\\)\|")


def git(root, *args):
    """A git command's output without its final newline, or None when it fails."""
    result = subprocess.run(["git", *args], cwd=root, text=True, capture_output=True)
    return result.stdout.rstrip("\n") if result.returncode == 0 else None


def span(seconds):
    seconds = max(0, int(seconds))
    if seconds < 3600:
        return f"{seconds // 60}m"
    if seconds < 172800:
        return f"{seconds // 3600}h"
    return f"{seconds // 86400}d"


def version_key(text):
    match = re.fullmatch(r"v?(\d+)\.(\d+)\.(\d+)", text or "")
    return tuple(map(int, match.groups())) if match else None


def workspace_version(root):
    try:
        text = (root / "Cargo.toml").read_text()
    except OSError:
        return None
    table = re.search(r"^\[workspace\.package\][ \t]*\n(.*?)(?=^\[|\Z)", text, re.M | re.S)
    found = table and re.search(r'^version\s*=\s*"([^"]+)"', table.group(1), re.M)
    return found.group(1) if found else None


def newest_final_tag(root):
    """The highest `vX.Y.Z` tag on HEAD's history; prerelease and off-history tags don't count."""
    refs = git(root, "for-each-ref", "--merged", "HEAD", "--format=%(refname:short)", "refs/tags/v*")
    tags = [tag for tag in (refs or "").split() if FINAL_TAG.fullmatch(tag)]
    return max(tags, key=version_key, default=None)


def git_section(root, now):
    out = ["== git"]
    branch = git(root, "branch", "--show-current") or "detached"
    upstream = git(root, "rev-parse", "--abbrev-ref", "@{upstream}") or "none"
    out.append(f"branch: {branch} · upstream: {upstream}")
    version, tag = workspace_version(root), newest_final_tag(root)
    line = f"version: {version or 'unknown'} (Cargo.toml)"
    if tag and version_key(version) and version_key(version) > version_key(tag):
        line += f" · a cut waits: the version is past {tag}"
    out.append(line)
    if not tag:
        out.append("newest final tag on HEAD's history: none")
        return out
    fetch_head = root / (git(root, "rev-parse", "--git-path", "FETCH_HEAD") or ".git/FETCH_HEAD")
    fetched = f"{span(now - fetch_head.stat().st_mtime)} ago" if fetch_head.exists() else "never"
    if git(root, "rev-parse", "--verify", "--quiet", "refs/remotes/origin/main") is None:
        on_main = "no origin/main"
    else:
        on_main = "yes" if git(root, "merge-base", "--is-ancestor", tag, "origin/main") is not None else "no"
    out.append(f"newest final tag on HEAD's history: {tag} · on origin/main: {on_main} (last fetch {fetched})")
    count = int(git(root, "rev-list", "--count", f"{tag}..HEAD") or 0)
    out.append(f"commits since {tag}: {count}")
    log = git(root, "log", f"-{SHOWN}", "--format=%h %s", f"{tag}..HEAD")
    out += [f"  {line}" for line in (log or "").splitlines()]
    if count > SHOWN:
        out.append(f"  +{count - SHOWN} older")
    return out


def tree_section(root, now):
    out = ["== tree"]
    status = git(root, "status", "--short")
    out.append(status or "clean")
    names = (git(root, "diff", "--name-only", "HEAD") or "").splitlines()
    names += (git(root, "ls-files", "-o", "--exclude-standard") or "").splitlines()
    recent = []
    for name in names:
        try:
            if now - (root / name).stat().st_mtime < LIVE:
                recent.append(name)
        except OSError:
            continue
    out.append(f"edited in the last {LIVE // 60}m: {' '.join(recent[:5]) or 'none'}")
    return out


def stat_fields(pid):
    """/proc/<pid>/stat's fields after the command name: index 0 is field 3, the state."""
    try:
        return (PROC / str(pid) / "stat").read_text().rsplit(")", 1)[1].split()
    except (OSError, IndexError):
        return None


def parent_of(pid):
    fields = stat_fields(pid)
    return int(fields[1]) if fields else None


def read(path):
    try:
        return path.read_text().strip()
    except OSError:
        return None


def sessions_section(root):
    out = ["== sessions"]
    if not (PROC / "self").exists():
        return out + ["other agent sessions in this tree: unavailable (no /proc)",
                      "perf capture running: unavailable (no /proc)"]
    mine, pid = set(), os.getpid()
    while pid and pid > 1 and pid not in mine:
        mine.add(pid)
        pid = parent_of(pid)
    try:
        uptime = float((PROC / "uptime").read_text().split()[0])
    except (OSError, ValueError, IndexError):
        uptime = None
    ticks = os.sysconf("SC_CLK_TCK")
    others, captures = [], []
    for entry in sorted(PROC.iterdir(), key=lambda p: (len(p.name), p.name)):
        if not entry.name.isdigit() or int(entry.name) in mine:
            continue
        try:
            argv = (entry / "cmdline").read_bytes().split(b"\0")
        except OSError:
            continue
        args = [a.decode(errors="replace") for a in argv if a]
        if args and any(a.endswith("scripts/perf.py") for a in args[:3]):
            captures.append(f"pid {entry.name}: {' '.join(args)[:120]}")
        name = read(entry / "comm")
        if name not in ("claude", "codex"):
            continue
        # A child with its parent's name is a worker of that client, not another session.
        if read(PROC / str(parent_of(entry.name)) / "comm") == name:
            continue
        try:
            cwd = Path(os.readlink(entry / "cwd"))
        except OSError:
            continue
        if cwd != root and root not in cwd.parents:
            continue
        fields = stat_fields(entry.name)
        up = span(uptime - int(fields[19]) / ticks) if fields and uptime is not None else "?"
        others.append(f"{name} pid {entry.name} (up {up})")
    out.append(f"other agent sessions in this tree: {'; '.join(others) or 'none'}")
    out.append(f"perf capture running: {'; '.join(captures) or 'none'}")
    return out


def frozen_section(root):
    out = ["== frozen records (a banner naming D-215; not edited again)"]
    d215 = git(root, "log", "--format=%h", "-1", "-S## D-215 ", "--", "DECISIONS.md")
    out.append(f"D-215 in HEAD: {d215 or 'no'}")
    hits = 0
    files = git(root, "grep", "-lE", FROZEN_BANNER, "--", "docs", ":!docs/plans/archive")
    for name in (files or "").splitlines():
        tree = (git(root, "status", "--short", "--", name) or "")[:2].strip()
        commits = (git(root, "log", "--format=%h", f"{d215}..HEAD", "--", name) or "").split() if d215 else []
        if not tree and not commits:
            continue
        hits += 1
        out.append(f"{name}:" + (f" tree {tree}" if tree else "")
                   + (f" · commits since D-215: {' '.join(commits)}" if commits else ""))
    if not hits:
        out.append("no edits since D-215")
    return out


def roadmap_section(root):
    out = ["== ROADMAP.md"]
    try:
        lines = (root / "ROADMAP.md").read_text().splitlines()
    except OSError:
        return out + ["missing"]
    section, now_items, shown = None, 0, []
    for line in lines:
        if line.startswith("## "):
            section = line[3:].strip()
        if section in ("Now", "Next"):
            shown.append(line)
            now_items += section == "Now" and line.startswith("- ")
    while shown and not shown[-1].strip():
        shown.pop()
    return out + shown + [f"Now items: {now_items}"]


def header(text, field):
    found = re.search(rf"^- \*\*{field}:\*\*[ \t]*(.*)$", text, re.M)
    return found.group(1).strip() if found else None


def ordered_steps(text):
    """The `ordered steps` header's count, written inline as (1) … (n) or as a list below it."""
    lines = text.splitlines()
    start = next((i for i, line in enumerate(lines) if line.startswith("- **ordered steps:**")), None)
    if start is None:
        return None
    inline = re.findall(r"\((\d+)\)", lines[start])
    if inline:
        return max(map(int, inline))
    items = []
    for line in lines[start + 1:]:
        item = re.match(r"(\s+)(\d+\.|-)\s", line)
        if not item:
            break
        items.append(len(item.group(1)))
    return items.count(items[0]) if items else 0


def evidence_rows(text):
    """The first table under an Evidence heading, as dicts keyed by its lowercased header."""
    lines = text.splitlines()
    start = next((i for i, line in enumerate(lines) if re.match(r"#+ .*\bEvidence\b", line)), None)
    if start is None:
        return []
    table = []
    for line in lines[start + 1:]:
        if line.startswith("#"):
            break
        if line.startswith("|"):
            table.append([cell.strip() for cell in CELL.split(line.strip().strip("|"))])
        elif table:
            break
    if len(table) < 2:
        return []
    keys = [cell.lower() for cell in table[0]]
    return [dict(zip(keys, row)) for row in table[2:]]


def plans_section(root):
    out = ["== plans (active, outside the frozen 1.0 folder)"]
    folder = root / "docs" / "plans"
    files = sorted(folder.glob("*.md")) + sorted(
        path for path in folder.glob("*/*.md") if path.parent.name not in ("1.0", "archive"))
    files = [path for path in files if path.name != "README.md"]
    for path in files:
        text = path.read_text()
        status = header(text, "status") or "no status"
        steps = ordered_steps(text)
        rows = evidence_rows(text)
        line = (f"{path.relative_to(root).as_posix()}: {status[:60]}{'…' if len(status) > 60 else ''}"
                f" · steps: {steps if steps is not None else '?'} · evidence rows: {len(rows)}")
        if rows:
            last = rows[-1]
            line += f" · last: step {last.get('step', '?')}, {last.get('verdict', '?')}"
        out.append(line)
    if not files:
        out.append("none")
    return out


def report(root, now=None):
    root = Path(root).resolve()
    now = time.time() if now is None else now
    sections = (git_section(root, now), tree_section(root, now), sessions_section(root),
                frozen_section(root), roadmap_section(root), plans_section(root))
    return "\n".join(line for section in sections for line in section)


def main():
    root = git(Path.cwd(), "rev-parse", "--show-toplevel")
    if root is None:
        print("status: not inside a git repository", file=sys.stderr)
        return 1
    print(report(root))
    return 0


if __name__ == "__main__":
    sys.exit(main())
