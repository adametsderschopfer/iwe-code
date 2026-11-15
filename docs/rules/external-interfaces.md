# External interfaces

Follow these rules when [AGENTS.md](../../AGENTS.md) directs you here.

Three interfaces connect the outside world with the application:

1. The command line (`src/cli/`).
2. The MCP server (`src/infrastructure/mcp/`, `src/app/mcp/`).
3. External opening and deep links (`src/infrastructure/external/`, `src/app/external/`).

Read the feature document before changing an interface:

- [Command line](../cli.md)
- [MCP server](../mcp.md)
- [External opening and deep links](../external-opening.md)

Keep the feature document current in the same change as the code.

## Behavior contracts

The feature documents define stable contracts. Contracts include:

- Command tree, options, and exit codes.
- JSON keys and output shapes.
- MCP protocol versions and transport limits.
- Deep link grammar and platform registration.

Change a contract only together with its document, tests, and translations. Interface text follows
the [i18n rules](i18n.md).

Keep every interface usable without the window. Headless code must not depend on the graphics stack
or open windows.

## Startup order

`main` handles the interfaces in one order:

1. MCP server.
2. Internal update flag.
3. Command line gate.
4. External request forwarding.
5. Interface.

Preserve this order. Each step can exit the process before later steps run. A new startup step joins
this order with an explicit reason.

## Data and security

The command line reads and writes workspace metadata through the shared workspace layer. The MCP
server reads it the same way and never writes it. Do not duplicate loading or saving logic.
Deployment commands are the only command-line path that opens network connections. They connect to
the servers their profiles describe and never print stored secrets.

The command line and the interface save without locking. Keep the documented warning about
concurrent changes.

Only `write_file` writes data through MCP. Require a revision token for updates to existing local
project text files. Keep workspace metadata and remote files read-only. Deployment read tools may
connect to the configured server with stored credentials. Exclude credentials from tool results. The
command line and interface manage deployment settings and transfers. Git reads run the `git` command
with read-only arguments. Validate every file path against the owning project directory or the
profile mappings. Keep error messages generic and free of file paths.

Deep links and forwarded requests carry absolute file-system paths only. Ignore unknown actions and
relative paths.

## Testing

Test each interface at two levels:

- Unit tests in adjacent `tests.rs` files.
- Integration tests under root `tests/` that run the real binary.

Integration tests use an isolated `IWE_DATA_DIR`. They never touch the user's data directory.

Verify the interfaces on macOS locally. Windows and Linux verification runs in the release CI
matrix. macOS open events need the released bundle. The development binary accepts command-line
files and links.
