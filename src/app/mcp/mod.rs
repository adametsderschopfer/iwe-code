//! Serve MCP tool calls with workspace, file, search, and Git access.

mod tools;

#[cfg(test)]
mod tests;

use crate::infrastructure::mcp::{self, Request, Response};
use serde_json::{Value, json};
use std::process::ExitCode;

/// Run the MCP server until the client closes the stream.
pub(crate) fn serve() -> ExitCode {
    match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime.block_on(mcp::serve(Server)),
        Err(_) => ExitCode::FAILURE,
    }
}

struct Server;

impl mcp::Handler for Server {
    async fn handle(&mut self, request: Request) -> Response {
        match request.method.as_str() {
            "tools/list" => Response::result(request.id, tools::list()),
            "tools/call" => call(request).await,
            _ => Response::method_not_found(request.id),
        }
    }
}

/// Run one tools/call envelope and wrap the outcome as tool content.
async fn call(request: Request) -> Response {
    let id = request.id;
    let Some(name) = request.params.get("name").and_then(Value::as_str) else {
        return Response::invalid_params(id, "A tool call must name a tool.");
    };
    let arguments = request
        .params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let (text, is_error) = if arguments.as_object().is_none() {
        ("Tool arguments must be an object.".to_owned(), true)
    } else {
        match tools::dispatch(name, &arguments).await {
            Ok(value) => (
                serde_json::to_string_pretty(&value).unwrap_or_default(),
                false,
            ),
            Err(message) => (message, true),
        }
    };
    Response::result(
        id,
        json!({
            "content": [{ "type": "text", "text": text }],
            "isError": is_error,
        }),
    )
}
