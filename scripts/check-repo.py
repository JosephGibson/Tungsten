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
import tomllib
import unicodedata
from pathlib import Path
from urllib.parse import unquote, urlsplit

DOCS = (
    "README.md", "CHANGELOG.md", "docs/LLM_INDEX.md", "docs/DECISION_INDEX.md",
    "docs/agent-setup.md", "docs/perf/profiling-workflow.md", "docs/perf/benchmarks.md",
    "docs/showcase/README.md", "docs/plans/README.md", "docs/releases.md", "docs/assets.md",
    "docs/getting-started.md",
    "docs/README.md", "docs/validation.md",
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
# Owned summary fields that roadmap.derive reads from local tags and origin/main.
REF_FIELDS = ("First open stop", "Stop state")
LOCAL_LINK = re.compile(r'\]\((<[^>]+>|[^\s)]+)(?:\s+"[^"]*")?\)|^\s{0,3}\[[^\]]+\]:\s*(<[^>]+>|\S+)', re.M)
# The milestone skeleton is not a live plan; its links resolve at its eventual
# docs/plans/1.0/ destination. It is outside active_plan_documents, intentionally.

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
    # Example- and template-local assets (D-123); a tree may have no templates.
    parents = [root / "examples"]
    if (root / "templates").is_dir():
        parents.append(root / "templates")
    for parent in parents:
        roots.extend(sorted(p / "assets" for p in parent.iterdir()
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


def check_templates(root, errors, notes):
    """A template pins the root's toolchain and declares the workspace's
    rust-version, so a copy builds with the compiler the engine is tested on
    (D-069, D-123)."""
    folder = root / "templates"
    if not folder.is_dir():
        return
    toolchain = (root / "rust-toolchain.toml").read_bytes()
    workspace = tomllib.loads((root / "Cargo.toml").read_text())["workspace"]["package"]["rust-version"]
    for template in sorted(p for p in folder.iterdir() if (p / "Cargo.toml").is_file()):
        rel = template.relative_to(root).as_posix()
        pin = template / "rust-toolchain.toml"
        if not pin.is_file() or pin.read_bytes() != toolchain:
            errors.append(f"{rel}/rust-toolchain.toml: must equal the root's rust-toolchain.toml")
        version = tomllib.loads((template / "Cargo.toml").read_text()).get("package", {}).get("rust-version")
        if version != workspace:
            errors.append(f"{rel}/Cargo.toml: rust-version {version!r} differs from the workspace's {workspace!r}")


def plan_fields(text):
    # YAML frontmatter and the repository's older bold bullet headers.
    header = text.split("\n## ", 1)[0]
    result = {}
    for line in header.splitlines():
        match = re.match(r"^(?:-\s+\*\*)?([a-z -]+):(?:\*\*)?\s*(.*)$", line)
        if match:
            result[match[1]] = match[2].strip().strip('"')
    return result


def active_plan_documents(root):
    """Markdown, including folder maps, at the top level and one program deep."""
    plans = root / "docs/plans"
    # Deliberately one level deep: archive is neither listed nor opened.
    folders = [plans] + sorted(d for d in plans.iterdir()
                               if d.is_dir() and not d.is_symlink() and d.name != "archive")
    for folder in folders:
        for path in sorted(folder.glob("*.md")):
            if not path.is_symlink():
                yield path


def plan_files(root):
    return (path for path in active_plan_documents(root) if path.name != "README.md")


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


def markdown_body(text):
    """Drop fenced examples, whose links/headings are not document navigation."""
    lines, fence = [], None
    for line in text.splitlines():
        mark = re.match(r"^\s{0,3}(`{3,}|~{3,})", line)
        if mark:
            if fence is None:
                fence = mark[1]
            elif mark[1][0] == fence[0] and len(mark[1]) >= len(fence):
                fence = None
        elif fence is None:
            lines.append(line)
    return "\n".join(lines)


def markdown_anchors(text):
    """GFM heading IDs (including duplicate suffixes) and explicit HTML anchors."""
    text = markdown_body(text)
    anchors = set(re.findall(r'\b(?:id|name)=["\']([^"\']+)["\']', text))
    used = set()
    for heading in re.findall(r"^\s{0,3}#{1,6}\s+(.+?)\s*#*\s*$", text, re.M):
        heading = re.sub(r"\[([^\]]+)\]\([^)]*\)", r"\1", heading)
        slug = "".join(c for c in heading.lower() if c in " -_" or unicodedata.category(c)[0] in "LN").replace(" ", "-")
        anchor, n = slug, 0
        while anchor in used:
            n += 1
            anchor = f"{slug}-{n}"
        used.add(anchor)
        anchors.add(anchor)
    return anchors


def check_local_links(root, path, text, errors, notes, anchors):
    rel = path.relative_to(root).as_posix()
    for match in LOCAL_LINK.finditer(markdown_body(text)):
        target = (match[1] or match[2]).strip("<>")
        url = urlsplit(target)
        if url.scheme or url.netloc:
            continue
        dest = Path(os.path.normpath(path.parent / unquote(url.path))) if url.path else path
        if archive_path(os.path.relpath(dest, root)):
            notes.append(f"archive link left unread: {rel}: {target}")
            continue
        if not dest.exists():
            errors.append(f"{rel}: broken link {target}")
        elif url.fragment and dest.suffix == ".md":
            if dest not in anchors:
                anchors[dest] = markdown_anchors(text if dest == path else dest.read_text())
            if unquote(url.fragment) not in anchors[dest]:
                errors.append(f"{rel}: broken fragment {target}")


def check_docs(root, errors, notes):
    decisions = set()
    with (root / "DECISIONS.md").open() as source:
        for line in source:
            match = re.match(r"^## (D-\d{3})\b", line)
            if match:
                decisions.add(match[1])
    docs = sorted(set(DOCS) | {p.relative_to(root).as_posix() for p in active_plan_documents(root)})
    anchors = {}
    for rel in docs:
        path = root / rel
        text = path.read_text()
        for decision in set(re.findall(r"\bD-\d{3}\b", text)) - decisions:
            errors.append(f"{rel}: unknown decision {decision}")
        check_local_links(root, path, text, errors, notes, anchors)
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


def has_release_refs(root):
    """Whether the checkout holds origin/main unshallowed, which REF_FIELDS derive from."""
    git = roadmap.Git(root)
    return (bool(git.run("rev-parse", "--verify", "--quiet", "refs/remotes/origin/main"))
            and git.run("rev-parse", "--is-shallow-repository") == "false")


def check_summary(root, errors, notes):
    if not (root / roadmap.CATALOG).is_file():
        return
    rel = roadmap.SUMMARY_DOC
    text = (root / rel).read_text()
    if text.count(roadmap.SUMMARY_START) != 1 or text.count(roadmap.SUMMARY_END) != 1:
        errors.append(f"{rel}: expected one owned roadmap summary block; run python3 -B scripts/roadmap.py summary")
        return
    if text.index(roadmap.SUMMARY_END) < text.index(roadmap.SUMMARY_START):
        errors.append(f"{rel}: owned roadmap summary end precedes its start")
        return
    block = text.split(roadmap.SUMMARY_START, 1)[1].split(roadmap.SUMMARY_END, 1)[0]
    lines = re.findall(r"^- \*\*([^*]+):\*\* `([^`]+)`$", block, re.M)
    expected = roadmap.summary_fields(roadmap.derive(root))
    if len(lines) != len(expected) or set(dict(lines)) != set(expected):
        errors.append(f"{rel}: owned roadmap summary needs exactly {', '.join(expected)}")
        return
    refs = has_release_refs(root)
    for field, value in lines:
        if value != expected[field] and (refs or field not in REF_FIELDS):
            errors.append(f"{rel}: stale owned field {field!r}: {value!r}, expected {expected[field]!r}; run python3 -B scripts/roadmap.py summary")
    if refs:
        notes.append("roadmap summary uses local refs only; inspect roadmap.py status's fetch freshness before publication claims")
    else:
        notes.append(f"roadmap summary: no unshallowed origin/main (CI's checkout), so {' and '.join(REF_FIELDS)} were not compared")


def changed_paths(root, base=None):
    """Compare a commit to the work tree, plus untracked files; no fetch/writes.

    Disable rename detection so both old and new locations contribute checks.
    NUL framing preserves whitespace. An unborn repository has no HEAD yet.
    """
    def git(*args, allow_failure=False):
        result = subprocess.run(["git", "-C", str(root), *args], capture_output=True,
                                env={**os.environ, "GIT_OPTIONAL_LOCKS": "0"})
        if result.returncode and not allow_failure:
            raise ValueError(result.stderr.decode(errors="replace").strip() or "Git query failed")
        return result

    top = git("rev-parse", "--show-toplevel").stdout.decode().strip()
    if Path(top).resolve() != root.resolve():
        raise ValueError("--root must be the Git work-tree root")
    ref = git("rev-parse", "--verify", "--end-of-options", f"{base or 'HEAD'}^{{commit}}",
              allow_failure=base is None)
    if ref.returncode:
        chunks = [git("diff", "--cached", "--name-only", "--no-renames", "-z", "--").stdout,
                  git("diff", "--name-only", "--no-renames", "-z", "--").stdout]
    else:
        chunks = [git("diff", ref.stdout.decode().strip(), "--name-only", "--no-renames", "-z", "--").stdout]
    chunks.append(git("ls-files", "--others", "--exclude-standard", "-z").stdout)
    return sorted({os.fsdecode(path) for chunk in chunks for path in chunk.split(b"\0") if path})


def normalize_paths(root, paths):
    result = set()
    for supplied in paths:
        path = Path(os.path.normpath(root / supplied))
        try:
            rel = path.relative_to(root).as_posix()
        except ValueError as exc:
            raise ValueError(f"path outside repository: {supplied}") from exc
        if rel == ".":
            raise ValueError("supply changed paths, not the repository root")
        result.add(rel)
    return sorted(result)


def select_checks(root, paths):
    """Conservative path routing for docs/validation.md; never executes checks.

    Source paths cannot prove an API, visual or performance effect. Those choices
    remain explicit review notes rather than invented completion evidence.
    """
    commands, review, unknown = {}, set(), []

    def add(command, kind, reason):
        item = commands.setdefault(command, {"kind": kind, "reasons": set()})
        item["reasons"].add(reason)

    if paths:
        add("just check", "CPU final", "final gate for substantial work; fmt, strict clippy, workspace tests/doctests")
        add("git diff --check", "CPU final", "patch whitespace")
    for path in paths:
        matched = False

        def require(command, kind, why):
            nonlocal matched
            matched = True
            add(command, kind, f"{path}: {why}")

        if path.endswith(".md"):
            require("just ctx", "CPU", "instruction/navigation links and budgets")
            require("just repo-check", "CPU", "maintained and active-plan links, headers, status, manifest/index QA")
            if path not in DOCS and not path.startswith("docs/plans/"):
                review.add(f"{path}: check touched local links/anchors manually; not all Markdown is in repository QA")
            continue
        if path.startswith(("scripts/", ".github/workflows/")) or path == "justfile":
            require("just script-test", "CPU", "script regressions, ShellCheck and workflow lint")
            require("just repo-check", "CPU", "repository/tooling consistency")
            require("just ctx", "CPU", "tooling/documentation routes")
        if path.startswith((".claude/", ".agents/")) or path in (".ignore", ".gitignore"):
            require("just ctx", "CPU", "shared instructions/skill routes and filters")
            require("just repo-check", "CPU", "agent configuration and repository consistency")
        if path == "docs/plans/1.0/roadmap.json":
            require("just repo-check", "CPU", "catalog/register/owned summary consistency")
        if path.endswith("Cargo.toml") or path in ("Cargo.lock", "rust-toolchain.toml", ".cargo/config.toml", "deny.toml"):
            require("just deps", "CPU", "dependency/toolchain/build policy")
            require("just repo-check", "CPU", "manifest/template/version consistency")
            require("just smoke", "Hardware", "dependency/build changes affect runtime wiring; local GPU/display")
            review.add("Dependency/toolchain changes: review API snapshots and release/template requirements in docs/validation.md.")
        asset = "/assets/" in f"/{path}" or path.endswith("manifest.json")
        if asset:
            require("cargo test -p tungsten-core --test manifests --locked", "Edit loop", "layer 1 manifest/asset graph")
            require("just repo-check", "CPU", "asset coverage")
            require("just smoke", "Hardware", "layer 2 asset upload/runtime integration; local GPU/display")
        platformer = "examples/01_platformer/"
        if path.startswith(platformer + "tools/") or path == platformer + "src/level_layout.rs":
            require("just level-check", "CPU", "generator source/inventory/output consistency and tests; Pillow required")
            require("cargo test -p example-01-platformer --locked", "Edit loop", "generated layout and real route/gameplay tests")
            require("just repo-check", "CPU", "generated asset coverage")
            require("cargo test -p tungsten-core --test manifests --locked", "Edit loop", "generated manifest graph")
            require("just smoke", "Hardware", "generated level/art runtime wiring; local GPU/display")
            review.add("Platformer art/layout/audio: run the authoring guide's applicable route, motion, resize and audible review; smoke is startup evidence only.")
        elif path.startswith(platformer + "assets/"):
            # A deleted output can still be named in the inventory; a removed inventory
            # cannot prove ownership, so recommend the consistency check conservatively.
            inventory = root / platformer / "tools/outputs.json"
            generated = json.loads(inventory.read_text()) if inventory.is_file() else None
            if generated is not None and (not isinstance(generated, list) or not all(isinstance(p, str) for p in generated)):
                raise ValueError(f"{platformer}tools/outputs.json: expected an array of output paths")
            if generated is None or path[len(platformer):] in generated:
                require("just level-check", "CPU", "generator-owned output (or missing ownership inventory)")
                require("cargo test -p example-01-platformer --locked", "Edit loop", "generated layout/gameplay integration")
            review.add("Platformer art/layout/audio: review applicable runtime appearance and sound through the authoring guide.")
        crate = re.match(r"crates/(tungsten(?:-core|-render)?)/(src|tests|benches)/", path)
        if crate:
            name, folder = crate.groups()
            require(f"cargo test -p {name} --locked", "Edit loop", "targeted crate tests")
            if folder == "src":
                require("just smoke", "Hardware", "engine runtime wiring; local GPU/display")
                review.add("Public API may change: inspect the diff; regenerate with just api when needed, run just api-check and update the W4 ledger/template for breaks or authoring changes.")
            if name == "tungsten-render" and folder == "src":
                require("just visual", "Hardware", "renderer pixels on the reference machine; requires visual switch")
                review.add("Renderer/frame work: inspect the affected perf rows and milestone requirements; captures follow docs/perf/profiling-workflow.md.")
        if path.startswith("crates/tungsten-core/src/physics/") or re.match(r"crates/tungsten-core/(?:tests|benches)/physics", path):
            require("just physics-release", "CPU release", "solver determinism/containment are ignored in debug; also tunneling")
        if (path.startswith(("crates/tungsten-core/src/assets/", "crates/tungsten/src/asset_loader/"))
                or path in ("crates/tungsten-core/src/components.rs", "crates/tungsten-core/src/text.rs", "crates/tungsten/src/extract.rs")
                or re.match(r"crates/tungsten/src/\w+_extract.rs$", path)):
            require("cargo test -p tungsten-core --test manifests --locked", "Edit loop", "core/render asset and extraction seam")
            require("just repo-check", "CPU", "seam/manifest consistency")
        if (path.startswith(("crates/tungsten-core/src/physics/", "crates/tungsten-core/src/ecs/"))
                or path in ("crates/tungsten/src/app.rs", "crates/tungsten-core/src/time.rs", "crates/tungsten-core/src/schedule.rs")
                or re.match(r"crates/tungsten/src/\w+_extract.rs$", path)):
            review.add("Hot ECS/physics/frame/extract work: select affected perf rows with the profiling guide; shared ECS/frame-loop milestone work captures every CPU row. No captures run here.")
        if path.endswith(".wgsl"):
            require("cargo test -p tungsten-render --test shader_coverage --locked", "Edit loop", "Naga validation and byte-equal shader mirrors")
            require("just smoke", "Hardware", "shader/pipeline fixtures; local GPU/display")
            require("just visual", "Hardware", "pixel comparisons; Naga success does not prove pixels")
            review.add("Shaders: edit stock mirrors together; exercise affected reload paths and explain intentional pixel differences before changing goldens.")
        example = re.match(r"examples/(\d{2}_[^/]+)/(src|tests|benches)/", path)
        if example:
            package = "example-" + example[1].replace("_", "-")
            require(f"cargo test -p {package} --locked", "Edit loop", "example's targeted behavior tests")
            if example[2] == "src":
                require("just smoke", "Hardware", "example runtime wiring; local GPU/display")
            if "regression" in path:
                require("just visual", "Hardware", "visual fixture changes need actual pixel comparisons")
        if path.startswith("templates/basic/"):
            require("cargo test -p tungsten-template-basic --locked", "Edit loop", "template behavior/wiring")
            require("just repo-check", "CPU", "template assets and toolchain pins")
            require("just template-check", "Hardware", "outside-copy build/test/smoke; local GPU/display and disk floor")
            require("just smoke", "Hardware", "template startup integration")
        if path.startswith("api/"):
            require("just api-check", "CPU", "tracked public API agrees with pinned-toolchain rustdoc")
            review.add("Public API: review additions/breaks, W4 ledger, rustdoc and template authoring examples; milestone level C review still applies.")
        if path in ("tungsten.json", "input.json") or re.match(r"examples/[^/]+/(tungsten|input)\.json$", path):
            require("just smoke", "Hardware", "engine/example configuration wiring")
        if Path(path).suffix in (".wav", ".ogg", ".mp3", ".flac") or path in ("crates/tungsten/src/audio.rs", "crates/tungsten-core/src/assets/audio.rs"):
            review.add("Audio changes: run applicable decode tests and audible review locally; a CPU or startup pass cannot certify sound.")
        if path.startswith(("licenses/", "tools/")) or path == "LICENSE":
            review.add(f"{path}: inspect release/notices/probe requirements in docs/releases.md.")
            # Release tooling not recognized by a more specific rule stays visible.
        if not matched:
            unknown.append(path)
    # A whole-crate edit-loop run includes its integration tests. Keep all the
    # reasons on the broader command instead of recommending duplicate runs.
    for package, integration in (("tungsten-core", "manifests"), ("tungsten-render", "shader_coverage")):
        full = f"cargo test -p {package} --locked"
        targeted = f"cargo test -p {package} --test {integration} --locked"
        if full in commands and targeted in commands:
            commands[full]["reasons"].update(commands.pop(targeted)["reasons"])
    return commands, sorted(review), unknown


def print_selection(root, paths):
    commands, review, unknown = select_checks(root, paths)
    print("Check recommendations only — no checks executed, no results certified.")
    print("Source: explicit paths or Git work-tree comparison; renamed/deleted paths are classified without opening them.")
    if not paths:
        print("No changed paths selected. Milestone/release gates still apply independently.")
        return
    for kind in ("Edit loop", "CPU", "CPU release", "CPU final", "Hardware"):
        items = [(command, item) for command, item in commands.items() if item["kind"] == kind]
        if items:
            print(f"\n{kind}" + (" (not run; local prerequisites apply)" if kind == "Hardware" else " (not run)") + ":")
            for command, item in items:
                print(f"  {command}")
                for reason in sorted(item["reasons"]):
                    print(f"    {reason}")
    for note in review:
        print(f"REVIEW: {note}")
    for path in unknown:
        print(f"UNCLASSIFIED: {path}; inspect docs/validation.md and task-specific requirements before claiming coverage.")
    print("\nVisual comparisons are NOT RUN by CPU tests without TUNGSTEN_VISUAL_REGRESSION=1; only actual pixel runs provide visual evidence.")
    print("Apply additional milestone/release checks from their canonical guides; this selector cannot infer behavior from paths.")


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parent.parent)
    parser.add_argument("--select-checks", action="store_true", help="print recommendations only; paths or Git changes including untracked files")
    parser.add_argument("--base", help="compare this commit/ref directly to the work tree (default HEAD); no fetch")
    parser.add_argument("paths", nargs="*", help="explicit repository-relative paths, including deleted files")
    args = parser.parse_args(argv)
    if (args.base or args.paths) and not args.select_checks:
        parser.error("paths and --base require --select-checks")
    if args.base and args.paths:
        parser.error("use explicit paths or --base, not both")
    errors, notes = [], []
    root = args.root.resolve()
    if args.select_checks:
        try:
            paths = normalize_paths(root, args.paths or changed_paths(root, args.base))
            print_selection(root, paths)
        except (OSError, ValueError) as exc:
            print(f"ERROR: check selection: {exc}")
            return 2
        return 0
    for check in (check_assets, check_templates, check_plans, check_docs, check_plan_citations, check_roadmap, check_summary):
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
