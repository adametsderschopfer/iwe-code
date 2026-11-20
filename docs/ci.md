# Continuous integration

GitHub Actions uses three workflows with English names.

- **Code Checks** runs application and website checks on main pushes. Application checks include
  formatting, lint, type checks, dependency security, script checks, and tests. Website checks
  include formatting, lint, type checks, dependency security, generation, and browser tests.
- **Release New Version** manually prepares a version, builds packages, and publishes the release
  with its tag. Its final jobs build and publish the website from the same prepared commit.

- **Build and Publish Website** manually generates the website from main, uploads an artifact, and
  publishes GitHub Pages.

Code Checks uses the release runners and architecture targets listed below for Clippy, Cargo type
checks, unit tests, and integration tests. It builds and packages all six targets after those checks
pass. Packaging uses the release scripts and tests both Windows installers. Code Checks uses no
release signing credentials and publishes no packages. Shared checks and website jobs run on Linux.

Integration tests cover the CLI, the local forwarding endpoint, external modules, and the MCP
server. CI reports explicitly if the workspace has no integration test targets.

## Local checks before committing

On Apple Silicon, install MinGW-w64, Docker CLI, and a Linux container runtime. The default setup
uses a dedicated Colima profile. It uses Rosetta for the x64 Linux container.

```sh
brew install mingw-w64 docker colima
rustup target add aarch64-apple-darwin x86_64-apple-darwin i686-pc-windows-gnu x86_64-pc-windows-gnu
colima start --profile iwe --vm-type vz --vz-rosetta --cpu 6 --memory 12 --disk 48 --activate=false
make platform-check
git config --local core.hooksPath .githooks
git config --local iwe.localPlatformChecks true
```

`make platform-check` checks formatting and runs Clippy for the six local targets. Clippy compiles
the application and test code for each target. MinGW-w64 selects Windows code for x64 and x86.
GitHub Actions uses MSVC to verify the released Windows binaries and installers. The local Linux
container checks x64 and x86 code. GitHub Actions runs the Linux test binaries and packages them.

The pre-commit hook checks a copy of the staged files. Unstaged edits stay outside these checks. The
hook starts the configured Colima profile when necessary. Set `IWE_DOCKER_CONTEXT` to use another
Docker context that runs x64 Linux containers.

## Local push checks

Install Python 3.11 or newer and actionlint for the preflight command. On Windows, run these
commands in Git Bash and set `PYTHON=python` if Python uses that command name.

```sh
make preflight
git config --local core.hooksPath .githooks
```

The hook runs `make preflight` before sending a commit to main. Check out the outgoing commit and
commit or stash local changes first. Preflight runs `make check`, release-script tests, and
actionlint before `make build`. Run `make fmt` to apply the root formatters. Cargo audit checks the
complete lockfile and requires network access to refresh RustSec advisories.

Local checks use the current operating system. Use GitHub-hosted runners or equivalent local
environments for the other platforms. The GitHub checks also cover pushes made without the local
hook.

GitHub Actions caches speed up repeated runs. Rust jobs restore the crate registry and compiled
dependencies. They disable incremental compilation because every run starts from cached artifacts.
The security job caches the cargo-audit binary. Website jobs cache npm packages and Playwright
browsers. Checks and release builds share the platform link settings. Package verification and
release builds share cached dependencies for each target.

## Release packages

| Platform | Architecture         | Rust target              | Runner         | Package                     |
| -------- | -------------------- | ------------------------ | -------------- | --------------------------- |
| macOS    | Apple Silicon, ARM64 | aarch64-apple-darwin     | macos-15       | DMG containing IWE Code.app |
| macOS    | Intel, 64-bit        | x86_64-apple-darwin      | macos-15-intel | DMG containing IWE Code.app |
| Windows  | x64, 64-bit          | x86_64-pc-windows-msvc   | windows-2022   | Setup EXE and portable ZIP  |
| Windows  | x86, 32-bit          | i686-pc-windows-msvc     | windows-2022   | Setup EXE and portable ZIP  |
| Linux    | x64, 64-bit          | x86_64-unknown-linux-gnu | ubuntu-22.04   | tar.gz, deb, and AppImage   |
| Linux    | x86, 32-bit          | i686-unknown-linux-gnu   | ubuntu-22.04   | tar.gz and deb              |

Windows x86 builds use the MSVC target on a 64-bit runner. Windows release binaries link the C
runtime statically. Linux x86 builds install multilib compilers and i386 development libraries.
Tests execute the selected architecture's binary, including both 32-bit targets. Linked-worktree
fixtures pass relative targets to Git. This avoids Windows verbatim-path prefixes during worktree
creation.

macOS packages target macOS 11 or newer. Each DMG includes an Applications shortcut. Linux packages
target Ubuntu 22.04 or compatible systems with glibc 2.35 or newer. Linux users need a graphical
session and a supported graphics driver. 32-bit Linux binaries require matching 32-bit runtime
libraries and graphics drivers. Windows packages require Windows 10 or newer and a supported
graphics driver. Git remains an external dependency on every platform.

