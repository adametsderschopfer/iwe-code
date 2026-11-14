//! End-to-end checks for the command line against isolated storage.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn run(arguments: &[&str], data_dir: &Path) -> (i32, String, String) {
    let output = spawn(arguments, data_dir);
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

fn spawn(arguments: &[&str], data_dir: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_iwe"))
        .args(arguments)
        .env("IWE_DATA_DIR", data_dir)
        .env("IWE_LANG", "en")
        .output()
        .unwrap()
}

fn run_with_stdin(arguments: &[&str], data_dir: &Path, input: &str) -> (i32, String, String) {
    use std::io::Write as _;
    let mut child = Command::new(env!("CARGO_BIN_EXE_iwe"))
        .args(arguments)
        .env("IWE_DATA_DIR", data_dir)
        .env("IWE_LANG", "en")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .as_mut()
        .expect("piped stdin")
        .write_all(input.as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

fn project_directory(root: &Path) -> PathBuf {
    let directory = root.join("repo");
    std::fs::create_dir_all(&directory).unwrap();
    directory
}

#[test]
fn version_prints_the_application_version() {
    let data = tempfile::tempdir().unwrap();
    let (code, stdout, _) = run(&["--version"], data.path());
    assert_eq!(code, 0);
    assert!(stdout.starts_with("iwe "));
}

#[test]
fn help_lists_the_commands_in_english() {
    let data = tempfile::tempdir().unwrap();
    let (code, stdout, _) = run(&["--help"], data.path());
    assert_eq!(code, 0);
    assert!(stdout.contains("Usage:"));
    assert!(stdout.contains("open"));
    assert!(stdout.contains("workspace"));
}

#[test]
fn list_prints_an_empty_workspace_as_json() {
    let data = tempfile::tempdir().unwrap();
    let (code, stdout, _) = run(&["workspace", "list", "--json"], data.path());
    assert_eq!(code, 0);
    let value: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(value["spaces"], serde_json::json!([]));
}

#[test]
fn add_project_creates_a_space_and_list_shows_the_project() {
    let data = tempfile::tempdir().unwrap();
    let directory = project_directory(data.path());
    let expected = std::fs::canonicalize(&directory).unwrap();

    let (code, stdout, _) = run(
        &["workspace", "add-project", directory.to_str().unwrap()],
        data.path(),
    );
    assert_eq!(code, 0);
    assert!(stdout.contains("Added repo to the workspace."));

    let (code, stdout, _) = run(&["workspace", "list", "--json"], data.path());
    assert_eq!(code, 0);
    let value: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(value["spaces"][0]["name"], "repo");
    assert_eq!(value["spaces"][0]["projects"][0]["name"], "repo");
    assert_eq!(
        value["spaces"][0]["projects"][0]["directory"],
        expected.to_str().unwrap()
    );
    assert_eq!(value["spaces"][0]["projects"][0]["pinned"], false);
}

#[test]
fn adding_the_same_project_twice_reports_it_as_present() {
    let data = tempfile::tempdir().unwrap();
    let directory = project_directory(data.path());
    let path = directory.to_str().unwrap();
    let (code, _, _) = run(&["workspace", "add-project", path], data.path());
    assert_eq!(code, 0);
    let (code, stdout, _) = run(&["workspace", "add-project", path], data.path());
    assert_eq!(code, 0);
    assert!(stdout.contains("is already in the workspace."));
}

#[test]
fn add_project_reports_missing_directories_and_spaces() {
    let data = tempfile::tempdir().unwrap();
    let missing = data.path().join("missing");
    let (code, _, stderr) = run(
        &["workspace", "add-project", missing.to_str().unwrap()],
        data.path(),
    );
    assert_eq!(code, 1);
    assert!(stderr.contains("does not exist"));

    let directory = project_directory(data.path());
    let (code, _, _) = run(
        &["workspace", "add-project", directory.to_str().unwrap()],
        data.path(),
    );
    assert_eq!(code, 0);
    let second = data.path().join("second");
    std::fs::create_dir_all(&second).unwrap();
    let (code, _, stderr) = run(
        &[
            "workspace",
            "add-project",
            second.to_str().unwrap(),
            "--space",
            "Absent",
        ],
        data.path(),
    );
    assert_eq!(code, 1);
    assert!(stderr.contains("No space is named Absent."));
}

#[test]
fn remove_project_supports_directory_and_name_and_reports_unknown_targets() {
    let data = tempfile::tempdir().unwrap();
    let directory = project_directory(data.path());
    let path = directory.to_str().unwrap();
    let (code, _, _) = run(&["workspace", "add-project", path], data.path());
    assert_eq!(code, 0);

    let (code, stdout, _) = run(&["workspace", "remove-project", "repo"], data.path());
    assert_eq!(code, 0);
    assert!(stdout.contains("Removed repo from the workspace."));

    let (code, stdout, _) = run(&["workspace", "list", "--json"], data.path());
    assert_eq!(code, 0);
    let value: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(value["spaces"][0]["projects"], serde_json::json!([]));

    let (code, _, stderr) = run(&["workspace", "remove-project", "absent"], data.path());
    assert_eq!(code, 1);
    assert!(stderr.contains("No project matches absent."));
}

#[test]
fn human_list_prints_spaces_and_projects() {
    let data = tempfile::tempdir().unwrap();
    let (code, stdout, _) = run(&["workspace", "list"], data.path());
    assert_eq!(code, 0);
    assert!(stdout.contains("No projects yet."));

    let directory = project_directory(data.path());
    let (code, _, _) = run(
        &["workspace", "add-project", directory.to_str().unwrap()],
        data.path(),
    );
    assert_eq!(code, 0);
    let (code, stdout, _) = run(&["workspace", "list"], data.path());
    assert_eq!(code, 0);
    assert!(stdout.contains("Space: repo"));
    assert!(stdout.contains("repo:"));
}

#[test]
fn open_and_unknown_commands_report_usage_errors() {
    let data = tempfile::tempdir().unwrap();
    let missing = data.path().join("missing");
    let (code, _, stderr) = run(&["open", missing.to_str().unwrap()], data.path());
    assert_eq!(code, 2);
    assert!(stderr.contains("does not exist"));

    let (code, _, stderr) = run(&["nonsense"], data.path());
    assert_eq!(code, 2);
    assert!(stderr.contains("Unknown command nonsense."));

    let (code, _, stderr) = run(&["workspace"], data.path());
    assert_eq!(code, 2);
    assert!(stderr.contains("Usage:"));
}

fn deployment_workspace(root: &Path) -> PathBuf {
    let directory = project_directory(root);
    let path = directory.to_str().unwrap();
    std::fs::write(
        root.join("workspace.json"),
        format!(
            r#"{{
    "spaces": [
        {{
            "name": "Work",
            "projects": [
                {{"name": "repo", "directory": "{}", "pinned": false}}
            ],
            "deployments": [
                {{
                    "id": "p1",
                    "name": "production",
                    "project": "{}",
                    "connection": "c1",
                    "root": "/var/www",
                    "mappings": [{{"local": "web", "remote": "assets"}}],
                    "excluded": ["web/uploads"],
                    "default": true
                }}
            ]
        }}
    ],
    "selected_space": 0,
    "selected_project": 0,
    "deployment": {{
        "connections": [
            {{
                "id": "c1",
                "name": "web",
                "protocol": "Sftp",
                "host": "example.com",
                "port": 22,
                "username": "deploy",
                "authentication": "Password"
            }}
        ],
        "keys": []
    }}
}}"#,
            path, path,
        ),
    )
    .unwrap();
    root.to_path_buf()
}

