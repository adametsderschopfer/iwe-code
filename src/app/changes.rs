//! Refresh Git decorations without interrupting navigation or source scrolling.

use super::{App, Message};
use crate::{
    domain::{Failure, changes::Worktree},
    infrastructure::repository,
    ui::code::{Document, syntax_for},
};
use iced::{Subscription, Task};
use std::{collections::BTreeSet, path::PathBuf, sync::Arc, time::Duration};

#[derive(Default)]
pub(crate) struct State {
    pub status: Arc<Worktree>,
    pub error: Option<Failure>,
    pub loaded: bool,
    pending: bool,
    disabled: bool,
    request: u64,
    pub(super) preview_pending: Option<u64>,
    pub(super) baseline: Arc<tokio::sync::Mutex<repository::BaselineCache>>,
}

#[derive(Debug, Clone)]
pub(crate) enum Event {
    Refresh,
    RefreshManually,
    Loaded(u64, u64, Result<Arc<Worktree>, Failure>),
    PreviewLoaded(u64, u64, Result<Option<Box<Document>>, Failure>),
}

fn idle_status(
    (generation, request, root): (u64, u64, PathBuf),
    interval: Duration,
) -> impl iced::futures::Stream<Item = Message> {
    iced::futures::stream::unfold((root, None), move |(root, previous)| async move {
        loop {
            tokio::time::sleep(interval).await;
            let result = repository::working_status(root.clone()).await.map(Arc::new);
            if previous.as_ref() != Some(&result) {
                let message = Message::Worktree(Event::Loaded(generation, request, result.clone()));
                return Some((message, (root, Some(result))));
            }
        }
    })
}

impl App {
    pub(super) fn toggle_worktree_folder(&mut self, repository: Option<PathBuf>, path: String) {
        let prefix = repository
            .as_ref()
            .map(|root| format!("repo:{}:", root.display()))
            .unwrap_or_default();
        let key = format!("{prefix}{path}");
        if self.collapsed_worktree_folders.remove(&key) {
            return;
        }
        self.collapsed_worktree_folders.insert(key);
        let Some(root) = repository.as_ref().or_else(|| self.workspace.directory()) else {
            return;
        };
        let descendant_prefix = format!("{path}/");
        for file in self.changes.status.files.keys() {
            if repository.is_some()
                && self
                    .changes
                    .status
                    .roots
                    .iter()
                    .filter(|candidate| file.starts_with(candidate))
                    .max_by_key(|candidate| candidate.components().count())
                    != Some(root)
            {
                continue;
            }
            let Ok(relative) = file.strip_prefix(root) else {
                continue;
            };
            for folder in relative
                .ancestors()
                .skip(1)
                .filter(|folder| !folder.as_os_str().is_empty())
            {
                let folder = folder
                    .components()
                    .map(|part| part.as_os_str().to_string_lossy())
                    .collect::<Vec<_>>()
                    .join("/");
                if path.is_empty() || folder == path || folder.starts_with(&descendant_prefix) {
                    self.collapsed_worktree_folders
                        .insert(format!("{prefix}{folder}"));
                }
            }
        }
    }

    pub(super) fn change_subscription(&self) -> Subscription<Message> {
        let Some(root) = self.workspace.directory() else {
            return Subscription::none();
        };
        if !self.module_enabled(crate::modules::ModuleId::Git)
            || !self.loaded
            || self.changes.disabled
        {
            return Subscription::none();
        }
        if self.preview().working_paths().is_some()
            && self
                .preview()
                .document
                .as_ref()
                .is_none_or(|doc| doc.page.is_none())
        {
            return iced::time::every(Duration::from_secs(3))
                .map(|_| Message::Worktree(Event::Refresh));
        }
        if self.changes.pending {
            return Subscription::none();
        }
        Subscription::run_with(
            (self.generation, self.changes.request, root.clone()),
            |key| idle_status(key.clone(), Duration::from_secs(3)),
        )
    }

    pub(super) fn refresh_changes_after_file_operation(&mut self) -> Task<Message> {
        self.changes.pending = false;
        self.changes.preview_pending = None;
        self.refresh_changes()
    }

    pub(super) fn refresh_changes(&mut self) -> Task<Message> {
        if !self.module_enabled(crate::modules::ModuleId::Git) {
            return Task::none();
        }
        if self.changes.pending {
            return Task::none();
        }
        let Some(root) = self.workspace.directory().cloned() else {
            return Task::none();
        };
        self.changes.pending = true;
        self.changes.request += 1;
        let (generation, request) = (self.generation, self.changes.request);
        Task::perform(repository::working_status(root), move |result| {
            Message::Worktree(Event::Loaded(generation, request, result.map(Arc::new)))
        })
    }

