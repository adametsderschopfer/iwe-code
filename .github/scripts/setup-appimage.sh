#!/usr/bin/env bash
# Download the pinned AppImage tooling and print the tool paths.
#
# Mirrors the Windows installer setup: pinned versioned downloads verified by
# checksum before any use. Prints the linuxdeploy path on the first line and
# the type2 runtime path on the second line. The tool runs on x86_64 hosts
# and builds x86_64 AppImages.
set -euo pipefail

: "${RUNNER_TEMP:?}"

linuxdeploy_version="1-alpha-20251107-1"
linuxdeploy_checksum="c20cd71e3a4e3b80c3483cef793cda3f4e990aca14014d23c544ca3ce1270b4d"
linuxdeploy_source="https://github.com/linuxdeploy/linuxdeploy/releases/download/${linuxdeploy_version}/linuxdeploy-x86_64.AppImage"
linuxdeploy_target="$RUNNER_TEMP/linuxdeploy-x86_64-${linuxdeploy_version}.AppImage"

runtime_version="20251108"
runtime_checksum="2fca8b443c92510f1483a883f60061ad09b46b978b2631c807cd873a47ec260d"
runtime_source="https://github.com/AppImage/type2-runtime/releases/download/${runtime_version}/runtime-x86_64"
runtime_target="$RUNNER_TEMP/runtime-x86_64-${runtime_version}"

verify() {
  echo "${1}  ${2}" | sha256sum --check --status
}

fetch() {
  if [ ! -s "$2" ] || ! verify "$1" "$2"; then
    curl --fail --location --silent --show-error --output "$2" "$3"
    verify "$1" "$2"
  fi
  chmod +x "$2"
}

fetch "$linuxdeploy_checksum" "$linuxdeploy_target" "$linuxdeploy_source"
fetch "$runtime_checksum" "$runtime_target" "$runtime_source"
echo "$linuxdeploy_target"
echo "$runtime_target"
