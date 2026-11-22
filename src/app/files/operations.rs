//! Coordinate file commands, editor saves, and derived workspace state.

use super::{App, Message};
use crate::{
    i18n::Key,
    infrastructure::file_operations::{self, Error, Operation, Request},
};
use iced::{Task, widget::operation};
use std::path::{Path, PathBuf};

pub(crate) const NAME_INPUT: &str = "project-file-name";

#[derive(Debug, Clone, Copy)]
pub(crate) enum Command {
    CreateFile,
    CreateTypedFile(FileKind),
    CreateFolder,
    Copy,
    Cut,
    Paste,
    Duplicate,
    Rename,
    Move,
    Trash,
    Reveal,
    OpenTerminal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FileKind {
    Html,
    Css,
    JavaScript,
    TypeScript,
    Python,
    Php,
}

impl FileKind {
    const WEB: [Self; 4] = [Self::Html, Self::Css, Self::JavaScript, Self::TypeScript];

    pub(crate) fn preset(self) -> (Key, &'static str) {
        match self {
            Self::Html => (Key::FileNewHtml, "index.html"),
            Self::Css => (Key::FileNewCss, "style.css"),
            Self::JavaScript => (Key::FileNewJavaScript, "script.js"),
            Self::TypeScript => (Key::FileNewTypeScript, "script.ts"),
            Self::Python => (Key::FileNewPython, "main.py"),
            Self::Php => (Key::FileNewPhp, "index.php"),
        }
    }
}

fn project_file_kinds<'a>(names: impl Iterator<Item = &'a str>) -> Vec<FileKind> {
    let (mut web, mut python, mut php) = (false, false, false);
    for name in names {
        match name {
            "package.json" | "tsconfig.json" => web = true,
            "pyproject.toml" | "requirements.txt" | "Pipfile" | "setup.py" => python = true,
            "composer.json" => php = true,
            _ => match Path::new(name)
                .extension()
                .and_then(|extension| extension.to_str())
            {
                Some("html" | "css" | "js" | "jsx" | "ts" | "tsx" | "vue" | "svelte") => web = true,
                Some("py" | "pyi") => python = true,
                Some("php") => php = true,
                _ => {}
            },
        }
    }
    let fallback = !(web || python || php);
    let mut kinds = Vec::new();
    if web || php || fallback {
        kinds.extend(FileKind::WEB);
    }
    if python || fallback {
        kinds.push(FileKind::Python);
    }
    if php || fallback {
        kinds.push(FileKind::Php);
    }
    kinds
}

#[derive(Debug, Clone)]
pub(crate) enum Event {
    Command(Command),
    Name(String),
    Confirm,
    Cancel,
    Picked(u64, PathBuf, Option<PathBuf>),
    Suggested(PathBuf, Result<String, Error>),
    Run(Request),
    Done(Request, Result<(), Error>),
}

#[derive(Clone)]
pub(crate) struct Draft {
    pub title: Key,
    pub name: String,
    pub error: Option<Key>,
    pub parent: Option<PathBuf>,
    source: Option<PathBuf>,
    operation: Operation,
    extension: Option<&'static str>,
}

#[derive(Default)]
pub(crate) struct State {
    clipboard: Option<(PathBuf, Operation)>,
    pub draft: Option<Draft>,
    pub busy: bool,
}

impl State {
    pub(crate) fn can_paste(&self) -> bool {
        self.clipboard.is_some()
    }
}

pub(crate) fn request(message: &Message) -> Option<&Request> {
    match message {
        Message::Files(super::Event::Operation(Event::Run(request))) => Some(request),
        _ => None,
    }
}

impl App {
    pub(crate) fn new_file_kinds(&self, path: &Path) -> Vec<FileKind> {
        let root = self
            .path_repository_root(path)
            .or_else(|| self.workspace.directory().map(PathBuf::as_path));
        let names = root
            .into_iter()
            .chain(std::iter::once(path))
            .chain(path.parent())
            .filter(|directory| root.is_none_or(|root| directory.starts_with(root)))
            .filter_map(|directory| self.tree.get(directory))
            .flatten()
            .filter(|entry| !entry.directory)
            .map(|entry| entry.name.as_str());
        project_file_kinds(names)
    }

