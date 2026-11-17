//! Refresh diagnostics for the active local document.

use super::{Event, Target};
use crate::{
    app::{App, Message},
    domain::language::{Diagnostic, Position},
};
use iced::Task;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub(crate) struct Report {
    pub generation: u64,
    pub tab: u64,
    pub text: String,
    pub diagnostics: Vec<Diagnostic>,
}

impl App {
    pub(super) fn refresh_diagnostics(&mut self) -> Task<Message> {
        if self.editor.diagnostics_busy || self.editor.pending.is_some() {
            return Task::none();
        }
        let buffers = self.language_buffers();
        let Some(buffer) = self.editor_buffer() else {
            let service = Arc::clone(&self.editor.languages);
            let root = self.workspace.directory().cloned().unwrap_or_default();
            self.editor.diagnostics_busy = true;
            return Task::perform(
                async move { service.lock().await.maintain(&root, buffers).await },
                |_| Message::Editor(Event::LanguagesMaintained),
            );
        };
        let Target::Local(path) = &buffer.target else {
            return Task::none();
        };
        let Some(root) = self.workspace.directory() else {
            return Task::none();
        };
        let path = path.clone();
        let root = root.to_owned();
        let text = buffer.document.text.clone();
        let tab = self.preview().id;
        let generation = self.generation;
        let service = Arc::clone(&self.editor.languages);
        let configured = self.workspace.preferences.language_servers.clone();
        self.editor.diagnostics_busy = true;
        Task::perform(
            async move {
                let mut service = service.lock().await;
                service.configure(&configured);
                service.maintain(&root, buffers).await;
                let result = service
                    .request(
                        &root,
                        &path,
                        &text,
                        Position {
                            line: 0,
                            character: 0,
                        },
                        "textDocument/diagnostic",
                    )
                    .await;
                let diagnostics = result
                    .ok()
                    .and_then(|value| serde_json::from_value(value).ok())
                    .unwrap_or_default();
                Report {
                    generation,
                    tab,
                    text,
                    diagnostics,
                }
            },
            |report| Message::Editor(Event::Diagnostics(report)),
        )
    }
}
