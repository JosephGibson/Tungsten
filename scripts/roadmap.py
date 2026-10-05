#!/usr/bin/env python3
"""Road-to-1.0 catalog and stop status for the roadmap page (stdlib only, read-only).

`catalog OUT` writes the page's `meta/catalog` document (`{schema, hash, catalog}`) to OUT: the
catalog plus each stop's stages, read from the Flow table in tungsten-next's `prompts.md`, and a mode,
model and effort for each session stage (`D-118`).
`status [--json OUT]` derives each stop's status from the tree: the register, plan files,
approval lines, evidence rows, tags and the local `origin/main` (never fetched), and prints
the evidence for each with the next release and milestone. `scripts/check-repo.py` imports
the register parsers, so both read the register one way.
"""

import argparse
import hashlib
import json
import os
import re
import subprocess
import sys
import time
from pathlib import Path

PROGRAM = "docs/plans/1.0"
CATALOG = f"{PROGRAM}/roadmap.json"
PLAN = f"{PROGRAM}/implementation-plan.md"
CRITERIA = f"{PROGRAM}/criteria.md"
PROMPTS = ".claude/skills/tungsten-next/prompts.md"
SCHEMA = 2  # the catalog shape `catalog` writes and the page reads
REGISTER_FIELDS = ("candidate", "plan", "release", "status")
FIRST_MILESTONE = 32  # docs/plans/1.0/README.md, Conventions

# Session recommendations (D-118). A stop's tier comes from its complexity and effort; the scaled
# steps take it, the fixed ones never vary, a same-session step keeps the step that opened its session.
MODELS = {"opus": "Opus 5.5", "fable": "Fable 5.1"}
SCALED = ("plan", "qa-plan", "gate", "run", "run-c1", "experiment", "rc-fix")
FIXED = {"game-spec": ("opus", "max"), "graduate": ("opus", "xhigh"), "rc-checklist": ("opus", "xhigh"),
         "release": ("opus", "xhigh"), "verify": ("opus", "high")}
PLAN_MODE = ("gate", "game-spec")  # they ask before they write
KEEPS = {"approve": ("plan", "qa-plan"), "run-c-rest": ("run-c1",)}
SESSIONS = ("release", "verify")  # session prompts outside every flow
FLOW_STEP = re.compile(r"`([\w-]+)` \((new|same|you), ([^;)]+?)(?:; ([^)]*))?\)")
GATE_ONLY = re.compile(r"\bthe ([\w-]+) gate only\b")


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError(f"duplicate JSON key: {key}")
        result[key] = value
    return result


def load_catalog(root):
    return json.loads((Path(root) / CATALOG).read_text(), object_pairs_hook=unique_object)


def section(text, number):
    """The `## <number>. …` section of a Markdown file, up to the next `## ` heading."""
    match = re.search(rf"^## {re.escape(str(number))}\. .*?(?=^## |\Z)", text, re.M | re.S)
    return match[0] if match else ""


def tables(text):
    """Each Markdown table in `text` as rows of cells, header first, rule row dropped."""
    result, current = [], []
    for line in text.splitlines() + [""]:
        if line.startswith("|"):
            cells = [cell.strip() for cell in line.strip().strip("|").split("|")]
            if not all(re.fullmatch(r":?-+:?", cell) for cell in cells):
                current.append(cells)
        elif current:
            result.append(current)
            current = []
    return result


def register(root):
    """Implementation plan §10, one dict per row: candidate, plan, release, status."""
    for table in tables(section((Path(root) / PLAN).read_text(), 10)):
        if table[0][:1] == ["Candidate"]:
            return [dict(zip(REGISTER_FIELDS, row)) for row in table[1:]]
    return []


def card_levels(root):
    """Implementation plan §3 cards: Candidate cell -> Level cell."""
    for table in tables(section((Path(root) / PLAN).read_text(), 3)):
        header = table[0]
        if header[:1] == ["Candidate"] and header[-1:] == ["Level"]:
            return {row[0]: row[-1] for row in table[1:]}
    return {}


def question_numbers(root):
    """Question numbers in implementation plan §7 (`Q<n>`) and criteria §10 (`Q<n>` or `**<n>.**`)."""
    plan = section((Path(root) / PLAN).read_text(), 7)
    criteria = section((Path(root) / CRITERIA).read_text(), 10)
    found = re.findall(r"\bQ(\d+)\b", plan + criteria) + re.findall(r"^- \*\*(\d+)\.\*\*", criteria, re.M)
    return {int(n) for n in found}