    pub(crate) fn file_source_protected(&self, path: &Path) -> bool {
        self.workspace.directory().is_some_and(|root| path == root)
            || self
                .changes
                .status
                .roots
                .iter()
                .any(|root| root.starts_with(path))
            || self
                .repository
                .iter()
                .flat_map(|set| &set.repositories)
                .any(|repository| repository.root.starts_with(path))
            || path
                .components()
                .any(|component| component.as_os_str().eq_ignore_ascii_case(".git"))
    }

    pub(super) fn update_file_operation(&mut self, event: Event) -> Task<Message> {
        use Event::*;
        let Some(root) = self.workspace.directory().cloned() else {
            return Task::none();
        };
        if self.files.operations.busy && !matches!(event, Done(..) | Suggested(..) | Picked(..)) {
            return Task::none();
        }
        match event {
            Command(command) => self.file_command(root, command),
            Name(name) => {
                if let Some(draft) = &mut self.files.operations.draft {
                    draft.name = name;
                    draft.error = None;
                }
                Task::none()
            }
            Confirm => {
                let Some(draft) = &mut self.files.operations.draft else {
                    return Task::none();
                };
                let destination = if let Some(parent) = &draft.parent {
                    if let Err(error) = file_operations::validate_name(&draft.name) {
                        draft.error = Some(error_key(error));
                        return Task::none();
                    }
                    let mut destination = parent.join(&draft.name);
                    if destination.extension().is_none()
                        && let Some(extension) = draft.extension
                    {
                        destination.set_extension(extension);
                    }
                    if draft.operation == Operation::Move
                        && draft.source.as_ref() == Some(&destination)
                    {
                        self.files.operations.draft = None;
                        return Task::none();
                    }
                    Some(destination)
                } else {
                    None
                };
                let request = Request {
                    operation: draft.operation,
                    source: draft.source.clone(),
                    destination,
                };
                self.update(message(Run(request)))
            }
            Cancel => {
                self.files.operations.draft = None;
                Task::none()
            }
            Picked(generation, source, parent) => {
                self.files.operations.busy = false;
                let Some(parent) = parent.filter(|_| generation == self.generation) else {
                    return Task::none();
                };
                if !parent.starts_with(&root) {
                    self.notifications.error(Key::FileProtected, None);
                    return Task::none();
                }
                let name = display_name(&source);
                self.file_draft(
                    Key::FileMove,
                    Operation::Move,
                    Some(source),
                    Some(parent),
                    name,
                )
            }
            Suggested(source, result) => {
                self.files.operations.busy = false;
                match result {
                    Ok(name) => self.file_draft(
                        Key::FileDuplicate,
                        Operation::Copy,
                        Some(source.clone()),
                        source.parent().map(Path::to_path_buf),
                        name,
                    ),
                    Err(error) => {
                        self.notifications.error(error_key(error), None);
                        Task::none()
                    }
                }
            }
            Run(request) => {
                self.files.operations.busy = true;
                let repositories = self
                    .changes
                    .status
                    .roots
                    .iter()
                    .cloned()
                    .chain(
                        self.repository
                            .iter()
                            .flat_map(|set| &set.repositories)
                            .map(|repository| repository.root.clone()),
                    )
                    .collect::<Vec<_>>();
                Task::perform(
                    async move {
                        let result = blocking({
                            let request = request.clone();
                            move || file_operations::run(&root, &repositories, &request)
                        })
                        .await;
                        (request, result)
                    },
                    |(request, result)| message(Done(request, result)),
                )
            }
            Done(request, result) => self.finish_file_operation(&root, request, result),
        }
    }

