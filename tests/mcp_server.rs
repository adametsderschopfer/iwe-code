//! End-to-end checks for the MCP server over standard input and output.
//!
//! Every test runs the server binary against an isolated temporary data
//! directory. The user's real workspace metadata is never touched.

use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Write},
    path::Path,
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
};

/// A running MCP server fed by the test.
struct Server {
    child: Child,
    input: Option<ChildStdin>,
    output: BufReader<ChildStdout>,
    next_id: u64,
}

impl Server {
    fn start(data_dir: &Path) -> Server {
        let mut child = Command::new(env!("CARGO_BIN_EXE_iwe"))
            .arg("--mcp")
            .env("IWE_DATA_DIR", data_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("start the MCP server");
        let input = child.stdin.take().expect("piped stdin");
        let output = BufReader::new(child.stdout.take().expect("piped stdout"));
        Server {
            child,
            input: Some(input),
            output,
            next_id: 0,
        }
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        self.next_id += 1;
        let id = self.next_id;
        self.send(&json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}));
        let response = self.receive();
        assert_eq!(response.get("id"), Some(&json!(id)), "{response}");
        response
    }

    fn send(&mut self, value: &Value) {
        let input = self.input.as_mut().expect("open stdin");
        writeln!(input, "{value}").expect("write a protocol line");
        input.flush().expect("flush stdin");
    }

    /// Close the input stream so the server sees a clean end of file.
    fn close(&mut self) {
        drop(self.input.take());
    }

    fn receive(&mut self) -> Value {
        let mut line = String::new();
        self.output
            .read_line(&mut line)
            .expect("read a protocol line");
        serde_json::from_str(line.trim_end()).expect("a JSON response")
    }

    /// Call a tool and return its result object.
    fn tool(&mut self, name: &str, arguments: Value) -> Value {
        let response = self.request("tools/call", json!({"name": name, "arguments": arguments}));
        response.get("result").expect("a tool result").clone()
    }

    /// Call a tool that must succeed and return its parsed JSON payload.
    fn tool_value(&mut self, name: &str, arguments: Value) -> Value {
        let result = self.tool(name, arguments);
        assert_eq!(result.get("isError"), Some(&json!(false)), "{result}");
        let text = result["content"][0]["text"].as_str().expect("text content");
        serde_json::from_str(text).expect("a JSON tool payload")
    }

    /// Call a tool that must fail and return its error message.
    fn tool_error(&mut self, name: &str, arguments: Value) -> String {
        let result = self.tool(name, arguments);
        assert_eq!(result.get("isError"), Some(&json!(true)), "{result}");
        result["content"][0]["text"]
            .as_str()
            .expect("text content")
            .to_owned()
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Create an isolated data directory with a workspace that owns one Git project.
fn fixture() -> (tempfile::TempDir, tempfile::TempDir) {
    let data = tempfile::tempdir().expect("data directory");
    let project = tempfile::tempdir().expect("project directory");
    std::fs::write(project.path().join("readme.md"), "hello workspace\n")
        .expect("write a project file");
    std::fs::create_dir_all(project.path().join("src")).expect("create a project directory");
    std::fs::write(project.path().join("src/main.rs"), "fn main() {}\n")
        .expect("write a project file");

    git(project.path(), &["init"]);
    git(project.path(), &["add", "-A"]);
    git(
        project.path(),
        &["commit", "--allow-empty", "--message", "Initial commit"],
    );

    let workspace = json!({
        "spaces": [{
            "name": "Main",
            "projects": [{
                "name": "sample",
                "directory": project.path().display().to_string(),
                "pinned": false,
            }],
        }],
        "selected_space": 0,
        "selected_project": 0,
        "preferences": {},
    });
    std::fs::write(data.path().join("workspace.json"), workspace.to_string())
        .expect("write workspace metadata");
    (data, project)
}

fn git(project: &Path, arguments: &[&str]) {
    let status = Command::new("git")
        .args([
            "--no-pager",
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.com",
        ])
        .args(arguments)
        .current_dir(project)
        .status()
        .expect("run git");
    assert!(status.success(), "git {arguments:?} failed");
}

/// Create an isolated data directory whose workspace owns one deployment profile.
fn deployment_fixture() -> (tempfile::TempDir, tempfile::TempDir) {
    let (data, project) = fixture();
    let directory = project.path().display().to_string();
    let workspace = json!({
        "spaces": [{
            "name": "Main",
            "projects": [{
                "name": "sample",
                "directory": directory,
                "pinned": false,
            }],
            "deployments": [{
                "id": "p1",
                "name": "production",
                "project": directory,
                "connection": "c1",
                "root": "/var/www",
                "mappings": [
                    {"local": "src", "remote": "code"},
                    {"local": "readme.md", "remote": "readme.md"},
                ],
                "excluded": ["src/tmp"],
                "default": true,
            }],
        }],
        "selected_space": 0,
        "selected_project": 0,
        "deployment": {
            "connections": [{
                "id": "c1",
                "name": "web",
                "protocol": "Sftp",
                "host": "example.com",
                "port": 22,
                "username": "deploy",
                "authentication": "Password",
            }],
            "keys": [],
        },
    });
    std::fs::write(data.path().join("workspace.json"), workspace.to_string())
        .expect("write workspace metadata");
    (data, project)
}

#[test]
fn lists_deployments_without_secrets() {
    let (data, _project) = deployment_fixture();
    let mut server = Server::start(data.path());
    server.request("initialize", json!({"protocolVersion": "2025-06-18"}));
    server.send(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));

    let value = server.tool_value("list_deployments", json!({}));
    let text = serde_json::to_string(&value).unwrap();
    assert!(!text.contains("secret"), "{text}");
    assert_eq!(value["connections"][0]["name"], "web");
    assert_eq!(value["connections"][0]["protocol"], "sftp");
    assert_eq!(value["connections"][0]["authentication"], "password");
    assert_eq!(value["keys"], serde_json::json!([]));
    assert_eq!(value["spaces"][0]["profiles"][0]["name"], "production");
    assert_eq!(value["spaces"][0]["profiles"][0]["connection"], "web");
    assert_eq!(value["spaces"][0]["profiles"][0]["root"], "/var/www");
    assert_eq!(
        value["spaces"][0]["profiles"][0]["mappings"][0],
        serde_json::json!({"local": "src", "remote": "code"})
    );
    assert_eq!(
        server.tool_value("list_deployments", json!({}))["spaces"][0]["profiles"][0]["default"],
        json!(true)
    );
}

