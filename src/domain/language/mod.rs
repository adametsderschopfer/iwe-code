//! Describe language documents and validate edits independently of the editor widget.

use serde::{Deserialize, Serialize};
use std::path::Path;
pub(crate) mod embedded;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub(crate) enum Language {
    Python,
    Go,
    JavaScript,
    JavaScriptReact,
    TypeScript,
    TypeScriptReact,
    Php,
    Rust,
    Markdown,
    Json,
    JsonComments,
    Yaml,
    Toml,
    Env,
    Shell,
    Html,
    Css,
}

impl Language {
    pub(crate) fn for_path(path: &Path) -> Option<Self> {
        let name = path.file_name()?.to_str()?;
        if name == ".env" || name.starts_with(".env.") || name.ends_with(".env") {
            return Some(Self::Env);
        }
        match name {
            ".bashrc" | ".zshrc" | ".profile" => return Some(Self::Shell),
            ".babelrc" | ".eslintrc" | ".prettierrc" => return Some(Self::JsonComments),
            _ => {}
        }
        Some(
            match path.extension()?.to_str()?.to_ascii_lowercase().as_str() {
                "py" | "pyi" => Self::Python,
                "go" => Self::Go,
                "js" | "mjs" | "cjs" => Self::JavaScript,
                "jsx" => Self::JavaScriptReact,
                "ts" | "mts" | "cts" => Self::TypeScript,
                "tsx" => Self::TypeScriptReact,
                "php" | "phtml" => Self::Php,
                "rs" => Self::Rust,
                "md" | "markdown" => Self::Markdown,
                "json" => Self::Json,
                "jsonc" => Self::JsonComments,
                "yaml" | "yml" => Self::Yaml,
                "toml" => Self::Toml,
                "sh" | "bash" | "zsh" => Self::Shell,
                "html" | "htm" => Self::Html,
                "css" => Self::Css,
                _ => return None,
            },
        )
    }

    pub(crate) fn id(self) -> &'static str {
        match self {
            Self::Python => "python",
            Self::Go => "go",
            Self::JavaScript => "javascript",
            Self::JavaScriptReact => "javascriptreact",
            Self::TypeScript => "typescript",
            Self::TypeScriptReact => "typescriptreact",
            Self::Php => "php",
            Self::Rust => "rust",
            Self::Markdown => "markdown",
            Self::Json => "json",
            Self::JsonComments => "jsonc",
            Self::Yaml => "yaml",
            Self::Toml => "toml",
            Self::Env => "dotenv",
            Self::Shell => "shellscript",
            Self::Html => "html",
            Self::Css => "css",
        }
    }
}

/// LSP positions use UTF-16 code units until a different encoding is negotiated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Position {
    pub line: u32,
    pub character: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Range {
    pub start: Position,
    pub end: Position,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TextEdit {
    pub range: Range,
    pub new_text: String,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct Diagnostic {
    pub range: Range,
    pub severity: Option<u8>,
    pub message: String,
}

pub(crate) fn byte_offset(text: &str, position: Position) -> Option<usize> {
    let mut start = 0;
    for _ in 0..position.line {
        start += text.get(start..)?.find('\n')? + 1;
    }
    let line = text
        .get(start..)?
        .split('\n')
        .next()?
        .trim_end_matches('\r');
    let mut units = 0;
    for (offset, character) in line.char_indices() {
        if units == position.character {
            return Some(start + offset);
        }
        units += character.len_utf16() as u32;
    }
    (units == position.character).then_some(start + line.len())
}

/// Validate every edit before constructing a replacement document.
pub(crate) fn apply_edits(text: &str, edits: &[TextEdit]) -> Option<String> {
    let mut ranges = Vec::with_capacity(edits.len());
    let mut size = text.len();
    for edit in edits {
        let start = byte_offset(text, edit.range.start)?;
        let end = byte_offset(text, edit.range.end)?;
        size = size
            .checked_sub(end.checked_sub(start)?)?
            .checked_add(edit.new_text.len())?;
        ranges.push((start, end, &edit.new_text));
    }
    if size > super::editor::LIMIT {
        return None;
    }
    ranges.sort_by_key(|(start, end, _)| (*start, *end));
    if ranges
        .windows(2)
        .any(|pair| pair[0].1 > pair[1].0 || (pair[0].0 == pair[1].0 && pair[1].0 == pair[1].1))
    {
        return None;
    }
    let mut result = text.to_owned();
    for (start, end, replacement) in ranges.into_iter().rev() {
        result.replace_range(start..end, replacement);
    }
    Some(result)
}

#[cfg(test)]
mod tests;
