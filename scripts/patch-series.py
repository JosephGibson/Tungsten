#!/usr/bin/env python3
"""Cut, verify and hand over an ordered per-step patch series for a human to commit.

For sessions where Git mutations are human-only. Read-only toward the
repository: it runs only git archive, ls-files, rev-parse, status --porcelain
and diff --no-index (GIT_OPTIONAL_LOCKS=0), plus GNU patch inside DIR, and it
never runs commit.sh. DIR must be outside the work tree.

Order: init before the first edit; cut at each step boundary once that step's
checks pass; verify; script. Each commit should pass the repository checks on
its own; the tool does not run them.

  init   --out DIR [--root REPO]
      Export HEAD to DIR/base and record its sha in DIR/series.json.
  cut    --out DIR --name NN-slug --msg-file FILE [PATH ...]
      Copy the PATHs (relative to the work tree root; files, symlinks or
      directories; a path missing from the work tree is a deletion) onto the
      previous state as DIR/NN-slug/, write DIR/NN-slug.patch (git diff
      --no-index -M --full-index --binary between the states, a/ and b/
      repository paths) and copy FILE verbatim to DIR/NN-slug.msg. Without
      PATHs it takes every path `git ls-files -m -d -o --exclude-standard`
      lists, plus earlier steps' paths, that differs from the previous state;
      that includes work left uncommitted before init. NN must ascend.
  drop   --out DIR
      Remove the last step so it can be cut again.
  verify --out DIR
      Replay the patches in order on a fresh export of HEAD (GNU patch for
      text; git binary hunks are decoded here and checked against their blob
      ids), compare each step's tree with its state byte for byte and every
      touched path with the work tree, record the verified patch hashes and
      list the work tree changes left out of the series.
  script --out DIR
      Write DIR/commit.sh for the human. It checks HEAD, a clean index and
      the verified patch hashes, then runs git apply --cached and
      git commit -F per step and ends with git status --short. The working
      tree is left as it is.

Messages are copied verbatim: no trailers or attribution are added.
Unsupported: non-sha1 repositories, submodules, attribute filters or
export-ignore, and paths Git quotes (control characters, '"' or '\\').
"""

import argparse
import base64
import hashlib
import io
import json
import os
import re
import shlex
import shutil
import stat
import subprocess
import sys
import tarfile
import zlib
from pathlib import Path

NAME_RE = re.compile(r"(\d\d)-[a-z0-9]+(?:-[a-z0-9]+)*")
QUOTED_RE = re.compile(r'[\x00-\x1f\x7f"\\]')  # Git C-quotes these even with core.quotePath=false
READ_ONLY = ("archive", "ls-files", "rev-parse", "status", "diff")
# Pin every setting that changes the diff text; --binary carries the blobs git apply needs.
DIFF = ("-c", "core.quotePath=false", "-c", "diff.suppressBlankEmpty=false", "diff", "--no-index",
        "--no-color", "--no-ext-diff", "--no-textconv", "--src-prefix=a/", "--dst-prefix=b/",
        "--unified=3", "-M", "--full-index", "--binary")
HEADER = ("index ", "old mode ", "new mode ", "new file mode ", "deleted file mode ", "similarity index ",
          "--- /dev/null", "+++ /dev/null")
SCRUB = ("GIT_DIR", "GIT_WORK_TREE", "GIT_INDEX_FILE", "GIT_EXTERNAL_DIFF", "GIT_DIFF_OPTS",
         "POSIXLY_CORRECT", "PATCH_GET", "VERSION_CONTROL", "PATCH_VERSION_CONTROL", "SIMPLE_BACKUP_SUFFIX")


