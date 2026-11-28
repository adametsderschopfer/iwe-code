//! Prepare the active Markdown document and route its links.

use super::{App, Preview, Source, State};
use crate::{
    app::Message,
    domain::{external::Request, language::Language, workspace::MarkdownViewMode},
};
use iced::{
    Task,
    widget::{markdown, operation},
};
use std::sync::{Arc, Weak};
use url::{ParseError, Url};

impl Preview {
    pub(in crate::app) fn supports_markdown_view(&self) -> bool {
        let (Some(path), Some(document), Some(source)) =
            (self.path.as_deref(), &self.document, &self.source)
        else {
            return false;
        };
        if document.comparison.is_some()
            || self.pending_comparison.is_some()
            || (self.editor.is_none() && (self.loading || self.error.is_some()))
        {
            return false;
        }
        let plain_file = match source {
            Source::Working { compare, .. } => !*compare,
            Source::Revision { .. } => true,
            Source::Commit { .. } => document.page.is_some(),
            Source::Remote {
                previous,
                local_root,
                ..
            } => previous.is_none() && local_root.is_none(),
        };
        plain_file && Language::for_path(path) == Some(Language::Markdown)
    }

    pub(crate) fn markdown_link(&self, link: String) -> Message {
        if let Some((_, file)) = self.working_paths()
            && !link.starts_with(['/', '\\', '#', '?'])
            && !link.is_empty()
            && matches!(Url::parse(&link), Err(ParseError::RelativeUrlWithoutBase))
            && let Ok(base) = Url::from_file_path(file)
            && let Ok(destination) = base.join(&link)
            && let Ok(path) = destination.to_file_path()
        {
            return Message::External(Request::File { path });
        }
        Message::OpenUrl(link)
    }
}

impl App {
    pub(crate) fn markdown_view_mode(&self) -> Option<MarkdownViewMode> {
        self.preview()
            .supports_markdown_view()
            .then_some(self.workspace.preferences.markdown_view_mode)
    }

    pub(in crate::app) fn set_markdown_view_mode(
        &mut self,
        id: u64,
        mode: MarkdownViewMode,
    ) -> Task<Message> {
        if self.preview().id != id
            || self
                .markdown_view_mode()
                .is_none_or(|current| current == mode)
        {
            return Task::none();
        }
        self.workspace.preferences.markdown_view_mode = mode;
        self.editor.menu = None;
        self.editor.rename = None;
        self.editor.suggestions = None;
        self.editor.definitions.clear();
        let focus = if mode == MarkdownViewMode::Preview {
            Task::none()
        } else if self.preview().editor.is_some() {
            operation::focus(format!("text-editor-{id}"))
        } else {
            self.restore_preview_scroll()
        };
        Task::batch([focus, self.save()])
    }
}

#[derive(Default)]
pub(crate) struct Cache {
    source: Option<Snapshot>,
    pub revision: u64,
    pub items: Vec<markdown::Item>,
}

enum Snapshot {
    Editor(Weak<String>),
    Page([u8; 32]),
}

impl State {
    pub(in crate::app) fn update_markdown(&self, mode: MarkdownViewMode, cache: &mut Cache) {
        let Some(tab) = self
            .files
            .get(self.active)
            .filter(|tab| mode != MarkdownViewMode::Source && tab.supports_markdown_view())
        else {
            if cache.source.is_some() {
                *cache = Cache::default();
            }
            return;
        };

        let (source, text) = if let Some(buffer) = &tab.editor {
            let snapshot = Arc::downgrade(&buffer.document.text);
            if matches!(&cache.source, Some(Snapshot::Editor(previous)) if previous.ptr_eq(&snapshot))
            {
                return;
            }
            (Snapshot::Editor(snapshot), buffer.document.text.as_str())
        } else if let Some(document) = &tab.document {
            if matches!(cache.source, Some(Snapshot::Page(key)) if key == document.fingerprint) {
                return;
            }
            (Snapshot::Page(document.fingerprint), document.text.as_str())
        } else {
            return;
        };
        cache.revision = cache.revision.wrapping_add(1);
        cache.items = markdown::parse(text).collect();
        cache.source = Some(source);
    }
}
