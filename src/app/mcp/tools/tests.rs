//! Checks for path validation, revision tokens, and candidate collection.

use super::{
    collect_candidates, forward_slashes, relative_path, revision_of, unknown_project,
    valid_revision,
};
use crate::domain::workspace::Workspace;
use std::path::Path;

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
