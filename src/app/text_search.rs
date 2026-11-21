//! Coordinate file and project searches without blocking the interface.
use super::{App, Message, editor};
use crate::{
    domain::text_search::{self, Options},
    i18n::Key,
    infrastructure::text_search::{Request, Results},
};
use iced::{
    Task,
    widget::{operation, text_editor},
};
use std::{path::PathBuf, sync::Arc, time::Duration};
use tokio_util::sync::CancellationToken;

#[derive(Default)]
pub(crate) struct State {
    pub file: Option<Search>,
    pub project: Option<Search>,
    pub saved: Option<Search>,
    pub pinned: bool,
    last_click: Option<(usize, std::time::Instant)>,
    serial: u64,
    replacement_request: u64,
    cancelled: CancellationToken,
    preview_cancelled: CancellationToken,
}
#[derive(Clone)]
pub(crate) struct Search {
    pub preview: String,
    pub query: String,
    pub replacement: String,
    pub replace: bool,
    pub options: Options,
    pub mask: String,
    pub folder: String,
    pub results: Results,
    pub selected: usize,
    pub error: Option<Key>,
    pub busy: bool,
    pub tab: u64,
    pub root: PathBuf,
}
impl Search {
    fn new(tab: u64, root: PathBuf) -> Self {
        Self {
            preview: String::new(),
            query: String::new(),
            replacement: String::new(),
            replace: false,
            options: Options::default(),
            mask: String::new(),
            folder: String::new(),
            results: Results::default(),
            selected: 0,
            error: None,
            busy: false,
            tab,
            root,
        }
    }
}
#[derive(Debug, Clone)]
pub(crate) enum Event {
    Preview(u64, usize, Option<String>),
    Reveal(u64, u8),
    Page(
        u64,
        u64,
        Result<Option<super::line_navigation::LoadedPage>, crate::domain::Failure>,
    ),
    Open(bool),
    Query(bool, String),
    Replacement(String),
    ToggleReplace,
    Option(bool, u8),
    Mask(String),
    Folder(String),
    Run(bool, u64),
    Loaded(bool, u64, Results),
    Move(bool, bool),
    Select(usize),
    Activate,
    Close(bool),
    Replace(bool),
    Replaced {
        request: u64,
        serial: u64,
        tab: u64,
        original: Arc<String>,
        result: Result<Option<String>, crate::domain::Failure>,
    },
    Pin,
    Restore,
    Refresh,
}
impl App {
    pub(crate) fn file_search(&self) -> Option<&Search> {
        self.text_search
            .file
            .as_ref()
            .filter(|search| search.tab == self.preview().id)
    }
    pub(super) fn update_text_search(&mut self, event: Event) -> Task<Message> {
        match event {
            Event::Preview(serial, index, value) if serial == self.text_search.serial => {
                if let Some(search) = self
                    .text_search
                    .project
                    .as_mut()
                    .filter(|search| search.selected == index)
                {
                    search.preview = value.unwrap_or_default();
                }
            }
            Event::Reveal(id, attempts) if self.preview().id == id => {
                if self.preview().loading && attempts < 100 {
                    return Task::perform(
                        async {
                            tokio::time::sleep(Duration::from_millis(50)).await;
                        },
                        move |_| Message::TextSearch(Event::Reveal(id, attempts + 1)),
                    );
                }
                return self.schedule_search(false);
            }
            Event::Page(id, serial, Ok(Some(page)))
                if self.preview().id == id && self.text_search.serial == serial =>
            {
                let task = self.accept_line_preview(*page.document, page.history);
                return Task::batch([task, self.reveal_match()]);
            }
            Event::Page(id, serial, _)
                if self.preview().id == id && self.text_search.serial == serial =>
            {
                if let Some(search) = self.text_search.file.as_mut() {
                    search.error = Some(Key::LineNumberNotFound);
                }
            }
            Event::Open(project) => {
                if !project && self.file_search().is_some() {
                    return Task::none();
                }
                if self.editor.pending.is_some()
                    || self.auth_dialog.is_some()
                    || self.line_navigation.is_some()
                    || self.space_draft.is_some()
                    || self.project_add.is_some()
                    || self.navigation == super::Navigation::Settings
                {
                    return Task::none();
                }
                if !project && self.preview().document.is_none() && self.editor_buffer().is_none() {
                    return Task::none();
                }
                let Some(root) = self.workspace.directory().map(PathBuf::from) else {
                    return Task::none();
                };
                self.search_everywhere.dialog = None;
                let mut search = Search::new(self.preview().id, root);
                if let Some(selected) = self
                    .editor_buffer()
                    .and_then(|buffer| buffer.content.selection())
                    .filter(|text| !text.contains(['\n', '\r']))
                {
                    search.query = selected;
                }
                if project {
                    self.text_search.pinned = false;
                    self.text_search.project = Some(search);
                } else if self.file_search().is_none() {
                    self.text_search.file = Some(search);
                }
                return Task::batch([
                    self.schedule_search(project),
                    operation::focus(if project {
                        "project-search"
                    } else {
                        "file-search"
                    }),
                ]);
            }
            Event::Query(project, value) => {
                if let Some(search) = self.search_mut(project) {
                    search.query = value.chars().take(1000).collect();
                }
                return self.schedule_search(project);
            }
            Event::Replacement(value) => {
                if let Some(search) = &mut self.text_search.file {
                    search.replacement = value;
                }
            }
            Event::ToggleReplace => {
                if let Some(search) = &mut self.text_search.file {
                    search.replace = !search.replace;
                }
            }
            Event::Option(project, option) => {
                if let Some(search) = self.search_mut(project) {
                    match option {
                        0 => search.options.case = !search.options.case,
                        1 => search.options.word = !search.options.word,
                        _ => search.options.regex = !search.options.regex,
                    }
                }
                return self.schedule_search(project);
            }
            Event::Mask(value) => {
                if let Some(search) = &mut self.text_search.project {
                    search.mask = value;
                }
                return self.schedule_search(true);
            }
            Event::Folder(value) => {
                if let Some(search) = &mut self.text_search.project {
                    search.folder = value;
                }
                return self.schedule_search(true);
            }
            Event::Run(project, serial) if serial == self.text_search.serial => {
                return self.run_search(project, serial);
            }
            Event::Loaded(project, serial, results) if serial == self.text_search.serial => {
                let root = self.workspace.directory().map(PathBuf::from);
                if let Some(search) = self
                    .search_mut(project)
                    .filter(|search| Some(&search.root) == root.as_ref())
                {
                    let previous = search
                        .results
                        .hits
                        .get(search.selected)
                        .map(|hit| (hit.found.line, hit.found.byte_column));
                    search.results = results;
                    search.busy = false;
                    search.selected =
                        previous
                            .and_then(|position| {
                                search.results.hits.iter().position(|hit| {
                                    (hit.found.line, hit.found.byte_column) == position
                                })
                            })
                            .unwrap_or(0);
                }
                if project && self.text_search.pinned {
                    self.text_search.saved = self.text_search.project.clone();
                }
                if !project {
                    return self.reveal_match();
                }
                return self.load_search_context();
            }
            Event::Move(project, forward) => {
                if let Some(search) = self.search_mut(project) {
                    let count = search.results.hits.len();
                    if count > 0 {
                        search.selected = if forward {
                            (search.selected + 1) % count
                        } else {
                            (search.selected + count - 1) % count
                        };
                    }
                }
                if !project {
                    return self.reveal_match();
                }
                return self.load_search_context();
            }
            Event::Select(index) => {
                let double = self.text_search.last_click.is_some_and(|(previous, time)| {
                    previous == index && time.elapsed() < Duration::from_millis(400)
                });
                self.text_search.last_click = Some((index, std::time::Instant::now()));
                if double {
                    return self.update_text_search(Event::Activate);
                }
                if let Some(search) = &mut self.text_search.project {
                    search.selected = index.min(search.results.hits.len().saturating_sub(1));
                }
                return self.load_search_context();
            }
            Event::Activate => {
                if let Some(hit) = self
                    .text_search
                    .project
                    .as_ref()
                    .and_then(|search| search.results.hits.get(search.selected))
                    .cloned()
                {
                    let query = self
                        .text_search
                        .project
                        .as_ref()
                        .map(|search| search.query.clone())
                        .unwrap_or_default();
                    let options = self
                        .text_search
                        .project
                        .as_ref()
                        .map(|search| search.options.clone())
                        .unwrap_or_default();
                    self.text_search.project = None;
                    self.text_search.pinned = false;
                    let task = self.open_file(hit.path.clone());
                    let mut search = Search::new(
                        self.preview().id,
                        self.workspace
                            .directory()
                            .map(PathBuf::from)
                            .unwrap_or_default(),
                    );
                    search.query = query;
                    search.options = options;
                    search.results.hits.push(hit);
                    self.text_search.file = Some(search);
                    return Task::batch([
                        task,
                        Task::done(Message::TextSearch(Event::Reveal(self.preview().id, 0))),
                    ]);
                }
            }
            Event::Close(project) => {
                self.text_search.cancelled.cancel();
                self.text_search.serial += 1;
                if project {
                    self.text_search.project = None;
                    self.text_search.pinned = false;
                } else {
                    self.text_search.file = None;
                }
                return operation::focus(format!("text-editor-{}", self.preview().id));
            }
            Event::Replace(all) => {
                let Some(search) = self.file_search() else {
                    return Task::none();
                };
                let Some(buffer) = self.editor_buffer() else {
                    return Task::none();
                };
                let Ok(pattern) = text_search::compile(&search.query, &search.options) else {
                    return Task::none();
                };
                if search.query.is_empty() {
                    return Task::none();
                }
                let original = Arc::clone(&buffer.document.text);
                let expand = search.options.regex;
                let selected = (!all).then_some(search.selected);
                let replacement = search.replacement.clone();
                let tab = search.tab;
                let serial = self.text_search.serial;
                self.text_search.replacement_request += 1;
                let request = self.text_search.replacement_request;
                return Task::perform(
                    async move {
                        let input = Arc::clone(&original);
                        let result = tokio::task::spawn_blocking(move || {
                            text_search::replace(
                                &input,
                                &pattern,
                                &replacement,
                                expand,
                                selected,
                                crate::domain::editor::LIMIT,
                            )
                        })
                        .await
                        .unwrap_or(Err(crate::domain::Failure::Unavailable));
                        Event::Replaced {
                            request,
                            serial,
                            tab,
                            original,
                            result,
                        }
                    },
                    Message::TextSearch,
                );
            }
            Event::Replaced {
                request,
                serial,
                tab,
                original,
                result,
            } if request == self.text_search.replacement_request
                && serial == self.text_search.serial
                && self.file_search().is_some_and(|search| search.tab == tab)
                && self
                    .editor_buffer()
                    .is_some_and(|buffer| Arc::ptr_eq(&buffer.document.text, &original)) =>
            {
                match result {
                    Ok(Some(text)) => return self.update_editor(editor::Event::Replace(tab, text)),
                    Ok(None) => {}
                    Err(crate::domain::Failure::TooLarge) => {
                        self.notifications.error(Key::EditorTooLarge, None)
                    }
                    Err(error) => self.notifications.error(super::error_key(error), None),
                }
            }
            Event::Pin => {
                self.text_search.saved = self.text_search.project.clone();
                self.text_search.pinned = true;
            }
            Event::Restore => {
                self.text_search.project = self.text_search.saved.clone();
                self.text_search.pinned = true;
            }
            Event::Refresh => return self.schedule_search(true),
            _ => {}
        }
        Task::none()
    }
    fn load_search_context(&mut self) -> Task<Message> {
        self.text_search.preview_cancelled.cancel();
        self.text_search.preview_cancelled = self.text_search.cancelled.child_token();
        let Some(search) = &mut self.text_search.project else {
            return Task::none();
        };
        search.preview.clear();
        let Some(hit) = search.results.hits.get(search.selected) else {
            return Task::none();
        };
        let serial = self.text_search.serial;
        let index = search.selected;
        let line = hit.found.line;
        let path = hit.path.clone();
        if let Some(text) = self
            .tabs
            .files
            .iter()
            .filter_map(|tab| tab.editor.as_ref())
            .find_map(|buffer| match &buffer.target {
                editor::Target::Local(local) if *local == path => Some(&buffer.document.text),
                _ => None,
            })
        {
            search.preview = text
                .lines()
                .enumerate()
                .take(line.saturating_add(13))
                .skip(line.saturating_sub(12))
                .map(|(index, text)| {
                    format!(
                        "{}  {}\n",
                        index + 1,
                        text.chars().take(2000).collect::<String>()
                    )
                })
                .collect();
            return Task::none();
        }
        Task::perform(
            crate::infrastructure::text_search::preview(
                path,
                line,
                self.text_search.preview_cancelled.clone(),
            ),
            move |value| Message::TextSearch(Event::Preview(serial, index, value)),
        )
    }
    fn search_mut(&mut self, project: bool) -> Option<&mut Search> {
        if project {
            self.text_search.project.as_mut()
        } else {
            self.text_search.file.as_mut()
        }
    }
    pub(super) fn schedule_search(&mut self, project: bool) -> Task<Message> {
        self.text_search.cancelled.cancel();
        self.text_search.cancelled = CancellationToken::new();
        self.text_search.serial += 1;
        let serial = self.text_search.serial;
        if let Some(search) = self.search_mut(project) {
            search.error = None;
            if search.query.is_empty() {
                search.busy = false;
                search.results = Results::default();
                search.preview.clear();
                return Task::none();
            }
            search.busy = true;
        }
        Task::perform(
            async {
                tokio::time::sleep(Duration::from_millis(180)).await;
            },
            move |_| Message::TextSearch(Event::Run(project, serial)),
        )
    }
    fn run_search(&mut self, project: bool, serial: u64) -> Task<Message> {
        let search = if project {
            self.text_search.project.as_ref()
        } else {
            self.file_search()
        };
        let Some(search) = search else {
            return Task::none();
        };
        if self.workspace.directory() != Some(&search.root) {
            return Task::none();
        }
        let Ok(pattern) = text_search::compile(&search.query, &search.options) else {
            if let Some(search) = self.search_mut(project) {
                search.error = Some(Key::FindInvalid);
                search.busy = false;
                search.results = Results::default();
            }
            return Task::none();
        };
        let folder = search.root.join(&search.folder);
        if !folder.starts_with(&search.root)
            || std::path::Path::new(&search.folder)
                .components()
                .any(|part| matches!(part, std::path::Component::ParentDir))
        {
            if let Some(search) = self.search_mut(project) {
                search.error = Some(Key::FindFolderInvalid);
                search.busy = false;
            }
            return Task::none();
        }

        if !project {
            let text = self
                .text_buffer()
                .map(|buffer| buffer.document.text.clone())
                .or_else(|| {
                    self.preview()
                        .document
                        .as_ref()
                        .filter(|document| document.page.is_none())
                        .map(|document| std::sync::Arc::new(document.text.clone()))
                });
            if let Some(text) = text {
                let query = search.query.clone();
                let path = self.preview().path.clone().unwrap_or_default();
                return Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || {
                            let hits = if query.is_empty() {
                                vec![]
                            } else {
                                text_search::matches(&text, &pattern, 10_000)
                            };
                            Results {
                                hits: hits
                                    .into_iter()
                                    .map(|found| crate::infrastructure::text_search::Hit {
                                        path: path.clone(),
                                        found,
                                    })
                                    .collect(),
                                ..Results::default()
                            }
                        })
                        .await
                        .unwrap_or_default()
                    },
                    move |results| Message::TextSearch(Event::Loaded(false, serial, results)),
                );
            }
        }
        let request = Request {
            root: search.root.clone(),
            folder,
            mask: search.mask.clone(),
            query: search.query.clone(),
            options: search.options.clone(),
            buffers: self.language_buffers().into_iter().collect(),
            file: if project {
                None
            } else {
                self.preview().path.clone()
            },
            cancelled: self.text_search.cancelled.clone(),
        };
        Task::perform(
            crate::infrastructure::text_search::search(request),
            move |results| Message::TextSearch(Event::Loaded(project, serial, results)),
        )
    }
    pub(super) fn reveal_match(&mut self) -> Task<Message> {
        let Some(found) = self
            .file_search()
            .and_then(|search| search.results.hits.get(search.selected))
            .map(|hit| hit.found.clone())
        else {
            return Task::none();
        };
        let id = self.preview().id;
        if let Some(buffer) = self
            .preview_for_request(id)
            .and_then(|tab| tab.editor.as_mut())
        {
            buffer.content.move_to(text_editor::Cursor {
                position: text_editor::Position {
                    line: found.line - 1,
                    column: found.end_byte_column,
                },
                selection: Some(text_editor::Position {
                    line: found.line - 1,
                    column: found.byte_column,
                }),
            });
        } else if self.reveal_source_line(found.line) {
            if let Some(preview) = self.preview_for_request(id)
                && let Some(document) = &preview.document
                && let Some(row) = document
                    .line_info
                    .iter()
                    .position(|info| info.number == found.line)
            {
                use crate::ui::code::selection::{Action, Lane, Position};
                preview.selection.update(
                    Action::Start(
                        Lane::Unified,
                        Position {
                            row,
                            byte: found.byte_column,
                        },
                    ),
                    document,
                );
                preview.selection.update(
                    Action::Extend(
                        Lane::Unified,
                        Position {
                            row,
                            byte: found.end_byte_column,
                        },
                    ),
                    document,
                );
                preview.selection.update(Action::End, document);
            }
            return self.restore_preview_scroll();
        } else if self
            .preview()
            .document
            .as_ref()
            .is_some_and(|document| document.page.is_some())
        {
            return self.load_search_page(id, self.text_search.serial, found.line);
        }
        Task::none()
    }
}

impl State {
    pub(super) fn cancel(&mut self) {
        self.cancelled.cancel();
        self.serial += 1;
        self.file = None;
        self.project = None;
        self.saved = None;
        self.pinned = false;
    }
}
#[cfg(test)]
mod tests;
