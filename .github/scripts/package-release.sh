#!/usr/bin/env bash
set -euo pipefail

: "${BUILD_TARGET:?}" "${ASSET_PLATFORM:?}" "${RELEASE_VERSION:?}" "${RUNNER_OS:?}"
mkdir -p dist
output=$(cd dist && pwd)
stage=$(mktemp -d)
trap 'rm -rf "$stage"' EXIT
name="iwe-code-${RELEASE_VERSION}-${ASSET_PLATFORM}"
update="iwe-${RELEASE_VERSION}-${ASSET_PLATFORM}-update.tar.gz"
binary="target/$BUILD_TARGET/release/iwe"

# Build the Debian package from a staged control tree.
#
# Arguments: the staging directory for the control tree and the output path.
# The executable, desktop entry, and icon must already be staged at the
# conventional paths used by the Linux archive above.
package_linux_deb() {
  local stage="$1" output="$2"
  local arch=amd64
  [ "$ASSET_PLATFORM" = "linux-x86" ] && arch=i386
  local tree="$stage/deb"
  install -Dm755 "$stage/iwe" "$tree/usr/bin/iwe"
  install -Dm644 assets/iwe.desktop "$tree/usr/share/applications/iwe.desktop"
  install -Dm644 assets/icon.png "$tree/usr/share/icons/hicolor/256x256/apps/iwe.png"
  install -Dm644 README.md "$tree/usr/share/doc/iwe-code/README.md"
  install -Dm644 assets/iwe.copyright "$tree/usr/share/doc/iwe-code/copyright"
  install -Dm755 /dev/null "$tree/DEBIAN/postinst"
  cat > "$tree/DEBIAN/postinst" <<'POSTINST'
#!/bin/sh
set -e
if command -v update-desktop-database >/dev/null 2>&1; then
  update-desktop-database -q /usr/share/applications || true
fi
POSTINST
  cat > "$tree/DEBIAN/control" <<CONTROL
Package: iwe-code
Version: ${RELEASE_VERSION%%[-+]*}
Architecture: $arch
Maintainer: IWE Code <iwe-code@users.noreply.github.com>
Section: devel
Priority: optional
Depends: libc6 (>= 2.35), libfontconfig1, libgcc-s1, libgl1, libudev1, libvulkan1, libwayland-client0, libx11-6, libxcb1, libxcursor1, libxi6, libxinerama1, libxkbcommon0, libxkbcommon-x11-0, libxrandr2
Description: Graph workspace for code and notes
 IWE Code is a workspace for projects, editing, Git history, pipelines,
 deployment, and terminals. This package installs the launcher on PATH with
 its desktop entry and icon.
CONTROL
  dpkg-deb --build --root-owner-group "$tree" "$output"
}

# Build the x86_64 AppImage with the pinned linuxdeploy tool.
#
# Arguments: the staging directory for the AppDir and the output directory.
package_linux_appimage() {
  local stage="$1" output="$2"
  local linuxdeploy
  linuxdeploy=$(bash "$(dirname "$0")/setup-appimage.sh")
  local appdir="$stage/AppDir"
  install -Dm755 "$stage/iwe" "$appdir/usr/bin/iwe"
  install -Dm644 assets/iwe.desktop "$appdir/usr/share/applications/iwe.desktop"
  install -Dm644 assets/icon.png "$appdir/usr/share/icons/hicolor/256x256/apps/iwe.png"
  APPIMAGE_EXTRACT_AND_RUN=1 "$linuxdeploy" \
    --appdir "$appdir" \
    --desktop-file "$appdir/usr/share/applications/iwe.desktop" \
    --icon-file "$appdir/usr/share/icons/hicolor/256x256/apps/iwe.png" \
    --output appimage \
    --destdir "$output"
  local image
  image=$(find "$output" -maxdepth 1 -name '*.AppImage' -print -quit)
  mv "$image" "$output/iwe-code-${RELEASE_VERSION}-${ASSET_PLATFORM}.AppImage"
}