def run(args, cwd, stdin=None, ok=(0,), env=None, what=None):
    full = {k: v for k, v in os.environ.items() if k not in SCRUB}
    full.update(GIT_OPTIONAL_LOCKS="0", GIT_TERMINAL_PROMPT="0", GIT_LITERAL_PATHSPECS="1", LC_ALL="C",
                **(env or {}))
    result = subprocess.run(args, cwd=cwd, env=full, input=stdin, capture_output=True)
    if result.returncode not in ok:
        detail = (result.stderr + result.stdout).decode(errors="replace").strip()
        raise ValueError(f"{what or args[0]} failed ({result.returncode}): {detail}")
    return result


def git(cwd, *args, ok=(0,), env=None):
    words = list(args)
    while words[0] == "-c":
        words = words[2:]
    command = words[0]
    if (command not in READ_ONLY or command == "diff" and "--no-index" not in words
            or command == "status" and "--porcelain" not in words):
        raise AssertionError(f"git {command} is not on the read-only list")
    return run(["git", *args], cwd, ok=ok, env=env, what=f"git {command}")


def text(result):
    return result.stdout.decode("utf-8", "surrogateescape")


def head(root):
    return text(git(root, "rev-parse", "--verify", "HEAD^{commit}")).strip()


def status(root, untracked):
    return [line for line in text(git(root, "status", "--porcelain", f"--untracked-files={untracked}")).split("\n")
            if line]


def ls_files(root, *args):
    paths = {p for p in text(git(root, "ls-files", "-z", *args)).split("\0") if p}
    nested = sorted(p for p in paths if p.endswith("/"))
    if nested:
        raise ValueError(f"nested repositories are not supported: {', '.join(nested)}")
    return paths


def blob_id(data):
    return hashlib.sha1(b"blob %d\0" % len(data) + data).hexdigest()


def outside(root, out):
    out, root = Path(os.path.realpath(out)), Path(os.path.realpath(root))
    if out == root or root in out.parents:
        raise ValueError(f"--out {out} is inside the work tree {root}; use a scratch directory")
    return out


def export(root, sha, dest):
    data = git(root, "archive", "--format=tar", sha).stdout
    with tarfile.open(fileobj=io.BytesIO(data)) as archive:
        archive.extractall(dest, filter="tar")


def tree(top):
    """Relative path -> absolute path of every file and symlink below `top`."""
    found = {}
    for base, dirs, files in os.walk(top):
        links = [d for d in dirs if os.path.islink(os.path.join(base, d))]
        dirs[:] = [d for d in dirs if d not in links]
        for name in files + links:
            path = os.path.join(base, name)
            found[os.path.relpath(path, top)] = path
    return found


def entry(path):
    """What Git records for `path`: None, ("l", target) or ("f"/"x", size)."""
    try:
        st = os.lstat(path)
    except (FileNotFoundError, NotADirectoryError):
        return None
    if stat.S_ISLNK(st.st_mode):
        return "l", os.readlink(path)
    if stat.S_ISREG(st.st_mode):
        return "x" if st.st_mode & stat.S_IXUSR else "f", st.st_size
    if stat.S_ISDIR(st.st_mode):
        return None
    raise ValueError(f"{path}: not a file, symlink or directory")


def same(a, b):
    kind = entry(a)
    if kind != entry(b):
        return False
    if kind is None or kind[0] == "l":
        return True
    with open(a, "rb") as fa, open(b, "rb") as fb:
        while True:
            block = fa.read(1 << 20)
            if block != fb.read(1 << 20):
                return False
            if not block:
                return True


def mismatches(a, b):
    return sorted(p for p in tree(a).keys() | tree(b).keys() if not same(Path(a) / p, Path(b) / p))


def load(out):
    path = out / "series.json"
    if not path.is_file():
        raise ValueError(f"{path} not found; run init first")
    series = json.loads(path.read_text())
    root = Path(series["root"])
    outside(root, out)
    return series, root


def save(out, series):
    (out / "series.json.tmp").write_text(json.dumps(series, indent=1) + "\n")
    (out / "series.json.tmp").replace(out / "series.json")


