"""Check that the pre-commit matrix receives the staged files."""

import os
from pathlib import Path
import runpy
import subprocess
import tempfile
import unittest
from unittest.mock import patch


SCRIPT = Path(__file__).resolve().parents[1] / "local-platform-checks.py"


class LocalPlatformChecksTests(unittest.TestCase):
    def test_snapshot_excludes_unstaged_and_untracked_content(self):
        git_variables = subprocess.check_output(["git", "rev-parse", "--local-env-vars"], text=True)
        environment = {key: value for key, value in os.environ.items()
                       if key not in git_variables.splitlines()}
        staged_checkout = runpy.run_path(str(SCRIPT))["staged_checkout"]
        with tempfile.TemporaryDirectory() as directory, patch.dict(os.environ, environment, clear=True):
            root = Path(directory) / "repo"
            root.mkdir()
            subprocess.run(["git", "init", "--quiet"], cwd=root, check=True)
            source = root / "fixture.txt"
            source.write_text("staged\n")
            subprocess.run(["git", "add", "fixture.txt"], cwd=root, check=True)
            source.write_text("unstaged\n")
            (root / "untracked.txt").touch()
            with staged_checkout(root, Path(directory)) as snapshot:
                self.assertEqual((snapshot / "fixture.txt").read_text(), "staged\n")
                self.assertFalse((snapshot / "untracked.txt").exists())
            self.assertFalse(snapshot.exists())
