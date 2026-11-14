# Command line interface

Status: accepted.

## Context

The application shipped with almost no command-line surface. A single directory argument opened a
project in the interface, and two internal flags served the update helper. Scripts and terminal
users could not inspect or change the saved workspace without opening the interface. External
opening now forwards files and deep links to a running instance, but the saved workspace metadata
stays invisible to scripts.

A command line needs a parser, localized help, and stable machine-readable output. The interface
layer stays untouched: scripts read and change `workspace.json` the same way the interface does.

## Decision

Keep one binary. `main` routes the command line before the interface and its request forwarding. The
gate in `src/cli` yields directories, files, deep links, and internal update flags, so existing
launch behavior stays intact. Everything else parses as a command.

The first version serves four commands: `open`, `workspace list`, `workspace add-project`, and
`workspace remove-project`. They cover launching, reading, and changing saved workspace metadata.
`--json` prints stable machine-readable documents with snake_case keys. Exit codes are `0` for
success, `1` for operation failures, and `2` for usage errors.

Use clap with its builder API, without derive. The command tree builds at runtime from the Fluent
catalogs. Help text and error messages follow the interface translations, and the parser brings
usage strings, suggestions, and argument validation. clap requires static strings for section
headings; the command leaks the localized heading words once per process.

Interface commands start a fresh process. `iwe open` launches the interface binary with the optional
directory and exits. This keeps the command line free of interface state and reuses the existing
directory-argument path.

## Alternatives

A separate `iwe-cli` binary would split installation and packaging for no gain. Everything is
`pub(crate)`, so a library target would add a public API boundary without a consumer. Derive macros
would bake English help text into the binary at compile time and fight the translation catalogs.

A daemon with a local socket could drive a running interface. The interface would need new message
routing for external commands, and operations would depend on an open window. The saved workspace
file already serves as the integration point, so the first version reads and writes it directly. A
socket protocol stays possible once interface-controlled commands arrive.

Hand-rolled argument parsing would avoid the clap dependency but reimplement help rendering,
suggestions, and validation for every future command.

## Consequences

Unknown arguments now fail with exit code `2` instead of silently opening the interface. The gate
preserves directories, files, deep links, and `--iwe-*` flags, so documented launch paths stay
intact.

The interface and the command line save `workspace.json` without locking. The documentation asks
users to close the application before command-line changes. A lock file or single-writer protocol
can arrive with the first command that cannot tolerate lost updates.

Future read-only commands can reuse the infrastructure functions directly: repository reads for
`status` and `log`, the project index and search ranking for `search`. They add no new layers.
