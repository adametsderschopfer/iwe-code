//! Coordinate Docker connections, resource lists and explicit operations.

mod adapter;
mod container;
mod model;
mod view;
use crate::i18n::Key;
use adapter::Error;
use iced::{Point, Task};
use model::{Action, Item, Kind};
use std::{path::Path, sync::Arc};
pub(crate) use view::{overlay, panel};

const PAGE: usize = 40;

#[derive(Debug)]
pub(crate) struct Output {
    text: String,
    offsets: Vec<usize>,
}
impl Output {
    fn new(text: String) -> Self {
        let mut offsets = vec![0];
        let mut lines = 0;
        let mut start = 0;
        for (byte, ch) in text.char_indices() {
            if byte - start >= 32 * 1024 || lines == 100 {
                offsets.push(byte);
                start = byte;
                lines = 0;
            }
            lines += usize::from(ch == '\n');
        }
        Self { text, offsets }
    }
    fn page(&self, index: usize) -> Option<&str> {
        let start = *self.offsets.get(index)?;
        Some(
            &self.text[start
                ..self
                    .offsets
                    .get(index + 1)
                    .copied()
                    .unwrap_or(self.text.len())],
        )
    }
}

#[derive(Debug, Clone)]
pub(crate) enum Event {
    Refresh,
    Tick,
    Connections,
    Connected(u64, Result<(Vec<String>, String), Error>),
    Context(String),
    Kind(Kind),
    Listed(u64, Result<Arc<Vec<Item>>, Error>),
    Search(String),
    Page(bool),
    Select(String),
    BackToList,
    Menu(String, Point),
    Dismiss,
    Act(Action),
    Value(String),
    Extra(String),
    ContainerField(container::Field, String),
    Browse,
    Picked(u64, Option<String>),
    Submit,
    Finished(u64, Action, Result<Arc<Output>, Error>),
    Shell(u64, Vec<String>),
    Hide,
    OutputPage(bool),
}

struct Draft {
    action: Action,
    value: String,
    extra: String,
    target: Option<String>,
    container: container::Options,
}

impl Draft {
    fn arguments(&self, kind: Kind, selected: Option<&Item>) -> Option<Vec<String>> {
        if kind == Kind::Containers && self.action == Action::Create {
            return self
                .container
                .arguments(self.value.trim(), self.extra.trim());
        }
        model::arguments(kind, self.action, selected, &self.value, &self.extra)
    }
}

#[derive(Default)]
pub(crate) struct State {
    pub open: bool,
    request: u64,
    contexts: Vec<String>,
    context: Option<String>,
    kind: Kind,
    items: Arc<Vec<Item>>,
    filtered: Vec<usize>,
    query: String,
    page: usize,
    selected: Option<String>,
    menu: Option<Point>,
    draft: Option<Draft>,
    pending_form: Option<Draft>,
    started: Option<std::time::Instant>,
    busy: Option<Key>,
    error: Option<Error>,
    connection_error: Option<Error>,
    detail: Option<Arc<Output>>,
    output_page: usize,
    completed: Option<Action>,
}

