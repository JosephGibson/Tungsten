#!/usr/bin/env python3
"""List the files outside a plan's files to touch that its steps reach (`just preflight <plan>`).

A file is listed when it names what the plan changes (a backticked type, system, event, kind,
ID, field, file stem or cited path from a step and the design sections and audit rows it cites,
or an item defined at a line range it cites in a file it touches); when it holds a value derived
from a source the step's list touches (a size, count, hash or snapshot: PINS below, keyed by
the crates and folders that feed it); or when a category rule ties it to what the step does
(RULES). Each pin and rule names the stops it came from. The hits print in groups: guard pins,
coverage counts, describe and genre tests, 01's names, the template, the rest. A list entry the
plan conditions excludes nothing (its files stay listed, marked), and one that resolves to no
path is reported.

Read-only and offline; it writes nothing. Standard library only: the tree at --root as
`git ls-files` lists it (a walk without .git), never target/, .git/, perf-runs/ at the root or
docs/plans/archive/. rg, when present, only narrows the files read; the result is the same.

Usage: preflight.py PLAN [--step N] [--root DIR] [--json]
"""

import argparse
import bisect
import json
import os
import re
import shlex
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
ARCHIVE = "docs/plans/archive"
SKIP_ANY = frozenset({".git", "target"})
SKIP_TOP = frozenset({"perf-runs"})
# This tool and its test, whose rule table and fixtures name every pin.
SELF = frozenset({"scripts/preflight.py", "scripts/test-preflight.py"})
BINARY = frozenset(
    ".png .jpg .jpeg .gif .webp .bmp .ico .wav .ogg .mp3 .flac .ttf .otf .woff .woff2 .bin .zip .gz"
    " .xz .zst .tar .pdf .exe .dll .so .dylib .a .rlib .pyc .psd .aseprite .kra .blend".split())
# A name in more files than this outside the list is too common to carry meaning: an
# identifier (a type, a snake_case or constant name, a tag, an ID) or a plain lowercase word.
# Swept over the 34 stops (build.md): 25 is the least that keeps all 13 name catches
# (pushes-and-riders 8's `Projectile` lists 25 files); 0 to 3 for a word tie at the least noise.
MAX_NAME_FILES = 25
MAX_WORD_FILES = 3
# Lines a file shows in the text report, rule lines first; --json holds every line.
SHOWN = 4

GROUPS = ("guard pins", "coverage counts", "describe and genre tests", "01's names", "the template", "the rest")
# A file's group: the first whose path globs or line pattern (on a listed line) match it.
GROUP_PATHS = {
    "guard pins": ("**/guards/*.json", "**/guards.rs", "crates/tungsten/src/tests/kit_recipes/determinism.rs"),
    "coverage counts": ("crates/tungsten/src/tests/kit_recipes/coverage.rs", "crates/tungsten-kit/src/tests/inspect.rs",
                        "crates/tungsten-kit/src/tests/tags.rs"),
    "describe and genre tests": ("crates/tungsten-kit/src/tests/describe.rs", "crates/tungsten-kit/src/foundation/describe.rs",
                                 "crates/tungsten/src/tests/plugins.rs", "crates/tungsten/tests/fixtures/kit/**"),
    "01's names": ("examples/01_platformer/**",),
    "the template": ("templates/basic/**",),
}
GROUP_LINES = {
    "guard pins": r'size_of::<|"allocated_bytes"|"allocations"|\b[A-Z_]*HASH\b[^=\n]*=\s*0x',
    "coverage counts": r"\b(?:[A-Z][A-Z0-9_]+|kinds|registered|described|names)\b[\w.()&]*\.(?:len|count)\(\),\s*\d+\s*\)",
    "describe and genre tests": r"\.genre\(|ComponentDescription::new\(",
}

PIT = "examples/01_platformer/tests/game/guards/pit.json"
PIT_TEST = "examples/01_platformer/tests/game/guards.rs"
COUNTS = r'"allocations"|"allocated_bytes"'
# "`TextOverrides` gains `outline`", "`WorldLabel` and `TextSection` take both", "`sprite` takes":
# the subjects a step gives new fields (rule R15).
GAINS = r"`([A-Za-z_][\w:]*)`((?:\s*(?:,|and|or)\s*`[A-Za-z_][\w:]*`)*)\s+(?:gains?|takes?|gets?)\b"

# The sources that feed derived values: a path a step's list holds (a path its text only cites
# keys no pin). Crates where every source feeds the value, folders where only some do.
FEEDS = {
    "kit": r"crates/tungsten-kit/src/(?!tests/).+",
    "core": r"crates/tungsten-core/src/(?!tests/).+",
    "umbrella": r"crates/tungsten/src/(?!tests/).+",
    "render": r"crates/tungsten-render/src/(?!tests/).+",
    "01": r"examples/01_platformer/src/.+",
    "02": r"examples/02_bench/src/(?!tests/).+",
    "template": r"templates/basic/src/.+",
    "kit fixtures": r"crates/tungsten/tests/fixtures/kit/.+",
    "kit recipes": r"crates/tungsten/src/tests/kit_recipes/.+",
    "frame loop": r"crates/tungsten-core/src/(?:ecs/.+|schedule\.rs|time\.rs)|crates/tungsten/src/(?:app|testing)\.rs",
    "physics": r"crates/tungsten-core/src/physics/.+",
    "drawing": r"crates/tungsten-render/src/(?!tests/).+|crates/tungsten/src/(?:\w+_)?extract\.rs"
               r"|crates/tungsten-core/src/(?:text\.rs|ui/.+)",
    "scenes": r"crates/tungsten-core/src/(?:components\.rs|assets/scene\.rs)|crates/tungsten/src/(?:asset_loader/.+|plugins\.rs)",
    "systems": r"crates/tungsten/src/(?:plugins|app)\.rs",
}
# The derived-value pins: fixed files holding a value a change derives (a size, a count, a hash,
# a snapshot), so no name in the plan reaches them. Each is listed, at the line holding the
# value, whenever a step's list touches a source in its `feeds`. Read off the 34 stops of
# 2026-11-07 (`stops`), so the next stops are the table's test. A later step adds one entry.
PINS = (
    {"id": "P1", "name": "01's pit", "feeds": ("kit", "core", "umbrella", "render", "01"),
     "files": ({"path": PIT, "line": COUNTS}, {"path": PIT_TEST, "line": r"\bfn the_full_pit_holds_its_counts\b"}),
     "holds": "the allocations and bytes of 01's full pit, which runs the engine, the kit and 01's whole schedule",
     "stops": "ui-pass 1, 6, 12, 13; runtime-terrain 2, 3, 5; boot-and-seams 5, 7; pushes-and-riders 7; "
              "clips-facing-and-lifecycle 3"},
    {"id": "P2", "name": "every counter guard", "feeds": ("frame loop", "02"),
     "files": ({"path": "examples/02_bench/src/tests/guards/*.json", "line": COUNTS},
               {"path": "crates/tungsten/src/tests/guards/*.json", "line": COUNTS}),
     "holds": "the allocations and bytes of every 02 row and the umbrella's debug views, which all run the frame loop",
     "stops": "ui-pass 6 (every counter pin, from the frame loop's per-system names)"},
    {"id": "P3", "name": "the schedule snapshots", "feeds": ("kit", "systems", "template"),
     "files": ({"path": "crates/tungsten/src/tests/kit.rs", "line": r"\bconst (?:KIT_FIXED_UPDATE|MOTION_EDGES|EFFECT_EDGES)\b"},
               {"path": "templates/basic/tests/game/sample.rs", "line": r"\bconst SCHEDULE\b"}),
     "holds": "the resolved fixed step and its edges, and the template's resolved schedule",
     "stops": "pushes-and-riders 6, 7, 8 (kit.rs)"},
    {"id": "P4", "name": "the recipes' counts and hash", "feeds": ("kit", "kit fixtures", "kit recipes"),
     "files": ({"path": "crates/tungsten/src/tests/kit_recipes/coverage.rs",
                "line": r"\b(?:kinds|registered)\.len\(\),\s*\d+|\bfn every_registered_scene_name_and_kind_appears_in_two_genres\b"},
               {"path": "crates/tungsten/src/tests/kit_recipes/determinism.rs", "line": r"\bKIT_RECIPES_HASH: u64\b"}),
     "holds": "coverage's counts of the registered names and kinds, and the recipes' state hash",
     "stops": "statuses-and-chemistry 3 (coverage.rs:85); pushes-and-riders 6 (determinism.rs:36)"},
    {"id": "P5", "name": "the kit's counts", "feeds": ("kit",),
     "files": ({"path": "crates/tungsten-kit/src/tests/inspect.rs", "line": r"\bcount\(\),\s*\d+\s*\)"},
               {"path": "crates/tungsten-kit/src/tests/describe.rs",
                "line": r'\b(?:described|kinds)\.len\(\),\s*\d+|count\("(?:component|kind|shape) "\),\s*\d+|\bchecked,\s*\d+'},
               {"path": "crates/tungsten-kit/src/tests/tags.rs", "line": r"\bKIT_TAGS\.len\(\)"}),
     "holds": "the census of described components and kinds, the numeric described fields, KIT_TAGS' length",
     "stops": "statuses-and-chemistry 3 (tags.rs:158); boot-and-seams 6 (inspect.rs:97), 7 (describe.rs:386); "
              "abilities-and-genres 5 (inspect.rs:97)"},
    {"id": "P6", "name": "the genre fixtures", "feeds": ("kit fixtures", "kit recipes"),
     "files": ({"folder": "crates/tungsten/tests/fixtures/kit/*/prefabs/"},),
     "holds": "every registered scene name and kind in two genres' fixtures (D-172)",
     "stops": "boot-and-seams 6 (platformer/prefabs/)"},
    {"id": "P7", "name": "App's scene names", "feeds": ("scenes",),
     "files": ({"path": "crates/tungsten/src/tests/plugins.rs", "line": r"\bfn each_owner_registers_its_scene_components\b"},),
     "holds": "every scene name App registers",
     "stops": "boot-and-seams 9, 10 (plugins.rs:196, :197)"},
    {"id": "P8", "name": "the physics rows", "feeds": ("physics",),
     "files": ({"path": "examples/02_bench/src/tests/guards/physics*.json", "line": COUNTS},
               {"path": "examples/02_bench/src/tests/guards/terrain.json", "line": COUNTS},
               {"path": "examples/02_bench/src/tests/guards/friction.json", "line": COUNTS},
               {"path": "examples/02_bench/src/tests/guards/slopes.json", "line": COUNTS}),
     "holds": "the allocations and bytes of 02's physics rows, which step the physics every frame",
     "stops": "runtime-terrain 2, 3, 5 (02's physics.json, physics-sparse.json moved with 01's pit)"},
    {"id": "P9", "name": "the drawing rows", "feeds": ("drawing",),
     "files": ({"path": "examples/02_bench/src/tests/guards/gpu*.json", "line": COUNTS},
               {"path": "crates/tungsten/src/tests/guards/views-*.json", "line": COUNTS}),
     "holds": "the allocations and bytes of 02's drawing rows and the debug views, which extract and lay out text each frame",
     "stops": "ui-pass 3 (gpu.json)"},
)

