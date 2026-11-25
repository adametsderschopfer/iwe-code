//! Coordinate asynchronous reads, workspace state, and the native window.
pub(crate) mod editor;
pub(crate) mod explain;
pub(crate) mod external;
use crate::modules::ModuleId;
pub(crate) use crate::modules::{git::GitPanel, pipelines::PipelineState};

pub(crate) mod auth;
pub(crate) use modules::git::branch_selection;
pub(crate) mod changes;
pub(crate) mod files;
pub(crate) use modules::git::git_actions;
pub(crate) use modules::pipelines::jobs;
pub(crate) mod line_navigation;
pub(crate) mod mcp;
#[cfg(target_os = "windows")]
pub(crate) mod menu_bar;
pub(crate) mod modules;
#[cfg(target_os = "macos")]
mod native_menu;
pub(crate) mod notifications;
pub(crate) mod project_add;
pub(crate) mod project_changes;
mod repositories;
mod repository_cache;
pub(crate) mod search_everywhere;
pub(crate) mod shortcuts;
pub(crate) mod spaces;
pub(crate) mod tabs;
pub(crate) mod text_search;
pub(crate) mod updates;
pub(crate) mod windows;

use crate::domain::{
    Failure,
    repository::{
        ChangedFile, Provider,
        project::{BranchKey, CommitKey, ProjectHistoryPage as HistoryPage, RepositorySet},
    },
    workspace::{Appearance, FileEntry, FilePanel, MarkdownViewMode, Workspace},
};
use crate::i18n::{I18n, Key, Language};
use crate::infrastructure::{pipelines, workspace};
use crate::ui::{code::Document, screens, theme};
use iced::{Element, Program, Subscription, Task, Theme, keyboard, widget::pane_grid, window};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
    rc::Rc,
};
use tokio_util::sync::CancellationToken;

const COPY_FEEDBACK_DURATION: std::time::Duration = std::time::Duration::from_secs(2);

pub(crate) fn run(i18n: Rc<I18n>) -> iced::Result {
    application(i18n).run()
}

fn application(
    i18n: Rc<I18n>,
) -> iced::Daemon<impl Program<State = App, Message = Message, Theme = Theme>> {
    iced::daemon(
        move || {
            let mut app = App::new(Rc::clone(&i18n));
            app.whats_new = updates::restarted_after_update().then(updates::WhatsNew::current);
            let (id, open) = window::open(windows::main_settings());
            app.window_id = Some(id);
            (
                app,
                Task::batch([
                    open.map(Message::WindowOpened),
                    Task::perform(workspace::load(), Message::Loaded),
                    modules::external::startup()
                        .map(|event| Message::Module(modules::Event::External(event))),
                    Task::done(Message::Update(updates::Event::Automatic(
                        updates::Trigger::Periodic,
                    ))),
                ]),
            )
        },
        |app: &mut App, message| {
            let task = app.update(message);
            #[cfg(target_os = "macos")]
            if app.exit_ready {
                // Closing an AppKit window during runtime teardown can stall WindowServer.
                crate::infrastructure::external::shutdown();
                std::process::exit(0);
            }
            #[cfg(target_os = "macos")]
            let task = Task::batch([task, app.sync_native_menu()]);
            task
        },
        App::view_window,
    )
    .scale_factor(|app: &App, _| f32::from(app.workspace.preferences.window_zoom) / 100.)
    .title(App::window_title)
    .subscription(|app| {
        Subscription::batch([
            if app
                .editor
                .suggestions
                .as_ref()
                .is_some_and(|suggestions| suggestions.opened.elapsed() < theme::COMPLETION_APPEAR)
            {
                iced::time::every(std::time::Duration::from_millis(16))
                    .map(|_| Message::Editor(editor::Event::CompletionFrame))
            } else {
                Subscription::none()
            },
            app.syntax_subscription(),
            app.diagnostics_subscription(),
            app.change_subscription(),
            app.project_change_subscription(),
            app.module_subscription(),
            iced::time::every(std::time::Duration::from_secs(60))
                .map(|_| Message::Update(updates::Event::Automatic(updates::Trigger::Periodic))),
            window::events().filter_map(|(id, event)| match event {
                window::Event::Unfocused => Some(Message::WindowUnfocused(id)),
                window::Event::Focused => Some(Message::WindowFocused(id)),
                _ => None,
            }),
            app.pipeline_subscription(),
            app.jobs_subscription(),
            app.notifications.subscription().map(Message::Notification),
            window::close_requests().map(|id| Message::Window(windows::Event::Close(id))),
            window::close_events().map(|id| Message::Window(windows::Event::Closed(id))),
            #[cfg(target_os = "macos")]
            app.native_menu_subscription(),
            #[cfg(target_os = "macos")]
            app.zoom_subscription(),
            app.spaces_sidebar_subscription(),
            window_state_subscription(),
            external::subscription(),
            iced::event::listen_with(|event, status, id| {
                if let iced::Event::Keyboard(keyboard::Event::ModifiersChanged(modifiers)) = event {
                    Some(Message::WindowModifiers(id, modifiers))
                } else if let iced::Event::Keyboard(event) = event {
                    Some(Message::Shortcut(shortcuts::Event::Key(id, event, status)))
                } else {
                    None
                }
            }),
        ])
    })
    .theme(|app: &App, _| app.theme())
    .font(include_bytes!("../../assets/fonts/JetBrainsMono-Regular.ttf").as_slice())
    .font(include_bytes!("../../assets/fonts/FiraCode-Regular.ttf").as_slice())
    .default_font(theme::font())
    .style(|_, theme| theme::application_style(theme))
}

fn window_state_subscription() -> Subscription<Message> {
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    {
        window::resize_events().map(|(id, _)| Message::WindowResized(id))
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        Subscription::none()
    }
}