    fn file_command(&mut self, root: PathBuf, command: Command) -> Task<Message> {
        let Some(menu) = self.files.menu.take() else {
            return Task::none();
        };
        let source = menu.path;
        let parent = if source == root
            || self
                .tree
                .get(source.parent().unwrap_or(&root))
                .into_iter()
                .flatten()
                .any(|entry| entry.path == source && entry.directory)
        {
            source.clone()
        } else {
            source.parent().unwrap_or(&root).to_path_buf()
        };
        if matches!(
            command,
            Command::Copy
                | Command::Cut
                | Command::Duplicate
                | Command::Rename
                | Command::Move
                | Command::Trash
        ) && self.file_source_protected(&source)
        {
            self.notifications.error(Key::FileProtected, None);
            return Task::none();
        }
        match command {
            Command::OpenTerminal => self.start_local_terminal(&parent),
            Command::CreateFile => self.file_draft(
                Key::FileNewFile,
                Operation::CreateFile,
                None,
                Some(parent),
                String::new(),
            ),
            Command::CreateTypedFile(kind) => {
                let (title, name) = kind.preset();
                let task = self.file_draft(
                    title,
                    Operation::CreateFile,
                    None,
                    Some(parent),
                    name.into(),
                );
                if let Some(draft) = &mut self.files.operations.draft {
                    draft.extension = Path::new(name).extension().and_then(|value| value.to_str());
                }
                task
            }
            Command::CreateFolder => self.file_draft(
                Key::FileNewFolder,
                Operation::CreateFolder,
                None,
                Some(parent),
                String::new(),
            ),
            Command::Copy | Command::Cut => {
                let operation = if matches!(command, Command::Cut) {
                    Operation::Move
                } else {
                    Operation::Copy
                };
                self.files.operations.clipboard = Some((source, operation));
                Task::none()
            }
            Command::Paste => {
                let Some((source, operation)) = self.files.operations.clipboard.clone() else {
                    return Task::none();
                };
                let name = display_name(&source);
                let title = if operation == Operation::Move {
                    Key::FileMove
                } else {
                    Key::FilePaste
                };
                let focus = self.file_draft(title, operation, Some(source), Some(parent), name);
                let run = self.update_file_operation(Event::Confirm);
                if self.files.operations.busy || self.editor.pending.is_some() {
                    run
                } else {
                    Task::batch([focus, run])
                }
            }
            Command::Duplicate => {
                self.files.operations.busy = true;
                Task::perform(
                    async move {
                        let result = blocking({
                            let source = source.clone();
                            move || file_operations::duplicate_name(&root, &source)
                        })
                        .await;
                        (source, result)
                    },
                    |(source, result)| message(Event::Suggested(source, result)),
                )
            }
            Command::Rename => self.file_draft(
                Key::FileRename,
                Operation::Move,
                Some(source.clone()),
                source.parent().map(Path::to_path_buf),
                display_name(&source),
            ),
            Command::Move => {
                self.files.operations.busy = true;
                let generation = self.generation;
                Task::perform(
                    async move {
                        let selected = rfd::AsyncFileDialog::new()
                            .set_directory(root)
                            .pick_folder()
                            .await;
                        (source, selected.map(|folder| folder.path().to_path_buf()))
                    },
                    move |(source, parent)| message(Event::Picked(generation, source, parent)),
                )
            }
            Command::Trash => self.file_draft(
                Key::FileTrash,
                Operation::Trash,
                Some(source.clone()),
                None,
                display_name(&source),
            ),
            Command::Reveal => self.update_file_operation(Event::Run(Request {
                operation: Operation::Reveal,
                source: Some(source),
                destination: None,
            })),
        }
    }

    fn file_draft(
        &mut self,
        title: Key,
        operation: Operation,
        source: Option<PathBuf>,
        parent: Option<PathBuf>,
        name: String,
    ) -> Task<Message> {
        let input = parent.is_some();
        let selected_text = if operation == Operation::CreateFile
            || (title == Key::FileRename
                && source.as_ref().is_some_and(|path| {
                    self.tree
                        .get(path.parent().unwrap_or(path))
                        .into_iter()
                        .flatten()
                        .any(|entry| entry.path == *path && !entry.directory)
                })) {
            Path::new(&name)
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or(&name)
        } else {
            &name
        };
        let select = iced::widget::text_input::Value::new(selected_text).len();
        self.files.operations.draft = Some(Draft {
            title,
            name,
            parent,
            source,
            operation,
            extension: None,
            error: None,
        });
        if input {
            operation::focus(NAME_INPUT).chain(operation::select_range(NAME_INPUT, 0, select))
        } else {
            Task::none()
        }
    }

