//! Checks for path validation, revision tokens, candidate collection, and deployment tools.

use super::{
    collect_candidates, deployment_profile, deployment_transfer, deployment_transfer_message,
    deployment_transfer_with_secret, forward_slashes, relative_path, revision_of, unknown_project,
    valid_revision,
};
use crate::domain::deployment::{
    Connection, Connections, Error as DeploymentError, Mapping, Profile, Protocol,
};
use crate::domain::workspace::{Space, Workspace};
use crate::infrastructure::deployment::transfer::{Operation, Output};
use std::path::{Path, PathBuf};

#[test]
fn accepts_relative_paths() {
    assert_eq!(
        relative_path("src/main.rs", "path").expect("valid"),
        Path::new("src").join("main.rs")
    );
    assert_eq!(
        relative_path("notes.md", "path").expect("valid"),
        Path::new("notes.md")
    );
}

#[test]
fn rejects_empty_paths() {
    assert!(relative_path("", "path").is_err());
}

#[test]
fn rejects_absolute_and_rooted_paths() {
    assert!(relative_path("/etc/passwd", "path").is_err());
    assert!(relative_path("/etc", "path").is_err());
}

#[test]
fn rejects_parent_directory_climbs() {
    assert!(relative_path("../outside.txt", "path").is_err());
    assert!(relative_path("src/../main.rs", "path").is_err());
    assert!(relative_path("a/../../b", "path").is_err());
}

#[test]
fn revision_tokens_are_hex_digests() {
    let revision = revision_of("hello");
    assert_eq!(revision.len(), 64);
    assert!(valid_revision(&revision));
    assert_eq!(revision_of("hello"), revision);
    assert_ne!(revision_of("hello"), revision_of("world"));
}

#[test]
fn revision_tokens_reject_other_shapes() {
    assert!(!valid_revision(""));
    assert!(!valid_revision("abc"));
    assert!(!valid_revision(&"g".repeat(64)));
    assert!(!valid_revision(&"a".repeat(63)));
}

#[test]
fn renders_relative_paths_with_forward_slashes() {
    assert_eq!(forward_slashes(Path::new("")), "");
    assert_eq!(forward_slashes(Path::new("notes.md")), "notes.md");
    assert_eq!(
        forward_slashes(&Path::new("src").join("main.rs")),
        "src/main.rs"
    );
}

#[test]
fn unknown_projects_list_the_available_names() {
    let mut workspace = Workspace::default();
    workspace.create_space(
        "Main".to_owned(),
        vec![Path::new("/tmp/alpha").to_path_buf()],
    );
    let message = unknown_project("beta", &workspace);
    assert!(message.contains("Unknown project 'beta'."), "{message}");
    assert!(message.contains("alpha"), "{message}");
}

#[test]
fn unknown_projects_report_an_empty_workspace() {
    let message = unknown_project("beta", &Workspace::default());
    assert!(message.contains("no projects"), "{message}");
}

#[test]
fn collects_candidates_across_directories() {
    let directory = tempfile::tempdir().expect("temp directory");
    let root = directory.path();
    std::fs::create_dir_all(root.join("src/deep")).expect("directories");
    std::fs::create_dir_all(root.join("node_modules/package")).expect("ignored directory");
    std::fs::write(root.join("readme.md"), "readme").expect("file");
    std::fs::write(root.join("src/main.rs"), "main").expect("file");
    std::fs::write(root.join("src/deep/nested.txt"), "nested").expect("file");
    std::fs::write(root.join("node_modules/package/index.js"), "js").expect("file");

    let files = collect_candidates(root);
    let paths: Vec<_> = files.iter().map(|file| file.path.clone()).collect();

    assert!(paths.contains(&root.join("readme.md")));
    assert!(paths.contains(&root.join("src/main.rs")));
    assert!(paths.contains(&root.join("src/deep/nested.txt")));
    assert!(!paths.contains(&root.join("node_modules/package/index.js")));
}

#[cfg(unix)]
#[test]
fn skips_symlinked_files() {
    let directory = tempfile::tempdir().expect("temp directory");
    let root = directory.path();
    std::fs::write(root.join("real.txt"), "real").expect("file");
    std::os::unix::fs::symlink(root.join("real.txt"), root.join("link.txt")).expect("symlink");

    let files = collect_candidates(root);
    let paths: Vec<_> = files.iter().map(|file| file.path.clone()).collect();
    assert!(paths.contains(&root.join("real.txt")));
    assert!(!paths.contains(&root.join("link.txt")));
}