case "$RUNNER_OS" in
  macOS)
    bundle="$stage/IWE.app/Contents"
    mkdir -p "$bundle/MacOS" "$bundle/Resources"
    cp assets/icon.icns "$bundle/Resources/icon.icns"
    cp "$binary" "$bundle/MacOS/iwe"
    chmod 755 "$bundle/MacOS/iwe"
    version=${RELEASE_VERSION%%[-+]*}
    cat > "$bundle/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>CFBundleExecutable</key><string>iwe</string>
  <key>CFBundleIdentifier</key><string>com.adametsderschopfer.iwe</string>
  <key>CFBundleName</key><string>IWE Code</string>
  <key>CFBundleDisplayName</key><string>IWE Code</string>
  <key>CFBundleIconFile</key><string>icon.icns</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>$version</string>
  <key>CFBundleVersion</key><string>$version</string>
  <key>CFBundleURLTypes</key><array><dict>
    <key>CFBundleURLName</key><string>com.adametsderschopfer.iwe</string>
    <key>CFBundleURLSchemes</key><array><string>iwe</string></array>
  </dict></array>
  <key>CFBundleDocumentTypes</key><array><dict>
    <key>CFBundleTypeName</key><string>Text and Source Files</string>
    <key>CFBundleTypeRole</key><string>Viewer</string>
    <key>LSHandlerRank</key><string>Alternate</string>
    <key>LSItemContentTypes</key><array>
      <string>public.plain-text</string>
      <string>public.source-code</string>
      <string>public.shell-script</string>
      <string>public.json</string>
      <string>public.xml</string>
    </array>
  </dict></array>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>NSHighResolutionCapable</key><true/>
</dict></plist>
PLIST
    plutil -lint "$bundle/Info.plist"
    if [ "${IWE_MACOS_SIGNING_ENABLED:-}" = "true" ]; then
      : "${IWE_MACOS_SIGNING_IDENTITY:?}"
      codesign --force --options runtime --timestamp \
        --entitlements packaging/macos/entitlements.plist \
        --sign "$IWE_MACOS_SIGNING_IDENTITY" "$stage/IWE.app"
    else
      codesign --force --sign - "$stage/IWE.app"
    fi
    codesign --verify --strict "$stage/IWE.app"
    COPYFILE_DISABLE=1 tar -czf "$output/$update" -C "$stage" IWE.app
    mv "$stage/IWE.app" "$stage/IWE Code.app"
    ln -s /Applications "$stage/Applications"
    hdiutil create -volname "IWE Code" -srcfolder "$stage" -ov -format UDZO "$output/$name.dmg"
    if [ "${IWE_MACOS_SIGNING_ENABLED:-}" = "true" ]; then
      : "${APPLE_API_ISSUER:?}" "${APPLE_API_KEY_ID:?}" "${APPLE_API_KEY_BASE64:?}"
      key_file="$stage/AuthKey_$APPLE_API_KEY_ID.p8"
      printf '%s' "$APPLE_API_KEY_BASE64" | base64 -D > "$key_file"
      chmod 600 "$key_file"
      xcrun notarytool submit "$output/$name.dmg" \
        --key "$key_file" --key-id "$APPLE_API_KEY_ID" --issuer "$APPLE_API_ISSUER" --wait
      xcrun stapler staple "$output/$name.dmg"
    fi
    ;;
  Windows)
    pwsh -NoProfile -File .github/scripts/package-windows.ps1
    ;;
  Linux)
    cp "$binary" "$stage/iwe"
    chmod 755 "$stage/iwe"
    cp README.md "$stage/README.md"
    cp assets/iwe.desktop "$stage/iwe.desktop"
    cp assets/icon.png "$stage/iwe.png"
    tar -czf "$output/$update" -C "$stage" iwe
    COPYFILE_DISABLE=1 tar -czf "$output/$name.tar.gz" -C "$stage" iwe README.md iwe.desktop iwe.png
    package_linux_deb "$stage" "$output/$name.deb"
    if [ "$ASSET_PLATFORM" = "linux-x64" ]; then
      package_linux_appimage "$stage" "$output"
    fi
    ;;
  *) echo "Unsupported release platform: $RUNNER_OS" >&2; exit 1 ;;
esac
