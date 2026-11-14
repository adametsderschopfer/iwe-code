# Command line

The application binary provides a small command line for scripts. It reads and changes saved
workspace metadata and starts the interface. The command line runs headless. It opens no window and
performs no network operations.

`iwe` names the application binary in the examples. On macOS the binary lives inside the application
bundle. On Windows it is `iwe.exe`. The packaging scripts put it next to the other release files.

## Commands

The command line starts only when the first argument is not a directory, a file, a deep link, or an
internal update flag. Those arguments keep their interface behavior.

| Command                                                         | Behavior                                                                          |
| --------------------------------------------------------------- | --------------------------------------------------------------------------------- |
| `iwe open [directory]`                                          | Start a fresh interface process. Pass an existing directory to open that project. |
| `iwe workspace list [--json]`                                   | Print spaces with their projects.                                                 |
| `iwe workspace add-project <directory> [--space NAME] [--json]` | Add a project directory to a space.                                               |
| `iwe workspace remove-project <project> [--json]`               | Remove a project by directory or exact name.                                      |
| `iwe --help`, `iwe -h`                                          | Print localized help.                                                             |
| `iwe --version`, `iwe -V`                                       | Print the version, for example `iwe 1.0.8`.                                       |

`iwe open` always starts a new interface process. Running instances receive nothing, because the
command line exits before the request-forwarding path.

`workspace add-project` accepts an existing directory. Without `--space`, the project goes to the
first space. An empty workspace gets a space named after the project folder. Adding an existing
project directory changes nothing and succeeds.

`workspace remove-project` accepts an existing directory, a stored project directory, or an exact
project name. A name that matches several project directories is rejected. The command removes every
space entry for that directory.

Unknown commands and options produce an error. They no longer fall through to the interface.

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

## Exit codes

| Code | Meaning                                                                                                              |
| ---- | -------------------------------------------------------------------------------------------------------------------- |
| `0`  | The command completed.                                                                                               |
| `1`  | The command failed. The workspace file was unreadable or unwritable, a directory was missing, or no project matched. |
| `2`  | The invocation was invalid. The option, command, or value did not parse, or `open` received a missing directory.     |

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

Unit tests sit in `src/cli/tests.rs` and `src/domain/workspace/tests.rs`. The integration test
`tests/cli.rs` runs the real binary against an isolated `IWE_DATA_DIR` and never touches user data.
