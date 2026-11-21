//! Coordinate project tabs and route events to their original terminal sessions.

mod title;
pub(crate) use title::{TabTitle, TitleAction, TitleControl};

use super::session::{Lifetime, MAX_INPUT, Output, Session};
use iced::{Subscription, Task};
use std::{
    collections::BTreeMap,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    sync::Arc,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Status {
    Starting,
    Running,
    Exited,
    Failed,
}

#[derive(Default)]
pub(crate) struct State {
    pub open: bool,
    pub tabs: Vec<Tab>,
    pub active: Option<u64>,
    background: BTreeMap<PathBuf, ProjectTabs>,
    pub lifetime: Lifetime,
    next_request: u64,
    next_local_number: u64,
}

#[derive(Default)]
struct ProjectTabs {
    open: bool,
    tabs: Vec<Tab>,
    active: Option<u64>,
    next_local_number: u64,
}

/// Preserve the selected neighbor when the active tab closes.
fn remove_tabs(
    tabs: &mut Vec<Tab>,
    active: &mut Option<u64>,
    open: &mut bool,
    remove: impl Fn(&Tab) -> bool,
) {
    if let Some(index) = tabs.iter().position(|tab| Some(tab.request) == *active)
        && remove(&tabs[index])
    {
        *active = tabs[index + 1..]
            .iter()
            .chain(tabs[..index].iter().rev())
            .find(|tab| !remove(tab))
            .map(|tab| tab.request);
    }
    tabs.retain(|tab| !remove(tab));
    *open &= !tabs.is_empty();
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Program {
    Docker,
    Kubernetes,
}

pub(crate) struct Tab {
    pub request: u64,
    pub title: TabTitle,
    pub program: Option<Program>,
    pub status: Status,
    pub parser: vt100::Parser<Replies>,
    pub revision: u64,
    session: Option<Arc<Session>>,
    lifetime: Lifetime,
}

impl Drop for Tab {
    fn drop(&mut self) {
        self.lifetime.stop();
    }
}

impl Tab {
    fn new(request: u64, title: TabTitle, program: Option<Program>, lifetime: Lifetime) -> Self {
        Self {
            request,
            title,
            program,
            status: Status::Starting,
            parser: vt100::Parser::new_with_callbacks(24, 80, 2000, Replies::default()),
            revision: 0,
            session: None,
            lifetime,
        }
    }

    pub(crate) fn subscription(&self) -> Subscription<Event> {
        let Some(session) = &self.session else {
            return Subscription::none();
        };
        Subscription::run_with(
            Stream {
                request: self.request,
                session: Arc::clone(session),
            },
            |data| {
                iced::futures::stream::unfold(
                    (data.request, Some(Arc::clone(&data.session))),
                    |(request, session)| async move {
                        let session = session?;
                        let output = session.next().await;
                        let finished = output
                            .iter()
                            .any(|event| matches!(event, Output::Closed | Output::Failed));
                        Some((
                            Event::Output(request, output),
                            (request, (!finished).then_some(session)),
                        ))
                    },
                )
            },
        )
    }

    pub(crate) fn running(&self) -> bool {
        self.session.is_some()
    }

    pub(crate) fn editing_title(&self) -> bool {
        matches!(self.title.control, TitleControl::Editing(_))
    }

    fn fail(&mut self) {
        self.session = None;
        self.status = Status::Failed;
    }

    fn send(&mut self, bytes: Vec<u8>) {
        if let Some(session) = &self.session {
            if session.input(bytes).is_err() {
                self.fail();
            } else {
                if self.parser.screen().scrollback() != 0 {
                    self.revision = self.revision.wrapping_add(1);
                }
                self.parser.screen_mut().set_scrollback(0);
            }
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) enum Event {
    Start,
    StartProgram {
        program: Program,
        executable: std::path::PathBuf,
        args: Vec<String>,
        title: String,
    },
    Title(u64, TitleAction),
    Started(u64, Result<Arc<Session>, ()>),
    Close(u64),
    Select(u64),
    Hide,
    PasteRejected(u64),
    Focus(u64),
    Output(u64, Vec<Output>),
    Input(u64, Vec<u8>),
    Resize(u64, u16, u16),
    Scroll(u64, i32),
    Paste(u64),
    Pasted(u64, Option<String>),
    Copy(String),
    Cut(u64, String, Vec<u8>),
}

impl State {
    pub(crate) fn active_tab(&self) -> Option<&Tab> {
        self.tabs
            .iter()
            .find(|tab| Some(tab.request) == self.active)
    }

    pub(crate) fn subscription(&self) -> Subscription<Event> {
        Subscription::batch(
            self.tabs
                .iter()
                .chain(self.background.values().flat_map(|project| &project.tabs))
                .map(Tab::subscription),
        )
    }

    pub(crate) fn switch_project(&mut self, previous: Option<&Path>, next: Option<&Path>) {
        self.dismiss_title_edit();
        let mut project = next
            .and_then(|path| self.background.remove(path))
            .unwrap_or_default();
        std::mem::swap(&mut self.open, &mut project.open);
        std::mem::swap(&mut self.tabs, &mut project.tabs);
        std::mem::swap(&mut self.active, &mut project.active);
        std::mem::swap(&mut self.next_local_number, &mut project.next_local_number);
        if let Some(previous) = previous
            && !project.tabs.is_empty()
        {
            self.background.insert(previous.to_owned(), project);
        }
    }

    pub(crate) fn retain_projects(&mut self, mut exists: impl FnMut(&Path) -> bool) {
        self.background.retain(|directory, _| exists(directory));
    }

    pub(crate) fn close_cluster_tabs(&mut self, all_projects: bool) {
        let cluster = |tab: &Tab| tab.program == Some(Program::Kubernetes);
        remove_tabs(&mut self.tabs, &mut self.active, &mut self.open, cluster);
        if all_projects {
            self.background.retain(|_, project| {
                remove_tabs(
                    &mut project.tabs,
                    &mut project.active,
                    &mut project.open,
                    cluster,
                );
                !project.tabs.is_empty()
            });
        }
    }

    fn tab_mut(&mut self, request: u64) -> Option<&mut Tab> {
        self.tabs
            .iter_mut()
            .chain(
                self.background
                    .values_mut()
                    .flat_map(|project| &mut project.tabs),
            )
            .find(|tab| tab.request == request)
    }

    pub(crate) fn reset(&mut self) {
        let next_request = self.next_request;
        *self = Self {
            next_request,
            lifetime: self.lifetime.clone(),
            ..Self::default()
        };
    }

    pub(crate) fn update(&mut self, event: Event, directory: Option<&Path>) -> Task<Event> {
        match event {
            Event::Start | Event::StartProgram { .. } => {
                let Some(directory) = directory.map(Path::to_owned) else {
                    return Task::none();
                };
                self.next_request = self.next_request.wrapping_add(1);
                let request = self.next_request;
                if self.tabs.is_empty() {
                    self.next_local_number = 0;
                }
                let (title, program) = match &event {
                    Event::StartProgram { title, program, .. } => {
                        (TabTitle::named(title, &self.tabs), Some(*program))
                    }
                    _ => {
                        self.next_local_number += 1;
                        (TabTitle::local(self.next_local_number), None)
                    }
                };
                self.dismiss_title_edit();
                let lifetime = self.lifetime.tab();
                self.tabs
                    .push(Tab::new(request, title, program, lifetime.clone()));
                self.active = Some(request);
                self.open = true;
                let start = Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || match event {
                            Event::StartProgram {
                                executable, args, ..
                            } => Session::program(&directory, &executable, args, lifetime),
                            _ => Session::start(&directory, 24, 80, lifetime),
                        })
                        .await
                        .unwrap_or(Err(()))
                    },
                    move |result| Event::Started(request, result),
                );
                return Task::batch([start, iced::widget::operation::snap_to_end("terminal-tabs")]);
            }
            Event::Hide => {
                self.dismiss_title_edit();
                self.open = false;
            }
            Event::Title(request, action) => return self.update_title(request, action),
            Event::Select(request) => {
                if self.tabs.iter().any(|tab| tab.request == request) {
                    self.dismiss_title_edit();
                    self.active = Some(request);
                }
            }
            Event::Close(request) => {
                let matches = |tab: &Tab| tab.request == request;
                if self.tabs.iter().any(matches) {
                    remove_tabs(&mut self.tabs, &mut self.active, &mut self.open, matches);
                } else if let Some(project) = self
                    .background
                    .values_mut()
                    .find(|project| project.tabs.iter().any(matches))
                {
                    remove_tabs(
                        &mut project.tabs,
                        &mut project.active,
                        &mut project.open,
                        matches,
                    );
                }
            }
            Event::Copy(text) => return iced::clipboard::write(text),
            Event::Cut(request, text, bytes) => {
                let Some(tab) = self.tab_mut(request) else {
                    return Task::none();
                };
                let input = tab.update(Event::Input(request, bytes));
                return Task::batch([iced::clipboard::write(text), input]);
            }
            event => {
                let request = match &event {
                    Event::Started(request, _)
                    | Event::Output(request, _)
                    | Event::Input(request, _)
                    | Event::Resize(request, _, _)
                    | Event::Scroll(request, _)
                    | Event::Paste(request)
                    | Event::Pasted(request, _) => *request,
                    _ => return Task::none(),
                };
                if let Some(tab) = self.tab_mut(request) {
                    return tab.update(event);
                }
            }
        }
        Task::none()
    }
}

impl Tab {
    fn update(&mut self, event: Event) -> Task<Event> {
        let state = self;
        match event {
            Event::Started(request, result)
                if request == state.request && state.status == Status::Starting =>
            {
                match result {
                    Ok(session) => {
                        let (rows, cols) = state.parser.screen().size();
                        if session.resize(rows, cols).is_err() {
                            state.fail();
                        } else {
                            state.session = Some(session);
                            state.status = Status::Running;
                        }
                    }
                    Err(()) => state.fail(),
                }
            }
            Event::Output(request, events) if request == state.request && state.running() => {
                if !events.is_empty() {
                    state.revision = state.revision.wrapping_add(1);
                }
                for event in events {
                    match event {
                        Output::Bytes(bytes) => state.parser.process(&bytes),
                        Output::Closed => {
                            state.session = None;
                            state.status = Status::Exited;
                            break;
                        }
                        Output::Failed => {
                            state.fail();
                            break;
                        }
                    }
                }
                let replies = std::mem::take(&mut state.parser.callbacks_mut().0);
                if !replies.is_empty() {
                    state.send(replies);
                }
            }
            Event::Input(request, bytes) if request == state.request => state.send(bytes),
            Event::Resize(request, rows, cols) if request == state.request => {
                let (rows, cols) = (rows.clamp(1, 200), cols.clamp(1, 400));
                if state.parser.screen().size() != (rows, cols) {
                    state.revision = state.revision.wrapping_add(1);
                    state.parser.screen_mut().set_size(rows, cols);
                    if let Some(session) = &state.session
                        && session.resize(rows, cols).is_err()
                    {
                        state.fail();
                    }
                }
            }
            Event::Scroll(request, lines) if request == state.request => {
                let previous = state.parser.screen().scrollback();
                let offset = state
                    .parser
                    .screen()
                    .scrollback()
                    .saturating_add_signed(lines as isize);
                state.parser.screen_mut().set_scrollback(offset);
                if state.parser.screen().scrollback() != previous {
                    state.revision = state.revision.wrapping_add(1);
                }
            }
            Event::Paste(request) if request == state.request && state.running() => {
                return iced::clipboard::read().map(move |text| Event::Pasted(request, text));
            }
            Event::Pasted(request, Some(text)) if request == state.request && state.running() => {
                if text.len() <= MAX_INPUT - 12 {
                    // Remove escape characters before applying the shell's paste protocol.
                    let text = text
                        .replace(['\0', '\u{1b}'], "")
                        .replace("\r\n", "\n")
                        .replace('\n', "\r");
                    let bytes = if state.parser.screen().bracketed_paste() {
                        format!("\u{1b}[200~{text}\u{1b}[201~").into_bytes()
                    } else {
                        text.into_bytes()
                    };
                    state.send(bytes);
                } else {
                    return Task::done(Event::PasteRejected(request));
                }
            }
            _ => {}
        }
        Task::none()
    }
}

#[derive(Default)]
pub(crate) struct Replies(Vec<u8>);

struct Stream {
    request: u64,
    session: Arc<Session>,
}

impl Hash for Stream {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.request.hash(state);
    }
}

impl vt100::Callbacks for Replies {
    fn unhandled_csi(
        &mut self,
        screen: &mut vt100::Screen,
        i1: Option<u8>,
        i2: Option<u8>,
        params: &[&[u16]],
        c: char,
    ) {
        if i1.is_some() || i2.is_some() || self.0.len() > 4096 {
            return;
        }
        match (
            c,
            params.first().and_then(|p| p.first()).copied().unwrap_or(0),
        ) {
            ('n', 5) => self.0.extend_from_slice(b"\x1b[0n"),
            ('n', 6) => {
                let (row, col) = screen.cursor_position();
                self.0
                    .extend_from_slice(format!("\x1b[{};{}R", row + 1, col + 1).as_bytes());
            }
            ('c', 0) => self.0.extend_from_slice(b"\x1b[?1;2c"),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests;
