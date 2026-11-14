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
