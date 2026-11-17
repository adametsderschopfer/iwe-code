use super::*;

#[tokio::test]
#[ignore = "requires installed JSON, YAML, Markdown, and Python language servers"]
async fn installed_servers_support_configuration_and_markdown() {
    let project = tempfile::tempdir().unwrap();
    let root = project.path().canonicalize().unwrap();
    let schema = root.join("schema.json");
    tokio::fs::write(
        &schema,
        r#"{"type":"object","properties":{"port":{"type":"integer"},"host":{"type":"string"}}}"#,
    )
    .await
    .unwrap();
    let uri = url::Url::from_file_path(&schema).unwrap().to_string();
    let cases = [
        (
            "config.json",
            format!("{{\"$schema\":\"{uri}\",\n  |\n}}"),
            "port",
        ),
        (
            "config.yaml",
            format!("# yaml-language-server: $schema={uri}\n|"),
            "port",
        ),
        (
            "example.md",
            "# Example\n```python\nmessage = 'hello'\nmessage.|\n```".to_owned(),
            "upper",
        ),
    ];
    for (filename, marked, expected) in cases {
        let path = root.join(filename);
        let offset = marked.find('|').unwrap();
        let prefix = &marked[..offset];
        let position = Position {
            line: prefix.bytes().filter(|byte| *byte == b'\n').count() as u32,
            character: prefix.rsplit('\n').next().unwrap().encode_utf16().count() as u32,
        };
        let text = marked.replacen('|', "", 1);
        tokio::fs::write(&path, &text).await.unwrap();
        let mut service = Service::default();
        let result = service
            .request(&root, &path, &text, position, "textDocument/completion")
            .await
            .unwrap();
        let items = result
            .as_array()
            .or_else(|| result["items"].as_array())
            .unwrap();
        assert!(
            items.iter().any(|item| item["label"]
                .as_str()
                .is_some_and(|label| label.contains(expected))),
            "{filename}: {result}"
        );
        eprintln!("Verified {filename}: {expected}");
    }
}

