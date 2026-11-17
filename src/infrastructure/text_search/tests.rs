use super::*;
#[test]
fn project_search_respects_masks_exclusions_and_live_buffers() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_path_buf();
    std::fs::create_dir_all(root.join("src/deep")).unwrap();
    std::fs::create_dir_all(root.join("target")).unwrap();
    for name in ["src/deep/a.rs", "target/b.rs", "c.txt"] {
        std::fs::write(root.join(name), "кот").unwrap();
    }
    let path = root.join("src/deep/a.rs");
    let request = Request {
        root: root.clone(),
        folder: root,
        mask: "*.rs".into(),
        query: "живой".into(),
        options: Options::default(),
        buffers: BTreeMap::from([(path.clone(), "строка\nживой".into())]),
        file: None,
        cancelled: CancellationToken::new(),
    };
    let results = scan(request.clone());
    assert_eq!(results.hits.len(), 1);
    assert_eq!(results.hits[0].path, path);
    assert_eq!(results.hits[0].found.line, 2);
    request.cancelled.cancel();
    assert!(scan(request).hits.is_empty());
}
#[test]
fn large_file_search_reaches_late_lines_and_skips_binary() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_path_buf();
    std::fs::write(
        root.join("large.txt"),
        format!("{}needle", "line\n".repeat(450_000)),
    )
    .unwrap();
    std::fs::write(root.join("binary"), b"\0needle").unwrap();
    let results = scan(Request {
        root: root.clone(),
        folder: root,
        mask: String::new(),
        query: "needle".into(),
        options: Options::default(),
        buffers: BTreeMap::new(),
        file: None,
        cancelled: CancellationToken::new(),
    });
    assert_eq!(results.hits.len(), 1);
    assert_eq!(results.hits[0].found.line, 450_001);
}