    fn finish_file_operation(
        &mut self,
        root: &Path,
        request: Request,
        result: Result<(), Error>,
    ) -> Task<Message> {
        self.files.operations.busy = false;
        let focus = self
            .files
            .operations
            .draft
            .as_ref()
            .is_some_and(|draft| draft.parent.is_some());
        if let Err(error) = result {
            let key = error_key(error);
            if let Some(draft) = &mut self.files.operations.draft {
                draft.error = Some(key);
                return if focus {
                    operation::focus(NAME_INPUT)
                } else {
                    Task::none()
                };
            }
            self.notifications.error(key, None);
            return Task::none();
        }

        self.files.operations.draft = None;
        if request.operation == Operation::Reveal {
            return Task::none();
        }
        let mut tabs = Task::none();
        if matches!(request.operation, Operation::Move | Operation::Trash)
            && let Some(source) = &request.source
        {
            tabs = self.reconcile_file_tabs(source, request.destination.as_deref());
            self.tree.retain(|path, _| !path.starts_with(source));
            self.expanded.retain(|path| !path.starts_with(source));
            if let Some(entries) = source.parent().and_then(|parent| self.tree.get_mut(parent)) {
                entries.retain(|entry| entry.path != *source);
            }
            if self
                .files
                .operations
                .clipboard
                .as_ref()
                .is_some_and(|(path, _)| path.starts_with(source))
            {
                self.files.operations.clipboard = None;
            }
        }
        let selected = request.destination.clone().or_else(|| {
            request
                .source
                .as_deref()
                .and_then(Path::parent)
                .map(Path::to_path_buf)
        });
        self.files.selected_path = selected;
        if let Some(parent) = request.destination.as_deref().and_then(Path::parent) {
            self.expanded.extend(
                parent
                    .ancestors()
                    .take_while(|path| path.starts_with(root))
                    .map(Path::to_path_buf),
            );
        }
        let affected = [request.source.as_deref(), request.destination.as_deref()]
            .into_iter()
            .flatten()
            .filter_map(Path::parent)
            .map(Path::to_path_buf)
            .collect::<Vec<_>>();
        self.files.worker = None;
        let tree = self.load_file_tree();
        self.search_everywhere.tree_changed();
        let changes = self.refresh_changes_after_file_operation();
        let folders = Task::batch(
            affected
                .into_iter()
                .map(|folder| self.refresh_folder(folder)),
        );
        let open = if request.operation == Operation::CreateFile {
            request
                .destination
                .map(|path| self.open_file(path))
                .unwrap_or_else(Task::none)
        } else {
            self.refresh_preview()
        };
        Task::batch([tree, folders, changes, tabs, open])
    }
}

async fn blocking<T: Send + 'static>(
    action: impl FnOnce() -> Result<T, Error> + Send + 'static,
) -> Result<T, Error> {
    tokio::task::spawn_blocking(action)
        .await
        .unwrap_or(Err(Error::Access))
}

fn display_name(path: &Path) -> String {
    path.file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned()
}

pub(crate) fn message(event: Event) -> Message {
    Message::Files(super::Event::Operation(event))
}

fn error_key(error: Error) -> Key {
    match error {
        Error::InvalidName => Key::FileInvalidName,
        Error::Protected => Key::FileProtected,
        Error::Missing => Key::FileMissing,
        Error::Conflict => Key::FileNameConflict,
        Error::CrossVolume => Key::FileCrossVolume,
        Error::Unsupported => Key::FileUnsupported,
        Error::Access => Key::FileAccess,
        Error::Trash => Key::FileTrashFailed,
    }
}