def flows(root):
    """prompts.md's Flow table: row label -> steps `{key, title, where, at, note}`, titled from the
    Session prompts table and the Owner steps headings."""
    text = (Path(root) / PROMPTS).read_text()
    titles, rows = {}, {}
    for table in tables(text):
        if table[0][:2] == ["Key", "Step"]:
            into, quote = titles, "`"
        elif table[0][:1] == ["Stop"]:
            into, quote = rows, ""
        else:
            continue
        for row in table[1:]:
            if len(row) < 2:
                raise ValueError(f"{PROMPTS}: table row {row!r} has fewer than two cells")
            into[row[0].strip(quote)] = row[1]
    owner = text.split("\n## Owner steps", 1)[-1] if "\n## Owner steps" in text else ""
    titles.update({key: title for title, key in re.findall(r"^\*\*([^*]+)\*\* \(`([\w-]+)`\)", owner, re.M)})
    result = {}
    for label, cell in rows.items():
        steps = []
        for part in cell.split(" → "):
            match = FLOW_STEP.fullmatch(part.strip())
            if not match:
                raise ValueError(f"{PROMPTS}: Flow row {label!r}: can't read {part.strip()!r}")
            key, where, at, note = match.groups()
            if key not in titles:
                raise ValueError(f"{PROMPTS}: Flow row {label!r}: no Step title for {key!r}")
            step = {"key": key, "title": titles[key], "where": where, "at": re.split(r"\s+or\s+|\s*,\s*", at.strip())}
            if note:
                step["note"] = note.strip()
            steps.append(step)
        result[label] = steps
    return result


def flow_row(stop):
    """The Flow row a stop runs: by kind, and level C apart for releases; None for a custom stop."""
    kind = stop["kind"]
    if kind == "release":
        return "Release, level C" if stop.get("level") == "C" else "Release, level A or B"
    rows = {"qa": "QA pass", "gate": "Gate", "spike": "Experiments (Track B)", "rc": "Release candidates (C3)"}
    if kind in rows:
        return rows[kind]
    if kind == "custom":
        return None
    raise ValueError(f"{stop['id']}: no Flow row for kind {kind!r}")


def tier(stop):
    """(model, effort) from complexity c and effort e: Fable for large architectural stops (c 4, or 3
    and XL), Opus max for c 2–3, Opus high for very small ones (c 1 and S), else the Opus xhigh default."""
    c, e = stop.get("complexity"), stop.get("effort")
    if c == 4 or (c == 3 and e == "XL"):
        return "fable", "xhigh"
    if c in (2, 3):
        return "opus", "max"
    if c == 1 and e == "S":
        return "opus", "high"
    return "opus", "xhigh"


def recommend(stop, key, opened=()):
    """`{mode, model, effort}` for a session step, `opened` holding the stop's earlier stages; a
    same-session step adds `keeps` and `if_new`. None for owner steps and steps without a rule."""
    def rec(model, effort):
        return {"mode": "plan" if key in PLAN_MODE else "auto", "model": MODELS[model], "effort": effort}
    if key in SCALED:
        return rec(*tier(stop))
    if key in FIXED:
        return rec(*FIXED[key])
    if key in KEEPS:
        opener = next((s for s in reversed(opened) if s["key"] in KEEPS[key] and "rec" in s), None)
        if opener is None:
            raise ValueError(f"{stop['id']}: {key} has no earlier {' or '.join(KEEPS[key])} step to keep")
        kept = {k: opener["rec"][k] for k in ("mode", "model", "effort")}
        return {"keeps": opener["key"], **kept, "if_new": rec("opus", "high") if key == "approve" else dict(kept)}
    return None


def stages(stop, flow):
    """The stop's steps from its Flow row, each session step with its recommendation."""
    label = flow_row(stop)
    if label is None:
        return []
    if label not in flow:
        raise ValueError(f"{stop['id']}: {PROMPTS} has no Flow row {label!r}")
    result = []
    for step in flow[label]:
        only = GATE_ONLY.search(step.get("note", ""))
        if only and f"{only[1]} gate" not in (stop.get("gate") or ""):
            continue
        stage = dict(step)
        rec = recommend(stop, step["key"], result)
        if rec:
            stage["rec"] = rec
        result.append(stage)
    return result


