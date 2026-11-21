//! Coordinate text actions, asynchronous saves, and unsaved-change decisions.

use super::{App, Message};
use crate::{domain::editor::Document, i18n::Key, infrastructure::editor};
use iced::{Task, widget::text_editor};
use std::{path::PathBuf, sync::Arc, time::Duration};

const AUTOSAVE_DELAY: Duration = Duration::from_millis(800);

mod completion;
mod definition;
mod diagnostics;
mod formatting;
pub(crate) mod marks;
pub(crate) mod menu;

#[derive(Debug, Clone)]
pub(crate) enum Target {
    Local(PathBuf),
    Remote { profile: String, path: String },
}

#[derive(Debug, Clone)]
pub(crate) struct Buffer {
    pub document: Document,
    pub content: text_editor::Content,
    pub target: Target,
    pub marks: marks::Marks,
    pub saving: bool,
    pub error: Option<Key>,
    autosave: u64,
    token: u64,
    snapshot: Arc<String>,
}

impl Buffer {
    pub(crate) fn new(text: String, target: Target) -> Self {
        Self {
            content: text_editor::Content::with_text(&text),
            document: Document::new(text),
            target,
            marks: Default::default(),
            saving: false,
            error: None,
            autosave: 0,
            token: 0,
            snapshot: Arc::default(),
        }
    }
    pub(crate) fn protected(&self) -> bool {
        self.saving || self.document.dirty()
    }
}

#[derive(Default)]
pub(crate) struct State {
    pub menu: Option<menu::Menu>,
    pub rename: Option<menu::Rename>,
    pub server_checks: std::collections::BTreeMap<String, bool>,
    pub diagnostics: Option<diagnostics::Report>,
    diagnostics_busy: bool,
    pub definitions: Vec<definition::Definition>,
    definition_target: Option<(u64, u64, definition::Definition)>,
    completion_revision: u64,
    pub suggestions: Option<completion::Suggestions>,
    languages: Arc<tokio::sync::Mutex<crate::infrastructure::language_server::service::Service>>,
    pub pending: Option<Box<Message>>,
    pub saving_all: bool,
    next_token: u64,
    bypass: bool,
}

#[derive(Debug, Clone)]
pub(crate) enum Event {
    Menu(menu::Event),
    Marks(marks::Event),
    DuplicateLine,
    Format,
    Formatted(formatting::Report),
    CompletionFrame,
    LanguagesMaintained,
    DismissDefinitions,
    RefreshDiagnostics,
    Diagnostics(diagnostics::Report),
    Definition,
    Definitions(
        u64,
        u64,
        Arc<String>,
        text_editor::Position,
        Result<Vec<definition::Definition>, Key>,
    ),
    OpenDefinition(usize),
    CompleteAfter(u64, u64),
    AcceptCompletion(usize),
    SelectCompletion(bool),
    DismissCompletion,
    Complete(u64),
    Completed(completion::Suggestions),
    Action(u64, text_editor::Action),
    Undo(u64, bool),
    Replace(u64, String),
    Save,
    Saved(u64, Result<(), Key>),
    Autosave(u64, u64),
    SaveAll,
    FlushPending,
    Discard,
    Cancel,
}

impl App {
    fn duplicate_editor_line(&mut self) -> Task<Message> {
        if self.editor.pending.is_some() {
            return Task::none();
        }
        let Some(buffer) = self.editor_buffer() else {
            return Task::none();
        };
        let cursor = buffer.content.cursor().position;
        let source = &buffer.document.text;
        let mut start = 0;
        for _ in 0..cursor.line {
            let Some(next) = source[start..].find('\n') else {
                return Task::none();
            };
            start += next + 1;
        }
        let mut text = source.as_ref().clone();
        if let Some(length) = source[start..].find('\n') {
            let end = start + length + 1;
            text.insert_str(end, &source[start..end]);
        } else {
            let ending = if source.contains("\r\n") {
                "\r\n"
            } else {
                "\n"
            };
            text.push_str(ending);
            text.push_str(&source[start..]);
        }
        if text.len() > crate::domain::editor::LIMIT {
            self.notifications.error(Key::EditorTooLarge, None);
            return Task::none();
        }
        let id = self.preview().id;
        self.editor.suggestions = None;
        self.editor.completion_revision += 1;
        let task = self.update_editor(Event::Replace(id, text));
        if let Some(buffer) = self
            .preview_for_request(id)
            .and_then(|tab| tab.editor.as_mut())
        {
            buffer.content.move_to(text_editor::Cursor {
                position: text_editor::Position {
                    line: cursor.line + 1,
                    column: cursor.column,
                },
                selection: None,
            });
        }
        task
    }

