use super::{args, belongs_to_interface, deployment, output};
use crate::domain::deployment::{Connection, Connections, Profile};
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

#[test]
fn parse_recognizes_deployment_configuration_commands() {
    let i18n = english();
    match args::parse_from(&i18n, &["iwe", "deployment", "list", "--json"]) {
        Ok(args::Invocation::Deployment(args::Deployment::List { json })) => assert!(json),
        _ => panic!("deployment list must accept --json"),
    }
    match args::parse_from(
        &i18n,
        &[
            "iwe",
            "deployment",
            "connection",
            "add",
            "web",
            "--host",
            "example.com",
            "--username",
            "deploy",
            "--protocol",
            "ftp",
            "--port",
            "2121",
            "--auth",
            "password",
            "--password-stdin",
            "--json",
        ],
    ) {
        Ok(args::Invocation::Deployment(args::Deployment::ConnectionAdd(draft))) => {
            assert_eq!(draft.name, "web");
            assert_eq!(draft.host, "example.com");
            assert_eq!(draft.username, "deploy");
            assert_eq!(draft.protocol.as_deref(), Some("ftp"));
            assert_eq!(draft.port.as_deref(), Some("2121"));
            assert_eq!(draft.auth.as_deref(), Some("password"));
            assert!(draft.password_stdin);
            assert!(draft.json);
        }
        _ => panic!("connection add must carry its values"),
    }
    match args::parse_from(
        &i18n,
        &[
            "iwe",
            "deployment",
            "connection",
            "set",
            "web",
            "--host",
            "other.com",
        ],
    ) {
        Ok(args::Invocation::Deployment(args::Deployment::ConnectionSet { name, changes })) => {
            assert_eq!(name, "web");
            assert_eq!(changes.host.as_deref(), Some("other.com"));
            assert!(changes.username.is_none());
            assert!(!changes.password_stdin);
        }
        _ => panic!("connection set must carry only changed values"),
    }
    match args::parse_from(
        &i18n,
        &["iwe", "deployment", "connection", "remove", "web", "--json"],
    ) {
        Ok(args::Invocation::Deployment(args::Deployment::ConnectionRemove { name, json })) => {
            assert_eq!(name, "web");
            assert!(json);
        }
        _ => panic!("connection remove must carry its name"),
    }
    match args::parse_from(
        &i18n,
        &[
            "iwe",
            "deployment",
            "key",
            "add",
            "staging",
            "/home/user/id_ed25519",
        ],
    ) {
        Ok(args::Invocation::Deployment(args::Deployment::KeyAdd { name, path, json })) => {
            assert_eq!(name, "staging");
            assert_eq!(path, PathBuf::from("/home/user/id_ed25519"));
            assert!(!json);
        }
        _ => panic!("key add must carry its name and path"),
    }
    match args::parse_from(&i18n, &["iwe", "deployment", "key", "remove", "staging"]) {
        Ok(args::Invocation::Deployment(args::Deployment::KeyRemove { name, json })) => {
            assert_eq!(name, "staging");
            assert!(!json);
        }
        _ => panic!("key remove must carry its name"),
    }
}

