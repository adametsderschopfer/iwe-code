use crate::domain::repository::project::{BranchKey, CommitKey};
use iced::Point;
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub(crate) enum Target {
    Branch(BranchKey),
    Commit(CommitKey),
}

impl Target {
    pub(crate) fn root(&self) -> &PathBuf {
        match self {
            Self::Branch(key) => &key.root,
            Self::Commit(key) => &key.root,
        }
    }

    pub(crate) fn revision(&self) -> &str {
        match self {
            Self::Branch(key) => &key.reference,
            Self::Commit(key) => &key.id,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Action {
    Tip,
    Merge,
    Rebase,
    SwitchRebase,
    PullMerge,
    PullRebase,
    PushTag,
    DeleteTag,
    Copy,
    Parents,
    Children,
    Snapshot,
    Browser,
    Create,
    Tag,
    Patch,
    Detach,
    Pick,
    Revert,
    Reset,
    Undo,
    Message,
    Continue,
    Abort,
}

impl Action {
    pub(crate) fn reads(self) -> bool {
        matches!(
            self,
            Self::Tip | Self::Parents | Self::Children | Self::Snapshot
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Context {
    pub id: String,
    pub head: String,
    pub branch: String,
    pub parents: Vec<String>,
    pub unfinished: Option<String>,
    pub unpublished: bool,
    pub refs: Vec<String>,
    pub message: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Reset {
    Soft,
    #[default]
    Mixed,
    Hard,
    Keep,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Input {
    pub name: String,
    pub message: String,
    pub checkout: bool,
    pub parent: Option<usize>,
    pub reset: Reset,
    pub path: Option<PathBuf>,
}

#[derive(Clone, Debug)]
pub(crate) struct Dialog {
    pub target: Target,
    pub action: Action,
    pub context: Context,
    pub input: Input,
    pub editor: iced::widget::text_editor::Content,
    pub output: Option<Output>,
    pub error: Option<crate::domain::git::Error>,
}

#[derive(Clone, Debug)]
pub(crate) enum Output {
    Done,
    Navigate(Vec<String>),
    Snapshot(Vec<String>),
}

#[derive(Clone, Debug)]
pub(crate) enum Event {
    Menu(CommitKey, Point),
    Choose(Target, Action),
    Inspected(
        u64,
        Target,
        Action,
        Result<Context, crate::domain::git::Error>,
    ),
    MenuInspected(u64, Result<Context, crate::domain::git::Error>),
    Input(Input),
    Edit(iced::widget::text_editor::Action),
    Browse,
    Path(u64, Option<PathBuf>),
    Submit,
    Finished(
        u64,
        Result<Output, crate::domain::git::Error>,
        Option<Context>,
    ),
    Navigate(String),
    ReadFile(String),
    CommitLoaded(
        u64,
        CommitKey,
        Result<crate::domain::repository::Commit, crate::domain::git::Error>,
    ),
}

#[derive(Default)]
pub(crate) struct State {
    pub revealed: Option<crate::domain::repository::project::ScopedCommit>,
    pub menu: Option<(CommitKey, Point)>,
    pub commit_menu_loading: bool,
    pub menu_context: Option<Context>,
    pub dialog: Option<Dialog>,
}

impl State {
    pub(crate) fn clear_menu(&mut self) {
        self.menu = None;
        self.commit_menu_loading = false;
        self.menu_context = None;
    }
}
