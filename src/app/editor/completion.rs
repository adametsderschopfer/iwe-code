//! Request completion candidates for an exact editor snapshot.

use super::{Event, Target};
use crate::{
    app::{App, Message},
    domain::language::Position,
};
use iced::Task;
use serde_json::Value;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub(crate) struct Suggestions {
    pub opened: std::time::Instant,
    pub revision: u64,
    pub tab: u64,
    pub snapshot: Arc<String>,
    pub cursor: iced::widget::text_editor::Position,
    pub items: Vec<Value>,
    pub selected: usize,
}

impl App {
    pub(super) fn schedule_completion(&self, tab: u64) -> Task<Message> {
        let Some(buffer) = self.editor_buffer() else {
            return Task::none();
        };
        let cursor = buffer.content.cursor().position;
        let prefix = buffer
            .document
            .text
            .split('\n')
            .nth(cursor.line)
            .and_then(|line| line.get(..cursor.column));
        if !prefix
            .and_then(|prefix| prefix.chars().last())
            .is_some_and(|character| {
                character.is_alphanumeric()
                    || matches!(character, '_' | '.' | ':' | '$' | '"' | '/' | '-')
            })
        {
            return Task::none();
        }
        let revision = self.editor.completion_revision;
        Task::perform(
            async move {
                tokio::time::sleep(std::time::Duration::from_millis(180)).await;
                (tab, revision)
            },
            |(tab, revision)| Message::Editor(Event::CompleteAfter(tab, revision)),
        )
    }

    pub(super) fn accept_completion(&mut self, index: usize) -> Task<Message> {
        let Some(suggestions) = self.editor.suggestions.take() else {
            return Task::none();
        };
        let Some(buffer) = self.editor_buffer() else {
            return Task::none();
        };
        if self.preview().id != suggestions.tab
            || buffer.document.text != suggestions.snapshot
            || buffer.content.cursor().position != suggestions.cursor
        {
            return Task::none();
        }
        let Some(item) = suggestions.items.get(index) else {
            return Task::none();
        };
        let Some(edits) = completion_edits(item, &suggestions.snapshot, suggestions.cursor) else {
            return Task::none();
        };
        let Some(text) = crate::domain::language::apply_edits(&suggestions.snapshot, &edits) else {
            return Task::none();
        };
        let Some(cursor) = inserted_cursor(&suggestions.snapshot, &text, &edits) else {
            return Task::none();
        };
        self.editor.completion_revision += 1;
        if let Some(buffer) = self
            .tabs
            .files
            .iter_mut()
            .find(|tab| tab.id == suggestions.tab)
            .and_then(|tab| tab.editor.as_mut())
        {
            buffer.document.edit(text);
            buffer
                .content
                .replace_text_preserving_scroll(&buffer.document.text, |editor| {
                    editor.buffer().scroll().line
                });
            buffer.content.move_to(iced::widget::text_editor::Cursor {
                position: cursor,
                selection: None,
            });
        }
        Task::batch([
            self.schedule_autosave(suggestions.tab),
            iced::widget::operation::focus(format!("text-editor-{}", suggestions.tab)),
        ])
    }

    pub(super) fn request_completion(&mut self, tab: u64) -> Task<Message> {
        self.editor.completion_revision += 1;
        let revision = self.editor.completion_revision;
        self.editor.suggestions = None;
        let Some(buffer) = self.editor_buffer() else {
            return Task::none();
        };
        if self.preview().id != tab {
            return Task::none();
        }
        let Target::Local(path) = &buffer.target else {
            return Task::none();
        };
        let Some(root) = self.workspace.directory() else {
            return Task::none();
        };
        let cursor = buffer.content.cursor().position;
        let snapshot = buffer.document.text.clone();
        let character = snapshot
            .split('\n')
            .nth(cursor.line)
            .unwrap_or_default()
            .get(..cursor.column)
            .unwrap_or_default()
            .encode_utf16()
            .count();
        let position = Position {
            line: cursor.line as u32,
            character: character as u32,
        };
        let root = root.to_owned();
        let path = path.clone();
        let service = Arc::clone(&self.editor.languages);
        let configured = self.workspace.preferences.language_servers.clone();
        let buffers = self.language_buffers();
        Task::perform(
            async move {
                let mut service = service.lock().await;
                service.configure(&configured);
                service.maintain(&root, buffers).await;
                let result = service
                    .request(&root, &path, &snapshot, position, "textDocument/completion")
                    .await;
                let mut items = result
                    .ok()
                    .and_then(|value| {
                        value
                            .as_array()
                            .cloned()
                            .or_else(|| value["items"].as_array().cloned())
                    })
                    .unwrap_or_default();
                rank_completions(&mut items, &snapshot, cursor);
                Suggestions {
                    opened: std::time::Instant::now(),
                    revision,
                    tab,
                    snapshot,
                    cursor,
                    items,
                    selected: 0,
                }
            },
            |suggestions| Message::Editor(Event::Completed(suggestions)),
        )
    }
}

