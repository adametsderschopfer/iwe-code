//! Checks for protocol parsing, version negotiation, and line transport.

use super::{Line, read_line};
use crate::infrastructure::mcp::protocol::{self, Incoming, Response, negotiate_version, parse};
use serde_json::{Value, json};
use tokio::io::BufReader;

fn call_response(value: Value) -> Option<Value> {
    value.get("result").cloned()
}

fn error_code(line: &str) -> i64 {
    let value: Value = serde_json::from_str(line).expect("valid json");
    value
        .get("error")
        .and_then(|error| error.get("code"))
        .and_then(Value::as_i64)
        .unwrap_or_default()
}

#[test]
fn parses_a_call_with_params() {
    let line = r#"{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"read_file"}}"#;
    let Incoming::Call(request) = parse(line) else {
        panic!("expected a call");
    };
    assert_eq!(request.id, json!(7));
    assert_eq!(request.method, "tools/call");
    assert_eq!(request.params, json!({"name": "read_file"}));
}

#[test]
fn parses_a_call_without_params() {
    let line = r#"{"jsonrpc":"2.0","id":"a","method":"ping"}"#;
    let Incoming::Call(request) = parse(line) else {
        panic!("expected a call");
    };
    assert_eq!(request.id, json!("a"));
    assert_eq!(request.params, Value::Null);
}

#[test]
fn parses_a_notification_without_an_id() {
    let line = r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#;
    assert!(matches!(parse(line), Incoming::Notification));
}

#[test]
fn rejects_invalid_json_with_a_parse_error() {
    let Incoming::Malformed(response) = parse("{not json") else {
        panic!("expected a rejection");
    };
    let Response::Error { id, code, message } = response else {
        panic!("expected an error response");
    };
    assert_eq!(id, Value::Null);
    assert_eq!(code, -32700);
    assert!(!message.is_empty());
}

#[test]
fn rejects_non_object_messages() {
    let Incoming::Malformed(response) = parse("[1, 2]") else {
        panic!("expected a rejection");
    };
    assert_eq!(error_code(&response.encode()), -32600);
}

#[test]
fn rejects_messages_without_a_method() {
    let Incoming::Malformed(response) = parse(r#"{"jsonrpc":"2.0","id":1}"#) else {
        panic!("expected a rejection");
    };
    assert_eq!(error_code(&response.encode()), -32600);
}

#[test]
fn method_not_found_uses_the_rpc_code() {
    let Response::Error { code, .. } = Response::method_not_found(json!(1)) else {
        panic!("expected an error response");
    };
    assert_eq!(code, -32601);
}

#[test]
fn encodes_results_as_one_line() {
    let line = Response::result(json!(3), json!({"answer": 42})).encode();
    assert!(line.ends_with('\n'));
    assert_eq!(line.matches('\n').count(), 1);
    let value: Value = serde_json::from_str(line.trim_end()).expect("valid json");
    assert_eq!(value.get("jsonrpc"), Some(&json!("2.0")));
    assert_eq!(value.get("id"), Some(&json!(3)));
    assert_eq!(call_response(value), Some(json!({"answer": 42})));
}

#[test]
fn encodes_errors_as_one_line() {
    let line = Response::invalid_params(json!(2), "bad input").encode();
    let value: Value = serde_json::from_str(line.trim_end()).expect("valid json");
    let error = value.get("error").expect("error object");
    assert_eq!(error.get("code"), Some(&json!(-32602)));
    assert_eq!(error.get("message"), Some(&json!("bad input")));
}

#[test]
fn echoes_supported_versions() {
    assert_eq!(negotiate_version(Some("2024-11-05")), "2024-11-05");
    assert_eq!(negotiate_version(Some("2025-03-26")), "2025-03-26");
    assert_eq!(negotiate_version(Some("2025-06-18")), "2025-06-18");
}

#[test]
fn falls_back_to_the_latest_version() {
    assert_eq!(negotiate_version(None), "2025-06-18");
    assert_eq!(negotiate_version(Some("1999-01-01")), "2025-06-18");
}

#[test]
fn initialize_reports_capabilities_and_identity() {
    let Response::Result { value, .. } = protocol::initialize(json!(1), Some("2024-11-05")) else {
        panic!("expected a result response");
    };
    assert_eq!(value.get("protocolVersion"), Some(&json!("2024-11-05")));
    assert!(
        value
            .get("capabilities")
            .and_then(|capabilities| capabilities.get("tools"))
            .is_some()
    );
    let info = value.get("serverInfo").expect("server info");
    assert_eq!(info.get("name"), Some(&json!("iwe")));
    assert_eq!(info.get("version"), Some(&json!(env!("CARGO_PKG_VERSION"))));
}

#[tokio::test]
async fn reads_complete_lines_one_by_one() {
    let mut input = BufReader::new(&b"one\ntwo\n"[..]);
    assert_eq!(
        read_line(&mut input).await.expect("readable"),
        Some(Line::Text("one".to_owned()))
    );
    assert_eq!(
        read_line(&mut input).await.expect("readable"),
        Some(Line::Text("two".to_owned()))
    );
    assert_eq!(read_line(&mut input).await.expect("readable"), None);
}

#[tokio::test]
async fn reads_an_empty_line_and_a_final_partial_line() {
    let mut input = BufReader::new(&b"\nlast"[..]);
    assert_eq!(
        read_line(&mut input).await.expect("readable"),
        Some(Line::Text(String::new()))
    );
    assert_eq!(
        read_line(&mut input).await.expect("readable"),
        Some(Line::Text("last".to_owned()))
    );
    assert_eq!(read_line(&mut input).await.expect("readable"), None);
}

#[tokio::test]
async fn discards_lines_above_the_cap_and_keeps_reading() {
    let long = "x".repeat(protocol::MAX_LINE + 1);
    let input = format!("{long}\nafter\n");
    let mut input = BufReader::new(input.as_bytes());
    assert_eq!(
        read_line(&mut input).await.expect("readable"),
        Some(Line::TooLong)
    );
    assert_eq!(
        read_line(&mut input).await.expect("readable"),
        Some(Line::Text("after".to_owned()))
    );
}

#[test]
fn requested_matches_only_the_server_flag() {
    // The check runs on the real process arguments; this process has none of them.
    assert!(!super::requested());
}
