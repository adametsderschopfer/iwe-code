//! Serve the Model Context Protocol over standard input and output.

mod protocol;

#[cfg(test)]
mod tests;

use crate::infrastructure::ipc::{Line, read_line};
use protocol::Incoming;
use serde_json::{Value, json};
use std::{future::Future, process::ExitCode};
use tokio::io::{AsyncWriteExt, BufReader};

pub(crate) use protocol::{Request, Response};

/// A receiver of tool requests that returns one response per request.
pub(crate) trait Handler {
    fn handle(&mut self, request: Request) -> impl Future<Output = Response>;
}

/// Report whether the process was started in server mode.
pub(crate) fn requested() -> bool {
    std::env::args_os()
        .skip(1)
        .any(|argument| argument.as_os_str() == std::ffi::OsStr::new("--mcp"))
}

/// Serve protocol requests until the client closes the stream.
///
/// Returns success after a clean end of stream and failure on transport errors.
pub(crate) async fn serve<H: Handler>(handler: H) -> ExitCode {
    let mut handler = handler;
    let mut input = BufReader::new(tokio::io::stdin());
    let mut output = tokio::io::stdout();
    loop {
        let line = match read_line(&mut input, protocol::MAX_LINE).await {
            Ok(Some(line)) => line,
            Ok(None) => return ExitCode::SUCCESS,
            Err(_) => return ExitCode::FAILURE,
        };
        let response = match &line {
            Line::TooLong => Response::error(
                Value::Null,
                protocol::PARSE_ERROR,
                "The message exceeds the line size limit.",
            ),
            Line::Text(line) => {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                match protocol::parse(line) {
                    Incoming::Notification => continue,
                    Incoming::Malformed(response) => response,
                    Incoming::Call(request) => route(request, &mut handler).await,
                }
            }
        };
        if write_line(&mut output, &response).await.is_err() {
            return ExitCode::FAILURE;
        }
    }
}

/// Route one request: protocol methods here, tool methods through the handler.
async fn route<H: Handler>(request: Request, handler: &mut H) -> Response {
    match request.method.as_str() {
        "initialize" => {
            let requested = request
                .params
                .get("protocolVersion")
                .and_then(Value::as_str);
            protocol::initialize(request.id, requested)
        }
        "ping" => Response::result(request.id, json!({})),
        "tools/list" | "tools/call" => handler.handle(request).await,
        _ => Response::method_not_found(request.id),
    }
}

async fn write_line(output: &mut tokio::io::Stdout, response: &Response) -> std::io::Result<()> {
    output.write_all(response.encode().as_bytes()).await?;
    output.flush().await
}