#[test]
fn parse_recognizes_deployment_profile_commands() {
    let i18n = english();
    match args::parse_from(
        &i18n,
        &[
            "iwe",
            "deployment",
            "profile",
            "add",
            "production",
            "--project",
            "/repo",
            "--connection",
            "web",
            "--root",
            "/var/www",
            "--mapping",
            "web=assets",
            "--mapping",
            "docs=docs",
            "--exclude",
            "web/uploads",
            "--default",
            "--space",
            "Work",
            "--json",
        ],
    ) {
        Ok(args::Invocation::Deployment(args::Deployment::ProfileAdd(draft))) => {
            assert_eq!(draft.name, "production");
            assert_eq!(draft.project, PathBuf::from("/repo"));
            assert_eq!(draft.connection, "web");
            assert_eq!(draft.root, "/var/www");
            assert_eq!(draft.mappings, vec!["web=assets", "docs=docs"]);
            assert_eq!(draft.excluded, vec!["web/uploads"]);
            assert_eq!(draft.default, Some(true));
            assert_eq!(draft.space.as_deref(), Some("Work"));
            assert!(draft.json);
        }
        _ => panic!("profile add must carry its values"),
    }
    match args::parse_from(
        &i18n,
        &[
            "iwe",
            "deployment",
            "profile",
            "add",
            "production",
            "--project",
            "/repo",
            "--connection",
            "web",
            "--root",
            "/var/www",
            "--mapping",
            "web=assets",
        ],
    ) {
        Ok(args::Invocation::Deployment(args::Deployment::ProfileAdd(draft))) => {
            assert_eq!(draft.default, None);
            assert!(draft.excluded.is_empty());
        }
        _ => panic!("profile add must default to no default flag"),
    }
    match args::parse_from(
        &i18n,
        &[
            "iwe",
            "deployment",
            "profile",
            "set",
            "production",
            "--root",
            "/srv/www",
            "--clear-excluded",
            "--default=false",
        ],
    ) {
        Ok(args::Invocation::Deployment(args::Deployment::ProfileSet { name, changes })) => {
            assert_eq!(name, "production");
            assert_eq!(changes.root.as_deref(), Some("/srv/www"));
            assert!(changes.project.is_none());
            assert!(changes.mappings.is_none());
            assert!(changes.clear_excluded);
            assert_eq!(changes.default, Some(false));
        }
        _ => panic!("profile set must carry only changed values"),
    }
    match args::parse_from(
        &i18n,
        &[
            "iwe",
            "deployment",
            "profile",
            "remove",
            "production",
            "--space",
            "Work",
            "--json",
        ],
    ) {
        Ok(args::Invocation::Deployment(args::Deployment::ProfileRemove { name, space, json })) => {
            assert_eq!(name, "production");
            assert_eq!(space.as_deref(), Some("Work"));
            assert!(json);
        }
        _ => panic!("profile remove must carry its name and space"),
    }
}

#[test]
fn parse_recognizes_deployment_transfer_commands() {
    let i18n = english();
    match args::parse_from(
        &i18n,
        &[
            "iwe",
            "deployment",
            "test",
            "production",
            "--space",
            "Work",
            "--json",
        ],
    ) {
        Ok(args::Invocation::Deployment(args::Deployment::Test {
            profile,
            space,
            trust,
            json,
        })) => {
            assert_eq!(profile, "production");
            assert_eq!(space.as_deref(), Some("Work"));
            assert_eq!(trust, None);
            assert!(json);
        }
        _ => panic!("test must carry its profile"),
    }
    match args::parse_from(
        &i18n,
        &[
            "iwe",
            "deployment",
            "upload",
            "production",
            "/repo/web",
            "--trust-host-key",
            "SHA256:abcd",
        ],
    ) {
        Ok(args::Invocation::Deployment(args::Deployment::Upload {
            profile,
            path,
            space,
            trust,
            json,
        })) => {
            assert_eq!(profile, "production");
            assert_eq!(path, PathBuf::from("/repo/web"));
            assert!(space.is_none());
            assert_eq!(trust.as_deref(), Some("SHA256:abcd"));
            assert!(!json);
        }
        _ => panic!("upload must carry its profile and path"),
    }
    match args::parse_from(
        &i18n,
        &[
            "iwe",
            "deployment",
            "download",
            "production",
            "/var/www/assets",
            "--overwrite",
            "--json",
        ],
    ) {
        Ok(args::Invocation::Deployment(args::Deployment::Download {
            profile,
            remote,
            overwrite,
            space: _,
            trust: _,
            json,
        })) => {
            assert_eq!(profile, "production");
            assert_eq!(remote, "/var/www/assets");
            assert!(overwrite);
            assert!(json);
        }
        _ => panic!("download must carry its profile and remote path"),
    }
    match args::parse_from(
        &i18n,
        &["iwe", "deployment", "list-remote", "production", "assets"],
    ) {
        Ok(args::Invocation::Deployment(args::Deployment::ListRemote {
            profile,
            directory,
            json,
            ..
        })) => {
            assert_eq!(profile, "production");
            assert_eq!(directory.as_deref(), Some("assets"));
            assert!(!json);
        }
        _ => panic!("list-remote must carry its profile and optional directory"),
    }
}

