//! Apply project or server formatting only to the document snapshot that requested it.

use super::{Event, Target};
use crate::{
    app::{App, Message},
    domain::language::{Position, TextEdit, apply_edits},
    i18n::Key,
};
use iced::Task;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub(crate) struct Report {
    pub generation: u64,
    pub tab: u64,
    pub snapshot: String,
    pub edits: Result<Vec<TextEdit>, Key>,
}

impl App {
    pub(super) fn request_formatting(&mut self) -> Task<Message> {
        let Some(buffer) = self.editor_buffer() else {
            return Task::none();
        };
        let Target::Local(path) = &buffer.target else {
            return Task::none();
        };
        let Some(root) = self.workspace.directory() else {
            return Task::none();
        };
        let path = path.clone();
        let root = root.clone();
        let snapshot = buffer.document.text.clone();
        let tab = self.preview().id;
        let generation = self.generation;
        let service = Arc::clone(&self.editor.languages);
        let configured = self.workspace.preferences.language_servers.clone();
        let buffers = self.language_buffers();
        Task::perform(
            async move {
                let result = match crate::infrastructure::formatter::format(&root, &path, &snapshot)
                    .await
                {
                    Ok(Some(text)) => Ok(vec![TextEdit {
                        range: crate::domain::language::Range {
                            start: Position {
                                line: 0,
                                character: 0,
                            },
                            end: Position {
                                line: snapshot.bytes().filter(|byte| *byte == b'\n').count() as u32,
                                character: snapshot
                                    .rsplit('\n')
                                    .next()
                                    .unwrap_or_default()
                                    .trim_end_matches('\r')
                                    .encode_utf16()
                                    .count() as u32,
                            },
                        },
                        new_text: text,
                    }]),
                    Ok(None) => {
                        let mut service = service.lock().await;
                        service.configure(&configured);
                        service.maintain(&root, buffers).await;
                        service
                            .request(
                                &root,
                                &path,
                                &snapshot,
                                Position {
                                    line: 0,
                                    character: 0,
                                },
                                "textDocument/formatting",
                            )
                            .await
                            .and_then(|value| {
                                if value.is_null() {
                                    Ok(Vec::new())
                                } else {
                                    serde_json::from_value(value).map_err(std::io::Error::other)
                                }
                            })
                    }
                    Err(error) => Err(error),
                };
                let edits = result.map_err(|error| match error.kind() {
                    std::io::ErrorKind::Unsupported | std::io::ErrorKind::NotFound => {
                        Key::FormatUnavailable
                    }
                    _ => Key::FormatFailed,
                });
                Report {
                    generation,
                    tab,
                    snapshot,
                    edits,
                }
            },
            |report| Message::Editor(Event::Formatted(report)),
        )
    }

    pub(super) fn apply_formatting(&mut self, report: Report) -> Task<Message> {
        if self.generation != report.generation
            || self.preview().id != report.tab
            || self
                .editor_buffer()
                .is_none_or(|buffer| buffer.document.text != report.snapshot)
        {
            return Task::none();
        }
        let text = report
            .edits
            .and_then(|edits| apply_edits(&report.snapshot, &edits).ok_or(Key::FormatFailed));
        let text = match text {
            Ok(text) => text,
            Err(error) => {
                self.notifications.error(error, None);
                return Task::none();
            }
        };
        if text == report.snapshot {
            return Task::none();
        }
        self.editor.completion_revision += 1;
        self.editor.suggestions = None;
        self.editor.diagnostics = None;
        self.update_editor(Event::Replace(report.tab, text))
    }
}