# The category rules: causes no name and no derived-value pin reaches. `when` holds
# alternatives; one fires the rule when all its conditions hold:
#   phrase: a regex (any case) in the step's text, its cited sections and their audit rows
#   touch: a regex matching a path the step's list holds (a file, a new file or a folder)
#   new_file: a regex matching a path the list holds that the tree lacks
#   name: a regex matching a name the step gives; new_name: one the tree holds nowhere
#   sized: a regex matching a type the step names whose size a `size_of` assert pins
# `files` holds globs with the line that carries the pinned value (the file alone when no line
# matches), or a computation by name (COMPUTED). A later step adds a rule with one entry.
RULES = (
    # R1 kit-kinds: retired, replaced by P4 and P5 (coverage's and the census's kinds counts).
    # R2 kit-tags: retired, replaced by P5 (KIT_TAGS' length).
    # R3 pit-01-code: retired, replaced by P1 (01's pit, fed by 01's source).
    {"id": "R4", "name": "size-asserts",
     "when": ({"sized": r"[A-Z]\w*"},),
     "files": ({"computed": "size_pins"},),
     "reason": "a size_of assert pins a type the step names; a common type's name is dropped as noise, its assert is not",
     "stop": "ui-pass 1, 3, runtime-terrain 2, 3, 5, boot-and-seams 5, 7 (sizes); its pins moved to P1 and P2"},
    # R5 new-system-pins: retired, its cause gone since 4e62d3d0 (a warm frame's timings allocate nothing).
    {"id": "R6", "name": "ui-node-state",
     "when": ({"phrase": r"`UiTree::set_[a-z_]+"}, {"phrase": r"\bper-(?:node|widget)\b"}),
     "files": ({"path": "crates/tungsten-core/src/ui/node.rs", "line": r"\bstruct Node\b"},),
     "reason": "a widget's flag lives on core's Node beside bare, not on UiTree's methods",
     "stop": "ui-pass step 6 (departure): crates/tungsten-core/src/ui/node.rs:250 struct Node"},
    {"id": "R7", "name": "register-rows",
     "when": ({"name": r"(?:S|P[78]|PA)-\d+|K\d+"},),
     "files": ({"computed": "row_lines"},),
     "reason": "phase 7's tables hold each register row a step lands or moves",
     "stop": "ui-pass step 16 (departure): docs/plans/phase-7.md:222, the S-162 row"},
    {"id": "R8", "name": "domain-state",
     "when": ({"new_file": r"crates/tungsten-kit/src/(?!tests/)[^/]+/[^/]+\.rs"},),
     "files": ({"computed": "domain_siblings"},),
     "reason": "a new file in a kit domain keeps its state in the domain's resource and queue files, outside the list",
     "stop": "chemistry step 5 (departure): crates/tungsten-kit/src/chemistry/queue.rs:152 pub struct Chemistry"},
    {"id": "R9", "name": "containment",
     "when": ({"touch": r"crates/tungsten-core/src/physics/(?:collision|step/(?:pairs|gather|outline|solver|arrival))\.rs"},),
     "files": ({"path": "examples/01_platformer/tests/game/ball_pit.rs", "line": r"wall escape"},
               {"path": "crates/tungsten-core/tests/integration/physics_containment.rs", "line": r"\bHASH\b|\bfn \w*contain"}),
     "reason": "contact generation decides 01's ball pit walls and the containment hash; nothing names the change",
     "stop": "runtime-terrain step 6 (first run): examples/01_platformer/tests/game/ball_pit.rs:275 wall escape"},
    {"id": "R10", "name": "sibling-callers",
     "when": ({"touch": r"crates/[^/]+/src/(?:.+/)?[^/]+\.rs"},),
     "files": ({"computed": "sibling_importers"},),
     "reason": "a sibling module that imports from a listed module through super:: calls what the step changes",
     "stop": "runtime-terrain step 7 (revised): crates/tungsten-core/src/physics/step/arrival.rs:275 narrow_phase_at"},
    # R11 kit-census: retired, replaced by P4 and P5 (the census in inspect.rs, describe.rs and coverage.rs).
    # R12 two-genres: retired, replaced by P4 and P6 (the genre fixtures' prefabs and coverage's rule).
    # R13 described-fields: retired, replaced by P5 (describe.rs's numeric-field count).
    # R14 app-scene-names: retired, replaced by P7 (plugins.rs's scene-name census).
    {"id": "R15", "name": "full-literals",
     "when": ({"phrase": GAINS},),
     "files": ({"computed": "full_literals"},),
     "reason": "a struct literal or pattern without a `..` rest names every field, so a new field breaks it",
     "stop": "boot-and-seams steps 12 and 13: crates/tungsten/src/debug_views.rs:280, tests/debug_views.rs:113, app.rs:334, tests/asset_loader.rs:223"},
    # R16 kit-schedule: retired, replaced by P3 (kit.rs's fixed step and edges, the template's schedule).
    # R17 kit-hash: retired, replaced by P4 (determinism.rs's recipes hash).
    # R18 kit-harness: retired, replaced by P3 (kit.rs, listed whole at its snapshot).
    # R19 query-pit: retired, replaced by P1 (01's pit, fed by every engine and kit source).
    {"id": "R20", "name": "cue-events-01",
     "when": ({"touch": r"crates/tungsten-kit/src/react/cues\.rs"}, {"phrase": r"\bcue events?\b"}),
     "files": ({"computed": "quoted_in_01"},),
     "reason": "01 registers its own event and cue names as strings, which a kit name cannot take",
     "stop": "looks-and-cues step 2: examples/01_platformer/src/features/audio/components.rs:379 \"air_jumped\""},
)

STOP = frozenset("""
as async await break const continue crate dyn else enum extern false fn for if impl in let loop match mod move
mut pub ref return self Self static struct super trait true type unsafe use where while
Option Some None Result Ok Err Vec Box Rc Arc RefCell Cell String str HashMap HashSet BTreeMap BTreeSet VecDeque
Default Clone Copy Debug Display PartialEq Eq Hash Ord PartialOrd Send Sync Sized Into From AsRef Iterator
IntoIterator Fn FnMut FnOnce Path PathBuf iter into from new default clone len push get set map unwrap expect
is_empty is_some is_none to_string as_str to_owned insert remove contains u8 u16 u32 u64 u128 usize i8 i16 i32
i64 i128 isize f32 f64 bool char assert assert_eq assert_ne debug_assert println eprintln format vec matches cfg
test tests derive size_of std mem serde json Serialize Deserialize glam Vec2 Vec3 Vec4 Mat4 Quat IVec2 UVec2
Entity World TODO null none the and not all any min max off
""".split())
GENERIC_STEMS = frozenset("mod lib main tests test README index manifest Cargo build common util utils helpers prelude types".split())
COMMANDS = frozenset("""
just cargo rg git python3 python test cmp bash sh cd tungsten wc ls diff patch find fd jq df tokei export rtk
critique codex cat head tail grep sed awk echo mkdir rm cp mv tar env time shellcheck actionlint du sort xargs
""".split())
EXTS = r"rs|md|json|toml|py|sh|wgsl|txt|tmj|yml|yaml|lock|png|wav|ogg|ttf"
ROOT_FILES = frozenset("""
ROADMAP.md DESIGN.md DECISIONS.md CHANGELOG.md README.md AGENTS.md CLAUDE.md justfile Cargo.toml Cargo.lock
tungsten.json input.json deny.toml rustfmt.toml rust-toolchain.toml LICENSE
""".split())
# Root file names a plan also writes for another folder's copy ("the template's `AGENTS.md`").
AMBIGUOUS = frozenset({"AGENTS.md", "CLAUDE.md", "README.md", "Cargo.toml", "tungsten.json", "input.json"})
CRATES = {"core": "crates/tungsten-core/src/", "kit": "crates/tungsten-kit/src/", "umbrella": "crates/tungsten/src/",
          "render": "crates/tungsten-render/src/"}
GAMES = {"01": "examples/01_platformer/", "02": "examples/02_bench/", "03": "examples/03_fixtures/",
         "template": "templates/basic/", "01 shell": "examples/01_platformer/src/shell/",
         "template shell": "templates/basic/src/shell/"}
RECIPES = "crates/tungsten/src/tests/kit_recipes/"
FIXTURES = "crates/tungsten/tests/fixtures/kit/"
QUALIFIER = re.compile(
    r"\b(core tests?|core|kit tests?|kit|umbrella tests?|umbrella|render tests?|render|01's shell|01's|02's|03's"
    r"|template's shell|template's|recipes?'?|fixtures?'?|tests?)(?![\w-])")

# A list entry the plan conditions: "any `x/*.rs` the rule in §6 admits", "only as §5 allows".
CONDITION = re.compile(r"\b(?:only|unless|if|admits?|allows?)\b", re.I)
# A list entry with no path that still names files ("the follow-ups' homes", "the fixture").
FILEISH = re.compile(r"\b(?:files?|scripts?|fixtures?|recipes?|tests?|homes?|plans?|docs?|sources?|snapshots?"
                     r"|manifests?|folders?|lists?|modules?|copies|pins?|guards?|records?|schemas?)\b", re.I)
CODE = re.compile(r"`([^`\n]+)`")
LINK = re.compile(r"\[[^\]]*\]\(([^)\s#]+)(?:#[^)]*)?\)")
TOKEN = re.compile(r"[A-Za-z0-9_]+")
WORDLIKE = re.compile(r"(?<![A-Za-z0-9_])[A-Za-z_][A-Za-z0-9_]*(?:\.[A-Za-z_][A-Za-z0-9_]*)*")
ROW_ID = re.compile(r"(?<![\w-])(?:S-\d+|P[78]-\d+|PA-\d+|K\d+)(?![\w-])")
HEX = re.compile(r"\b0x[0-9a-fA-F_]{8,}\b")
QUOTED = re.compile(r'"([A-Za-z_][\w.\-]*)"')
COMMIT = re.compile(r"^(?:feat|fix|docs|test|refactor|perf|chore|build|ci|style|revert)(?:\([^)]*\))?!?:\s")
SHA = re.compile(r"[0-9a-f]{7,40}(?:[\^~]\d*)?(?:\.\.\.?[0-9a-f]{7,40})?")
HEXISH = re.compile(r"[a-f]*\d[0-9a-f]*")
HEADING = re.compile(r"^(#{1,6})\s+(.*?)\s*#*\s*$")
FENCE = re.compile(r"^\s{0,3}(`{3,}|~{3,})")
FIELD = re.compile(r"^-\s+\*\*([A-Za-z][A-Za-z' -]*):\*\*\s*(.*)$")
SECTION_CITE = re.compile(r"§\s*(\d+(?:\.\d+)*)(?:\s*[–-]\s*§?\s*(\d+(?:\.\d+)*))?")
ROW_CITE = re.compile(r"\b([AQ]\d+)\b")
RS_DEF = re.compile(r"\b(?:struct|enum|union|trait|type|const|static|fn|mod)\s+([A-Za-z_]\w*)")


