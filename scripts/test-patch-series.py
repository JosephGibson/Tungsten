#!/usr/bin/env python3
"""Real temporary Git repositories: the tool reads them, the generated commit.sh commits in them."""

import contextlib
import hashlib
import importlib.util
import io
import json
import os
import random
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

spec = importlib.util.spec_from_file_location("patch_series", Path(__file__).with_name("patch-series.py"))
ps = importlib.util.module_from_spec(spec)
spec.loader.exec_module(ps)

READ_ONLY = {"archive", "ls-files", "rev-parse", "status", "diff"}
BIG = random.Random(7).randbytes(4096)


class PatchSeries(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        base = Path(os.path.realpath(temp.name))
        self.root, self.out, self.msgs = base / "work", base / "series", base / "msgs"
        self.root.mkdir()
        self.msgs.mkdir()
        self.git("init", "-b", "main")
        for key, value in (("user.name", "Patch Test"), ("user.email", "patch@example.invalid"),
                           ("commit.gpgSign", "false"), ("core.autocrlf", "false"),
                           ("core.hooksPath", str(base / "no-hooks"))):
            self.git("config", key, value)
        self.write(".gitignore", "*.log\n")
        self.write("README.md", "# Engine\n\nOne.\nTwo.\nThree.\n")
        self.write("docs/guide.md", "".join(f"Guide line {i}.\n" for i in range(20)))
        self.write("docs/crlf.txt", "one\r\ntwo\r\n")
        self.write("docs/no-newline.txt", "last line")
        self.write("tool.sh", "#!/bin/sh\necho tool\n", executable=True)
        self.write("assets/old.bin", b"\x00\x01old\xff" * 4)
        self.write("assets/big.bin", BIG)
        self.git("add", ".")
        self.git("commit", "-m", "first")
        self.base = self.git("rev-parse", "HEAD")

    def git(self, *args):
        result = subprocess.run(["git", *args], cwd=self.root, text=True, capture_output=True, check=True,
                                env=dict(os.environ, GIT_OPTIONAL_LOCKS="0"))
        return result.stdout.rstrip("\n")

    def write(self, name, data, executable=False):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data.encode() if isinstance(data, str) else data)
        path.chmod(0o755 if executable else 0o644)

    def files(self):
        """Mode and bytes of every work tree file and symlink outside .git."""
        found = {}
        for base, dirs, names in os.walk(self.root):
            dirs[:] = [d for d in dirs if not (d == ".git" and base == str(self.root))]
            for name in names + [d for d in dirs if os.path.islink(os.path.join(base, d))]:
                path = Path(base, name)
                found[str(path.relative_to(self.root))] = (
                    path.lstat().st_mode, os.readlink(path) if path.is_symlink() else path.read_bytes())
        return found

    def repo(self):
        index = self.root / ".git/index"
        return (self.git("rev-parse", "HEAD"), index.read_bytes(), index.stat().st_mtime_ns,
                self.git("status", "--porcelain", "--untracked-files=all"), self.files())

    def tool(self, command, *args, code=0, out=None):
        """One in-process tool call: the repository must not change and every command must be read-only."""
        before, calls, real = self.repo(), [], ps.run

        def record(argv, cwd, *rest, **kwargs):
            calls.append(([str(a) for a in argv], Path(cwd)))
            return real(argv, cwd, *rest, **kwargs)

        output = io.StringIO()
        with (patch.object(ps, "run", side_effect=record), contextlib.redirect_stdout(output),
              contextlib.redirect_stderr(output)):
            result = ps.main([command, "--out", str(out or self.out), *args])
        self.assertEqual(self.repo(), before, f"{command} changed the repository")
        for argv, cwd in calls:
            if argv[0] == "git":
                words = argv[1:]
                while words[0] == "-c":
                    words = words[2:]
                self.assertIn(words[0], READ_ONLY)
                self.assertTrue(words[0] != "diff" or "--no-index" in words, argv)
                self.assertTrue(words[0] != "status" or "--porcelain" in words, argv)
            else:
                self.assertEqual(Path(argv[0]).name, "patch")
                self.assertTrue(cwd.is_relative_to(self.out), cwd)
        self.assertEqual(result, code, output.getvalue())
        return output.getvalue()

    def init(self):
        return self.tool("init", "--root", str(self.root))

    def cut(self, name, *paths, code=0):
        message = self.msgs / f"{name}.txt"
        message.write_text(f"Step {name}\n\nWhat {name} changes.\n")
        return self.tool("cut", "--name", name, "--msg-file", str(message), *paths, code=code)

    def steps(self):
        return json.loads((self.out / "series.json").read_text())["steps"]

    def patch_text(self, name):
        return (self.out / f"{name}.patch").read_text(errors="replace")

    def handoff(self, *steps):
        """init; per step change the work tree and cut (default paths unless named); verify; script; commit."""
        self.init()
        for name, change, *paths in steps:
            change()
            self.cut(name, *paths)
        self.tool("verify")
        self.tool("script")
        self.commit()

    def tree(self, commit):
        found = {}
        for record in self.git("ls-tree", "-r", "-z", commit).split("\0"):
            if record:
                meta, path = record.split("\t", 1)
                mode, _, oid = meta.split(" ")
                found[path] = (mode, oid)
        return found

    def state(self, top):
        """Mode and blob id of every file and symlink in a state directory, computed without the tool."""
        found = {}
        for base, dirs, names in os.walk(top):
            for name in names + [d for d in dirs if os.path.islink(os.path.join(base, d))]:
                path = os.path.join(base, name)
                if os.path.islink(path):
                    mode, data = "120000", os.readlink(path).encode()
                else:
                    mode = "100755" if os.stat(path).st_mode & 0o100 else "100644"
                    data = Path(path).read_bytes()
                found[os.path.relpath(path, top)] = (mode, hashlib.sha1(b"blob %d\0" % len(data) + data).hexdigest())
        return found

    def commit(self, code=0, left=""):
        """Run commit.sh in the temporary repository; check commits, trees, messages and an untouched work tree."""
        work = self.files()
        result = subprocess.run(["bash", str(self.out / "commit.sh")], capture_output=True, text=True)
        self.assertEqual(result.returncode, code, result.stderr)
        self.assertEqual(self.files(), work, "commit.sh touched the working tree")
        if code:
            return result.stderr
        names = [step["name"] for step in self.steps()]
        self.assertEqual(self.git("rev-list", "--count", f"{self.base}..HEAD"), str(len(names)))
        for back, name in enumerate(reversed(names)):
            self.assertEqual(self.tree(f"HEAD~{back}"), self.state(self.out / name), name)
            self.assertEqual(self.git("log", "-1", "--format=%B", f"HEAD~{back}"),
                             (self.msgs / f"{name}.txt").read_text().strip())
        self.assertEqual(self.git("status", "--porcelain", "--untracked-files=all"), left)
        if shutil.which("shellcheck"):
            lint = subprocess.run(["shellcheck", str(self.out / "commit.sh")], capture_output=True, text=True)
            self.assertEqual(lint.returncode, 0, lint.stdout)
        return result.stdout

    def test_text_edit(self):
        def edit():
            self.write("README.md", "# Engine\n\nOne.\nTWO.\nThree.\n")
            self.write("docs/crlf.txt", "one\r\nTWO\r\n")
            self.write("docs/no-newline.txt", "last line, edited")
        self.handoff(("01-text", edit))
        text = self.patch_text("01-text")
        self.assertIn("diff --git a/README.md b/README.md\n", text)
        self.assertIn("--- a/docs/crlf.txt\n+++ b/docs/crlf.txt\n", text)
        self.assertNotIn("base/", text)

    def test_new_file(self):
        self.handoff(("01-add", lambda: (self.write("docs/new/page.md", "New page.\n"), self.write("empty.txt", ""))))
        self.assertIn("diff --git a/docs/new/page.md b/docs/new/page.md\nnew file mode 100644\n", self.patch_text("01-add"))

    def test_deleted_file(self):
        self.handoff(("01-delete", lambda: (self.root / "docs/guide.md").unlink()))
        self.assertIn("diff --git a/docs/guide.md b/docs/guide.md\ndeleted file mode 100644\n",
                      self.patch_text("01-delete"))

    def test_binary_add_change_and_delete(self):
        def change():
            self.write("assets/new.bin", b"\x00" + random.Random(8).randbytes(63))
            self.write("assets/big.bin", BIG[:100] + b"\x00changed\x00" + BIG[109:])
            (self.root / "assets/old.bin").unlink()
        self.handoff(("01-binary", change))
        text = self.patch_text("01-binary")
        self.assertIn("new file mode 100644\nindex 0000000000000000000000000000000000000000..", text)
        self.assertIn("GIT binary patch\nliteral ", text)
        self.assertIn("GIT binary patch\ndelta ", text)
        self.assertIn("diff --git a/assets/old.bin b/assets/old.bin\ndeleted file mode 100644\n", text)

    def test_rename(self):
        def move():
            guide = (self.root / "docs/guide.md").read_text()
            (self.root / "docs/guide.md").unlink()
            self.write("manual/guide.md", guide.replace("line 3.", "line three."))
            os.rename(self.root / "README.md", self.root / "README.txt")
        self.handoff(("01-rename", move))
        text = self.patch_text("01-rename")
        self.assertIn("diff --git a/docs/guide.md b/manual/guide.md\n", text)
        self.assertIn("rename from docs/guide.md\nrename to manual/guide.md\n", text)
        self.assertIn("similarity index 100%\nrename from README.md\nrename to README.txt\n", text)

    def test_exec_bit(self):
        def modes():
            (self.root / "tool.sh").chmod(0o644)
            (self.root / "README.md").chmod(0o755)
            self.write("bin/run.sh", "#!/bin/sh\necho run\n", executable=True)
        self.handoff(("01-modes", modes))
        text = self.patch_text("01-modes")
        self.assertIn("diff --git a/tool.sh b/tool.sh\nold mode 100755\nnew mode 100644\n", text)
        self.assertIn("old mode 100644\nnew mode 100755\n", text)
        self.assertIn("new file mode 100755\n", text)

    def test_later_step_touches_an_earlier_step_file(self):
        self.handoff(
            ("01-first", lambda: (self.write("README.md", "# Engine\n\nOne.\nTwo, first edit.\nThree.\n"),
                                  self.write("notes.txt", "added in 01\n"))),
            ("02-second", lambda: (self.write("README.md", "# Engine\n\nOne.\nTwo, second edit.\nThree.\n"),
                                   (self.root / "notes.txt").unlink())))
        # notes.txt was never tracked: only the earlier step's path list finds its deletion.
        self.assertEqual(self.steps()[1]["paths"], ["README.md", "notes.txt"])
        self.assertIn("-Two, first edit.\n+Two, second edit.\n", self.patch_text("02-second"))

    def test_symlink_named_directories_and_unrelated_work(self):
        self.write("README.md", "# Engine\n\nUncommitted before init.\n")
        self.assertIn("already uncommitted", self.init())
        self.write(".claude/skills/demo/SKILL.md", "---\nname: demo\n---\n")
        (self.root / ".agents/skills").mkdir(parents=True)
        os.symlink("../../.claude/skills/demo", self.root / ".agents/skills/demo")
        self.write(".claude/skills/demo/debug.log", "ignored\n")
        self.write("scratch.txt", "not part of the step\n")
        cut = self.cut("01-skill", ".claude/skills/demo/", ".agents/skills/demo")
        self.assertIn("NOTE: ignored files left out (name them, or fix .gitignore): .claude/skills/demo/debug.log", cut)
        self.assertEqual(self.steps()[0]["paths"], [".agents/skills/demo", ".claude/skills/demo/SKILL.md"])
        self.assertIn("new file mode 120000\n", self.patch_text("01-skill"))
        self.assertIn("  README.md\n  scratch.txt\n", self.tool("verify"))
        self.tool("script")
        self.commit(left=" M README.md\n?? scratch.txt")

    def test_verify_catches_tampered_patches(self):
        self.init()
        self.write("README.md", "# Engine\n\nOne.\nTWO.\nThree.\n")
        self.cut("01-text")
        self.write("assets/big.bin", BIG[::-1])
        self.cut("02-binary")
        self.tool("verify")
        self.tool("script")

        def corrupt(text):
            lines = text.split("\n")
            at = next(i for i, line in enumerate(lines) if line.startswith(("literal ", "delta "))) + 1
            lines[at] = lines[at][:5] + ("0" if lines[at][5] != "0" else "1") + lines[at][6:]
            return "\n".join(lines)

        for name, tamper in (("01-text", lambda text: text.replace("+TWO.", "+TOO.")), ("02-binary", corrupt)):
            with self.subTest(name=name):
                path = self.out / f"{name}.patch"
                original = path.read_bytes()
                path.write_bytes(tamper(original.decode()).encode())
                self.assertIn("run verify first", self.tool("script", code=1))
                self.assertIn(f"ERROR: {name}:", self.tool("verify", code=1))
                path.write_bytes(original)
        self.tool("verify")

    def test_out_inside_the_work_tree_is_refused(self):
        link = self.root.parent / "link"
        link.symlink_to(self.root / "docs")
        for out in (self.root, self.root / "series", link / "series"):
            with self.subTest(out=out):
                self.assertIn("inside the work tree", self.tool("init", "--root", str(self.root), code=1, out=out))
        self.assertFalse((self.root / "docs/series").exists())

    def test_step_names_ascend_and_steps_need_changes(self):
        self.init()
        self.write("README.md", "changed\n")
        for name in ("2-short", "02_under", "02-Upper", "02-", "102-long"):
            self.assertIn("NN-slug", self.cut(name, code=1))
        self.cut("02-first")
        self.write("README.md", "changed again\n")
        self.assertIn("ascend", self.cut("02-again", code=1))
        self.assertIn("ascend", self.cut("01-earlier", code=1))
        self.cut("03-next", "README.md")
        self.assertIn("nothing differs", self.cut("04-empty", code=1))
        self.assertIn("neither", self.cut("04-typo", "no/such/file", code=1))
        self.assertIn("not a path inside", self.cut("04-outside", "../elsewhere", code=1))
        self.assertEqual(sorted(p.name for p in self.out.iterdir()),
                         ["02-first", "02-first.msg", "02-first.patch", "03-next", "03-next.msg", "03-next.patch",
                          "base", "series.json"])

    def test_head_and_index_guards(self):
        self.init()
        self.write("README.md", "changed\n")
        self.cut("01-readme")
        self.tool("verify")
        self.tool("script")
        self.write("docs/guide.md", "staged\n")
        self.git("add", "docs/guide.md")
        self.assertIn("staged changes", self.tool("verify", code=1))
        self.assertIn("staged changes", self.commit(code=1))
        self.git("reset", "-q")
        self.git("commit", "-q", "--allow-empty", "-m", "moved")
        self.assertIn("HEAD moved", self.tool("cut", "--name", "02-late", "--msg-file", str(self.msgs / "01-readme.txt"),
                                              code=1))
        self.assertIn("HEAD is no longer", self.commit(code=1))
        self.assertEqual(self.git("rev-list", "--count", f"{self.base}..HEAD"), "1")

    def test_verify_fails_clearly_without_gnu_patch(self):
        self.init()
        self.write("README.md", "changed\n")
        self.cut("01-readme")
        bin_dir = self.root.parent / "bin"
        bin_dir.mkdir()
        (bin_dir / "git").symlink_to(shutil.which("git"))
        with patch.dict(os.environ, {"PATH": str(bin_dir)}):
            self.assertIn("needs GNU patch", self.tool("verify", code=1))
        self.assertIn("run verify first", self.tool("script", code=1))

    def test_drop_then_cut_again(self):
        self.init()
        self.write("README.md", "first\n")
        self.cut("01-readme")
        self.write("docs/guide.md", "wrong\n")
        self.cut("02-guide")
        self.assertIn("dropped 02-guide", self.tool("drop"))
        self.assertEqual([p.name for p in self.out.glob("02-*")], [])
        self.write("docs/guide.md", "right\n")
        self.cut("02-guide")
        self.tool("verify")
        self.tool("script")
        self.commit()

    def test_help_names_every_command(self):
        output = io.StringIO()
        with contextlib.redirect_stdout(output), self.assertRaises(SystemExit):
            ps.main(["--help"])
        for word in ("init", "cut", "drop", "verify", "script", "git apply --cached", "NN-slug"):
            self.assertIn(word, output.getvalue())


if __name__ == "__main__":
    unittest.main()
