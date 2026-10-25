#!/usr/bin/env python3
"""The commit-msg hook's accept and reject cases (.githooks/commit-msg; docs/operating-rules.md#git)."""

import os
import subprocess
import tempfile
import unittest
from pathlib import Path

HOOK = Path(__file__).resolve().parent.parent / ".githooks" / "commit-msg"


class CommitMsgHook(unittest.TestCase):
    def run_hook(self, message):
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / "COMMIT_EDITMSG"
            path.write_bytes(message.encode())
            return subprocess.run(["sh", str(HOOK), str(path)], capture_output=True, text=True)

    def assert_accepts(self, message):
        result = self.run_hook(message)
        self.assertEqual(result.returncode, 0, f"{message!r} rejected: {result.stderr}")

    def assert_rejects(self, message, reason):
        result = self.run_hook(message)
        self.assertEqual(result.returncode, 1, f"{message!r} accepted")
        self.assertIn(reason, result.stderr)

    def test_the_hook_is_executable(self):
        if os.name == "posix":
            self.assertTrue(os.access(HOOK, os.X_OK))

    def test_conventional_one_line_subjects_pass(self):
        for message in ("feat: add a ratchet\n", "fix(render): clamp the bloom radius", "refactor(core/ecs)!: drop arity queries\n",
                        "docs(plans): 0.108 notes\n", "chore(scripts/check-repo.py): retarget\n", "revert: undo the scissors\n",
                        "feat: " + "a" * 66 + "\n", "feat: a" + "é" * 65 + "\n", "test: em dash — counted once\n"):
            with self.subTest(message=message):
                self.assert_accepts(message)

    def test_comments_scissors_blank_lines_and_trailing_whitespace_are_ignored(self):
        self.assert_accepts("\n\nfeat: subject  \r\n\n# Please enter the commit message.\n# On branch 0.108\n")
        self.assert_accepts("fix: subject\n# ------------------------ >8 ------------------------\n"
                            "# Do not modify or remove the line above.\ndiff --git a/x b/x\n+Body text\n")

    def test_git_generated_messages_pass_whole(self):
        for message in ("Merge branch 'main' into 0.108\n\n# Conflicts:\n#\tjustfile\n",
                        "Merge remote-tracking branch 'origin/main'\n\n* origin/main:\n  Update 0.107\n",
                        'Revert "feat: add a ratchet"\n\nThis reverts commit 0123abc.\n',
                        "fixup! feat: add a ratchet\n", "squash! feat: add a ratchet\n\nMore words.\n",
                        "amend! feat: add a ratchet\n\nfeat: add the ratchet\n"):
            with self.subTest(message=message):
                self.assert_accepts(message)

    def test_an_empty_message_is_left_to_git(self):
        self.assert_accepts("# only comments\n\n")

    def test_malformed_subjects_fail(self):
        for message in ("Update 0.108: the last release\n", "Feat: add a ratchet\n", "feature: add a ratchet\n",
                        "feat:add a ratchet\n", "feat: Add a ratchet\n", "feat(Core): add a ratchet\n",
                        "feat(): add a ratchet\n", "feat!(core): add a ratchet\n", "feat : add a ratchet\n",
                        "merge branch 'main'\n"):
            with self.subTest(message=message):
                self.assert_rejects(message, "the subject is type(scope): summary")

    def test_a_subject_over_72_characters_fails(self):
        self.assert_rejects("feat: " + "a" * 67 + "\n", "the subject is 73 characters, over 72")
        self.assert_rejects("feat: a" + "é" * 66 + "\n", "the subject is 73 characters, over 72")

    def test_a_body_or_trailer_fails(self):
        for message in ("feat: add a ratchet\n\nWhy it exists.\n",
                        "feat: add a ratchet\n\nCo-Authored-By: Someone <someone@example.com>\n",
                        "feat: add a ratchet\nwrapped subject\n"):
            with self.subTest(message=message):
                self.assert_rejects(message, "a message is one line")


if __name__ == "__main__":
    unittest.main()