def glob_re(pattern):
    """A glob as a regex over repository paths: `**/` any folders, `*` within one."""
    out, i = [], 0
    while i < len(pattern):
        if pattern.startswith("**/", i):
            out.append("(?:.*/)?")
            i += 3
        elif pattern.startswith("**", i):
            out.append(".*")
            i += 2
        elif pattern[i] == "*":
            out.append("[^/]*")
            i += 1
        elif pattern[i] == "?":
            out.append("[^/]")
            i += 1
        else:
            out.append(re.escape(pattern[i]))
            i += 1
    return re.compile("".join(out) + r"\Z")


def expand_braces(text):
    match = re.search(r"\{([^{}]*)\}", text)
    if not match:
        return [text]
    out = []
    for alternative in match.group(1).split(","):
        out.extend(expand_braces(text[:match.start()] + alternative.strip() + text[match.end():]))
    return out


def split_top(text, separators):
    """`text` cut at the separator characters outside code spans and brackets (a `.` only
    before a space or the end), empty parts dropped."""
    parts, current, depth, code = [], [], 0, False
    for i, c in enumerate(text):
        if c == "`":
            code = not code
        elif not code:
            if c in "([":
                depth += 1
            elif c in ")]":
                depth = max(0, depth - 1)
            elif depth == 0 and c in separators and (c != "." or i + 1 == len(text) or text[i + 1].isspace()):
                parts.append("".join(current))
                current = []
                continue
        current.append(c)
    parts.append("".join(current))
    return [part for part in parts if part.strip(" .")]


def skipped(rel):
    parts = rel.split("/")
    return (parts[0] in SKIP_TOP or any(part in SKIP_ANY for part in parts) or rel in SELF
            or rel == ARCHIVE or rel.startswith(ARCHIVE + "/"))


class Corpus:
    """The searchable tree: every file's path, and each text file's lines and tokens on demand."""

    def __init__(self, root):
        self.root = root
        self.paths = sorted(rel for rel in self.list_files() if not skipped(rel))
        self.files = set(self.paths)
        self.dirs = {rel[:i + 1] for rel in self.paths for i, c in enumerate(rel) if c == "/"}
        self.text_paths = [rel for rel in self.paths if os.path.splitext(rel)[1].lower() not in BINARY]
        self._lines, self._text, self._tokens, self._compounds, self._starts = {}, {}, {}, {}, {}

    def list_files(self):
        if (self.root / ".git").exists():
            try:
                listed = subprocess.run(["git", "-C", str(self.root), "ls-files", "-z"], capture_output=True,
                                        check=True, env={**os.environ, "GIT_OPTIONAL_LOCKS": "0"}).stdout
                return [rel for rel in os.fsdecode(listed).split("\0") if rel and (self.root / rel).is_file()]
            except (OSError, subprocess.CalledProcessError):
                pass
        files = []
        for base, dirs, names in os.walk(self.root, followlinks=False):
            rel_base = Path(base).relative_to(self.root).as_posix()
            rel_base = "" if rel_base == "." else rel_base + "/"
            dirs[:] = sorted(d for d in dirs if not skipped(rel_base + d) and not (Path(base) / d).is_symlink())
            files.extend(rel_base + name for name in names if not (Path(base) / name).is_symlink())
        return files

    def lines(self, rel):
        """A text file's lines, numbered from 1 at index 0; None for a binary or unreadable file."""
        if rel not in self._lines:
            lines = None
            if rel in self.files and os.path.splitext(rel)[1].lower() not in BINARY:
                try:
                    data = (self.root / rel).read_bytes()
                    if b"\0" not in data[:8192]:
                        lines = [line.rstrip("\r") for line in data.decode("utf-8", "replace").split("\n")]
                except OSError:
                    pass
            self._lines[rel] = lines
        return self._lines[rel]

    def text(self, rel):
        if rel not in self._text:
            lines = self.lines(rel)
            self._text[rel] = "\n".join(lines) if lines is not None else ""
        return self._text[rel]

    def line_starts(self, rel):
        """The offset in text() where each line begins: bisect gives a line number."""
        if rel not in self._starts:
            starts, at = [], 0
            for line in self.lines(rel) or []:
                starts.append(at)
                at += len(line) + 1
            self._starts[rel] = starts
        return self._starts[rel]

    def tokens(self, rel):
        if rel not in self._tokens:
            self._tokens[rel] = set(TOKEN.findall(self.text(rel)))
        return self._tokens[rel]

    def compounds(self, rel):
        """Every whole run of segments in the file's dotted or hyphenated tokens (`S-162`,
        `stimulus.impact`): a name with a `.` or `-` matches the file only as one of them."""
        if rel not in self._compounds:
            self._compounds[rel] = compound_runs(self.text(rel))
        return self._compounds[rel]

    def match(self, pattern):
        """Paths a glob or a plain path names."""
        if not any(c in pattern for c in "*?"):
            return [pattern] if pattern in self.files else []
        regex = glob_re(pattern)
        return [rel for rel in self.paths if regex.match(rel)]

    def grep(self, rel, regex):
        lines = self.lines(rel) or []
        return [n for n, line in enumerate(lines, 1) if regex.search(line)]

    def is_dir(self, rel):
        return rel.rstrip("/") + "/" in self.dirs


class Section:
    def __init__(self, level, title, start, end, lines):
        self.level, self.title, self.start, self.end = level, title, start, end
        number = re.match(r"(\d+(?:\.\d+)*)\.?\s", title + " ")
        self.number = number.group(1) if number else None
        self.text = "\n".join(lines[start:end])


class Step:
    def __init__(self, number, title, text=""):
        self.number, self.title, self.text = number, title, text
        self.header_list = ""   # the header's `Step N:` line of files to touch
        self.files = ""         # the step's own **Files:** field
        self.preamble = ""      # the steps section's text before its first step (common checks)


class Plan:
    def __init__(self, rel, text):
        self.rel, self.text = rel, text
        self.lines = text.split("\n")
        headings, fence = [], None
        for i, line in enumerate(self.lines):
            mark = FENCE.match(line)
            if mark:
                fence = None if fence and mark.group(1)[0] == fence[0] else (fence or mark.group(1))
                continue
            heading = HEADING.match(line) if fence is None else None
            if heading:
                headings.append((i, len(heading.group(1)), heading.group(2)))
        self.sections = []
        for k, (i, level, title) in enumerate(headings):
            end = next((j for j, other, _ in headings[k + 1:] if other <= level), len(self.lines))
            self.sections.append(Section(level, title, i, end, self.lines))
        header_end = next((i for i, level, _ in headings if level == 2), len(self.lines))
        self.fields = self.header_fields(self.lines[:header_end])
        self.numbered = {s.number: s for s in self.sections if s.number}
        self.rows = {}
        for line in self.lines:
            row = re.match(r"^\|\s*([AQ]\d+)\s*\|", line)
            if row:
                self.rows.setdefault(row.group(1), line)
        self.steps = self.find_steps()
        self.prose = self.prose_words()

    @staticmethod
    def header_fields(lines):
        """Each `- **field:** inline` bullet with its indented continuation lines."""
        fields, current = {}, None
        for line in lines:
            field = FIELD.match(line)
            if field:
                current = field.group(1).lower()
                fields[current] = [field.group(2)]
            elif current and line[:1] in (" ", "\t") and line.strip():
                fields[current].append(line.strip())
            else:
                current = None
        return fields

    def find_steps(self):
        steps = {}
        for section in self.sections:
            match = re.match(r"Step\s+(\d+)\b\s*(?:\([^)]*\))?\s*:?\s*(.*)", section.title)
            if match and int(match.group(1)) not in steps:
                step = Step(int(match.group(1)), match.group(2), section.text)
                step.files = " ".join(value for name, value in self.section_fields(section.text) if name == "files")
                parent = next((s for s in self.sections if s.level == section.level - 1
                               and s.start < section.start < s.end), None)
                if parent:
                    first = min(s.start for s in self.sections if s.level == section.level
                                and parent.start < s.start < parent.end)
                    step.preamble = "\n".join(self.lines[parent.start + 1:first])
                steps[step.number] = step
        if not steps:
            for number, text in self.ordered_items():
                steps[number] = Step(number, text[:60], text)
        for entry in self.fields.get("files to touch", [])[1:]:
            line = re.match(r"^-\s+Step\s+(\d+)\s*:\s*(.*)$", entry)
            if line and int(line.group(1)) in steps:
                steps[int(line.group(1))].header_list += " " + line.group(2)
            elif re.match(r"^-\s+Each (?:later )?step\b", entry, re.I):
                for step in steps.values():
                    step.header_list += " " + entry.split(":", 1)[-1]
            elif re.match(r"^-\s+The close\b", entry, re.I):
                close = next((s for s in steps.values() if re.search(r"\bclose\b", s.title, re.I)), None)
                if close:
                    close.header_list += " " + entry.split(":", 1)[-1]
        return dict(sorted(steps.items()))

    def ordered_items(self):
        field = self.fields.get("ordered steps", [])
        listed = [re.match(r"^(\d+)\.\s+(.*)", item) for item in field[1:]]
        if any(listed):
            return [(int(m.group(1)), m.group(2)) for m in listed if m]
        inline = " ".join(field)
        return [(int(m.group(1)), m.group(2).strip()) for m in re.finditer(r"\((\d+)\)\s*([^()]*?)(?=\(\d+\)|$)", inline)]

    @staticmethod
    def section_fields(text):
        return [(m.group(1).lower(), m.group(2)) for m in
                (re.match(r"^\s*-\s+\*\*([A-Za-z][A-Za-z' -]*):\*\*\s*(.*)$", line) for line in text.split("\n")) if m]

    def prose_words(self):
        """Lowercase words the plan writes outside code spans: its prose."""
        text = CODE.sub(" ", self.text)
        return {word.lower() for word in re.findall(r"[A-Za-z][a-z]+", text)}

    def plan_list_text(self):
        """Every part of the header's files to touch (the resolver skips its base definitions)."""
        return " ; ".join(self.fields.get("files to touch", []))

    def cited_sections(self, text, step_number=None):
        numbers = set()
        for match in SECTION_CITE.finditer(text):
            if re.search(r"(?:['’]s|\bits)\s*$", text[max(0, match.start() - 4):match.start()]):
                continue  # "the redo's §5.2", "its §2": another document's section
            start, end = match.group(1), match.group(2)
            if end and start.isdigit() and end.isdigit():
                numbers.update(str(n) for n in range(int(start), int(end) + 1))
            else:
                numbers.add(start)
        for name, value in self.section_fields(text):
            if name in ("change", "design", "sketch"):
                numbers.update(re.findall(r"(?<![\d.])(\d+\.\d+(?:\.\d+)?)(?![\d.])", value))
        found = [self.numbered[n] for n in sorted(numbers) if n in self.numbered]
        if step_number is not None:
            for section in self.sections:
                if section.number and not section.title.lower().startswith("step"):
                    named = re.search(r"\bSteps?\s+([\d][\d ,and–-]*)", section.title)
                    if named and step_number in numbers_in(named.group(1)):
                        found.append(section)
        return found


