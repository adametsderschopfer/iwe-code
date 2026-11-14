#!/usr/bin/env bash
# Download the pinned linuxdeploy AppImage tool and print its path.
#
# Mirrors the Windows installer setup: a pinned versioned download verified
# by checksum before any use. The tool runs on x86_64 hosts and builds
# x86_64 AppImages.
set -euo pipefail

: "${RUNNER_TEMP:?}"

version="1-alpha-20251107-1"
checksum="c20cd71e3a4e3b80c3483cef793cda3f4e990aca14014d23c544ca3ce1270b4d"
source_url="https://github.com/linuxdeploy/linuxdeploy/releases/download/${version}/linuxdeploy-x86_64.AppImage"
target="$RUNNER_TEMP/linuxdeploy-x86_64-${version}.AppImage"

verify() {
  echo "${checksum}  $target" | sha256sum --check --status
}

if [ ! -f "$target" ] || ! verify; then
  curl --fail --location --silent --show-error --output "$target" "$source_url"
  verify
fi
chmod +x "$target"
echo "$target"
