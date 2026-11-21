# Modules

IWE Code provides Git, Pipelines, Terminal, Docker, Kubernetes, and Deployment under **Settings →
Modules**. Each module is enabled by default. Each row provides an activation switch and a
**System** badge. Changes apply immediately and persist across restarts. Modules remain installed
when disabled. Saved connections, profiles, credentials, shortcuts, and panel preferences remain
unchanged. Unknown module identifiers survive settings updates. Older settings without activation
preferences enable every module.

Disabling hides module controls and panels. It also stops subscriptions and rejects new module
actions. Terminal sessions end. Deployment transfers receive cancellation, and queued transfers are
cleared. Git and provider operations already submitted can finish externally. Their obsolete results
cannot replace current module state. Re-enabling never repeats a write operation automatically.
Existing source and comparison tabs remain available.

Module panels start collapsed when no visibility preference exists. Use the bottom toolbar to toggle
Terminal, Docker, Kubernetes, Git, or Pipelines. Only one bottom panel can open at a time.
Deployment uses the right server icon. The minus button in each panel header collapses that panel.
Terminal and Deployment start collapsed after each application restart.

## Git and Pipelines

Git owns history panels, branch selection, branch operations, and their presentation state.
Pipelines owns run lists, job details, status popups, caches, and provider adapters.

Pipelines works with Git disabled. Shared repository discovery reads provider endpoints without
loading history or branches. Disabling Git stops status polling and new working-file comparisons.
Normal local file viewing remains available. Disabling Pipelines stops run and job polling,
including pipeline status updates in Git history.

The application has no module store. Users install external modules from local package files.

## Docker

Open **Docker** on the vertical toolbar. Choose a connection above the resource list. IWE reads
existing Docker CLI contexts. The initial selection follows the current Docker context. Choose **Add
SSH connection** to save another named context. Docker uses your existing SSH configuration.
Changing the panel selection only changes the connection used by IWE. The panel reports when the
local Docker socket or named pipe stops accepting connections. It lists resources automatically when
Docker becomes available again.

Git and Docker share a 42-pixel toolbar with 16-pixel icons. Use the icons on the left to select a
resource type. Clicking the active resource or tab preserves its current view. Hover over an icon to
see its name. The adjacent list shows matching resources and their count. Select a row to open
**Properties** with the available actions. Thin dividers separate the sections. Each section scrolls
independently. In a narrow panel, properties occupy the list area. In wider panels, drag the divider
to resize the properties. Container properties initially use 30% of the space beside the navigation.
Click the image name in container properties to reveal the exact image in the inventory. In image
properties, open **Containers** to see running containers created from that image. Click a container
in the tab to reveal it in the inventory. Use **Back to list** to return to the filtered results.

In **Compose**, right-click a project and choose **Relationships**, or open **Relationships** from
its **Properties**. The diagram shows the project's containers and their images, networks, and
volumes. Shared resources appear once. Drag the background to pan, drag a node to move it, or use
the wheel to zoom. Select a node and choose **Open resource** to reveal its entry. The diagram
refreshes every minute and keeps node positions. **Reset view** restores its initial layout. Press
**Escape** or click the close button to return to the same list and selection.

| Section    | Operations                                                                                                 |
| ---------- | ---------------------------------------------------------------------------------------------------------- |
| Containers | Create from an image, inspect, start, pause, resume, restart, stop, read logs, open a terminal, or remove. |
| Images     | Pull an image, build from a Dockerfile, inspect, add a tag, or remove a tag.                               |
| Networks   | Create a network, inspect it, connect or disconnect a container, or remove a user network.                 |
| Volumes    | Create a volume, inspect it, or remove it. Docker protects volumes that containers still use.              |
| Compose    | Deploy a local Compose file, inspect services, read logs, start, stop, restart, or remove a project.       |

