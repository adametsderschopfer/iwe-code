# MCP server

IWE Code ships a Model Context Protocol (MCP) server inside the main binary. Start it with the
`--mcp` flag:

```sh
iwe --mcp
```

The server reads the same workspace metadata as the application. AI assistants connect to it and
browse the configured spaces and projects, read and edit project files, search file names, read Git
status and history, and read files on deployment servers. The server never starts the interface. The
flag works before every other startup step, including the command line and update handling.

## Transport

The server speaks JSON-RPC 2.0 over standard input and output, one message per line. This matches
the MCP stdio transport. Each line holds one UTF-8 JSON document. Incoming lines may not exceed 1
MiB. The server stops when the input stream closes. Standard output carries protocol messages only.

The server supports the MCP protocol versions `2024-11-05`, `2025-03-26`, and `2025-06-18`. It
echoes a supported client version during `initialize` and answers with `2025-06-18` otherwise. It
exposes the tools capability only. Unknown notifications are ignored. Unknown requests receive the
JSON-RPC method-not-found error.

## Tools

The server exposes eleven tools. Only `write_file` changes an existing local project file.

| Tool                      | Input                                     | Behavior                                                                                                   |
| ------------------------- | ----------------------------------------- | ---------------------------------------------------------------------------------------------------------- |
| `list_projects`           | —                                         | Lists spaces and projects with the selected marks and pinned flags.                                        |
| `list_files`              | `project?`, `directory?`                  | Lists one directory level. Skips `.git` and symlinks. Rejects levels above 5,000 entries.                  |
| `read_file`               | `project?`, `path`                        | Reads a UTF-8 text file. Rejects files above 512 KiB or 100,000 lines. Returns a revision token.           |
| `write_file`              | `project?`, `path`, `content`, `revision` | Replaces the content of an existing text file. Requires the revision token.                                |
| `search_files`            | `query`, `project?`, `limit?`             | Ranks file names and paths with the same fuzzy matcher as the workspace search. Returns up to 50 results.  |
| `git_status`              | `project?`                                | Shows the current branch and the changed files of every repository in the project.                         |
| `git_log`                 | `project?`, `limit?`                      | Shows recent commits with authors, dates, and refs. The default limit is 50; the maximum is 250.           |
| `list_deployments`        | —                                         | Lists deployment connections, SSH keys, and profiles with their mappings. Carries no secrets.              |
| `resolve_deployment_path` | `profile`, `local` or `remote`            | Translates one path across a profile mapping. Local paths are project-relative; remote paths are absolute. |
| `deployment_read_remote`  | `profile`, `path`                         | Reads one remote UTF-8 text file through the profile connection. Rejects files above 512 KiB.              |
| `deployment_compare`      | `profile`, `path`                         | Reads the remote counterpart of one project file and returns both texts with an identical flag.            |

The `project` argument selects a project by name. Without it, the tools use the selected project.
Unknown names fail with the list of available names.

The deployment tools split into two groups. `list_deployments` and `resolve_deployment_path` stay on
workspace metadata: they never expose passwords, passphrases, or key files, and they connect to no
server. `deployment_read_remote` and `deployment_compare` connect to the server of one profile with
its stored credentials and read text files. They never modify the remote server and never return
credentials. Transfers and remote writes belong to the command line. An untrusted SFTP host key
causes an error with the fingerprint. Verify the fingerprint before using `iwe deployment test
<profile> --trust-host-key FINGERPRINT`. Use a unique profile name or its identifier when calling
the MCP deployment tools. Connections that [use SSH config](deployment.md#connections-and-servers)
report the authentication value `ssh-config`. Their host field contains the alias.

Tool results arrive as text content with a JSON payload. `resolve_deployment_path` returns
`profile`, `local`, and `remote`. `deployment_read_remote` returns `profile`, `path`, and `content`.
`deployment_compare` returns `profile`, `local`, `remote`, `identical`, `localText`, and
`remoteText`. Each text in the comparison has a 512 KiB limit. Tool failures carry `isError: true`
and a short message. The messages stay generic and never contain file paths.

## Editing files

`read_file` returns the content and a revision token. The token is the SHA-256 hash of the content.
`write_file` needs that token. The server re-reads the file and compares the hash before writing.
The write fails when the file changed after the read. This check prevents accidental overwrites.

`write_file` reuses the editor's save operation. The save writes through a temporary file, keeps the
file permissions, and rechecks the disk content before replacing it. The editor applies the same
check in reverse: its autosave refuses to replace disk content that changed underneath it. Both
sides therefore keep their own data.

`write_file` changes existing text files only. It cannot create files, delete files, or write binary
content. The editor limits the content to 2 MiB. The complete JSON-RPC request must also fit the 1
MiB incoming line limit.

## Project resolution and limits

The server resolves the workspace metadata like the application: `IWE_DATA_DIR/workspace.json`, or
the platform configuration directory. Set `IWE_DATA_DIR` to serve a different workspace.

The tools read projects from the workspace metadata. They reject paths outside the project
directory, absolute paths, and paths with parent components. Directory listings and file reads skip
symlinks. The search walk reuses the background tree limits: 4,096 directories, 100,000 files, and a
depth of 24. The walk skips the same directories as the background indexer, such as `node_modules`
and `target`.

Only `write_file` writes data. It can replace an existing local project text file. Workspace
metadata and remote files stay unchanged. Remote reads use the stored credentials of the selected
profile. The tool results omit credentials. Git tools run read-only commands without optional locks.

Use the [CLI](cli.md) to manage workspace projects or deployment settings, test connections, and
transfer files.

## Connecting a client

Point an MCP client at the installed binary. Examples:

Claude Code:

```sh
claude mcp add iwe -- /path/to/iwe --mcp
```

Claude Desktop, Cursor, and other clients with `mcpServers` configuration:

```json
{
  "mcpServers": {
    "iwe": {
      "command": "/path/to/iwe",
      "args": ["--mcp"]
    }
  }
}
```

On macOS the binary sits inside the application bundle at `/Applications/IWE
Code.app/Contents/MacOS/iwe`. On Windows the installed executable provides standard input and output
to spawning clients. Building from source yields `target/release/iwe`.

## Platform notes

The server uses standard input and output only. It runs on macOS, Windows, and Linux. The release
build on Windows uses the windowed subsystem, which does not prevent clients from piping protocol
messages. The Windows and Linux checks run in the release CI matrix. Local Windows verification is
not available on the macOS development machine.

## Testing

Protocol parsing, version negotiation, and transport limits carry unit tests in
`src/infrastructure/mcp/tests.rs`. Tool helpers, path validation, and the deployment tools carry
unit tests beside `src/app/mcp/tools.rs`; the deployment tests connect to the in-memory SSH fixture
from `infrastructure/deployment/transport/fixtures`. The integration test `tests/mcp_server.rs`
starts the binary with a temporary data directory and exercises the full protocol: handshake, tool
catalog, workspace reads, search, Git reads, and revision-checked writes. Tests never touch the
user's data directory.

Run `make check` to run formatting, lint, type checks, tests, and security checks. The website
explains client setup for users on the
[MCP page](https://adametsderschopfer.github.io/iwe-code/docs/mcp/).
