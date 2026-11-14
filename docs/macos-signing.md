# macOS signing and notarization

The release workflow builds the application bundle and the disk image in the macOS build jobs. By
default the bundle carries an ad-hoc signature. When an owner configures a Developer ID certificate
and an App Store Connect API key, the same jobs sign the bundle with Developer ID signing and the
hardened runtime, submit the disk image to Apple notarization, and staple the ticket.

## Ad-hoc installation

Leave `IWE_MACOS_SIGNING_ENABLED` unset or set it to anything other than `true`. The workflow builds
ad-hoc signed bundles and disk images. Update signatures verify package integrity. They do not
establish a trusted publisher. Gatekeeper warns that the application comes from an unidentified
developer and blocks opening until the user confirms it in System Settings.

## Prepare Developer ID signing and notarization

This integration remains inactive until an owner configures it. It requires a paid Apple Developer
Program membership and an Apple ID that belongs to that team.

1. Generate a Developer ID Application certificate for the team and install it in the keychain of
   the macOS build runner image, or provision it through the repository's secret manager.
2. Create an App Store Connect API key with the App Manager role or higher in App Store Connect.
3. Download the private key file (`.p8`), base64-encode it, and keep the original outside the
   repository.
4. Configure the repository values below. Enable signing only after configuring every required
   value.

| Repository value             | Kind     | Value                                         |
| ---------------------------- | -------- | --------------------------------------------- |
| `IWE_MACOS_SIGNING_ENABLED`  | variable | `true` to require signing and notarization    |
| `IWE_MACOS_SIGNING_IDENTITY` | secret   | Developer ID Application certificate identity |
| `APPLE_API_ISSUER`           | secret   | App Store Connect API key issuer identifier   |
| `APPLE_API_KEY_ID`           | secret   | App Store Connect API key identifier          |
| `APPLE_API_KEY_BASE64`       | secret   | Base64-encoded `.p8` private key              |

The build jobs sign the bundle with `--options runtime`, a trusted timestamp, and the entitlements
file at `packaging/macos/entitlements.plist`. The dictionary is intentionally empty: the application
renders through Metal and needs no runtime exemptions. Failed signing prevents packaging. Failed
notarization or stapling fails the build job and blocks publication. Neither step falls back to
unsigned output.

Notarization submits the disk image. The staple attaches to the disk image, and the signed bundle
inside carries the Developer ID signature with a timestamp. First launch checks therefore pass
without an unidentified-developer warning on macOS 12 and newer.

## Sources

- [Notarizing macOS software](https://developer.apple.com/documentation/xcode/notarizing-macos-software-before-distribution)
- [Developer ID signing and notarization requirements](https://developer.apple.com/support/developer-id/)
- [App Store Connect API keys](https://developer.apple.com/documentation/appstoreconnectapi/creating-api-keys-for-app-store-connect-api)

## Local verification limits

Source checks can validate the packaging script and the entitlements file with `plutil`. Live
signing requires a Developer ID certificate on a macOS host. Live notarization requires the
configured App Store Connect API key and network access to Apple services.
