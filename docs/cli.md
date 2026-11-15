# Command line

The application binary provides a small command line for scripts. It reads and changes saved
workspace metadata and starts the interface. Workspace and deployment commands run without opening a
window. Workspace commands use local files. Deployment transfers and connection tests use the
protocol configured in the profile. Use [MCP](mcp.md) for project file editing, Git inspection, and
remote text comparison.

`iwe` names the application binary in the examples. On macOS the binary lives inside the application
bundle. On Windows it is `iwe.exe`. The packaging scripts put it next to the other release files.

## Commands

The command line starts only when the first argument is not a directory, a file, a deep link, or an
internal update flag. Those arguments keep their interface behavior.

| Command                                                                                                                                                                                    | Behavior                                                                          |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | --------------------------------------------------------------------------------- |
| `iwe open [directory]`                                                                                                                                                                     | Start a fresh interface process. Pass an existing directory to open that project. |
| `iwe workspace list [--json]`                                                                                                                                                              | Print spaces with their projects.                                                 |
| `iwe workspace add-project <directory> [--space NAME] [--json]`                                                                                                                            | Add a project directory to a space.                                               |
| `iwe workspace remove-project <project> [--json]`                                                                                                                                          | Remove a project by directory or exact name.                                      |
| `iwe deployment list [--json]`                                                                                                                                                             | Print deployment connections, keys, and profiles.                                 |
| `iwe deployment connection add <name> --host HOST --username NAME [options] [--json]`                                                                                                      | Add a deployment connection.                                                      |
| `iwe deployment connection set <name> [options] [--json]`                                                                                                                                  | Change the given connection values.                                               |
| `iwe deployment connection remove <name> [--json]`                                                                                                                                         | Remove an unused connection and its stored secret.                                |
| `iwe deployment key add <name> <path> [--json]`                                                                                                                                            | Register an SSH key file.                                                         |
| `iwe deployment key remove <name> [--json]`                                                                                                                                                | Remove an unused key and its stored passphrase.                                   |
| `iwe deployment profile add <name> --project DIR --connection NAME --root PATH --mapping LOCAL=REMOTE [--mapping LOCAL=REMOTE]... [--exclude PATH]... [--default] [--space NAME] [--json]` | Add a deployment profile.                                                         |
| `iwe deployment profile set <name> [options] [--json]`                                                                                                                                     | Change the given profile values.                                                  |
| `iwe deployment profile remove <name> [--space NAME] [--json]`                                                                                                                             | Remove a deployment profile.                                                      |
| `iwe deployment test <profile> [--space NAME] [--trust-host-key FINGERPRINT] [--json]`                                                                                                     | Check the profile connection.                                                     |
| `iwe deployment upload <profile> <path> [--space NAME] [--trust-host-key FINGERPRINT] [--json]`                                                                                            | Upload a local file or folder.                                                    |
| `iwe deployment download <profile> <remote> [--overwrite] [--space NAME] [--trust-host-key FINGERPRINT] [--json]`                                                                          | Download a remote file or folder.                                                 |
| `iwe deployment list-remote <profile> [directory] [--space NAME] [--trust-host-key FINGERPRINT] [--json]`                                                                                  | List one remote directory.                                                        |
| `iwe --help`, `iwe -h`                                                                                                                                                                     | Print localized help.                                                             |
| `iwe --version`, `iwe -V`                                                                                                                                                                  | Print the version, for example `iwe 1.4.0`.                                       |

`iwe open` always starts a new interface process. Running instances receive nothing, because the
command line exits before the request-forwarding path.

`workspace add-project` accepts an existing directory. Without `--space`, the project goes to the
first space. An empty workspace gets a space named after the project folder. Adding an existing
project directory changes nothing and succeeds.

`workspace remove-project` accepts an existing directory, a stored project directory, or an exact
project name. A name that matches several project directories is rejected. The command removes every
space entry for that directory.

Unknown commands and options produce an error. They no longer fall through to the interface.

### Deployment commands

Use `--help` after any command to see its arguments, for example `iwe deployment profile set
--help`.

Connection add and set accept `--host`, `--username`, `--protocol`, `--port`, `--auth`, `--key`, and
`--password-stdin`. Adding a connection requires `--host` and `--username`.

Connections carry a protocol (`sftp`, `ftp`, `ftps`, `ftps-implicit`), a host, a port, a user name,
and an authentication method (`password`, `agent`, or `key`). Without `--port`, the protocol default
applies. Without `--auth`, the connection uses a password. `--auth key` needs `--key` with a
registered key name. `--password-stdin` reads the password or the key passphrase from standard
input. Secrets are never accepted as command-line options.

