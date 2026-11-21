use super::*;

#[test]
fn reordered_projects_keep_their_order_and_selection_after_reopening() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("workspace.json");
    let mut workspace = Workspace::default();
    workspace.create_space(
        "Team".to_owned(),
        ["api", "web", "docs"].map(PathBuf::from).to_vec(),
    );
    workspace.select_project(0, 1);
    assert!(workspace.move_project(0, 1, 0));
    save_at(&path, &workspace).unwrap();
    let mut restored = load_at(&path).unwrap();
    restored.normalize();
    assert_eq!(
        restored.spaces[0]
            .projects
            .iter()
            .map(|project| project.name.as_str())
            .collect::<Vec<_>>(),
        ["web", "api", "docs"]
    );
    assert_eq!(restored.directory(), Some(&PathBuf::from("web")));
}

#[test]
fn legacy_storage_is_backed_up_before_spaces_are_saved() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("workspace.json");
    let bytes = br#"{"projects":[{"name":"Team","directories":["frontend","backend"]}],"selected_project":0,"selected_directory":1,"preferences":{"language":"ru"}}"#;
    std::fs::write(&path, bytes).unwrap();
    let mut workspace = load_at(&path).unwrap();
    workspace.normalize();
    assert_eq!(workspace.directory(), Some(&PathBuf::from("backend")));
    let backup = path.with_extension("v1.json");
    assert_eq!(std::fs::read(&backup).unwrap(), bytes);
    save_at(&path, &workspace).unwrap();
    let restored = load_at(&path).unwrap();
    assert_eq!(restored.spaces[0].name, "Team");
    assert_eq!(restored.spaces[0].projects.len(), 2);
    assert_eq!(restored.preferences.language.as_deref(), Some("ru"));
    assert_eq!(
        restored.preferences.appearance,
        crate::domain::workspace::Appearance::Dark
    );
    assert_eq!(std::fs::read(&backup).unwrap(), bytes);
    std::fs::write(&path, b"invalid json").unwrap();
    assert!(matches!(load_at(&path), Err(Failure::Storage)));
    assert_eq!(std::fs::read(&path).unwrap(), b"invalid json");
}

#[test]
fn storage_roundtrip_preserves_preferences_and_overwrites_atomically() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("config/workspace.json");
    let mut workspace = Workspace::default();
    workspace.create_space("Work".to_owned(), vec![directory.path().to_path_buf()]);
    save_at(&path, &workspace).unwrap();
    workspace.preferences.language = Some("es".to_owned());
    workspace.preferences.code_size = 18;
    workspace.preferences.appearance = crate::domain::workspace::Appearance::Light;
    save_at(&path, &workspace).unwrap();
    let restored: Workspace = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    assert_eq!(restored.preferences.language.as_deref(), Some("es"));
    assert_eq!(restored.preferences.code_size, 18);
    assert_eq!(
        restored.preferences.appearance,
        crate::domain::workspace::Appearance::Light
    );
    assert_eq!(restored.spaces.len(), 1);
    assert_eq!(restored.spaces[0].projects.len(), 1);
}

#[test]
fn saves_carry_an_explicit_format_version() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("workspace.json");
    save_at(&path, &Workspace::default()).unwrap();
    let value: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(value["version"], 2);
    load_at(&path).unwrap();
}

#[test]
fn documents_from_a_newer_version_are_rejected_without_changes() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("workspace.json");
    let bytes = br#"{"version":3,"spaces":[]}"#;
    std::fs::write(&path, bytes).unwrap();
    assert!(matches!(load_at(&path), Err(Failure::NewerVersion)));
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
}

#[test]
fn version_one_documents_read_like_current_ones() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("workspace.json");
    let mut workspace = Workspace::default();
    workspace.create_space("Team".to_owned(), vec![directory.path().to_path_buf()]);
    save_at(&path, &workspace).unwrap();
    let mut value: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    value["version"] = serde_json::json!(1);
    std::fs::write(&path, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
    let restored = load_at(&path).unwrap();
    assert_eq!(restored.spaces[0].name, "Team");
    // Only the pre-spaces layout earns a backup.
    assert!(!path.with_extension("v1.json").exists());
}

#[tokio::test]
async fn directory_reads_are_shallow_sorted_and_exclude_git_metadata() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir(root.path().join("src")).unwrap();
    std::fs::create_dir(root.path().join(".git")).unwrap();
    std::fs::write(root.path().join("a.rs"), "fn main() {}").unwrap();
    std::fs::write(root.path().join("src/nested.rs"), "nested").unwrap();
    let entries = entries(root.path().to_owned(), root.path().to_owned())
        .await
        .unwrap();
    assert_eq!(
        entries
            .iter()
            .map(|entry| entry.name.as_str())
            .collect::<Vec<_>>(),
        ["src", "a.rs"]
    );
    assert_eq!(
        read(root.path().to_owned(), root.path().join("a.rs"))
            .await
            .unwrap(),
        "fn main() {}"
    );
}

#[tokio::test]
async fn previews_reject_binary_large_missing_and_external_files() {
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::NamedTempFile::new().unwrap();
    assert_eq!(
        read(root.path().to_owned(), outside.path().to_owned())
            .await
            .unwrap_err(),
        Failure::Read
    );
    assert_eq!(
        read(root.path().to_owned(), root.path().join("missing"))
            .await
            .unwrap_err(),
        Failure::Read
    );
    assert_eq!(decode(vec![0, 1]).unwrap_err(), Failure::Binary);
    assert_eq!(decode(vec![255]).unwrap_err(), Failure::Binary);
    assert_eq!(
        decode(vec![b'x'; PREVIEW_LIMIT as usize + 1]).unwrap_err(),
        Failure::TooLarge
    );
}

#[cfg(unix)]
#[tokio::test]
async fn symlinks_cannot_read_outside_the_linked_directory() {
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::NamedTempFile::new().unwrap();
    let link = root.path().join("outside");
    std::os::unix::fs::symlink(outside.path(), &link).unwrap();
    assert!(
        entries(root.path().to_owned(), root.path().to_owned())
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        read(root.path().to_owned(), link).await.unwrap_err(),
        Failure::Read
    );
}

#[tokio::test]
async fn dense_sources_use_pages_without_discarding_content() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("dense.txt");
    let text = "x\n".repeat(crate::domain::changes::PREVIEW_LINES + 1);
    std::fs::write(&path, &text).unwrap();
    assert_eq!(
        read(directory.path().to_owned(), path.clone()).await,
        Err(Failure::TooLarge)
    );
    let (page, cursor) = pages::first(directory.path(), &path).unwrap();
    let cursor = cursor.unwrap();
    assert!(text.starts_with(&page));
    assert!(cursor.next.is_some());
    assert_eq!(page.lines().count(), 4096);
}

#[test]
fn first_local_read_keeps_the_size_boundary_and_next_cursor() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("source.txt");
    let prefix = "x".repeat(PREVIEW_LIMIT as usize);
    std::fs::write(&path, &prefix).unwrap();
    let (text, page) = pages::first(root.path(), &path).unwrap();
    assert_eq!(text, prefix);
    assert!(page.is_none());

    std::fs::write(&path, format!("{prefix}y")).unwrap();
    let (text, page) = pages::first(root.path(), &path).unwrap();
    let page = page.unwrap();
    assert_eq!(text, prefix);
    assert_eq!(page.next.unwrap().offset, PREVIEW_LIMIT);
    assert_eq!(page.version.unwrap().0, PREVIEW_LIMIT + 1);
}