#[test]
fn parse_rejects_bare_deployment_groups() {
    let i18n = english();
    for arguments in [
        vec!["iwe", "deployment"],
        vec!["iwe", "deployment", "connection"],
        vec!["iwe", "deployment", "key"],
        vec!["iwe", "deployment", "profile"],
    ] {
        let error = args::parse_from(&i18n, &arguments).unwrap_err();
        assert_eq!(
            error.kind(),
            ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand
        );
    }
    for arguments in [
        vec!["iwe", "deployment", "connection", "add", "web"],
        vec!["iwe", "deployment", "upload", "production"],
    ] {
        let error = args::parse_from(&i18n, &arguments).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::MissingRequiredArgument);
    }
}

#[test]
fn deployment_view_serializes_settings_without_secrets() {
    let mut workspace = Workspace::default();
    workspace.create_space("Work".to_owned(), vec![PathBuf::from("/repo")]);
    workspace.deployment.connections.push(Connection {
        id: "c1".into(),
        name: "web".into(),
        host: "example.com".into(),
        port: 22,
        username: "deploy".into(),
        ..Connection::default()
    });
    workspace.spaces[0].deployments.push(Profile {
        id: "p1".into(),
        name: "production".into(),
        project: PathBuf::from("/repo"),
        connection: "c1".into(),
        root: "/var/www".into(),
        ..Profile::default()
    });
    let view = output::DeploymentView::from_workspace(&workspace);
    let value: serde_json::Value =
        serde_json::from_str(&serde_json::to_string(&view).unwrap()).unwrap();
    assert_eq!(value["connections"][0]["name"], "web");
    assert_eq!(value["connections"][0]["protocol"], "sftp");
    assert_eq!(value["keys"], serde_json::json!([]));
    assert_eq!(value["spaces"][0]["profiles"][0]["name"], "production");
    assert_eq!(value["spaces"][0]["profiles"][0]["connection_name"], "web");
    assert_eq!(value["spaces"][0]["profiles"][0]["root"], "/var/www");
}

#[test]
fn deployment_resolvers_match_identifiers_names_and_ambiguity() {
    let settings = Connections {
        connections: vec![
            Connection {
                id: "c1".into(),
                name: "web".into(),
                ..Connection::default()
            },
            Connection {
                id: "c2".into(),
                name: "web".into(),
                ..Connection::default()
            },
        ],
        keys: Vec::new(),
    };
    assert_eq!(deployment::connection_index(&settings, "c2").ok(), Some(1));
    assert_eq!(
        deployment::connection_index(&settings, "missing").ok(),
        None
    );
    assert!(matches!(
        deployment::connection_index(&settings, "web"),
        Err(deployment::Failure::Missing(
            crate::i18n::Key::CliErrorAmbiguousConnection,
            _
        ))
    ));
}

#[test]
fn deployment_mapping_parser_splits_and_normalizes() {
    let values = vec!["web=assets".to_owned(), "docs=/docs".to_owned()];
    let mappings = deployment::mappings_of(&values).unwrap();
    assert_eq!(mappings[0].local, PathBuf::from("web"));
    assert_eq!(mappings[0].remote, "assets");
    assert_eq!(mappings[1].remote, "docs");
    for raw in ["web", "=assets", "web="] {
        assert!(
            deployment::mappings_of(&[raw.to_owned()]).is_err(),
            "{raw} must not parse"
        );
    }
}