def check_repo(root, series):
    sha = head(root)
    if sha != series["head"]:
        raise ValueError(f"HEAD moved to {sha[:12]} since init at {series['head'][:12]}; start a new series")
    staged = [line[3:] for line in status(root, "no") if line[0] != " "]
    if staged:
        raise ValueError(f"the index has staged changes ({', '.join(staged[:5])}); commit.sh needs a clean index")


def check_export(root, base):
    """The export must hold exactly the index's blobs: no attribute filters, no export-ignore."""
    index = {}
    for record in text(git(root, "ls-files", "-s", "-z")).split("\0"):
        if record:
            meta, path = record.split("\t", 1)
            mode, oid, _ = meta.split(" ")
            index[path] = (mode, oid)
    gitlinks = [p for p, (mode, _) in index.items() if mode == "160000"]
    if gitlinks:
        raise ValueError(f"submodules are not supported: {', '.join(gitlinks)}")
    if tree(base).keys() != index.keys():
        raise ValueError("the HEAD export and the index list different files (export-ignore attributes?)")
    for path, (mode, oid) in index.items():
        kind = entry(base / path)
        data = os.fsencode(kind[1]) if kind and kind[0] == "l" else (base / path).read_bytes()
        expected = "l" if mode == "120000" else "x" if mode == "100755" else "f"
        if kind is None or kind[0] != expected or blob_id(data) != oid:
            raise ValueError(f"{path}: the HEAD export differs from its blob (gitattributes filter or eol?)")


def init(out, root):
    root = Path(text(git(root, "rev-parse", "--show-toplevel")).strip())
    out = outside(root, out)
    if out.exists() and any(out.iterdir()):
        raise ValueError(f"{out} is not empty; pick a new directory")
    if text(git(root, "rev-parse", "--show-object-format")).strip() != "sha1":
        raise ValueError("only sha1 repositories are supported")
    sha = head(root)
    check_repo(root, {"head": sha})
    out.mkdir(parents=True, exist_ok=True)
    try:
        export(root, sha, out / "base")
        check_export(root, out / "base")
    except BaseException:
        shutil.rmtree(out / "base", ignore_errors=True)
        raise
    save(out, {"root": str(root), "head": sha, "steps": [], "verified": {}})
    print(f"Series {out} starts at {sha[:12]} ({len(tree(out / 'base'))} files exported).")
    dirty = status(root, "all")
    if dirty:
        print(f"NOTE: {len(dirty)} path(s) were already uncommitted; name PATHs in cut to keep them out:")
        print("\n".join(f"  {line}" for line in dirty))


def relative(root, path):
    """`path` (relative to the work tree root, or absolute) as a work tree path."""
    full = os.path.normpath(os.path.join(root, path))
    full = os.path.join(os.path.realpath(os.path.dirname(full)), os.path.basename(full))
    rel = os.path.relpath(full, root)
    if rel in (".", "..") or rel.startswith("../") or rel.split("/")[0] == ".git":
        raise ValueError(f"{path}: not a path inside the work tree")
    return rel


def select(root, prev, named, earlier):
    """The paths whose Git view differs between the work tree and the previous state."""
    if named:
        candidates, ignored = set(), set()
        for name in named:
            rel = relative(root, name)
            if any(d.is_dir() and not d.is_symlink() for d in (root / rel, prev / rel)):
                candidates |= ls_files(root, "-c", "-o", "--exclude-standard", "--", rel)
                candidates |= {f"{rel}/{p}" for p in tree(prev / rel)}
                ignored |= ls_files(root, "-o", "-i", "--exclude-standard", "--", rel)
            elif os.path.lexists(root / rel) or os.path.lexists(prev / rel):
                candidates.add(rel)
            else:
                raise ValueError(f"{name}: in neither the work tree nor the previous state")
        if ignored - candidates:
            print("NOTE: ignored files left out (name them, or fix .gitignore): "
                  + ", ".join(sorted(ignored - candidates)[:10]))
    else:
        candidates = ls_files(root, "-m", "-d", "-o", "--exclude-standard") | set(earlier)
    changed = sorted(p for p in candidates if not same(root / p, prev / p))
    quoted = [p for p in changed if QUOTED_RE.search(p)]
    if quoted:
        raise ValueError(f"paths Git would quote are not supported: {quoted}")
    return changed


