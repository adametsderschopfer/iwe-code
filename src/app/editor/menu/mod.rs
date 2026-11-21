//! Resolve editor commands against the clicked buffer, selection and language.

use super::{App, Message, Target};
use crate::{
    domain::{
        language::{self, Language, TextEdit},
        shortcuts::Command,
    },
    i18n::Key,
};
use iced::{
    Point, Task,
    widget::text_editor::{self, Position},
};
use std::{path::Path, sync::Arc};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Action {
    Undo,
    Redo,
    Cut,
    Copy,
    Paste,
    All,
    Duplicate,
    Find,
    FindProject,
    Line,
    Definition,
    Usages,
    Rename,
    Format,
}

impl Action {
    pub(crate) fn key(self) -> Key {
        match self {
            Self::Undo => Key::EditorUndo,
            Self::Redo => Key::EditorRedo,
            Self::Cut => Key::FileCut,
            Self::Copy => Key::FileCopy,
            Self::Paste => Key::FilePaste,
            Self::All => Key::SelectAllCode,
            Self::Duplicate => Key::DuplicateLine,
            Self::Find => Key::FindFile,
            Self::FindProject => Key::FindFiles,
            Self::Line => Key::GoToLine,
            Self::Definition => Key::GoToDefinition,
            Self::Usages => Key::EditorUsages,
            Self::Rename => Key::EditorRenameLocal,
            Self::Format => Key::FormatCode,
        }
    }
    pub(crate) fn shortcut(self, app: &App) -> String {
        let cmd = match self {
            Self::Duplicate => Command::DuplicateLine,
            Self::All => Command::SelectAll,
            Self::Copy => Command::Copy,
            Self::Find => Command::Search,
            Self::FindProject => Command::FindInFiles,
            Self::Line => Command::GoToLine,
            Self::Definition => Command::GoToDefinition,
            Self::Format => Command::FormatCode,
            Self::Undo | Self::Redo | Self::Cut | Self::Paste => {
                let prefix = if cfg!(target_os = "macos") {
                    "⌘"
                } else {
                    "Ctrl+"
                };
                let key = match self {
                    Self::Undo => "Z",
                    Self::Redo => "Shift+Z",
                    Self::Cut => "X",
                    _ => "V",
                };
                return format!("{prefix}{key}");
            }
            _ => return String::new(),
        };
        let bindings = &app.workspace.preferences.shortcuts;
        if bindings.conflict(cmd) {
            String::new()
        } else {
            bindings.binding(cmd).label()
        }
    }
}

pub(crate) const SCROLL_ID: &str = "editor-context-actions";

pub(crate) struct Menu {
    pub tab: u64,
    pub point: Point,
    pub selected: usize,
    symbol: Option<String>,
}

pub(crate) struct Rename {
    pub tab: u64,
    pub value: String,
    pub symbol: String,
    snapshot: Arc<String>,
    position: Position,
}

#[derive(Debug, Clone)]
pub(crate) enum Event {
    Open(u64, Point, Option<Position>),
    OpenViewer(u64, Point),
    Close,
    Step(bool),
    Choose(Action),
    Paste(u64, Arc<String>, text_editor::Cursor, Option<String>),
    RenameInput(String),
    RenameCancel,
    RenameSubmit,
    Renamed(u64, u64, Arc<String>, Result<String, Key>),
}

pub(crate) fn symbol_at(line: &str, column: usize) -> Option<&str> {
    let word = |c: char| c.is_alphanumeric() || c == '_' || c == '$';
    let left = line.get(..column)?;
    let right = line.get(column..)?;
    let start = left.trim_end_matches(word).len();
    let end = line.len() - right.trim_start_matches(word).len();
    (end > start).then(|| &line[start..end])
}

impl App {
    pub(crate) fn editor_menu_items(&self, menu: &Menu) -> Vec<Action> {
        use Action::*;
        let Some(buffer) = self.editor_buffer() else {
            let Some(document) = &self.preview().document else {
                return vec![];
            };
            let mut items = vec![];
            if self.preview().selection.has_text(document) {
                items.push(Copy);
            }
            items.push(All);
            if self.workspace.directory().is_some() {
                items.extend([Find, FindProject]);
            }
            items.push(Line);
            return items;
        };
        let language = match &buffer.target {
            Target::Local(path) => Language::for_path(path),
            Target::Remote { .. } => None,
        };
        let has_project = self.workspace.directory().is_some();
        let supported = has_project && language.is_some();
        let symbol = supported && menu.symbol.is_some();
        let format = supported && language != Some(Language::Env);
        let rename = symbol && !matches!(language, Some(Language::Env | Language::Markdown));
        let mut items = Vec::new();
        if buffer.document.can_undo(false) {
            items.push(Undo);
        }
        if buffer.document.can_undo(true) {
            items.push(Redo);
        }
        if buffer.content.cursor().selection.is_some() {
            items.extend([Cut, Copy]);
        }
        items.extend([Paste, All, Duplicate]);
        if has_project {
            items.extend([Find, FindProject]);
        }
        items.push(Line);
        if symbol {
            items.push(Definition);
            if language != Some(Language::Env) {
                items.push(Usages);
            }
        }
        if rename {
            items.push(Rename);
        }
        if format {
            items.push(Format);
        }
        items
    }

