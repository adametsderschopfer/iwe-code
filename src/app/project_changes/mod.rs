//! Refresh sidebar indicators independently of the selected project.

use super::{App, Message};
use crate::{
    domain::{Failure, changes::Worktree},
    infrastructure::repository,
};
use iced::{
    Subscription, Task,
    futures::{StreamExt, stream},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    time::Duration,
};

#[derive(Default)]
pub(crate) struct State {
    known: BTreeMap<PathBuf, bool>,
    last_scan: BTreeMap<PathBuf, Result<bool, Failure>>,
    stable_scans: u8,
    pending: bool,
    request: u64,
}

#[derive(Debug, Clone)]
pub(crate) enum Event {
    Refresh,
    Loaded(u64, u64, Vec<(PathBuf, Result<bool, Failure>)>),
}

impl App {
    pub(super) fn project_change_subscription(&self) -> Subscription<Message> {
        if self.module_enabled(crate::modules::ModuleId::Git)
            && self.loaded
            && self
                .workspace
                .spaces
                .iter()
                .flat_map(|space| &space.projects)
                .any(|project| Some(&project.directory) != self.workspace.directory())
        {
            iced::time::every(self.project_changes.interval())
                .map(|_| Message::ProjectChanges(Event::Refresh))
        } else {
            Subscription::none()
        }
    }

    pub(crate) fn project_has_changes(&self, root: &Path) -> bool {
        if self
            .workspace
            .directory()
            .is_some_and(|selected| selected == root)
            && self.changes.loaded
            && self.changes.error.is_none()
            && (!self.changes.status.files.is_empty() || self.changes.status.failures.is_empty())
        {
            return !self.changes.status.files.is_empty();
        }
        self.project_changes
            .known
            .get(root)
            .copied()
            .unwrap_or(false)
    }

    pub(super) fn remember_project_changes(&mut self, root: PathBuf, status: &Worktree) {
        if !status.files.is_empty() || status.failures.is_empty() {
            self.project_changes
                .known
                .insert(root, !status.files.is_empty());
        }
    }

    pub(super) fn refresh_project_changes(&mut self) -> Task<Message> {
        if !self.module_enabled(crate::modules::ModuleId::Git)
            || !self.loaded
            || self.project_changes.pending
        {
            return Task::none();
        }
        let roots: BTreeSet<_> = self
            .workspace
            .spaces
            .iter()
            .flat_map(|space| {
                space
                    .projects
                    .iter()
                    .map(|project| project.directory.clone())
            })
            .collect();
        self.project_changes
            .known
            .retain(|root, _| roots.contains(root));
        let roots: Vec<_> = roots
            .into_iter()
            .filter(|root| Some(root) != self.workspace.directory())
            .collect();
        if roots.is_empty() {
            return Task::none();
        }
        self.project_changes.pending = true;
        self.project_changes.request += 1;
        let request = self.project_changes.request;
        let generation = self.generation;
        let results = paced_roots(roots)
            .map(move |root| async move {
                let result = repository::has_working_changes(&root).await;
                (root, result)
            })
            .buffer_unordered(2);
        Task::perform(results.collect(), move |results| {
            Message::ProjectChanges(Event::Loaded(request, generation, results))
        })
    }

    pub(super) fn update_project_changes(&mut self, event: Event) -> Task<Message> {
        match event {
            Event::Refresh => return self.refresh_project_changes(),
            Event::Loaded(request, generation, results)
                if request == self.project_changes.request && self.project_changes.pending =>
            {
                self.project_changes.pending = false;
                if generation != self.generation {
                    return Task::none();
                }
                let listed: BTreeSet<_> = self
                    .workspace
                    .spaces
                    .iter()
                    .flat_map(|space| space.projects.iter().map(|project| &project.directory))
                    .collect();
                let scan: BTreeMap<_, _> = results
                    .into_iter()
                    .filter(|(root, _)| {
                        listed.contains(root) && self.workspace.directory() != Some(root)
                    })
                    .collect();
                self.project_changes.stable_scans = if scan == self.project_changes.last_scan {
                    (self.project_changes.stable_scans + 1).min(3)
                } else {
                    0
                };
                for (root, result) in &scan {
                    match result {
                        Ok(dirty) => {
                            self.project_changes.known.insert(root.clone(), *dirty);
                        }
                        Err(Failure::NotRepository) => {
                            self.project_changes.known.remove(root);
                        }
                        Err(_) => {}
                    }
                }
                self.project_changes.last_scan = scan;
            }
            _ => {}
        }
        Task::none()
    }
}

/// Start the first check immediately. Spread later launches over a four-second delay budget.
fn paced_roots(roots: Vec<PathBuf>) -> impl iced::futures::Stream<Item = PathBuf> {
    let count = u32::try_from(roots.len()).unwrap_or(u32::MAX).max(1);
    let spacing = Duration::from_millis(150).min(Duration::from_secs(4) / count);
    stream::iter(roots.into_iter().enumerate()).then(move |(index, root)| async move {
        if index > 0 {
            tokio::time::sleep(spacing).await;
        }
        root
    })
}

#[cfg(test)]
mod tests;

impl State {
    fn interval(&self) -> Duration {
        Duration::from_secs((10_u64 << self.stable_scans).min(60))
    }

    pub(crate) fn suspend(&mut self) {
        let request = self.request + 1;
        *self = Self {
            request,
            ..Self::default()
        };
    }
}
