use super::*;

#[test]
fn rejects_a_relative_application_path() {
    assert!(launch_command(Some("Codex".into()), None, "Explain").is_none());
}

#[cfg(target_os = "macos")]
#[test]
fn passes_the_project_and_prompt_to_the_codex_composer() {
    let directory = std::env::current_dir().unwrap();
    let prompt = "Explain café & file #1\nSecond line";
    let (program, arguments) = launch_command(None, Some(directory.clone()), prompt).unwrap();
    assert_eq!(program, PathBuf::from("/usr/bin/open"));
    assert_eq!(arguments[..2], ["-b", "com.openai.codex"]);
    let link = url::Url::parse(arguments[2].to_str().unwrap()).unwrap();
    assert_eq!(link.scheme(), "codex");
    assert_eq!(link.host_str(), Some("new"));
    assert_eq!(
        link.query_pairs().collect::<Vec<_>>(),
        [
            ("prompt".into(), prompt.into()),
            ("path".into(), directory.to_string_lossy())
        ]
    );
}
