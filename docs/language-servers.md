# Code assistance

IWE uses local language servers to analyze editable project files. Language servers communicate
through the Language Server Protocol (LSP). The application starts each server when a document first
needs it. Servers receive unsaved text from open project documents.

## Editing

Type an identifier or member access to request suggestions. Press Control+Space to request
suggestions explicitly. Use the arrow keys to select a suggestion. Press Enter or Tab to insert it.
Matching preserves the typed letter case: `per` ranks `person` above `Person`, while `Per` reverses
that order. Equally matching candidates retain server priorities and preferred selections. Press
Escape to dismiss suggestions. Insertion and additional import edits form one undo operation. The
application rejects responses for changed document text or cursor positions.

Use Command+B on macOS to open a definition. Use Control+B on Windows and Linux. Change this
shortcut in Settings, Shortcuts. Multiple definitions appear in a selection list. Code errors appear
as underlines. Hover over an underline to read the diagnostic.

Use Command+Option+L on macOS to format the current document. Use Control+Alt+L on Windows and
Linux. IWE first searches for a configured project formatter. It otherwise asks the language server
for document formatting. Formatting forms one undo operation and follows normal autosave behavior.
The application rejects formatting responses after the document changes. Settings, Shortcuts lets
users change the formatting shortcut.

Use Command+D on macOS to duplicate the current line. Use Control+D on Windows and Linux.
Duplication preserves line endings and moves the cursor to the copied line. One undo operation
removes the copied line.

On macOS, Command+Backspace deletes from the cursor to the start of the line. Option+Backspace
deletes the preceding word. Control+Backspace provides word deletion on Windows and Linux. With a
selection, these shortcuts delete the selection. Modified forward deletion uses the corresponding
line or word boundary.

Completion and diagnostic surfaces preserve the editor widget state when they appear or disappear.
The editor keeps keyboard focus and drag selection during these changes.

## Editor context menu

Right-click source text to open the editor menu. Shift+F10 opens it from the focused editor. A click
inside the selection preserves it. A click outside moves the cursor. Up and Down move between
available commands. The menu lists navigation and code actions under separate headings. Enter runs
an action. Escape closes the menu. Read-only source tabs provide copying, selection, search, and
line navigation.

Editable tabs add clipboard commands, undo, redo, and line duplication. **Navigate** offers line
navigation, definitions, and language-server references for supported symbols. **Code actions**
provides formatting and semantic renaming for file-local symbols. Renaming asks the language server
for edits. IWE validates every edit before changing the file. Renaming a symbol that requires
changes elsewhere leaves every file intact and explains the scope. A successful rename forms one
undo operation. Language actions require a compatible server.

## Project formatting

IWE searches from the file's directory toward the project root. The nearest formatter configuration
wins. The formatter reads the unsaved document through standard input. Its output replaces the
document after validation. The formatter invocation never passes an option to rewrite source files.

| Formatter | Detection                                                                          | Executable selection                                                              |
| --------- | ---------------------------------------------------------------------------------- | --------------------------------------------------------------------------------- |
| Prettier  | `.prettierrc.*`, `prettier.config.*`, or the `prettier` property in `package.json` | Nearest installed `node_modules/prettier`, then a system executable on Unix       |
| Biome     | `biome.json` or `biome.jsonc`                                                      | Nearest installed `node_modules/@biomejs/biome`, then a system executable on Unix |
| dprint    | `dprint.json`, `dprint.jsonc`, or their dot-prefixed forms                         | System executable                                                                 |
| Ruff      | `ruff.toml`, `.ruff.toml`, or `[tool.ruff]` in `pyproject.toml`                    | Nearest `.venv` or `venv`, then a system executable                               |
| Black     | `[tool.black]` in `pyproject.toml`                                                 | Nearest `.venv` or `venv`, then a system executable                               |

At one directory level, Biome takes priority over dprint and Prettier. Ruff takes priority over
Black. Without configuration, a local Prettier or Biome package can provide defaults for supported
web documents. PHP and TOML require explicit Prettier configuration for plugins. Biome does not
capture PHP or TOML files. IWE does not download missing formatters or invoke package-manager
scripts during formatting. A configured formatter failure stops formatting. IWE does not silently
substitute another tool's rules.

