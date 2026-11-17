use super::*;

#[tokio::test]
async fn completes_names_without_including_secret_values() {
    let project = tempfile::tempdir().unwrap();
    let path = project.path().join(".env");
    tokio::fs::write(
        project.path().join(".env.example"),
        "DATABASE_URL=template-secret\n",
    )
    .await
    .unwrap();
    let text = "export DATABASE_HOST=private-host\nDATABASE_PASSWORD=private-password\nURL=${DATA";
    let result = request(
        project.path(),
        &path,
        text,
        Position {
            line: 2,
            character: 10,
        },
        "textDocument/completion",
    )
    .await;
    assert_eq!(result.as_array().unwrap().len(), 3);
    assert!(!result.to_string().contains("private"));
    assert!(!result.to_string().contains("template-secret"));
    assert_eq!(result[0]["textEdit"]["range"]["start"]["character"], 6);
}

#[tokio::test]
async fn finds_definition_at_middle_of_variable_reference() {
    let project = tempfile::tempdir().unwrap();
    let path = project.path().join(".env");
    let result = request(
        project.path(),
        &path,
        "export HOST=localhost\nURL=${HOST}",
        Position {
            line: 1,
            character: 8,
        },
        "textDocument/definition",
    )
    .await;
    assert_eq!(result["range"]["start"]["line"], 0);
    assert_eq!(result["range"]["start"]["character"], 7);
}
