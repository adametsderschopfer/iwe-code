#!/usr/bin/env bash
set -euo pipefail

if [[ ! "$RELEASE_TAG" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "Use a stable release tag in the form vMAJOR.MINOR.PATCH." >&2
  exit 1
fi
version=${RELEASE_TAG#v}
assets=(
  "dist/iwe-code-$version-macos-arm64.dmg" "dist/iwe-code-$version-macos-x64.dmg"
  "dist/iwe-code-$version-windows-x64.zip" "dist/iwe-code-$version-windows-x86.zip"
  "dist/iwe-code-$version-windows-x64-setup.exe" "dist/iwe-code-$version-windows-x86-setup.exe"
  "dist/iwe-code-$version-linux-x64.tar.gz" "dist/iwe-code-$version-linux-x86.tar.gz"
  "dist/iwe-code-$version-linux-x64.deb" "dist/iwe-code-$version-linux-x86.deb"
  "dist/iwe-code-$version-linux-x64.AppImage"
)
for platform in macos-arm64 macos-x64 windows-x64 windows-x86 linux-x64 linux-x86; do
  assets+=("dist/iwe-$version-$platform-update.tar.gz")
done
for asset in "${assets[@]}"; do
  test -s "$asset"
done
(
  cd dist
  sha256sum -- iwe-* > SHA256SUMS
)
python3 .github/scripts/release-notes.py --version "$version" --output dist/release-notes.md
test -s dist/updates.json
test -s dist/updates.sig
release_sha=$(git rev-parse HEAD)
tag_ref="refs/tags/$RELEASE_TAG"
remote_refs=$(git ls-remote --tags origin "$tag_ref" "$tag_ref^{}")
tag_sha=$(awk -v ref="$tag_ref" '
  $2 == ref {direct = $1}
  $2 == ref "^{}" {peeled = $1}
  END {print peeled ? peeled : direct}
' <<< "$remote_refs")
if [[ -n "$tag_sha" && "$tag_sha" != "$release_sha" ]]; then
  echo "Existing tag $RELEASE_TAG points to a different commit." >&2
  exit 1
fi
# GitHub CLI stages the assets in a draft before publishing and creating the tag.
gh release create "$RELEASE_TAG" "${assets[@]}" dist/SHA256SUMS dist/updates.json dist/updates.sig \
  --target "$release_sha" --notes-file dist/release-notes.md --title "IWE Code $version"