#[test]
fn resolves_deployment_paths_across_mappings() {
    let (data, _project) = deployment_fixture();
    let mut server = Server::start(data.path());
    server.request("initialize", json!({"protocolVersion": "2025-06-18"}));
    server.send(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));

    let value = server.tool_value(
        "resolve_deployment_path",
        json!({"profile": "production", "local": "src/main.rs"}),
    );
    assert_eq!(value["profile"], "production");
    assert_eq!(value["remote"], "/var/www/code/main.rs");

    let value = server.tool_value(
        "resolve_deployment_path",
        json!({"profile": "production", "remote": "/var/www/readme.md"}),
    );
    assert_eq!(value["local"], "readme.md");

    let error = server.tool_error(
        "resolve_deployment_path",
        json!({"profile": "production", "local": "absent.txt"}),
    );
    assert!(error.contains("outside the profile mappings"), "{error}");

    let error = server.tool_error(
        "resolve_deployment_path",
        json!({"profile": "absent", "local": "src/main.rs"}),
    );
    assert!(
        error.contains("No deployment profile matches 'absent'."),
        "{error}"
    );

    let error = server.tool_error("resolve_deployment_path", json!({"profile": "production"}));
    assert!(
        error.contains("Provide the 'local' or the 'remote'"),
        "{error}"
    );

    let error = server.tool_error(
        "resolve_deployment_path",
        json!({"profile": "production", "local": "src/main.rs", "remote": "/var/www/code"}),
    );
    assert!(error.contains("not both"), "{error}");
}

/// Create an isolated data directory whose profile points at a closed loopback port.
fn offline_deployment_fixture() -> tempfile::TempDir {
    let (data, project) = fixture();
    let directory = project.path().display().to_string();
    let workspace = json!({
        "spaces": [{
            "name": "Main",
            "projects": [{"name": "sample", "directory": directory, "pinned": false}],
            "deployments": [{
                "id": "p1",
                "name": "production",
                "project": directory,
                "connection": "c1",
                "root": "/var/www",
                "mappings": [{"local": "src", "remote": "code"}],
                "excluded": [],
                "default": true,
            }],
        }],
        "selected_space": 0,
        "selected_project": 0,
        "deployment": {
            "connections": [{
                "id": "c1",
                "name": "web",
                "protocol": "Sftp",
                "host": "127.0.0.1",
                "port": 1,
                "username": "deploy",
                "authentication": "Password",
            }],
            "keys": [],
        },
    });
    std::fs::write(data.path().join("workspace.json"), workspace.to_string())
        .expect("write workspace metadata");
    data
}