    pub(super) fn update_changes(&mut self, event: Event) -> Task<Message> {
        match event {
            Event::Refresh => return self.refresh_changes(),
            Event::RefreshManually => {
                self.changes.disabled = false;
                return self.refresh_changes();
            }
            Event::Loaded(generation, request, result)
                if generation == self.generation && request == self.changes.request =>
            {
                self.changes.pending = false;
                self.changes.loaded = true;
                self.changes.error = result.as_ref().err().copied();
                match result {
                    Ok(status) => {
                        if let Some(root) = self.workspace.directory().cloned() {
                            self.remember_project_changes(root, &status);
                        }
                        let previous = std::mem::replace(&mut self.changes.status, status);
                        let mut folders = BTreeSet::new();
                        for path in previous
                            .files
                            .keys()
                            .chain(self.changes.status.files.keys())
                        {
                            if previous.files.get(path) != self.changes.status.files.get(path) {
                                let original = self
                                    .changes
                                    .status
                                    .files
                                    .get(path)
                                    .or_else(|| previous.files.get(path))
                                    .and_then(|change| change.original.as_ref());
                                for path in std::iter::once(path).chain(original) {
                                    folders.extend(
                                        path.ancestors()
                                            .skip(1)
                                            .filter(|parent| self.tree.contains_key(*parent))
                                            .map(std::path::Path::to_owned),
                                    );
                                }
                            }
                        }
                        let refresh =
                            Task::batch(folders.into_iter().map(|path| self.refresh_folder(path)));
                        return Task::batch([
                            refresh,
                            self.refresh_preview(),
                            self.refresh_editor_marks(),
                        ]);
                    }
                    Err(error @ (Failure::NotRepository | Failure::MissingTool)) => {
                        if error == Failure::NotRepository
                            && let Some(root) = self.workspace.directory().cloned()
                        {
                            self.remember_project_changes(root, &Worktree::default());
                        }
                        self.changes.status = Arc::default();
                        self.changes.disabled = true;
                    }
                    Err(_) => {}
                }
            }
            Event::PreviewLoaded(generation, request, result) if generation == self.generation => {
                if self.changes.preview_pending == Some(request) {
                    self.changes.preview_pending = None;
                }
                match result {
                    Ok(Some(document)) => return self.finish_preview(request, Ok(*document)),
                    Err(Failure::TooLarge) => return self.reload_large_preview(request),
                    result => {
                        if let Some(preview) = self.preview_for_request(request) {
                            preview.error = result.err();
                        }
                    }
                }
            }
            _ => {}
        }
        Task::none()
    }

    pub(super) fn refresh_preview(&mut self) -> Task<Message> {
        if !self.module_enabled(crate::modules::ModuleId::Git) {
            return Task::none();
        }
        if self
            .preview()
            .editor
            .as_ref()
            .is_some_and(super::editor::Buffer::protected)
            || self.preview().loading
            || self
                .preview()
                .document
                .as_ref()
                .is_some_and(|document| document.page.is_some())
            || self.preview().revision.is_some()
            || self.changes.preview_pending.is_some()
        {
            return Task::none();
        }
        let preview = self.preview();
        let (Some((root, path)), Some(document)) =
            (preview.working_paths(), preview.document.as_ref())
        else {
            return Task::none();
        };
        let (generation, request) = (self.generation, self.preview().id);
        let fingerprint = document.fingerprint;
        let (root, path) = (root.to_owned(), path.to_owned());
        let compare = preview.working_comparison();
        self.changes.preview_pending = Some(request);
        let snapshot = if compare {
            repository::working_snapshot(&self.changes.status, &path)
        } else {
            None
        };
        let baseline = Arc::clone(&self.changes.baseline);
        Task::perform(
            async move {
                let extension = syntax_for(&path);
                let source = if !compare {
                    crate::domain::changes::Source {
                        text: crate::infrastructure::workspace::read(root, path).await?,
                        ..Default::default()
                    }
                } else if let Some(snapshot) = snapshot {
                    repository::working_file_cached(root, path, snapshot, baseline).await?
                } else {
                    repository::working_file(root, path).await?
                };
                if Document::fingerprint(&source) == fingerprint {
                    return Ok(None);
                }
                super::prepare_document(Ok(source), extension)
                    .await
                    .map(Some)
            },
            move |result| {
                Message::Worktree(Event::PreviewLoaded(
                    generation,
                    request,
                    result.map(|document| document.map(Box::new)),
                ))
            },
        )
    }
}

#[cfg(test)]
mod tests;

impl State {
    pub(crate) fn suspend(&mut self) {
        let request = self.request + 1;
        *self = Self {
            request,
            ..Self::default()
        };
    }
}