def numbers_in(text):
    out = set()
    for start, end in re.findall(r"(\d+)(?:\s*[–-]\s*(\d+))?", text):
        out.update(range(int(start), int(end or start) + 1))
    return out


class Touch:
    """A list's resolved entries: files, new files, folders and globs. Beside them: the entries
    the plan conditions ("any `x/*.rs` the rule in §6 admits"), which exclude nothing, the
    entries that resolved to no path, and those resolved through another step's alias."""

    def __init__(self):
        self.files, self.new, self.folders, self.globs, self.unresolved = set(), set(), set(), {}, []
        self.conditional = {}  # (kind, path) -> the entry's text
        self.aliases = []      # {"entry", "name", "paths"}

    def add(self, kind, path, condition=None):
        if kind == "folder":
            path = path.rstrip("/") + "/"
        if condition:
            self.conditional.setdefault((kind, path), condition)
        elif kind == "file":
            self.files.add(path)
        elif kind == "new":
            self.new.add(path)
        elif kind == "folder":
            self.folders.add(path)
        elif kind == "glob":
            self.globs[path] = glob_re(path)

    def update(self, other):
        self.files |= other.files
        self.new |= other.new
        self.folders |= other.folders
        self.globs.update(other.globs)
        self.unresolved += [u for u in other.unresolved if u not in self.unresolved]
        for key, condition in other.conditional.items():
            self.conditional.setdefault(key, condition)
        self.aliases += [a for a in other.aliases if a not in self.aliases]

    def covers(self, path):
        return (path in self.files or any(path.startswith(folder) for folder in self.folders)
                or any(regex.match(path) for regex in self.globs.values()))

    def condition_of(self, path):
        """The text of the conditional entry that admits `path`, if one does."""
        for (kind, entry), condition in self.conditional.items():
            if (path == entry or (kind == "folder" and path.startswith(entry))
                    or (kind == "glob" and glob_re(entry).match(path))):
                return condition
        return None

    def paths(self):
        """Every path the list names, the conditional ones too (what a step may touch)."""
        return sorted(self.files | self.new | self.folders | set(self.globs) | {p for _, p in self.conditional})

    def summary(self):
        conditional = {}
        for (_, path), condition in sorted(self.conditional.items()):
            conditional.setdefault(condition, []).append(path)
        return {"files": sorted(self.files), "new": sorted(self.new), "folders": sorted(self.folders),
                "globs": sorted(self.globs), "unresolved": self.unresolved,
                "conditional": [{"entry": entry, "paths": paths} for entry, paths in conditional.items()],
                "aliases": self.aliases}


class Resolver:
    """Resolves a plan's list paths against the bases its header defines and the tree."""

    def __init__(self, plan, corpus):
        self.plan, self.corpus = plan, corpus
        self.plan_dir = os.path.dirname(plan.rel)
        self.bases = {name: [path] for name, path in CRATES.items()}
        self.bases.update({name + " tests": [path + "tests/"] for name, path in CRATES.items()})
        self.bases.update({name: [path] for name, path in GAMES.items()})
        self.bases["recipes"], self.bases["fixtures"] = [RECIPES], [FIXTURES]
        self.crate_order = []
        self.definitions = set()
        self.defined = set()  # the bases the header defines
        self._default = self._named = None
        self._expanding = set()  # the steps whose lists a "Step N's list" is expanding
        inline = (plan.fields.get("files to touch") or [""])[0]
        self.define(inline)

    def define(self, text):
        """`kit paths under `P``, `its tests under `P``, `the recipes are `P``: bases for the lists."""
        for clause in re.split(r";", text):
            crate = None
            for m in re.finditer(r'(?P<label>"[^"]+"|[A-Za-z][\w \'’]*?)\s+(?:paths?\s+)?(?:are|is|under)\s+`(?P<path>[^`]+)`',
                                 clause):
                label, path = m.group("label").strip('"').lower(), m.group("path")
                named = next((word for word in CRATES if re.search(rf"\b{word}\b", label)), None)
                crate = named or crate
                key = None
                if "test" in label and (crate or self.crate_order):
                    owner = crate or self.crate_order[0]
                    key = owner + " tests"
                    if not path.startswith(("crates/", "examples/", "templates/")):
                        path = self.bases[owner][0].rsplit("src/", 1)[0] + path
                elif named:
                    key = named
                    if named not in self.crate_order:
                        self.crate_order.append(named)
                elif "recipe" in label:
                    key = "recipes"
                elif "fixture" in label:
                    key = "fixtures"
                self.definitions.add(m.group("path"))
                if key:
                    self.bases[key] = expand_braces(path)
                    self.defined.add(key)

    def entry(self, path):
        if any(c in path for c in "*?"):
            return "glob", path
        if path.endswith("/"):
            return ("folder" if self.corpus.is_dir(path) else "new"), path
        if path in self.corpus.files:
            return "file", path
        if self.corpus.is_dir(path):
            return "folder", path + "/"
        return "new", path

    def exists(self, path):
        if any(c in path for c in "*?"):
            return bool(self.corpus.match(path)) or self.corpus.is_dir(path.split("*")[0].rsplit("/", 1)[0] + "/")
        return path.rstrip("/") in self.corpus.files or self.corpus.is_dir(path)

    def rooted(self, item):
        """A path from the root: a root file, or a first folder only the root has (`assets/` is
        also a crate's `src/assets/`, so it resolves against the bases first)."""
        first = item.split("/", 1)[0]
        return item in ROOT_FILES or ("/" in item and (first in ("crates", "examples", "templates", "docs", "scripts",
                                                                "api", "tools", "licenses", "perf-runs")
                                                       or first.startswith(".")
                                                       or (first != "assets" and self.corpus.is_dir(first + "/"))))

    def candidates(self, key, item):
        """Paths for a relative item under a base: a base that names files stands for its folder,
        and the recipes and fixtures also resolve from their usual folders."""
        bases = [base if base.endswith("/") else base.rsplit("/", 1)[0] + "/" for base in self.bases.get(key, [])]
        bases += {"recipes": [RECIPES], "fixtures": [FIXTURES]}.get(key, [])
        out = []
        for base in dict.fromkeys(bases):
            out.append(base + item)
            if base.endswith("tests/") and item.startswith("tests/"):
                out.append(base[:-len("tests/")] + item)
        return out

    def resolve(self, item, qualifier=None, last=None):
        """(entries, base key used) for one path of a list, its braces already expanded."""
        item = re.sub(r"\s*\(new\)", "", item.strip()).rstrip(".,;:")
        item = re.sub(r":\d+(?:-\d+)?$", "", item)
        item = re.sub(r"<[^>]+>", "*", item)
        if not item or not re.search(r"[A-Za-z]", item):
            return [], last
        if self.rooted(item):
            return [self.entry(item)], last
        if qualifier:
            keys = [qualifier]
        else:
            keys = self.crate_order + [k for k in CRATES if k not in self.crate_order]
        found = [(key, path) for key in keys for path in self.candidates(key, item) if self.exists(path)]
        if qualifier:
            if found:
                return [self.entry(path) for _, path in found[:1]], qualifier
            paths = self.candidates(qualifier, item)
            return ([self.entry(paths[0])], qualifier) if paths else ([], last)
        if len({key for key, _ in found}) > 1:
            if last not in {key for key, _ in found}:
                return [("ambiguous", found)], last  # `lib.rs`: settled by its group's crate
            found = [(key, path) for key, path in found if key == last]
        if found:
            return [self.entry(path) for _, path in found], found[0][0]
        if item.split("/", 1)[0] == "assets" and self.exists(item):
            return [self.entry(item)], last
        suffix = [rel for rel in self.corpus.paths if rel.endswith("/" + item)] if not item.endswith("/") else []
        if 0 < len(suffix) <= 3:
            return [("file", rel) for rel in suffix], last
        # A new file: under the base whose folder holds its parent, the last one used first.
        parent = item.rstrip("/").rsplit("/", 1)[0] + "/" if "/" in item.rstrip("/") else ""
        homes = [key for key in ([last] if last in keys else []) + keys
                 if parent and self.corpus.is_dir(self.candidates(key, parent)[0])]
        if homes:
            return [("new", self.candidates(homes[0], item)[0])], homes[0]
        # A new folder too (`chemistry/{mod.rs,rules.rs}` (new)): its group's crate, once known.
        return [("pending", item)], last

    def default_crates(self):
        """The crates the plan's unqualified paths resolve to, most first: where a path no
        qualifier places and no folder holds goes by default."""
        if self._default is None:
            self._default = []
            self._default = self.resolve_list(self.plan.plan_list_text(), votes_only=True)
        return self._default

    def named_groups(self):
        """{name: [paths]}: "the `seams` fixture (§2.18): `a`, `b`" names `a` and `b` (the spans
        after the colon that share the first one's two leading folders), so another step's "the
        `seams` fixture's files" resolves to them."""
        if self._named is None:
            self._named = {}
            text = self.plan.plan_list_text() + " ; " + " ; ".join(step.files for step in self.plan.steps.values())
            for match in re.finditer(r"\bthe `([^`]+)` [a-z]+(?:\s*\([^)]*\))?:\s*((?:`[^`]+`(?:\s*,\s*|\s+and\s+)?)+)", text):
                spans = CODE.findall(match.group(2))
                head = "/".join(spans[0].split("/")[:2]) + "/" if spans and "/" in spans[0] else None
                paths = [s for s in spans if head and s.startswith(head)]
                if paths:
                    self._named.setdefault(match.group(1), paths)
        return self._named

    def resolve_list(self, text, votes_only=False):
        """A list's text: `;` groups of `,` entries, each of prose qualifiers ("kit tests", "the
        recipes'") and code spans. An entry the plan conditions ("only as §5 allows", "the rule in
        §6 admits") excludes nothing; one that resolves to no path is reported, never dropped."""
        touch = Touch()
        last = None
        deferred, crates = [], {}  # (group, kind, item, condition); group -> crates its paths are in
        for group, group_text in enumerate(split_top(text, ";")):
            qualifier, recipe_stems = None, set()
            for entry_text in split_top(group_text, ",."):
                prose = CODE.sub(" ", entry_text)
                condition = " ".join(entry_text.split()) if CONDITION.search(prose) else None
                found, used, names = False, set(), []
                for chunk in re.split(r"(`[^`\n]*`)", entry_text):
                    if not (len(chunk) >= 2 and chunk.startswith("`") and chunk.endswith("`")):
                        for link in LINK.finditer(chunk):
                            target = os.path.normpath(os.path.join(self.plan_dir, link.group(1))).replace(os.sep, "/")
                            if not target.startswith(".."):
                                touch.add(*self.entry(target), condition)
                                found = True
                        qualifiers = QUALIFIER.findall(chunk.lower())
                        if qualifiers:
                            qualifier = qualifiers[-1]
                        continue
                    span = chunk[1:-1].strip()
                    if span in self.definitions:
                        found = True
                        continue
                    if is_command(span):
                        found = True
                        for path, pattern in command_paths(span):
                            if pattern and command_words(span)[:1] == ["rg"] and "-l" in command_words(span):
                                self.add_rg_list(touch, path, pattern)
                        continue
                    if not pathlike(span):
                        names.append(span)
                        continue
                    for item in expand_braces(re.sub(r"\s*\(new\)", "", span)):
                        key = self.qualifier_key(qualifier, last)
                        entries, base = self.resolve(item, key, last)
                        if key:
                            used.add(key.split()[0] if key.endswith("tests") else key)
                            if key == "recipes":
                                recipe_stems.add(Path(item).stem)
                        last = base if base in CRATES or (base or "").endswith("tests") else last
                        for kind, path in entries:
                            if kind == "unresolved":
                                touch.unresolved.append(path)
                                continue
                            found = True
                            if kind in ("pending", "ambiguous"):
                                deferred.append((group, kind, path, condition))
                                continue
                            touch.add(kind, path, condition)
                            crate = next((name for name, base_path in CRATES.items() if path.startswith(base_path)), None)
                            if crate and not key:  # "core `x.rs`" says nothing of the unqualified
                                crates.setdefault(group, []).append(crate)
                found = self.prose_aliases(touch, prose.lower(), used, recipe_stems, condition) or found
                for number in map(int, re.findall(r"\b[Ss]teps? (\d+)['’]s list\b", prose)):  # "Step 3's list in the header"
                    if number in self.plan.steps and number not in self._expanding:
                        self._expanding.add(number)
                        touch.update(self.resolve_list(self.plan.steps[number].header_list))
                        self._expanding.discard(number)
                        found = True
                if not found and (names or FILEISH.search(prose)):
                    named = next((n for n in names if n in self.named_groups()), None)
                    if named:
                        paths = []
                        for span in self.named_groups()[named]:
                            for item in expand_braces(span):
                                for kind, path in self.resolve(item)[0]:
                                    if kind not in ("unresolved", "pending", "ambiguous"):
                                        touch.add(kind, path, condition)
                                        paths.append(path)
                        alias = {"entry": " ".join(entry_text.split()), "name": named, "paths": paths}
                        if alias not in touch.aliases:
                            touch.aliases.append(alias)
                    else:
                        entry = re.sub(r"^(?:and|or)\s+", "", " ".join(entry_text.split()).strip(" ."))
                        if entry not in touch.unresolved:
                            touch.unresolved.append(entry)
        # A new path in no existing folder, or one more than one crate has (`lib.rs`), goes to the
        # crate most of its group's unqualified paths are in, then the list's, then the plan's.
        every = [crate for group in crates.values() for crate in group]
        if votes_only:
            return sorted(dict.fromkeys(every), key=every.count, reverse=True)
        shared = {}  # group -> the crates that hold every ambiguous path of the group
        for group, kind, item, _ in deferred:
            if kind == "ambiguous":
                keys = {key for key, _ in item}
                shared[group] = shared[group] & keys if group in shared else keys
        for group, kind, item, condition in deferred:
            order = [c for pool in (crates.get(group, []), every, self.default_crates(), self.crate_order)
                     for c in sorted(dict.fromkeys(pool), key=pool.count, reverse=True)]
            order = [c for c in order if c in shared.get(group, ())] + order
            if kind == "pending":
                if order:
                    touch.add("new", self.candidates(order[0], item)[0], condition)
                else:
                    touch.unresolved.append(item)
            else:
                choice = next(([path for key, path in item if key == crate] for crate in order
                               if any(key == crate for key, _ in item)), [path for _, path in item])
                for path in choice:
                    touch.add(*self.entry(path), condition)
        return touch

    def prose_aliases(self, touch, prose, used, recipe_stems, condition):
        """"recipes and fixtures", "the platformer recipe and fixture", "their fixtures", "this
        plan": the defined recipes and fixtures, by the genres the entry names. A lone "the
        fixture" is the kit's only where the header defines fixtures or the entry names recipes."""
        genres = self.genres()
        named = {g for g in genres if re.search(rf"\b{re.escape(g)}\b", prose)} | (recipe_stems & genres)
        recipes = bool(re.search(r"\brecipes?\b", prose))
        found = False
        if recipes and "recipes" not in used:
            for path in self.alias("recipes", named):
                touch.add(*self.entry(path), condition)
                found = True
        if (re.search(r"\bfixtures?\b", prose) and "fixtures" not in used
                and ("fixtures" in self.defined or recipes or recipe_stems)):
            for path in self.alias("fixtures", named):
                touch.add(*self.entry(path), condition)
                found = True
        if re.search(r"\bthis (?:plan|file)\b", prose):
            touch.add("file", self.plan.rel)
            found = True
        return found

    def qualifier_key(self, qualifier, last):
        if not qualifier:
            return None
        word = qualifier.rstrip("'")
        if word.startswith("recipe"):
            return "recipes"
        if word.startswith("fixture"):
            return "fixtures"
        if word in ("test", "tests"):
            crate = last.split()[0] if last and last.split()[0] in CRATES else (self.crate_order or ["kit"])[0]
            return crate + " tests"
        if word.endswith(" test"):
            word += "s"
        word = {"01's": "01", "02's": "02", "03's": "03", "template's": "template", "01's shell": "01 shell",
                "template's shell": "template shell"}.get(word, word)
        return word if word in self.bases else None

    def genres(self):
        return {rel[len(FIXTURES):].split("/", 1)[0] for rel in self.corpus.dirs if rel.startswith(FIXTURES)
                and rel.count("/") == FIXTURES.count("/") + 1}

    def alias(self, key, named):
        out = []
        for base in self.bases.get(key, []):
            if base.endswith("/") and self.corpus.is_dir(base) and base in (RECIPES, FIXTURES):
                for genre in sorted(named or self.genres()):
                    out.append(base + genre + (".rs" if key == "recipes" else "/"))
            elif not named or Path(base.rstrip("/")).stem in named:
                out.append(base)
        return [path for path in out if self.exists(path)]

    def add_rg_list(self, touch, path, pattern):
        regex = compile_pattern(pattern)
        prefix = path.rstrip("/") + "/"
        for rel in self.corpus.text_paths:
            if (rel == path or rel.startswith(prefix)) and any(regex.search(line) for line in self.corpus.lines(rel) or []):
                touch.add("file", rel)


