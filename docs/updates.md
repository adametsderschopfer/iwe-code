# Application updates

IWE Code checks GitHub Releases at startup and once per hour while running. Returning to the window
also checks when at least 15 minutes have passed since the last attempt. Failed checks become
eligible for another automatic attempt after 15 minutes. A lightweight local timer evaluates
eligibility once per minute. It does not send a network request every minute. Wall-clock timestamps
account for time spent suspended. Clock changes can trigger one early attempt before the cooldown
starts again.

Checks never use repository accounts or tokens. The updater selects stable releases for the current
operating system, with shared releases as a compatibility fallback. Only newer versions qualify.
Automatic checks continue while a version is available to download. Checks pause during download and
while a downloaded package is ready or installing. Open **Settings → About** to check manually
without the automatic cooldown. Duplicate checks cannot overlap.

When IWE Code finds an update, it shows a dialog with the current version, new version, and release
notes. The dialog shows the English notes published on GitHub. Choose **Later** to dismiss the
dialog for that version until the next launch. Select the update icon to the right of **Settings**
to reopen it.

Choose **Download update** to download the package for the current operating system and processor.
Select **Install and restart after download** to start installation when the download finishes.
Choosing **Later** during download cancels automatic installation and lets the download finish.
Choose **Install and restart** when the package is ready. IWE Code saves workspace settings before
starting installation. A failed save keeps the window open and stops installation. Installation
never requests administrator access. The installation directory must be writable by the current
user.

The first installation of version 0.1.10 still requires a downloaded release package. That version
can receive future updates. Older versions do not acquire an updater automatically. Development
binaries cannot install updates. macOS development runs outside an application bundle also cannot
install updates.

After a successful in-app update, IWE Code shows the installed version and its release notes. The
window uses your application language. Choose **Continue** or press Escape to dismiss it. The
package includes these notes, so the window also works offline.

## Package verification

The fixed update source is
[the IWE Code release repository](https://github.com/adametsderschopfer/iwe-code/releases). The
updater accepts HTTPS redirects only to GitHub API, release, and asset hosts. It never forwards
provider credentials. Metadata reads have size limits and a 45-second combined deadline. Downloads
have a five-minute deadline and a 512 MiB maximum.

Each new release includes update archives for its enabled targets, `updates.json`, and its Ed25519
signature `updates.sig`. Historical platform releases contain two update archives. The embedded
public key verifies the manifest before version or package data becomes trusted. The signed manifest
identifies each package's version, platform, byte size, and SHA-256 digest. The updater verifies
package bytes before extraction and repeats verification immediately before installation. Archives
may contain only regular files and directories beneath the expected application root. Extraction
rejects links, path traversal, duplicate paths, excess entries, and oversized expanded content.

## Installation and recovery

Staging directories live beside the installed application, keeping replacements on one filesystem. A
copied installer confirms that it accepted the handoff before the application exits. Failed helper
startup keeps the application open and reports an installation error. The installer normalizes
Windows paths before validation. It waits for the application's file lock after shutdown. After the
helper confirms readiness, the installation worker stops all terminal sessions and exits the
application. This releases the operating-system lock without waiting for window events, graphics
teardown, or blocked background workers. This exit path runs only after the latest workspace save
succeeds. A failed helper handoff keeps the application open. Replacement retries temporary file
locks for ten seconds while the graphics launcher exits. The installer replaces the complete macOS
application bundle, including its signature and icon. Windows and Linux updates replace the
standalone executable. Workspace data and credentials remain separate.

The installer moves the current application into its staging directory before moving the new
application into place. A failed replacement restores the previous application. A failed process
launch also restores it and relaunches the previous application. Successful process creation does
not prove that the new application will remain running. The restarted application removes its
staging directory. A power failure can leave a staging directory with the previous application. No
background cleanup deletes unrelated application directories.

## Release signing

Release tags use `vMAJOR.MINOR.PATCH`. Their numeric version must match Cargo.toml. The updater
reads at most three pages of 100 GitHub releases and selects the highest eligible version. For equal
versions, it prefers a historical platform release over a shared release. Drafts, prereleases,
unrelated tags, and other operating systems do not qualify. List responses have a 4 MiB limit per
page and share the 45-second metadata deadline.

The publication job signs the shared manifest after the enabled targets pass their CI prerequisites.
Only the publication step receives the repository secret `IWE_UPDATE_SIGNING_KEY`. That secret
contains an Ed25519 private key in PEM format. Keep its backup outside the repository.
`assets/update-public-key.bin` contains the matching 32-byte public key. The signing script rejects
a private key that does not match the embedded public key. Missing or invalid signing configuration
prevents publication. Unsigned updates are never accepted.

The signing script requires Python 3.11 or newer and OpenSSL with Ed25519 support. Run
`.github/scripts/sign-updates.py` with `RELEASE_TAG` and `IWE_UPDATE_SIGNING_KEY` after generating
all update archives for the release. Never print the private key or place it in command arguments.

## Verification

Local tests verify version selection, signatures, digest mismatches, redirect restrictions, metadata
bounds, extraction limits, and replacement recovery. Interface tests cover translated update
actions, duplicate requests, pending workspace saves, and failed handoffs. A subprocess test on
macOS and Linux verifies signed extraction, process exit, replacement, and restarting the new
executable. That test deliberately blocks a background worker to detect shutdown hangs. Its signing
key belongs only to the fixture. Automated tests never replace the installed application or contact
GitHub. Platform CI compiles and tests the enabled targets. A live update between signed releases
still requires installed application testing.
