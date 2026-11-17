use super::*;

/// Run explicitly with IWE_TEST_LANGUAGE_SERVER pointing to pyright-langserver.
#[tokio::test]
#[ignore = "requires an installed Pyright language server"]
async fn python_completion_uses_unsaved_document_text() {
    let program = std::env::var_os("IWE_TEST_LANGUAGE_SERVER").expect("server path");
    let project = tempfile::tempdir().unwrap();
    let root = project.path().canonicalize().unwrap();
    let path = root.join("example.py");
    let uri = url::Url::from_file_path(&path).unwrap().to_string();
    let mut client = Client::connect(Path::new(&program), &["--stdio".to_owned()], &root)
        .await
        .unwrap();
    client
        .synchronize(&uri, "python", "message = 'hello'\nmessage.")
        .await
        .unwrap();
    let result = client
        .call(
            "textDocument/completion",
            json!({
                "textDocument": {"uri": uri}, "position": {"line": 1, "character": 8}
            }),
        )
        .await
        .unwrap();
    let items = result
        .as_array()
        .or_else(|| result["items"].as_array())
        .unwrap();
    assert!(
        items.iter().any(|item| item["label"] == "upper"),
        "{result}"
    );
    client
        .synchronize(&uri, "python", "message = []\nmessage.")
        .await
        .unwrap();
    let result = client
        .call(
            "textDocument/completion",
            json!({
                "textDocument": {"uri": uri}, "position": {"line": 1, "character": 8}
            }),
        )
        .await
        .unwrap();
    let items = result
        .as_array()
        .or_else(|| result["items"].as_array())
        .unwrap();
    assert!(
        items.iter().any(|item| item["label"] == "append"),
        "{result}"
    );
    client.close(&uri).await.unwrap();
}
