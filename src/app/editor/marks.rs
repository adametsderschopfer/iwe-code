//! Compare the current editable snapshot with its repository baseline on a worker.

use super::{App, Message};
use crate::{
    domain::{changes::FileStatus, editor::hunks},
    infrastructure::repository,
};
use iced::{Point, Task};
use std::sync::Arc;

#[derive(Debug, Clone, Default)]
pub(crate) struct Marks {
    request: u64,
    pub before: Arc<String>,
    pub after: Arc<String>,
    pub hunks: Arc<Vec<hunks::Hunk>>,
    pub open: Option<(usize, Point)>,
    pub page: usize,
}

#[derive(Debug, Clone)]
pub(crate) enum Event {
    After(u64, u64),
    Compared(
        u64,
        u64,
        u64,
        Arc<String>,
        Arc<String>,
        Arc<Vec<hunks::Hunk>>,
    ),
    Show(u64, usize, Point),
    Page(bool),
    Close,
}

impl App {
    pub(in crate::app) fn refresh_editor_marks(&mut self) -> Task<Message> {
        if !self.module_enabled(crate::modules::ModuleId::Git) {
            return Task::none();
        }
        let id = self.preview().id;
        let Some((root, path)) = self.preview().working_paths() else {
            return Task::none();
        };
        let (root, path) = (root.to_owned(), path.to_owned());
        let Some(snapshot) = repository::working_snapshot(&self.changes.status, &path) else {
            return Task::none();
        };
        let status = self.changes.status.status(&path, false);
        let cache = self.changes.baseline.clone();
        let generation = self.generation;
        let Some(buffer) = self
            .preview_for_request(id)
            .and_then(|tab| tab.editor.as_mut())
        else {
            return Task::none();
        };
        buffer.marks.request += 1;
        if status == Some(FileStatus::Ignored) || (status.is_none() && !buffer.document.dirty()) {
            buffer.marks.hunks = Arc::default();
            buffer.marks.open = None;
            return Task::none();
        }
        let request = buffer.marks.request;
        let after = buffer.document.text.clone();
        Task::perform(
            async move {
                let before = repository::cached_baseline(&root, &path, &snapshot, cache)
                    .await
                    .ok()
                    .flatten();
                let after_copy = after.clone();
                let (before, hunks) = tokio::task::spawn_blocking(move || {
                    let hunks = before
                        .as_ref()
                        .map(|before| hunks::compare(before, &after_copy))
                        .unwrap_or_default();
                    (Arc::new(before.unwrap_or_default()), Arc::new(hunks))
                })
                .await
                .unwrap_or_default();
                Event::Compared(generation, id, request, before, after, hunks)
            },
            |event| Message::Editor(super::Event::Marks(event)),
        )
    }

    pub(super) fn update_marks(&mut self, event: Event) -> Task<Message> {
        let id = self.preview().id;
        match event {
            Event::After(tab, revision)
                if id == tab
                    && self
                        .editor_buffer()
                        .is_some_and(|buffer| buffer.autosave == revision) =>
            {
                return self.refresh_editor_marks();
            }
            Event::Compared(generation, tab, request, before, after, hunks)
                if generation == self.generation
                    && self.module_enabled(crate::modules::ModuleId::Git) =>
            {
                if let Some(buffer) = self
                    .preview_for_request(tab)
                    .and_then(|tab| tab.editor.as_mut())
                    && buffer.marks.request == request
                    && buffer.document.text == after
                {
                    if buffer.marks.before == before && buffer.marks.after == after {
                        return Task::none();
                    }
                    buffer.marks.before = before;
                    buffer.marks.after = after;
                    buffer.marks.hunks = hunks;
                    buffer.marks.open = None;
                }
            }
            Event::Show(tab, index, point) if id == tab => {
                if let Some(buffer) = self
                    .preview_for_request(id)
                    .and_then(|tab| tab.editor.as_mut())
                    && buffer.marks.hunks.get(index).is_some()
                    && buffer.marks.after == buffer.document.text
                {
                    buffer.marks.open = Some((index, point));
                    buffer.marks.page = 0;
                }
            }
            Event::Page(next) => {
                if let Some(marks) = self
                    .preview_for_request(id)
                    .and_then(|tab| tab.editor.as_mut())
                    .map(|buffer| &mut buffer.marks)
                    && let Some((index, _)) = marks.open
                    && let Some(hunk) = marks.hunks.get(index)
                {
                    let pages = (hunk.old.len() + hunk.new.len()).div_ceil(40);
                    marks.page = if next {
                        (marks.page + 1).min(pages.saturating_sub(1))
                    } else {
                        marks.page.saturating_sub(1)
                    };
                }
            }
            Event::Close => {
                if let Some(buffer) = self
                    .preview_for_request(id)
                    .and_then(|tab| tab.editor.as_mut())
                {
                    buffer.marks.open = None;
                }
            }
            _ => {}
        }
        Task::none()
    }
}
