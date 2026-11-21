//! Navigate to language-server definitions using existing file tabs.

use super::{Event, Target};
use crate::{
    app::{App, Message},
    domain::language::Position,
};
use iced::{Task, widget::text_editor};
use serde_json::Value;
use std::{path::PathBuf, sync::Arc};

#[derive(Debug, Clone)]
pub(crate) struct Definition {
    pub path: PathBuf,
    pub position: Position,
}

impl App {
    pub(super) fn request_locations(&mut self, references: bool) -> Task<Message> {
        let Some(buffer) = self.editor_buffer() else {
            return Task::none();
        };
        let Target::Local(path) = &buffer.target else {
            return Task::none();
        };
        let Some(root) = self.workspace.directory() else {
            return Task::none();
        };
        let tab = self.preview().id;
        let cursor = buffer.content.cursor().position;
        let text = buffer.document.text.clone();
        let character = text
            .split('\n')
            .nth(cursor.line)
            .and_then(|line| line.get(..cursor.column))
            .unwrap_or_default()
            .encode_utf16()
            .count();
        let position = Position {
            line: cursor.line as u32,
            character: character as u32,
        };
        let path = path.clone();
        let root = root.to_owned();
        let generation = self.generation;
        let service = Arc::clone(&self.editor.languages);
        let configured = self.workspace.preferences.language_servers.clone();
        let buffers = self.language_buffers();
        Task::perform(
            async move {
                let mut service = service.lock().await;
                service.configure(&configured);
                service.maintain(&root, buffers).await;
                let method = if references {
                    "textDocument/references"
                } else {
                    "textDocument/definition"
                };
                let result = service.request(&root, &path, &text, position, method).await;
                let definitions = result
                    .map(parse)
                    .map_err(|_| crate::i18n::Key::EditorLanguageUnavailable);
                (generation, tab, text, cursor, definitions)
            },
            |(generation, tab, text, cursor, definitions)| {
                Message::Editor(Event::Definitions(
                    generation,
                    tab,
                    text,
                    cursor,
                    definitions,
                ))
            },
        )
    }

    pub(super) fn open_definition(&mut self, index: usize) -> Task<Message> {
        let Some(target) = self.editor.definitions.get(index).cloned() else {
            return Task::none();
        };
        self.editor.definitions.clear();
        let task = self.open_file(target.path.clone());
        self.editor.definition_target = Some((self.generation, self.preview().id, target));
        Task::batch([task, self.reveal_definition()])
    }

    pub(crate) fn reveal_definition(&mut self) -> Task<Message> {
        let Some((generation, tab, target)) = self.editor.definition_target.clone() else {
            return Task::none();
        };
        if generation != self.generation || tab != self.preview().id {
            self.editor.definition_target = None;
            return Task::none();
        }
        let Some(buffer) = self
            .preview_for_request(tab)
            .and_then(|preview| preview.editor.as_mut())
        else {
            return Task::none();
        };
        if !matches!(&buffer.target, Target::Local(path) if *path == target.path) {
            return Task::none();
        }
        let Some(offset) =
            crate::domain::language::byte_offset(&buffer.document.text, target.position)
        else {
            self.editor.definition_target = None;
            return Task::none();
        };
        let column = buffer.document.text[..offset]
            .rsplit('\n')
            .next()
            .unwrap_or_default()
            .len();
        buffer.content.move_to(text_editor::Cursor {
            position: text_editor::Position {
                line: target.position.line as usize,
                column,
            },
            selection: None,
        });
        self.editor.definition_target = None;
        iced::widget::operation::focus(format!("text-editor-{tab}"))
    }
}

fn parse(value: Value) -> Vec<Definition> {
    let values = match value {
        Value::Array(values) => values,
        Value::Null => return Vec::new(),
        value => vec![value],
    };
    values
        .into_iter()
        .filter_map(|value| {
            let uri = value["uri"]
                .as_str()
                .or_else(|| value["targetUri"].as_str())?;
            let path = url::Url::parse(uri).ok()?.to_file_path().ok()?;
            let position = serde_json::from_value(
                value
                    .get("targetSelectionRange")
                    .or_else(|| value.get("range"))?["start"]
                    .clone(),
            )
            .ok()?;
            Some(Definition { path, position })
        })
        .collect()
}
