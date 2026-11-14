//! Serve the Model Context Protocol over standard input and output.

mod protocol;

#[cfg(test)]
mod tests;

use protocol::Incoming;
use serde_json::{Value, json};
use std::{future::Future, process::ExitCode};
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncWriteExt, BufReader};

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

/// One input line as read from the stream.
#[derive(Debug, PartialEq, Eq)]
enum Line {
    Text(String),
    /// The line exceeded the size cap. Its remainder was discarded.
    TooLong,
}

/// Serve protocol requests until the client closes the stream.
///
/// Returns success after a clean end of stream and failure on transport errors.
pub(crate) async fn serve<H: Handler>(handler: H) -> ExitCode {
    let mut handler = handler;
    let mut input = BufReader::new(tokio::io::stdin());
    let mut output = tokio::io::stdout();
    loop {
        let line = match read_line(&mut input).await {
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

/// Read one newline-terminated line, buffering at most [`protocol::MAX_LINE`] bytes.
///
/// Returns `None` only when the stream ends before any bytes arrive. A final
/// line without a newline is returned like a complete line.
async fn read_line<R: AsyncBufRead + Unpin>(input: &mut R) -> std::io::Result<Option<Line>> {
    let mut bytes = Vec::new();
    let mut consumed_any = false;
    let mut too_long = false;
    loop {
        let available = input.fill_buf().await?;
        if available.is_empty() {
            break;
        }
        let complete = available
            .iter()
            .position(|byte| *byte == b'\n')
            .unwrap_or(available.len());
        let newline_found = complete < available.len();
        if !too_long {
            if bytes.len() + complete > protocol::MAX_LINE {
                too_long = true;
                bytes = Vec::new();
            } else {
                bytes.extend_from_slice(&available[..complete]);
            }
        }
        let consumed = complete + usize::from(newline_found);
        input.consume(consumed);
        consumed_any |= consumed > 0;
        if newline_found {
            break;
        }
    }
    let line = if too_long {
        Line::TooLong
    } else {
        Line::Text(String::from_utf8_lossy(&bytes).into_owned())
    };
    Ok(consumed_any.then_some(line))
}
