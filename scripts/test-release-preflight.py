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
        self.pulls = []
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
        if "/pulls?" in endpoint:
            return self.pulls
        if "/git/commits/" in endpoint:
            return {"tree": {"sha": self.git("rev-parse", endpoint.split("/commits/")[1] + "^{tree}")}}
        if "/workflows/release.yml/" in endpoint:
            return {"workflow_runs": self.runs}
        if "/workflows/ci.yml/" in endpoint:
            return {"workflow_runs": self.ci_runs}
        self.fail(f"Unexpected API lookup: {endpoint}")

    def check(self, version="1.0.0", ref="HEAD", branch="main", rehearsal=False, **options):
        with patch.object(pre, "api", side_effect=self.github), patch.object(pre, "github_repository", return_value="owner/repo"):
            return pre.preflight(self.root, version, ref, "origin", branch, "owner/repo", rehearsal, **options)

    def milestone(self, commit=True):
        """Branch 1.1 carrying the 1.1.0 cut, committed or left in the working tree."""
        self.git("checkout", "-b", "1.1")
        self.write("Cargo.toml", '[workspace.package]\nversion = "1.1.0"\n')
        self.write("CHANGELOG.md", "## [Unreleased]\n\n## [1.1.0] - 2026-09-30\n\n- Second.\n\n"
                   "## [1.0.0] - 2026-09-26\n\n- First.\n")
        self.write("DESIGN.md", "Workspace `v1.1.0`\n")
        if commit:
            self.commit("Update 1.1: second")
            return self.git("rev-parse", "HEAD")

    def pull(self, sha, merged=None, number=7):
        return dict(number=number, state="closed" if merged else "open", merged_at="2026-09-30T00:00:00Z" if merged else None,
                    merge_commit_sha=merged, head={"sha": sha}, html_url=f"https://github.com/owner/repo/pull/{number}")

    def tag(self, remote=True):
        self.git("tag", "-a", "v1.0.0", self.sha, "-m", "Release")
        if remote:
            self.git("push", "origin", "refs/tags/v1.0.0")

    def run_info(self, tag="v1.0.0", sha=None, status="completed", conclusion="success", event="push", id=42):
        return dict(id=id, head_sha=sha or self.sha, head_branch=tag, status=status,
                    conclusion=conclusion, event=event, html_url=f"https://github.com/owner/repo/actions/runs/{id}")

    TAG = ["git", "tag", "-a", "v1.0.0", "-m", "Tungsten 1.0.0"]
    START = ["gh", "workflow", "run", "release.yml", "--repo", "owner/repo", "--ref", "v1.0.0"]
    PULL = ["gh", "pr", "create", "--repo", "owner/repo", "--base", "main", "--head", "1.1", "--title",
            "Update 1.1: second", "--body", "Release v1.1.0, published by its tag push. Merging it publishes nothing."]
    NEXT = [["git", "switch", "-c", "1.2", "1.1"], ["git", "push", "-u", "origin", "1.2"]]
    TAG11 = [["git", "tag", "-a", "v1.1.0", "-m", "Tungsten 1.1.0"], ["git", "push", "origin", "v1.1.0"]]

    def main(self, *argv):
        out = io.StringIO()
        with patch.object(pre, "api", side_effect=self.github), patch.object(pre, "github_repository", return_value="owner/repo"), contextlib.redirect_stdout(out):
            code = pre.main([*argv, "--root", str(self.root), "--repo", "owner/repo"])
        return code, out.getvalue()

    def cut(self, version, branch):
        """A release commit for VERSION on a new BRANCH from HEAD."""
        self.git("checkout", "-q", "-b", branch)
        self.write("Cargo.toml", f'[workspace.package]\nversion = "{version}"\n')
        self.write("CHANGELOG.md", f"## [Unreleased]\n\n## [{version}] - 2026-10-01\n\n- Later.\n\n"
                   "## [1.1.0] - 2026-09-30\n\n- Second.\n\n## [1.0.0] - 2026-09-26\n\n- First.\n")
        self.write("DESIGN.md", f"Workspace `v{version}`\n")
        self.commit(f"chore(release): {version} later")
        return self.git("rev-parse", "HEAD")

    def test_release_from_the_base_branch_prints_exact_commands_without_writing(self):
        before = self.git("show-ref"), self.git("status", "--porcelain"), (self.root / "README.md").read_bytes()
        errors, info, agent, owner = self.check()
        self.assertEqual(errors, [])
        self.assertEqual((agent, owner), ([], [self.TAG, ["git", "push", "origin", "v1.0.0"]]))
        after = self.git("show-ref"), self.git("status", "--porcelain"), (self.root / "README.md").read_bytes()
        self.assertEqual(before, after)
        self.assertTrue(any("no run for this commit" in line for line in info))

    def test_final_release_on_a_version_branch_hands_off_tag_push_next_branch_and_pull_request(self):
        self.milestone()
        before = self.git("show-ref"), self.git("status", "--porcelain")
        errors, _, agent, owner = self.check("1.1.0", branch="1.1")
        self.assertEqual(errors, [])
        self.assertEqual(agent, [["git", "push", "origin", "1.1"]])
        self.assertEqual(owner, [*self.TAG11, *self.NEXT, self.PULL])
        self.assertEqual(before, (self.git("show-ref"), self.git("status", "--porcelain")))
        code, out = self.main("1.1.0")
        self.assertEqual(code, 0)
        self.assertIn("The agent's push:\ngit push origin 1.1\nThe owner's block, whose tag push publishes:\n"
                      "git tag -a v1.1.0 -m 'Tungsten 1.1.0' &&\ngit push origin v1.1.0 &&\n"
                      "git switch -c 1.2 1.1 &&\ngit push -u origin 1.2 &&\ngh pr create --repo owner/repo", out)
        self.assertNotIn("git add", out)
        self.assertNotIn("After the pull request merges", out)
        # The next branch already made: a note in place of its commands.
        self.git("branch", "1.2", "1.1")
        errors, info, _, owner = self.check("1.1.0", branch="1.1")
        self.assertEqual((errors, owner), ([], [*self.TAG11, self.PULL]))
        self.assertTrue(any("1.2 already exists" in line for line in info))
        self.write("README.md", "dirty")
        self.assertIn("dirty", " ".join(self.check("1.1.0", branch="1.1")[0]))
        self.git("checkout", "--", "README.md")
        self.assertIn("does not match", " ".join(self.check("1.2.0", branch="1.1")[0]))
        self.git("tag", "-a", "v1.1.0", "HEAD~1", "-m", "Early")
        self.assertIn("different commit", " ".join(self.check("1.1.0", branch="1.1")[0]))

    def test_pull_request_states_are_information_and_a_pushed_tag_without_a_run_starts_one(self):
        sha = self.milestone()
        self.git("tag", "-a", "v1.1.0", "-m", "Tungsten 1.1.0")
        self.git("push", "-u", "origin", "1.1", "v1.1.0")
        start = ["gh", "workflow", "run", "release.yml", "--repo", "owner/repo", "--ref", "v1.1.0"]
        errors, info, agent, owner = self.check("1.1.0", branch="1.1")
        self.assertEqual((errors, agent, owner), ([], [], [*self.NEXT, self.PULL, start]))
        self.assertTrue(any("No release run found" in line for line in info))
        self.git("branch", "1.2", "1.1")
        self.pulls = [self.pull("f" * 40, merged="f" * 40, number=6), self.pull(sha)]
        errors, info, _, owner = self.check("1.1.0", branch="1.1")
        self.assertEqual((errors, owner), ([], [start]))
        self.assertTrue(any("#7: open; main's merge, which publishes nothing" in line for line in info))
        self.runs = [self.run_info(tag="1.1", sha=sha, event="pull_request", id=40),
                     self.run_info(tag="v1.1.0", sha=sha, conclusion="skipped", id=41),
                     self.run_info(tag="v1.1.0", sha=sha, status="in_progress", conclusion=None)]
        self.assertEqual(self.check("1.1.0", branch="1.1")[3],
                         [["gh", "run", "watch", "42", "--repo", "owner/repo", "--exit-status"]])
        self.pulls = [self.pull(sha, merged="e" * 40)]
        errors, info, _, _ = self.check("1.1.0", branch="1.1")
        self.assertEqual(errors, [])
        self.assertTrue(any("#7: merged; main's merge, which publishes nothing" in line for line in info))

    def test_main_ahead_by_an_earlier_release_merge_passes(self):
        self.milestone()
        self.git("push", "-u", "origin", "1.1")
        later = self.cut("1.2.0", "1.2")
        self.git("checkout", "-q", "main")
        self.git("merge", "-q", "--no-ff", "1.1", "-m", "Merge 1.1")
        self.git("push", "origin", "main")
        self.git("checkout", "-q", "1.2")
        base_tip = self.git("rev-parse", "origin/main")
        self.assertNotEqual(base_tip, self.sha)
        self.assertEqual(pre.foreign_commits(self.root, later, base_tip), [])
        errors, _, agent, owner = self.check("1.2.0", branch="1.2")
        self.assertEqual(errors, [])
        self.assertEqual(agent, [["git", "push", "origin", "1.2"]])
        self.assertEqual(owner[:2], [["git", "tag", "-a", "v1.2.0", "-m", "Tungsten 1.2.0"],
                                     ["git", "push", "origin", "v1.2.0"]])

    def test_main_ahead_by_a_commit_or_a_merge_with_its_own_resolution_is_refused(self):
        self.milestone()
        self.git("push", "-u", "origin", "1.1")
        self.cut("1.2.0", "1.2")
        self.git("checkout", "-q", "main")
        self.git("merge", "-q", "--no-ff", "--no-commit", "1.1")
        self.write("README.md", "resolved differently")
        self.commit("Merge 1.1 with a change of its own")
        self.git("push", "origin", "main")
        self.git("checkout", "-q", "1.2")
        self.assertIn("has commits 1.2 lacks", " ".join(self.check("1.2.0", branch="1.2")[0]))
        self.git("checkout", "-q", "main")
        self.write("README.md", "hotfix")
        self.commit("hotfix")
        self.git("push", "origin", "main")
        self.git("checkout", "-q", "1.2")
        self.assertIn("merge main into 1.2", " ".join(self.check("1.2.0", branch="1.2")[0]))

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

    def test_unpublished_commit_is_pushed_before_its_tag_unless_diverged(self):
        self.write("README.md", "new")
        self.commit("not pushed")
        errors, _, agent, owner = self.check()
        self.assertEqual(errors, [])
        # main's push stays the owner's; a version branch's is the agent's.
        self.assertEqual((agent, owner[:3]), ([], [["git", "push", "origin", "main"], self.TAG,
                                                    ["git", "push", "origin", "v1.0.0"]]))
        self.assertIn("does not match", " ".join(self.check("2.0.0")[0]))
        self.git("push", "origin", "main")
        self.git("reset", "--hard", self.sha)
        self.write("README.md", "diverged")
        self.commit("diverged")
        self.assertIn("has commits the selected commit lacks", " ".join(self.check()[0]))
        self.assertIn("local main is not the selected commit", " ".join(self.check(ref=self.sha)[0]))

    def test_reads_selected_commit_not_clean_working_files(self):
        self.write("DESIGN.md", "Workspace `v9.0.0`\n")
        self.commit("bad working version")
        errors, _, _, owner = self.check(ref=self.sha)
        self.assertEqual(errors, [])
        self.assertEqual(owner[:2], [["git", "tag", "-a", "v1.0.0", self.sha, "-m", "Tungsten 1.0.0"],
                                     ["git", "push", "origin", "v1.0.0"]])
        self.git("push", "origin", "main")
        self.assertIn("status line says", " ".join(self.check()[0]))

    def test_maintenance_branch_without_a_pull_request_publishes_on_its_tag_push(self):
        self.git("checkout", "-b", "maintenance")
        self.git("push", "-u", "origin", "maintenance")
        errors, _, agent, owner = self.check(branch="maintenance", no_pr=True)
        self.assertEqual(errors, [])
        self.assertEqual((agent, owner), ([], [self.TAG, ["git", "push", "origin", "v1.0.0"]]))
        self.assertEqual(self.check(branch="maintenance")[3][-1][:3], ["gh", "pr", "create"])

    def test_rehearsal_requires_explicit_flag_and_own_section_is_not_rehearsal(self):
        version = "0.0.0-test.20260926.gabc123"
        self.assertIn("--rehearsal", " ".join(self.check(version)[0]))
        errors, _, agent, owner = self.check(version, rehearsal=True)
        self.assertEqual(errors, [])
        # The rehearsal block: a pushed tag starts its own run; no next branch or pull request.
        self.assertEqual((agent, owner), ([], [["git", "tag", "-a", "v" + version, "-m", "Tungsten " + version],
                                               ["git", "push", "origin", "v" + version]]))
        code, out = self.main(version, "--rehearsal")
        self.assertEqual(code, 0)
        self.assertIn(f"The owner's block, whose tag push publishes:\ngit tag -a v{version} -m 'Tungsten {version}' &&\n"
                      f"git push origin v{version}\n", out)
        self.assertIn("--rehearsal", " ".join(self.check(rehearsal=True)[0]))
        self.write("Cargo.toml", '[workspace.package]\nversion = "1.1.0-rc.1"\n')
        self.write("DESIGN.md", "Workspace `v1.1.0-rc.1`\n")
        self.write("CHANGELOG.md", "## [Unreleased]\n\n## [1.1.0-rc.1] - 2026-09-26\n\n- Candidate.\n")
        self.commit("candidate")
        self.git("push", "origin", "main")
        self.assertEqual(self.check("1.1.0-rc.1")[0], [])

    def test_matching_local_tag_is_not_recreated(self):
        self.tag(remote=False)
        errors, _, _, owner = self.check()
        self.assertEqual(errors, [])
        self.assertEqual(owner, [["git", "push", "origin", "v1.0.0"]])

    def test_published_release_resumes_by_original_sha_after_branch_advance(self):
        self.tag()
        self.write("README.md", "later")
        self.commit("advance")
        self.git("push", "origin", "main")
        self.existing = {"draft": False, "html_url": "https://github.com/owner/repo/releases/tag/v1.0.0"}
        self.runs = [self.run_info()]
        errors, info, agent, owner = self.check(ref=self.sha)
        self.assertEqual(errors, [])
        self.assertTrue(any("published; verify" in line for line in info))
        self.assertTrue(agent == [] and all(c[0] == "gh" for c in owner))
        self.assertIn("different commit", " ".join(self.check()[0]))

    def test_draft_and_exact_run_selection(self):
        self.tag()
        self.existing = {"draft": True, "html_url": "https://github.com/owner/repo/releases/tag/v1.0.0"}
        self.runs = [self.run_info(id=99, tag="v0.0.0-test"), self.run_info(id=98, sha="f" * 40),
                     self.run_info(id=97, event="pull_request"), self.run_info(id=96, conclusion="skipped"),
                     self.run_info(status="in_progress", conclusion=None)]
        self.ci_runs = [self.run_info(tag="main", conclusion="failure"),
                        self.run_info(id=99, event="pull_request", conclusion="success")]
        errors, info, _, owner = self.check()
        self.assertEqual(errors, [])
        self.assertTrue(any("draft; inspect" in line for line in info))
        self.assertTrue(any("CI (informational)" in line and "failure" in line for line in info))
        self.assertEqual(owner[-1], ["gh", "run", "watch", "42", "--repo", "owner/repo", "--exit-status"])
        self.runs = [self.run_info(event="workflow_dispatch")]
        self.assertEqual(self.check()[3][-1], ["gh", "run", "view", "42", "--repo", "owner/repo"])

    def test_remote_tag_without_run_starts_one_instead_of_another_push(self):
        self.tag()
        errors, info, agent, owner = self.check()
        self.assertEqual(errors, [])
        self.assertEqual((agent, owner), ([], [self.START]))
        self.assertTrue(any("No release run found" in line for line in info))

    def test_lightweight_and_conflicting_tag_objects_fail(self):
        self.git("tag", "v1.0.0")
        self.assertIn("not an annotated", " ".join(self.check()[0]))
        self.git("push", "origin", "refs/tags/v1.0.0")
        self.assertIn("remote v1.0.0 is not an annotated", " ".join(self.check()[0]))
        self.git("tag", "-f", "-a", "v1.0.0", self.sha, "-m", "different object")
        self.assertIn("different tag objects", " ".join(self.check()[0]))

    def test_github_repository_mismatch_stops_commands(self):
        with patch.object(pre, "api", return_value={"object": {"sha": "f" * 40}}), patch.object(pre, "github_repository", return_value="owner/repo"):
            errors, _, agent, owner = pre.preflight(self.root, "1.0.0", "HEAD", "origin", "main", "owner/repo")
        self.assertIn("GitHub branch differs", " ".join(errors))
        self.assertEqual((agent, owner), ([], []))

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