/// Build one workspace with one SFTP profile pointing at the loopback fixture server.
fn fixture_workspace(project: &Path, port: u16, trusted: Option<String>) -> Workspace {
    Workspace {
        spaces: vec![Space {
            name: "Main".to_owned(),
            deployments: vec![Profile {
                id: "p1".to_owned(),
                name: "production".to_owned(),
                project: project.to_path_buf(),
                connection: "c1".to_owned(),
                root: "/".to_owned(),
                mappings: vec![Mapping {
                    local: PathBuf::from("readme.md"),
                    remote: "hello.txt".to_owned(),
                }],
                excluded: Vec::new(),
                default: false,
            }],
            projects: Vec::new(),
        }],
        deployment: Connections {
            connections: vec![Connection {
                id: "c1".to_owned(),
                name: "web".to_owned(),
                protocol: Protocol::Sftp,
                host: "127.0.0.1".to_owned(),
                port,
                username: "user".to_owned(),
                authentication: crate::domain::deployment::Authentication::Password,
                trusted_host_key: trusted,
            }],
            keys: Vec::new(),
        },
        ..Workspace::default()
    }
}

#[test]
fn deployment_profiles_resolve_by_unique_name() {
    let project = Path::new("/tmp/repo");
    let workspace = fixture_workspace(project, 22, None);
    let profile = deployment_profile(&workspace, "production").expect("unique profile");
    assert_eq!(profile.name, "production");
    assert_eq!(profile.id, "p1");
    let message = deployment_profile(&workspace, "absent").unwrap_err();
    assert!(
        message.contains("No deployment profile matches 'absent'."),
        "{message}"
    );
}

#[test]
fn deployment_transfer_errors_name_the_untrusted_fingerprint() {
    let message = deployment_transfer_message(DeploymentError::HostUnknown("SHA256:abc".into()));
    assert!(message.contains("SHA256:abc"), "{message}");
    assert!(message.contains("--trust-host-key"), "{message}");
    assert_eq!(
        deployment_transfer_message(DeploymentError::Connection),
        "IWE Code could not connect to the deployment server. Check that the server is reachable and the connection settings are current."
    );
}

#[tokio::test]
async fn deployment_transfer_reads_and_compares_over_loopback_sftp() {
    use base64::Engine;
    use sha2::Digest;

    let server = crate::infrastructure::deployment::transport::fixtures::Ssh::start().await;
    let fingerprint = format!(
        "SHA256:{}",
        base64::engine::general_purpose::STANDARD_NO_PAD.encode(sha2::Sha256::digest(
            server.key.public_key().to_bytes().unwrap()
        ))
    );
    let directory = tempfile::tempdir().expect("temp directory");
    std::fs::write(directory.path().join("readme.md"), "local readme\n").expect("file");
    let workspace = fixture_workspace(directory.path(), server.port, Some(fingerprint));
    let profile = deployment_profile(&workspace, "production").expect("profile");
    let secret = Some(crate::infrastructure::deployment::transport::Secret(
        zeroize::Zeroizing::new("fixture".to_owned()),
    ));

    let output = deployment_transfer_with_secret(
        &workspace,
        profile,
        Operation::Preview("/hello.txt".into()),
        secret.clone(),
    )
    .await
    .expect("read");
    let Output::Preview { text, .. } = output else {
        panic!("expected a preview");
    };
    assert_eq!(*text, "server\n");

    let output = deployment_transfer_with_secret(
        &workspace,
        profile,
        Operation::Compare(directory.path().join("readme.md")),
        secret,
    )
    .await
    .expect("compare");
    let Output::Preview {
        text,
        previous,
        remote,
        ..
    } = output
    else {
        panic!("expected a preview");
    };
    let previous = previous.expect("a remote text");
    assert_eq!(*text, "local readme\n");
    assert_eq!(*previous, "server\n");
    assert_eq!(remote, "/hello.txt");
}

#[tokio::test]
async fn deployment_transfer_reports_untrusted_server_keys() {
    let server = crate::infrastructure::deployment::transport::fixtures::Ssh::start().await;
    let directory = tempfile::tempdir().expect("temp directory");
    let workspace = fixture_workspace(directory.path(), server.port, None);
    let profile = deployment_profile(&workspace, "production").expect("profile");

    let message = deployment_transfer(&workspace, profile, Operation::Preview("/hello.txt".into()))
        .await
        .expect_err("an untrusted key");
    assert!(message.contains("is not trusted"), "{message}");
    assert!(message.contains("--trust-host-key"), "{message}");
}