def catalog_payload(root):
    """The `meta/catalog` document: the catalog, each stop's stages and the session recommendations
    outside every flow; the hash covers the whole catalog's canonical JSON, derived fields included."""
    catalog = load_catalog(root)
    if catalog.get("schema") != SCHEMA:
        raise ValueError(f"{CATALOG}: schema {catalog.get('schema')!r}, scripts/roadmap.py writes {SCHEMA}")
    flow = flows(root)
    for stop in catalog["stops"]:
        stop["stages"] = stages(stop, flow)
    catalog["sessions"] = {key: recommend(None, key) for key in SESSIONS}
    canonical = json.dumps(catalog, sort_keys=True, separators=(",", ":"), ensure_ascii=False)
    return {"schema": catalog["schema"], "hash": hashlib.sha256(canonical.encode()).hexdigest()[:16],
            "catalog": catalog}


class Git:
    """Read-only queries; an empty answer when git or the ref is missing."""

    def __init__(self, root):
        self.root = Path(root)
        self._main = None

    def run(self, *args):
        try:
            return subprocess.run(["git", "-C", str(self.root), *args], capture_output=True, text=True,
                                  check=True, env={**os.environ, "GIT_OPTIONAL_LOCKS": "0"}).stdout.strip()
        except (OSError, subprocess.CalledProcessError):
            return ""

    def branch(self):
        return self.run("branch", "--show-current")

    def main_subjects(self):
        if self._main is None:
            self._main = self.run("log", "--format=%s", "refs/remotes/origin/main").splitlines()
        return self._main

    def on_main(self, release):
        return any(s.startswith(f"Update {release}:") for s in self.main_subjects())

    def tag_time(self, release):
        """Unix time of tag v<release>.0, or None when there is no such tag."""
        out = self.run("for-each-ref", "--format=%(creatordate:unix)", f"refs/tags/v{release}.0")
        return int(out) if out else None

    def fetch_time(self):
        rel = self.run("rev-parse", "--git-path", "FETCH_HEAD")
        path = self.root / rel if rel else None
        return path.stat().st_mtime if path and path.is_file() else None


def next_release(root, git):
    """The current branch's `0.NN`, else Cargo.toml's minor plus one."""
    branch = git.branch()
    if re.fullmatch(r"0\.\d+", branch):
        return branch
    match = re.search(r'^version\s*=\s*"0\.(\d+)\.\d+"', (Path(root) / "Cargo.toml").read_text(), re.M)
    return f"0.{int(match[1]) + 1}" if match else None


def milestone_number(text):
    match = re.search(r"milestone-(\d+)-|\bM(\d+)\b", text)
    return int(match[1] or match[2]) if match else None


def next_milestone(rows):
    numbers = [n for n in (milestone_number(row["plan"]) for row in rows) if n is not None]
    return f"M{max(numbers) + 1 if numbers else FIRST_MILESTONE}"


def plan_file(root, cell):
    """The milestone plan a register cell names, as a repo path, and whether it exists."""
    match = re.search(r"([\w./-]+\.md)", cell)
    if not match:
        return None, False
    name = match[1]
    rel = name if name.startswith("docs/") else f"{PROGRAM}/{Path(name).name}"
    if "/archive/" in rel:  # released plans; never opened
        return rel, False
    return rel, (Path(root) / rel).is_file()


def plan_progress(text):
    """(approved, step count, steps with an evidence row), the patterns state.sh uses."""
    approved = re.search(r"^Approved", text, re.M) is not None
    steps = len(re.findall(r"^### Step ", text, re.M))
    log = re.search(r"^## Evidence.*?(?=^## |\Z)", text, re.M | re.S)
    rows = re.findall(r"^\| *(?:Step +)?(\d+) *\|", log[0], re.M) if log else []
    return approved, steps, len(set(rows))


def gate_record(plan_text, name):
    """The `### <Name>, <date>` record under implementation plan §11, or ''."""
    records = section(plan_text, 11)
    match = re.search(rf"^### {re.escape(name)}, .*?(?=^##|\Z)", records, re.M | re.S | re.I)
    return match[0] if match else ""


def release_state(git, release, fetched):
    """(status, why) from tags and origin/main, or None when the release is neither tagged nor merged."""
    if git.on_main(release):
        return "done", f'origin/main has "Update {release}:"'
    tagged = git.tag_time(release)
    if tagged is None:
        return None
    if fetched is None or fetched < tagged:
        return "review", f"tag v{release}.0, not on origin/main; unverified: fetch older than the tag"
    return "review", f"tag v{release}.0, not on origin/main as of the last fetch"