#[test]
fn deployment_reads_report_unknown_profiles_and_unreachable_servers() {
    let data = offline_deployment_fixture();
    let mut server = Server::start(data.path());
    server.request("initialize", json!({"protocolVersion": "2025-06-18"}));
    server.send(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));

    let error = server.tool_error(
        "deployment_read_remote",
        json!({"profile": "absent", "path": "/var/www/readme.md"}),
    );
    assert!(
        error.contains("No deployment profile matches 'absent'."),
        "{error}"
    );

    let error = server.tool_error(
        "deployment_read_remote",
        json!({"profile": "production", "path": "/var/www/readme.md"}),
    );
    assert!(error.contains("IWE Code could not connect"), "{error}");

    let error = server.tool_error(
        "deployment_compare",
        json!({"profile": "production", "path": "src/main.rs"}),
    );
    assert!(error.contains("IWE Code could not connect"), "{error}");
}

#[test]
fn completes_the_handshake_and_lists_tools() {
    let (data, _project) = fixture();
    let mut server = Server::start(data.path());

    let response = server.request("initialize", json!({"protocolVersion": "2025-06-18"}));
    let result = response.get("result").expect("an initialize result");
    assert_eq!(result.get("protocolVersion"), Some(&json!("2025-06-18")));
    assert!(result["capabilities"].get("tools").is_some());
    assert_eq!(result["serverInfo"].get("name"), Some(&json!("iwe")));
    assert!(
        result["serverInfo"]["version"]
            .as_str()
            .is_some_and(|version| !version.is_empty())
    );

    // Notifications never produce a response line.
    server.send(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));
    let response = server.request("tools/list", json!({}));
    let tools = response["result"]["tools"].as_array().expect("tools");
    let names: Vec<_> = tools
        .iter()
        .filter_map(|tool| tool.get("name").and_then(Value::as_str))
        .collect();
    assert_eq!(
        names,
        [
            "list_projects",
            "list_files",
            "read_file",
            "write_file",
            "search_files",
            "git_status",
            "git_log",
            "list_deployments",
            "resolve_deployment_path",
            "deployment_read_remote",
            "deployment_compare",
        ]
    );
}

#[test]
fn reads_the_workspace_and_project_files() {
    let (data, _project) = fixture();
    let mut server = Server::start(data.path());
    server.request("initialize", json!({"protocolVersion": "2025-03-26"}));
    server.send(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));

    let projects = server.tool_value("list_projects", json!({}));
    let spaces = projects
        .get("spaces")
        .and_then(Value::as_array)
        .expect("spaces");
    assert_eq!(spaces.len(), 1);
    assert_eq!(spaces[0].get("name"), Some(&json!("Main")));
    assert_eq!(spaces[0]["projects"][0].get("name"), Some(&json!("sample")));
    assert_eq!(spaces[0]["projects"][0].get("selected"), Some(&json!(true)));

    let files = server.tool_value("list_files", json!({}));
    let entries = files
        .get("entries")
        .and_then(Value::as_array)
        .expect("entries");
    let names: Vec<_> = entries
        .iter()
        .filter_map(|entry| entry.get("name").and_then(Value::as_str))
        .collect();
    assert!(names.contains(&"readme.md"), "{names:?}");
    assert!(names.contains(&"src"), "{names:?}");

    let nested = server.tool_value("list_files", json!({"directory": "src"}));
    let nested_names: Vec<_> = nested["entries"]
        .as_array()
        .expect("entries")
        .iter()
        .filter_map(|entry| entry.get("name").and_then(Value::as_str))
        .collect();
    assert_eq!(nested_names, ["main.rs"]);

    let file = server.tool_value("read_file", json!({"path": "readme.md"}));
    assert_eq!(file.get("content"), Some(&json!("hello workspace\n")));
    let revision = file
        .get("revision")
        .and_then(Value::as_str)
        .expect("revision");
    assert_eq!(revision.len(), 64);
}