    pub(super) fn language_buffers(&self) -> Vec<(PathBuf, Arc<String>)> {
        self.tabs
            .files
            .iter()
            .filter_map(|tab| {
                let buffer = tab.editor.as_ref()?;
                let Target::Local(path) = &buffer.target else {
                    return None;
                };
                Some((path.clone(), buffer.document.text.clone()))
            })
            .collect()
    }
    pub(crate) fn editor_buffer(&self) -> Option<&Buffer> {
        let tab = self.preview();
        tab.editor
            .as_ref()
            .filter(|_| !tab.local_svg() || tab.svg_source)
    }

    pub(crate) fn editor_pending_tab(&self, id: u64) -> bool {
        if let Some(path) = self
            .editor
            .pending
            .as_deref()
            .and_then(super::files::operations::request)
            .and_then(|request| request.source.as_ref())
        {
            return self.local_editor_at(id, path);
        }
        if let Some(path) = self
            .editor
            .pending
            .as_deref()
            .and_then(|message| self.deployment_local_path(message))
        {
            return self.tabs.files.iter().find(|tab| tab.id == id)
                .and_then(|tab| tab.editor.as_ref())
                .is_some_and(|buffer| matches!(&buffer.target, Target::Local(local) if local.starts_with(&path)));
        }
        match self.editor.pending.as_deref() {
            Some(Message::CloseTab(target)) => id == *target,
            Some(Message::CloseActiveTab) => id == self.preview().id,
            _ => true,
        }
    }

    fn local_editor_at(&self, id: u64, path: &std::path::Path) -> bool {
        self.tabs
            .files
            .iter()
            .find(|tab| tab.id == id)
            .and_then(|tab| tab.editor.as_ref())
            .is_some_and(
                |buffer| matches!(&buffer.target, Target::Local(local) if local.starts_with(path)),
            )
    }

    pub(super) fn protect_editor(&mut self, message: &Message) -> bool {
        if self.editor.bypass {
            self.editor.bypass = false;
            return false;
        }
        if let Some(request) = super::files::operations::request(message)
            && request.operation != crate::infrastructure::file_operations::Operation::Reveal
            && let Some(path) = &request.source
            && self.tabs.files.iter().any(|tab| {
                self.local_editor_at(tab.id, path)
                    && tab.editor.as_ref().is_some_and(Buffer::protected)
            })
        {
            if self.editor.pending.is_none() {
                self.editor.pending = Some(Box::new(message.clone()));
                self.editor.saving_all = true;
            }
            return true;
        }
        if let Some(path) = self.deployment_local_path(message) {
            let protected = self.tabs.files.iter().filter_map(|tab| tab.editor.as_ref())
                .any(|buffer| buffer.protected() && matches!(&buffer.target, Target::Local(local) if local.starts_with(&path)));
            if protected && self.editor.pending.is_none() {
                self.editor.pending = Some(Box::new(message.clone()));
            }
            return protected;
        }
        let target = match message {
            Message::CloseTab(id) => Some(*id),
            Message::CloseActiveTab => Some(self.preview().id),
            Message::SelectProject(..)
            | Message::RemoveProjectFromSpace
            | Message::Space(
                super::spaces::Message::Delete(_)
                | super::spaces::Message::Create
                | super::spaces::Message::RemoveProject(_)
                | super::spaces::Message::ProjectsPicked(..),
            )
            | Message::Update(super::updates::Event::Install)
            | Message::Module(super::modules::Event::SetEnabled(_, false)) => None,
            Message::Window(super::windows::Event::Close(id)) if self.window_id == Some(*id) => {
                None
            }
            Message::Module(super::modules::Event::Deployment(
                crate::modules::deployment::Event::SpaceSettings(_),
            )) => None,
            _ => return false,
        };
        if self.tabs.files.iter().any(|tab| {
            target.is_none_or(|id| tab.id == id)
                && tab.editor.as_ref().is_some_and(Buffer::protected)
        }) {
            if self.editor.pending.is_none() {
                self.editor.pending = Some(Box::new(message.clone()));
            }
            return true;
        }
        false
    }