    pub(super) fn update_editor_menu(&mut self, event: Event) -> Task<Message> {
        use Action::*;
        let id = self.preview().id;
        match event {
            Event::OpenViewer(tab, point)
                if tab == id
                    && self.preview().document.is_some()
                    && self.editor.pending.is_none() =>
            {
                self.editor.menu = Some(Menu {
                    tab,
                    point,
                    selected: 0,
                    symbol: None,
                });
            }
            Event::Open(tab, point, hit) if tab == id && self.editor.pending.is_none() => {
                let Some(buffer) = self
                    .preview_for_request(tab)
                    .and_then(|tab| tab.editor.as_mut())
                else {
                    return Task::none();
                };
                let cursor = buffer.content.cursor();
                if let Some(hit) = hit {
                    let pair = |p: Position| (p.line, p.column);
                    let inside = cursor.selection.is_some_and(|anchor| {
                        let (a, b) = (pair(anchor), pair(cursor.position));
                        (a.min(b)..a.max(b)).contains(&pair(hit))
                    });
                    if !inside {
                        buffer.content.move_to(text_editor::Cursor {
                            position: hit,
                            selection: None,
                        });
                    }
                }
                let pos = buffer.content.cursor().position;
                let symbol = buffer
                    .content
                    .line(pos.line)
                    .and_then(|line| symbol_at(&line.text, pos.column).map(str::to_owned));
                buffer.marks.open = None;
                self.editor.suggestions = None;
                self.editor.definitions.clear();
                self.editor.menu = Some(Menu {
                    tab,
                    point,
                    selected: 0,
                    symbol,
                });
                return iced::widget::operation::focus(format!("text-editor-{id}"));
            }
            Event::Close => self.editor.menu = None,
            Event::Step(next) => {
                if let Some(menu) = &self.editor.menu {
                    let len = self.editor_menu_items(menu).len();
                    if len > 0 {
                        let selected = if next {
                            (menu.selected + 1) % len
                        } else {
                            (menu.selected + len - 1) % len
                        };
                        if let Some(menu) = &mut self.editor.menu {
                            menu.selected = selected;
                            return iced::widget::operation::snap_to(
                                SCROLL_ID,
                                iced::widget::operation::RelativeOffset {
                                    x: None,
                                    y: Some(selected as f32 / len.saturating_sub(1).max(1) as f32),
                                },
                            );
                        }
                    }
                }
            }
            Event::Choose(action) => {
                let Some(menu) = self.editor.menu.take().filter(|menu| menu.tab == id) else {
                    return Task::none();
                };
                if !self.editor_menu_items(&menu).contains(&action) {
                    return Task::none();
                }
                let Some(buffer) = self.editor_buffer() else {
                    use crate::ui::code::selection::{Action as Select, Lane};
                    let Some(document) = &self.preview().document else {
                        return Task::none();
                    };
                    return match action {
                        Copy => self
                            .preview()
                            .selection
                            .copy(document)
                            .map(iced::clipboard::write)
                            .unwrap_or_else(Task::none),
                        All => {
                            let lane = self.preview().selection.lane().unwrap_or(
                                if document.comparison.is_some()
                                    && self.workspace.preferences.diff_view
                                        == crate::domain::workspace::DiffView::SideBySide
                                {
                                    Lane::After
                                } else {
                                    Lane::Unified
                                },
                            );
                            self.update(Message::CodeSelection(id, Select::All(lane)))
                        }
                        Find | FindProject => self.update_text_search(
                            crate::app::text_search::Event::Open(action == FindProject),
                        ),
                        Line => {
                            self.update_line_navigation(crate::app::line_navigation::Event::Open)
                        }
                        _ => Task::none(),
                    };
                };
                let dispatch = |action| super::Event::Action(id, action);
                match action {
                    Undo | Redo => {
                        return self.update_editor(super::Event::Undo(id, action == Redo));
                    }
                    Cut | Copy => {
                        if let Some(value) = buffer.content.selection() {
                            let copy = iced::clipboard::write(value);
                            if action == Cut {
                                return Task::batch([
                                    copy,
                                    self.update_editor(dispatch(text_editor::Action::Edit(
                                        text_editor::Edit::Delete,
                                    ))),
                                ]);
                            }
                            return copy;
                        }
                    }
                    Paste => {
                        let snapshot = buffer.document.text.clone();
                        let cursor = buffer.content.cursor();
                        return iced::clipboard::read().map(move |value| {
                            message(Event::Paste(id, snapshot.clone(), cursor, value))
                        });
                    }
                    All => return self.update_editor(dispatch(text_editor::Action::SelectAll)),
                    Duplicate => return self.update_editor(super::Event::DuplicateLine),
                    Find | FindProject => {
                        return self.update_text_search(crate::app::text_search::Event::Open(
                            action == FindProject,
                        ));
                    }
                    Line => {
                        return self
                            .update_line_navigation(crate::app::line_navigation::Event::Open);
                    }
                    Definition | Usages => return self.request_locations(action == Usages),
                    Format => return self.request_formatting(),
                    Rename => {
                        let Some(symbol) = menu.symbol else {
                            return Task::none();
                        };
                        self.editor.rename = Some(self::Rename {
                            tab: id,
                            value: symbol.clone(),
                            symbol,
                            snapshot: buffer.document.text.clone(),
                            position: buffer.content.cursor().position,
                        });
                        return iced::widget::operation::focus("editor-rename");
                    }
                }
            }
            Event::Paste(tab, snapshot, cursor, Some(value))
                if tab == id
                    && self.editor_buffer().is_some_and(|buffer| {
                        buffer.document.text == snapshot && buffer.content.cursor() == cursor
                    }) =>
            {
                return self.update_editor(super::Event::Action(
                    id,
                    text_editor::Action::Edit(text_editor::Edit::Paste(Arc::new(value))),
                ));
            }
            Event::RenameInput(value) => {
                if let Some(rename) = &mut self.editor.rename {
                    rename.value = value;
                }
            }
            Event::RenameCancel => self.editor.rename = None,
            Event::RenameSubmit => return self.rename_local(),
            Event::Renamed(generation, tab, snapshot, result)
                if generation == self.generation
                    && self
                        .editor_buffer()
                        .is_some_and(|buffer| tab == id && buffer.document.text == snapshot) =>
            {
                match result {
                    Ok(text) => return self.update_editor(super::Event::Replace(tab, text)),
                    Err(key) => self.notifications.error(key, None),
                }
            }
            _ => {}
        }
        Task::none()
    }

