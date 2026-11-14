# External file opening and deep links

IWE Code accepts open requests from outside the application. Opened files appear in the running
application. A file opens inside the project that owns it, or on its own when no project owns it.

## Request sources

| Source        | Behavior                                              |
| ------------- | ----------------------------------------------------- |
| Command line  | `iwe /path/to/file.md` or `iwe "iwe://open?path=..."` |
| macOS events  | Finder open-with commands and `iwe://` links          |
| Second launch | The new process forwards its requests and exits       |
| Drag and drop | Dropped files behave like command-line files          |

Each source produces the same request. Requests that arrive before the workspace loads wait in a
queue.

## Opening rules

- A file inside a project directory opens with that project selected.
- Nested projects win, so the most specific project matches.
- A file that no project owns opens on its own tab without a project.
- A missing file shows a notification.

## Deep links

| Link                                  | Effect                         |
| ------------------------------------- | ------------------------------ |
| `iwe://open?path=/abs/path/file.md`   | Open the file                  |
| `iwe://project?path=/abs/path/folder` | Switch to the existing project |

- The scheme is `iwe`.
- The `path` value must be an absolute file-system path.
- Unknown actions and relative paths are ignored.
- A project link shows a notification when no space holds that project.

## Single instance

The first launch binds a local endpoint in the settings directory. Later launches forward their
requests to that endpoint and exit. A launch without requests forwards a focus request, so a second
launch always raises the running window instead of starting a second one. macOS delivers open events
directly, so LaunchServices handles the same deduplication.

The endpoint is private to your account. The Unix socket and its directory restrict access to the
current user, and connecting requires write permission on the socket file. The Windows named pipe
reserves its name for the first instance, rejects remote clients, and caps payload size. Shared
systems remain an open limitation: the pipe's access control cannot be narrowed to one user without
security-descriptor access outside the supported safe APIs. Payloads are size-capped on every
platform.

## Platform registration

- macOS bundles declare the `iwe://` scheme and text document types in `Info.plist`.
- The Windows installer registers the `iwe://` scheme and the application entry for the current user
  under `Software\Classes`.
- The Linux desktop file declares `x-scheme-handler/iwe`.

## Limitations

- macOS open events need the released bundle. The bare development binary accepts files and links
  through the command line.
- Windows file associations stay manual. Users pick IWE Code through the Open With dialog once, and
  Windows remembers the choice.
- Linux users register the desktop file with `xdg-mime default` or a matching desktop tool.
