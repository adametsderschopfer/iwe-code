"""Exercise the main push hook against a disposable Git remote."""

import os
from pathlib import Path
import subprocess
import tempfile
import unittest

HOOKS = Path(__file__).resolve().parents[3] / ".githooks"


class PrePushTests(unittest.TestCase):
    def setUp(self):
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        self.repo = Path(directory.name)
        self.env = os.environ.copy()
        git_vars = subprocess.check_output(["git", "rev-parse", "--local-env-vars"], text=True)
        for name in git_vars.splitlines():
            self.env.pop(name, None)
        self.env["PATH"] = str(self.repo / "bin") + os.pathsep + self.env["PATH"]
        self.env["PREFLIGHT_LOG"] = str(self.repo / "preflight.log")
        self.git("init", "--bare", "remote.git")
        (self.repo / "checkout").mkdir()
        self.repo /= "checkout"
        self.git("init", "-b", "main", ".")
        self.git("config", "core.hooksPath", str(HOOKS))
        self.git("config", "user.name", "Preflight test")
        self.git("config", "user.email", "preflight@example.invalid")
        self.git("commit", "--allow-empty", "-m", "Initial commit")
        tools = self.repo.parent / "bin"
        tools.mkdir()
        make = tools / "make"
        make.write_text('#!/bin/sh\n[ "$*" = preflight ] || exit 2\necho ran >> "$PREFLIGHT_LOG"\nexit "${PREFLIGHT_EXIT:-0}"\n')
        make.chmod(0o755)

    def git(self, *args, check=True):
        return subprocess.run(["git", *args], cwd=self.repo, env=self.env,
                              capture_output=True, text=True, check=check)

    def push(self, ref):
        return self.git("push", "../remote.git", ref, check=False)

    def test_main_push_requires_successful_preflight(self):
        self.env["PREFLIGHT_EXIT"] = "1"
        self.assertNotEqual(self.push("HEAD:main").returncode, 0)
        self.assertEqual(self.git("ls-remote", "../remote.git", "refs/heads/main").stdout, "")
        self.env["PREFLIGHT_EXIT"] = "0"
        result = self.push("HEAD:main")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(Path(self.env["PREFLIGHT_LOG"]).read_text(), "ran\nran\n")

    def test_dirty_worktree_blocks_main_but_allows_topic_push(self):
        (self.repo / "untracked.txt").touch()
        result = self.push("HEAD:main")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Commit or stash", result.stderr)
        self.assertEqual(self.push("HEAD:topic").returncode, 0)
        self.assertFalse(Path(self.env["PREFLIGHT_LOG"]).exists())

    def test_main_push_checks_the_outgoing_commit(self):
        self.git("branch", "earlier")
        self.git("commit", "--allow-empty", "-m", "Next commit")
        result = self.push("earlier:main")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Check out the commit", result.stderr)
        self.assertFalse(Path(self.env["PREFLIGHT_LOG"]).exists())