    fn rename_local(&mut self) -> Task<Message> {
        let Some(form) = self.editor.rename.take() else {
            return Task::none();
        };
        let Some(buffer) = self.editor_buffer() else {
            return Task::none();
        };
        let Target::Local(path) = &buffer.target else {
            return Task::none();
        };
        let Some(root) = self.workspace.directory().cloned() else {
            return Task::none();
        };
        if self.preview().id != form.tab
            || buffer.document.text != form.snapshot
            || form.value.trim().is_empty()
            || form.value == form.symbol
        {
            return Task::none();
        }
        let path = path.clone();
        let service = self.editor.languages.clone();
        let configured = self.workspace.preferences.language_servers.clone();
        let buffers = self.language_buffers();
        let generation = self.generation;
        let tab = form.tab;
        let snapshot = form.snapshot;
        let position = language::Position {
            line: form.position.line as u32,
            character: snapshot
                .split('\n')
                .nth(form.position.line)
                .and_then(|line| line.get(..form.position.column))
                .unwrap_or_default()
                .encode_utf16()
                .count() as u32,
        };
        Task::perform(
            async move {
                let mut service = service.lock().await;
                service.configure(&configured);
                service.maintain(&root, buffers).await;
                let result = service
                    .request(
                        &root,
                        &path,
                        &snapshot,
                        position,
                        crate::infrastructure::language_server::service::Query::Rename(
                            form.value.trim(),
                        ),
                    )
                    .await;
                let result = result
                    .map_err(|_| Key::EditorLanguageUnavailable)
                    .and_then(|edit| apply_local(&path, &snapshot, &edit));
                Event::Renamed(generation, tab, snapshot, result)
            },
            message,
        )
    }
}

fn apply_local(path: &Path, text: &str, edit: &serde_json::Value) -> Result<String, Key> {
    let uri = url::Url::from_file_path(path).map_err(|_| Key::EditorRenameFailed)?;
    let mut edits = Vec::<TextEdit>::new();
    let mut collect = |target: &str, value: &serde_json::Value| {
        if target != uri.as_str() {
            return Err(Key::EditorRenameScope);
        }
        edits.extend(
            serde_json::from_value::<Vec<TextEdit>>(value.clone())
                .map_err(|_| Key::EditorRenameFailed)?,
        );
        Ok(())
    };
    if let Some(changes) = edit.get("changes").and_then(serde_json::Value::as_object) {
        for (target, value) in changes {
            collect(target, value)?;
        }
    }
    if let Some(changes) = edit
        .get("documentChanges")
        .and_then(serde_json::Value::as_array)
    {
        for change in changes {
            let target = change["textDocument"]["uri"]
                .as_str()
                .ok_or(Key::EditorRenameScope)?;
            collect(target, &change["edits"])?;
        }
    }
    if edits.is_empty() {
        return Err(Key::EditorRenameFailed);
    }
    language::apply_edits(text, &edits).ok_or(Key::EditorRenameFailed)
}

pub(crate) fn message(event: Event) -> Message {
    Message::Editor(super::Event::Menu(event))
}

#[cfg(test)]
mod tests;
