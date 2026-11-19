//! Refresh diagnostics for the active local document.

use super::{Event, Target};
use crate::{
    app::{App, Message},
    domain::language::{Diagnostic, Language, Position},
};
use iced::{Subscription, Task};
use std::{path::Path, sync::Arc, time::Duration};

#[derive(Debug, Clone)]
pub(crate) struct Report {
    pub generation: u64,
    pub tab: u64,
    pub text: Arc<String>,
    pub diagnostics: Vec<Diagnostic>,
    pub failed: bool,
}

impl App {
    pub(in crate::app) fn diagnostics_subscription(&self) -> Subscription<Message> {
        match self.diagnostics_interval() {
            Some(delay) => {
                iced::time::every(delay).map(|_| Message::Editor(Event::RefreshDiagnostics))
            }
            None => Subscription::none(),
        }
    }

    pub(super) fn diagnostics_interval(&self) -> Option<Duration> {
        let normal = Duration::from_millis(800);
        if self.diagnostic_document().is_none() {
            return self.languages_need_maintenance().then_some(normal);
        }
        let failed = self.editor.diagnostics.as_ref().is_some_and(|report| {
            report.failed && report.generation == self.generation && report.tab == self.preview().id
        });
        Some(if failed && !self.languages_need_maintenance() {
            Duration::from_secs(30)
        } else {
            normal
        })
    }

    fn diagnostic_document(&self) -> Option<(&Path, &Arc<String>)> {
        self.workspace.directory()?;
        let buffer = self.editor_buffer()?;
        let Target::Local(path) = &buffer.target else {
            return None;
        };
        Language::for_path(path).filter(|language| *language != Language::Env)?;
        Some((path, &buffer.document.text))
    }

    fn languages_need_maintenance(&self) -> bool {
        self.editor.languages.try_lock().is_ok_and(|service| {
            service.needs_maintenance(
                self.workspace.directory().map(|root| root.as_path()),
                |path| {
                    self.tabs.files.iter().any(|tab| {
                        tab.editor.as_ref().is_some_and(|buffer| {
                        matches!(&buffer.target, Target::Local(local) if local == path)
                    })
                    })
                },
            )
        })
    }

    pub(super) fn refresh_diagnostics(&mut self) -> Task<Message> {
        if self.editor.diagnostics_busy || self.editor.pending.is_some() {
            return Task::none();
        }
        let root = self.workspace.directory().cloned().unwrap_or_default();
        let Some((path, text)) = self.diagnostic_document() else {
            if !self.languages_need_maintenance() {
                return Task::none();
            }
            let buffers = self.language_buffers();
            let service = Arc::clone(&self.editor.languages);
            self.editor.diagnostics_busy = true;
            return Task::perform(
                async move { service.lock().await.maintain(&root, buffers).await },
                |_| Message::Editor(Event::LanguagesMaintained),
            );
        };
        let (path, text) = (path.to_owned(), Arc::clone(text));
        let buffers = self.language_buffers();
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
                let failed = result.is_err();
                let diagnostics = result
                    .ok()
                    .and_then(|value| serde_json::from_value(value).ok())
                    .unwrap_or_default();
                Report {
                    generation,
                    tab,
                    text,
                    diagnostics,
                    failed,
                }
            },
            |report| Message::Editor(Event::Diagnostics(report)),
        )
    }
}