#[test]
fn writes_files_with_revision_checks() {
    let (data, project) = fixture();
    let mut server = Server::start(data.path());
    server.request("initialize", json!({"protocolVersion": "2025-06-18"}));
    server.send(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));

    let file = server.tool_value("read_file", json!({"path": "readme.md"}));
    let revision = file
        .get("revision")
        .and_then(Value::as_str)
        .expect("revision");

    // A stale revision must not overwrite the file.
    let message = server.tool_error(
        "write_file",
        json!({"path": "readme.md", "content": "wrong\n", "revision": &"a".repeat(64)}),
    );
    assert!(message.contains("changed after it was read"), "{message}");
    assert_eq!(
        std::fs::read_to_string(project.path().join("readme.md")).expect("file"),
        "hello workspace\n"
    );

    let written = server.tool_value(
        "write_file",
        json!({"path": "readme.md", "content": "replaced\n", "revision": revision}),
    );
    assert_eq!(
        std::fs::read_to_string(project.path().join("readme.md")).expect("file"),
        "replaced\n"
    );
    let next = written
        .get("revision")
        .and_then(Value::as_str)
        .expect("revision");
    assert_ne!(next, revision);

    // The returned token allows chaining another write without a fresh read.
    server.tool_value(
        "write_file",
        json!({"path": "readme.md", "content": "chained\n", "revision": next}),
    );
    assert_eq!(
        std::fs::read_to_string(project.path().join("readme.md")).expect("file"),
        "chained\n"
    );
}

#[test]
fn searches_files_and_reports_git_state() {
    let (data, project) = fixture();
    std::fs::write(project.path().join("todo.txt"), "untracked\n").expect("write");
    let mut server = Server::start(data.path());
    server.request("initialize", json!({"protocolVersion": "2025-06-18"}));
    server.send(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));

    let search = server.tool_value("search_files", json!({"query": "readme"}));
    let results = search
        .get("results")
        .and_then(Value::as_array)
        .expect("results");
    assert_eq!(results[0].get("name"), Some(&json!("readme.md")));
    assert_eq!(results[0].get("path"), Some(&json!("readme.md")));

    let status = server.tool_value("git_status", json!({}));
    assert!(
        status
            .get("head")
            .and_then(Value::as_str)
            .is_some_and(|head| !head.is_empty())
    );
    let changed = status["changedFiles"].as_array().expect("changed files");
    assert!(
        changed
            .iter()
            .any(|file| file.get("path") == Some(&json!("todo.txt"))),
        "{changed:?}"
    );

    let log = server.tool_value("git_log", json!({"limit": 5}));
    let commits = log
        .get("commits")
        .and_then(Value::as_array)
        .expect("commits");
    assert_eq!(commits[0].get("subject"), Some(&json!("Initial commit")));
    assert!(
        commits[0]
            .get("id")
            .and_then(Value::as_str)
            .is_some_and(|id| id.len() == 40)
    );
}

#[test]
fn rejects_paths_that_escape_the_project() {
    let (data, _project) = fixture();
    let outside = tempfile::tempdir().expect("outside directory");
    std::fs::write(outside.path().join("secret.txt"), "secret\n").expect("write");
    let mut server = Server::start(data.path());
    server.request("initialize", json!({"protocolVersion": "2025-06-18"}));
    server.send(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));

    let message = server.tool_error("read_file", json!({"path": "../secret.txt"}));
    assert!(message.contains("must not contain '..'"), "{message}");

    let message = server.tool_error(
        "write_file",
        json!({
            "path": "../secret.txt",
            "content": "x\n",
            "revision": &"a".repeat(64),
        }),
    );
    assert!(message.contains("must not contain '..'"), "{message}");
}

#[test]
fn reports_unknown_projects_and_methods() {
    let (data, _project) = fixture();
    let mut server = Server::start(data.path());
    server.request("initialize", json!({}));
    server.send(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));

    let message = server.tool_error("list_files", json!({"project": "missing"}));
    assert!(message.contains("Unknown project 'missing'"), "{message}");
    assert!(message.contains("sample"), "{message}");

    let response = server.request("resources/list", json!({}));
    assert_eq!(response["error"].get("code"), Some(&json!(-32601)));
}

#[test]
fn exits_cleanly_when_the_stream_ends() {
    let (data, _project) = fixture();
    let mut server = Server::start(data.path());
    server.request("initialize", json!({"protocolVersion": "2025-06-18"}));
    server.close();
    let status = server.child.wait().expect("wait for the server");
    assert!(status.success(), "clean shutdown expected: {status}");
}