Click a resource to open its properties. Right-click a row to act on it while keeping the current
properties. The menu follows the pointer even in a scrolled list. Removing a running or paused
container requires confirmation and stops it immediately. Use the filter in the resource heading to
search by name, identifier, driver, image, or status. Lists display forty results per page. Refresh
updates the selected section. The visible panel also checks idle lists every twelve seconds. Choose
**Logs** or **Inspect** from a resource's context menu or properties to open a separate dialog. The
title identifies the resource. Log requests retrieve the latest 500 lines per container. Scroll
through the output or use the page arrows for long results. **Refresh** reads the same resource
again. Press **Escape** or click the close button to return to the previous selection.

Terminal sessions run the selected executable inside the container. Enter its path if the image
lacks the default shell. Closing the terminal tab ends the session. Removing a Compose project
preserves named volumes.

IWE asks for confirmation before stopping, pausing, restarting, disconnecting, or removing
resources. The panel shows the operation and elapsed time while the editor remains available. Read
requests time out after thirty seconds. Changes time out after ten minutes. Refresh after a timeout
to see what Docker completed. Disabling the module discards pending interface results. Docker can
finish an operation already submitted to the daemon.

Docker forms display labeled fields directly below the title. The container form starts with its
image and container name. **Runtime options** contains port mappings, mounts, environment variables,
and an optional command. Scroll through the fields in smaller windows. The form keeps the submit and
cancel buttons visible. Separate multiple values with spaces. Quote a value when it contains spaces
or backslashes. Docker receives each value as a separate argument.

Install Docker CLI and enable access to the selected daemon. Compose operations require the Docker
Compose plugin. Existing contexts support the connection methods configured in Docker, including
TLS. IWE never changes Docker's active context. A remote build sends its build context to the
selected daemon. The module uses the selected Dockerfile's directory as its build context.

### Registries

Open **Registries** on the Docker rail. Docker Hub lists official repositories. Enter the exact
repository name to browse other publishers or registries that restrict catalog access. Use the page
controls to browse repositories and tags. Select a version to inspect its digest and platforms.

Add a registry URL and test the connection. Private connections can use Docker's configured
credential helper. Specify the trusted token issuer when your registry uses a separate login
service. Anonymous local registries can use HTTP on loopback. Browsing works with Docker Engine
stopped. Pull requires an active Docker connection.

Select a local image and choose **Publish**. Review the connection and full destination before
confirming. The registry browser refreshes the tags and offers the published address for copying.

## Kubernetes

Enable **Kubernetes** in **Settings → Modules** and open its toolbar button. Install kubectl on the
application's command path. Choose a context from the default kubeconfig or select a file. The
default uses KUBECONFIG when set. IWE passes the context to each command. Select a namespace, enter
its exact name, or choose **All namespaces**. Node resources belong to the cluster.

Browse workloads, Pods, Services, Ingresses, ConfigMaps, Secrets, and Nodes. Select a row to see
properties, events, and YAML. IWE masks Secret values in resource views and manifest comparisons.

Use **Related Pods** on a Deployment to follow its ReplicaSet ownership. On a Pod, select the
container before opening logs, a terminal, or its registry image. Pod and Service port forwarding
binds to localhost. Stop each forward in the panel. Closing a terminal or log session ends its
process. Switching context or disabling Kubernetes ends its sessions.

Review the target before scaling, restarting, or replacing a Deployment image. Enter an image
address manually or choose a tag or digest through Docker. Kubernetes uses the cluster's image pull
credentials. Inspect Pod status and events if a rollout cannot pull its image.

Use **Compare local YAML** for a server-side preview. Review each resource and confirm to apply the
reviewed file contents. Multi-resource changes run in order. Review the file again after a partial
failure.

## External modules

External modules add capabilities without recompiling the application. A module is a separate
operating-system process. The host exchanges one JSON document per line with the process over stdin
and stdout. The protocol version is `api: 1`. See [ADR 0002](adr/0002-external-modules.md) for the
decision record and the module development guide on the website for the protocol and package
reference.

A package is a gzipped tar archive with the `.iwe-module` extension. It contains `manifest.json`, a
`manifest.sig` signature, one executable per platform under `binaries/<target>/`, and optional
`assets`. The manifest declares the identifier, localized name and description, semantic version,
protocol version, minimum application version, entry names, permissions, panel placement, and a
SHA-256 digest per packaged file. The signature covers the manifest bytes and, through the digests,
every packaged file. Package files are limited to 128 MiB. Updates must increase the module version.

