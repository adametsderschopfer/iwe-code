//! Checks for the tools/call envelope that stay off the filesystem.

use super::{Server, call};
use crate::infrastructure::mcp::{Handler, Request, Response};
use serde_json::{Value, json};

fn request(method: &str, params: Value) -> Request {
    Request {
        id: json!(1),
        method: method.to_owned(),
        params,
    }
}

#[tokio::test]
async fn rejects_calls_without_a_tool_name() {
    let Response::Error { code, .. } = call(request("tools/call", json!({}))).await else {
        panic!("expected an error response");
    };
    assert_eq!(code, -32602);
}

#[tokio::test]
async fn rejects_calls_with_non_object_arguments() {
    let response = call(request(
        "tools/call",
        json!({"name": "list_projects", "arguments": "nope"}),
    ))
    .await;
    let Response::Result { value, .. } = response else {
        panic!("expected a tool result");
    };
    assert_eq!(value.get("isError"), Some(&json!(true)));
    let content = value
        .get("content")
        .and_then(Value::as_array)
        .expect("content");
    assert_eq!(
        content[0].get("text"),
        Some(&json!("Tool arguments must be an object."))
    );
}

#[tokio::test]
async fn reports_unknown_tools_as_tool_errors() {
    let response = call(request(
        "tools/call",
        json!({"name": "no_such_tool", "arguments": {}}),
    ))
    .await;
    let Response::Result { value, .. } = response else {
        panic!("expected a tool result");
    };
    assert_eq!(value.get("isError"), Some(&json!(true)));
    let text = value["content"][0]["text"].as_str().expect("text");
    assert!(text.starts_with("Unknown tool 'no_such_tool'."), "{text}");
}

#[tokio::test]
async fn lists_seven_tools_with_unique_names() {
    let Response::Result { value, .. } = Server.handle(request("tools/list", json!({}))).await
    else {
        panic!("expected a result response");
    };
    let tools = value.get("tools").and_then(Value::as_array).expect("tools");
    assert_eq!(tools.len(), 7);
    let mut names: Vec<_> = tools
        .iter()
        .filter_map(|tool| tool.get("name").and_then(Value::as_str))
        .collect();
    let expected = [
        "list_projects",
        "list_files",
        "read_file",
        "write_file",
        "search_files",
        "git_status",
        "git_log",
    ];
    assert_eq!(names, expected);
    names.sort();
    names.dedup();
    assert_eq!(names.len(), expected.len());
}

#[tokio::test]
async fn every_tool_describes_an_object_schema() {
    let Response::Result { value, .. } = Server.handle(request("tools/list", json!({}))).await
    else {
        panic!("expected a result response");
    };
    for tool in value["tools"].as_array().expect("tools") {
        assert_eq!(tool["inputSchema"]["type"], json!("object"));
        assert!(
            tool.get("description")
                .and_then(Value::as_str)
                .is_some_and(|text| !text.is_empty()),
            "{tool}"
        );
    }
}

#[tokio::test]
async fn unknown_methods_report_method_not_found() {
    let Response::Error { code, .. } = Server.handle(request("resources/list", json!({}))).await
    else {
        panic!("expected an error response");
    };
    assert_eq!(code, -32601);
}
