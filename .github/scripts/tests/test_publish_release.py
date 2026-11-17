"""Exercise release publication with local Git refs and a fake GitHub CLI."""

import os
from pathlib import Path
import shutil
import subprocess
from tempfile import TemporaryDirectory
import unittest

SCRIPTS = Path(__file__).resolve().parents[1]
ROOT = SCRIPTS.parents[1]
VERSION = "1.4.0"
TAG = f"v{VERSION}"
PACKAGES = (
    "macos-arm64.dmg", "macos-x64.dmg",
    "windows-x64.zip", "windows-x86.zip",
    "windows-x64-setup.exe", "windows-x86-setup.exe",
    "linux-x64.tar.gz", "linux-x86.tar.gz",
    "linux-x64.deb", "linux-x86.deb", "linux-x64.AppImage",
)
PLATFORMS = ("macos-arm64", "macos-x64", "windows-x64", "windows-x86", "linux-x64", "linux-x86")


class PublishReleaseTests(unittest.TestCase):
    def setUp(self):
        folder = self.enterContext(TemporaryDirectory())
        self.root = Path(folder)
        self.repo = self.root / "checkout"
        self.repo.mkdir()
        self.remote = self.root / "remote.git"
        self.git("init", "-q", "--bare", str(self.remote))
        self.git("init", "-q")
        self.git("config", "user.email", "test@example.com")
        self.git("config", "user.name", "Release Test")
        self.git("remote", "add", "origin", str(self.remote))
        self.git("commit", "-qm", "Before preparation", "--allow-empty")
        self.git("commit", "-qm", "Prepared version", "--allow-empty")
        self.sha = self.git("rev-parse", "HEAD")
        self.git("push", "-q", "origin", "HEAD:refs/heads/main")

        dist = self.repo / "dist"
        dist.mkdir()
        self.files = [
            *(f"iwe-code-{VERSION}-{name}" for name in PACKAGES),
            *(f"iwe-{VERSION}-{name}-update.tar.gz" for name in PLATFORMS),
            "updates.json", "updates.sig",
        ]
        for name in self.files:
            (dist / name).write_text("Fixture")

        scripts = self.repo / ".github/scripts"
        scripts.mkdir(parents=True)
        shutil.copy(SCRIPTS / "release-notes.py", scripts)
        ledger = Path("website/app/release-notes/releases.json")
        (self.repo / ledger).parent.mkdir(parents=True)
        shutil.copy(ROOT / ledger, self.repo / ledger)
        self.calls = self.root / "gh-args"
        gh = self.root / "gh"
        gh.write_text('#!/usr/bin/env bash\nprintf "%s\\n" "$@" > "$GH_ARGS"\n')
        gh.chmod(0o755)

    def git(self, *args):
        return subprocess.check_output(["git", *args], cwd=self.repo, text=True).strip()

    def publish(self, tag=TAG):
        env = dict(os.environ, RELEASE_TAG=tag, GH_ARGS=str(self.calls),
                   PATH=f"{self.root}{os.pathsep}{os.environ['PATH']}")
        return subprocess.run(["bash", str(SCRIPTS / "publish-release.sh")],
                              cwd=self.repo, env=env, capture_output=True, text=True)

    def test_publication_passes_all_assets_and_the_exact_commit_to_github(self):
        result = self.publish()
        self.assertEqual(result.returncode, 0, result.stderr)
        args = self.calls.read_text().splitlines()
        self.assertEqual(args[:3], ["release", "create", TAG])
        self.assertEqual(args[args.index("--target") + 1], self.sha)
        self.assertEqual(
            set(args[3:args.index("--target")]),
            {f"dist/{name}" for name in [*self.files, "SHA256SUMS"]},
        )
        self.assertNotIn("--verify-tag", args)
        self.assertEqual(self.git("ls-remote", "--tags", "origin"), "")

    def test_macos_release_publishes_only_macos_packages(self):
        result = self.publish(f"macos-{TAG}")
        self.assertEqual(result.returncode, 0, result.stderr)
        args = self.calls.read_text().splitlines()
        self.assertEqual(args[:3], ["release", "create", f"macos-{TAG}"])
        published = {name for name in self.files if "macos" in name or name.startswith("updates.")}
        self.assertEqual(
            set(args[3:args.index("--target")]),
            {f"dist/{name}" for name in [*published, "SHA256SUMS"]},
        )

    def test_missing_package_stops_before_publication(self):
        (self.repo / "dist" / f"iwe-code-{VERSION}-windows-x86-setup.exe").unlink()
        self.assertNotEqual(self.publish().returncode, 0)
        self.assertFalse(self.calls.exists())
        self.assertEqual(self.git("ls-remote", "--tags", "origin"), "")

    def test_conflicting_remote_tag_stops_before_publication(self):
        self.git("tag", TAG, "HEAD^")
        self.git("push", "-q", "origin", f"refs/tags/{TAG}")
        result = self.publish()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("points to a different commit", result.stderr)
        self.assertFalse(self.calls.exists())

    def test_annotated_tag_on_prepared_commit_can_be_reused(self):
        self.git("tag", "-am", "Existing tag", TAG, self.sha)
        self.git("push", "-q", "origin", f"refs/tags/{TAG}")
        result = self.publish()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue(self.calls.exists())