/// Explicitly run against locally installed servers after changing protocol integration.
#[tokio::test]
#[ignore = "requires Python, TypeScript, PHP, Go, and Rust language servers"]
async fn installed_servers_complete_project_documents() {
    let cases = [
        ("example.py", "message = 'hello'\nmessage.|", "upper"),
        (
            "example.ts",
            "const message: string = 'hello';\nmessage.|",
            "toUpperCase",
        ),
        (
            "example.js",
            "const message = 'hello';\nmessage.|",
            "toUpperCase",
        ),
        (
            "example.php",
            "<?php\nclass Person { public function greet(): string { return 'hello'; } }\n$person = new Person();\n$person->|",
            "greet",
        ),
        (
            "main.go",
            "package main\nimport \"fmt\"\nfunc main() { fmt.Pr| }\n",
            "Println",
        ),
        (
            "src/main.rs",
            "fn main() { let message = String::new(); message.| }",
            "len",
        ),
    ];
    for (filename, marked, expected) in cases {
        let project = tempfile::tempdir().unwrap();
        let root = project.path().canonicalize().unwrap();
        let path = root.join(filename);
        tokio::fs::create_dir_all(path.parent().unwrap())
            .await
            .unwrap();
        if filename.ends_with(".go") {
            tokio::fs::write(
                root.join("go.mod"),
                "module example.com/completion\n\ngo 1.24\n",
            )
            .await
            .unwrap();
        }
        if filename.ends_with(".rs") {
            tokio::fs::write(
                root.join("Cargo.toml"),
                "[package]\nname = 'completion_test'\nversion = '0.1.0'\nedition = '2024'\n",
            )
            .await
            .unwrap();
        }
        let offset = marked.find('|').unwrap();
        let prefix = &marked[..offset];
        let position = Position {
            line: prefix.bytes().filter(|byte| *byte == b'\n').count() as u32,
            character: prefix.rsplit('\n').next().unwrap().encode_utf16().count() as u32,
        };
        let text = marked.replacen('|', "", 1);
        tokio::fs::write(&path, &text).await.unwrap();
        let mut service = Service::default();
        let mut found = false;
        let mut last = String::new();
        // Rust can still be indexing its standard library after initialization completes.
        for _ in 0..60 {
            match service
                .request(&root, &path, &text, position, "textDocument/completion")
                .await
            {
                Ok(result) => {
                    let items = result.as_array().or_else(|| result["items"].as_array());
                    found = items.is_some_and(|items| {
                        items.iter().any(|item| {
                            item["label"].as_str().is_some_and(|label| {
                                label == expected || label.starts_with(&format!("{expected}("))
                            })
                        })
                    });
                    last = format!("{} candidates", items.map_or(0, Vec::len));
                    if found {
                        break;
                    }
                }
                Err(error) => {
                    last = format!("{:?}", error.kind());
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }
        assert!(found, "{filename}: missing {expected}; {last}");
        eprintln!("Verified {filename}: {expected}");
    }
}

#[test]
fn embedded_completion_cannot_change_surrounding_markdown() {
    let block = embedded::at(
        "# Title\n```ts\nfoo\n```",
        Position {
            line: 2,
            character: 3,
        },
    )
    .unwrap();
    let edit = |line| json!({"range": {"start": {"line": line, "character": 0}, "end": {"line": line, "character": 0}}, "newText": "value"});
    assert!(edits_stay_in_block(&json!({"textEdit": edit(2)}), &block));
    assert!(!edits_stay_in_block(
        &json!({"textEdit": edit(2), "additionalTextEdits": [edit(0)]}),
        &block
    ));
}

#[test]
fn definition_links_return_to_original_markdown_document() {
    let mut value = json!([{"targetUri": "file:///project/example.md.ts", "targetSelectionRange": {"start": {"line": 3, "character": 2}}}]);
    restore_uri(
        &mut value,
        "file:///project/example.md.ts",
        "file:///project/example.md",
    );
    assert_eq!(value[0]["targetUri"], "file:///project/example.md");
    assert_eq!(value[0]["targetSelectionRange"]["start"]["line"], 3);
}

#[tokio::test]
#[ignore = "requires installed language servers and language toolchains"]
async fn installed_servers_resolve_definitions_in_other_project_files() {
    let cases = [
        (
            "main.py",
            "from models import Person\nvalue = Per|son()\n",
            "models.py",
            "class Person:\n    pass\n",
        ),
        (
            "main.js",
            "import { Person } from './models.js';\nconst value = new Per|son();\n",
            "models.js",
            "export class Person {}\n",
        ),
        (
            "main.ts",
            "import { Person } from './models';\nconst value = new Per|son();\n",
            "models.ts",
            "export class Person {}\n",
        ),
        (
            "main.php",
            "<?php\nrequire_once __DIR__ . '/models.php';\n$value = new Per|son();\n",
            "models.php",
            "<?php\nclass Person {}\n",
        ),
        (
            "main.go",
            "package main\nfunc main() { _ = Per|son{} }\n",
            "models.go",
            "package main\ntype Person struct {}\n",
        ),
        (
            "src/main.rs",
            "mod models;\nfn main() { let _ = models::Per|son; }\n",
            "src/models.rs",
            "pub struct Person;\n",
        ),
    ];
    for (filename, marked, dependency, declaration) in cases {
        let project = tempfile::tempdir().unwrap();
        let root = project.path().canonicalize().unwrap();
        let path = root.join(filename);
        let target = root.join(dependency);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        if filename.ends_with(".go") {
            std::fs::write(
                root.join("go.mod"),
                "module example.com/navigation\n\ngo 1.24\n",
            )
            .unwrap();
        }
        if filename.ends_with(".rs") {
            std::fs::write(
                root.join("Cargo.toml"),
                "[package]\nname = 'navigation_test'\nversion = '0.1.0'\nedition = '2024'\n",
            )
            .unwrap();
        }
        let offset = marked.find('|').unwrap();
        let prefix = &marked[..offset];
        let position = Position {
            line: prefix.bytes().filter(|byte| *byte == b'\n').count() as u32,
            character: prefix.rsplit('\n').next().unwrap().encode_utf16().count() as u32,
        };
        let text = marked.replacen('|', "", 1);
        std::fs::write(&path, &text).unwrap();
        std::fs::write(&target, declaration).unwrap();
        let mut service = Service::default();
        let expected_uri = url::Url::from_file_path(&target).unwrap();
        let mut found = false;
        for _ in 0..60 {
            let response = service
                .request(&root, &path, &text, position, "textDocument/definition")
                .await
                .unwrap();
            let values = response
                .as_array()
                .cloned()
                .unwrap_or_else(|| vec![response]);
            found = values.iter().any(|location| {
                location["uri"]
                    .as_str()
                    .or_else(|| location["targetUri"].as_str())
                    == Some(expected_uri.as_str())
            });
            if found {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }
        assert!(found, "{filename}: definition did not reach {dependency}");
        eprintln!("Verified definition: {filename} -> {dependency}");
    }
}

#[tokio::test]
#[ignore = "requires an installed JSON language server"]
async fn installed_json_server_formats_unsaved_text_without_changing_values() {
    let project = tempfile::tempdir().unwrap();
    let root = project.path().canonicalize().unwrap();
    let path = root.join("config.json");
    let original = "{\"port\":8080,\"label\":\"Пример\",\"enabled\":true}";
    let mut service = Service::default();
    let result = service
        .request(
            &root,
            &path,
            original,
            Position {
                line: 0,
                character: 0,
            },
            "textDocument/formatting",
        )
        .await
        .unwrap();
    let edits = serde_json::from_value::<Vec<crate::domain::language::TextEdit>>(result).unwrap();
    let formatted = crate::domain::language::apply_edits(original, &edits).unwrap();
    assert!(formatted.contains('\n'));
    assert_eq!(
        serde_json::from_str::<Value>(&formatted).unwrap(),
        serde_json::from_str::<Value>(original).unwrap()
    );
    assert!(!path.exists(), "formatting must not save the document");
}