#[test]
fn deployment_list_prints_settings_and_profiles_as_json() {
    let data = tempfile::tempdir().unwrap();
    let (code, stdout, _) = run(&["deployment", "list"], data.path());
    assert_eq!(code, 0);
    assert!(stdout.contains("No deployment connections, keys, or profiles yet."));

    let workspace = deployment_workspace(data.path());
    let (code, stdout, _) = run(&["deployment", "list", "--json"], &workspace);
    assert_eq!(code, 0);
    let value: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(value["connections"][0]["name"], "web");
    assert_eq!(value["connections"][0]["protocol"], "sftp");
    assert_eq!(value["connections"][0]["port"], 22);
    assert_eq!(value["keys"], serde_json::json!([]));
    assert_eq!(value["spaces"][0]["profiles"][0]["name"], "production");
    assert_eq!(value["spaces"][0]["profiles"][0]["connection_name"], "web");
    assert_eq!(value["spaces"][0]["profiles"][0]["default"], true);
    assert_eq!(
        value["spaces"][0]["profiles"][0]["excluded"],
        serde_json::json!(["web/uploads"])
    );
}

#[test]
fn deployment_connection_add_saves_a_secret_and_remove_deletes_it() {
    let data = tempfile::tempdir().unwrap();
    let (code, stdout, _) = run_with_stdin(
        &[
            "deployment",
            "connection",
            "add",
            "web",
            "--host",
            "example.com",
            "--username",
            "deploy",
            "--password-stdin",
        ],
        data.path(),
        "secret123\n",
    );
    assert_eq!(code, 0);
    assert!(stdout.contains("Added the connection web."));
    let secrets = data.path().join("secrets");
    assert_eq!(secrets.read_dir().unwrap().count(), 1);

    let (code, stdout, _) = run(&["deployment", "list", "--json"], data.path());
    assert_eq!(code, 0);
    let value: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(value["connections"][0]["name"], "web");
    assert_eq!(value["connections"][0]["authentication"], "password");
    assert_eq!(value["connections"][0]["key"], serde_json::Value::Null);

    let (code, stdout, _) = run(
        &["deployment", "connection", "remove", "web", "--json"],
        data.path(),
    );
    assert_eq!(code, 0);
    let value: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(value["removed"], "web");
    assert_eq!(secrets.read_dir().unwrap().count(), 0);

    let (code, stdout, _) = run(&["deployment", "list", "--json"], data.path());
    assert_eq!(code, 0);
    let value: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(value["connections"], serde_json::json!([]));
}

