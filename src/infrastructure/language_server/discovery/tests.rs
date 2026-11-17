use super::*;

#[test]
fn npm_launch_uses_node_and_preserves_script_paths_as_one_argument() {
    let temp = tempfile::tempdir().unwrap();
    let directory = temp.path().join("tools with spaces & symbols");
    let package = directory.join("node_modules/pyright");
    std::fs::create_dir_all(package.join("dist")).unwrap();
    std::fs::write(directory.join("node.exe"), "").unwrap();
    std::fs::write(package.join("dist/server.js"), "").unwrap();
    std::fs::write(
        package.join("package.json"),
        r#"{"bin":{"pyright-langserver":"./dist/server.js"}}"#,
    )
    .unwrap();
    for candidate in [&directory, &directory.join("node_modules/.bin")] {
        let server = npm_server(
            candidate,
            "pyright-langserver",
            &["--stdio"],
            std::slice::from_ref(&directory),
        )
        .unwrap();
        assert_eq!(server.program, directory.join("node.exe"));
        assert_eq!(
            server.arguments,
            [
                package.join("./dist/server.js").to_str().unwrap(),
                "--stdio"
            ]
        );
    }
}

#[test]
fn npm_discovery_rejects_missing_runtime_and_escaping_entry_paths() {
    let temp = tempfile::tempdir().unwrap();
    let package = temp.path().join("node_modules/intelephense");
    std::fs::create_dir_all(&package).unwrap();
    std::fs::write(package.join("server.js"), "").unwrap();
    std::fs::write(package.join("package.json"), r#"{"bin":"server.js"}"#).unwrap();
    assert!(npm_server(temp.path(), "intelephense", &["--stdio"], &[]).is_none());
    std::fs::write(temp.path().join("node.exe"), "").unwrap();
    assert!(npm_server(temp.path(), "intelephense", &["--stdio"], &[]).is_some());
    std::fs::write(
        package.join("package.json"),
        r#"{"bin":"../intelephense/server.js"}"#,
    )
    .unwrap();
    assert!(npm_server(temp.path(), "intelephense", &["--stdio"], &[]).is_none());
}
