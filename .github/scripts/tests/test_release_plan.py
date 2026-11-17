"""Check release tag validation and the cross-platform build plan."""

import json
import os
from pathlib import Path
import runpy
import tempfile
import unittest
from unittest.mock import patch

PLAN = Path(__file__).resolve().parents[1] / "release-plan.py"


class ReleasePlanTests(unittest.TestCase):
    def test_shared_tag_plans_all_release_targets(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "plan"
            env = {"REF_TYPE": "tag", "REF_NAME": "v1.2.3", "GITHUB_OUTPUT": str(output)}
            with patch.dict(os.environ, env), patch("subprocess.run") as validate_notes:
                runpy.run_path(str(PLAN))
            self.assertEqual(validate_notes.call_args.args[0][-2:], ["--version", "1.2.3"])
            plan = dict(line.split("=", 1) for line in output.read_text().splitlines())
            targets = json.loads(plan["targets"])["include"]
            self.assertCountEqual(
                [target["platform"] for target in targets],
                ["macos-arm64", "macos-x64", "windows-x64", "windows-x86", "linux-x64", "linux-x86"],
            )

    def test_macos_only_plans_branch_checks_and_tagged_builds(self):
        cases = [("branch", "main", ""), ("tag", "macos-v1.2.3", "false")]
        for ref_type, name, flag in cases:
            with self.subTest(ref_type=ref_type), tempfile.TemporaryDirectory() as directory:
                output = Path(directory) / "plan"
                env = {"REF_TYPE": ref_type, "REF_NAME": name, "GITHUB_OUTPUT": str(output),
                       "IWE_MACOS_ONLY": flag}
                with patch.dict(os.environ, env), patch("subprocess.run"):
                    runpy.run_path(str(PLAN))
                plan = dict(line.split("=", 1) for line in output.read_text().splitlines())
                self.assertEqual(json.loads(plan["checks"]), {"os": ["macos-latest"]})
                self.assertEqual(
                    [target["platform"] for target in json.loads(plan["targets"])["include"]],
                    ["macos-arm64", "macos-x64"],
                )

    def test_other_platform_tags_cannot_plan_a_release(self):
        for tag in ("windows-v1.2.3", "linux-v1.2.3"):
            with self.subTest(tag=tag), patch.dict(os.environ, {"REF_TYPE": "tag", "REF_NAME": tag}):
                with self.assertRaisesRegex(ValueError, "vMAJOR.MINOR.PATCH"):
                    runpy.run_path(str(PLAN))
