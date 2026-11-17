//! Complete environment variable names from the document and project templates.

use crate::domain::language::{Position, byte_offset};
use serde_json::{Value, json};
use std::{collections::BTreeMap, path::Path};

pub(super) async fn request(
    root: &Path,
    path: &Path,
    text: &str,
    position: Position,
    method: &str,
) -> Value {
    let mut names = BTreeMap::new();
    for filename in [".env.example", ".env.sample", ".env.template"] {
        let template = root.join(filename);
        if template == path {
            continue;
        }
        if let Ok(metadata) = tokio::fs::metadata(&template).await
            && metadata.len() <= 256 * 1024
            && let Ok(text) = tokio::fs::read_to_string(&template).await
        {
            collect(&text, &template, &mut names);
        }
    }
    collect(text, path, &mut names);
    let Some(offset) = byte_offset(text, position) else {
        return Value::Null;
    };
    let prefix: String = text[..offset]
        .chars()
        .rev()
        .take_while(|character| character.is_ascii_alphanumeric() || *character == '_')
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    if method == "textDocument/completion" {
        return json!(names.keys().filter(|name| name.starts_with(&prefix)).map(|name| json!({"label": name, "kind": 6, "textEdit": {"range": {"start": {"line": position.line, "character": position.character.saturating_sub(prefix.len() as u32)}, "end": position}, "newText": name}})).collect::<Vec<_>>());
    }
    if method == "textDocument/definition" {
        let suffix: String = text[offset..]
            .chars()
            .take_while(|character| character.is_ascii_alphanumeric() || *character == '_')
            .collect();
        if let Some((file, line, column)) = names.get(&format!("{prefix}{suffix}"))
            && let Ok(uri) = url::Url::from_file_path(file)
        {
            return json!({"uri": uri.as_str(), "range": {"start": {"line": line, "character": column}, "end": {"line": line, "character": column}}});
        }
    }
    json!([])
}

fn collect(
    text: &str,
    path: &Path,
    names: &mut BTreeMap<String, (std::path::PathBuf, usize, usize)>,
) {
    for (line_number, line) in text.lines().enumerate() {
        let source = line.trim_start();
        let source = source
            .strip_prefix("export ")
            .unwrap_or(source)
            .trim_start();
        let Some((name, _)) = source.split_once('=') else {
            continue;
        };
        let name = name.trim_end();
        if name.is_empty()
            || !name.chars().enumerate().all(|(index, character)| {
                character == '_'
                    || character.is_ascii_alphabetic()
                    || (index > 0 && character.is_ascii_digit())
            })
        {
            continue;
        }
        let column = line[..line.len() - source.len()].encode_utf16().count();
        names.insert(name.to_owned(), (path.to_owned(), line_number, column));
    }
}

#[cfg(test)]
mod tests;