impl State {
    pub(crate) fn refresh_interval(&self) -> std::time::Duration {
        std::time::Duration::from_secs(if self.busy.is_some() { 1 } else { 12 })
    }
    pub(crate) fn has_dialog(&self) -> bool {
        self.open && self.draft.is_some()
    }
    pub(crate) fn reset(&mut self) {
        *self = Self {
            request: self.request + 1,
            ..Default::default()
        };
    }
    pub(crate) fn accepts_shell(&self, request: u64) -> bool {
        self.open && self.request == request && self.busy.is_none()
    }
    fn selected(&self) -> Option<&Item> {
        let id = self.selected.as_deref()?;
        self.items.iter().find(|item| item.id == id)
    }
    fn filter(&mut self) {
        let query = self.query.trim().to_lowercase();
        self.filtered = self
            .items
            .iter()
            .enumerate()
            .filter(|(_, item)| item.search.contains(&query))
            .map(|(i, _)| i)
            .collect();
        self.page = self.page.min(self.filtered.len().saturating_sub(1) / PAGE);
    }
    fn clear_selection(&mut self) {
        self.selected = None;
        self.detail = None;
        self.output_page = 0;
        self.menu = None;
        self.draft = None;
        self.completed = None;
    }
    fn list_failed(&mut self, error: Error) {
        if matches!(error, Error::Stopped | Error::Unavailable | Error::Missing) {
            self.connection_error = Some(error);
            self.items = Arc::default();
            self.clear_selection();
            self.filter();
        } else {
            self.connection_error = None;
            self.error = Some(error);
        }
    }
    fn begin(&mut self, key: Key) -> u64 {
        self.request += 1;
        self.started = Some(std::time::Instant::now());
        self.busy = Some(key);
        self.error = None;
        self.request
    }
    fn list(&mut self, root: &Path) -> Task<Event> {
        let Some(context) = self.context.clone() else {
            return self.connections(root);
        };
        let kind = self.kind;
        let root = root.to_owned();
        let id = self.begin(Key::Loading);
        Task::perform(
            async move {
                let output =
                    adapter::run(Some(&context), &kind.list(), &root, false, false).await?;
                tokio::task::spawn_blocking(move || adapter::parse(kind, &output).map(Arc::new))
                    .await
                    .map_err(|_| Error::Unavailable)?
            },
            move |result| Event::Listed(id, result),
        )
    }
    fn connections(&mut self, root: &Path) -> Task<Event> {
        let id = self.begin(Key::Loading);
        let root = root.to_owned();
        Task::perform(
            async move { adapter::contexts(&root).await },
            move |result| Event::Connected(id, result),
        )
    }