def compile_pattern(pattern):
    try:
        return re.compile(pattern)
    except re.error:
        return re.compile(re.escape(pattern))


def command_words(span):
    try:
        lexer = shlex.shlex(span, posix=True, punctuation_chars=True)
        lexer.whitespace_split = True
        words = list(lexer)
    except ValueError:
        words = span.split()
    while words and re.match(r"^[A-Z_][A-Z0-9_]*=", words[0]):
        words = words[1:]
    return words


def is_command(span):
    words = command_words(span)
    return len(words) > 1 and words[0] in COMMANDS


def command_paths(span):
    """(path, rg pattern or None) for each path-like argument of a command or its chained parts."""
    words, out, part = command_words(span), [], []
    for word in words + [";"]:
        if word in (";", "&&", "||", "|", "&"):
            if part and part[0] == "rg":
                out.extend(rg_paths(part[1:]))
            elif part:
                out.extend((w, None) for w in part[1:] if not w.startswith("-") and pathlike(w))
            part = []
        else:
            part.append(word)
    return out


def rg_paths(args):
    takes = {"-g", "--glob", "-t", "--type", "-T", "--type-not", "-e", "--regexp", "-f", "--file", "-r", "--replace",
             "-A", "-B", "-C", "-m", "--max-count", "-M", "--max-columns", "-j", "--threads", "--max-depth", "-d"}
    pattern, positional, skip = None, [], False
    for i, arg in enumerate(args):
        if skip:
            skip = False
            continue
        if arg in ("-e", "--regexp") and i + 1 < len(args):
            pattern = args[i + 1]
            skip = True
        elif arg in takes:
            skip = True
        elif not arg.startswith("-") or arg == "-":
            positional.append(arg)
    if pattern is None and positional:
        pattern, positional = positional[0], positional[1:]
    return [(path, pattern) for path in positional if pathlike(path)]


def pathlike(span):
    span = re.sub(r"\s*\(new\)", "", span.strip())
    if not span or any(c in span for c in ' "=()') or "::" in span or not re.search(r"[A-Za-z]", span):
        return False
    if span in ROOT_FILES or span.startswith(("http:", "https:")):
        return span in ROOT_FILES
    core = re.sub(r":\d+(?:-\d+)?$", "", span)
    return "/" in core or bool(re.search(rf"\.(?:{EXTS})$", core)) or bool(re.search(rf"\}}\.(?:{EXTS})$", core))


class Name:
    def __init__(self, text, kind, origin):
        self.text, self.kind, self.origin = text, kind, origin


def kind_of(name):
    if ROW_ID.fullmatch(name):
        return "id"
    if HEX.fullmatch(name):
        return "literal"
    if "." in name:
        return "tag"
    if re.fullmatch(r"[A-Z][A-Z0-9_]*", name):
        return "const"
    if re.fullmatch(r"[A-Z]\w*", name):
        return "type"
    if "_" in name.strip("_"):
        return "snake"
    return "word"


class Cite:
    """A path a step's text spells, with the lines it points at (a line, a range or an rg pattern)."""

    def __init__(self, token, path, first=None, last=None, pattern=None):
        self.token, self.path, self.first, self.last, self.pattern = token, path, first, last, pattern