IWE Code accepts only packages signed with its embedded module signing key. Unsigned or tampered
packages fail installation with an explanation before anything is unpacked or run. Distribution
packages are signed with `.github/scripts/sign-module.py` and the maintainer's private key.
Development builds can skip signature enforcement with the `IWE_ALLOW_UNSIGNED_MODULES` environment
variable; the application tests use their own fixture keys.

**Settings → Modules → Install from file…** reads the package, verifies the signature, shows the
requested access, and asks for confirmation. Installation stores files under
`modules/<id>/<version>` in the application data directory and records the module in `modules.json`
next to `workspace.json`. The `IWE_DATA_DIR` environment variable redirects both locations, as
everywhere else.

Permissions are declarative: `filesystem-read`, `filesystem-write`, `network`, and `process`. The
application shows them before installation and requires confirmation. Module processes run with
normal user rights; the application does not sandbox them.

The manifest placement selects the workspace zone for the module panel: header, top, left, right, or
bottom. Several modules in one zone switch through tabs. Modules describe panels as a declarative
widget tree of native components. The application renders these trees and never runs module code in
the interface process. Modules localize their own panel text; the host passes the interface language
during initialization.

A module process runs while its module stays enabled. It starts on application start or activation
and stops on deactivation, removal, or shutdown. A crashed module restarts at most three times per
session. After that the application disables the module and reports it. Removal stops the process,
deletes the module directory, and drops the registry record. System modules stay bundled and cannot
be removed.

The reference implementation lives in `examples/hello-module`. Integration tests exercise the
protocol against `tests/bin/iwe-test-module.rs`. Package reading, installation, updates, removal,
and the registry have unit tests under `src/infrastructure/modules/`. Host state and zone
coordination live under `src/app/modules/external/`.

## Deployment

The Deployment system module provides shared SFTP, FTP, and FTPS connections. Profiles belong to
spaces and target individual projects. Its resizable right panel browses remote files independently
of the terminal and Git panels. See [deployment](deployment.md) for mappings, credentials, transfer
behavior, keyboard shortcuts, and verification limits.

## Terminal

Select a project, then choose **Terminal** in the bottom toolbar. The first opening creates a
terminal tab in the project's directory. Later openings restore existing tabs. Each tab owns an
independent interactive shell. Commands inherit your account's permissions and environment. Commands
can modify files, launch programs, and access the network.

Click the terminal area before typing. The terminal supports Unicode text, colors, cursor movement,
command history, Tab completion, and common function keys. Ctrl+C interrupts the current command.
The shell determines which commands and programs are available.

| Control                             | Action                                                 |
| ----------------------------------- | ------------------------------------------------------ |
| Plus beside the tabs                | Create and select another terminal tab                 |
| Tab label                           | Select that tab without changing its session or output |
| Cross on a tab                      | Close that tab and end its session                     |
| Minus opposite the Terminal heading | Hide the panel while keeping all tabs and sessions     |
| Mouse wheel                         | Read earlier terminal output                           |
| Mouse drag                          | Select visible terminal text                           |

Closing the last tab hides the panel. Opening the panel again creates a new tab when no tabs remain.
Tabs scroll horizontally when they exceed the available width. New tabs scroll into view. An ended
shell retains its tab and output until you close the tab. The session releases its pseudo-terminal
after shell exit while a separate reader drains the remaining output. This also completes Windows
ConPTY sessions without waiting for the tab to close.

Each tab groups its label and close control within one shared surface. The plus follows the last tab
inside the same horizontal scrolling area. Each project numbers its local tabs **Terminal 1**,
**Terminal 2**, and so on. Docker tabs show the container name. Kubernetes tabs show the Pod and
container names. IWE adds a number to repeated container titles, such as `api (2)`.

Right-click a tab and choose **Rename**. Press **Enter** to save or **Escape** to cancel. Clear the
name and press **Enter** to restore the automatic name.