Each formatter resolves its own project options and supported file formats. Prettier also resolves
its plugins and EditorConfig settings. IWE passes the original document path for this purpose. See
the [Prettier CLI](https://prettier.io/docs/cli), [Biome CLI](https://biomejs.dev/reference/cli/),
[Ruff configuration](https://docs.astral.sh/ruff/configuration/), and
[Black configuration](https://black.readthedocs.io/en/stable/usage_and_configuration/the_basics.html).

Language servers remain the fallback for Go, Rust, PHP, and other supported languages. The fallback
requests four-space indentation. Go and Rust servers use their language-specific formatting
behavior. Pyright does not provide formatting; Python projects need a configured Ruff or Black
installation. ESLint configuration alone does not enable automatic fixes. Formatting does not run
semantic lint fixes or change imports automatically.

Formatter execution has a 25-second limit. IWE rejects oversized output, unsuccessful processes, and
unexpected empty output. Normal editor autosave can write an accepted formatting result afterward.

## Language programs

Language servers install separately from IWE. The application installer does not bundle language
servers or their runtimes. In Settings, Language servers lists the default executable for each
language. Use **Detect** to search for it or check the selected executable. Use **Choose path** to
select a local executable. **Automatic** clears a custom path. **Install guide** opens the server
maintainer's instructions or download page. Detection checks the file location. Startup also
requires the server's runtime and project dependencies.

| Documents                        | Program                       | Default arguments |
| -------------------------------- | ----------------------------- | ----------------- |
| Python                           | `pyright-langserver`          | `--stdio`         |
| Go                               | `gopls`                       | `serve`           |
| JavaScript, JSX, TypeScript, TSX | `typescript-language-server`  | `--stdio`         |
| PHP                              | `intelephense`                | `--stdio`         |
| Rust                             | `rust-analyzer`               | none              |
| Markdown                         | `marksman`                    | `server`          |
| JSON, JSONC                      | `vscode-json-language-server` | `--stdio`         |
| YAML                             | `yaml-language-server`        | `--stdio`         |
| TOML                             | `taplo`                       | `lsp stdio`       |
| Shell                            | `bash-language-server`        | `start`           |
| HTML                             | `vscode-html-language-server` | `--stdio`         |
| CSS                              | `vscode-css-language-server`  | `--stdio`         |

Automatic discovery searches PATH first. On macOS it also searches the standard Homebrew folders. It
then searches the application data directory under `iwe/language-servers/node_modules/.bin` and
`iwe/language-servers/bin`. It also searches `.cargo/bin` and `go/bin` under the home directory.
Custom executable paths use the arguments listed above. They do not accept shell commands. Each
server requires its own runtime and project dependencies. Go and Rust servers need compatible
language toolchains.

## Configuration files and Markdown

JSON supports schema references through `$schema`. YAML supports schema references through the
language server's schema comment. The language server supplies schema suggestions. These references
can require network access when they name remote schemas.

Markdown code fences select their language from the fence label. Completion requests use a temporary
document identity without writing a file. The application preserves source line numbers. Completion
edits cannot modify text outside the active fence. Standalone examples can provide fewer project
symbols than normal source files. Diagnostics currently refresh the Markdown document itself.

ENV files use local key suggestions without an external server. The application reads keys from the
current document and project templates. Supported templates are `.env.example`, `.env.sample`, and
`.env.template`. Suggestions do not expose template values.

## Scope

Code assistance applies to local editable files. Remote deployment previews and historical
comparisons do not start language servers. Server capabilities determine available suggestions and
definitions. The diagnostics timer runs for supported active documents and when closed documents
need cleanup. IWE retries failed diagnostics after 30 seconds. Successful program detection restores
the normal 800-millisecond interval. Missing servers currently produce no suggestions. On Windows,
IWE resolves supported npm packages to their JavaScript entry points. It runs those entry points
through `node.exe` without executing command wrappers. Node must exist beside the wrapper or in the
discovery search paths.