def names_in(text, origin):
    """Names and cited paths in a text's code spans, with the register row IDs it writes anywhere."""
    names, cites = [], []
    for span in CODE.findall(text):
        span = span.strip()
        if COMMIT.match(span) or SHA.fullmatch(span):
            continue
        if is_command(span):
            cites.extend((path, pattern) for path, pattern in command_paths(span))
            continue
        if pathlike(span):
            clean = re.sub(r"\s*\(new\)", "", span)
            cites.append((clean, None))
            for item in expand_braces(clean):
                # A source file's stem is a module name; a document's names only documents.
                path = re.sub(r":\d+(?:-\d+)?$", "", item)
                stem = Path(path).stem
                if (path.endswith((".rs", ".py", ".sh", ".wgsl")) and stem not in GENERIC_STEMS
                        and re.fullmatch(r"[A-Za-z_]\w*", stem)):
                    names.append(Name(stem, kind_of(stem), "stem"))
            continue
        words = []
        for word in span.split():
            bare = re.sub(r"['’]s$", "", word.strip("(),;"))
            if pathlike(bare):
                cites.append((bare, None))
            else:
                words.append(word)
        span = re.sub(r"[\w.]*\*[\w.*]*", " ", " ".join(words))  # `ability_*`: a pattern, not a name
        for literal in HEX.findall(span):
            names.append(Name(literal, "literal", origin))
        span = HEX.sub(" ", span)
        for quoted in QUOTED.findall(span):
            names.append(Name(quoted, kind_of(quoted), "string"))
        span = QUOTED.sub(" ", span)
        previous = None
        while previous != span:
            previous, span = span, re.sub(r"(\w)\s*\([^()]*\)", r"\1", span)
        for word in WORDLIKE.findall(span):
            if HEXISH.fullmatch(word) or re.search(rf"\.(?:{EXTS})$", word):
                continue
            parts = word.split(".")
            if len(parts) > 1 and (parts[0][:1].isupper() or parts[0] in STOP or parts[0] == "self"):
                for part in parts:
                    names.append(Name(part, kind_of(part), origin))
            else:
                names.append(Name(word, kind_of(word), origin))
    for row in ROW_ID.findall(text):
        names.append(Name(row, "id", origin))
    return names, cites


COMPOUND = re.compile(r"[A-Za-z0-9_]+(?:[.\-][A-Za-z0-9_]+)+")
WORD_CHAR = re.compile(r"[A-Za-z0-9_]")


def compound_runs(text):
    """The whole runs of segments of each dotted or hyphenated token: `a.b-c` gives `a.b`,
    `b-c` and `a.b-c`. A name is bounded by characters outside [A-Za-z0-9_] exactly when it is
    one of the runs of the token around it, so this is a whole-word match."""
    runs = set()
    for token in set(COMPOUND.findall(text)):
        parts = re.split(r"([.\-])", token)
        segments, joints = parts[0::2], parts[1::2]
        for i in range(len(segments)):
            run = segments[i]
            for j in range(i + 1, len(segments)):
                run += joints[j - 1] + segments[j]
                runs.add(run)
    return runs


def search(corpus, names, skip):
    """{name: [paths]}: the text files but `skip` where a name occurs as a whole word, with no
    [A-Za-z0-9_] on either side (rg -w's rule). A file's token sets decide it exactly."""
    found = {name: [] for name in names}
    simple = {name for name in names if TOKEN.fullmatch(name)}
    compound = {name: set(TOKEN.findall(name)) for name in names if name not in simple}
    for rel in candidate_files(corpus, names):
        if rel == skip:
            continue
        tokens = corpus.tokens(rel)
        wanted = simple & tokens
        maybe = [name for name, parts in compound.items() if parts <= tokens]
        if maybe:
            runs = corpus.compounds(rel)
            wanted.update(name for name in maybe if name in runs)
        for name in wanted:
            found[name].append(rel)
    return found


def name_lines(corpus, name, rel):
    """The lines of `rel` where `name` occurs as a whole word (every occurrence, overlaps too)."""
    text, starts, lines = corpus.text(rel), corpus.line_starts(rel), set()
    at = text.find(name)
    while at >= 0:
        end = at + len(name)
        if not (at and WORD_CHAR.match(text, at - 1)) and not WORD_CHAR.match(text, end):
            lines.add(bisect.bisect_right(starts, at))
        at = text.find(name, at + 1)
    return sorted(lines)


def candidate_files(corpus, names):
    """The text files that can hold a name: rg narrows them when present (fixed strings, so a
    superset of the whole-word matches); without rg, every text file."""
    rg = shutil.which("rg") if os.environ.get("PREFLIGHT_RG", "1") != "0" else None
    paths = corpus.text_paths
    if not rg or not names:
        return paths
    patterns = "\n".join(sorted(names)) + "\n"
    out = set()
    for i in range(0, len(paths), 2000):
        chunk = paths[i:i + 2000]
        try:
            result = subprocess.run([rg, "-l", "-F", "-a", "--no-messages", "--no-config", "-f", "-", "--", *chunk],
                                    input=patterns.encode(), capture_output=True, cwd=corpus.root)
        except OSError:
            return paths
        if result.returncode not in (0, 1):
            return paths
        out.update(os.fsdecode(line) for line in result.stdout.split(b"\n") if line)
    return [rel for rel in paths if rel in out]


class View:
    """One step's (or the whole plan's) text, list, names and cites."""

    def __init__(self, label, number, phrase_text, name_text, touch):
        self.label, self.number = label, number
        self.phrase_text, self.name_text, self.touch = phrase_text, name_text, touch
        self.names, raw_cites = names_in(name_text, "span")
        self.raw_cites = raw_cites
        self.cites, self.derived = [], []


def step_view(plan, resolver, step):
    own = "\n".join(part for part in (step.header_list, step.text) if part)
    # A files line's `§2` is the file's part a step edits, not design it cites.
    sections = plan.cited_sections(step.text, step.number)
    section_text = "\n".join(s.text for s in sections if s.text not in own)
    rows = sorted(set(ROW_CITE.findall(own + "\n" + section_text)))
    row_text = "\n".join(plan.rows[row] for row in rows if row in plan.rows)
    phrase_text = "\n".join(part for part in (own, section_text, row_text) if part)
    name_text = "\n".join(part for part in (phrase_text, step.preamble) if part)
    touch = resolver.resolve_list(step.header_list + " ; " + step.files)
    view = View(f"step {step.number}", step.number, phrase_text, name_text, touch)
    view.sections = [s.number or s.title for s in sections]
    return view


def plan_view(plan, resolver, views):
    """Every step at once: their texts and cited sections, and the header's whole list."""
    phrase_text = "\n".join(view.phrase_text for view in views)
    name_text = "\n".join(view.name_text for view in views)
    touch = resolver.resolve_list(plan.plan_list_text())
    for view in views:
        touch.update(view.touch)
    out = View("plan", None, phrase_text, name_text, touch)
    out.sections = []
    return out


def resolve_cites(view, resolver, corpus, plan):
    """Cited paths to tree files, and the items defined at cited lines in files the step touches."""
    for token, pattern in view.raw_cites:
        for item in expand_braces(token):
            lines = re.search(r":(\d+)(?:-(\d+))?$", item)
            path_item = re.sub(r":\d+(?:-\d+)?$", "", item)
            if path_item.endswith("/") or "*" in path_item or path_item in AMBIGUOUS:
                continue
            entries, _ = resolver.resolve(path_item)
            files = [path for kind, path in entries if kind == "file"]
            files += [path for kind, found in entries if kind == "ambiguous" for _, path in found if path in corpus.files]
            if len(files) > 1:
                listed = [path for path in files if view.touch.covers(path)]
                files = listed[:1] if listed else []
            for path in files:
                first = int(lines.group(1)) if lines else None
                last = int(lines.group(2) or lines.group(1)) if lines else None
                view.cites.append(Cite(item, path, first, last, pattern))
                if first and view.touch.covers(path) and path.endswith(".rs") and last - first <= 40:
                    text = corpus.lines(path) or []
                    for line in text[first - 1:last]:
                        for name in RS_DEF.findall(line):
                            view.derived.append(Name(name, kind_of(name), f"{item}"))


class Context:
    """What the rules read: the corpus, the plan, the name index and the output's list."""

    def __init__(self, corpus, plan, index, exclusion):
        self.corpus, self.plan, self.index, self.exclusion = corpus, plan, index, exclusion
        self._defs = self._sized = None

    def outside(self, rel):
        return rel != self.plan.rel and not self.exclusion.covers(rel)

    def sized(self):
        """The types a `size_of::<T>()` assert pins (in a test, or on an assert's line)."""
        if self._sized is None:
            self._sized = set()
            regex = re.compile(r"size_of::<\s*([A-Z]\w*)\s*>\s*\(\)")
            for rel in self.corpus.text_paths:
                if rel.endswith(".rs") and "size_of" in self.corpus.tokens(rel):
                    for line in self.corpus.lines(rel) or []:
                        if "/tests/" in rel or "assert" in line:
                            self._sized.update(regex.findall(line))
        return self._sized

    def definitions(self):
        """{type: [files]} for every struct and enum the tree defines, and the braced structs."""
        if self._defs is None:
            defs, braced = {}, set()
            pattern = re.compile(r"^\s*(?:pub(?:\([^)]*\))?\s+)?(struct|enum)\s+([A-Z]\w*)(\s*<[^>{]*>)?\s*(\{|\(|;|where|$)")
            for rel in self.corpus.text_paths:
                if not rel.endswith(".rs"):
                    continue
                tokens = self.corpus.tokens(rel)
                if "struct" not in tokens and "enum" not in tokens:
                    continue
                for line in self.corpus.lines(rel) or []:
                    match = pattern.match(line)
                    if match:
                        defs.setdefault(match.group(2), []).append(rel)
                        if match.group(1) == "struct" and match.group(4) in ("{", "where", ""):
                            braced.add(match.group(2))
            self._defs = (defs, braced)
        return self._defs


def touched_existing(view, corpus):
    return sorted(path for path in view.touch.files if path in corpus.files)


def condition_holds(key, value, view, ctx):
    """(holds, evidence) for one condition of a rule's alternative."""
    regex = re.compile(value, re.I) if key == "phrase" else re.compile(value)
    if key == "phrase":
        match = regex.search(view.phrase_text)
        return bool(match), match and f"phrase {short(match.group(0))!r}"
    if key == "touch":
        path = next((p for p in view.touch.paths() if regex.fullmatch(p) or regex.fullmatch(p.rstrip("/"))), None)
        return path is not None, path and f"touches {path}"
    if key == "new_file":
        path = next((p for p in sorted(view.touch.new) if regex.fullmatch(p)), None)
        return path is not None, path and f"new file {path}"
    if key in ("name", "new_name"):
        for name in view.all_names:
            if regex.fullmatch(name) and (key == "name" or not ctx.index.get(name)):
                return True, f"{'new name' if key == 'new_name' else 'names'} `{name}`"
        return False, None
    if key == "sized":
        name = next((n for n in view.all_names if regex.fullmatch(n) and n in ctx.sized()), None)
        return name is not None, name and f"names `{name}`, whose size an assert pins"
    raise ValueError(f"unknown rule condition {key!r}")


