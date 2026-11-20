#!/usr/bin/env python3
"""Run the macOS, Windows, and Linux Rust checks from a Mac."""

import argparse
from contextlib import contextmanager
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile


ROOT = Path(__file__).resolve().parents[2]
FLAGS = ["--locked", "--all-targets", "--all-features"]


@contextmanager
def staged_checkout(root, scratch):
    with tempfile.TemporaryDirectory(prefix="iwe-index-", dir=scratch) as directory:
        snapshot = Path(directory)
        subprocess.run(
            ["git", "checkout-index", "--all", f"--prefix={snapshot}{os.sep}"],
            cwd=root,
            check=True,
        )
        yield snapshot


def local_checks(source, cache):
    env = {**os.environ, "CARGO_TARGET_DIR": str(cache),
           "MACOSX_DEPLOYMENT_TARGET": "11.0"}
    subprocess.run(["make", "fmt-check"], cwd=source, env=env, check=True)
    for target in ("aarch64-apple-darwin", "x86_64-apple-darwin",
                   "x86_64-pc-windows-gnu", "i686-pc-windows-gnu"):
        print(f"Checking {target}", flush=True)
        subprocess.run(
            ["cargo", "clippy", *FLAGS, "--target", target, "--", "-D", "warnings"],
            cwd=source, env=env, check=True,
        )


def linux_checks(source):
    context = os.environ.get("IWE_DOCKER_CONTEXT", "colima-iwe")
    docker = ["docker", "--context", context]
    if context == "colima-iwe" and subprocess.run(
        [*docker, "info"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL
    ).returncode != 0:
        subprocess.run(["colima", "start", "--profile", "iwe", "--activate=false"], check=True)
    image = "iwe-local-linux-checks"
    subprocess.run(
        [*docker, "build", "--platform", "linux/amd64", "--tag", image,
         str(source / ".github/local-checks")],
        check=True,
    )
    subprocess.run([
        *docker, "run", "--rm", "--platform", "linux/amd64",
        "--mount", f"type=bind,source={source},target=/workspace,readonly",
        "--mount", "type=volume,source=iwe-check-cargo,target=/usr/local/cargo/registry",
        "--mount", "type=volume,source=iwe-check-target,target=/cargo-target",
        "--env", "CARGO_TARGET_DIR=/cargo-target", "--workdir", "/workspace", image,
        "bash", "-e", "-c",
        """echo 'Checking x86_64-unknown-linux-gnu'
cargo clippy --locked --all-targets --all-features --target x86_64-unknown-linux-gnu -- -D warnings
echo 'Checking i686-unknown-linux-gnu'
PKG_CONFIG_ALLOW_CROSS=1 \
PKG_CONFIG_LIBDIR=/usr/lib/i386-linux-gnu/pkgconfig:/usr/share/pkgconfig \
  cargo clippy --locked --all-targets --all-features --target i686-unknown-linux-gnu -- -D warnings""",
    ], check=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--staged", action="store_true", help="Check a copy of the Git index.")
    args = parser.parse_args()
    if sys.platform != "darwin":
        parser.exit(1, "The local platform matrix requires macOS. Run make preflight on this host.\n")
    for program in ("docker", "x86_64-w64-mingw32-gcc", "i686-w64-mingw32-gcc"):
        if not shutil.which(program):
            parser.exit(1, f"Missing {program}. See docs/ci.md for local platform checks.\n")
    target = ROOT / "target"
    target.mkdir(exist_ok=True)
    cache = target / "local-platform-checks"
    if args.staged:
        with staged_checkout(ROOT, target) as snapshot:
            local_checks(snapshot, cache)
            linux_checks(snapshot)
    else:
        local_checks(ROOT, cache)
        linux_checks(ROOT)


if __name__ == "__main__":
    try:
        main()
    except subprocess.CalledProcessError as error:
        print(f"Local platform checks stopped. Exit code {error.returncode}.", file=sys.stderr)
        sys.exit(1)