def derive(root):
    root = Path(root)
    git = Git(root)
    catalog = load_catalog(root)
    plan_text = (root / PLAN).read_text()
    rows = {row["candidate"]: row for row in register(root)}
    fetched = git.fetch_time()
    stops = []
    for stop in catalog["stops"]:
        doc = {"status": "todo", "release": stop.get("release"), "milestone": None, "plan": None,
               "plan_steps": None, "evidence": None}
        why = []
        if stop["kind"] == "gate":
            names = [n.strip() for n in re.split(r" and (?:the )?", stop["gate"])]
            records = [gate_record(plan_text, n) for n in names]
            signed = [bool(re.search(r"^Sign-off: Signed", r, re.M)) for r in records]
            if all(signed):
                doc["status"] = "done"
                branch = re.search(r"branch `(0\.\d+)`", records[0])
                doc["release"] = branch[1] if branch else None
            why.append("; ".join(f"{n}: {'signed' if s else 'record unsigned' if r else 'no record'}"
                                 for n, r, s in zip(names, records, signed)))
        elif stop["kind"] == "spike":
            pass  # second pass: done once the gate its verdicts feed is
        else:
            row = rows.get(stop.get("row")) if stop.get("row") else None
            if row:
                why.append(f"register {row['candidate']!r}")
                cell = re.search(r"\b0\.\d+\b", row["release"])
                doc["release"] = cell[0] if cell else doc["release"]
                path, exists = plan_file(root, row["plan"])
                if path:
                    doc["plan"] = path
                    number = milestone_number(path)
                    doc["milestone"] = f"M{number}" if number else None
            else:
                path, exists = None, False
            state = release_state(git, doc["release"], fetched) if doc["release"] else None
            if state:
                doc["status"] = state[0]
                why.append(state[1])
            elif row and doc["release"] and re.search(r"\bcut\b", row["status"], re.I):
                doc["status"] = "ready"
                why.append(f"cut for {doc['release']}, tag v{doc['release']}.0 none")
            if exists:
                approved, steps, evidence = plan_progress((root / path).read_text())
                doc["plan_steps"], doc["evidence"] = steps, evidence
                if doc["status"] == "todo":
                    doc["status"] = "run" if approved else "plan"
                why.append(f"{path}: {'approved' if approved else 'not approved'}, "
                           f"{evidence} of {steps} steps with evidence")
            elif path:
                why.append(f"{path} not in the tree")
            elif doc["status"] == "todo":
                why.append("no milestone plan" if row else "no register row")
        stops.append({"id": stop["id"], **doc, "why": "; ".join(why)})
    for i, stop in enumerate(catalog["stops"]):
        if stop["kind"] == "spike":
            gate = next((stops[j] for j in range(i + 1, len(stops)) if catalog["stops"][j]["kind"] == "gate"), None)
            if gate and gate["status"] == "done":
                stops[i]["status"] = "done"
            stops[i]["why"] = f"verdicts feed {gate['id']}, {gate['status']}" if gate else "no gate after it"
    current =next((s["id"] for s in stops if s["status"] not in ("done", "skip")), None)
    return {
        "branch": git.branch(), "release": next_release(root, git), "milestone": next_milestone(rows.values()),
        "fetched": time.strftime("%Y-%m-%d %H:%M", time.localtime(fetched)) if fetched else None,
        "stop": current, "stops": stops,
    }


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parent.parent)
    sub = parser.add_subparsers(dest="command", required=True)
    sub.add_parser("catalog", help="write the meta/catalog document").add_argument("out", type=Path)
    status = sub.add_parser("status", help="derive each stop's status from the tree")
    status.add_argument("--json", type=Path, help="also write the result as JSON")
    args = parser.parse_args(argv)
    if args.command == "catalog":
        payload = catalog_payload(args.root)
        text = json.dumps(payload, ensure_ascii=False, separators=(",", ":"))
        args.out.write_text(text)
        print(f"catalog hash {payload['hash']} · {len(text.encode())} bytes · {args.out}")
        return 0
    state = derive(args.root)
    print(f"branch {state['branch'] or '(detached)'} · next release {state['release']} · "
          f"next milestone {state['milestone']} · last fetch {state['fetched'] or 'never'} · "
          f"current stop {state['stop']}")
    for s in state["stops"]:
        print(f"{s['id']:<8} {s['status']:<7} {s['release'] or '-':<5} {s['why']}")
    if args.json:
        args.json.write_text(json.dumps(state, ensure_ascii=False, indent=1))
    return 0


if __name__ == "__main__":
    sys.exit(main())