def short(text, width=60):
    text = " ".join(text.split())
    return text if len(text) <= width else text[:width - 1] + "…"


def fired_rules(view, ctx):
    """[(rule, evidence)] for the rules the view's text and list fire."""
    out = []
    for rule in RULES:
        for alternative in rule["when"]:
            results = [condition_holds(key, value, view, ctx) for key, value in alternative.items()]
            if all(holds for holds, _ in results):
                out.append((rule, "; ".join(evidence for _, evidence in results)))
                break
    return out


def fired_pins(view):
    """[(pin, evidence)] for the derived-value pins a source the view's list touches feeds.

    A path the text only cites keys no pin: keyed by cites too, the table listed 107 more files
    over the 34 stops and caught none of them that the lists missed.
    """
    touched = sorted(view.touch.paths())
    out = []
    for pin in PINS:
        regexes = [(feed, re.compile(FEEDS[feed])) for feed in pin["feeds"]]
        found = next(((p, feed) for p in touched for feed, regex in regexes
                      if regex.fullmatch(p) or regex.fullmatch(p.rstrip("/"))), None)
        if found:
            out.append((pin, f"touches {found[0]} ({found[1]})"))
    return out


def pin_hits(pin, ctx):
    for spec in pin["files"]:
        if "folder" in spec:
            regex = glob_re(spec["folder"].rstrip("/"))
            for rel in sorted(ctx.corpus.dirs):
                if regex.match(rel.rstrip("/")):
                    yield rel, None, pin["name"]
        else:
            for rel in ctx.corpus.match(spec["path"]):
                yield from lines_or_file(ctx, rel, spec.get("line"), pin["name"])


# --- the rules' computations: each yields (path, line or None, note) -------------------------------

def c_size_pins(view, ctx):
    types = sorted({n for n in view.all_names if kind_of(n) == "type"})
    if not types:
        return
    regex = re.compile(r"size_of::<\s*(" + "|".join(map(re.escape, types)) + r")\s*>\s*\(\)")
    for rel in ctx.corpus.text_paths:
        if rel.endswith(".rs") and "size_of" in ctx.corpus.tokens(rel):
            for n, line in enumerate(ctx.corpus.lines(rel) or [], 1):
                match = regex.search(line)
                if match and ("/tests/" in rel or "assert" in line):
                    yield rel, n, match.group(1)


def c_row_lines(view, ctx):
    ids = sorted({n for n in view.all_names if kind_of(n) == "id"})
    if not ids:
        return
    regex = re.compile(r"^\|\s*(" + "|".join(map(re.escape, ids)) + r")\s*\|")
    for rel in ctx.corpus.match("docs/plans/phase-7.md"):
        for n, line in enumerate(ctx.corpus.lines(rel) or [], 1):
            match = regex.search(line)
            if match:
                yield rel, n, match.group(1)


def c_domain_siblings(view, ctx):
    for new in sorted(view.touch.new):
        match = re.fullmatch(r"(crates/tungsten-kit/src/(?!tests/)[^/]+/)[^/]+\.rs", new)
        if match:
            for rel in ctx.corpus.match(match.group(1) + "*.rs"):
                yield from lines_or_file(ctx, rel, r"^\s*pub(?:\([^)]*\))?\s+struct\s", f"{match.group(1)} beside {Path(new).name}")


def c_sibling_importers(view, ctx):
    for path in touched_existing(view, ctx.corpus):
        if not path.endswith(".rs") or Path(path).name in ("mod.rs", "lib.rs", "main.rs"):
            continue
        folder, stem = path.rsplit("/", 1)[0] + "/", Path(path).stem
        regex = re.compile(r"\bsuper::(?:\{[^}\n]*)?\b" + re.escape(stem) + r"\b")
        for rel in ctx.corpus.match(folder + "*.rs"):
            if rel != path:
                for n in ctx.corpus.grep(rel, regex):
                    yield rel, n, f"imports {stem}"


def gaining(view):
    """The subjects the step gives fields: types, and lowercase fields whose type gains one."""
    subjects = set()
    for match in re.finditer(GAINS, view.phrase_text):
        subjects.add(match.group(1))
        subjects.update(re.findall(r"`([A-Za-z_][\w:]*)`", match.group(2)))
    return {s.split("::")[0] for s in subjects}


def c_full_literals(view, ctx):
    defs, braced = ctx.definitions()
    subjects = gaining(view)
    types = {s for s in subjects if kind_of(s) == "type"}
    lowercase = {s for s in subjects if kind_of(s) in ("word", "snake")}
    for path in touched_existing(view, ctx.corpus):
        if path.endswith(".rs") and lowercase & ctx.corpus.tokens(path):
            for line in ctx.corpus.lines(path) or []:
                field = re.match(r"^\s+(?:pub(?:\([^)]*\))?\s+)?([a-z_][a-z0-9_]*)\s*:\s*(.+?),?\s*$", line)
                if field and field.group(1) in lowercase and not re.search(r"\b(?:fn|let)\b", line):
                    types.update(t for t in re.findall(r"\b[A-Z]\w*", field.group(2)) if t not in STOP)
    types = sorted(t for t in types if t in braced and t not in STOP)
    if not types:
        return
    start = re.compile(r"\b(" + "|".join(map(re.escape, types)) + r")\s*\{")
    for rel in ctx.corpus.text_paths:
        if not rel.endswith(".rs") or not (set(types) & ctx.corpus.tokens(rel)) or not ctx.outside(rel):
            continue
        lines, text, starts = ctx.corpus.lines(rel) or [], ctx.corpus.text(rel), ctx.corpus.line_starts(rel)
        for match in start.finditer(text):
            line_no = bisect.bisect_right(starts, match.start())
            line = lines[line_no - 1]
            before = text[starts[line_no - 1]:match.start()]
            if line.lstrip().startswith("//") or re.search(
                    r"(?:\b(?:struct|enum|union|trait|impl|for|type|mod|dyn)\b[^;{}]*|->\s*(?:[\w:]+::)?)$", before):
                continue
            body = braced_body(text, match.end() - 1)
            if body is not None and body.strip() and not re.search(r"(?:^|[{,])\s*\.\.", body):
                yield rel, line_no, match.group(1)


def braced_body(text, at):
    """The text directly inside the brace at `at` (nested brackets cut out), or None if unclosed."""
    depth, out, i = 0, [], at
    while i < len(text):
        c = text[i]
        if c == '"':
            end = i + 1
            while end < len(text) and text[end] != '"':
                end += 2 if text[end] == "\\" else 1
            i = end + 1
            continue
        if c == "/" and text.startswith("//", i):
            i = text.find("\n", i)
            i = len(text) if i < 0 else i
            continue
        if c == "'" and re.match(r"'(?:\\.|[^\\'])'", text[i:i + 4]):
            i += len(re.match(r"'(?:\\.|[^\\'])'", text[i:i + 4]).group(0))
            continue
        if c in "([{":
            depth += 1
            if depth == 1:
                i += 1
                continue
        elif c in ")]}":
            depth -= 1
            if depth == 0:
                return "".join(out)
        if depth == 1:
            out.append(c)
        i += 1
    return None


def c_quoted_in_01(view, ctx):
    # Event and cue names: snake_case or dotted, or a word the plan never writes as prose.
    names = sorted({n for n in view.all_names if n not in STOP and len(n) >= 3 and (
        kind_of(n) in ("snake", "tag") or (kind_of(n) == "word" and n not in ctx.plan.prose))})
    if not names:
        return
    regex = re.compile(r'"(' + "|".join(map(re.escape, names)) + r')"')
    for pattern in ("examples/01_platformer/src/**/*.rs", "examples/01_platformer/assets/data/**/*.json"):
        for rel in ctx.corpus.match(pattern):
            for n, line in enumerate(ctx.corpus.lines(rel) or [], 1):
                match = regex.search(line)
                if match:
                    yield rel, n, f'"{match.group(1)}"'


def lines_or_file(ctx, rel, line_pattern, note):
    lines = ctx.corpus.grep(rel, re.compile(line_pattern)) if line_pattern else []
    if lines:
        for n in lines:
            yield rel, n, note
    else:
        yield rel, None, note


COMPUTED = {name[2:]: function for name, function in globals().items() if name.startswith("c_")}


def rule_hits(rule, view, ctx):
    for spec in rule["files"]:
        if "computed" in spec:
            yield from COMPUTED[spec["computed"]](view, ctx)
        else:
            for rel in ctx.corpus.match(spec["path"]):
                yield from lines_or_file(ctx, rel, spec.get("line"), rule["name"])


# --- the report ---------------------------------------------------------------------------------

GROUP_PATH_RES = {group: [glob_re(pattern) for pattern in patterns] for group, patterns in GROUP_PATHS.items()}
GROUP_LINE_RES = {group: re.compile(pattern) for group, pattern in GROUP_LINES.items()}


class Report:
    def __init__(self, ctx):
        self.ctx = ctx
        self.files = {}       # path (a folder ends in /) -> {line or None: {"text": str, "sources": [...]}}
        self.conditions = {}  # path -> the conditional list entry that admits it

    def add(self, path, line, source):
        """Records a hit outside the list (a conditional entry's files stay in, marked)."""
        if not self.ctx.outside(path):
            return
        lines = self.ctx.corpus.lines(path) if line else None
        if line and not (lines and 0 < line <= len(lines)):
            line = None
        text = lines[line - 1].strip() if line else ""
        if path not in self.files:
            condition = self.ctx.exclusion.condition_of(path)
            if condition:
                self.conditions[path] = condition
        entry = self.files.setdefault(path, {}).setdefault(line, {"text": text, "sources": []})
        if source not in entry["sources"]:
            entry["sources"].append(source)

    def group(self, path):
        """A pin by its path or a listed line; then the first group its path names; then the
        first whose pattern a listed line matches; else the rest."""
        texts = [entry["text"] for entry in self.files[path].values()]

        def by_path(group):
            return any(regex.match(path) or regex.match(path.rstrip("/")) for regex in GROUP_PATH_RES.get(group, ()))

        def by_line(group):
            regex = GROUP_LINE_RES.get(group)
            return bool(regex) and any(regex.search(text) for text in texts)

        if by_path(GROUPS[0]) or by_line(GROUPS[0]):
            return GROUPS[0]
        return next((g for g in GROUPS[1:-1] if by_path(g)), None) or next(
            (g for g in GROUPS[1:-1] if by_line(g)), GROUPS[-1])


