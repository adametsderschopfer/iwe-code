use super::*;
use serde_json::json;

#[test]
fn resolves_unicode_symbols_at_character_boundaries() {
    assert_eq!(symbol_at("let имя = 2", 6), Some("имя"));
    assert_eq!(symbol_at("x + y", 2), None);
    assert_eq!(symbol_at("имя", 1), None);
}

#[test]
fn rename_validates_every_file_before_changing_the_buffer() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("a.py");
    let uri = url::Url::from_file_path(&path).unwrap().to_string();
    let change = json!({"range":{"start":{"line":0,"character":0},"end":{"line":0,"character":1}},"newText":"count"});
    let local = json!({"changes":{uri.clone():[change.clone()]}});
    assert_eq!(apply_local(&path, "x = 1", &local).unwrap(), "count = 1");
    let external = json!({"changes":{uri:[change.clone()],"file:///other":[change]}});
    assert_eq!(
        apply_local(&path, "x = 1", &external),
        Err(Key::EditorRenameScope)
    );
}

#[tokio::test]
#[ignore = "requires an installed Pyright language server"]
async fn live_symbol_references_and_rename_use_the_python_server() {
    use crate::infrastructure::language_server::service::{Query, Service};
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let path = root.join("example.py");
    let text = "def sample():\n    count = 1\n    return count + count\n";
    std::fs::write(&path, text).unwrap();
    let mut service = Service::default();
    let pos = language::Position {
        line: 1,
        character: 6,
    };
    let usages = service
        .request(&root, &path, text, pos, "textDocument/references")
        .await
        .unwrap();
    let locations = usages.as_array().unwrap();
    assert_eq!(
        locations
            .iter()
            .filter(|location| location["range"]["start"]["line"] == 2)
            .count(),
        2
    );
    let edits = service
        .request(&root, &path, text, pos, Query::Rename("amount"))
        .await
        .unwrap();
    assert_eq!(
        apply_local(&path, text, &edits).unwrap(),
        text.replace("count", "amount")
    );
}