#[test]
fn deployment_connection_remove_reports_connections_in_use() {
    let data = tempfile::tempdir().unwrap();
    let workspace = deployment_workspace(data.path());
    let (code, _, stderr) = run(&["deployment", "connection", "remove", "web"], &workspace);
    assert_eq!(code, 1);
    assert!(stderr.contains("is used by a deployment profile."));
}

#[test]
fn deployment_key_add_registers_and_removes_key_files() {
    let data = tempfile::tempdir().unwrap();
    let key = data.path().join("id_ed25519");
    std::fs::write(&key, "key").unwrap();
    let (code, stdout, _) = run(
        &[
            "deployment",
            "key",
            "add",
            "staging",
            key.to_str().unwrap(),
            "--json",
        ],
        data.path(),
    );
    assert_eq!(code, 0);
    let value: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(value["name"], "staging");
    assert!(value["id"].as_str().is_some_and(|id| id.len() == 32));
    assert!(
        value["path"]
            .as_str()
            .is_some_and(|path| path.ends_with("id_ed25519"))
    );

    let (code, _, stderr) = run(
        &[
            "deployment",
            "key",
            "add",
            "broken",
            data.path().join("absent").to_str().unwrap(),
        ],
        data.path(),
    );
    assert_eq!(code, 1);
    assert!(stderr.contains("The path does not exist"));

    let (code, stdout, _) = run(
        &["deployment", "key", "remove", "staging", "--json"],
        data.path(),
    );
    assert_eq!(code, 0);
    let value: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(value["removed"], "staging");
}

#[test]
fn deployment_profile_add_validates_values_and_resolves_connections() {
    let data = tempfile::tempdir().unwrap();
    let directory = project_directory(data.path());
    let (code, _, _) = run(
        &["workspace", "add-project", directory.to_str().unwrap()],
        data.path(),
    );
    assert_eq!(code, 0);
    let (code, _, _) = run(
        &[
            "deployment",
            "connection",
            "add",
            "web",
            "--host",
            "example.com",
            "--username",
            "deploy",
        ],
        data.path(),
    );
    assert_eq!(code, 0);

    let (code, _, stderr) = run(
        &[
            "deployment",
            "profile",
            "add",
            "production",
            "--project",
            directory.to_str().unwrap(),
            "--connection",
            "absent",
            "--root",
            "/var/www",
            "--mapping",
            "web=assets",
        ],
        data.path(),
    );
    assert_eq!(code, 1);
    assert!(stderr.contains("No connection matches absent."));

    let (code, _, stderr) = run(
        &[
            "deployment",
            "profile",
            "add",
            "production",
            "--project",
            directory.to_str().unwrap(),
            "--connection",
            "web",
            "--root",
            "/var/www",
            "--mapping",
            "broken",
        ],
        data.path(),
    );
    assert_eq!(code, 2);
    assert!(stderr.contains("Use the form local=remote."));

    let (code, stdout, _) = run(
        &[
            "deployment",
            "profile",
            "add",
            "production",
            "--project",
            directory.to_str().unwrap(),
            "--connection",
            "web",
            "--root",
            "/var/www",
            "--mapping",
            "web=assets",
            "--json",
        ],
        data.path(),
    );
    assert_eq!(code, 0);
    let value: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(value["name"], "production");
    assert_eq!(value["connection_name"], "web");
    assert_eq!(value["root"], "/var/www");

    let (code, _, stderr) = run(&["deployment", "profile", "remove", "absent"], data.path());
    assert_eq!(code, 1);
    assert!(stderr.contains("No profile matches absent."));
}

#[test]
fn deployment_profile_remove_reports_the_removed_profile() {
    let data = tempfile::tempdir().unwrap();
    let workspace = deployment_workspace(data.path());
    let (code, stdout, _) = run(
        &["deployment", "profile", "remove", "production", "--json"],
        &workspace,
    );
    assert_eq!(code, 0);
    let value: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(value["removed"], "production");

    let (code, _, _) = run(
        &["deployment", "connection", "remove", "web", "--json"],
        &workspace,
    );
    assert_eq!(code, 0);
}