#[cfg(test)]
fn shortcut(event: iced::Event) -> Option<Message> {
    let bindings = crate::domain::shortcuts::Shortcuts::default();
    let iced::Event::Keyboard(keyboard::Event::KeyPressed {
        key,
        physical_key,
        modifiers,
        repeat: false,
        ..
    }) = event
    else {
        return None;
    };
    shortcuts::key_message(&key, physical_key, modifiers, &bindings)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Navigation {
    Projects,
    Files,
    Settings,
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum SettingsTab {
    #[default]
    General,
    Code,
    Terminal,
    LanguageServers,
    Modules,
    Connections,
    Deployment,
    Shortcuts,
    About,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Panel {
    Workspace,
    Module(crate::modules::ModuleId),
}
pub(crate) struct App {
    pub i18n: Rc<I18n>,
    pub workspace: Workspace,
    pub navigation: Navigation,
    pub settings_tab: SettingsTab,
    window_zoom_preview: Option<u16>,
    pub shortcuts: shortcuts::State,
    pub modules: modules::State,
    pub panes: pane_grid::State<Panel>,
    focused_panel: Panel,
    pub repository: Option<RepositorySet>,
    pub repository_error: Option<Failure>,
    pub loading: bool,
    pub tree: BTreeMap<PathBuf, Vec<FileEntry>>,
    pub expanded: BTreeSet<PathBuf>,
    pub files: files::State,
    pub file_panel: FilePanel,
    pub file_panel_visible: bool,
    spaces_sidebar_transition: Option<(std::time::Instant, f32)>,
    pub explain: explain::State,
    pub project_menu_page: spaces::ProjectMenuPage,
    pub project_selection: Option<crate::domain::workspace::ProjectSelection>,
    pub project_modifiers: keyboard::Modifiers,
    pub project_menu: Option<(usize, PathBuf, iced::Point)>,
    pub space_menu: Option<(usize, spaces::Menu)>,
    pub repository_menu: Option<iced::Point>,
    #[cfg(target_os = "windows")]
    pub(crate) menu_bar: Option<(menu_bar::Menu, iced::Point)>,
    pub collapsed_worktree_folders: BTreeSet<String>,
    pub changes: changes::State,
    repository_cache: repository_cache::Cache,
    pub project_changes: project_changes::State,
    pub tabs: tabs::State,
    pub markdown: tabs::markdown::Cache,
    path_copy_request: u64,
    copied_path: Option<PathBuf>,
    pub editor: editor::State,
    pub collapsed_spaces: BTreeSet<usize>,
    pub auth_dialog: Option<auth::Dialog>,
    pub space_draft: Option<spaces::Draft>,
    pub project_add: Option<project_add::Draft>,
    pub line_navigation: Option<line_navigation::Dialog>,
    pub search_everywhere: search_everywhere::State,
    pub text_search: text_search::State,
    pub external: external::State,
    pub account_username: Option<String>,
    pub disconnecting: bool,
    pub notifications: notifications::State,
    pub updates: updates::State,
    pub update_dialog: updates::Dialog,
    pub whats_new: Option<updates::WhatsNew>,
    update_schedule: updates::Schedule,
    pub picking: bool,
    pub loaded: bool,
    window_id: Option<window::Id>,
    pub(crate) windows: windows::State,
    #[cfg(target_os = "macos")]
    native_menu: native_menu::State,
    pub window_maximized: bool,
    generation: u64,
    project_directory: Option<PathBuf>,
    repo_request: u64,
    file_request: u64,
    auth_service: crate::infrastructure::auth::Service,
    auth_cancel: Option<CancellationToken>,
    auth_request: u64,
    space_request: u64,
    saving: bool,
    save_pending: bool,
    save_blocked: bool,
    exit_pending: bool,
    #[cfg(target_os = "macos")]
    exit_ready: bool,
    #[cfg(target_os = "macos")]
    exit_fade_started: bool,
}

#[derive(Debug, Clone)]
pub(crate) enum Message {
    TerminalsStopped,
    Explain(explain::Event),
    Editor(editor::Event),
    WindowOpened(window::Id),
    WindowFocused(window::Id),
    WindowModifiers(window::Id, keyboard::Modifiers),
    WindowUnfocused(window::Id),
    Window(windows::Event),
    WindowShortcut(window::Id, Box<Message>),
    #[cfg(target_os = "macos")]
    NativeMenu(native_menu::Event),
    Update(updates::Event),
    DragWindow,
    ToggleWindowMaximized,
    RevealGitBranches,
    RevealRepositoryBranches(PathBuf),
    OpenRepositoryMenu(iced::Point),
    CloseRepositoryMenu,
    ClearBranchAttention(u64),
    WindowMaximized(bool),
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    WindowResized(window::Id),
    #[cfg(target_os = "windows")]
    MinimizeWindow,
    #[cfg(target_os = "windows")]
    CloseWindow,
    #[cfg(target_os = "windows")]
    ResizeWindow(window::Direction),
    #[cfg(target_os = "windows")]
    OpenMenuBar(menu_bar::Menu, iced::Point),
    #[cfg(target_os = "windows")]
    CloseMenuBar,
    #[cfg(target_os = "windows")]
    MenuAbout,
    Loaded(Result<(Workspace, Option<PathBuf>), Failure>),
    Saved(Result<(), Failure>),
    Navigate(Navigation),
    SelectSettingsTab(SettingsTab),
    Module(modules::Event),
    Space(spaces::Message),
    ProjectAdd(project_add::Message),
    Git(git_actions::Message),
    BranchSelection(branch_selection::Event),
    ProjectMenu(usize, PathBuf, iced::Point),
    SpaceMenu(usize, iced::Point),
    OpenSpaceAddMenu(usize),
    SpaceAddAnchor(usize, Option<iced::Point>, Option<iced::Rectangle>),
    CloseSpaceAddMenu(usize),
    CloseSpaceMenu,
    TogglePinned,
    RemoveProjectFromSpace,
    CloseProjectMenu,
    CloseProjectDestinations,
    ProjectMenuPage(spaces::ProjectMenuPage),
    ProjectDestinationsAnchor(usize, PathBuf, iced::Point, Option<iced::Rectangle>),
    ToggleProjectSelection(usize, PathBuf),
    ProjectFolder(bool),
    ProjectFolderOpened(bool),
    TransferProject(usize),
    AddProjectFromMenu(usize, bool),
    DismissDialog,
    SelectProject(usize, usize),
    ToggleSpace(usize),
    Refresh,
    RefreshGitRepositories,
    RepositoryLoaded(u64, u64, Result<RepositorySet, Failure>),
    HistoryLoaded(u64, u64, usize, Result<HistoryPage, Failure>),
    HistoryScrolled {
        offset: f32,
        remaining: f32,
    },
    LoadMoreHistory,
    ToggleFolder(PathBuf),
    SelectFilePanel(FilePanel),
    ToggleFilePanel(FilePanel),
    ToggleSpacesSidebar,
    SpacesSidebarFrame(std::time::Instant),
    ResizeSpacesSidebar(f32, bool),
    ResizeFilePanel(f32, bool),
    MoveFilePanel(usize, usize),
    ToggleWorktreeFolder {
        repository: Option<PathBuf>,
        path: String,
    },
    Files(files::Event),
    Worktree(changes::Event),
    ProjectChanges(project_changes::Event),
    OpenFile(PathBuf),
    External(crate::domain::external::Request),
    InspectWindow(bool, tokio::sync::mpsc::Sender<serde_json::Value>),
    OpenWorkingChange(PathBuf),
    GoToLine(line_navigation::Event),
    SearchEverywhere(search_everywhere::Event),
    TextSearch(text_search::Event),
    CodeSelection(u64, crate::ui::code::selection::Action),
    ReadFilePart(u64, tabs::PageDirection),
    OpenCommitFile(usize),
    FileLoaded(u64, u64, Result<Document, Failure>),
    ToggleSvgSource(u64),
    SetMarkdownViewMode(u64, MarkdownViewMode),
    ImageLoaded(u64, u64, Result<iced::widget::image::Handle, Failure>),
    EditorSyntax(u64, std::sync::Arc<crate::ui::editor::syntax::Prepared>),
    EditorFileRead(u64, u64, Document, Box<editor::Buffer>),
    FilePrepared(u64, u64, tabs::Preparation, Result<Document, Failure>),
    SelectBranch(BranchKey),
    AllRepositories,
    Search(String),
    SelectCommit(CommitKey),
    CloseCommitFiles,
    ToggleCommitFolder(String),
    ToggleBranchFolder(String),
    ChangesLoaded(u64, u64, Result<Vec<ChangedFile>, Failure>),
    TogglePanel(Panel),
    PaneClicked(pane_grid::Pane),
    PipelineSearch(String),
    Resize(pane_grid::ResizeEvent),
    ResizeGit(pane_grid::ResizeEvent),
    RefreshPipelines,
    PollPipelines,
    PipelinesLoaded(u64, u64, pipelines::Loaded),
    Jobs(jobs::Event),
    ChooseProvider(Provider),
    SignIn,
    Auth(auth::Message),
    OpenUrl(String),
    Opened(Result<(), Failure>),
    SetLanguage(Language),
    SetAppearance(Appearance),
    Shortcut(shortcuts::Event),
    SetCodeSize(u16),
    SetTerminalSize(u16),
    SetLineHeight(bool, u16),
    SetFont(bool, crate::domain::workspace::CodeFont),
    SetWindowZoom(u16),
    PreviewWindowZoom(u16),
    ApplyWindowZoom,
    Zoom(i16),
    ChooseLanguageServer(crate::domain::language::Language),
    LanguageServerChosen(crate::domain::language::Language, Option<PathBuf>),
    ResetLanguageServer(crate::domain::language::Language),
    DetectLanguageServer(crate::domain::language::Language),
    LanguageServerDetected(crate::domain::language::Language, Option<String>, bool),
    SetLineNumbers(bool),
    SetRestoreProjectTabs(bool),
    CopyPath,
    PathCopied(u64, PathBuf),
    ClearCopiedPath(u64),
    OpenTabMenu(iced::Point),
    CloseTabMenu,
    CopyCommitId(String),
    CommitIdCopied(u64, String),
    ClearCopiedCommit(u64),
    SelectTab(u64),
    CloseTab(u64),
    CloseActiveTab,
    ReopenClosedTab,
    ToggleUnchanged(u64),
    ToggleDiffView(u64),
    ExpandUnchanged(u64, std::ops::Range<usize>),
    CodeScrolled {
        id: u64,
        x: f32,
        y: f32,
    },
    DiffScrolled {
        id: u64,
        side: crate::ui::code::comparison::Side,
        x: f32,
        y: f32,
    },
    Notification(notifications::Event),
}

impl App {
    pub(crate) fn theme(&self) -> Theme {
        match self.workspace.preferences.appearance {
            Appearance::Dark => Theme::Dark,
            Appearance::Light => Theme::Light,
        }
    }

    pub(crate) fn new(i18n: Rc<I18n>) -> Self {
        let mut app = Self {
            i18n,
            workspace: Workspace::default(),
            navigation: Navigation::Projects,
            settings_tab: SettingsTab::default(),
            window_zoom_preview: None,
            shortcuts: shortcuts::State::default(),
            modules: modules::State::default(),
            focused_panel: Panel::Workspace,
            panes: pane_grid::State::new(Panel::Workspace).0,
            repository: None,
            repository_error: None,
            loading: false,
            tree: BTreeMap::new(),
            expanded: BTreeSet::new(),
            files: files::State::default(),
            file_panel: FilePanel::Project,
            file_panel_visible: true,
            spaces_sidebar_transition: None,
            explain: explain::State::default(),
            project_menu_page: spaces::ProjectMenuPage::Actions,
            project_selection: None,
            project_modifiers: keyboard::Modifiers::empty(),
            project_menu: None,
            space_menu: None,
            repository_menu: None,
            #[cfg(target_os = "windows")]
            menu_bar: None,
            collapsed_worktree_folders: BTreeSet::new(),
            changes: changes::State::default(),
            repository_cache: repository_cache::Cache::default(),
            project_changes: project_changes::State::default(),
            tabs: tabs::State::default(),
            markdown: Default::default(),
            path_copy_request: 0,
            copied_path: None,
            editor: editor::State::default(),
            collapsed_spaces: BTreeSet::new(),
            auth_dialog: None,
            space_draft: None,
            project_add: None,
            line_navigation: None,
            search_everywhere: search_everywhere::State::default(),
            text_search: text_search::State::default(),
            external: external::State::default(),
            account_username: None,
            disconnecting: false,
            notifications: notifications::State::default(),
            updates: if std::env::var_os("IWE_UPDATE_FAILED").as_deref() == Some("1".as_ref()) {
                updates::State::Failed(crate::infrastructure::updates::Error::Install)
            } else {
                updates::State::default()
            },
            picking: false,
            update_schedule: updates::Schedule::default(),
            update_dialog: updates::Dialog::default(),
            whats_new: None,
            loaded: false,
            window_id: None,
            windows: windows::State::default(),
            #[cfg(target_os = "macos")]
            native_menu: native_menu::State::default(),
            window_maximized: false,
            generation: 0,
            project_directory: None,
            repo_request: 0,
            file_request: 0,
            auth_service: crate::infrastructure::auth::Service::new(),
            auth_cancel: None,
            auth_request: 0,
            space_request: 0,
            saving: false,
            save_pending: false,
            save_blocked: false,
            exit_pending: false,
            #[cfg(target_os = "macos")]
            exit_ready: false,
            #[cfg(target_os = "macos")]
            exit_fade_started: false,
        };
        if matches!(app.updates, updates::State::Failed(_)) {
            app.notifications.error(Key::UpdateInstallFailed, None);
        }
        app
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        let task = self.handle_message(message);
        self.tabs.update_markdown(
            self.workspace.preferences.markdown_view_mode,
            &mut self.markdown,
        );
        self.dismiss_stale_color_picker();
        task
    }

    fn handle_message(&mut self, message: Message) -> Task<Message> {
        #[cfg(target_os = "macos")]
        if self.exit_fade_started {
            return match message {
                Message::Window(windows::Event::ExitFadeFinished) => {
                    self.exit_ready = true;
                    Task::none()
                }
                Message::Window(windows::Event::Closed(id)) if self.window_id == Some(id) => {
                    self.exit_ready = true;
                    Task::none()
                }
                _ => Task::none(),
            };
        }
        if self.modules.terminal.lifetime.stopping() {
            return if matches!(message, Message::TerminalsStopped) {
                self.finish_exit()
            } else {
                Task::none()
            };
        }
        if let Message::InspectWindow(include_text, reply) = &message {
            let _ = reply.try_send(external::window_snapshot(self, *include_text));
            return Task::none();
        }
        let file_transaction = self.files.operations.busy
            || self
                .editor
                .pending
                .as_deref()
                .and_then(files::operations::request)
                .is_some();
        if file_transaction
            && matches!(
                message,
                Message::SelectProject(..)
                    | Message::RemoveProjectFromSpace
                    | Message::TransferProject(_)
                    | Message::AddProjectFromMenu(..)
                    | Message::Space(_)
                    | Message::ProjectAdd(_)
                    | Message::External(_)
                    | Message::Module(modules::Event::External(_))
                    | Message::Window(windows::Event::Close(_))
                    | Message::Update(updates::Event::Install)
            )
        {
            self.notifications.error(Key::FileBusy, None);
            return Task::none();
        }
        if self.protect_editor(&message) {
            return self.flush_editor();
        }
        if matches!(message, Message::SelectProject(..)) {
            self.text_search.cancel();
        }
        #[cfg(target_os = "windows")]
        if !matches!(message, Message::OpenMenuBar(..)) {
            self.menu_bar = None;
        }
        if self.editor.pending.is_some() && matches!(message, Message::DismissDialog) {
            return self.update_editor(editor::Event::Cancel);
        }
        use crate::modules::ModuleId;
        let required = match &message {
            Message::OpenWorkingChange(_)
            | Message::Git(_)
            | Message::BranchSelection(_)
            | Message::RevealGitBranches
            | Message::RevealRepositoryBranches(_)
            | Message::OpenRepositoryMenu(_)
            | Message::SelectBranch(_)
            | Message::AllRepositories
            | Message::Search(_)
            | Message::SelectCommit(_)
            | Message::OpenCommitFile(_)
            | Message::LoadMoreHistory
            | Message::HistoryScrolled { .. }
            | Message::HistoryLoaded(..)
            | Message::ChangesLoaded(..)
            | Message::Worktree(_)
            | Message::ProjectChanges(_) => Some(ModuleId::Git),
            Message::Jobs(_)
            | Message::PipelineSearch(_)
            | Message::RefreshPipelines
            | Message::PollPipelines
            | Message::PipelinesLoaded(..)
            | Message::ChooseProvider(_)
            | Message::SignIn => Some(ModuleId::Pipelines),
            Message::SelectSettingsTab(SettingsTab::Connections | SettingsTab::Deployment) => {
                Some(ModuleId::Deployment)
            }
            Message::TogglePanel(Panel::Module(id)) => Some(*id),
            Message::SelectFilePanel(FilePanel::Changes)
            | Message::ToggleFilePanel(FilePanel::Changes) => Some(ModuleId::Git),
            _ => None,
        };
        if required.is_some_and(|id| !self.module_enabled(id)) {
            return Task::none();
        }

        if matches!(
            self.updates,
            updates::State::Saving(_) | updates::State::Installing(_)
        ) && !matches!(
            message,
            Message::Saved(_)
                | Message::Update(updates::Event::InstallFailed(_))
                | Message::Module(modules::Event::Terminal(
                    crate::modules::terminal::Event::Started(..)
                        | crate::modules::terminal::Event::Output(..)
                ))
        ) {
            return Task::none();
        }
        if (self.editor.pending.is_some()
            || self.files.operations.draft.is_some()
            || self.auth_dialog.is_some()
            || self.line_navigation.is_some()
            || self.search_everywhere.dialog.is_some()
            || self.project_add.is_some()
            || self.space_draft.is_some()
            || self.modules.git.git_actions.has_dialog())
            && matches!(
                message,
                Message::Navigate(_)
                    | Message::SelectSettingsTab(_)
                    | Message::Module(modules::Event::Toggle(_))
                    | Message::Module(modules::Event::Terminal(
                        crate::modules::terminal::Event::Start
                            | crate::modules::terminal::Event::Title(..)
                            | crate::modules::terminal::Event::Search(..)
                            | crate::modules::terminal::Event::Close(_)
                            | crate::modules::terminal::Event::Select(_)
                            | crate::modules::terminal::Event::Hide
                            | crate::modules::terminal::Event::Input(..)
                            | crate::modules::terminal::Event::Paste(_)
                            | crate::modules::terminal::Event::Pasted(..)
                            | crate::modules::terminal::Event::Focus(_)
                    ))
                    | Message::SelectProject(_, _)
                    | Message::ToggleProjectSelection(..)
                    | Message::TransferProject(_)
                    | Message::RevealGitBranches
                    | Message::RevealRepositoryBranches(_)
                    | Message::RemoveProjectFromSpace
                    | Message::SelectFilePanel(_)
                    | Message::ToggleFilePanel(_)
                    | Message::ToggleSpacesSidebar
                    | Message::ResizeSpacesSidebar(..)
                    | Message::ResizeFilePanel(_, _)
                    | Message::MoveFilePanel(_, _)
                    | Message::Refresh
                    | Message::RefreshGitRepositories
                    | Message::SelectBranch(_)
                    | Message::AllRepositories
                    | Message::SelectCommit(_)
                    | Message::CopyCommitId(_)
                    | Message::SelectTab(_)
                    | Message::CloseTab(_)
                    | Message::CloseActiveTab
                    | Message::ReopenClosedTab
                    | Message::Update(updates::Event::Download | updates::Event::Install)
                    | Message::ToggleUnchanged(_)
                    | Message::ToggleDiffView(_)
                    | Message::SetMarkdownViewMode(..)
                    | Message::ExpandUnchanged(_, _)
                    | Message::Jobs(
                        jobs::Event::Select(_)
                            | jobs::Event::Back
                            | jobs::Event::Control(_, _)
                            | jobs::Event::Overview(jobs::overview::Event::Open(_, _, _))
                    )
            )
        {
            return Task::none();
        }
        match message {
            Message::EditorSyntax(id, prepared) => {
                if let Some(tab) = self.preview_for_request(id)
                    && let Some(buffer) = &mut tab.editor
                {
                    buffer
                        .syntax
                        .accept(prepared, &buffer.document.text, tab.path.as_deref());
                }
                return Task::none();
            }
            Message::Editor(event) => return self.update_editor(event),
            Message::OpenTabMenu(point)
                if self.space_draft.is_none()
                    && self.project_add.is_none()
                    && self.auth_dialog.is_none()
                    && !self.modules.git.git_actions.has_dialog() =>
            {
                self.space_menu = None;
                self.project_menu = None;
                self.files.menu = None;
                self.modules.git.git_actions.close_menu();
                self.modules.git.git_actions.bulk.menu = None;
                self.tabs.menu = Some(point);
                self.repository_menu = None;
            }
            Message::CloseTabMenu => self.tabs.menu = None,
            Message::OpenRepositoryMenu(point)
                if self.space_draft.is_none()
                    && self.project_add.is_none()
                    && self.auth_dialog.is_none()
                    && self.line_navigation.is_none()
                    && !self.modules.git.git_actions.has_dialog()
                    && self
                        .repository
                        .as_ref()
                        .is_some_and(|repos| repos.repositories.len() > 2) =>
            {
                self.tabs.menu = None;
                self.space_menu = None;
                self.project_menu = None;
                self.files.menu = None;
                self.modules.git.git_actions.close_menu();
                self.modules.git.git_actions.bulk.menu = None;
                self.repository_menu = Some(point);
            }
            Message::CloseRepositoryMenu => self.repository_menu = None,
            Message::RevealRepositoryBranches(root) => {
                return self.reveal_repository_branches(root);
            }
            Message::ProjectChanges(event) => return self.update_project_changes(event),
            Message::Explain(event) => return self.update_explain(event),
            Message::Update(event) => return self.update_release(event),
            Message::GoToLine(event) => return self.update_line_navigation(event),
            Message::SearchEverywhere(event) => return self.update_search_everywhere(event),
            Message::TextSearch(event) => return self.update_text_search(event),
            Message::Git(message) => return self.update_git(message),
            Message::WindowModifiers(id, modifiers) => {
                if self.window_id == Some(id) {
                    self.project_modifiers = modifiers;
                }
                return self
                    .update_branch_selection(branch_selection::Event::Modifiers(id, modifiers));
            }
            Message::WindowUnfocused(id) => {
                if self.window_id == Some(id) {
                    self.project_modifiers = keyboard::Modifiers::empty();
                }
                if self.windows.settings == Some(id) {
                    self.window_zoom_preview = None;
                }
                return self.update_branch_selection(branch_selection::Event::Unfocused(id));
            }
            Message::BranchSelection(event) => return self.update_branch_selection(event),
            Message::ToggleProjectSelection(space, path) => {
                self.toggle_project_selection(space, path);
            }
            Message::ProjectMenu(space, path, point)
                if self.space_draft.is_none()
                    && self.project_add.is_none()
                    && self.auth_dialog.is_none()
                    && !self.modules.git.git_actions.has_dialog() =>
            {
                self.space_menu = None;
                self.modules.git.git_actions.close_menu();
                self.files.menu = None;
                self.project_menu_page = spaces::ProjectMenuPage::Actions;
                if !self
                    .project_selection
                    .as_ref()
                    .is_some_and(|set| set.contains(space, &path))
                {
                    self.project_selection = None;
                }
                self.project_menu = Some((space, path, point));
            }
            Message::ProjectMenuPage(spaces::ProjectMenuPage::Destinations(None)) => {
                if let Some((space, path, point)) = self.project_menu.clone()
                    && self.project_menu_page == spaces::ProjectMenuPage::Actions
                {
                    self.project_menu_page = spaces::ProjectMenuPage::Destinations(None);
                    return iced::widget::selector::find(iced::widget::Id::new(
                        "project-move-spaces",
                    ))
                    .map(move |row| {
                        Message::ProjectDestinationsAnchor(
                            space,
                            path.clone(),
                            point,
                            row.and_then(|item| item.visible_bounds()),
                        )
                    });
                }
            }
            Message::ProjectDestinationsAnchor(space, path, point, anchor)
                if self.project_menu.as_ref() == Some(&(space, path.clone(), point))
                    && self.project_menu_page == spaces::ProjectMenuPage::Destinations(None) =>
            {
                self.project_menu_page = if anchor.is_some() {
                    spaces::ProjectMenuPage::Destinations(anchor)
                } else {
                    spaces::ProjectMenuPage::Actions
                };
            }
            Message::CloseProjectDestinations
                if matches!(
                    self.project_menu_page,
                    spaces::ProjectMenuPage::Destinations(_)
                ) =>
            {
                self.project_menu_page = spaces::ProjectMenuPage::Actions;
            }
            Message::ProjectMenuPage(page) if self.project_menu.is_some() => {
                self.project_menu_page = page
            }
            Message::TransferProject(destination) => {
                return self.change_selected_projects(Some(destination));
            }
            Message::ProjectFolder(copy) => {
                if let Some((_, path, _)) = self.project_menu.take() {
                    if copy {
                        return iced::clipboard::write(path.to_string_lossy().into_owned());
                    }
                    return Task::perform(
                        async move {
                            tokio::task::spawn_blocking(move || open::that(path))
                                .await
                                .is_ok_and(|result| result.is_ok())
                        },
                        Message::ProjectFolderOpened,
                    );
                }
            }
            Message::ProjectFolderOpened(false) => self.notifications.error(Key::FileAccess, None),
            Message::AddProjectFromMenu(index, clone) if self.project_add.is_none() => {
                let open = self.update_project_add(project_add::Message::Open(Some(index)));
                let source = if clone {
                    project_add::Message::Remote
                } else {
                    project_add::Message::Directories
                };
                return Task::batch([open, self.update_project_add(source)]);
            }
            Message::CloseProjectMenu => self.project_menu = None,
            Message::SpaceMenu(index, _) | Message::OpenSpaceAddMenu(index)
                if self.loaded
                    && !self.picking
                    && self.space_draft.is_none()
                    && self.project_add.is_none()
                    && self.auth_dialog.is_none()
                    && !self.modules.git.git_actions.has_dialog()
                    && index < self.workspace.spaces.len() =>
            {
                self.project_menu = None;
                self.modules.git.git_actions.close_menu();
                self.modules.git.git_actions.bulk.menu = None;
                self.files.menu = None;
                self.project_menu_page = spaces::ProjectMenuPage::Actions;
                if let Message::SpaceMenu(_, point) = message {
                    self.space_menu = Some((index, spaces::Menu::Context(point)));
                } else {
                    let parent = self.space_menu.and_then(|(owner, menu)| {
                        (owner == index).then(|| menu.parent()).flatten()
                    });
                    if matches!(self.space_menu, Some((owner, spaces::Menu::AddProject { .. })) if owner == index)
                    {
                        return Task::none();
                    }
                    self.space_menu = Some((
                        index,
                        spaces::Menu::AddProject {
                            parent,
                            anchor: None,
                        },
                    ));
                    return iced::widget::selector::find(spaces::add_project_id(
                        index,
                        parent.is_some(),
                    ))
                    .map(move |target| {
                        Message::SpaceAddAnchor(
                            index,
                            parent,
                            target.and_then(|item| item.visible_bounds()),
                        )
                    });
                }
            }
            Message::SpaceAddAnchor(index, parent, anchor)
                if self.space_menu
                    == Some((
                        index,
                        spaces::Menu::AddProject {
                            parent,
                            anchor: None,
                        },
                    )) =>
            {
                self.space_menu = match anchor {
                    Some(_) => Some((index, spaces::Menu::AddProject { parent, anchor })),
                    None => parent.map(|point| (index, spaces::Menu::Context(point))),
                };
            }
            Message::CloseSpaceAddMenu(owner) if matches!(self.space_menu, Some((index, spaces::Menu::AddProject { .. })) if index == owner) =>
            {
                self.space_menu = self.space_menu.and_then(|(index, menu)| {
                    menu.parent()
                        .map(|point| (index, spaces::Menu::Context(point)))
                });
            }
            Message::CloseSpaceMenu => self.space_menu = None,
            #[cfg(target_os = "windows")]
            Message::OpenMenuBar(menu, point) => {
                self.space_menu = None;
                self.project_menu = None;
                self.files.menu = None;
                self.modules.git.git_actions.close_menu();
                self.modules.git.git_actions.bulk.menu = None;
                self.tabs.menu = None;
                self.repository_menu = None;
                self.menu_bar = Some((menu, point));
            }
            #[cfg(target_os = "windows")]
            Message::CloseMenuBar => self.menu_bar = None,
            #[cfg(target_os = "windows")]
            Message::MenuAbout if !self.modules.deployment.has_dialog() => {
                self.shortcuts = shortcuts::State::default();
                self.settings_tab = SettingsTab::About;
                return self.update(Message::Navigate(Navigation::Settings));
            }
            Message::RemoveProjectFromSpace => return self.change_selected_projects(None),
            Message::TogglePinned => {
                if let Some((space, path, _)) = self.project_menu.take()
                    && let Some(project) = self.workspace.spaces.get_mut(space).and_then(|group| {
                        group
                            .projects
                            .iter_mut()
                            .find(|project| project.directory == path)
                    })
                {
                    project.pinned = !project.pinned;
                    return self.save();
                }
            }
            Message::WindowOpened(id) => return self.window_opened(id),
            Message::WindowFocused(id) => {
                let updates =
                    self.update_release(updates::Event::Automatic(updates::Trigger::Focused));
                let git = if self.window_id == Some(id) {
                    self.refresh_git_repositories()
                } else {
                    Task::none()
                };
                return Task::batch([updates, git]);
            }
            Message::Window(event) => return self.update_window(event),
            #[cfg(target_os = "macos")]
            Message::NativeMenu(event) => return self.update_native_menu(event),
            Message::WindowShortcut(id, message) => {
                if self.windows.is_secondary(id) {
                    return match *message {
                        Message::DismissDialog if self.editor.pending.is_some() => {
                            self.update_editor(editor::Event::Cancel)
                        }
                        Message::DismissDialog
                            if self.windows.is_deployment(id)
                                && self
                                    .modules
                                    .deployment
                                    .has_settings_dialog(self.windows.connections == Some(id)) =>
                        {
                            self.update_module(modules::Event::Deployment(
                                crate::modules::deployment::Event::CloseDialog,
                            ))
                        }
                        Message::DismissDialog | Message::CloseActiveTab => {
                            self.update_window(windows::Event::Close(id))
                        }
                        Message::Navigate(Navigation::Settings) => self.open_settings(),
                        _ => Task::none(),
                    };
                }
                if self.window_id == Some(id) {
                    if matches!(
                        *message,
                        Message::TextSearch(text_search::Event::Open(false))
                    ) && let Some(input) = self.panel_search_input()
                    {
                        return iced::widget::operation::focus(input);
                    }
                    return self.update(*message);
                }
            }
            Message::DragWindow => {
                return self.window_id.map_or_else(Task::none, window::drag);
            }
            #[cfg(target_os = "macos")]
            Message::ToggleWindowMaximized => {
                return self
                    .window_id
                    .map_or_else(Task::none, |id| self.toggle_window_zoom(id));
            }
            #[cfg(not(target_os = "macos"))]
            Message::ToggleWindowMaximized => {
                return self.window_id.map_or_else(Task::none, |id| {
                    window::toggle_maximize::<Message>(id)
                        .chain(window::is_maximized(id).map(Message::WindowMaximized))
                });
            }
            Message::WindowMaximized(maximized) => self.window_maximized = maximized,
            #[cfg(target_os = "macos")]
            Message::WindowResized(id)
                if self.window_id == Some(id) || self.windows.is_secondary(id) =>
            {
                return self.align_window_controls(id);
            }
            #[cfg(target_os = "windows")]
            Message::WindowResized(id) if self.windows.is_secondary(id) => {
                return window::is_maximized(id)
                    .map(move |value| Message::Window(windows::Event::Maximized(id, value)));
            }
            #[cfg(target_os = "windows")]
            Message::WindowResized(id) if self.window_id == Some(id) => {
                return window::is_maximized(id).map(Message::WindowMaximized);
            }
            #[cfg(target_os = "windows")]
            Message::MinimizeWindow => {
                return self
                    .window_id
                    .map_or_else(Task::none, |id| window::minimize(id, true));
            }
            #[cfg(target_os = "windows")]
            Message::CloseWindow => {
                return self.window_id.map_or_else(Task::none, |id| {
                    self.update_window(windows::Event::Close(id))
                });
            }
            #[cfg(target_os = "windows")]
            Message::ResizeWindow(direction) if !self.window_maximized => {
                return self
                    .window_id
                    .map_or_else(Task::none, |id| window::drag_resize(id, direction));
            }
            Message::Loaded(result) => {
                self.loaded = true;
                let mut dialog = Task::none();
                match result {
                    Ok((workspace, directory)) => {
                        self.workspace = workspace;
                        self.workspace.normalize();
                        self.file_panel = self.workspace.preferences.file_panels[0];
                        if let Some(directory) = directory {
                            dialog = self.open_space(vec![directory]);
                        }
                    }
                    Err(Failure::NewerVersion) => {
                        self.notifications.error(Key::StorageNewerVersion, None);
                        self.save_blocked = true;
                    }
                    Err(_) => {
                        self.notifications.error(Key::StorageLoadFailed, None);
                        self.save_blocked = true;
                    }
                }
                self.apply_language();
                return Task::batch([
                    #[cfg(target_os = "macos")]
                    self.align_open_window_controls(),
                    self.load_directory(),
                    self.refresh_project_changes(),
                    self.save(),
                    dialog,
                    external::flush(self),
                ]);
            }
            Message::Saved(result) => {
                self.saving = false;
                if result.is_err() {
                    self.notifications.error(Key::StorageSaveFailed, None);
                }
                if result.is_err() && matches!(self.updates, updates::State::Saving(_)) {
                    return self.restart_after_save(false);
                }
                if self.save_pending {
                    self.save_pending = false;
                    return self.save();
                }
                if matches!(self.updates, updates::State::Saving(_)) {
                    return self.restart_after_save(true);
                }
                if self.exit_pending {
                    self.exit_pending = false;
                    return if result.is_ok() || self.window_id.is_none() {
                        self.finalize_exit()
                    } else {
                        Task::none()
                    };
                }
            }
            Message::Navigate(Navigation::Settings) => return self.open_settings(),
            Message::Navigate(navigation) => {
                self.navigation = navigation;
                self.modules.pipelines.jobs.overview.popup = None;
                return self.refresh_pipeline_overviews(false);
            }
            Message::SelectSettingsTab(tab) => {
                self.window_zoom_preview = None;
                self.shortcuts = shortcuts::State::default();
                let (connections, event) = match tab {
                    SettingsTab::Connections => {
                        (true, crate::modules::deployment::Event::OpenConnections)
                    }
                    SettingsTab::Deployment => {
                        (false, crate::modules::deployment::Event::OpenProfiles)
                    }
                    _ => {
                        self.settings_tab = tab;
                        return Task::none();
                    }
                };
                return Task::batch([
                    self.update_module(modules::Event::Deployment(event)),
                    self.open_deployment_settings(connections),
                ]);
            }
            Message::Shortcut(event) => return self.update_shortcuts(event),
            Message::Module(event) => return self.update_module(event),
            Message::Space(message) => return self.update_space(message),
            Message::ProjectAdd(message) => return self.update_project_add(message),
            Message::DismissDialog => {
                if self.text_search.project.is_some() {
                    return self.update_text_search(text_search::Event::Close(true));
                }
                if self.file_search().is_some() {
                    return self.update_text_search(text_search::Event::Close(false));
                }
                if self.files.operations.draft.is_some() || self.files.menu.is_some() {
                    self.files.menu = None;
                    return self
                        .update_files(files::Event::Operation(files::operations::Event::Cancel));
                }
                if self.modules.deployment.target.is_some()
                    || (self.navigation == Navigation::Settings
                        && self.modules.deployment.has_dialog())
                {
                    return self.update_module(modules::Event::Deployment(
                        crate::modules::deployment::Event::CloseDialog,
                    ));
                }
                if self.modules.deployment.menu.take().is_some() {
                    return Task::none();
                }
                if self.modules.deployment.activity_expanded {
                    return self.update_module(modules::Event::Deployment(
                        crate::modules::deployment::Event::DismissActivity,
                    ));
                }
                if self.repository_menu.take().is_some() {
                    return Task::none();
                }
                if self.tabs.menu.take().is_some() {
                    return Task::none();
                }
                if self.space_menu.take().is_some() {
                    return Task::none();
                }
                if self.search_everywhere.dialog.take().is_some() {
                    return Task::none();
                }
                if self.line_navigation.take().is_some() {
                    return Task::none();
                }
                if self.files.menu.take().is_some() {
                    return Task::none();
                }
                if self.project_menu.take().is_some() {
                    return Task::none();
                }
                if self.project_selection.take().is_some() {
                    return Task::none();
                }
                if self.modules.git.git_actions.menu.is_some()
                    || self.modules.git.git_actions.history.menu.is_some()
                    || self.modules.git.git_actions.bulk.menu.is_some()
                    || self.modules.git.git_actions.has_dialog()
                {
                    return self.update_git(git_actions::Message::Close);
                }
                if self.project_add.is_some() {
                    return self.update_project_add(project_add::Message::Cancel);
                }
                if self.space_draft.is_some() {
                    return self.update_space(spaces::Message::Cancel);
                }
                if !self.modules.git.branch_selection.selected.is_empty() {
                    return self.update_branch_selection(branch_selection::Event::Clear);
                }
                if self.auth_dialog.is_none()
                    && self.modules.pipelines.jobs.overview.popup.take().is_some()
                {
                    return Task::none();
                }
                return self.update_auth(auth::Message::Cancel);
            }
            Message::SelectProject(space, project) => {
                self.project_selection = None;
                self.navigation = Navigation::Projects;
                if self.workspace.selected_space == Some(space)
                    && self.workspace.selected_project == Some(project)
                {
                    return Task::none();
                }
                if self.workspace.select_project(space, project) {
                    return Task::batch([self.load_directory(), self.save()]);
                }
            }
            Message::ToggleSpace(index) if index < self.workspace.spaces.len() => {
                self.space_menu = None;
                if !self.collapsed_spaces.remove(&index) {
                    self.collapsed_spaces.insert(index);
                }
            }
            Message::Refresh => return self.load_directory(),
            Message::RefreshGitRepositories => return self.refresh_git_repositories(),
            Message::SelectFilePanel(panel) => {
                self.file_panel = panel;
                self.file_panel_visible = true;
            }
            Message::ToggleFilePanel(panel) => {
                self.file_panel_visible = !self.file_panel_visible || self.file_panel != panel;
                self.file_panel = panel;
            }
            Message::ToggleSpacesSidebar if self.loaded => {
                self.project_selection = None;
                self.space_menu = None;
                self.project_menu = None;
                let now = std::time::Instant::now();
                let from = self.spaces_sidebar_progress_at(now);
                self.workspace.preferences.show_spaces_sidebar =
                    !self.workspace.preferences.show_spaces_sidebar;
                let animate = self.window_id.is_some();
                #[cfg(target_os = "macos")]
                let animate = animate
                    && !objc2_app_kit::NSWorkspace::sharedWorkspace()
                        .accessibilityDisplayShouldReduceMotion();
                self.spaces_sidebar_transition = animate.then_some((now, from));
                return self.save();
            }
            Message::SpacesSidebarFrame(now) => {
                if self.spaces_sidebar_transition.is_some_and(|(start, _)| {
                    now.saturating_duration_since(start) >= theme::SPACES_SIDEBAR_TRANSITION
                }) {
                    self.spaces_sidebar_transition = None;
                }
            }
            Message::ResizeSpacesSidebar(width, finished)
                if self.workspace.preferences.show_spaces_sidebar
                    && self.spaces_sidebar_transition.is_none()
                    && width.is_finite() =>
            {
                self.workspace.preferences.spaces_sidebar_width =
                    crate::domain::workspace::Preferences::clamp_spaces_sidebar_width(width);
                if finished {
                    return self.save();
                }
            }
            Message::ResizeFilePanel(width, finished)
                if self.file_panel_visible && width.is_finite() =>
            {
                use crate::domain::workspace::Preferences;
                self.workspace.preferences.file_panel_width = width.clamp(
                    Preferences::MIN_FILE_PANEL_WIDTH,
                    Preferences::MAX_FILE_PANEL_WIDTH,
                );
                if finished {
                    return self.save();
                }
            }
            Message::MoveFilePanel(from, to) => {
                let panels = &mut self.workspace.preferences.file_panels;
                if from < panels.len() && to < panels.len() && from != to {
                    let panel = panels.remove(from);
                    panels.insert(to, panel);
                    self.file_panel = panels[0];
                    self.file_panel_visible = true;
                    return self.save();
                }
            }
            Message::ToggleWorktreeFolder { repository, path } => {
                self.toggle_worktree_folder(repository, path)
            }
            Message::RepositoryLoaded(generation, request, result)
                if generation == self.generation && request == self.repo_request =>
            {
                self.loading = false;
                let reload_pipelines =
                    std::mem::take(&mut self.modules.pipelines.pipeline_reload_after_repository);
                match result {
                    Ok(mut repo) => {
                        if repo.repositories.len() <= 2 {
                            self.repository_menu = None;
                        }
                        let multiple = repo.repositories.len() > 1;
                        for member in &mut repo.repositories {
                            if let Some(remote) = member.remote.as_mut()
                                && let Some(provider) = self.workspace.providers.get(&remote.host)
                            {
                                remote.provider = Some(*provider);
                            }
                            let known = self.repository.as_ref().is_some_and(|previous| {
                                previous.repositories.len() > 1
                                    && previous.get(&member.root).is_some()
                            });
                            if multiple && !known {
                                self.modules
                                    .git
                                    .expanded_branch_folders
                                    .insert(format!("repo:{}", member.root.display()));
                                self.modules
                                    .git
                                    .expanded_branch_folders
                                    .insert(format!("{}:false", member.root.display()));
                            }
                        }
                        let changed_remote = self
                            .active_repository()
                            .and_then(|repo| repo.remote.as_ref())
                            != repo.active().and_then(|repo| repo.remote.as_ref());
                        if changed_remote {
                            self.reset_jobs();
                            self.modules.pipelines.pipelines = PipelineState::Idle;
                            self.account_username = None;
                            self.modules.pipelines.pipeline_request += 1;
                            self.modules.pipelines.pipeline_refreshing = false;
                        }
                        if self.modules.git.branch.as_ref().is_some_and(|key| {
                            repo.get(&key.root).is_none_or(|member| {
                                !member
                                    .branches
                                    .iter()
                                    .any(|branch| branch.reference == key.reference)
                            })
                        }) {
                            self.modules.git.branch = None;
                        }
                        if self.modules.git.branch.is_none() && repo.repositories.len() == 1 {
                            self.modules.git.branch =
                                repo.active().and_then(|member| {
                                    member.branches.iter().find(|branch| branch.current).map(
                                        |branch| BranchKey {
                                            root: member.root.clone(),
                                            reference: branch.reference.clone(),
                                        },
                                    )
                                });
                        }
                        self.modules.git.branch_selection.prune(&repo);
                        self.repository = Some(repo);
                        self.repository_error = None;
                        self.cache_repository();
                        self.sync_panels();
                        let pipeline_task = if (self.workspace.preferences.show_pipelines
                            || self.workspace.preferences.show_git)
                            && (reload_pipelines
                                || matches!(self.modules.pipelines.pipelines, PipelineState::Idle))
                        {
                            self.load_pipelines()
                        } else {
                            Task::none()
                        };
                        let history_task = if self.modules.git.query.is_empty() {
                            Task::none()
                        } else {
                            self.load_more_history()
                        };
                        return Task::batch([pipeline_task, history_task]);
                    }
                    Err(error) => {
                        self.repository_error = self.repository.is_none().then_some(error);
                        if error == Failure::NotRepository {
                            self.repository_menu = None;
                            if let Some(root) = self.workspace.directory() {
                                self.repository_cache.remove(root);
                            }
                            self.repository_error = Some(error);
                            self.reset_jobs();
                            self.repository = None;
                            self.modules.pipelines.pipelines = PipelineState::Idle;
                            self.account_username = None;
                            self.modules.pipelines.pipeline_request += 1;
                            self.modules.pipelines.pipeline_refreshing = false;
                        } else if self.repository.is_some() {
                            self.notifications.error(error_key(error), None);
                        }
                        self.sync_panels();
                    }
                }
            }
            Message::HistoryLoaded(generation, request, offset, result)
                if generation == self.generation
                    && request == self.repo_request
                    && self.modules.git.history_loading
                    && self
                        .repository
                        .as_ref()
                        .is_some_and(|repo| repo.commits.len() == offset) =>
            {
                self.modules.git.history_loading = false;
                match result {
                    Ok(page) => {
                        if let Some(repo) = self.repository.as_mut() {
                            repo.commits.extend(page.commits);
                            repo.next_history = page.next;
                        }
                        self.modules.git.history_error = None;
                        if !self.modules.git.query.is_empty() {
                            return self.load_more_history();
                        }
                    }
                    Err(error) => self.modules.git.history_error = Some(error),
                }
            }
            Message::HistoryScrolled { offset, remaining } => {
                self.modules.git.history_scroll_y = offset;
                if remaining <= 480. && self.modules.git.history_error.is_none() {
                    return self.load_more_history();
                }
            }
            Message::LoadMoreHistory => return self.load_more_history(),
            Message::ToggleFolder(path) => return self.toggle_folder(path),
            Message::Files(event) => return self.update_files(event),
            Message::Worktree(event) => return self.update_changes(event),
            Message::ReadFilePart(id, direction) => return self.read_file_part(id, direction),
            Message::CodeSelection(id, action) => {
                if self.preview().id == id
                    && let Some(preview) = self.preview_for_request(id)
                    && let Some(document) = &preview.document
                {
                    preview.selection.update(action, document);
                }
            }
            Message::OpenFile(path) => return self.open_file(path),
            Message::External(request) => return external::open(self, request),
            Message::InspectWindow(..) => return Task::none(),
            Message::OpenWorkingChange(path) => return self.open_working_change(path),
            Message::OpenCommitFile(index) => return self.open_commit_file(index),
            Message::ToggleSvgSource(id) => return self.toggle_svg_source(id),
            Message::SetMarkdownViewMode(id, mode) => return self.set_markdown_view_mode(id, mode),
            Message::ImageLoaded(generation, request, result) if generation == self.generation => {
                if let Some(preview) = self.preview_for_request(request) {
                    preview.loading = false;
                    match result {
                        Ok(handle) => {
                            preview.image = Some(handle);
                            preview.error = None;
                        }
                        Err(error) => {
                            preview.image = None;
                            preview.error = Some(error);
                        }
                    }
                }
            }
            Message::EditorFileRead(generation, request, document, buffer)
                if generation == self.generation =>
            {
                if let Some(tab) = self.preview_for_request(request)
                    && tab.loading
                    && tab.preparation == tabs::Preparation::Text
                    && tab.editor_target(&document).is_some()
                    && tab.editor.as_ref().is_none_or(|current| {
                        !current.protected() && current.document.text != buffer.document.text
                    })
                {
                    tab.editor = Some(*buffer);
                }
                let task = self.continue_preview(request, tabs::Preparation::Text, Ok(document));
                return Task::batch([task, self.reveal_definition()]);
            }
            Message::FilePrepared(generation, request, stage, result)
                if generation == self.generation =>
            {
                let task = self.continue_preview(request, stage, result);
                return Task::batch([task, self.reveal_definition()]);
            }
            Message::FileLoaded(generation, request, result) if generation == self.generation => {
                let task = self.finish_preview(request, result);
                return Task::batch([task, self.reveal_definition()]);
            }
            Message::SelectBranch(reference) => {
                self.modules.git.branch_selection.selected.clear();
                if self
                    .repository
                    .as_ref()
                    .and_then(|repos| repos.get(&reference.root))
                    .is_none_or(|repo| {
                        !repo
                            .branches
                            .iter()
                            .any(|branch| branch.reference == reference.reference)
                    })
                {
                    return Task::none();
                }
                self.modules.git.branch = Some(reference);
                self.close_commit_files();
                return self.load_repository();
            }
            Message::AllRepositories => {
                self.modules.git.branch_selection.selected.clear();
                self.modules.git.branch = None;
                self.close_commit_files();
                return self.load_repository();
            }
            Message::Search(query) => {
                self.modules.git.query = query;
                let scroll = self.reset_history_scroll();
                let history = if self.modules.git.query.is_empty() {
                    Task::none()
                } else {
                    self.load_more_history()
                };
                return Task::batch([scroll, history]);
            }
            Message::ToggleCommitFolder(path) => {
                if !self.modules.git.collapsed_commit_folders.remove(&path) {
                    self.modules.git.collapsed_commit_folders.insert(path);
                }
            }
            Message::ToggleBranchFolder(path) => {
                self.modules.git.branch_selection.selected.clear();
                if !self.modules.git.expanded_branch_folders.remove(&path) {
                    self.modules.git.expanded_branch_folders.insert(path);
                }
            }
            Message::SelectCommit(id) => return self.select_commit(id),
            Message::CloseCommitFiles => self.close_commit_files(),
            Message::ChangesLoaded(generation, request, result)
                if generation == self.generation && request == self.modules.git.commit_request =>
            {
                self.modules.git.changes_loading = false;
                match result {
                    Ok(files) => self.modules.git.changed_files = files,
                    Err(error) => self.modules.git.changes_error = Some(error),
                }
            }
            Message::RevealGitBranches => {
                if self.workspace.directory().is_none() {
                    return Task::none();
                }
                if !self.workspace.preferences.show_git {
                    return self.update(Message::TogglePanel(Panel::Module(
                        crate::modules::ModuleId::Git,
                    )));
                }
                self.modules.git.branch_attention_request =
                    self.modules.git.branch_attention_request.wrapping_add(1);
                let request = self.modules.git.branch_attention_request;
                self.modules.git.branch_attention = Some(request);
                return Task::perform(
                    async {
                        tokio::time::sleep(std::time::Duration::from_millis(700)).await;
                    },
                    move |_| Message::ClearBranchAttention(request),
                );
            }
            Message::ClearBranchAttention(request) => {
                if self.modules.git.branch_attention == Some(request) {
                    self.modules.git.branch_attention = None;
                }
            }
            Message::PaneClicked(pane) => {
                if let Some(panel) = self.panes.get(pane) {
                    self.focused_panel = *panel;
                }
            }
            Message::TogglePanel(panel) => {
                self.modules.git.branch_attention = None;
                self.modules.pipelines.jobs.overview.popup = None;
                let Panel::Module(id) = panel else {
                    return Task::none();
                };
                let open = !self.module_open(id);
                self.focused_panel = if open { panel } else { Panel::Workspace };
                match id {
                    ModuleId::Git => self.workspace.preferences.show_git = open,
                    ModuleId::Pipelines => self.workspace.preferences.show_pipelines = open,
                    _ => return self.update_module(modules::Event::Toggle(id)),
                }
                if open {
                    self.hide_other_bottom_panels(id);
                }
                self.sync_panels();
                let pipelines = if (self.workspace.preferences.show_pipelines
                    || self.workspace.preferences.show_git)
                    && matches!(self.modules.pipelines.pipelines, PipelineState::Idle)
                {
                    self.load_pipelines()
                } else {
                    Task::none()
                };
                let jobs = if self.workspace.preferences.show_pipelines {
                    Task::batch([self.refresh_jobs(), self.refresh_pipeline_overviews(false)])
                } else {
                    Task::none()
                };
                return Task::batch([pipelines, jobs, self.save()]);
            }
            Message::Resize(event) => {
                self.modules.pipelines.jobs.overview.popup = None;
                self.panes
                    .resize(event.split, event.ratio.clamp(0.15, 0.85));
            }
            Message::ResizeGit(event) => self
                .modules
                .git
                .git_panes
                .resize(event.split, event.ratio.clamp(0.12, 0.88)),
            Message::RefreshPipelines => return self.request_pipelines(true),
            Message::PollPipelines => return self.load_pipelines(),
            Message::PipelineSearch(value) => {
                if self.modules.pipelines.jobs.selected.is_some() {
                    self.modules.pipelines.job_query = value;
                } else {
                    self.modules.pipelines.query = value;
                    let overview = &mut self.modules.pipelines.jobs.overview;
                    overview.visible = 0..overview.visible.len().max(1);
                    overview.popup = None;
                    return Task::batch([
                        iced::widget::operation::scroll_to(
                            "pipelines-list",
                            iced::widget::operation::AbsoluteOffset { x: 0., y: 0. },
                        ),
                        self.refresh_pipeline_overviews(false),
                    ]);
                }
            }
            Message::Jobs(event) => return self.update_jobs(event),
            Message::PipelinesLoaded(generation, request, result)
                if generation == self.generation
                    && request == self.modules.pipelines.pipeline_request =>
            {
                self.account_username = result.username;
                self.modules.pipelines.pipeline_refreshing = false;
                match result.runs {
                    Ok(runs) => {
                        if let Some(repo) = self.active_repository()
                            && let Some(remote) = &repo.remote
                        {
                            let root = repo.root.clone();
                            let remote = remote.clone();
                            self.modules.pipelines.pipeline_cache.insert(
                                root,
                                remote,
                                runs.clone(),
                            );
                        }
                        self.modules.pipelines.pipeline_poll_blocked = false;
                        self.modules.pipelines.pipelines = PipelineState::Ready(runs);
                    }
                    Err(error) => {
                        self.modules.pipelines.pipeline_poll_blocked = true;
                        if matches!(
                            error,
                            Failure::Unavailable | Failure::Read | Failure::RateLimited
                        ) && matches!(self.modules.pipelines.pipelines, PipelineState::Ready(_))
                        {
                            self.notifications.error(error_key(error), None);
                        } else {
                            self.modules.pipelines.pipeline_cache.clear();
                            self.modules.pipelines.pipelines = PipelineState::Failed(error);
                        }
                    }
                }
                if let Some(selected) = &mut self.modules.pipelines.jobs.selected
                    && let PipelineState::Ready(runs) = &self.modules.pipelines.pipelines
                    && let Some(run) = runs.iter().find(|run| run.id == selected.id)
                {
                    *selected = run.clone();
                }
                if self.workspace.preferences.show_pipelines
                    && self.navigation != Navigation::Settings
                {
                    return Task::batch([
                        self.refresh_jobs(),
                        self.refresh_pipeline_overviews(false),
                    ]);
                }
            }
            Message::ChooseProvider(provider) => {
                self.reset_jobs();
                self.cancel_auth();
                self.account_username = None;
                self.modules.pipelines.pipelines = PipelineState::Idle;
                if let Some(remote) = self
                    .repository
                    .as_mut()
                    .and_then(RepositorySet::active_mut)
                    .and_then(|repo| repo.remote.as_mut())
                {
                    remote.provider = Some(provider);
                    self.modules.pipelines.pipeline_refreshing = false;
                    self.workspace
                        .providers
                        .insert(remote.host.clone(), provider);
                    return Task::batch([self.load_pipelines(), self.save()]);
                }
            }
            Message::SignIn => return self.begin_auth(),
            Message::Auth(message) => return self.update_auth(message),
            Message::OpenUrl(url) => {
                return Task::perform(pipelines::open_url(url), Message::Opened);
            }
            Message::Opened(Err(_)) => self.notifications.error(Key::OpenFailed, None),
            Message::SetLanguage(language) => {
                self.workspace.preferences.language = Some(language.code().to_owned());
                self.apply_language();
                return self.save();
            }
            Message::SetAppearance(appearance) => {
                self.workspace.preferences.appearance = appearance;
                return self.save();
            }
            Message::SetTerminalSize(size) => {
                self.workspace.preferences.terminal_size = size.clamp(10, 32);
                return self.save();
            }
            Message::SetLineHeight(terminal, height) => {
                let target = if terminal {
                    &mut self.workspace.preferences.terminal_line_height
                } else {
                    &mut self.workspace.preferences.code_line_height
                };
                *target = height.clamp(110, 220);
                return self.save();
            }
            Message::SetFont(terminal, font) => {
                let target = if terminal {
                    &mut self.workspace.preferences.terminal_font
                } else {
                    &mut self.workspace.preferences.code_font
                };
                *target = font;
                return self.save();
            }
            Message::PreviewWindowZoom(zoom) => {
                self.window_zoom_preview = Some(zoom.clamp(75, 200));
            }
            Message::ApplyWindowZoom => {
                if let Some(zoom) = self.window_zoom_preview.take() {
                    return self.update(Message::SetWindowZoom(zoom));
                }
            }
            Message::SetWindowZoom(zoom) => {
                self.window_zoom_preview = None;
                let zoom = zoom.clamp(75, 200);
                if self.workspace.preferences.window_zoom == zoom {
                    return Task::none();
                }
                self.workspace.preferences.window_zoom = zoom;
                return Task::batch([
                    self.save(),
                    #[cfg(target_os = "macos")]
                    self.align_open_window_controls(),
                ]);
            }
            Message::Zoom(delta) => {
                let zoom = if delta == 0 {
                    100
                } else {
                    (self.workspace.preferences.window_zoom as i16 + delta).clamp(75, 200) as u16
                };
                return self.update(Message::SetWindowZoom(zoom));
            }
            Message::SetCodeSize(size) => {
                self.workspace.preferences.code_size = size.clamp(10, 32);
                return self.save();
            }
            Message::ChooseLanguageServer(language) => {
                let title = self.i18n.text(Key::LanguageServerChoosePath);
                return Task::perform(
                    async move {
                        rfd::AsyncFileDialog::new()
                            .set_title(title)
                            .pick_file()
                            .await
                            .map(|file| file.path().to_owned())
                    },
                    move |path| Message::LanguageServerChosen(language, path),
                );
            }
            Message::LanguageServerChosen(language, Some(path)) => {
                let id = language.id().to_owned();
                let Some(path) = path.to_str() else {
                    return Task::none();
                };
                self.editor.server_checks.remove(&id);
                self.workspace
                    .preferences
                    .language_servers
                    .insert(id, path.to_owned());
                return Task::batch([
                    self.save(),
                    self.update(Message::DetectLanguageServer(language)),
                ]);
            }
            Message::ResetLanguageServer(language) => {
                let id = language.id();
                self.editor.server_checks.remove(id);
                self.workspace.preferences.language_servers.remove(id);
                return Task::batch([
                    self.save(),
                    self.update(Message::DetectLanguageServer(language)),
                ]);
            }
            Message::DetectLanguageServer(language) => {
                let configured = self
                    .workspace
                    .preferences
                    .language_servers
                    .get(language.id())
                    .cloned();
                let path = configured.clone();
                return Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || {
                            crate::infrastructure::language_server::discovery::check(
                                language,
                                path.as_deref(),
                            )
                        })
                        .await
                        .unwrap_or(false)
                    },
                    move |found| {
                        Message::LanguageServerDetected(language, configured.clone(), found)
                    },
                );
            }
            Message::LanguageServerDetected(language, configured, found) => {
                if configured.as_ref()
                    == self
                        .workspace
                        .preferences
                        .language_servers
                        .get(language.id())
                {
                    self.editor
                        .server_checks
                        .insert(language.id().to_owned(), found);
                    if found
                        && self
                            .editor
                            .diagnostics
                            .as_ref()
                            .is_some_and(|report| report.failed)
                    {
                        self.editor.diagnostics = None;
                    }
                }
            }
            Message::SetLineNumbers(value) => {
                self.workspace.preferences.line_numbers = value;
                return self.save();
            }
            Message::SetRestoreProjectTabs(value) => {
                self.workspace.preferences.restore_project_tabs = value;
                return self.save();
            }
            Message::CopyPath => {
                if let Some(path) = self.footer_path().map(std::path::Path::to_owned) {
                    self.path_copy_request += 1;
                    return iced::clipboard::write(path.to_string_lossy().into_owned()).chain(
                        Task::done(Message::PathCopied(self.path_copy_request, path)),
                    );
                }
            }
            Message::PathCopied(request, path) if request == self.path_copy_request => {
                self.copied_path = Some(path);
                return Task::perform(
                    async { tokio::time::sleep(COPY_FEEDBACK_DURATION).await },
                    move |()| Message::ClearCopiedPath(request),
                );
            }
            Message::ClearCopiedPath(request) if request == self.path_copy_request => {
                self.copied_path = None;
            }
            Message::CopyCommitId(id) => {
                self.modules.git.copy_request += 1;
                return iced::clipboard::write(id.clone()).chain(Task::done(
                    Message::CommitIdCopied(self.modules.git.copy_request, id),
                ));
            }
            Message::CommitIdCopied(request, id) if request == self.modules.git.copy_request => {
                self.modules.git.copied_commit = Some(id);
                return Task::perform(
                    async { tokio::time::sleep(COPY_FEEDBACK_DURATION).await },
                    move |()| Message::ClearCopiedCommit(request),
                );
            }
            Message::ClearCopiedCommit(request) if request == self.modules.git.copy_request => {
                self.modules.git.copied_commit = None;
            }
            Message::SelectTab(id) => {
                self.text_search.pinned = false;
                self.text_search.project = None;
                return self.select_tab(id);
            }
            Message::CloseTab(id) => return self.close_tab(id),
            Message::ToggleUnchanged(id) => return self.toggle_unchanged(id),
            Message::ToggleDiffView(id) => return self.toggle_diff_view(id),
            Message::ExpandUnchanged(id, range) => return self.expand_unchanged(id, range),
            Message::DiffScrolled { id, side, x, y } => return self.scroll_diff(id, side, x, y),
            Message::CloseActiveTab if self.navigation != Navigation::Settings => {
                return self.close_tab(self.preview().id);
            }
            Message::ReopenClosedTab if self.navigation != Navigation::Settings => {
                return self.reopen_closed_tab();
            }
            Message::CodeScrolled { id, x, y } => {
                if let Some(preview) = self.preview_for_request(id) {
                    preview.scroll_x = x;
                    preview.scroll_y = y;
                }
            }
            Message::Notification(event) => self.notifications.update(event),
            _ => {}
        }
        Task::none()
    }

    fn save(&mut self) -> Task<Message> {
        if self.save_blocked {
            return Task::none();
        }
        self.remember_project_tabs();
        if self.saving {
            self.save_pending = true;
            return Task::none();
        }
        self.saving = true;
        Task::perform(workspace::save(self.workspace.clone()), Message::Saved)
    }

    /// Save the current tab session before leaving the application.
    fn exit(&mut self) -> Task<Message> {
        if self.exit_pending {
            return Task::none();
        }
        if self.loaded && self.workspace.preferences.restore_project_tabs && !self.save_blocked {
            self.exit_pending = true;
            return self.save();
        }
        if self.saving && !self.save_blocked {
            self.exit_pending = true;
            return Task::none();
        }
        self.finalize_exit()
    }

    /// Release external endpoints and background work, then exit.
    fn finalize_exit(&mut self) -> Task<Message> {
        self.files.stop_index();
        if let Some(cancel) = self.auth_cancel.take() {
            cancel.cancel();
        }
        let stopped = self.modules.terminal.lifetime.clone().shutdown();
        self.modules.terminal.reset();
        if let Some(stopped) = stopped {
            return Task::perform(stopped, |()| Message::TerminalsStopped);
        }
        self.finish_exit()
    }

    fn finish_exit(&mut self) -> Task<Message> {
        #[cfg(target_os = "macos")]
        {
            if let Some(id) = self.window_id {
                self.exit_fade_started = true;
                windows::fade_for_exit(id)
            } else {
                self.exit_ready = true;
                Task::none()
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            crate::infrastructure::external::shutdown();
            iced::exit()
        }
    }

    pub(crate) fn window_zoom_preview(&self) -> u16 {
        self.window_zoom_preview
            .unwrap_or(self.workspace.preferences.window_zoom)
    }

    pub(crate) fn footer_path(&self) -> Option<&std::path::Path> {
        self.preview()
            .path
            .as_ref()
            .or(self.workspace.directory())
            .map(PathBuf::as_path)
    }

    pub(crate) fn footer_path_was_copied(&self) -> bool {
        self.copied_path
            .as_deref()
            .is_some_and(|path| self.footer_path() == Some(path))
    }

    fn apply_language(&mut self) {
        let language = Language::select(self.workspace.preferences.language.as_deref());
        self.i18n = Rc::new(I18n::new(language));
    }

    fn load_directory(&mut self) -> Task<Message> {
        self.repository_menu = None;
        if let Some(root) = self.project_directory.as_ref() {
            self.repository_cache.remember_folders(
                root,
                &self.modules.git.branch,
                &self.modules.git.expanded_branch_folders,
            );
        }
        self.tabs.menu = None;
        self.modules.git.branch_selection.selected.clear();
        self.modules.git.git_actions.bulk.menu = None;
        self.modules.git.branch_attention = None;
        self.line_navigation = None;
        self.modules.git.git_actions.close_menu();
        self.project_menu = None;
        self.project_selection = None;
        self.space_menu = None;
        let directory = self.workspace.directory().cloned();
        self.modules
            .deployment
            .sync_context(self.workspace.selected_space, directory.as_deref());
        let previous = std::mem::replace(&mut self.project_directory, directory);
        let same_project = previous == self.project_directory;
        self.cancel_auth();
        self.disconnecting = false;
        self.generation += 1;
        if !same_project {
            let has_cluster_shells = self
                .modules
                .terminal
                .tabs
                .iter()
                .any(|tab| tab.program == Some(crate::modules::terminal::Program::Kubernetes));
            self.modules.kubernetes.switch_project(
                previous.as_deref(),
                self.project_directory.as_deref(),
                has_cluster_shells,
            );
            self.modules
                .terminal
                .switch_project(previous.as_deref(), self.project_directory.as_deref());
            if self.modules.terminal.open {
                self.hide_other_bottom_panels(ModuleId::Terminal);
            }
            self.modules.git.git_actions.history.dialog = None;
            self.modules.git.git_actions.request += 1;
            self.modules.git.git_actions.pending = false;
            self.editor.menu = None;
            self.editor.rename = None;
            self.modules.docker.reset();
            self.modules.pipelines.query.clear();
            self.modules.pipelines.job_query.clear();
            self.file_panel = self.workspace.preferences.file_panels[0];
            self.file_panel_visible = true;
            self.collapsed_worktree_folders.clear();
            self.reset_jobs();
            self.repository = None;
            self.account_username = None;
            self.modules.pipelines.pipelines = PipelineState::Idle;
        }
        self.repository_error = None;
        self.modules.git.history_loading = false;
        self.modules.git.history_error = None;
        self.modules.git.history_scroll_y = 0.;
        self.changes = changes::State::default();
        self.modules.git.branch = None;
        self.modules.git.query.clear();
        self.close_commit_files();
        self.modules.git.expanded_branch_folders = BTreeSet::from(["false".to_owned()]);
        let restored_git = !same_project && self.restore_cached_repository();
        if !restored_git
            && let Some(repos) = &self.repository
            && repos.repositories.len() > 1
        {
            for repo in &repos.repositories {
                self.modules
                    .git
                    .expanded_branch_folders
                    .insert(format!("repo:{}", repo.root.display()));
                self.modules
                    .git
                    .expanded_branch_folders
                    .insert(format!("{}:false", repo.root.display()));
            }
        }
        self.modules.git.collapsed_commit_folders.clear();
        self.modules.git.changed_files.clear();
        self.modules.git.changes_loading = false;
        self.modules.git.changes_error = None;
        self.modules.pipelines.pipeline_refreshing = false;
        self.modules.pipelines.pipeline_reload_after_repository = true;
        self.sync_panels();
        let files = self.load_file_tree();
        let previews = self.sync_preview_project();
        if self.workspace.directory().is_some() {
            Task::batch([
                self.load_repository(),
                self.refresh_changes(),
                files,
                previews,
            ])
        } else {
            self.loading = false;
            Task::none()
        }
    }

    fn show_commit_files(&mut self) {
        if self
            .modules
            .git
            .git_panes
            .iter()
            .any(|(_, panel)| matches!(panel, GitPanel::Files))
        {
            return;
        }
        let commit_pane = self
            .modules
            .git
            .git_panes
            .iter()
            .find(|(_, panel)| matches!(panel, GitPanel::Commits))
            .map(|(pane, _)| *pane);
        if let Some(pane) = commit_pane
            && let Some((_, split)) =
                self.modules
                    .git
                    .git_panes
                    .split(pane_grid::Axis::Vertical, pane, GitPanel::Files)
        {
            self.modules.git.git_panes.resize(split, 0.72);
        }
    }

    fn close_commit_files(&mut self) {
        self.modules.git.git_actions.history.revealed = None;
        self.modules.git.selected_commit = None;
        self.modules.git.changed_files.clear();
        self.modules.git.changes_loading = false;
        self.modules.git.changes_error = None;
        self.modules.git.commit_request += 1;
        let pane = self
            .modules
            .git
            .git_panes
            .iter()
            .find(|(_, panel)| matches!(panel, GitPanel::Files))
            .map(|(pane, _)| *pane);
        if let Some(pane) = pane {
            let _ = self.modules.git.git_panes.close(pane);
        }
    }

    pub(crate) fn has_remote(&self) -> bool {
        self.active_repository()
            .is_some_and(|repo| repo.remote.is_some())
    }

    pub(crate) fn panel_available(&self, id: ModuleId) -> bool {
        self.module_enabled(id)
            && match id {
                ModuleId::Kubernetes | ModuleId::Docker | ModuleId::Terminal | ModuleId::Git => {
                    self.workspace.directory().is_some()
                }
                ModuleId::Pipelines => self.has_remote(),
                ModuleId::Deployment => false,
            }
    }

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    pub(crate) fn menu_actions_enabled(&self) -> bool {
        self.loaded
            && self.editor.pending.is_none()
            && self.space_draft.is_none()
            && self.project_add.is_none()
            && self.auth_dialog.is_none()
            && !self.modules.git.git_actions.has_dialog()
            && self.line_navigation.is_none()
            && !self.modules.deployment.has_dialog()
    }

    #[cfg(target_os = "windows")]
    pub(crate) fn main_window_close(&self) -> Option<Message> {
        self.window_id
            .map(|id| Message::Window(windows::Event::Close(id)))
    }

    fn hide_other_bottom_panels(&mut self, id: ModuleId) {
        self.workspace.preferences.show_git &= id == ModuleId::Git;
        self.workspace.preferences.show_pipelines &= id == ModuleId::Pipelines;
        self.modules.kubernetes.open &= id == ModuleId::Kubernetes;
        self.modules.docker.open &= id == ModuleId::Docker;
        self.modules.terminal.open &= id == ModuleId::Terminal;
    }

    fn sync_panels(&mut self) {
        let visible = [
            (ModuleId::Kubernetes, 0.51),
            (ModuleId::Docker, 0.51),
            (ModuleId::Terminal, 0.68),
            (ModuleId::Git, 0.62),
            (
                ModuleId::Pipelines,
                if self.module_enabled(ModuleId::Kubernetes) {
                    0.64
                } else {
                    0.72
                },
            ),
        ]
        .into_iter()
        .find(|(id, _)| self.panel_available(*id) && self.module_open(*id));
        let panel = visible.map(|(id, _)| Panel::Module(id));
        if self.panes.iter().find_map(|(_, pane)| match pane {
            Panel::Module(_) => Some(*pane),
            Panel::Workspace => None,
        }) == panel
        {
            return;
        }
        let (mut panes, top) = pane_grid::State::new(Panel::Workspace);
        if let Some((id, ratio)) = visible
            && let Some((_, split)) =
                panes.split(pane_grid::Axis::Horizontal, top, Panel::Module(id))
        {
            panes.resize(split, ratio);
        }
        self.panes = panes;
    }

    fn view(&self) -> Element<'_, Message> {
        screens::workspace::view(self)
    }
}

pub(crate) use crate::ui::presentation::error_key;

#[cfg(test)]
mod tests;

#[cfg(test)]
use crate::modules::git::git_panes;

#[cfg(test)]
use crate::domain::repository::Pipeline;