Every package uses the IWE Code display name and generated application icon. macOS bundles include
an ICNS resource. Windows executables embed an ICO resource during compilation. Linux archives
include `iwe.desktop` and `iwe.png`. Install the executable on PATH, the desktop entry under
`~/.local/share/applications`, and the icon under `~/.local/share/icons`. The deb package installs
those locations system-wide as `/usr/bin/iwe` and refreshes the desktop database. The AppImage
packages the launcher as a single executable file and ships for x86_64 only; the 32-bit builds use
the tar.gz and deb packages.

macOS bundles use ad-hoc signatures by default. Optional Developer ID signing with the hardened
runtime and Apple notarization of the disk image activate when an owner configures them. See
[macOS signing setup](macos-signing.md). Windows installers use Inno Setup 7.1.1 with modern light
and dark pages. They support English, Russian, and Spanish. Installation uses the current user's
Programs directory without administrator access. The installer creates a Start menu shortcut and
offers a desktop shortcut. Uninstalling preserves workspace data. CI checks silent installation,
registration, and removal on both Windows architectures.

Windows packaging uses PowerShell and the system's `tar.exe` for update archives.

Windows packages remain unsigned by default. Optional Microsoft Artifact Signing covers the
application, installer, and uninstaller. See [Windows signing setup](windows-signing.md). Operating
systems can require confirmation when opening downloaded packages.

## Publish a version

1. Collect reviewed English, Russian, and Spanish items in the unreleased release-note record.
2. Open **Release New Version** in GitHub Actions and select main.
3. Choose **Fixes — patch**, **New features — minor**, or **Major update — major**.
4. Run the workflow. It updates Cargo.toml, Cargo.lock, and the release-note ledger.
5. The workflow pushes the version commit and builds its packages.
6. GitHub creates the tag when it publishes the release with all packages.
7. Verify the GitHub release and the published website.

For version 1.0.7, patch produces 1.0.8, minor produces 1.1.0, and major produces 2.0.0. Empty
unreleased notes stop preparation. Existing version records also stop preparation. Version workflows
serialize their work. A concurrent main push causes a normal push conflict. Branch protections must
permit the workflow bot to push version commits. The workflow never force-pushes. Configure
repository permissions before using this workflow.

Code Checks runs only for main pushes. The version workflow uses needs dependencies for builds,
publication, and website deployment. No dispatch requests or additional personal access tokens are
required. Only version preparation and release publication receive contents write permission.
Website deployment receives pages and id-token write permissions. The workflow checks out the
prepared commit explicitly because the initiating event references main. The built-in token pushes
the version commit without triggering another Code Checks run.

Website jobs explicitly require successful publication even when the release skips Windows
packaging.

The first version workflow job waits for the main push checks for the dispatched commit. These
checks include packaging on all six targets. Preparation stops if checks fail, cancel, or time out.
The release workflow polls for up to 90 minutes. New version preparation also stops if main advances
before checkout. Build or packaging failure stops publication.

Retry failed release jobs after transient errors. A full rerun reuses its original version commit
and release tag. The commit message records the run identifier and release tag. If code needs
correction, push the correction and run a new version request with reviewed notes. Existing tags
must point to the prepared commit. The workflow never moves an existing tag.

Start releases through Release New Version on main. Choose patch, minor, or major. Tag pushes do not
start separate workflows. Retry failed jobs within the original run after transient build or
deployment errors. Website failures leave the published application release available. The website
download page lists the latest release with complete downloads for each platform. Release builds
validate the tag, release notes, and package version. They build the prepared commit. Installer
installation checks run when Windows packaging is enabled. All enabled builds and installers must
succeed before publication.

The publication job signs one update manifest. It verifies the installation packages and update
archives for the enabled targets, then adds SHA256SUMS. Releases include all targets by default and
use `vMAJOR.MINOR.PATCH`. Set `IWE_MACOS_ONLY` to `true` to publish a macOS-only release with
`macos-vMAJOR.MINOR.PATCH`. This variable does not change Code Checks. GitHub CLI uploads the files
to a draft. GitHub creates the tag on the prepared commit when it publishes that release. Configure
`IWE_UPDATE_SIGNING_KEY` before publishing. See [application updates](updates.md#release-signing)
for key handling. The version workflow and publication job can write to the repository. Build jobs
have read-only access. Existing releases are not overwritten. If uploading fails, inspect the
remaining draft before retrying publication. Main pushes cancel superseded branch checks. Manual
version runs serialize their work.

Set repository variables IWE_GITHUB_CLIENT_ID and IWE_GITLAB_CLIENT_ID to embed public OAuth Client
IDs in release builds. These values are public identifiers, not secrets. Missing values leave
browser sign-in unconfigured. See [authentication setup](authentication.md) for provider
registration.

Linux builds and website browser tests share an APT setup script. If setup fails, the script
replaces the configured Azure Ubuntu mirror with the official Ubuntu archive and retries. It
preserves the Ubuntu release, security repositories, and signature checks.

## Naming compatibility

Public downloads use the `iwe-code-` prefix. Executables retain `iwe` or `iwe.exe`. The macOS bundle
identifier and existing workspace paths remain unchanged. Update archives retain the `iwe-` prefix
and their previous payload roots, including `IWE.app`. Existing updater versions require those
names. The GitHub repository redirect preserves their previous update URLs.
