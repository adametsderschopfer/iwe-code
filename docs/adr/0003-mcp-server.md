# MCP server

Status: accepted.

## Context

AI assistants need structured access to the IWE Code workspace. The Model Context Protocol (MCP) is
the established standard for this access. Clients such as Claude Code, Claude Desktop, and Cursor
spawn a server process and exchange JSON-RPC messages over its standard streams. The workspace
metadata, file reads, Git reads, and conflict-checked writes already exist in the application crate.

## Decision

Embed the MCP server in the main `iwe` binary behind the `--mcp` flag. The flag runs before every
other startup step, including the command line and update handling. The server speaks JSON-RPC 2.0
over standard input and output, one message per line, and implements the MCP stdio transport with
the tools capability only. The implementation follows the line protocol pattern of the external
module system and uses the existing dependencies only.

Expose seven tools: `list_projects`, `list_files`, `read_file`, `write_file`, `search_files`,
`git_status`, and `git_log`. All tools except `write_file` only read data. The server loads the
workspace metadata for every request, so it always reflects the running application's saved state.

Route file writes through `infrastructure::editor::save`. `read_file` returns a SHA-256 revision
token with the content. `write_file` requires that token, rechecks the disk content, and hands the
current content to the editor's save operation. The save rechecks again before replacing the file.
The editor's autosave applies the same check against disk changes. Both sides therefore refuse to
overwrite newer content.

Keep the transport and protocol in `infrastructure/mcp` and the tool orchestration in `app/mcp`. The
infrastructure layer owns the streams and JSON-RPC framing. The application layer maps tool calls to
workspace, search, Git, and editor operations and injects the handler into the transport.

## Alternatives

A separate `mcp` crate would isolate the server, but the data access code is `pub(crate)` inside the
single application crate. Sharing it would require converting the crate to a library target with
public visibility across every layer. Duplicating the readers in a second crate would violate the
single-source rule for storage behavior.

A server inside the running application, exposed through a socket or HTTP, would reach live
interface state. It would require transport and authentication choices that MCP clients cannot
discover automatically, and it would serve nothing when the application is closed.

The external module system fits interface panels, not protocol servers. Its process host renders
widget trees and has no JSON-RPC surface for AI clients.

## Consequences

The binary gains a second startup mode with no new dependencies and no interface changes. The server
exposes only projects listed in `workspace.json` and never writes workspace metadata. It never reads
stored credentials and runs Git with read-only arguments. File writes stay bounded by the editor
limits and keep the conflict checks on both sides. The MCP tool names, arguments, and revision token
become public interface. The website documents client setup, and [the MCP guide](../mcp.md)
documents the protocol surface, limits, and tests.
