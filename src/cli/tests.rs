use super::{args, belongs_to_interface, output};
use crate::domain::workspace::Workspace;
use crate::i18n::{I18n, Language};
use clap::error::ErrorKind;
use std::{ffi::OsStr, fs, path::PathBuf, process::ExitCode};

fn english() -> I18n {
    I18n::new(Language::English)
}

#[test]
fn gate_yields_directories_files_links_and_internal_flags() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().to_str().unwrap();
    assert!(belongs_to_interface(OsStr::new(root)));
    let file = directory.path().join("notes.txt");
    fs::write(&file, "text").unwrap();
    assert!(belongs_to_interface(file.as_os_str()));
    assert!(belongs_to_interface(OsStr::new("--iwe-update-cleanup")));
    assert!(belongs_to_interface(OsStr::new(
        "iwe://open?path=/tmp/notes.md"
    )));
    assert!(belongs_to_interface(OsStr::new(
        "iwe://project?path=/tmp/project"
    )));
    assert!(!belongs_to_interface(OsStr::new("workspace")));
    assert!(!belongs_to_interface(OsStr::new("open")));
    assert!(!belongs_to_interface(OsStr::new("--help")));
    assert!(!belongs_to_interface(OsStr::new("--version")));
    assert!(!belongs_to_interface(OsStr::new("nonsense")));
    assert!(!belongs_to_interface(OsStr::new("--json")));
}

#[test]
fn parse_recognizes_open_with_and_without_a_directory() {
    let i18n = english();
    match args::parse_from(&i18n, &["iwe", "open", "/tmp/project"]) {
        Ok(args::Invocation::Open(Some(directory))) => {
            assert_eq!(directory, PathBuf::from("/tmp/project"));
        }
        _ => panic!("open must carry its directory"),
    }
    match args::parse_from(&i18n, &["iwe", "open"]) {
        Ok(args::Invocation::Open(None)) => {}
        _ => panic!("open without a directory must stay optional"),
    }
}

#[test]
fn parse_recognizes_workspace_commands_and_flags() {
    let i18n = english();
    match args::parse_from(&i18n, &["iwe", "workspace", "list", "--json"]) {
        Ok(args::Invocation::List { json }) => assert!(json),
        _ => panic!("list must accept --json"),
    }
    match args::parse_from(&i18n, &["iwe", "workspace", "list"]) {
        Ok(args::Invocation::List { json }) => assert!(!json),
        _ => panic!("list must parse without flags"),
    }
    match args::parse_from(
        &i18n,
        &[
            "iwe",
            "workspace",
            "add-project",
            "/tmp/repo",
            "--space",
            "Work",
            "--json",
        ],
    ) {
        Ok(args::Invocation::AddProject {
            directory,
            space,
            json,
        }) => {
            assert_eq!(directory, PathBuf::from("/tmp/repo"));
            assert_eq!(space.as_deref(), Some("Work"));
            assert!(json);
        }
        _ => panic!("add-project must carry directory, space, and json"),
    }
    match args::parse_from(&i18n, &["iwe", "workspace", "remove-project", "web"]) {
        Ok(args::Invocation::RemoveProject { target, json }) => {
            assert_eq!(target, "web");
            assert!(!json);
        }
        _ => panic!("remove-project must carry its target"),
    }
}

#[test]
fn parse_rejects_unknown_commands_options_and_missing_values() {
    let i18n = english();
    for (arguments, kind) in [
        (vec!["iwe", "nonsense"], ErrorKind::InvalidSubcommand),
        (vec!["iwe", "--bogus"], ErrorKind::UnknownArgument),
        (
            vec!["iwe", "workspace", "add-project"],
            ErrorKind::MissingRequiredArgument,
        ),
        (
            vec!["iwe", "workspace"],
            ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand,
        ),
        (vec!["iwe", "--help"], ErrorKind::DisplayHelp),
        (vec!["iwe", "--version"], ErrorKind::DisplayVersion),
    ] {
        let error = args::parse_from(&i18n, &arguments).unwrap_err();
        assert_eq!(error.kind(), kind);
    }
}

#[test]
fn report_turns_parse_failures_into_exit_codes() {
    let i18n = english();
    let help = args::parse_from(&i18n, &["iwe", "--help"]).unwrap_err();
    assert_eq!(args::report(&i18n, &help), ExitCode::SUCCESS);
    let version = args::parse_from(&i18n, &["iwe", "--version"]).unwrap_err();
    assert_eq!(args::report(&i18n, &version), ExitCode::SUCCESS);
    let unknown = args::parse_from(&i18n, &["iwe", "nonsense"]).unwrap_err();
    assert_eq!(args::report(&i18n, &unknown), ExitCode::from(2));
    let missing = args::parse_from(&i18n, &["iwe", "workspace", "add-project"]).unwrap_err();
    assert_eq!(args::report(&i18n, &missing), ExitCode::from(2));
    let bare = args::parse_from(&i18n, &["iwe", "workspace"]).unwrap_err();
    assert_eq!(args::report(&i18n, &bare), ExitCode::from(2));
}

#[test]
fn workspace_view_serializes_spaces_projects_and_pin_state() {
    let mut workspace = Workspace::default();
    workspace.create_space("Work".to_owned(), vec![PathBuf::from("/repo")]);
    workspace.spaces[0].projects[0].pinned = true;
    let view = output::WorkspaceView::from_workspace(&workspace);
    let value: serde_json::Value =
        serde_json::from_str(&serde_json::to_string(&view).unwrap()).unwrap();
    assert_eq!(value["spaces"][0]["name"], "Work");
    assert_eq!(value["spaces"][0]["projects"][0]["name"], "repo");
    assert_eq!(value["spaces"][0]["projects"][0]["directory"], "/repo");
    assert_eq!(value["spaces"][0]["projects"][0]["pinned"], true);
}