Profiles bind one project directory to one connection. `--root` is the absolute remote root.
`--mapping` pairs a project-relative local folder with a root-relative remote folder. At least one
mapping is required. `--exclude` ignores project-relative paths. `--default` marks the profile as
the space default and clears the flag on the other profiles of that space. Without `--space`, the
profile goes to the selected space, or to the first space.

Use `profile set` to update the project, connection, root, mappings, exclusions, or default status.
`--clear-excluded` removes exclusions. `--default false` clears the default status.

Commands that take a profile resolve it by name or identifier. A match in the selected space takes
priority. If several other spaces contain a match, use `--space` or the profile identifier from
`deployment list --json`. Connections and keys also accept a name or identifier.

`deployment download` keeps existing local files unless `--overwrite` is given. An SFTP connection
to an unknown host reports its key fingerprint. Verify the fingerprint before retrying with
`--trust-host-key FINGERPRINT`. The CLI saves the supplied fingerprint. If the operation still
reports an unknown key, run it again so the next invocation uses the saved fingerprint.

## Output formats

Every listing command prints localized text by default. The `--json` flag prints stable JSON with
two-space indentation. Keys use snake_case, matching `workspace.json`.

`workspace list --json`:

```json
{
  "spaces": [
    {
      "name": "Work",
      "projects": [
        {
          "name": "app",
          "directory": "/Users/name/code/app",
          "pinned": false
        }
      ]
    }
  ]
}
```

`workspace add-project --json` and `workspace remove-project --json` print the affected project
object. Removal prints `{"removed": [project, ...]}` because one target can remove several space
entries.

`deployment list --json` prints connections without secrets, registered keys, and the profiles of
every space:

```json
{
  "connections": [
    {
      "id": "c1",
      "name": "web",
      "protocol": "sftp",
      "host": "example.com",
      "port": 22,
      "username": "deploy",
      "authentication": "password",
      "key": null
    }
  ],
  "keys": [
    {
      "id": "k1",
      "name": "staging",
      "path": "/Users/name/.ssh/id_ed25519"
    }
  ],
  "spaces": [
    {
      "name": "Work",
      "profiles": [
        {
          "id": "p1",
          "name": "production",
          "project": "/Users/name/code/app",
          "connection": "c1",
          "connection_name": "web",
          "root": "/var/www",
          "mappings": [
            {
              "local": "web",
              "remote": "assets"
            }
          ],
          "excluded": ["web/uploads"],
          "default": true
        }
      ]
    }
  ]
}
```

`connection add --json` and `connection set --json` print the connection object. `profile add
--json` and `profile set --json` print the profile object. Removal commands print `{"removed":
name}`. `test --json` prints `{"profile": name, "connected": true}`. Successful transfers print
`{"profile": name, "files": count, "bytes": count}`. `list-remote --json` prints `{"directory":
path, "entries": [{"name": name, "path": path, "kind": "file", "size": size}]}` with kinds `file`,
`directory`, and `symlink`.

## Exit codes

| Code | Meaning                                                                                                                                     |
| ---- | ------------------------------------------------------------------------------------------------------------------------------------------- |
| `0`  | The command completed.                                                                                                                      |
| `1`  | The command failed. The workspace file was unreadable or unwritable, a path was missing, no deployment entry matched, or a transfer failed. |
| `2`  | The invocation was invalid. The option, command, or value did not parse, or `open` received a missing directory.                            |

## Storage and language

The commands read and write the same `workspace.json` file as the interface. `IWE_DATA_DIR`
overrides its location, as described in [workspace behavior](workspace.md). Close the application
before changing the workspace from the command line. Both processes save without locking, so a
concurrent interface save would overwrite command-line changes.

`IWE_LANG` and the system locale select the output language, including help text. Unsupported
languages fall back to English.

## Implementation

The command line lives in `src/cli/`. `main` calls `cli::run()` after the update helper and before
external request handling. The gate yields interface-owned arguments, then parses the rest with
clap. The command tree is built at runtime from the Fluent catalogs, so help text and errors follow
the same translations as the interface. Section headings require static strings, so the localized
heading words are leaked once per process.

`workspace` commands reuse `infrastructure::workspace::load` and `save` on a local tokio runtime.
Space and project changes use the pure `Workspace` operations in `domain`, including
`Workspace::remove_projects`. JSON views live in `src/cli/output.rs`.

`deployment` commands reuse the same load and save helpers. Settings changes mutate the saved
`Connections` and `Profile` values from `domain::deployment` and validate them with
`Connection::validate` and `Profile::validate`. Transfers call the shared engine in
`infrastructure::deployment::transfer`, the engine that also serves the interface. Connection
removal deletes the stored secret file, which the interface currently keeps.

Unit tests sit in `src/cli/tests.rs` and `src/domain/workspace/tests.rs`. The integration test
`tests/cli.rs` runs the real binary against an isolated `IWE_DATA_DIR` and never touches user data.