def build(plan, corpus, resolver, step_number):
    steps = plan.steps
    if step_number is not None:
        views = [step_view(plan, resolver, steps[step_number])]
        output = views[0]
    else:
        views = [step_view(plan, resolver, step) for step in steps.values()]
        output = plan_view(plan, resolver, views)
    for view in views + ([output] if output not in views else []):
        resolve_cites(view, resolver, corpus, plan)
        view.all_names = sorted({n.text for n in view.names + view.derived})
    every = sorted({n for view in views + [output] for n in view.all_names})
    index = search(corpus, every, plan.rel)
    ctx = Context(corpus, plan, index, output.touch)
    for view in views + ([output] if output not in views else []):
        view.outside = {name: [p for p in index.get(name, []) if ctx.outside(p)] for name in view.all_names}
    report = Report(ctx)
    names = classify_names(output, plan, index, ctx)
    for name in names["searched"]:
        for path in output.outside[name["name"]]:
            for line in name_lines(corpus, name["name"], path):
                report.add(path, line, {"source": "name", "name": name["name"], "kind": name["kind"]})
    for cite in output.cites:
        if ctx.outside(cite.path):
            lines = []
            if cite.first:
                lines = list(range(cite.first, min(cite.last, cite.first + 2) + 1))
            elif cite.pattern:
                lines = corpus.grep(cite.path, compile_pattern(cite.pattern))
            source = {"source": "name", "name": cite.token, "kind": "cited path"}
            for line in lines or [None]:
                report.add(cite.path, line if line and line <= len(corpus.lines(cite.path) or []) else None, source)
    fired, pins = {}, {}

    def note(store, item, view, evidence, extra):
        record = store.setdefault(item["id"], {"id": item["id"], "name": item["name"], "why": [], "steps": [],
                                               "files": set(), **extra})
        # The whole plan's pass only says why when no single step fired it.
        if view.label not in record["steps"] and not (view is output and view not in views and record["steps"]):
            record["steps"].append(view.label)
            record["why"].append(f"{view.label}: {evidence}")
        return record

    for view in views + ([output] if output not in views else []):
        for rule, evidence in fired_rules(view, ctx):
            record = note(fired, rule, view, evidence, {"reason": rule["reason"], "stop": rule["stop"]})
            for path, line, what in rule_hits(rule, view, ctx):
                if ctx.outside(path):
                    report.add(path, line, {"source": "rule", "rule": rule["id"], "note": what})
                    record["files"].add(path)
        for pin, evidence in fired_pins(view):
            record = note(pins, pin, view, evidence, {"holds": pin["holds"], "stops": pin["stops"]})
            for path, line, what in pin_hits(pin, ctx):
                if ctx.outside(path):
                    report.add(path, line, {"source": "pin", "pin": pin["id"], "note": what})
                    record["files"].add(path)
    return output, names, fired, pins, report


def limit(name):
    return MAX_WORD_FILES if kind_of(name) == "word" else MAX_NAME_FILES


def keep_name(name, plan, view):
    kind = kind_of(name)
    if name in STOP or (len(name) < 3 and kind != "id"):
        return False
    return not (kind == "word" and name.lower() in plan.prose)


def classify_names(view, plan, index, ctx):
    origin = {n.text: n for n in view.derived + view.names}
    searched, common, prose, absent = [], [], [], []
    for name in view.all_names:
        kind = kind_of(name)
        if name in STOP or (len(name) < 3 and kind != "id"):
            continue
        files = len(view.outside[name])
        record = {"name": name, "kind": kind if origin[name].origin not in ("stem",) else "stem", "files": files}
        if origin[name].origin not in ("span", "stem", "string"):
            record["kind"], record["from"] = "derived", origin[name].origin
        if kind == "word" and name.lower() in plan.prose:
            prose.append(record)
        elif files == 0:
            record["anywhere"] = len([p for p in index.get(name, []) if p != plan.rel])
            absent.append(record)
        elif files > limit(name):
            common.append(record)
        else:
            searched.append(record)
    return {"searched": searched, "common": common, "prose": prose, "absent": absent}


def sources_of(entries):
    return {s["source"] for e in entries.values() for s in e["sources"]}


def tag(source):
    return source.get("rule") or source.get("pin") or f"`{source['name']}`"


def by_count(id_):
    return (id_[0], int(id_[1:]))


def text_report(plan_rel, root, step, output, names, fired, pins, report):
    out = [f"preflight {plan_rel}{f', step {step}' if step is not None else ', every step'} (root {root})"]
    summary = output.touch.summary()
    out.append(f"files to touch: {len(summary['files'])} files, {len(summary['folders'])} folders, "
               f"{len(summary['globs'])} globs, {len(summary['new'])} new")
    if summary["unresolved"]:
        out.append(f"list entries that resolve to no path ({len(summary['unresolved'])}): "
                   + "; ".join(summary["unresolved"]))
    if summary["conditional"]:
        out.append(f"conditional list entries, which exclude nothing ({len(summary['conditional'])}): "
                   + "; ".join(c["entry"] for c in summary["conditional"]))
    for alias in summary["aliases"]:
        out.append(f"list entry from another step's list: {alias['entry']} -> {', '.join(alias['paths'])}")
    out.append(f"names searched ({len(names['searched'])}): "
               + (", ".join(f"`{n['name']}` {n['files']}" for n in names["searched"]) or "none"))
    if names["absent"]:
        out.append(f"names in no file outside the list ({len(names['absent'])}): "
                   + ", ".join(f"`{n['name']}`" for n in names["absent"]))
    if names["common"]:
        out.append(f"names dropped, in over {MAX_NAME_FILES} files ({MAX_WORD_FILES} for a plain word) "
                   f"({len(names['common'])}): "
                   + ", ".join(f"`{n['name']}` {n['files']}" for n in names["common"]))
    if names["prose"]:
        out.append(f"names dropped as the plan's prose ({len(names['prose'])}): "
                   + ", ".join(f"`{n['name']}`" for n in names["prose"]))
    for title, store in (("rules", fired), ("pins", pins)):
        out.append(f"{title} fired ({len(store)}):" if store else f"{title} fired: none")
        for record in sorted(store.values(), key=lambda r: by_count(r["id"])):
            out.append(f"  {record['id']} {record['name']}: {'; '.join(record['why'][:3])}"
                       + (f" (+{len(record['why']) - 3} more)" if len(record["why"]) > 3 else "")
                       + f" -> {len(record['files'])} files")
    grouped = {group: [] for group in GROUPS}
    for path in sorted(report.files):
        grouped[report.group(path)].append(path)
    for group in GROUPS:
        out.append("")
        out.append(f"{group} ({len(grouped[group])})")
        for path in grouped[group]:
            entries = report.files[path]
            ordered = sorted(entries.items(), key=lambda kv: (all(s["source"] == "name" for s in kv[1]["sources"]),
                                                              kv[0] or 0))
            for i, (line, entry) in enumerate(ordered[:SHOWN]):
                tags = ", ".join(sorted({tag(s) for s in entry["sources"]}))
                where = f"{path}:{line}" if line else path
                row = f"  {where}  {short(entry['text'], 110)}  [{tags}]" if entry["text"] else f"  {where}  [{tags}]"
                if i == 0 and path in report.conditions:
                    row += f"  (conditional entry: {report.conditions[path]})"
                out.append(row)
            if len(ordered) > SHOWN:
                out.append(f"  {path}: {len(ordered) - SHOWN} more lines")
    counts = report_counts(report)
    out.append("")
    out.append(f"{counts['files']} files outside the list: {counts['name']} by name, {counts['rule']} by rule, "
               f"{counts['pin']} by the pin table, {counts['several']} of them by more than one."
               if counts["files"] else "no files outside the list name what the plan changes.")
    return "\n".join(out)


def report_counts(report):
    """Files listed, and by each source (a file a name and a pin list counts under both)."""
    kinds = [sources_of(entries) for entries in report.files.values()]
    return {"files": len(kinds), "name": sum("name" in k for k in kinds), "rule": sum("rule" in k for k in kinds),
            "pin": sum("pin" in k for k in kinds), "several": sum(len(k) > 1 for k in kinds)}


def json_report(plan_rel, root, step, output, names, fired, pins, report):
    files = []
    for path in sorted(report.files):
        entries = report.files[path]
        sources = [s for e in entries.values() for s in e["sources"]]
        files.append({
            "path": path, "group": report.group(path), "by": sorted(sources_of(entries)),
            "names": sorted({s["name"] for s in sources if s["source"] == "name"}),
            "rules": sorted({s["rule"] for s in sources if s["source"] == "rule"}, key=by_count),
            "pins": sorted({s["pin"] for s in sources if s["source"] == "pin"}, key=by_count),
            "conditional": report.conditions.get(path),
            "hits": [{"line": line, "text": entry["text"], "sources": entry["sources"]}
                     for line, entry in sorted(entries.items(), key=lambda kv: kv[0] or 0)],
        })
    return {
        "plan": plan_rel, "root": str(root), "step": step,
        "files_to_touch": output.touch.summary(),
        "names": names,
        "rules": {"fired": [{**record, "files": sorted(record["files"])}
                            for record in sorted(fired.values(), key=lambda r: by_count(r["id"]))],
                  "table": [{key: rule[key] for key in ("id", "name", "reason", "stop")} for rule in RULES]},
        "pins": {"fired": [{**record, "files": sorted(record["files"])}
                           for record in sorted(pins.values(), key=lambda r: by_count(r["id"]))],
                 "table": [{key: pin[key] for key in ("id", "name", "feeds", "holds", "stops")} for pin in PINS]},
        "files": files,
        "summary": report_counts(report),
    }


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("plan", type=Path, help="the plan file, e.g. docs/plans/phase-7.md")
    parser.add_argument("--step", type=int, help="that step's names, rules and its own list only")
    parser.add_argument("--root", type=Path, default=ROOT, help="the tree to search (default: this repository)")
    parser.add_argument("--json", action="store_true", help="the hits with each one's source, for the back-tester")
    args = parser.parse_args(argv)
    root = args.root.resolve()
    plan_path = args.plan if args.plan.is_absolute() else Path.cwd() / args.plan
    if not plan_path.is_file() and (root / args.plan).is_file():
        plan_path = root / args.plan
    try:
        text = plan_path.read_text(encoding="utf-8")
    except OSError as exc:
        print(f"preflight: cannot read {args.plan}: {exc}", file=sys.stderr)
        return 2
    try:
        rel = plan_path.resolve().relative_to(root).as_posix()
    except ValueError:
        rel = f"docs/plans/{plan_path.name}"
    corpus = Corpus(root)
    plan = Plan(rel, text)
    if args.step is not None and args.step not in plan.steps:
        print(f"preflight: {rel} has no step {args.step} (steps: {', '.join(map(str, plan.steps)) or 'none'})",
              file=sys.stderr)
        return 2
    resolver = Resolver(plan, corpus)
    output, names, fired, pins, report = build(plan, corpus, resolver, args.step)
    if args.json:
        print(json.dumps(json_report(rel, root, args.step, output, names, fired, pins, report), indent=1))
    else:
        print(text_report(rel, root, args.step, output, names, fired, pins, report))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