/// Match the typed identifier before applying the server's semantic ordering.
fn rank_completions(items: &mut [Value], text: &str, cursor: iced::widget::text_editor::Position) {
    let prefix = text
        .split('\n')
        .nth(cursor.line)
        .and_then(|line| line.get(..cursor.column))
        .unwrap_or_default();
    let start = prefix
        .char_indices()
        .rev()
        .find(|(_, character)| {
            !character.is_alphanumeric() && *character != '_' && *character != '$'
        })
        .map_or(0, |(index, character)| index + character.len_utf8());
    let query = &prefix[start..];
    let folded = query.to_lowercase();
    // Stable sorting preserves server order when all ranking keys are equal.
    items.sort_by_cached_key(|item| {
        let label = item["label"].as_str().unwrap_or_default();
        let candidate = item["filterText"].as_str().unwrap_or(label);
        let folded_candidate = candidate.to_lowercase();
        let relevance = if query.is_empty() || candidate == query {
            0
        } else if candidate.starts_with(query) {
            1
        } else if folded_candidate == folded {
            2
        } else if folded_candidate.starts_with(&folded) {
            3
        } else {
            4
        };
        (
            relevance,
            !item["preselect"].as_bool().unwrap_or(false),
            item["sortText"].as_str().unwrap_or(label).to_owned(),
        )
    });
}

fn completion_edits(
    item: &Value,
    text: &str,
    cursor: iced::widget::text_editor::Position,
) -> Option<Vec<crate::domain::language::TextEdit>> {
    use crate::domain::language::{Range, TextEdit};
    // Servers receive snippetSupport=false until tab-stop editing is available.
    if item["insertTextFormat"].as_u64() == Some(2) {
        return None;
    }
    let primary = if let Some(edit) = item.get("textEdit") {
        if edit.get("range").is_some() {
            serde_json::from_value(edit.clone()).ok()?
        } else {
            TextEdit {
                range: serde_json::from_value(edit.get("replace")?.clone()).ok()?,
                new_text: edit["newText"].as_str()?.to_owned(),
            }
        }
    } else {
        let line = text.split('\n').nth(cursor.line)?;
        let prefix = line.get(..cursor.column)?;
        let word: String = prefix
            .chars()
            .rev()
            .take_while(|character| {
                character.is_alphanumeric() || *character == '_' || *character == '$'
            })
            .collect();
        let end = prefix.encode_utf16().count() as u32;
        let start = end - word.encode_utf16().count() as u32;
        TextEdit {
            range: Range {
                start: Position {
                    line: cursor.line as u32,
                    character: start,
                },
                end: Position {
                    line: cursor.line as u32,
                    character: end,
                },
            },
            new_text: item["insertText"]
                .as_str()
                .or_else(|| item["label"].as_str())?
                .to_owned(),
        }
    };
    let mut edits = vec![primary];
    if let Some(additional) = item.get("additionalTextEdits") {
        edits.extend(serde_json::from_value::<Vec<TextEdit>>(additional.clone()).ok()?);
    }
    Some(edits)
}

fn inserted_cursor(
    before: &str,
    after: &str,
    edits: &[crate::domain::language::TextEdit],
) -> Option<iced::widget::text_editor::Position> {
    use crate::domain::language::byte_offset;
    let primary = edits.first()?;
    let start = byte_offset(before, primary.range.start)?;
    let mut offset = start + primary.new_text.len();
    for edit in &edits[1..] {
        let end = byte_offset(before, edit.range.end)?;
        if end <= start {
            offset = offset
                .checked_sub(end - byte_offset(before, edit.range.start)?)?
                .checked_add(edit.new_text.len())?;
        }
    }
    let prefix = after.get(..offset)?;
    Some(iced::widget::text_editor::Position {
        line: prefix.bytes().filter(|byte| *byte == b'\n').count(),
        column: prefix.rsplit('\n').next()?.len(),
    })
}

#[cfg(test)]
mod tests;
