//! Coordinate explicit repository actions and discard results from other projects.

pub(crate) mod bulk;
pub(crate) mod history;

use crate::{
    domain::{
        git::Error,
        repository::{Branch, ReferenceKind, project::BranchKey},
    },
    i18n::Key,
};
use iced::Point;
use std::path::PathBuf;

#[derive(Default)]
pub(crate) struct State {
    pub menu: Option<(BranchKey, Branch, Point)>,
    pub dialog: Option<Dialog>,
    pub pending: bool,
    pub history: history::State,
    pub bulk: bulk::State,
    pub(crate) completion: Option<(Key, Option<String>)>,
    pub(crate) request: u64,
}

impl State {
    pub(crate) fn close_menu(&mut self) {
        self.menu = None;
        self.history.clear_menu();
    }

    pub(crate) fn has_dialog(&self) -> bool {
        self.dialog.is_some() || self.bulk.dialog.is_some() || self.history.dialog.is_some()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Action {
    Checkout,
    Create,
    Update,
    Push,
    Rename,
    Delete,
}

pub(crate) struct Dialog {
    pub root: PathBuf,
    pub action: Action,
    pub branch: Branch,
    pub name: String,
    pub remote: String,
    pub checkout: bool,
    pub error: Option<Error>,
    pub expected: Option<(history::Target, history::Context)>,
}

#[derive(Clone, Debug)]
pub(crate) enum Message {
    Menu(BranchKey, Point),
    Bulk(bulk::Event),
    History(Box<history::Event>),
    HistoryBrowserOpened(bool),
    Close,
    Fetch,
    Choose(Action, BranchKey),
    Name(String),
    Remote(String),
    Checkout(bool),
    Submit,
    Finished(u64, PathBuf, Result<(), Error>),
    Fetched(u64, u64, Vec<(PathBuf, Result<(), Error>)>),
}

pub(crate) fn allowed(action: Action, branch: &Branch) -> bool {
    match action {
        Action::Checkout => !branch.current,
        Action::Create => true,
        Action::Update => branch.kind != ReferenceKind::Tag,
        Action::Push | Action::Rename => branch.kind == ReferenceKind::Local,
        Action::Delete => branch.kind != ReferenceKind::Tag && !branch.current,
    }
}

pub(crate) fn error_key(error: Error) -> Key {
    match error {
        Error::Stale => Key::GitHistoryStale,
        Error::Unfinished => Key::GitHistoryUnfinished,
        Error::Failed => Key::GitOperationFailed,
        Error::InvalidName => Key::GitInvalidName,
        Error::MissingUpstream => Key::GitMissingUpstream,
        Error::Dirty => Key::GitDirty,
        Error::CurrentBranch => Key::GitCurrentBranch,
        Error::BranchInWorktree => Key::GitBranchInWorktree,
    }
}