On macOS, Command+C copies selected text and Command+V pastes text. With no selection, copying uses
the visible screen. Windows and Linux use Ctrl+Shift+C and Ctrl+Shift+V. Ctrl+C remains available to
the shell on every platform. Clicking the terminal clears source-text and branch selections.

Dragging the horizontal divider resizes the terminal. The terminal also follows window resizing. The
terminal uses the saved source font size and current appearance. Output does not change when the
appearance changes.

The terminal reuses prepared drawing geometry while its visible screen remains unchanged. Output,
resizing, and scrollback changes update a screen revision. Font size, appearance, selection, focus,
and tab changes invalidate drawing geometry. The terminal uses the source viewer's platform font.
This avoids rebuilding terminal cells when unrelated panels redraw.

Selecting a different project leaves the previous project's shells running. Each shell continues
collecting output within its history limit. Return to a project to see its tabs, selection, and
accumulated output. Project reordering and refresh preserve these tabs. Removing the project from
the workspace ends its sessions. Changing the Kubernetes context ends that project's Kubernetes
sessions. Quitting IWE ends all sessions. Programs that deliberately detach from the shell may
continue independently.

## Resource and platform limits

Each tab retains up to 2,000 scrollback lines. Screen dimensions stay within 200 rows and 400
columns. Input requests have a 64 KiB limit. Paste requests reserve 12 bytes for bracketed-paste
markers. Input and output queues each hold up to 64 requests. Output requests contain at most 4 KiB.
The application consumes at most 32 output requests per message.

Large paste requests are rejected. The terminal removes NUL and escape characters from pasted text.
Shells that enable bracketed paste receive the corresponding boundary markers. The terminal ignores
program requests to read or replace the system clipboard. It does not interpret output as
application commands or open links automatically.

macOS and Linux use a pseudo-terminal and the operating system's default shell. Windows uses ConPTY
and the system's default command processor. Windows requires a ConPTY-capable Windows version. No
shell is downloaded or installed. Unix builds use `portable-pty`; Windows builds use pinned `xpty`
0.3.6. Its Windows bindings use the system calling convention on x86 and x64. Its default flags
avoid the cursor-inheritance handshake, which would stall a terminal without a cursor-position
reply. The application uses safe Rust interfaces for both adapters.

The terminal has no search, clickable links,
program-controlled mouse reporting, or input-method composition interface. Tests verify process
input, output, resizing, cleanup, preferences, translated settings, and session guards. Native
checks on one platform do not verify the other platforms.

## Source organization

Each bundled module has its own directory under `src/modules/`. The terminal keeps its
implementation and unit tests under `src/modules/terminal/`:

```text
modules/
├── mod.rs
├── catalog/
└── terminal/
    ├── mod.rs
    ├── style.rs
    ├── locales/
    │   ├── en.ftl
    │   ├── ru.ftl
    │   └── es.ftl
    ├── state/
    │   ├── mod.rs
    │   └── tests.rs
    ├── session/
    │   ├── mod.rs
    │   └── tests.rs
    └── view/
        ├── mod.rs
        ├── input.rs
        └── tests.rs
```

Each module exposes its state, events, and views through its `mod.rs` boundary. Git and pipeline
host coordination lives under `app/modules/git/` and `app/modules/pipelines/`. Their views receive
explicit read-only projections instead of application state. The terminal also exposes tasks and
subscriptions through `terminal/mod.rs`. The module does not import host application state. The
session adapter does not import presentation code. Shared application code connects module events to
workspace actions. Host integration tests remain under `src/app/modules/`. Shared settings remain
under `src/ui/modules.rs`.

Module translations live in each module's own `locales/` directory. Shared application translations
remain under root `locales/`. The shared localizer embeds and combines both resource sets. Duplicate
message identifiers reject the combined catalog. Typed built-in message keys remain registered in
`src/i18n/mod.rs`. Catalog tests validate each source and the combined messages.

Git loading and operation status appear in the shared bottom status bar, even when its panel is
collapsed. The Git header, history list, and changed-file list do not repeat progress messages.
Errors and retry controls remain beside the affected content.