    pub(crate) fn update(&mut self, event: Event, root: &Path, terminal: bool) -> Task<Event> {
        match event {
            Event::Connected(id, result) if id == self.request => {
                self.busy = None;
                match result {
                    Ok((names, current)) => {
                        if self
                            .context
                            .as_ref()
                            .is_none_or(|name| !names.contains(name))
                        {
                            self.context = Some(current);
                        }
                        self.contexts = names;
                        return self.list(root);
                    }
                    Err(error) => self.list_failed(error),
                }
            }
            Event::Listed(id, result) if id == self.request => {
                self.busy = None;
                match result {
                    Ok(items) => {
                        self.connection_error = None;
                        self.items = items;
                        self.filter();
                        if self.selected().is_none() {
                            self.selected = None;
                            self.menu = None;
                        }
                    }
                    Err(error) => self.list_failed(error),
                }
            }
            Event::Finished(id, action, result) if id == self.request => {
                let form = self.pending_form.take();
                self.busy = None;
                match result {
                    Err(error @ (Error::Stopped | Error::Missing)) => self.list_failed(error),
                    Err(error) => {
                        self.error = Some(error);
                        self.draft = form
                            .filter(|form| !matches!(form.action, Action::Inspect | Action::Logs));
                    }
                    Ok(output) => {
                        self.completed = Some(action);
                        self.draft = None;
                        if action == Action::AddContext {
                            return self.connections(root);
                        }
                        if action.writes() {
                            return self.list(root);
                        }
                        self.detail = Some(output);
                        self.output_page = 0;
                    }
                }
            }
            Event::Picked(id, path) if id == self.request => {
                self.busy = None;
                if let (Some(path), Some(draft)) = (path, &mut self.draft) {
                    draft.value = path;
                }
            }
            Event::Dismiss if self.busy.is_none() => {
                self.menu = None;
                self.draft = None;
            }
            Event::Search(query) => {
                self.query = query;
                self.page = 0;
                self.filter();
            }
            Event::Page(next) => {
                self.page = if next {
                    (self.page + 1).min(self.filtered.len().saturating_sub(1) / PAGE)
                } else {
                    self.page.saturating_sub(1)
                };
            }
            Event::OutputPage(next) => {
                self.output_page = if next {
                    (self.output_page + 1).min(
                        self.detail
                            .as_ref()
                            .map_or(0, |out| out.offsets.len().saturating_sub(1)),
                    )
                } else {
                    self.output_page.saturating_sub(1)
                };
            }
            _ if self.busy.is_some() => {}
            Event::Tick
                if !self.open
                    || self.error.is_some()
                    || self.menu.is_some()
                    || self.draft.is_some() => {}
            Event::Connections => return self.connections(root),
            Event::Tick | Event::Refresh => return self.list(root),
            Event::Context(name) if self.contexts.contains(&name) => {
                self.context = Some(name);
                self.connection_error = None;
                self.items = Arc::default();
                self.clear_selection();
                self.filter();
                return self.list(root);
            }
            Event::Kind(kind) => {
                self.kind = kind;
                self.items = Arc::default();
                self.query.clear();
                self.clear_selection();
                self.filter();
                return self.list(root);
            }
            Event::Menu(id, point) if self.items.iter().any(|item| item.id == id) => {
                self.clear_selection();
                self.selected = Some(id);
                self.menu = Some(point);
            }
            Event::BackToList => self.clear_selection(),
            Event::Select(id) if self.items.iter().any(|item| item.id == id) => {
                self.clear_selection();
                self.selected = Some(id);
            }
            Event::Act(action) => {
                self.menu = None;
                let available = self.selected().is_some_and(|item| {
                    Action::actions(self.kind, item, terminal).contains(&action)
                });
                if action != Action::AddContext
                    && (self.connection_error.is_some()
                        || self.context.is_none()
                        || action != self.kind.create()
                            && !(self.kind == Kind::Images && action == Action::Build)
                            && !available)
                {
                    return Task::none();
                }
                self.error = None;
                self.draft = Some(Draft {
                    action,
                    value: if action == Action::Terminal {
                        "sh".into()
                    } else {
                        String::new()
                    },
                    extra: String::new(),
                    target: self.selected.clone(),
                    container: Default::default(),
                });
                if !action.confirm()
                    && matches!(
                        action,
                        Action::Inspect | Action::Logs | Action::Start | Action::Resume
                    )
                {
                    return self.submit(root);
                }
            }
            Event::Value(value) => {
                if let Some(draft) = &mut self.draft {
                    draft.value = value;
                }
            }
            Event::ContainerField(field, value) => {
                if let Some(draft) = &mut self.draft {
                    draft.container.set(field, value);
                }
            }
            Event::Extra(value) => {
                if let Some(draft) = &mut self.draft {
                    draft.extra = value;
                }
            }
            Event::Browse
                if self
                    .draft
                    .as_ref()
                    .is_some_and(|draft| matches!(draft.action, Action::Up | Action::Build)) =>
            {
                let compose = self
                    .draft
                    .as_ref()
                    .is_some_and(|draft| draft.action == Action::Up);
                let id = self.begin(Key::Loading);
                let root = root.to_owned();
                return Task::perform(
                    async move {
                        let picker = rfd::AsyncFileDialog::new().set_directory(root);
                        let picker = if compose {
                            picker.add_filter("YAML", &["yaml", "yml"])
                        } else {
                            picker
                        };
                        picker
                            .pick_file()
                            .await
                            .and_then(|file| file.path().to_str().map(str::to_owned))
                    },
                    move |path| Event::Picked(id, path),
                );
            }
            Event::Submit => return self.submit(root),
            _ => {}
        }
        Task::none()
    }

    fn submit(&mut self, root: &Path) -> Task<Event> {
        let Some(draft) = &self.draft else {
            return Task::none();
        };
        let action = draft.action;
        if draft.target != self.selected {
            self.draft = None;
            return Task::none();
        }
        let Some(args) = draft.arguments(self.kind, self.selected()) else {
            self.error = Some(Error::Invalid);
            return Task::none();
        };
        let context = self.context.clone();
        if action == Action::Terminal {
            let Some(context) = context else {
                return Task::none();
            };
            self.draft = None;
            self.request += 1;
            return Task::done(Event::Shell(
                self.request,
                ["--context".to_owned(), context]
                    .into_iter()
                    .chain(args)
                    .collect(),
            ));
        }
        let root = root.to_owned();
        self.pending_form = self.draft.take();
        let id = self.begin(action.key());
        Task::perform(
            async move {
                adapter::run(
                    if action == Action::AddContext {
                        None
                    } else {
                        context.as_deref()
                    },
                    &args,
                    &root,
                    action.writes(),
                    action == Action::Logs,
                )
                .await
                .map(|output| Arc::new(Output::new(output)))
            },
            move |result| Event::Finished(id, action, result),
        )
    }
}

#[cfg(test)]
mod tests;