    fn schedule_autosave(&mut self, id: u64) -> Task<Message> {
        self.editor.next_token += 1;
        let revision = self.editor.next_token;
        let Some(tab) = self.tabs.files.iter_mut().find(|tab| tab.id == id) else {
            return Task::none();
        };
        let Some(buffer) = &mut tab.editor else {
            return Task::none();
        };
        if buffer.document.dirty() {
            tab.temporary = false;
        }
        buffer.autosave = revision;
        buffer.marks.open = None;
        let save = if buffer.error.is_none() && buffer.document.dirty() {
            autosave_after(id, revision)
        } else {
            Task::none()
        };
        if self.changes.status.comparison_scope.is_none() {
            return save;
        }
        let marks = Task::perform(
            async { tokio::time::sleep(Duration::from_millis(180)).await },
            move |_| Message::Editor(Event::Marks(marks::Event::After(id, revision))),
        );
        Task::batch([save, marks])
    }

    pub(super) fn flush_editor(&mut self) -> Task<Message> {
        if self
            .editor
            .pending
            .as_deref()
            .and_then(super::files::operations::request)
            .is_some()
            && self.tabs.files.iter().any(|tab| {
                self.editor_pending_tab(tab.id)
                    && tab
                        .editor
                        .as_ref()
                        .is_some_and(|buffer| buffer.error.is_some())
            })
        {
            self.editor.pending = None;
            self.editor.saving_all = false;
            self.notifications.error(Key::FileSaveFirst, None);
            return Task::none();
        }
        if self.editor.pending.is_none()
            || self.tabs.files.iter().any(|tab| {
                self.editor_pending_tab(tab.id)
                    && tab
                        .editor
                        .as_ref()
                        .is_some_and(|buffer| buffer.error.is_some())
            })
        {
            return Task::none();
        }
        self.editor.saving_all = true;
        self.save_next_editor()
    }

    fn save_editor(&mut self, id: u64) -> Task<Message> {
        self.editor.next_token += 1;
        let token = self.editor.next_token;
        let Some(buffer) = self
            .tabs
            .files
            .iter_mut()
            .find(|tab| tab.id == id)
            .and_then(|tab| tab.editor.as_mut())
        else {
            return Task::none();
        };
        if buffer.saving || !buffer.document.dirty() {
            return Task::none();
        }
        buffer.token = token;
        buffer.snapshot = buffer.document.text.clone();
        buffer.saving = true;
        buffer.error = None;

        let text = buffer.snapshot.clone();
        let expected = buffer.document.saved.clone();
        match buffer.target.clone() {
            Target::Local(path) => Task::perform(
                async move {
                    tokio::task::spawn_blocking(move || editor::save(&path, &expected, &text))
                        .await
                        .unwrap_or(Err(editor::Error::Access))
                        .map_err(|error| match error {
                            editor::Error::Changed => Key::EditorConflict,
                            editor::Error::TooLarge => Key::EditorTooLarge,
                            editor::Error::Symlink => Key::EditorSymlink,
                            editor::Error::Access => Key::EditorSaveFailed,
                        })
                },
                move |result| Message::Editor(Event::Saved(token, result)),
            ),
            Target::Remote { profile, path, .. } => {
                if self.modules.deployment.busy {
                    return self.update_editor(Event::Saved(token, Err(Key::EditorBusy)));
                }
                if !self.module_enabled(crate::modules::ModuleId::Deployment) {
                    return self
                        .update_editor(Event::Saved(token, Err(Key::EditorDeploymentDisabled)));
                }
                self.update_module(super::modules::Event::Deployment(
                    crate::modules::deployment::Event::SaveText {
                        profile,
                        token,
                        path,
                        expected,
                        text,
                    },
                ))
            }
        }
    }