def build(prev, nxt, root, changed):
    """`prev` with `changed` taken from the work tree; deletions first so a file may become a directory."""
    shutil.copytree(prev, nxt, symlinks=True)
    for rel in changed:
        if entry(nxt / rel) is not None:
            (nxt / rel).unlink()
            parent = (nxt / rel).parent
            while parent != nxt and not any(parent.iterdir()):
                parent.rmdir()
                parent = parent.parent
    for rel in changed:
        src, dst = root / rel, nxt / rel
        kind = entry(src)
        if kind is None:
            continue
        dst.parent.mkdir(parents=True, exist_ok=True)
        if kind[0] == "l":
            os.symlink(kind[1], dst)
        else:
            shutil.copyfile(src, dst)
            dst.chmod(0o755 if kind[0] == "x" else 0o644)


def chunks(lines):
    starts = [i for i, line in enumerate(lines) if line.startswith("diff --git ")]
    if not starts or starts[0] != 0:
        raise ValueError("unexpected diff output")
    return [lines[a:b] for a, b in zip(starts, starts[1:] + [len(lines)])]


def unstate(name, states):
    for state in states:
        if name.startswith(state + "/"):
            return name[len(state) + 1:]
    raise ValueError(f"diff path {name!r} names no state directory")


def rewrite(chunk, states):
    """One file's diff with the state directory names stripped from its header."""
    end = next((i for i, line in enumerate(chunk) if line.startswith(("@@ ", "GIT binary patch"))), len(chunk))
    moved = {line.split(" ", 2)[1]: unstate(line.split(" ", 2)[2], states)
             for line in chunk[1:end] if line.startswith(("rename from ", "rename to "))}
    body = chunk[0][len("diff --git "):]
    if moved:
        if body not in (f"a/{s}/{moved['from']} b/{t}/{moved['to']}" for s in states for t in states):
            raise ValueError(f"unexpected diff header: {chunk[0]!r}")
        first = f"diff --git a/{moved['from']} b/{moved['to']}"
    else:
        # Unquoted and unrenamed: "a/<state>/<path> b/<state>/<path>" with the same path twice.
        first = next((f"diff --git a/{rest[:n]} b/{rest[:n]}" for s in states for t in states
                      if body.startswith(f"a/{s}/")
                      for rest in [body[len(s) + 3:]] for n in [(len(rest) - len(t) - 4) // 2]
                      if n > 0 and rest == f"{rest[:n]} b/{t}/{rest[:n]}"), None)
        if first is None:
            raise ValueError(f"unexpected diff header: {chunk[0]!r}")
    result = [first]
    for line in chunk[1:end]:
        if line.startswith(("--- a/", "+++ b/")):
            result.append(line[:6] + unstate(line[6:], states))
        elif line.startswith(("rename from ", "rename to ")):
            kind = line.split(" ", 2)[1]
            result.append(f"rename {kind} {moved[kind]}")
        elif line.startswith(HEADER):
            result.append(line)
        else:
            raise ValueError(f"unexpected diff header line: {line!r}")
    return result + chunk[end:]


def make_patch(out, old, new):
    # The ceiling keeps a repository around DIR from lending the diff its config or attributes.
    result = git(out, *DIFF, "--", old, new, ok=(0, 1), env={"GIT_CEILING_DIRECTORIES": str(out.parent)})
    if result.returncode == 0:
        raise ValueError("git diff found no difference between the states")
    lines = text(result).split("\n")
    lines.pop()  # the output ends with a newline
    fixed = [line for chunk in chunks(lines) for line in rewrite(chunk, (old, new))]
    return ("\n".join(fixed) + "\n").encode("utf-8", "surrogateescape")


def cut(out, name, msg_file, paths):
    series, root = load(out)
    check_repo(root, series)
    match = NAME_RE.fullmatch(name)
    if not match:
        raise ValueError(f"--name {name!r} must be NN-slug: two digits, then lowercase words joined by '-'")
    steps = series["steps"]
    if steps and int(match[1]) <= int(steps[-1]["name"][:2]):
        raise ValueError(f"step {name} must come after {steps[-1]['name']}: step numbers ascend")
    message = Path(msg_file).read_bytes()
    if not message.strip():
        raise ValueError(f"{msg_file} is empty")
    prev = out / (steps[-1]["name"] if steps else "base")
    changed = select(root, prev, paths, [p for step in steps for p in step["paths"]])
    if not changed:
        raise ValueError("nothing differs from the previous state")
    try:
        build(prev, out / name, root, changed)
        data = make_patch(out, prev.name, name)
        (out / f"{name}.patch").write_bytes(data)
        (out / f"{name}.msg").write_bytes(message)
    except BaseException:
        remove(out, name)
        raise
    steps.append({"name": name, "paths": changed})
    series["verified"] = {}
    save(out, series)
    (out / "commit.sh").unlink(missing_ok=True)
    print(f"cut {name}: {len(changed)} path(s), {len(data)} B patch")
    print("\n".join(f"  {p}" for p in changed))


def remove(out, name):
    shutil.rmtree(out / name, ignore_errors=True)
    for suffix in (".patch", ".msg"):
        (out / f"{name}{suffix}").unlink(missing_ok=True)


def drop(out):
    series, _ = load(out)
    if not series["steps"]:
        raise ValueError("no steps to drop")
    name = series["steps"].pop()["name"]
    remove(out, name)
    series["verified"] = {}
    save(out, series)
    (out / "commit.sh").unlink(missing_ok=True)
    print(f"dropped {name}")


def apply_delta(base, delta):
    """Git's copy/insert delta format."""
    def varint(pos):
        value = shift = 0
        while True:
            byte = delta[pos]
            pos += 1
            value |= (byte & 0x7F) << shift
            shift += 7
            if not byte & 0x80:
                return value, pos
    size, pos = varint(0)
    target, pos = varint(pos)
    if size != len(base):
        raise ValueError("binary delta expects a different preimage")
    result = bytearray()
    while pos < len(delta):
        op = delta[pos]
        pos += 1
        if op & 0x80:
            offset = length = 0
            for bit in range(7):
                if op & (1 << bit):
                    value = delta[pos] << (8 * (bit if bit < 4 else bit - 4))
                    pos += 1
                    if bit < 4:
                        offset |= value
                    else:
                        length |= value
            length = length or 0x10000
            if offset + length > len(base):
                raise ValueError("binary delta copies past its preimage")
            result += base[offset:offset + length]
        elif op:
            result += delta[pos:pos + op]
            pos += op
        else:
            raise ValueError("binary delta has a zero opcode")
    if len(result) != target:
        raise ValueError("binary delta produced the wrong size")
    return bytes(result)


def decode(lines, base):
    """The forward hunk of a git binary patch: `literal N` or `delta N`, base85 lines of deflated data."""
    kind, size = lines[0].split(" ")
    data = bytearray()
    for line in lines[1:]:
        if not line:
            break
        if not ("A" <= line[0] <= "Z" or "a" <= line[0] <= "z"):
            raise ValueError(f"bad binary hunk line {line!r}")
        data += base64.b85decode(line[1:])[:ord(line[0]) - (64 if line[0] <= "Z" else 70)]
    raw = zlib.decompress(bytes(data))
    if len(raw) != int(size) or kind not in ("literal", "delta"):
        raise ValueError(f"bad binary hunk header {lines[0]!r}")
    return raw if kind == "literal" else apply_delta(base, raw)


def inside(top, rel):
    path = top / rel
    real = os.path.realpath(path.parent)
    if os.path.isabs(rel) or ".." in rel.split("/") or not (real + "/").startswith(os.path.realpath(top) + "/"):
        raise ValueError(f"{rel}: outside the replay tree")
    return path


def apply_binary(top, chunk):
    """GNU patch cannot apply git binary hunks; check their blob ids and apply them here."""
    end = chunk.index("GIT binary patch")
    fields = {}
    for line in chunk[1:end]:
        for key in ("rename from ", "rename to ", "new file mode ", "deleted file mode ", "new mode ", "index "):
            if line.startswith(key):
                fields[key.strip()] = line[len(key):]
    body = chunk[0][len("diff --git "):]
    path = body[2:2 + (len(body) - 5) // 2]
    if "rename from" not in fields and body != f"a/{path} b/{path}":
        raise ValueError(f"unexpected diff header: {chunk[0]!r}")
    old = None if "new file mode" in fields else fields.get("rename from", path)
    new = None if "deleted file mode" in fields else fields.get("rename to", path)
    ids, _, mode = fields.get("index", "").partition(" ")
    old_id, new_id = ids.split("..")
    before = b""
    if old:
        src = inside(top, old)
        if entry(src) is None or entry(src)[0] == "l":
            raise ValueError(f"{old}: no file to patch")
        before = src.read_bytes()
        if blob_id(before) != old_id:
            raise ValueError(f"{old}: content is not blob {old_id}")
        src.unlink()
    if new is None:
        return
    try:
        after = decode(chunk[end + 1:], before)
    except (IndexError, zlib.error) as exc:
        raise ValueError(f"{new}: corrupt binary hunk ({exc})") from None
    if blob_id(after) != new_id:
        raise ValueError(f"{new}: binary hunk does not produce blob {new_id}")
    dst = inside(top, new)
    if os.path.lexists(dst):
        raise ValueError(f"{new}: already exists")
    dst.parent.mkdir(parents=True, exist_ok=True)
    dst.write_bytes(after)
    mode = fields.get("new mode") or fields.get("new file mode") or mode
    dst.chmod(0o755 if mode == "100755" else 0o644)


def replay(top, data, patch):
    """Runs of text chunks through GNU patch, binary chunks through apply_binary, in patch order."""
    lines = data.decode("utf-8", "surrogateescape").split("\n")
    lines.pop()
    pending = []

    def flush():
        if pending:
            stdin = ("\n".join(pending) + "\n").encode("utf-8", "surrogateescape")
            run([patch, "-p1", "--force", "--fuzz=0", "--binary", "--no-backup-if-mismatch", "--reject-file=-"],
                top, stdin=stdin)
            pending.clear()

    for chunk in chunks(lines):
        if "GIT binary patch" in chunk:
            flush()
            apply_binary(top, chunk)
        else:
            pending.extend(chunk)
    flush()


def find_patch(out):
    patch = shutil.which("patch")
    if not patch:
        raise ValueError("verify needs GNU patch on PATH (it replays with patch -p1); install it and rerun")
    if not text(run([patch, "--version"], out)).startswith("GNU patch"):
        raise ValueError(f"{patch} is not GNU patch")
    return patch


def verify(out):
    series, root = load(out)
    check_repo(root, series)
    patch = find_patch(out)
    steps = series["steps"]
    if not steps:
        raise ValueError("no steps cut yet")
    top = out / "verify"
    shutil.rmtree(top, ignore_errors=True)
    export(root, series["head"], top)
    verified = {}
    for step in steps:
        name = step["name"]
        data = (out / f"{name}.patch").read_bytes()
        try:
            replay(top, data, patch)
        except ValueError as exc:
            raise ValueError(f"{name}: {exc}") from None
        wrong = mismatches(top, out / name)
        if wrong:
            raise ValueError(f"{name}: the replayed tree differs from its state in {', '.join(wrong[:10])}")
        verified[name] = hashlib.sha256(data).hexdigest()
        print(f"ok {name}: {len(step['paths'])} path(s)")
    touched = sorted({p for step in steps for p in step["paths"]})
    stale = [p for p in touched if not same(top / p, root / p)]
    if stale:
        raise ValueError(f"the work tree changed after the last cut of {', '.join(stale)}; "
                         "cut another step, or drop and cut again")
    series["verified"] = verified
    save(out, series)
    print(f"Verified {len(steps)} step(s) on {series['head'][:12]}; the final tree matches the work tree.")
    left = sorted(p for p in ls_files(root, "-m", "-d", "-o", "--exclude-standard") if not same(root / p, top / p))
    if left:
        print("NOTE: work tree changes not in the series (commit.sh leaves them as they are):")
        print("\n".join(f"  {p}" for p in left))


def script(out):
    series, root = load(out)
    check_repo(root, series)
    steps = series["steps"]
    if not steps:
        raise ValueError("no steps cut yet")
    digests = {s["name"]: hashlib.sha256((out / f"{s['name']}.patch").read_bytes()).hexdigest() for s in steps}
    if digests != series["verified"]:
        raise ValueError("run verify first: the patches changed after the last passing verify")
    sha, q = series["head"], shlex.quote
    lines = [
        "#!/usr/bin/env bash",
        f"# Generated by scripts/patch-series.py: commits {len(steps)} step(s) onto {sha[:12]} through the",
        "# index (git apply --cached), leaving the working tree as it is. The human runs it.",
        "set -euo pipefail",
        f"series={q(str(out))}",
        f"cd -- {q(str(root))}",
        f"if [[ $(git rev-parse HEAD) != {sha} ]]; then",
        f'    echo "commit.sh: HEAD is no longer {sha[:12]}; cut the series again" >&2',
        "    exit 1",
        "fi",
        "if ! git diff --cached --quiet; then",
        '    echo "commit.sh: the index has staged changes; unstage them first" >&2',
        "    exit 1",
        "fi",
        "(cd -- \"$series\" && sha256sum --check --strict --quiet) <<'EOF'",
        *(f"{digest}  {name}.patch" for name, digest in digests.items()),
        "EOF",
    ]
    for name in digests:
        lines += [f'git apply --cached --whitespace=nowarn -- "$series/{name}.patch"',
                  f'git commit -F "$series/{name}.msg"']
    lines.append("git status --short")
    path = out / "commit.sh"
    path.write_text("\n".join(lines) + "\n")
    path.chmod(0o755)
    print(path.read_text(), end="")
    print(f"\nWrote {path}; the human runs it: bash {q(str(path))}")


def main(argv=None):
    parser = argparse.ArgumentParser(prog="patch-series.py", description=__doc__,
                                     formatter_class=argparse.RawDescriptionHelpFormatter)
    commands = parser.add_subparsers(dest="command", required=True, metavar="COMMAND")
    for name in ("init", "cut", "drop", "verify", "script"):
        sub = commands.add_parser(name, help=f"see {name} above")
        sub.add_argument("--out", type=Path, required=True, metavar="DIR", help="series directory outside the work tree")
        if name == "init":
            sub.add_argument("--root", type=Path, default=Path(__file__).resolve().parent.parent, metavar="REPO",
                             help="work tree (default: this repository)")
        if name == "cut":
            sub.add_argument("--name", required=True, help="NN-slug; NN ascends")
            sub.add_argument("--msg-file", type=Path, required=True, metavar="FILE", help="commit message, copied verbatim")
            sub.add_argument("paths", nargs="*", metavar="PATH", help="relative to the work tree root")
    args = parser.parse_args(argv)
    try:
        out = Path(os.path.realpath(args.out))
        if args.command == "init":
            init(out, args.root)
        elif args.command == "cut":
            cut(out, args.name, args.msg_file, args.paths)
        else:
            {"drop": drop, "verify": verify, "script": script}[args.command](out)
        return 0
    except (OSError, ValueError, subprocess.SubprocessError) as exc:
        print(f"ERROR: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
