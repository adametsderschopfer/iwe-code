//! Project fenced Markdown code into a document with matching line positions.

use super::{Language, Position};

pub(crate) struct Block {
    pub language: Language,
    pub extension: &'static str,
    pub start: usize,
    pub end: usize,
    pub text: String,
}

#[derive(Clone, Copy)]
struct Fence {
    marker: char,
    length: usize,
    start: usize,
    language: Option<(Language, &'static str)>,
}

pub(crate) fn at(text: &str, position: Position) -> Option<Block> {
    let lines: Vec<_> = text.split('\n').collect();
    let mut opening: Option<Fence> = None;
    for (index, line) in lines.iter().enumerate() {
        let trimmed = line.trim_start_matches(' ');
        if line.len() - trimmed.len() > 3 {
            continue;
        }
        let Some(marker) = trimmed
            .chars()
            .next()
            .filter(|value| matches!(value, '`' | '~'))
        else {
            continue;
        };
        let count = trimmed.chars().take_while(|value| *value == marker).count();
        if count < 3 {
            continue;
        }
        let tail = &trimmed[count..];
        if let Some(Fence {
            marker: fence,
            length,
            start,
            language,
        }) = opening
        {
            if marker == fence && count >= length && tail.trim().is_empty() {
                if position.line as usize >= start && (position.line as usize) < index {
                    return make_block(&lines, start, index, language?);
                }
                opening = None;
            }
        } else {
            opening = Some(Fence {
                marker,
                length: count,
                start: index + 1,
                language: language(tail.split_whitespace().next().unwrap_or_default()),
            });
        }
    }
    let Fence {
        start, language, ..
    } = opening?;
    if position.line as usize >= start && (position.line as usize) < lines.len() {
        return make_block(&lines, start, lines.len(), language?);
    }
    None
}

fn make_block(
    lines: &[&str],
    start: usize,
    end: usize,
    (language, extension): (Language, &'static str),
) -> Option<Block> {
    let text = lines
        .iter()
        .enumerate()
        .map(|(index, line)| {
            if (start..end).contains(&index) {
                *line
            } else {
                ""
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    Some(Block {
        language,
        extension,
        start,
        end,
        text,
    })
}

fn language(name: &str) -> Option<(Language, &'static str)> {
    Some(match name.to_ascii_lowercase().as_str() {
        "python" | "py" => (Language::Python, "py"),
        "go" | "golang" => (Language::Go, "go"),
        "javascript" | "js" => (Language::JavaScript, "js"),
        "typescript" | "ts" => (Language::TypeScript, "ts"),
        "jsx" => (Language::JavaScriptReact, "jsx"),
        "tsx" => (Language::TypeScriptReact, "tsx"),
        "rust" | "rs" => (Language::Rust, "rs"),
        "php" => (Language::Php, "php"),
        "json" => (Language::Json, "json"),
        "jsonc" => (Language::JsonComments, "jsonc"),
        "yaml" | "yml" => (Language::Yaml, "yaml"),
        "toml" => (Language::Toml, "toml"),
        "sh" | "bash" | "shell" => (Language::Shell, "sh"),
        "html" => (Language::Html, "html"),
        "css" => (Language::Css, "css"),
        _ => return None,
    })
}

#[cfg(test)]
mod tests;