    fn continue_editor_action(&mut self) -> Task<Message> {
        if self.tabs.files.iter().any(|tab| {
            self.editor_pending_tab(tab.id) && tab.editor.as_ref().is_some_and(Buffer::protected)
        }) {
            return Task::none();
        }
        self.editor.saving_all = false;
        if let Some(message) = self.editor.pending.take() {
            self.editor.bypass = true;
            return self.update(*message);
        }
        Task::none()
    }

    pub(super) fn update_editor(&mut self, event: Event) -> Task<Message> {
        if self.files.operations.busy
            && !matches!(
                event,
                Event::Saved(..) | Event::Diagnostics(_) | Event::LanguagesMaintained
            )
        {
            if let Event::Autosave(id, revision) = event {
                return autosave_after(id, revision);
            }
            return Task::none();
        }
        if !matches!(
            event,
            Event::Saved(..)
                | Event::Diagnostics(_)
                | Event::LanguagesMaintained
                | Event::Autosave(..)
                | Event::FlushPending
                | Event::SaveAll
                | Event::Discard
                | Event::Cancel
        ) && (self.auth_dialog.is_some()
            || self.line_navigation.is_some()
            || self.space_draft.is_some()
            || self.project_add.is_some()
            || self.modules.git.git_actions.has_dialog()
            || self.modules.deployment.has_dialog()
            || self.navigation == super::Navigation::Settings)
        {
            return Task::none();
        }
        match event {
            Event::Menu(event) => return self.update_editor_menu(event),
            Event::Marks(event) => return self.update_marks(event),
            Event::DuplicateLine => return self.duplicate_editor_line(),
            Event::Format => return self.request_formatting(),
            Event::Formatted(report) => return self.apply_formatting(report),
            Event::CompletionFrame => {}
            Event::LanguagesMaintained => self.editor.diagnostics_busy = false,
            Event::DismissDefinitions => self.editor.definitions.clear(),
            Event::RefreshDiagnostics => return self.refresh_diagnostics(),
            Event::Diagnostics(report) => {
                self.editor.diagnostics_busy = false;
                if report.generation == self.generation
                    && report.tab == self.preview().id
                    && self
                        .editor_buffer()
                        .is_some_and(|buffer| buffer.document.text == report.text)
                {
                    self.editor.diagnostics = Some(report);
                }
            }
            Event::Definition => return self.request_locations(false),
            Event::OpenDefinition(index) => return self.open_definition(index),
            Event::Definitions(generation, tab, text, cursor, definitions) => {
                if self.generation == generation
                    && self.preview().id == tab
                    && self.editor_buffer().is_some_and(|buffer| {
                        buffer.document.text == text && buffer.content.cursor().position == cursor
                    })
                {
                    match definitions {
                        Err(key) => {
                            self.notifications.error(key, None);
                            return Task::none();
                        }
                        Ok(found) if found.is_empty() => {
                            self.notifications.success(Key::EditorNoLocations, None);
                            return Task::none();
                        }
                        Ok(found) => self.editor.definitions = found,
                    }
                    if self.editor.definitions.len() == 1 {
                        return self.open_definition(0);
                    }
                }
            }
            Event::AcceptCompletion(index) => return self.accept_completion(index),
            Event::DismissCompletion => {
                self.editor.completion_revision += 1;
                self.editor.suggestions = None;
            }
            Event::SelectCompletion(next) => {
                if let Some(suggestions) = &mut self.editor.suggestions {
                    let count = suggestions.items.len();
                    if count > 0 {
                        suggestions.selected = if next {
                            (suggestions.selected + 1) % count
                        } else {
                            (suggestions.selected + count - 1) % count
                        };
                        return iced::widget::operation::scroll_to(
                            "editor-completions",
                            iced::widget::operation::AbsoluteOffset {
                                x: 0.,
                                y: suggestions.selected.saturating_sub(4) as f32
                                    * crate::ui::theme::COMPLETION_ROW_HEIGHT,
                            },
                        );
                    }
                }
            }
            Event::Complete(id) => return self.request_completion(id),
            Event::CompleteAfter(id, revision) => {
                if self.editor.completion_revision == revision {
                    return self.request_completion(id);
                }
            }
            Event::Completed(suggestions) => {
                if self.editor.completion_revision == suggestions.revision
                    && self.preview().id == suggestions.tab
                    && self.editor_buffer().is_some_and(|buffer| {
                        buffer.document.text == suggestions.snapshot
                            && buffer.content.cursor().position == suggestions.cursor
                    })
                    && !suggestions.items.is_empty()
                {
                    self.editor.suggestions = Some(suggestions);
                }
            }
            Event::Replace(id, text) if self.editor.pending.is_none() => {
                if let Some(buffer) = self
                    .tabs
                    .files
                    .iter_mut()
                    .find(|tab| tab.id == id)
                    .and_then(|tab| tab.editor.as_mut())
                    && buffer.document.edit(text)
                {
                    buffer
                        .content
                        .replace_text_preserving_scroll(&buffer.document.text, |editor| {
                            editor.buffer().scroll().line
                        });
                }
                return Task::batch([
                    self.schedule_autosave(id),
                    if self.file_search().is_some() {
                        self.schedule_search(false)
                    } else {
                        Task::none()
                    },
                ]);
            }
            Event::Action(id, action) => {
                self.editor.completion_revision += 1;
                self.editor.suggestions = None;
                let edited = action.is_edit();
                if self.editor.pending.is_some()
                    || self.auth_dialog.is_some()
                    || self.line_navigation.is_some()
                    || self.space_draft.is_some()
                    || self.project_add.is_some()
                    || self.modules.git.git_actions.has_dialog()
                {
                    return Task::none();
                }
                if let Some(buffer) = self
                    .tabs
                    .files
                    .iter_mut()
                    .find(|tab| tab.id == id)
                    .and_then(|tab| tab.editor.as_mut())
                {
                    buffer.content.perform(action);
                    if edited {
                        let text = buffer.content.text();
                        if text.len() > crate::domain::editor::LIMIT {
                            buffer.content = text_editor::Content::with_text(&buffer.document.text);
                            buffer.error = Some(Key::EditorTooLarge);
                            self.notifications.error(Key::EditorTooLarge, None);
                        } else {
                            buffer.document.edit(text);
                        }
                    }
                }
                if edited {
                    return Task::batch([
                        self.schedule_completion(id),
                        self.schedule_autosave(id),
                        if self.file_search().is_some() {
                            self.schedule_search(false)
                        } else {
                            Task::none()
                        },
                    ]);
                }
            }
            Event::Undo(id, redo) if self.editor.pending.is_none() => {
                if let Some(buffer) = self
                    .tabs
                    .files
                    .iter_mut()
                    .find(|tab| tab.id == id)
                    .and_then(|tab| tab.editor.as_mut())
                    && buffer.document.undo(redo)
                {
                    buffer
                        .content
                        .replace_text_preserving_scroll(&buffer.document.text, |editor| {
                            editor.buffer().scroll().line
                        });
                    return Task::batch([
                        self.schedule_autosave(id),
                        if self.file_search().is_some() {
                            self.schedule_search(false)
                        } else {
                            Task::none()
                        },
                    ]);
                }
            }
            Event::Autosave(id, revision) => {
                let Some(buffer) = self
                    .tabs
                    .files
                    .iter()
                    .find(|tab| tab.id == id)
                    .and_then(|tab| tab.editor.as_ref())
                else {
                    return Task::none();
                };
                if buffer.autosave != revision
                    || revision == 0
                    || buffer.error.is_some()
                    || buffer.saving
                    || !buffer.document.dirty()
                    || self.editor.pending.is_some()
                {
                    return Task::none();
                }
                if matches!(buffer.target, Target::Remote { .. }) && self.modules.deployment.busy {
                    return autosave_after(id, revision);
                }
                return self.save_editor(id);
            }
            Event::Save if self.editor.pending.is_none() => {
                return self.save_editor(self.preview().id);
            }
            Event::Saved(token, result) => {
                let Some(tab) = self.tabs.files.iter_mut().find(|tab| {
                    tab.editor
                        .as_ref()
                        .is_some_and(|buffer| buffer.token == token && token != 0 && buffer.saving)
                }) else {
                    return Task::none();
                };
                let buffer = tab.editor.as_mut().expect("Matched editor");
                buffer.saving = false;
                buffer.error = result.err();

                if result.is_ok() {
                    buffer.document.saved.clone_from(&buffer.snapshot);
                } else {
                    self.editor.saving_all = false;
                }
                let id = tab.id;
                if let Err(error) = result {
                    self.notifications.error(error, Some(tab.title.clone()));
                    if self
                        .editor
                        .pending
                        .as_deref()
                        .and_then(super::files::operations::request)
                        .is_some()
                    {
                        return self.flush_editor();
                    }
                }
                let refresh = if result.is_ok() {
                    self.refresh_saved_editor(id)
                } else {
                    Task::none()
                };
                if self.editor.saving_all {
                    return Task::batch([refresh, self.save_next_editor()]);
                }
                return Task::batch([refresh, self.schedule_autosave(id)]);
            }
            Event::FlushPending if self.editor.saving_all => {
                return self.flush_editor();
            }
            Event::SaveAll => {
                self.editor.saving_all = true;
                return self.save_next_editor();
            }
            Event::Discard => {
                if self
                    .editor
                    .pending
                    .as_deref()
                    .and_then(super::files::operations::request)
                    .is_some()
                    || self
                        .tabs
                        .files
                        .iter()
                        .any(|tab| tab.editor.as_ref().is_some_and(|buffer| buffer.saving))
                {
                    return Task::none();
                }
                let target = self
                    .editor
                    .pending
                    .as_deref()
                    .and_then(|message| match message {
                        Message::CloseTab(id) => Some(*id),
                        Message::CloseActiveTab => Some(self.preview().id),
                        _ => None,
                    });
                for tab in &mut self.tabs.files {
                    if target.is_none_or(|id| tab.id == id)
                        && let Some(buffer) = &mut tab.editor
                    {
                        self.modules.deployment.forget_editor_save(buffer.token);
                        *buffer =
                            Buffer::new((*buffer.document.saved).clone(), buffer.target.clone());
                    }
                }
                self.editor.saving_all = false;
                if let Some(message) = self.editor.pending.take() {
                    self.editor.bypass = true;
                    return self.update(*message);
                }
            }
            Event::Cancel => {
                self.editor.pending = None;
                self.editor.saving_all = false;
            }
            _ => {}
        }
        Task::none()
    }

