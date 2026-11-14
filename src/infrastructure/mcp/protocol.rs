//! Encode and parse JSON-RPC 2.0 messages for the Model Context Protocol.

use serde_json::{Value, json};

/// Maximum size of one protocol line, matching the external module protocol.
pub(crate) const MAX_LINE: usize = 1024 * 1024;

pub(crate) const PARSE_ERROR: i32 = -32700;
const INVALID_REQUEST: i32 = -32600;
const METHOD_NOT_FOUND: i32 = -32601;
const INVALID_PARAMS: i32 = -32602;

const LATEST_VERSION: &str = "2025-06-18";
const SUPPORTED_VERSIONS: [&str; 3] = ["2024-11-05", "2025-03-26", "2025-06-18"];

/// A JSON-RPC request with an identifier that expects a response.
pub(crate) struct Request {
    pub(crate) id: Value,
    pub(crate) method: String,
    pub(crate) params: Value,
}

/// A JSON-RPC response to one request.
pub(crate) enum Response {
    Result {
        id: Value,
        value: Value,
    },
    Error {
        id: Value,
        code: i32,
        message: String,
    },
}

impl Response {
    pub(crate) fn result(id: Value, value: Value) -> Self {
        Self::Result { id, value }
    }

    pub(crate) fn error(id: Value, code: i32, message: impl Into<String>) -> Self {
        Self::Error {
            id,
            code,
            message: message.into(),
        }
    }

    pub(crate) fn method_not_found(id: Value) -> Self {
        Self::error(
            id,
            METHOD_NOT_FOUND,
            "The requested method is not available.",
        )
    }

    pub(crate) fn invalid_params(id: Value, message: impl Into<String>) -> Self {
        Self::error(id, INVALID_PARAMS, message)
    }

    /// Encode the response as one protocol line, including the newline.
    pub(crate) fn encode(&self) -> String {
        let value = match self {
            Self::Result { id, value } => json!({ "jsonrpc": "2.0", "id": id, "result": value }),
            Self::Error { id, code, message } => json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": { "code": code, "message": message }
            }),
        };
        format!("{}\n", value)
    }
}

/// One parsed input line.
pub(crate) enum Incoming {
    Call(Request),
    /// A request without an identifier. Notifications never receive a reply.
    Notification,
    /// The line was not a usable JSON-RPC message. The response reports why.
    Malformed(Response),
}

/// Parse one protocol line into a request, a notification, or a rejection.
pub(crate) fn parse(line: &str) -> Incoming {
    let Ok(value) = serde_json::from_str::<Value>(line) else {
        return Incoming::Malformed(Response::error(
            Value::Null,
            PARSE_ERROR,
            "The message is not valid JSON.",
        ));
    };
    let Some(object) = value.as_object() else {
        return Incoming::Malformed(Response::error(
            Value::Null,
            INVALID_REQUEST,
            "A protocol message must be a JSON object.",
        ));
    };
    let id = object.get("id").cloned();
    let Some(method) = object.get("method").and_then(Value::as_str) else {
        return Incoming::Malformed(Response::error(
            id.unwrap_or(Value::Null),
            INVALID_REQUEST,
            "A protocol message must name a method.",
        ));
    };
    match id {
        Some(id) => Incoming::Call(Request {
            id,
            method: method.to_owned(),
            params: object.get("params").cloned().unwrap_or(Value::Null),
        }),
        None => Incoming::Notification,
    }
}

/// Pick the protocol version: echo a supported client version or the latest.
pub(crate) fn negotiate_version(requested: Option<&str>) -> &'static str {
    requested
        .and_then(|requested| {
            SUPPORTED_VERSIONS
                .iter()
                .copied()
                .find(|supported| *supported == requested)
        })
        .unwrap_or(LATEST_VERSION)
}

/// Answer the initialize handshake with capabilities and server identity.
pub(crate) fn initialize(id: Value, requested_version: Option<&str>) -> Response {
    Response::result(
        id,
        json!({
            "protocolVersion": negotiate_version(requested_version),
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "iwe", "version": env!("CARGO_PKG_VERSION") },
            "instructions": "IWE Code exposes the projects of its workspace. Browse and read project files, search file names, and read Git status and history. write_file replaces existing text files and requires the revision token returned by read_file.",
        }),
    )
}
