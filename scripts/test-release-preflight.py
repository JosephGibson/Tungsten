#!/usr/bin/env python3
"""Real temporary Git repositories; mocked GitHub responses, no network or cargo."""

import contextlib
import importlib.util
import io
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

spec = importlib.util.spec_from_file_location("preflight", Path(__file__).with_name("release-preflight.py"))
pre = importlib.util.module_from_spec(spec)
spec.loader.exec_module(pre)


class Preflight(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        base = Path(temp.name)
        self.root = base / "work"
        self.root.mkdir()
        self.remote = base / "origin.git"
        self.git("init", "-b", "main")
        self.git("config", "user.name", "Release Test")
        self.git("config", "user.email", "release@example.invalid")
        self.git("config", "commit.gpgSign", "false")
        self.git("config", "tag.gpgSign", "false")
        self.git("config", "core.hooksPath", str(base / "no-hooks"))
        self.write("Cargo.toml", '[workspace.package]\nversion = "1.0.0"\n')
        self.write("CHANGELOG.md", "# Changes\n\n## [Unreleased]\n\n## [1.0.0] - 2026-09-26\n\n- First.\n")
        self.write("DESIGN.md", "Workspace `v1.0.0`\n")
        self.write("README.md", "# Engine\n")
        self.commit("first")
        self.sha = self.git("rev-parse", "HEAD")
        self.git("init", "--bare", str(self.remote))
        self.git("remote", "add", "origin", str(self.remote))
        self.git("push", "-u", "origin", "main")
        self.existing = None
        self.runs = []
        self.ci_runs = []
        self.api_calls = []

    def git(self, *args):
        result = subprocess.run(["git", *args], cwd=self.root, text=True, capture_output=True, check=True)
        return result.stdout.strip()

    def write(self, name, text):
        (self.root / name).write_text(text)

    def commit(self, message):
        self.git("add", ".")
        self.git("commit", "-m", message)

    def github(self, root, endpoint, missing=False):
        self.api_calls.append(endpoint)
        if "/git/ref/heads/" in endpoint:
            branch = endpoint.split("/heads/")[1]
            return {"object": {"sha": self.git("--git-dir", str(self.remote), "rev-parse", f"refs/heads/{branch}")}}
        if "/git/ref/tags/" in endpoint:
            tag = endpoint.split("/tags/")[1]
            return {"object": {"sha": self.git("--git-dir", str(self.remote), "rev-parse", f"refs/tags/{tag}")}}
        if "/releases/tags/" in endpoint:
            return self.existing
        if "/workflows/release.yml/" in endpoint:
            return {"workflow_runs": self.runs}
        if "/workflows/ci.yml/" in endpoint:
            return {"workflow_runs": self.ci_runs}
        self.fail(f"Unexpected API lookup: {endpoint}")

    def check(self, version="1.0.0", ref="HEAD", branch="main", rehearsal=False):
        with patch.object(pre, "api", side_effect=self.github), patch.object(pre, "github_repository", return_value="owner/repo"):
            return pre.preflight(self.root, version, ref, "origin", branch, "owner/repo", rehearsal)

    def tag(self, remote=True):
        self.git("tag", "-a", "v1.0.0", self.sha, "-m", "Release")
        if remote:
            self.git("push", "origin", "refs/tags/v1.0.0")

    def run_info(self, tag="v1.0.0", sha=None, status="completed", conclusion="success", event="push", id=42):
        return dict(id=id, head_sha=sha or self.sha, head_branch=tag, status=status,
                    conclusion=conclusion, event=event, html_url=f"https://github.com/owner/repo/actions/runs/{id}")

    def test_new_release_prints_exact_commands_without_writing(self):
        before = self.git("show-ref"), self.git("status", "--porcelain"), (self.root / "README.md").read_bytes()
        errors, info, commands = self.check()
        self.assertEqual(errors, [])
        self.assertEqual(commands[0], ["git", "tag", "-a", "v1.0.0", self.sha, "-m", "Tungsten 1.0.0"])
        self.assertEqual(commands[1][-2:], ["origin", "refs/tags/v1.0.0:refs/tags/v1.0.0"])
        self.assertIn("push.followTags=false", commands[1])
        after = self.git("show-ref"), self.git("status", "--porcelain"), (self.root / "README.md").read_bytes()
        self.assertEqual(before, after)
        self.assertTrue(any("no run for this commit" in line for line in info))

    def test_dirty_untracked_and_staged_changes_fail(self):
        self.write("new.txt", "untracked")
        self.assertIn("dirty", " ".join(self.check()[0]))
        self.git("add", "new.txt")
        self.assertIn("dirty", " ".join(self.check()[0]))
        self.assertEqual(self.api_calls, [])

    def test_unfinished_git_operation_fails_even_with_clean_tree(self):
        for marker in ("MERGE_HEAD", "CHERRY_PICK_HEAD", "rebase-merge", "rebase-apply", "sequencer"):
            with self.subTest(marker=marker):
                path = self.root / self.git("rev-parse", "--git-path", marker)
                path.write_text(self.sha)
                self.assertIn(marker, " ".join(self.check()[0]))
                path.unlink()

    def test_stale_tracking_ref_is_detected_against_live_remote(self):
        self.write("README.md", "new")
        self.commit("advance")
        self.git("push", "origin", "main")
        self.git("update-ref", "refs/remotes/origin/main", self.sha)
        self.assertIn("stale/missing", " ".join(self.check()[0]))

    def test_unpublished_commit_and_wrong_version_fail(self):
        self.write("README.md", "new")
        self.commit("not pushed")
        self.assertIn("not the live", " ".join(self.check()[0]))
        self.assertIn("does not match", " ".join(self.check("2.0.0")[0]))

    def test_reads_selected_commit_not_clean_working_files(self):
        self.write("DESIGN.md", "Workspace `v9.0.0`\n")
        self.commit("bad working version")
        errors, _, commands = self.check(ref=self.sha)
        self.assertEqual(errors, [])
        self.assertIn(self.sha, commands[0])
        self.git("push", "origin", "main")
        self.assertIn("status line says", " ".join(self.check()[0]))

    def test_squash_equivalent_tree_is_not_the_same_commit(self):
        self.git("checkout", "-b", "feature")
        self.write("README.md", "same files")
        self.commit("feature")
        feature_sha = self.git("rev-parse", "HEAD")
        tree = self.git("rev-parse", "HEAD^{tree}")
        self.git("checkout", "main")
        self.git("merge", "--squash", "feature")
        self.commit("squash")
        self.assertEqual(tree, self.git("rev-parse", "HEAD^{tree}"))
        self.git("push", "origin", "main")
        self.assertIn("not the live", " ".join(self.check(ref=feature_sha)[0]))
        self.assertEqual(self.check()[0], [])

    def test_explicit_maintenance_branch(self):
        self.git("checkout", "-b", "maintenance")
        self.git("push", "-u", "origin", "maintenance")
        self.assertEqual(self.check(branch="maintenance")[0], [])

    def test_rehearsal_requires_explicit_flag_and_own_section_is_not_rehearsal(self):
        version = "0.0.0-test.20260926.gabc123"
        self.assertIn("--rehearsal", " ".join(self.check(version)[0]))
        self.assertEqual(self.check(version, rehearsal=True)[0], [])
        self.assertIn("--rehearsal", " ".join(self.check(rehearsal=True)[0]))
        self.write("Cargo.toml", '[workspace.package]\nversion = "1.1.0-rc.1"\n')
        self.write("DESIGN.md", "Workspace `v1.1.0-rc.1`\n")
        self.write("CHANGELOG.md", "## [Unreleased]\n\n## [1.1.0-rc.1] - 2026-09-26\n\n- Candidate.\n")
        self.commit("candidate")
        self.git("push", "origin", "main")
        self.assertEqual(self.check("1.1.0-rc.1")[0], [])

    def test_matching_local_tag_is_not_recreated(self):
        self.tag(remote=False)
        errors, _, commands = self.check()
        self.assertEqual(errors, [])
        self.assertEqual(len(commands), 1)
        self.assertIn("push", commands[0])

    def test_published_release_resumes_by_original_sha_after_branch_advance(self):
        self.tag()
        self.write("README.md", "later")
        self.commit("advance")
        self.git("push", "origin", "main")
        self.existing = {"draft": False, "html_url": "https://github.com/owner/repo/releases/tag/v1.0.0"}
        self.runs = [self.run_info()]
        errors, info, commands = self.check(ref=self.sha)
        self.assertEqual(errors, [])
        self.assertTrue(any("published; verify" in line for line in info))
        self.assertTrue(all(c[0] == "gh" for c in commands))
        self.assertIn("different commit", " ".join(self.check()[0]))

    def test_draft_and_exact_run_selection(self):
        self.tag()
        self.existing = {"draft": True, "html_url": "https://github.com/owner/repo/releases/tag/v1.0.0"}
        self.runs = [self.run_info(id=99, tag="v0.0.0-test"), self.run_info(id=98, sha="f" * 40),
                     self.run_info(id=97, event="workflow_dispatch"),
                     self.run_info(status="in_progress", conclusion=None)]
        self.ci_runs = [self.run_info(tag="main", conclusion="failure"),
                        self.run_info(id=99, event="pull_request", conclusion="success")]
        errors, info, commands = self.check()
        self.assertEqual(errors, [])
        self.assertTrue(any("draft; inspect" in line for line in info))
        self.assertTrue(any("CI (informational)" in line and "failure" in line for line in info))
        self.assertEqual(commands[-1], ["gh", "run", "watch", "42", "--repo", "owner/repo", "--exit-status"])

    def test_remote_tag_without_run_does_not_propose_another_push(self):
        self.tag()
        errors, info, commands = self.check()
        self.assertEqual(errors, [])
        self.assertEqual(commands, [])
        self.assertTrue(any("No matching release run" in line for line in info))

    def test_lightweight_and_conflicting_tag_objects_fail(self):
        self.git("tag", "v1.0.0")
        self.assertIn("not an annotated", " ".join(self.check()[0]))
        self.git("push", "origin", "refs/tags/v1.0.0")
        self.assertIn("remote v1.0.0 is not an annotated", " ".join(self.check()[0]))
        self.git("tag", "-f", "-a", "v1.0.0", self.sha, "-m", "different object")
        self.assertIn("different tag objects", " ".join(self.check()[0]))

    def test_github_repository_mismatch_stops_commands(self):
        with patch.object(pre, "api", return_value={"object": {"sha": "f" * 40}}), patch.object(pre, "github_repository", return_value="owner/repo"):
            errors, _, commands = pre.preflight(self.root, "1.0.0", "HEAD", "origin", "main", "owner/repo")
        self.assertIn("GitHub branch differs", " ".join(errors))
        self.assertEqual(commands, [])

    def test_release_without_tag_is_not_recreated(self):
        self.existing = {"draft": True, "html_url": "https://github.com/owner/repo/releases/tag/v1.0.0"}
        self.assertIn("without the observed remote tag", " ".join(self.check()[0]))

    def test_api_404_is_absence_only_when_requested_other_errors_fail(self):
        for code in (401, 403, 404, 500):
            result = subprocess.CompletedProcess([], 1, "", f"gh: request failed (HTTP {code})")
            with self.subTest(code=code), patch.object(pre, "run", return_value=result):
                if code == 404:
                    self.assertIsNone(pre.api(self.root, "releases/tags/v1.0.0", missing=True))
                else:
                    with self.assertRaisesRegex(ValueError, "GitHub lookup failed"):
                        pre.api(self.root, "releases/tags/v1.0.0", missing=True)
                with self.assertRaises(ValueError):
                    pre.api(self.root, "repo")

    def test_cli_returns_nonzero_and_no_commands_on_failure(self):
        out, err = io.StringIO(), io.StringIO()
        with patch.object(pre, "api", side_effect=ValueError("authentication failed")), patch.object(pre, "github_repository", return_value="owner/repo"), contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
            code = pre.main(["1.0.0", "--root", str(self.root), "--repo", "owner/repo"])
        self.assertEqual(code, 1)
        self.assertNotIn("git tag", out.getvalue())
        self.assertIn("authentication failed", err.getvalue())

    def test_push_url_repository_identity_and_fork_mismatch(self):
        for url in ("https://github.com/owner/repo.git", "git@github-work:owner/repo.git",
                    "ssh://git@github.com/owner/repo", "git@github.com:owner/repo"):
            self.assertEqual(pre.github_repository(url), "owner/repo")
        for url in ("/tmp/repo.git", "https://example.com/owner/repo.git", "git@github.com:repo.git"):
            with self.assertRaises(ValueError):
                pre.github_repository(url)
        self.git("remote", "set-url", "--push", "origin", "git@github.com:fork/repo.git")
        with self.assertRaisesRegex(ValueError, "differs from the push destination"):
            pre.preflight(self.root, "1.0.0", "HEAD", "origin", "main", "owner/repo")


if __name__ == "__main__":
    unittest.main()