    fn save_next_editor(&mut self) -> Task<Message> {
        if self
            .tabs
            .files
            .iter()
            .any(|tab| tab.editor.as_ref().is_some_and(|buffer| buffer.saving))
        {
            return Task::none();
        }
        if let Some(id) = self
            .tabs
            .files
            .iter()
            .find(|tab| {
                self.editor_pending_tab(tab.id)
                    && tab
                        .editor
                        .as_ref()
                        .is_some_and(|buffer| buffer.document.dirty())
            })
            .map(|tab| tab.id)
        {
            let remote = self
                .tabs
                .files
                .iter()
                .find(|tab| tab.id == id)
                .and_then(|tab| tab.editor.as_ref())
                .is_some_and(|buffer| matches!(buffer.target, Target::Remote { .. }));
            if remote && self.modules.deployment.busy {
                return Task::perform(async { tokio::time::sleep(AUTOSAVE_DELAY).await }, |_| {
                    Message::Editor(Event::FlushPending)
                });
            }
            self.save_editor(id)
        } else {
            self.continue_editor_action()
        }
    }
}

fn autosave_after(id: u64, revision: u64) -> Task<Message> {
    Task::perform(
        async { tokio::time::sleep(AUTOSAVE_DELAY).await },
        move |_| Message::Editor(Event::Autosave(id, revision)),
    )
}

#[cfg(test)]
mod tests;
