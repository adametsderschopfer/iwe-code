use super::*;
use crate::domain::language::apply_edits;
use serde_json::json;

#[test]
fn replaces_unicode_prefix_using_editor_byte_columns() {
    let text = "🦀 объект.им";
    let edits = completion_edits(
        &json!({"label": "имя"}),
        text,
        iced::widget::text_editor::Position {
            line: 0,
            column: text.len(),
        },
    )
    .unwrap();
    let result = apply_edits(text, &edits).unwrap();
    assert_eq!(result, "🦀 объект.имя");
    assert_eq!(
        inserted_cursor(text, &result, &edits).unwrap().column,
        result.len()
    );
}

#[test]
fn positions_cursor_after_import_and_multiline_completion() {
    let text = "pri";
    let item = json!({"label": "print", "insertText": "print(\n)", "additionalTextEdits": [{"range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 0}}, "newText": "import x\n"}]});
    let edits = completion_edits(
        &item,
        text,
        iced::widget::text_editor::Position { line: 0, column: 3 },
    )
    .unwrap();
    assert_eq!(
        apply_edits(text, &edits).as_deref(),
        Some("import x\nprint(\n)")
    );
    let text = "\npri";
    let edits = completion_edits(
        &item,
        text,
        iced::widget::text_editor::Position { line: 1, column: 3 },
    )
    .unwrap();
    let result = apply_edits(text, &edits).unwrap();
    assert_eq!(result, "import x\n\nprint(\n)");
    let cursor = inserted_cursor(text, &result, &edits).unwrap();
    assert_eq!((cursor.line, cursor.column), (3, 1));
}

#[test]
fn uses_server_replacement_range() {
    let item = json!({"label": "name", "textEdit": {"replace": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 4}}, "insert": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 2}}, "newText": "name"}});
    let edits = completion_edits(
        &item,
        "naxx",
        iced::widget::text_editor::Position { line: 0, column: 2 },
    )
    .unwrap();
    assert_eq!(apply_edits("naxx", &edits).as_deref(), Some("name"));
}

fn ranked_labels(query: &str, mut items: Vec<Value>) -> Vec<String> {
    rank_completions(
        &mut items,
        query,
        iced::widget::text_editor::Position {
            line: 0,
            column: query.len(),
        },
    );
    items
        .iter()
        .map(|item| item["label"].as_str().unwrap().to_owned())
        .collect()
}

#[test]
fn typed_case_prefers_variable_but_capitalized_input_prefers_class() {
    let items = vec![
        json!({"label":"Person", "kind":7, "sortText":"0"}),
        json!({"label":"person", "kind":6, "sortText":"1"}),
        json!({"label":"super", "sortText":"0"}),
    ];
    assert_eq!(
        ranked_labels("per", items.clone()),
        ["person", "Person", "super"]
    );
    assert_eq!(ranked_labels("Per", items), ["Person", "person", "super"]);
}

#[test]
fn equal_matches_keep_server_semantics_and_honor_filter_text() {
    let items = vec![
        json!({"label":"print(value)", "filterText":"print", "sortText":"20"}),
        json!({"label":"private", "sortText":"10"}),
        json!({"label":"priority", "sortText":"99", "preselect":true}),
    ];
    assert_eq!(
        ranked_labels("pri", items.clone()),
        ["priority", "private", "print(value)"]
    );
    assert_eq!(
        ranked_labels("print", items),
        ["print(value)", "priority", "private"]
    );
}

#[test]
fn ranking_supports_unicode_empty_queries_and_stable_ties() {
    assert_eq!(
        ranked_labels(
            "об",
            vec![json!({"label":"Объект"}), json!({"label":"объект"})]
        ),
        ["объект", "Объект"]
    );
    let mut items = vec![
        json!({"label":"same", "detail":"first"}),
        json!({"label":"same", "detail":"second"}),
    ];
    rank_completions(
        &mut items,
        "",
        iced::widget::text_editor::Position { line: 0, column: 0 },
    );
    assert_eq!(items[0]["detail"], "first");
    assert_eq!(
        ranked_labels(
            "",
            vec![
                json!({"label":"z", "sortText":"01"}),
                json!({"label":"a", "sortText":"02"})
            ]
        ),
        ["z", "a"]
    );
}
