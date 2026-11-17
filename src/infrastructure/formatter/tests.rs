use super::*;
use discovery::{Tool, ancestors, configured, discover};

#[test]
fn nested_configuration_takes_priority_over_root_formatter() {
    let project = tempfile::tempdir().unwrap();
    let root = project.path().canonicalize().unwrap();
    let nested = root.join("packages/frontend/src");
    std::fs::create_dir_all(&nested).unwrap();
    std::fs::write(root.join("biome.json"), "{}").unwrap();
    std::fs::write(nested.parent().unwrap().join(".prettierrc"), "{}").unwrap();
    let package = root.join("node_modules/prettier");
    std::fs::create_dir_all(&package).unwrap();
    std::fs::write(package.join("package.json"), r#"{"bin":"entry.js"}"#).unwrap();
    std::fs::write(package.join("entry.js"), "").unwrap();
    let path = nested.join("file.ts");
    let plan = discover(&root, &path).unwrap().unwrap();
    assert_eq!(plan.directory, nested.parent().unwrap());
    assert_eq!(plan.arguments[0], package.join("entry.js").as_os_str());
    assert_eq!(plan.arguments[1], "--stdin-filepath");
    assert_eq!(plan.arguments[2], path.as_os_str());
}

#[test]
fn python_configuration_uses_toml_tables_not_mentions_in_comments() {
    let project = tempfile::tempdir().unwrap();
    let config = project.path().join("pyproject.toml");
    std::fs::write(&config, "# [tool.ruff]\n[tool.black]\nline-length = 79\n").unwrap();
    assert_eq!(
        configured(
            project.path(),
            Some(crate::domain::language::Language::Python)
        )
        .unwrap(),
        Some(Tool::Black)
    );
    std::fs::write(&config, "[tool.ruff.format]\nquote-style = 'single'\n").unwrap();
    assert_eq!(
        configured(
            project.path(),
            Some(crate::domain::language::Language::Python)
        )
        .unwrap(),
        Some(Tool::Ruff)
    );
    std::fs::write(&config, "[tool.ruff\n").unwrap();
    assert!(
        configured(
            project.path(),
            Some(crate::domain::language::Language::Python)
        )
        .is_err()
    );
}

#[test]
fn formatter_search_stays_within_project_and_rejects_package_path_escape() {
    let project = tempfile::tempdir().unwrap();
    let root = project.path().canonicalize().unwrap();
    assert!(ancestors(&root, &root.join("../outside.py")).is_err());
    assert!(ancestors(&root, Path::new("relative.py")).is_err());
    let package = root.join("node_modules/prettier");
    std::fs::create_dir_all(&package).unwrap();
    std::fs::write(
        package.join("package.json"),
        r#"{"bin":"../../outside.js"}"#,
    )
    .unwrap();
    assert!(discover(&root, &root.join("main.ts")).is_err());
}

#[test]
fn eslint_configuration_does_not_enable_semantic_fixes_during_formatting() {
    let project = tempfile::tempdir().unwrap();
    let root = project.path().canonicalize().unwrap();
    std::fs::write(root.join("eslint.config.js"), "export default [];").unwrap();
    assert!(discover(&root, &root.join("main.js")).unwrap().is_none());
}

#[test]
fn python_formatter_prefers_project_virtual_environment() {
    let project = tempfile::tempdir().unwrap();
    let root = project.path().canonicalize().unwrap();
    std::fs::write(root.join("ruff.toml"), "line-length = 100").unwrap();
    let executable = root.join(if cfg!(windows) {
        ".venv/Scripts/ruff.exe"
    } else {
        ".venv/bin/ruff"
    });
    std::fs::create_dir_all(executable.parent().unwrap()).unwrap();
    std::fs::write(&executable, "").unwrap();
    let plan = discover(&root, &root.join("main.py")).unwrap().unwrap();
    assert_eq!(plan.program, executable);
    assert_eq!(plan.arguments[0], "format");
}

#[cfg(unix)]
#[tokio::test]
#[ignore = "requires IWE_TEST_FORMATTER_TOOLS with Prettier, Biome, Ruff, and Black"]
async fn installed_formatters_use_project_rules_and_leave_disk_content_unchanged() {
    let tools = std::path::PathBuf::from(
        std::env::var_os("IWE_TEST_FORMATTER_TOOLS").expect("formatter fixture tools"),
    );
    let cases = [
        (
            "example.js",
            ".prettierrc.json",
            r#"{"singleQuote":true,"semi":false}"#,
            "const value=\"hello\";",
            "const value = 'hello'\n",
        ),
        (
            "example.ts",
            "biome.json",
            r#"{"javascript":{"formatter":{"quoteStyle":"single","semicolons":"asNeeded"}}}"#,
            "const value=\"hello\";",
            "const value = 'hello'\n",
        ),
        (
            "example.py",
            "ruff.toml",
            "[format]\nquote-style = 'single'\n",
            "value=\"hello\"",
            "value = 'hello'\n",
        ),
        (
            "example.py",
            "pyproject.toml",
            "[tool.black]\nskip-string-normalization = true\n",
            "value= 'hello'",
            "value = 'hello'\n",
        ),
    ];
    for (filename, config, rules, source, expected) in cases {
        let project = tempfile::tempdir().unwrap();
        let root = project.path().canonicalize().unwrap();
        std::os::unix::fs::symlink(tools.join("node_modules"), root.join("node_modules")).unwrap();
        std::os::unix::fs::symlink(tools.join("venv"), root.join(".venv")).unwrap();
        std::fs::write(root.join(config), rules).unwrap();
        let path = root.join(filename);
        std::fs::write(&path, "unchanged on disk").unwrap();
        let output = format(&root, &path, source).await.unwrap().unwrap();
        assert_eq!(output, expected, "{config}");
        assert_eq!(std::fs::read_to_string(path).unwrap(), "unchanged on disk");
        eprintln!("Verified project formatting: {config}");
    }
}

#[tokio::test]
#[ignore = "requires Node.js"]
async fn formatter_failures_cannot_replace_the_document_with_empty_or_excessive_output() {
    let project = tempfile::tempdir().unwrap();
    for script in [
        "process.exit(1)",
        "process.exit(0)",
        "process.stdout.write('x'.repeat(2097153))",
    ] {
        let plan = discovery::Plan {
            program: crate::infrastructure::process::executable("node"),
            arguments: vec!["-e".into(), script.into()],
            directory: project.path().to_owned(),
        };
        assert!(run(&plan, "unsaved text").await.is_err());
    }
}

#[test]
fn web_defaults_do_not_capture_php_or_toml_documents() {
    let project = tempfile::tempdir().unwrap();
    let root = project.path().canonicalize().unwrap();
    std::fs::write(root.join("biome.json"), "{}").unwrap();
    for package in ["prettier", "@biomejs/biome"] {
        let directory = root.join("node_modules").join(package);
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join("package.json"), r#"{"bin":"entry.js"}"#).unwrap();
        std::fs::write(directory.join("entry.js"), "").unwrap();
    }
    for filename in ["index.php", "config.toml"] {
        assert!(discover(&root, &root.join(filename)).unwrap().is_none());
    }
    std::fs::write(root.join(".prettierrc"), "{}").unwrap();
    for filename in ["index.php", "config.toml"] {
        let plan = discover(&root, &root.join(filename)).unwrap().unwrap();
        assert_eq!(plan.arguments[1], "--stdin-filepath");
    }
}
